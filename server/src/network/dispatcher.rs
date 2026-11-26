use crate::init::{ServerLobby, ServerTime};
use crate::mob::behavior::mob_behavior_system;
use crate::mods::Mods;
use crate::network::broadcast_chat::*;
use crate::world;
use crate::world::broadcast_world::broadcast_world_state;
use crate::world::data::ServerWorldMap;
use crate::world::dropped_items::{
    drop_on_death_system, dropped_items_physics_system, pickup_dropped_items_system,
};
use crate::world::generation::WorldGeneration;
use crate::world::locate::Locator;
use crate::world::save::SaveRequestEvent;
use crate::world::simulation::{
    breathe_system, broadcast_player_updates_system, burn_system, handle_player_inputs_system,
    metabolism_system, status_effects_system, tick_damage_timers_system, PlayerInputsEvent,
};
use crate::world::weather::ServerWeather;
use bevy::prelude::*;
use bevy_renet::renet::{ClientId, DisconnectReason, ServerEvent};
use bevy_renet::{RenetReceive, RenetServer, RenetServerEvent};
use shared::messages::{ChatConversation, ClientToServerMessage};
use shared::world::WorldSeed;
use shared::{
    GameFolderPaths, GameServerConfig, CTS_AUTH_CHANNEL, CTS_STANDARD_CHANNEL, MAX_INPUTS_PER_TICK,
    TICKS_PER_SECOND,
};
use std::collections::HashMap;

use super::extensions::SendGameMessageExtension;
use super::handlers::{self, Access, Clients, Join};
use super::join::{
    dismiss_late_clients_system, dismiss_revoked_players_system, send_challenges_system,
    AuthTimeout, Challenges, ServerKey,
};
use super::permissions::{disconnect_dismissed_system, Dismissals, Permissions};
use super::profiles::{finish_lookups_system, ProfileLookups};

/// bevy_renet reports connections as observer events: this buffers them as a message so that
/// `handle_connection_events_system` still reads them in `Update`. The event is copied field by field
/// because `ServerEvent` is not `Clone`.
#[derive(Message)]
pub(super) enum ServerEventMessage {
    ClientConnected {
        client_id: ClientId,
    },
    ClientDisconnected {
        client_id: ClientId,
        reason: DisconnectReason,
    },
}

fn buffer_server_event(
    event: On<RenetServerEvent>,
    mut messages: MessageWriter<ServerEventMessage>,
) {
    messages.write(match event.0 {
        ServerEvent::ClientConnected { client_id } => {
            ServerEventMessage::ClientConnected { client_id }
        }
        ServerEvent::ClientDisconnected { client_id, reason } => {
            ServerEventMessage::ClientDisconnected { client_id, reason }
        }
    });
}

pub fn setup_resources_and_events(app: &mut App) {
    app.add_message::<SaveRequestEvent>()
        .add_message::<PlayerInputsEvent>()
        .add_message::<ServerEventMessage>()
        .init_resource::<JoinRequests>()
        .init_resource::<world::flow::LiquidFlow>()
        .init_resource::<world::ticks::BlockTicks>()
        .init_resource::<Locator>()
        // a dedicated server replaces them with those of its files (`init::run`)
        .init_resource::<Permissions>()
        .init_resource::<Dismissals>()
        .init_resource::<ProfileLookups>()
        .init_resource::<Challenges>()
        .init_resource::<AuthTimeout>()
        // a server replaces it with the one of its file (`init::run`)
        .init_resource::<ServerKey>()
        .add_observer(buffer_server_event);

    setup_chat_resources(app);
}

/// The stages of a server tick, in this order. A save is done on the tick of its request and
/// holds every change made until then, which is why the broadcast (it drains the change log
/// into the dirty chunks) comes before it.
///
/// There is no stage for chunk generation: it happens in `handle_player_inputs_system`, before
/// and after each batch of inputs, so that a falling player is never held on an ungenerated
/// chunk.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum ServerSet {
    /// Connection events and client messages.
    Receive,
    /// The player inputs are applied to the world, then the furnaces smelt, the water
    /// flows and the weather changes.
    Simulate,
    /// The world changes are sent to the clients.
    Broadcast,
    /// The world is handed to the writer.
    Save,
}

