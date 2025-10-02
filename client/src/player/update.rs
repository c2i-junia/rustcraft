use crate::{
    camera::CameraController,
    network::{CurrentPlayerProfile, TargetServer, TargetServerState, UnacknowledgedInputs},
    player::{PlayerLabel, PlayerMaterialHandle},
    world::ClientWorldMap,
    GameState,
};
use bevy::color::palettes::css::ORANGE;
use bevy::prelude::*;
use shared::{
    messages::{PlayerSpawnEvent, PlayerUpdateEvent},
    players::{simulation::simulate_player_actions, blocks::CallerType, Inventory, Player},
};

#[derive(Component)]
pub struct CurrentPlayerMarker {}

pub const PLAYER_LABEL_FONT_SIZE: f32 = 24.0;

pub fn spawn_players_system(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    player_profile: Res<CurrentPlayerProfile>,
    mut ev_spawn: EventReader<PlayerSpawnEvent>,
    mut target_server: ResMut<TargetServer>,
    players: Query<&Player>,
    assets: Res<AssetServer>,
    mut camera_query: Query<(&mut Transform, &mut CameraController), With<Camera>>,
) {
    let current_id = player_profile.into_inner().id;
    'event_loop: for event in ev_spawn.read() {
        info!("Executing spawn player for event: {:?}", event);
        for player in players.iter() {
            if player.id == event.id {
                info!(
                    "Ignored spawn order, player was already there: {}",
                    player.id
                );
                continue 'event_loop;
            }
        }
        let is_current_player = event.id == current_id;
        let player = Player {
            id: event.id,
            name: event.name.clone(),
            position: event.data.position,
            camera_transform: event.data.camera_transform,
            is_flying: event.data.is_flying,
            ..default()
        };

        let color = if is_current_player {
            Color::srgba(1.0, 0.0, 0.0, 1.0)
        } else {
            Color::srgba(0.0, 0.0, 1.0, 1.0)
        };

        info!("Spawning new player object: {}", player.id);

        let player_name = event.name.clone();

        let mut entity = commands.spawn((
            StateScoped(GameState::Game),
            Transform::from_translation(player.position),
            Visibility::default(),
            Mesh3d(meshes.add(Mesh::from(Cuboid::new(
                player.width,
                player.height,
                player.width,
            )))),
            MeshMaterial3d(materials.add(color)),
            PlayerMaterialHandle {
                handle: materials.add(color),
            },
            player.clone(),
            Name::new(player_name.clone()),
        ));

        let text_style = TextFont {
            font: assets.load("fonts/FiraMono-Medium.ttf"),
            font_size: PLAYER_LABEL_FONT_SIZE,
            ..default()
        };

        let label_text_style = (text_style.clone(), TextColor(ORANGE.into()));

        if is_current_player {
            target_server.state = TargetServerState::FullyReady;
            entity.insert(CurrentPlayerMarker {});
            info!("Inserted current player marker");

            info!("aaa ---");
            for (transform, controller) in camera_query.iter_mut() {
                *transform.into_inner() = event.data.camera_transform;
                *controller.into_inner() = event.data.camera_transform.rotation.into();

                info!(
                    "Setting camera transform: {:?}",
                    event.data.camera_transform
                );
            }
            info!("bbb ---");
        }

        let entity_id = entity.id();

        commands
            .spawn((
                Node {
                    position_type: PositionType::Absolute,
                    ..default()
                },
                PlayerLabel {
                    entity: entity_id,
                    name: player_name.clone(),
                },
                Transform::from_translation(Vec3::new(0.0, 5.0, 0.0)),
                StateScoped(GameState::Game),
            ))
            .with_children(|parent| {
                parent.spawn((
                    Text::new(player_name),
                    label_text_style.clone(),
                    TextLayout::default().with_no_wrap(),
                ));
            });
    }
}

pub fn update_players_system(
    mut players: Query<(&mut Player, &mut Transform)>,
    mut ev_player_update: EventReader<PlayerUpdateEvent>,
    mut unacknowledged_inputs: ResMut<UnacknowledgedInputs>,
    client: Res<TargetServer>,
    world_map: ResMut<ClientWorldMap>,
    mut inventory: ResMut<Inventory>,
) {
    let my_id = client.session_token.unwrap();

    let world_map = world_map.into_inner();

    // Read all updates
    for event in ev_player_update.read() {
        // Get the player associated with the event
        for (mut player, mut transform) in players.iter_mut() {
            if player.id == event.id && event.id == my_id {
                player.inventory = event.inventory.clone();
                inventory.inner = event.inventory.inner.clone();

                // Get the local input matching this update event
                let matching_input = unacknowledged_inputs
                    .0
                    .iter()
                    .find(|input| input.time_ms == event.last_ack_time);

                if let Some(matching_input) = matching_input {
                    let does_position_match = event.position == matching_input.position;

                    if !does_position_match {
                        warn!(
                            "Player position mismatch: Client({:?}) != Server({:?}) at t={} (id={})",
                            event.position, matching_input.position, matching_input.time_ms, player.id
                        );

                        // Reconcile the player position
                        player.position = event.position;

                        let remaining_inputs = unacknowledged_inputs
                            .0
                            .iter()
                            .filter(|input| input.time_ms > event.last_ack_time)
                            .cloned()
                            .collect::<Vec<_>>();

                        for input in remaining_inputs.iter() {
                            // debug!("Reapplying input: {:?}", input);
                            simulate_player_actions(&mut player, world_map, input, CallerType::Client);
                        }

                        debug!(
                            "final player position: {:?} after reapplying {} inputs",
                            player.position,
                            remaining_inputs.len()
                        );
                    }
                } else {
                    debug!(
                        "No matching input found for last ack time: {} | {:?}",
                        event.last_ack_time, unacknowledged_inputs
                    );
                    player.position = event.position;
                    if !unacknowledged_inputs.0.is_empty() && event.last_ack_time != 0 {
                        warn!(
                            "Unacknowledged inputs: {:?}",
                            unacknowledged_inputs
                                .0
                                .iter()
                                .map(|input| input.time_ms)
                                .collect::<Vec<_>>()
                        );
                        panic!("No matching input found for last ack time but there are unacknowledged inputs {:?}", event.last_ack_time);
                    }
                }

                *unacknowledged_inputs = UnacknowledgedInputs(
                    unacknowledged_inputs
                        .0
                        .iter()
                        .filter(|input| input.time_ms >= event.last_ack_time)
                        .cloned()
                        .collect(),
                );
            } else if player.id != my_id && player.id == event.id {
                debug!(
                    "Corrected player position: {:?} => {:?}",
                    player.id, event.position
                );
                player.position = event.position;
                *transform = Transform::from_translation(event.position);
            }
        }
    }
}
