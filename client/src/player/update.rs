use super::{body_transform, PlayerBody};
use crate::{
    camera::CameraController,
    network::{
        buffered_client::PlayerTickInputsBuffer, CurrentPlayerProfile, TargetServer,
        TargetServerState, UnacknowledgedInputs,
    },
    player::{PlayerLabel, PlayerMaterialHandle, LOCAL_PLAYER_COLOR},
    state::GameState,
    ui::assets::MONO_FONT_PATH,
    world::ClientWorldMap,
};
use bevy::color::palettes::css::ORANGE;
use bevy::prelude::*;
use shared::{
    messages::{PlayerFrameInput, PlayerSpawnEvent, PlayerUpdateEvent},
    players::{
        blocks::CallerType, inventory::simulate_inventory_actions,
        movement::simulate_player_movement, Inventory, Player,
    },
    world::ChunkMap,
};

#[derive(Component)]
pub struct CurrentPlayerMarker {}

pub const PLAYER_LABEL_FONT_SIZE: f32 = 24.0;

/// Colour of the body of the other players.
const OTHER_PLAYER_COLOR: Color = Color::srgba(0.0, 0.0, 1.0, 1.0);
/// Where a name label waits, above the origin, until `player_labels_system` places it.
const LABEL_START_POSITION: Vec3 = Vec3::new(0.0, 5.0, 0.0);

/// Spawns the players the server announces. The local player also gets the marker the other
/// systems find it by, and the camera takes the orientation the server saved for it.
pub fn spawn_players_system(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    player_profile: Res<CurrentPlayerProfile>,
    mut ev_spawn: MessageReader<PlayerSpawnEvent>,
    mut target_server: ResMut<TargetServer>,
    players: Query<&Player>,
    assets: Res<AssetServer>,
    mut camera_query: Query<(&mut Transform, &mut CameraController), With<Camera>>,
) {
    // Two events of one frame may announce the same player: the query does not see the
    // first one's entity yet
    let mut known: Vec<_> = players.iter().map(|player| player.id).collect();

    for event in ev_spawn.read() {
        if known.contains(&event.id) {
            debug!(
                "Ignored spawn order, player was already there: {}",
                event.id
            );
            continue;
        }
        known.push(event.id);
        info!("Spawning new player object: {}", event.id);

        let is_current_player = event.id == player_profile.id;
        let player = Player {
            id: event.id,
            name: event.name.clone(),
            position: event.data.position,
            camera_transform: event.data.camera_transform,
            is_flying: event.data.is_flying,
            ..default()
        };

        // One material shared by the mesh and the handle, so the first/third person
        // system mutates the material the player is drawn with
        let material = materials.add(if is_current_player {
            LOCAL_PLAYER_COLOR
        } else {
            OTHER_PLAYER_COLOR
        });
        let body = Cuboid::new(player.width, player.height, player.width);
        let body = (
            PlayerBody,
            Mesh3d(meshes.add(Mesh::from(body))),
            MeshMaterial3d(material.clone()),
            body_transform(player.height, 1.0),
        );
        let mut entity = commands.spawn((
            DespawnOnExit(GameState::Game),
            Transform::from_translation(player.position),
            Visibility::default(),
            PlayerMaterialHandle { handle: material },
            player,
            Name::new(event.name.clone()),
            children![body],
        ));

        if is_current_player {
            target_server.state = TargetServerState::FullyReady;
            entity.insert(CurrentPlayerMarker {});

            for (mut transform, mut controller) in camera_query.iter_mut() {
                *transform = event.data.camera_transform;
                *controller = event.data.camera_transform.rotation.into();
            }
        }

        let player_entity = entity.id();
        spawn_player_label(&mut commands, &assets, player_entity, &event.name);
    }
}

/// The name floating above a player.
fn spawn_player_label(commands: &mut Commands, assets: &AssetServer, player: Entity, name: &str) {
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                ..default()
            },
            PlayerLabel {
                entity: player,
                name: name.to_string(),
            },
            Transform::from_translation(LABEL_START_POSITION),
            DespawnOnExit(GameState::Game),
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new(name),
                TextFont {
                    font: assets.load(MONO_FONT_PATH).into(),
                    font_size: PLAYER_LABEL_FONT_SIZE.into(),
                    ..default()
                },
                TextColor(ORANGE.into()),
                TextLayout::default().with_no_wrap(),
            ));
        });
}