/// Registers the systems of a tick in `FixedUpdate`, in the order of `ServerSet`. The loop of
/// the server turns faster than the ticks: the transport sends and receives on every turn, so
/// that the round trips of a connection do not each wait for a tick, and a tick runs on the
/// turns its time has come.
pub fn register_systems(app: &mut App) {
    app.configure_sets(
        FixedUpdate,
        (
            ServerSet::Receive,
            ServerSet::Simulate,
            ServerSet::Broadcast,
            ServerSet::Save,
        )
            .chain(),
    )
    .add_systems(
        FixedUpdate,
        (
            handle_connection_events_system,
            handle_client_messages_system,
            finish_lookups_system,
        )
            .chain()
            .in_set(ServerSet::Receive),
    )
    .add_systems(
        FixedUpdate,
        (
            tick_damage_timers_system,
            handle_player_inputs_system,
            // The mounts go where their riders took them
            crate::mob::riding::riding_system,
            // Once the blows and the arrows of the tick landed
            crate::mob::boss::boss_system,
            breathe_system,
            burn_system,
            status_effects_system,
            world::furnaces::tick_furnaces_system,
            metabolism_system,
            crate::mob::villager::trade_system,
            // After every source of damage of the tick, the mobs' included
            drop_on_death_system,
            pickup_dropped_items_system,
            world::depths::announce_depths_system,
            world::progress::progress_system,
            (
                world::home::home_system,
                world::cartography::cartography_system,
            )
                .chain(),
            // Fires, lava, plants, cisterns and campfires; before the flow, which washes the
            // fires away and fills what they burnt
            world::ticks::block_ticks_system,
            world::flow::liquid_flow_system,
            world::weather::weather_system,
            // After every block change of the tick
            world::portals::remove_unfit_portals_system,
        )
            .chain()
            .in_set(ServerSet::Simulate),
    )
    .add_systems(
        FixedUpdate,
        (
            broadcast_player_updates_system,
            broadcast_world_state,
            world::portals::broadcast_portals_system,
        )
            .in_set(ServerSet::Broadcast),
    )
    .add_systems(
        FixedUpdate,
        (
            super::realm::realm_system.run_if(resource_exists::<super::realm::RealmEvents>),
            crate::shutdown::shutdown_system,
            world::save::save_world_system,
            world::unload::unload_far_chunks_system,
        )
            .chain()
            .in_set(ServerSet::Save),
    );

    app.add_systems(FixedPostUpdate, update_server_time)
        .add_systems(Update, world::locate::answer_locates_system)
        .add_systems(
            Update,
            (
                dismiss_late_clients_system,
                dismiss_revoked_players_system,
                disconnect_dismissed_system,
            )
                .chain(),
        )
        .add_systems(
            PreUpdate,
            (send_challenges_system, take_join_requests_system)
                .chain()
                .after(RenetReceive),
        );

    // The mobs and the items move before the inputs of the tick are read
    app.add_systems(
        FixedUpdate,
        (
            (
                crate::mob::villager::villager_day_system,
                mob_behavior_system,
                crate::mob::cinder::cinder_system,
                crate::mob::arrow::arrow_system,
                crate::mob::fishing::fishing_system,
                crate::mob::spawning::hostile_spawning_system,
                crate::mob::wildlife::wildlife_system,
                crate::mob::guards::dungeon_guards_system,
            )
                .chain(),
            dropped_items_physics_system,
        )
            .before(ServerSet::Receive),
    );
}

fn handle_connection_events_system(
    mut server_events: MessageReader<ServerEventMessage>,
    mut lobby: ResMut<ServerLobby>,
    mut world_map: ResMut<ServerWorldMap>,
    mut ev_save_request: MessageWriter<SaveRequestEvent>,
    mut dismissals: ResMut<Dismissals>,
) {
    for event in server_events.read() {
        match event {
            ServerEventMessage::ClientConnected { client_id } => {
                handlers::client_connected(*client_id);
            }
            ServerEventMessage::ClientDisconnected { client_id, reason } => {
                dismissals.forget(*client_id);
                handlers::client_disconnected(
                    client_id,
                    reason,
                    &mut lobby,
                    &mut world_map,
                    &mut ev_save_request,
                );
            }
        }
    }
}

/// The requests to join that came between two ticks, by client: they are handled by the next
/// tick, after the other messages the client sent before them.
#[derive(Resource, Default)]
pub struct JoinRequests(HashMap<ClientId, Vec<ClientToServerMessage>>);

