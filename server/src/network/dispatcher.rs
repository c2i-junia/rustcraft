use crate::init::{LobbyPlayer, ServerLobby, ServerTime};
use crate::mob::behavior::mob_behavior_system;
use crate::network::broadcast_chat::*;
use crate::network::cleanup::cleanup_player_from_world;
use crate::world;
use crate::world::broadcast_world::broadcast_world_state;
use crate::world::load_from_file::load_player_data;
use crate::world::save::SaveRequestEvent;
use crate::world::simulation::{handle_player_inputs_system, PlayerInputsEvent};
use bevy::prelude::*;
use bevy_renet::renet::{ClientId, DisconnectReason, ServerEvent};
use bevy_renet::{RenetServer, RenetServerEvent};
use shared::messages::{
    AuthRegisterResponse, ChatConversation, ClientToServerMessage, PlayerSave, PlayerSpawnEvent,
    ServerToClientMessage,
};
use shared::world::ServerWorldMap;
use shared::{GameFolderPaths, GameServerConfig, TICKS_PER_SECOND};

use super::extensions::SendGameMessageExtension;

/// bevy_renet reports connections as observer events: this buffers them as a message so that
/// `server_update_system` still reads them in `Update`. The event is copied field by field
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
        .add_observer(buffer_server_event);

    setup_chat_resources(app);
}

pub fn register_systems(app: &mut App) {
    // Chaining the two so that saves are always done on the same frame as the request
    app.add_systems(
        Update,
        (server_update_system, world::save::save_world_system).chain(),
    );

    app.add_systems(Update, broadcast_world_state);

    app.add_systems(Update, crate::mob::manage_mob_spawning_system);

    app.add_systems(Update, handle_player_inputs_system);

    app.add_systems(PostUpdate, update_server_time);

    app.add_systems(FixedUpdate, mob_behavior_system);
}

