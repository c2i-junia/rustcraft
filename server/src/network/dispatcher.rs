use crate::init::{ServerLobby, ServerTime, TICKS_PER_SECOND};
use crate::network::broadcast_chat::*;
use crate::network::broadcast_world::WorldUpdateRequestEvent;
use crate::network::broadcast_world::*;
use crate::world;
use crate::world::save::SaveRequestEvent;
use crate::world::BlockInteractionEvent;
use bevy::prelude::*;
use bevy_renet::renet::{DefaultChannel, RenetServer, ServerEvent};
use bincode::Options;
use shared::messages::{
    AuthRegisterResponse, ChatConversation, ClientToServerMessage, FullChatMessage,
    PlayerSpawnEvent, ServerToClientMessage,
};
use shared::world::ServerWorldMap;
use shared::GameServerConfig;

#[derive(Resource)]
pub struct BroadcastTimer {
    pub timer: Timer,
}

pub fn setup_resources_and_events(app: &mut App) {
    app.insert_resource(BroadcastTimer {
        timer: Timer::from_seconds(2.0, TimerMode::Repeating),
    })
    .add_event::<WorldUpdateRequestEvent>()
    .add_event::<SaveRequestEvent>()
    .add_event::<BlockInteractionEvent>();

    setup_chat_resources(app);
}

pub fn register_systems(app: &mut App) {
    app.add_systems(Update, server_update_system);

    app.add_systems(Update, broadcast_chat_messages);

    app.add_systems(Update, (broadcast_world_state, send_world_update));

    app.add_systems(Update, world::save::save_world_system);
    app.add_systems(Update, world::handle_block_interactions);

    app.add_systems(Update, crate::mob::manage_mob_spawning_system);

    app.add_systems(PostUpdate, update_server_time);
}

fn server_update_system(
    mut server_events: EventReader<ServerEvent>,
    (mut server, mut chat_conversation, mut lobby): (
        ResMut<RenetServer>,
        ResMut<ChatConversation>,
        ResMut<ServerLobby>,
    ),
    (
        mut ev_chat,
        mut ev_app_exit,
        mut ev_world_update_request,
        mut ev_save_request,
        mut ev_block_interaction,
    ): (
        EventWriter<ChatMessageEvent>,
        EventWriter<AppExit>,
        EventWriter<WorldUpdateRequestEvent>,
        EventWriter<SaveRequestEvent>,
        EventWriter<BlockInteractionEvent>,
    ),
    config: Res<GameServerConfig>,
    mut world_map: ResMut<ServerWorldMap>,
) {
    for event in server_events.read() {
        debug!("event received");
        match event {
            ServerEvent::ClientConnected { client_id } => {
                info!("Player {} connected.", client_id);
            }
            ServerEvent::ClientDisconnected { client_id, reason } => {
                info!("Player {} disconnected: {}", client_id, reason);
            }
        }
    }

    for client_id in server.clients_id() {
        while let Some(message) = server.receive_message(client_id, DefaultChannel::ReliableOrdered)
        {
            let msg = bincode::options().deserialize::<ClientToServerMessage>(&message);
            let msg = match msg {
                Ok(msg) => msg,
                Err(e) => {
                    error!("Failed to parse incoming message: {}", e);
                    continue;
                }
            };

            match msg {
                ClientToServerMessage::AuthRegisterRequest(auth_req) => {
                    info!("Auth request received {:?}", auth_req);

                    if lobby.players.values().any(|v| *v == auth_req.username) {
                        debug!("Username already in map: {}", &auth_req.username);
                        return;
                    }

                    lobby.players.insert(client_id, auth_req.username.clone());
                    debug!("New lobby : {:?}", lobby);

                    let spawn_message = PlayerSpawnEvent {
                        id: client_id,
                        name: auth_req.username,
                        position: Vec3::new(0.0, 80.0, 0.0),
                    };

                    // TODO: add cleanup system if no heartbeat
                    let msg = &(AuthRegisterResponse {
                        username: spawn_message.name.clone(),
                        session_token: client_id as u128,
                        spawn_event: spawn_message.clone(),
                    });
                    let auth_response_payload = bincode::options().serialize(msg).unwrap();

                    server.send_message(
                        client_id,
                        DefaultChannel::ReliableOrdered,
                        auth_response_payload,
                    );

                    for (id, name) in lobby.players.iter() {
                        let spawn_message = PlayerSpawnEvent {
                            id: *id,
                            name: name.into(),
                            position: Vec3::new(0.0, 80.0, 0.0),
                        };

                        let spawn_message_wrapped =
                            &ServerToClientMessage::PlayerSpawn(spawn_message);

                        let spawn_payload = bincode::options()
                            .serialize(&spawn_message_wrapped.clone())
                            .unwrap();

                        server.broadcast_message(DefaultChannel::ReliableUnordered, spawn_payload);
                        info!("Sending spawn order {:?}", spawn_message_wrapped);
                    }
                }
                ClientToServerMessage::ChatMessage(chat_msg) => {
                    info!("Chat message received: {:?}", &chat_msg);
                    let current_timestamp: u64 = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_millis() as u64;

                    let current_author = lobby.players.get(&client_id).unwrap().clone();

                    chat_conversation.messages.push(FullChatMessage {
                        author: current_author,
                        content: chat_msg.content,
                        timestamp: current_timestamp,
                    });
                    ev_chat.send(ChatMessageEvent);
                }
                ClientToServerMessage::Exit => {
                    debug!("Received shutdown order...");
                    // TODO: add permission checks
                    if config.is_solo {
                        info!("Server is going down...");
                        ev_app_exit.send(AppExit::Success);
                    } else {
                        server.disconnect(client_id);
                        lobby.players.remove(&client_id);
                        info!("Player {:?} disconnected", client_id);
                    }
                }
                ClientToServerMessage::PlayerInputs(_inputs) => {
                    // debug!("Not implemented yet: {:?}", inputs);
                }
                ClientToServerMessage::SaveWorldRequest => {
                    debug!("Save request received from client with session token");

                    ev_save_request.send(SaveRequestEvent);
                }
                ClientToServerMessage::WorldUpdateRequest {
                    player_chunk_position,
                    requested_chunks,
                    render_distance,
                } => {
                    debug!(
                        "Received WorldUpdateRequest: client_id = {}, player_chunk_position = {:?}, render_distance = {}, requested_chunks = {}",
                        client_id,
                        player_chunk_position,
                        render_distance,
                        requested_chunks.len(),
                    );
                    ev_world_update_request.send(WorldUpdateRequestEvent {
                        render_distance,
                        client: client_id,
                        chunks: requested_chunks,
                        player_chunk_position,
                    });
                }
                ClientToServerMessage::BlockInteraction {
                    position,
                    block_type,
                } => {
                    debug!(
                        "Block interaction received at {:?}: {:?}",
                        position, block_type
                    );

                    ev_block_interaction.send(BlockInteractionEvent {
                        position,
                        block_type,
                    });
                }
                ClientToServerMessage::SetPlayerPosition { position } => {
                    world_map.player_positions.insert(client_id, position);
                }
            }
        }
    }
}

fn update_server_time(mut time: ResMut<ServerTime>) {
    if (time.0 % (5 * TICKS_PER_SECOND)) == 0 {
        debug!("Server time: {}", time.0);
    }
    time.0 += 1;
}