/// Takes the requests to join of the clients not yet in the lobby as soon as they arrive, and
/// has a tick run in this turn of the loop: a joining player gets its chunks without waiting
/// for the next tick. That tick comes early once, which shifts the following ones.
fn take_join_requests_system(
    mut server: ResMut<RenetServer>,
    lobby: Res<ServerLobby>,
    mut requests: ResMut<JoinRequests>,
    mut fixed: ResMut<Time<Fixed>>,
) {
    let mut asked = false;
    for client_id in server.clients_id() {
        if lobby.players.contains_key(&client_id) {
            continue;
        }
        while let Some(Ok(message)) =
            server.receive_game_message_by_channel(client_id, CTS_AUTH_CHANNEL)
        {
            requests.0.entry(client_id).or_default().push(message);
            asked = true;
        }
    }
    if asked {
        let missing = fixed.timestep().saturating_sub(fixed.overstep());
        fixed.accumulate_overstep(missing);
    }
}

/// Reads what every client sent and calls the handler of each message.
fn handle_client_messages_system(
    (mut server, mut chat_conversation, mut lobby): (
        ResMut<RenetServer>,
        ResMut<ChatConversation>,
        ResMut<ServerLobby>,
    ),
    (mut ev_app_exit, mut ev_save_request, mut ev_player_inputs): (
        MessageWriter<AppExit>,
        MessageWriter<SaveRequestEvent>,
        MessageWriter<PlayerInputsEvent>,
    ),
    config: Res<GameServerConfig>,
    mut world_map: ResMut<ServerWorldMap>,
    mut time: ResMut<ServerTime>,
    (seed, generation, locator): (Res<WorldSeed>, Option<Res<WorldGeneration>>, Res<Locator>),
    game_folder_paths: Res<GameFolderPaths>,
    mods: Res<Mods>,
    mut join_requests: ResMut<JoinRequests>,
    mut weather: ResMut<ServerWeather>,
    (mut permissions, mut dismissals, mut lookups): (
        ResMut<Permissions>,
        ResMut<Dismissals>,
        ResMut<ProfileLookups>,
    ),
    (challenges, server_key): (Res<Challenges>, Res<ServerKey>),
) {
    // the base game's, for the apps of the tests that set up no world
    let default_generation = WorldGeneration::default();
    for client_id in server.clients_id() {
        let mut input_budget = MAX_INPUTS_PER_TICK;
        let mut dropped_inputs = 0;
        // The order of `receive_game_message`, with the requests to join taken early
        let mut joins = join_requests
            .0
            .remove(&client_id)
            .unwrap_or_default()
            .into_iter();
        while let Some(Ok(message)) = server
            .receive_game_message_by_channel(client_id, CTS_STANDARD_CHANNEL)
            .or_else(|| joins.next().map(Ok))
            .or_else(|| server.receive_game_message(client_id))
        {
            if !may_send(&message, client_id, &lobby, &config) {
                warn!(
                    "Ignoring {} message from unauthenticated client {}",
                    message_kind(&message),
                    client_id
                );
                continue;
            }

            // A dismissed client is still connected, for its message to reach it, and does
            // nothing more: an operator being banned could pardon itself. A solo player must
            // still be able to stop its server.
            if dismissals.contains(client_id)
                && !(config.is_solo && matches!(message, ClientToServerMessage::Exit))
            {
                continue;
            }

            let clients = Clients {
                server: &mut server,
                lobby: &mut lobby,
            };
            match message {
                ClientToServerMessage::AuthRegisterRequest(auth_req) => {
                    let flow = handlers::auth_register_request(
                        clients,
                        client_id,
                        auth_req,
                        &mut world_map,
                        &chat_conversation,
                        &time,
                        (*seed, generation.as_deref().unwrap_or(&default_generation)),
                        &game_folder_paths,
                        config.max_render_distance,
                        &mods,
                        &weather,
                        Access {
                            permissions: &mut permissions,
                            dismissals: &mut dismissals,
                            is_solo: config.is_solo,
                            auth: &config.auth,
                            lookups: &mut lookups,
                        },
                        Join {
                            auth: &config.auth,
                            nonce: challenges.get(client_id),
                            server_key: &server_key.0,
                        },
                    );
                    // The client was refused: drop the rest of its messages
                    if flow.is_break() {
                        break;
                    }
                }
                ClientToServerMessage::ChatMessage(chat_msg) => {
                    handlers::chat_message(
                        clients,
                        client_id,
                        chat_msg,
                        &mut chat_conversation,
                        &mut world_map,
                        &mut time,
                        &mut weather,
                        &mods,
                        (
                            *seed,
                            generation.as_deref().unwrap_or(&default_generation),
                            &locator,
                        ),
                        Access {
                            permissions: &mut permissions,
                            dismissals: &mut dismissals,
                            is_solo: config.is_solo,
                            auth: &config.auth,
                            lookups: &mut lookups,
                        },
                    );
                }
                ClientToServerMessage::Exit => {
                    handlers::exit(
                        clients,
                        client_id,
                        &config,
                        &mut ev_save_request,
                        &mut ev_app_exit,
                    );
                }
                ClientToServerMessage::PlayerInputs(inputs) => {
                    dropped_inputs += handlers::player_inputs(
                        client_id,
                        inputs,
                        &mut input_budget,
                        &mut ev_player_inputs,
                    );
                }
                ClientToServerMessage::SaveWorldRequest => {
                    handlers::save_world_request(client_id, &config, &mut ev_save_request);
                }
                ClientToServerMessage::SetRenderDistance(asked) => {
                    handlers::set_render_distance(
                        client_id,
                        asked,
                        &mut world_map,
                        config.max_render_distance,
                    );
                }
                ClientToServerMessage::Respawn => {
                    handlers::respawn(client_id, &mut world_map);
                }
                ClientToServerMessage::Trade { mob, offer } => {
                    world_map.trades.ask(client_id, mob, offer);
                }
                ClientToServerMessage::FirstStep(step) => {
                    if let Some(player) = world_map.players.get_mut(&client_id) {
                        player.learn(step);
                    }
                }
                ClientToServerMessage::EndTrade => {
                    world_map.trades.end(client_id);
                }
                ClientToServerMessage::WriteSign { at, lines } => {
                    crate::world::home::write_sign_lines(&mut world_map, client_id, at, lines);
                }
                ClientToServerMessage::PlaceMarker { at, name, color } => {
                    if let Some(player) = world_map.players.get_mut(&client_id) {
                        player.waypoints.mark(&name, color, at);
                    }
                }
                ClientToServerMessage::RemoveMarker(index) => {
                    if let Some(player) = world_map.players.get_mut(&client_id) {
                        player.waypoints.unmark(usize::from(index));
                    }
                }
            }
        }
        if dropped_inputs > 0 {
            warn!(
                "Dropped {dropped_inputs} inputs of client {client_id}: more than {MAX_INPUTS_PER_TICK} in one tick"
            );
        }
    }
    // Those of the clients that left
    join_requests.0.clear();
}