fn server_update_system(
    mut server_events: MessageReader<ServerEventMessage>,
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
    time: Res<ServerTime>,
    game_folder_paths: Res<GameFolderPaths>,
) {
    for event in server_events.read() {
        debug!("event received");
        match event {
            ServerEventMessage::ClientConnected { client_id } => {
                info!("Player {} connected.", client_id);
            }
            ServerEventMessage::ClientDisconnected { client_id, reason } => {
                info!("Player {} disconnected: {}", client_id, reason);
                lobby.players.remove(client_id);
                cleanup_player_from_world(
                    &mut world_map,
                    client_id,
                    &game_folder_paths,
                    &mut ev_save_request,
                );
            }
        }
    }

    for client_id in server.clients_id() {
        while let Some(Ok(message)) = server.receive_game_message(client_id) {
            let is_auth_request = matches!(message, ClientToServerMessage::AuthRegisterRequest(_));
            // In solo mode the only client is the local player: it must be able to stop its
            // server even before the authentication went through (cancelled connection).
            let is_solo_exit = config.is_solo && matches!(message, ClientToServerMessage::Exit);
            if !is_auth_request && !is_solo_exit && !lobby.players.contains_key(&client_id) {
                warn!(
                    "Ignoring {} message from unauthenticated client {}",
                    message_kind(&message),
                    client_id
                );
                continue;
            }

            match message {
                ClientToServerMessage::AuthRegisterRequest(auth_req) => {
                    info!("Auth request received {:?}", auth_req);

                    if lobby.players.values().any(|v| v.name == auth_req.username) {
                        warn!(
                            "Username already in use: {}, disconnecting client {}",
                            &auth_req.username, client_id
                        );
                        server.disconnect(client_id);
                        break;
                    }

                    lobby
                        .players
                        .insert(client_id, LobbyPlayer::new(auth_req.username.clone()));
                    debug!("New lobby : {:?}", lobby);

                    // Load player data if it doesn't already exist
                    let registered_player = if let Some(player) = world_map.players.get(&client_id)
                    {
                        player
                    } else {
                        let player = match load_player_data(
                            &world_map.name,
                            &client_id,
                            &auth_req.username,
                            &game_folder_paths,
                        ) {
                            Ok(player) => player,
                            Err(err) => {
                                error!("Refusing player {client_id}: {err}");
                                lobby.players.remove(&client_id);
                                server.disconnect(client_id);
                                break;
                            }
                        };

                        world_map.players.insert(client_id, player);

                        world_map.players.get(&client_id).unwrap()
                    };

                    let timestamp_ms: u64 = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_millis() as u64;

                    let all_player_spawn_events = world_map
                        .players
                        .iter()
                        .map(|(id, player)| PlayerSpawnEvent {
                            id: *id,
                            name: player.name.clone(),
                            data: PlayerSave {
                                position: player.position,
                                camera_transform: player.camera_transform,
                                is_flying: player.is_flying,
                            },
                        })
                        .collect();

                    // TODO: add cleanup system if no heartbeat
                    let auth_res = AuthRegisterResponse {
                        username: auth_req.username,
                        session_token: client_id,
                        tick: time.0,
                        timestamp_ms,
                        players: all_player_spawn_events,
                    };

                    server.send_game_message(client_id, auth_res.into());

                    for message in &chat_conversation.messages {
                        server.send_game_message(
                            client_id,
                            ServerToClientMessage::ChatMessage(message.clone()),
                        );
                    }

                    // Send message to all players that a new one spawned
                    for (id, player) in lobby.players.iter() {
                        let spawn_message = PlayerSpawnEvent {
                            id: *id,
                            name: player.name.clone(),
                            data: PlayerSave {
                                position: registered_player.position,
                                camera_transform: registered_player.camera_transform,
                                is_flying: registered_player.is_flying,
                            },
                        };

                        let spawn_message_wrapped =
                            ServerToClientMessage::PlayerSpawn(spawn_message);

                        info!("Sending spawn order {:?}", spawn_message_wrapped);
                        server.broadcast_game_message(spawn_message_wrapped);
                    }
                }
                ClientToServerMessage::ChatMessage(chat_msg) => {
                    let current_timestamp: u64 = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_millis() as u64;

                    let Some(current_author) = lobby.players.get(&client_id) else {
                        continue;
                    };

                    let Some(message) = record_chat_message(
                        &mut chat_conversation,
                        &current_author.name,
                        chat_msg.content,
                        current_timestamp,
                    ) else {
                        warn!("Rejected chat message from client {}", client_id);
                        continue;
                    };
                    // Only authenticated clients: the others are not in the lobby yet.
                    for &id in lobby.players.keys() {
                        server.send_game_message(
                            id,
                            ServerToClientMessage::ChatMessage(message.clone()),
                        );
                    }
                }
                ClientToServerMessage::Exit => {
                    debug!("Received shutdown order...");

                    // Save player data on exit
                    ev_save_request.write(SaveRequestEvent::Player(client_id));

                    // TODO: add permission checks
                    if config.is_solo {
                        info!("Server is going down...");
                        ev_app_exit.write(AppExit::Success);
                    } else {
                        server.disconnect(client_id);
                        lobby.players.remove(&client_id);
                        info!("Player {:?} disconnected", client_id);
                    }
                }
                ClientToServerMessage::PlayerInputs(inputs) => {
                    for input in inputs.iter() {
                        ev_player_inputs.write(PlayerInputsEvent {
                            client_id,
                            input: input.clone(),
                        });
                    }
                }
                ClientToServerMessage::SaveWorldRequest => {
                    debug!("Save request received from client with session token");

                    // TODO : Check for permissions on multiplayer mode (server admin)

                    // If in solo mode, save both world and player data
                    if config.is_solo {
                        ev_save_request.write(SaveRequestEvent::World);
                        ev_save_request.write(SaveRequestEvent::Player(client_id));
                    }
                }
            }
        }
    }
}

/// Name of the variant of `message`, to log what a client sent without dumping its payload.
fn message_kind(message: &ClientToServerMessage) -> &'static str {
    match message {
        ClientToServerMessage::AuthRegisterRequest(_) => "AuthRegisterRequest",
        ClientToServerMessage::ChatMessage(_) => "ChatMessage",
        ClientToServerMessage::Exit => "Exit",
        ClientToServerMessage::PlayerInputs(_) => "PlayerInputs",
        ClientToServerMessage::SaveWorldRequest => "SaveWorldRequest",
    }
}

fn update_server_time(mut time: ResMut<ServerTime>) {
    if time.0.is_multiple_of(5 * TICKS_PER_SECOND) {
        debug!("Server time: {}", time.0);
    }
    time.0 += 1;
}
