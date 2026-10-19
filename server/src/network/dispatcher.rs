use crate::init::{ServerLobby, ServerTime};
use crate::mob::behavior::mob_behavior_system;
use crate::mods::Mods;
use crate::network::broadcast_chat::*;
use crate::world;
use crate::world::broadcast_world::broadcast_world_state;
use crate::world::data::ServerWorldMap;
use crate::world::dropped_items::{dropped_items_physics_system, pickup_dropped_items_system};
use crate::world::save::SaveRequestEvent;
use crate::world::simulation::{
    broadcast_player_updates_system, handle_player_inputs_system, PlayerInputsEvent,
};
use bevy::prelude::*;
use bevy_renet::renet::{ClientId, DisconnectReason, ServerEvent};
use bevy_renet::{RenetServer, RenetServerEvent};
use shared::messages::{ChatConversation, ClientToServerMessage};
use shared::world::WorldSeed;
use shared::{GameFolderPaths, GameServerConfig, MAX_INPUTS_PER_TICK, TICKS_PER_SECOND};

use super::extensions::SendGameMessageExtension;
use super::handlers::{self, Clients};

/// bevy_renet reports connections as observer events: this buffers them as a message so that
/// `handle_connection_events_system` still reads them in `Update`. The event is copied field by field
/// because `ServerEvent` is not `Clone`.
#[derive(Message)]
enum ServerEventMessage {
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
        .init_resource::<world::flow::WaterFlow>()
        .add_observer(buffer_server_event);

    setup_chat_resources(app);
}

/// The stages of a server tick, in this order. A save is done on the frame of its request and
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
    /// The player inputs are applied to the world, then the water flows.
    Simulate,
    /// The world changes are sent to the clients.
    Broadcast,
    /// The world is handed to the writer.
    Save,
}

pub fn register_systems(app: &mut App) {
    app.configure_sets(
        Update,
        (
            ServerSet::Receive,
            ServerSet::Simulate,
            ServerSet::Broadcast,
            ServerSet::Save,
        )
            .chain(),
    )
    .add_systems(
        Update,
        (
            handle_connection_events_system,
            handle_client_messages_system,
        )
            .chain()
            .in_set(ServerSet::Receive),
    )
    .add_systems(
        Update,
        (
            handle_player_inputs_system,
            pickup_dropped_items_system,
            crate::mob::manage_mob_spawning_system,
            world::flow::water_flow_system,
        )
            .chain()
            .in_set(ServerSet::Simulate),
    )
    .add_systems(
        Update,
        (broadcast_player_updates_system, broadcast_world_state).in_set(ServerSet::Broadcast),
    )
    .add_systems(
        Update,
        world::save::save_world_system.in_set(ServerSet::Save),
    );

    app.add_systems(PostUpdate, update_server_time);

    // `FixedUpdate` runs before `Update`: the mobs and the items move before the inputs of the
    // tick are read
    app.add_systems(
        FixedUpdate,
        (mob_behavior_system, dropped_items_physics_system),
    );
}

fn handle_connection_events_system(
    mut server_events: MessageReader<ServerEventMessage>,
    mut lobby: ResMut<ServerLobby>,
    mut world_map: ResMut<ServerWorldMap>,
    game_folder_paths: Res<GameFolderPaths>,
    mut ev_save_request: MessageWriter<SaveRequestEvent>,
) {
    for event in server_events.read() {
        match event {
            ServerEventMessage::ClientConnected { client_id } => {
                handlers::client_connected(*client_id);
            }
            ServerEventMessage::ClientDisconnected { client_id, reason } => {
                handlers::client_disconnected(
                    client_id,
                    reason,
                    &mut lobby,
                    &mut world_map,
                    &game_folder_paths,
                    &mut ev_save_request,
                );
            }
        }
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
    seed: Res<WorldSeed>,
    game_folder_paths: Res<GameFolderPaths>,
    mods: Res<Mods>,
) {
    for client_id in server.clients_id() {
        let mut input_budget = MAX_INPUTS_PER_TICK;
        let mut dropped_inputs = 0;
        while let Some(Ok(message)) = server.receive_game_message(client_id) {
            if !may_send(&message, client_id, &lobby, &config) {
                warn!(
                    "Ignoring {} message from unauthenticated client {}",
                    message_kind(&message),
                    client_id
                );
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
                        *seed,
                        &game_folder_paths,
                        config.max_render_distance,
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
                        &mods,
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
            }
        }
        if dropped_inputs > 0 {
            warn!(
                "Dropped {dropped_inputs} inputs of client {client_id}: more than {MAX_INPUTS_PER_TICK} in one tick"
            );
        }
    }
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
    use bevy::time::TimePlugin;
    use shared::messages::{NetworkAction, PlayerFrameInput};
    use shared::players::{blocks::HIT_MS, Player};
    use shared::world::{BlockData, BlockDirection, BlockId, ChunkMap, WorldSeed};

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

        let mut app = App::new();
        app.add_plugins(TimePlugin)
            .insert_resource(RenetServer::new(shared::get_shared_renet_config()))
            .insert_resource(ServerLobby::default())
            .insert_resource(GameServerConfig {
                world_name: "w".into(),
                is_solo: true,
                saves: None,
                max_render_distance: shared::DEFAULT_MAX_RENDER_DISTANCE,
            })
            .insert_resource(GameFolderPaths {
                game_folder_path: tmp.path().join("game"),
                assets_folder_path: tmp.path().join("assets"),
            })
            .insert_resource(world_map)
            .insert_resource(WorldSeed(1))
            .insert_resource(crate::world::save::Autosave { last_save_tick: 0 })
            .insert_resource(ServerTime(0))
            .insert_resource(Mods::new().unwrap())
            .insert_resource(WorldWriter::spawn(tmp.path().into(), None).unwrap());
        setup_resources_and_events(&mut app);
        register_systems(&mut app);

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
}