/// Whether the server reads `message` from this client. A client that is not authenticated
/// may only ask to be.
fn may_send(
    message: &ClientToServerMessage,
    client_id: ClientId,
    lobby: &ServerLobby,
    config: &GameServerConfig,
) -> bool {
    let is_auth_request = matches!(message, ClientToServerMessage::AuthRegisterRequest(_));
    // In solo mode the only client is the local player: it must be able to stop its
    // server even before the authentication went through (cancelled connection).
    let is_solo_exit = config.is_solo && matches!(message, ClientToServerMessage::Exit);
    is_auth_request || is_solo_exit || lobby.players.contains_key(&client_id)
}

/// Name of the variant of `message`, to log what a client sent without dumping its payload.
fn message_kind(message: &ClientToServerMessage) -> &'static str {
    match message {
        ClientToServerMessage::AuthRegisterRequest(_) => "AuthRegisterRequest",
        ClientToServerMessage::ChatMessage(_) => "ChatMessage",
        ClientToServerMessage::Exit => "Exit",
        ClientToServerMessage::PlayerInputs(_) => "PlayerInputs",
        ClientToServerMessage::SaveWorldRequest => "SaveWorldRequest",
        ClientToServerMessage::SetRenderDistance(_) => "SetRenderDistance",
        ClientToServerMessage::Respawn => "Respawn",
        ClientToServerMessage::Trade { .. } => "Trade",
        ClientToServerMessage::EndTrade => "EndTrade",
        ClientToServerMessage::FirstStep(_) => "FirstStep",
        ClientToServerMessage::WriteSign { .. } => "WriteSign",
        ClientToServerMessage::PlaceMarker { .. } => "PlaceMarker",
        ClientToServerMessage::RemoveMarker(_) => "RemoveMarker",
    }
}