/// Applies the states the server sends: the other players are placed where it says, and the
/// local player's prediction is checked against it and corrected if they differ.
pub fn update_players_system(
    mut players: Query<(&mut Player, &mut Transform)>,
    mut ev_player_update: MessageReader<PlayerUpdateEvent>,
    mut unacknowledged_inputs: ResMut<UnacknowledgedInputs>,
    mut tick_buffer: ResMut<PlayerTickInputsBuffer>,
    client: Res<TargetServer>,
    mut world_map: ResMut<ClientWorldMap>,
    mut inventory: ResMut<Inventory>,
) {
    // No update can concern us before the server gave us our id
    let Some(my_id) = client.session_token else {
        return;
    };

    for event in ev_player_update.read() {
        let Some((mut player, mut transform)) =
            players.iter_mut().find(|(player, _)| player.id == event.id)
        else {
            continue;
        };

        player.health = event.health;
        player.game_mode = event.game_mode;
        player.death_cause = event.death_cause;
        if event.id != my_id {
            player.set_movement_state(event.movement);
            transform.translation = event.movement.position;
            continue;
        }

        reconcile_prediction(
            &mut player,
            event,
            &mut unacknowledged_inputs.0,
            &mut tick_buffer.buffer,
            &world_map.blocks,
        );
        transform.translation = player.position;
        // The inventory and the open chest or furnace resume from the server's, with the actions
        // it has not applied yet
        let Some(server_inventory) = &event.inventory else {
            continue;
        };
        player.inventory = server_inventory.clone();
        if let (Some(chest), Some(contents)) = (server_inventory.open_chest, &event.open_chest) {
            world_map
                .blocks
                .insert_container(chest, (**contents).clone());
        }
        if let (Some(pos), Some(furnace)) = (server_inventory.open_furnace, &event.open_furnace) {
            world_map.blocks.insert_furnace(pos, furnace.clone());
        }
        let not_applied = unacknowledged_inputs
            .0
            .iter()
            .chain(tick_buffer.buffer.iter())
            .filter(|input| input.time_ms > event.last_ack_time);
        for input in not_applied {
            simulate_inventory_actions(
                &mut player,
                &mut world_map.blocks,
                input,
                CallerType::Client,
            );
        }
        inventory.clone_from(&player.inventory);
    }
}

/// Compares what the client predicted after the input `update` acknowledges with what the
/// server computed. When they differ, the prediction resumes from the server's state: every
/// input the server has not simulated yet is replayed on top of it.
fn reconcile_prediction(
    player: &mut Player,
    update: &PlayerUpdateEvent,
    unacknowledged_inputs: &mut Vec<PlayerFrameInput>,
    not_uploaded_inputs: &mut [PlayerFrameInput],
    blocks: &ChunkMap,
) {
    let acknowledged = update.last_ack_time;
    let predicted = unacknowledged_inputs
        .iter()
        .find(|input| input.time_ms == acknowledged)
        .map(|input| input.predicted);

    if predicted.is_none() && !unacknowledged_inputs.is_empty() && acknowledged != 0 {
        // The server acknowledged an input we do not know: forget the inputs it cannot
        // have taken into account.
        warn!(
            "No matching input for last ack time {acknowledged}, resyncing on the server state. Unacknowledged inputs: {:?}",
            unacknowledged_inputs
                .iter()
                .map(|input| input.time_ms)
                .collect::<Vec<_>>()
        );
        unacknowledged_inputs.clear();
    }
    unacknowledged_inputs.retain(|input| input.time_ms >= acknowledged);

    if predicted == Some(update.movement) {
        return;
    }
    if let Some(predicted) = predicted {
        warn!(
            "Player position mismatch: Client({predicted:?}) != Server({:?}) at t={acknowledged} (id={})",
            update.movement, player.id
        );
    }

    // Replay the inputs the server has not simulated yet, those not uploaded yet included, and
    // record what each of them now predicts, which later updates are checked against. Only
    // the movement is replayed: the blocks are not rolled back.
    player.set_movement_state(update.movement);
    let remaining_inputs = unacknowledged_inputs
        .iter_mut()
        .chain(not_uploaded_inputs.iter_mut())
        .filter(|input| input.time_ms >= acknowledged);
    for input in remaining_inputs {
        // The acknowledged input is only given the state the server sent
        if input.time_ms > acknowledged {
            simulate_player_movement(player, blocks, input);
        }
        input.predicted = player.movement_state();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::data::GameAction;
    use crate::network::buffered_client::{CurrentFrameInputs, SyncTime};
    use crate::network::upload_player_inputs_system;
    use crate::player::{
        local_player_is_simulated, player_movement_system, pre_input_update_system,
        LocalPlayerMoved,
    };
    use crate::state::KeyMap;
    use crate::ui::hud::UIMode;
    use bevy_renet::RenetClient;
    use shared::messages::PlayerFrameInput;
    use shared::players::{blocks::CallerType, simulation::simulate_player_actions, MovementState};
    use shared::world::{BlockId, ChunkMap};
    use std::collections::{BTreeMap, VecDeque};

    const PLAYER_ID: u64 = 1;

    fn input(time_ms: u64) -> PlayerFrameInput {
        PlayerFrameInput {
            time_ms,
            ..default()
        }
    }

    fn app_with_player() -> (App, Entity) {
        let mut app = App::new();
        app.add_message::<PlayerUpdateEvent>()
            .init_resource::<ClientWorldMap>()
            .init_resource::<PlayerTickInputsBuffer>()
            .insert_resource(Inventory::new())
            .insert_resource(UnacknowledgedInputs(vec![input(10), input(20)]))
            .insert_resource(TargetServer {
                address: None,
                username: None,
                session_token: Some(PLAYER_ID),
                state: TargetServerState::FullyReady,
            })
            .add_systems(Update, update_players_system);
        let player = app
            .world_mut()
            .spawn((
                Player::new(PLAYER_ID, "me".into(), Vec3::ZERO, Transform::default()),
                Transform::default(),
            ))
            .id();
        (app, player)
    }

    #[test]
    fn an_ack_matching_no_pending_input_resyncs_on_the_server_position() {
        let (mut app, player) = app_with_player();
        let server_position = Vec3::new(1.0, 2.0, 3.0);

        app.world_mut().write_message(PlayerUpdateEvent {
            id: PLAYER_ID,
            movement: MovementState {
                position: server_position,
                ..default()
            },
            orientation: Quat::IDENTITY,
            last_ack_time: 15,
            inventory: Some(Inventory::new()),
            open_chest: None,
            open_furnace: None,
            health: shared::players::game_mode::MAX_HEALTH,
            game_mode: Default::default(),
            death_cause: None,
        });
        app.update();

        assert_eq!(
            app.world().get::<Player>(player).unwrap().position,
            server_position
        );
        assert!(app.world().resource::<UnacknowledgedInputs>().0.is_empty());
    }

    #[test]
    fn unacknowledged_inventory_actions_survive_a_server_update() {
        use shared::players::inventory::InventoryAction;
        use shared::world::{ItemId, ItemStack};

        let (mut app, player) = app_with_player();
        app.world_mut().resource_mut::<UnacknowledgedInputs>().0[1]
            .inventory_actions
            .push(InventoryAction::LeftClickSlot(0));
        let dirt = ItemStack {
            item_id: ItemId::Dirt,
            item_type: ItemId::Dirt.get_default_type(),
            nb: 5,
        };
        let mut server_inventory = Inventory::new();
        server_inventory.inner.insert(0, dirt);

        app.world_mut().write_message(PlayerUpdateEvent {
            id: PLAYER_ID,
            movement: MovementState::default(),
            orientation: Quat::IDENTITY,
            last_ack_time: 10,
            inventory: Some(server_inventory),
            open_chest: None,
            open_furnace: None,
            health: shared::players::game_mode::MAX_HEALTH,
            game_mode: Default::default(),
            death_cause: None,
        });
        app.update();

        let mut expected = Inventory::new();
        expected.floating = Some(dirt);
        assert_eq!(
            app.world().get::<Player>(player).unwrap().inventory,
            expected
        );
        assert_eq!(*app.world().resource::<Inventory>(), expected);
    }

    #[test]
    fn unacknowledged_chest_actions_are_replayed_once_on_the_chest_the_server_sends() {
        use shared::players::inventory::{InventoryAction, CHEST_SLOT_START};
        use shared::world::{Container, ItemId, ItemStack};

        let chest = IVec3::new(0, 0, -2);
        let stone = ItemStack {
            item_id: ItemId::Stone,
            item_type: ItemId::Stone.get_default_type(),
            nb: 4,
        };
        let (mut app, player) = app_with_player();
        let mut blocks = ChunkMap::with_loaded_region(IVec3::splat(-16), IVec3::splat(15));
        blocks.fill_box(chest, chest, BlockId::Chest);
        // The client already predicted the click: the stone is in its cursor, not in its chest
        blocks.insert_container(chest, Container::default());
        app.world_mut().resource_mut::<ClientWorldMap>().blocks = blocks;
        app.world_mut().resource_mut::<UnacknowledgedInputs>().0[1]
            .inventory_actions
            .push(InventoryAction::LeftClickSlot(CHEST_SLOT_START));
        let mut server_inventory = Inventory::new();
        server_inventory.open_chest = Some(chest);
        let mut server_chest = Container::default();
        server_chest.slots[0] = Some(stone);

        app.world_mut().write_message(PlayerUpdateEvent {
            id: PLAYER_ID,
            movement: MovementState::default(),
            orientation: Quat::IDENTITY,
            last_ack_time: 10,
            inventory: Some(server_inventory),
            open_chest: Some(Box::new(server_chest)),
            open_furnace: None,
            health: shared::players::game_mode::MAX_HEALTH,
            game_mode: Default::default(),
            death_cause: None,
        });
        app.update();

        let inventory = &app.world().get::<Player>(player).unwrap().inventory;
        assert_eq!(inventory.floating, Some(stone));
        assert_eq!(inventory.open_chest, Some(chest));
        assert_eq!(
            app.world()
                .resource::<ClientWorldMap>()
                .blocks
                .container(chest),
            Some(&Container::default())
        );
    }

    #[test]
    fn unacknowledged_furnace_actions_are_replayed_once_on_the_furnace_the_server_sends() {
        use shared::players::inventory::{InventoryAction, FURNACE_INPUT_SLOT};
        use shared::world::{Furnace, ItemId, ItemStack};

        let furnace = IVec3::new(0, 0, -2);
        let sand = ItemStack {
            item_id: ItemId::Sand,
            item_type: ItemId::Sand.get_default_type(),
            nb: 4,
        };
        let (mut app, player) = app_with_player();
        let mut blocks = ChunkMap::with_loaded_region(IVec3::splat(-16), IVec3::splat(15));
        blocks.fill_box(furnace, furnace, BlockId::LitFurnace);
        // The client already predicted the click: the sand is in its cursor, not in its furnace
        blocks.insert_furnace(furnace, Furnace::default());
        app.world_mut().resource_mut::<ClientWorldMap>().blocks = blocks;
        app.world_mut().resource_mut::<UnacknowledgedInputs>().0[1]
            .inventory_actions
            .push(InventoryAction::LeftClickSlot(FURNACE_INPUT_SLOT));
        let mut server_inventory = Inventory::new();
        server_inventory.open_furnace = Some(furnace);
        let server_furnace = Furnace {
            input: Some(sand),
            burn_left: 30,
            cook: 12,
            ..Default::default()
        };

        app.world_mut().write_message(PlayerUpdateEvent {
            id: PLAYER_ID,
            movement: MovementState::default(),
            orientation: Quat::IDENTITY,
            last_ack_time: 10,
            inventory: Some(server_inventory),
            open_chest: None,
            open_furnace: Some(server_furnace),
            health: shared::players::game_mode::MAX_HEALTH,
            game_mode: Default::default(),
            death_cause: None,
        });
        app.update();

        let inventory = &app.world().get::<Player>(player).unwrap().inventory;
        assert_eq!(inventory.floating, Some(sand));
        assert_eq!(inventory.open_furnace, Some(furnace));
        // Its input taken out, the furnace has nothing cooking any more
        let expected = Furnace {
            burn_left: 30,
            ..Default::default()
        };
        assert_eq!(
            app.world()
                .resource::<ClientWorldMap>()
                .blocks
                .furnace(furnace),
            Some(&expected)
        );
    }

    #[test]
    fn a_queued_frame_takes_its_inventory_actions_along() {
        use shared::players::inventory::InventoryAction;

        let mut app = App::new();
        app.add_message::<LocalPlayerMoved>()
            .init_resource::<ClientWorldMap>()
            .init_resource::<PlayerTickInputsBuffer>()
            .init_resource::<CurrentFrameInputs>()
            .init_resource::<ButtonInput<KeyCode>>()
            .insert_resource(UIMode::Closed)
            .insert_resource(KeyMap {
                map: BTreeMap::new(),
            })
            .add_systems(Update, player_movement_system);
        app.world_mut().spawn((
            Player::new(PLAYER_ID, "me".into(), Vec3::ZERO, Transform::default()),
            Transform::default(),
            CurrentPlayerMarker {},
        ));
        let mut frame = app.world_mut().resource_mut::<CurrentFrameInputs>();
        frame.0.delta_ms = 16;
        frame.0.inventory_actions.push(InventoryAction::Close);

        app.update();

        let queued = &app.world().resource::<PlayerTickInputsBuffer>().buffer;
        assert_eq!(queued[0].inventory_actions, [InventoryAction::Close]);
        // Taken by the queued frame: the next frame does not send them again
        let frame = app.world().resource::<CurrentFrameInputs>();
        assert!(frame.0.inventory_actions.is_empty());
    }

    /// Frames between two network ticks of the client.
    const FRAMES_PER_TICK: u32 = 3;
    /// Ticks an input takes to reach the server, and an update to come back.
    const LATENCY_TICKS: u32 = 2;
    /// Uneven frame times: what the player rests at depends on them.
    const FRAME_TIMES_MS: [u64; 7] = [16, 17, 15, 18, 16, 21, 16];

    #[derive(Resource, Default)]
    struct Script {
        time_ms: u64,
        delta_ms: u64,
        is_tick: bool,
    }

    /// Gives the frame the scripted time instead of the one of the system clock.
    fn scripted_clock(script: Res<Script>, mut frame: ResMut<CurrentFrameInputs>) {
        frame.0.time_ms = script.time_ms;
        frame.0.delta_ms = script.delta_ms;
    }

    /// A stone floor whose top is at y = 64, in loaded chunks.
    fn flat_world() -> ChunkMap {
        let mut world =
            ChunkMap::with_loaded_region(IVec3::new(-64, 48, -64), IVec3::new(63, 95, 63));
        world.fill_box(
            IVec3::new(-64, 60, -64),
            IVec3::new(63, 63, 63),
            BlockId::Stone,
        );
        world
    }

    /// The frames of the local player and the server simulating its inputs, without the
    /// network in between: the real client systems run in their order, and what they upload is
    /// handed to a server player `LATENCY_TICKS` later, whose updates come back as late.
    struct Session {
        app: App,
        player: Entity,
        server_player: Player,
        server_world: ChunkMap,
        /// Inputs and updates in flight, with the tick they arrive at.
        to_server: VecDeque<(u32, PlayerFrameInput)>,
        to_client: VecDeque<(u32, PlayerUpdateEvent)>,
        uploaded_until: u64,
        time_ms: u64,
        frame: u32,
        /// Whether the server sends each of its updates twice.
        repeats_updates: bool,
        /// Updates acknowledging an input the client had predicted, and those among them
        /// that differ from the prediction.
        checked_updates: u32,
        corrections: u32,
    }

    impl Session {
        fn new(spawn: Vec3) -> Self {
            let mut app = App::new();
            app.add_message::<PlayerUpdateEvent>()
                .add_message::<LocalPlayerMoved>()
                .insert_resource(ClientWorldMap {
                    blocks: flat_world(),
                    ..default()
                })
                .insert_resource(Inventory::new())
                .init_resource::<UnacknowledgedInputs>()
                .init_resource::<PlayerTickInputsBuffer>()
                .init_resource::<CurrentFrameInputs>()
                .init_resource::<SyncTime>()
                .init_resource::<Script>()
                .init_resource::<ButtonInput<KeyCode>>()
                .insert_resource(UIMode::Closed)
                .insert_resource(KeyMap {
                    map: BTreeMap::from([
                        (GameAction::MoveForward, vec![KeyCode::KeyW]),
                        (GameAction::Jump, vec![KeyCode::Space]),
                        (GameAction::ToggleFlyMode, vec![KeyCode::KeyF]),
                    ]),
                })
                .insert_resource(RenetClient::new(shared::get_shared_renet_config()))
                .insert_resource(TargetServer {
                    address: None,
                    username: None,
                    session_token: Some(PLAYER_ID),
                    state: TargetServerState::FullyReady,
                })
                .add_systems(
                    PreUpdate,
                    (
                        pre_input_update_system,
                        scripted_clock,
                        upload_player_inputs_system.run_if(|script: Res<Script>| script.is_tick),
                    )
                        .chain(),
                )
                .add_systems(
                    Update,
                    (
                        player_movement_system.run_if(local_player_is_simulated),
                        update_players_system,
                    )
                        .chain(),
                );
            let player = Player::new(PLAYER_ID, "me".into(), spawn, Transform::default());
            let entity = app
                .world_mut()
                .spawn((player.clone(), Transform::default(), CurrentPlayerMarker {}))
                .id();
            Self {
                app,
                player: entity,
                server_player: player,
                server_world: flat_world(),
                to_server: VecDeque::new(),
                to_client: VecDeque::new(),
                uploaded_until: 0,
                time_ms: 1_000_000,
                frame: 0,
                repeats_updates: false,
                checked_updates: 0,
                corrections: 0,
            }
        }

        fn client_player(&self) -> &Player {
            self.app.world().get::<Player>(self.player).unwrap()
        }

        fn set_client_world(&mut self, world: ChunkMap) {
            self.app.world_mut().resource_mut::<ClientWorldMap>().blocks = world;
        }

        fn hold(&mut self, key: KeyCode, held: bool) {
            let mut keys = self.app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
            if held {
                keys.press(key);
            } else {
                keys.release(key);
            }
        }

        fn server_simulates(&mut self, input: &PlayerFrameInput) {
            simulate_player_actions(
                &mut self.server_player,
                &mut self.server_world,
                input,
                CallerType::Server,
            );
            self.server_player.last_input_processed = input.time_ms;
        }

        /// One tick of the server: the inputs that arrived are simulated, then answered.
        fn server_tick(&mut self, tick: u32) {
            while self.to_server.front().is_some_and(|(at, _)| *at <= tick) {
                let (_, input) = self.to_server.pop_front().unwrap();
                self.server_simulates(&input);
            }
            let update = PlayerUpdateEvent {
                id: PLAYER_ID,
                movement: self.server_player.movement_state(),
                orientation: Quat::IDENTITY,
                last_ack_time: self.server_player.last_input_processed,
                inventory: Some(Inventory::new()),
                open_chest: None,
                open_furnace: None,
                health: self.server_player.health,
                game_mode: self.server_player.game_mode,
                death_cause: self.server_player.death_cause,
            };
            for _ in 0..=u32::from(self.repeats_updates) {
                self.to_client
                    .push_back((tick + LATENCY_TICKS, update.clone()));
            }
        }

        /// Hands the client the next update that arrived, one per frame, after checking it
        /// against what the client predicted for the input it acknowledges.
        fn deliver_update(&mut self, tick: u32) {
            if self.to_client.front().is_none_or(|(at, _)| *at > tick) {
                return;
            }
            let (_, update) = self.to_client.pop_front().unwrap();
            let predicted = self
                .app
                .world()
                .resource::<UnacknowledgedInputs>()
                .0
                .iter()
                .find(|input| input.time_ms == update.last_ack_time)
                .map(|input| input.predicted);
            if let Some(predicted) = predicted {
                self.checked_updates += 1;
                self.corrections += u32::from(predicted != update.movement);
            }
            self.app.world_mut().write_message(update);
        }

        fn frame(&mut self) {
            let delta_ms = FRAME_TIMES_MS[self.frame as usize % FRAME_TIMES_MS.len()];
            self.frame += 1;
            self.time_ms += delta_ms;
            let is_tick = self.frame.is_multiple_of(FRAMES_PER_TICK);
            let tick = self.frame / FRAMES_PER_TICK;
            if is_tick {
                self.server_tick(tick);
            }
            self.deliver_update(tick);

            *self.app.world_mut().resource_mut::<Script>() = Script {
                time_ms: self.time_ms,
                delta_ms,
                is_tick,
            };
            self.app.update();
            self.app
                .world_mut()
                .resource_mut::<ButtonInput<KeyCode>>()
                .clear();

            let uploaded: Vec<_> = self
                .app
                .world()
                .resource::<UnacknowledgedInputs>()
                .0
                .iter()
                .filter(|input| input.time_ms > self.uploaded_until)
                .cloned()
                .collect();
            for input in uploaded {
                self.uploaded_until = input.time_ms;
                self.to_server.push_back((tick + LATENCY_TICKS, input));
            }
        }

        /// Lets the server simulate every input the client has recorded, then checks that the
        /// client is exactly where the server ends up.
        fn assert_the_client_predicted_the_server(mut self) {
            let in_flight: Vec<_> = self.to_server.drain(..).map(|(_, input)| input).collect();
            let not_uploaded = self
                .app
                .world()
                .resource::<PlayerTickInputsBuffer>()
                .buffer
                .clone();
            for input in in_flight.iter().chain(&not_uploaded) {
                self.server_simulates(input);
            }
            assert_eq!(
                self.client_player().movement_state(),
                self.server_player.movement_state()
            );
        }
    }

    /// The whole start of a session: the terrain arrives after the first frames, the player
    /// falls from its spawn, lands, walks and jumps.
    #[test]
    fn a_session_start_is_predicted_without_a_single_correction() {
        let mut session = Session::new(Vec3::new(0.5, 70.0, 0.5));
        session.set_client_world(ChunkMap::default());
        session.hold(KeyCode::KeyW, true);

        for frame in 0..400 {
            if frame == 20 {
                session.set_client_world(flat_world());
            }
            session.hold(KeyCode::Space, (120..200).contains(&frame));
            session.frame();
        }

        assert!(session.checked_updates > 100);
        assert_eq!(session.corrections, 0);
        let end = session.client_player().position;
        assert!(end.z < -20.0 && (end.y - 64.9).abs() < 0.05, "{end:?}");
        session.assert_the_client_predicted_the_server();
    }

    /// A swim across a pool and onto its shore: sinking, swimming up and climbing out are
    /// predicted as the server simulates them.
    #[test]
    fn a_swim_is_predicted_without_a_single_correction() {
        let pool = || {
            let mut world = flat_world();
            world.fill_box(
                IVec3::new(-64, 64, -64),
                IVec3::new(63, 66, 63),
                BlockId::Water,
            );
            // The shore, one block above the water
            world.fill_box(
                IVec3::new(-64, 64, -64),
                IVec3::new(63, 67, -20),
                BlockId::Stone,
            );
            world
        };
        let mut session = Session::new(Vec3::new(0.5, 70.0, 0.5));
        session.set_client_world(pool());
        session.server_world = pool();
        session.hold(KeyCode::KeyW, true);

        for frame in 0..600 {
            session.hold(KeyCode::Space, (150..450).contains(&frame));
            session.frame();
        }

        assert!(session.checked_updates > 150);
        assert_eq!(session.corrections, 0);
        let end = session.client_player().position;
        assert!(end.z < -20.0 && (end.y - 68.9).abs() < 0.05, "{end:?}");
        session.assert_the_client_predicted_the_server();
    }

    /// The server moves the player by itself, and the player toggles the fly mode before
    /// hearing of it: one correction, from which the pending inputs are replayed exactly.
    fn a_move_made_by_the_server_alone(repeats_updates: bool) {
        let mut session = Session::new(Vec3::new(0.5, 64.9, 0.5));
        session.repeats_updates = repeats_updates;
        // A creative player, which may fly, as the client learns from the first update
        session.server_player.game_mode = shared::players::game_mode::GameMode::Creative;
        session.hold(KeyCode::KeyW, true);

        for frame in 0..200 {
            if frame == 100 {
                session.server_player.position.y += 3.0;
                session.hold(KeyCode::KeyF, true);
            }
            session.hold(KeyCode::Space, (101..140).contains(&frame));
            session.frame();
        }

        assert_eq!(session.corrections, 1);
        assert!(session.client_player().is_flying);
        session.assert_the_client_predicted_the_server();
    }

    #[test]
    fn a_move_made_by_the_server_alone_is_corrected_once() {
        a_move_made_by_the_server_alone(false);
    }

    #[test]
    fn a_repeated_acknowledgement_does_not_repeat_the_correction() {
        a_move_made_by_the_server_alone(true);
    }
}