fn update_server_time(mut time: ResMut<ServerTime>) {
    if time.0.is_multiple_of(5 * TICKS_PER_SECOND) {
        debug!("Server time: {}", time.0);
    }
    time.0 += 1;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::chunk_files::{read_chunks, CHUNKS_DIR};
    use crate::world::simulation::PlayerInputsEvent;
    use crate::world::writer::WorldWriter;
    use bevy::time::{TimePlugin, TimeUpdateStrategy};
    use shared::messages::{NetworkAction, PlayerFrameInput};
    use shared::players::{blocks::HIT_MS, Player};
    use shared::world::{BlockData, BlockDirection, BlockId, ChunkMap, WorldSeed};
    use std::time::Duration;

    const TICK: Duration = Duration::from_millis(1000 / TICKS_PER_SECOND);

    /// A server whose world is `world_map` and whose files are in `folder`, its clock moving by
    /// `update` on each update.
    fn server_app(folder: &std::path::Path, world_map: ServerWorldMap, update: Duration) -> App {
        let mut app = App::new();
        app.add_plugins(TimePlugin)
            .insert_resource(TimeUpdateStrategy::ManualDuration(update))
            .insert_resource(RenetServer::new(shared::get_shared_renet_config()))
            .insert_resource(ServerLobby::default())
            .insert_resource(GameServerConfig {
                world_name: "w".into(),
                is_solo: true,
                saves: None,
                max_render_distance: shared::DEFAULT_MAX_RENDER_DISTANCE,
                max_players: shared::MAX_PLAYERS,
                new_world: Default::default(),
                auth: Default::default(),
            })
            .insert_resource(GameFolderPaths {
                game_folder_path: folder.join("game"),
                assets_folder_path: folder.join("assets"),
            })
            .insert_resource(world_map)
            .insert_resource(WorldSeed(1))
            .insert_resource(crate::world::save::Autosave { last_save_tick: 0 })
            .insert_resource(ServerTime(0))
            .insert_resource(ServerWeather::new(&mut rand::thread_rng()))
            .insert_resource(Mods::new().unwrap())
            .insert_resource(WorldWriter::spawn(folder.into(), None).unwrap());
        setup_resources_and_events(&mut app);
        register_systems(&mut app);
        app
    }

    /// A player who asks to join between two ticks is accepted at once, and a tick runs in the
    /// same update to send it its chunks.
    #[test]
    fn a_player_joins_and_gets_a_tick_without_waiting_for_the_next_one() {
        let tmp = tempfile::tempdir().unwrap();
        let world_map = ServerWorldMap {
            name: "w".into(),
            ..default()
        };
        let mut app = server_app(tmp.path(), world_map, Duration::from_millis(1));
        // the first update only starts the clock
        app.update();
        let mut client = app
            .world_mut()
            .resource_mut::<RenetServer>()
            .new_local_client(7);
        client.send_message(
            CTS_AUTH_CHANNEL,
            shared::game_message_to_payload(ClientToServerMessage::AuthRegisterRequest(
                shared::messages::AuthRegisterRequest {
                    username: "p".into(),
                    render_distance: 1,
                    skin: Default::default(),
                    proof: None,
                },
            )),
        );
        app.world_mut()
            .resource_mut::<RenetServer>()
            .process_local_client(7, &mut client)
            .unwrap();

        app.update();

        assert!(app
            .world()
            .resource::<ServerLobby>()
            .players
            .contains_key(&7));
        assert_eq!(app.world().resource::<ServerTime>().0, 1);
        let chunks = &app.world().resource::<ServerWorldMap>().sent_chunks;
        assert!(chunks.get(&7).is_some_and(|sent| !sent.chunks.is_empty()));

        // the next tick comes a whole tick later
        app.update();
        assert_eq!(app.world().resource::<ServerTime>().0, 1);
        drop(app.world_mut().remove_resource::<WorldWriter>());
    }

    /// A client that connects and never asks to join is dismissed once `AuthTimeout` passed,
    /// whatever the mode: it would hold a place, or the id of a player, for ever.
    #[test]
    fn a_client_that_does_not_ask_to_join_in_time_is_dismissed() {
        let tmp = tempfile::tempdir().unwrap();
        let world_map = ServerWorldMap {
            name: "w".into(),
            ..default()
        };
        let mut app = server_app(tmp.path(), world_map, Duration::from_millis(100));
        app.insert_resource(AuthTimeout(Duration::from_millis(250)));
        app.update();
        let _client = app
            .world_mut()
            .resource_mut::<RenetServer>()
            .new_local_client(7);
        // the app of the tests has no transport to report it
        app.world_mut()
            .write_message(ServerEventMessage::ClientConnected { client_id: 7 });
        app.update();
        assert!(app.world().resource::<Challenges>().get(7).is_some());
        assert!(!app.world().resource::<Dismissals>().contains(7));

        for _ in 0..3 {
            app.update();
        }
        assert!(app.world().resource::<Dismissals>().contains(7));
        drop(app.world_mut().remove_resource::<WorldWriter>());
    }

    /// The inputs that break a block and the exit arrive in the same frame: the block change
    /// made by the inputs is in the saved chunk.
    #[test]
    fn a_block_broken_in_the_frame_of_the_exit_is_in_the_exit_save() {
        let tmp = tempfile::tempdir().unwrap();
        let leaves = BlockData::new(BlockId::OakLeaves, BlockDirection::Front);
        let target = IVec3::ZERO;
        let mut chunks = ChunkMap::with_loaded_region(IVec3::splat(-16), IVec3::splat(15));
        chunks.fill_box(IVec3::splat(-16), IVec3::new(15, -1, 15), BlockId::Stone);
        chunks.set_block(target, leaves);
        chunks.take_changed_blocks();
        let player_position = Vec3::new(0.5, 1.0, 0.5);
        let mut world_map = ServerWorldMap {
            name: "w".into(),
            chunks,
            ..default()
        };
        world_map.players.insert(
            1,
            Player::new(1, "p".into(), player_position, Transform::default()),
        );

        // each update is a tick
        let mut app = server_app(tmp.path(), world_map, TICK);

        let camera = Transform::from_translation(player_position).looking_to(Vec3::NEG_Y, Vec3::Z);
        let hits = leaves.id.get_break_time().unwrap();
        for time_ms in 1..=u64::from(hits) {
            app.world_mut().write_message(PlayerInputsEvent {
                client_id: 1,
                input: PlayerFrameInput {
                    time_ms,
                    delta_ms: HIT_MS,
                    inputs: [NetworkAction::LeftClick].into(),
                    camera,
                    ..default()
                },
            });
        }
        app.world_mut().write_message(SaveRequestEvent::Player(1));

        // the first update only starts the clock
        app.update();
        app.update();
        drop(app.world_mut().remove_resource::<WorldWriter>());

        assert!(app
            .world()
            .resource::<ServerWorldMap>()
            .chunks
            .get_block(target)
            .is_none());
        let saved = read_chunks(&tmp.path().join(CHUNKS_DIR)).unwrap();
        let chunk = saved
            .get(&IVec3::ZERO)
            .expect("the chunk of the change was not saved");
        assert!(chunk.get(target).is_none(), "the block is back in the save");
    }

    /// The server measures how much the inputs of a sprint tire the player, as the client's
    /// prediction only moves it.
    #[test]
    fn a_sprint_simulated_by_the_server_tires_the_player() {
        let tmp = tempfile::tempdir().unwrap();
        let mut chunks = ChunkMap::with_loaded_region(IVec3::splat(-32), IVec3::splat(31));
        chunks.fill_box(IVec3::splat(-32), IVec3::new(31, -1, 31), BlockId::Stone);
        let start = Vec3::new(0.5, 0.9, 0.5);
        let mut world_map = ServerWorldMap {
            name: "w".into(),
            chunks,
            ..default()
        };
        world_map
            .players
            .insert(1, Player::new(1, "p".into(), start, Transform::default()));
        let mut app = server_app(tmp.path(), world_map, TICK);
        for time_ms in 1..=20 {
            app.world_mut().write_message(PlayerInputsEvent {
                client_id: 1,
                input: PlayerFrameInput {
                    time_ms,
                    delta_ms: 50,
                    inputs: [NetworkAction::MoveForward, NetworkAction::Sprint].into(),
                    ..default()
                },
            });
        }
        app.update();
        app.update();
        drop(app.world_mut().remove_resource::<WorldWriter>());

        let player = &app.world().resource::<ServerWorldMap>().players[&1];
        assert!(player.is_sprinting);
        let run = start.xz().distance(player.position.xz());
        assert!(run > 5.0, "ran {run} blocks");
        let expected = run * shared::players::hunger::SPRINT_EXHAUSTION;
        assert!(
            (player.exhaustion - expected).abs() < 1e-3,
            "{} not {expected}",
            player.exhaustion
        );
    }
}
