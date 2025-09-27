use crate::init::{LobbyPlayer, ServerLobby, ServerTime};
use crate::mob::behavior::mob_behavior_system;
use crate::network::broadcast_chat::*;
use crate::network::cleanup::cleanup_player_from_world;
use crate::world;
use crate::world::background_generation::background_world_generation_system;
use crate::world::broadcast_world::broadcast_world_state;
use crate::world::load_from_file::load_player_data;
use crate::world::save::SaveRequestEvent;
use crate::world::simulation::{handle_player_inputs_system, PlayerInputsEvent};
use crate::world::BlockInteractionEvent;
use bevy::prelude::*;
use bevy_renet::renet::{RenetServer, ServerEvent};
use shared::messages::{
    AuthRegisterResponse, ChatConversation, ClientToServerMessage, FullChatMessage, PlayerSave,
    PlayerSpawnEvent, ServerToClientMessage,
};
use shared::players::Player;
use shared::world::ServerWorldMap;
use shared::{GameFolderPaths, GameServerConfig, TICKS_PER_SECOND};

use super::extensions::SendGameMessageExtension;

pub fn setup_resources_and_events(app: &mut App) {
    app.add_event::<SaveRequestEvent>()
        .add_event::<BlockInteractionEvent>()
        .add_event::<PlayerInputsEvent>();

    setup_chat_resources(app);
}

pub fn register_systems(app: &mut App) {
    // Chaining the two so that saves are always done on the same frame as the request
    app.add_systems(
        Update,
        (server_update_system, world::save::save_world_system).chain(),
    );

    app.add_systems(Update, broadcast_world_state);

    app.add_systems(Update, world::handle_block_interactions);

    app.add_systems(Update, crate::mob::manage_mob_spawning_system);

    app.add_systems(Update, handle_player_inputs_system);

    app.add_systems(Update, background_world_generation_system);

    app.add_systems(PostUpdate, update_server_time);

    app.add_systems(FixedUpdate, mob_behavior_system);
}

fn server_update_system(
    mut server_events: EventReader<ServerEvent>,
    (mut server, mut chat_conversation, mut lobby): (
        ResMut<RenetServer>,
        ResMut<ChatConversation>,
        ResMut<ServerLobby>,
    ),
    (mut ev_chat, mut ev_app_exit, mut ev_save_request, mut ev_player_inputs): (
        EventWriter<ChatMessageEvent>,
        EventWriter<AppExit>,
        EventWriter<SaveRequestEvent>,
        EventWriter<PlayerInputsEvent>,
    ),
    config: Res<GameServerConfig>,
    mut world_map: ResMut<ServerWorldMap>,
    time: Res<ServerTime>,
    game_folder_paths: Res<GameFolderPaths>,
) {
    for event in server_events.read() {
        debug!("event received");
        match event {
            ServerEvent::ClientConnected { client_id } => {
                info!("Player {} connected.", client_id);
            }
            ServerEvent::ClientDisconnected { client_id, reason } => {
                info!("Player {} disconnected: {}", client_id, reason);
                lobby.players.remove(client_id);
                cleanup_player_from_world(&mut world_map, client_id, &mut ev_save_request);
            }
        }
    }

    for client_id in server.clients_id() {
        while let Some(Ok(message)) = server.receive_game_message(client_id) {
            match message {
                ClientToServerMessage::AuthRegisterRequest(auth_req) => {
                    info!("Auth request received {:?}", auth_req);

                    if lobby.players.values().any(|v| v.name == auth_req.username) {
                        debug!("Username already in map: {}", &auth_req.username);
                        return;
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
                        let data =
                            load_player_data(&world_map.name, &client_id, &game_folder_paths);

                        world_map.players.insert(
                            client_id,
                            Player {
                                id: client_id,
                                is_flying: data.is_flying,
                                position: data.position,
                                camera_transform: data.camera_transform,
                                name: auth_req.username.clone(),
                                ..default()
                            },
                        );

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
                    info!("Chat message received: {:?}", &chat_msg);
                    let current_timestamp: u64 = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_millis() as u64;

                    let current_author = lobby.players.get(&client_id).unwrap();

                    chat_conversation.messages.push(FullChatMessage {
                        author: current_author.name.clone(),
                        content: chat_msg.content,
                        timestamp: current_timestamp,
                    });
                    ev_chat.write(ChatMessageEvent);
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

fn update_server_time(mut time: ResMut<ServerTime>) {
    if time.0.is_multiple_of(5 * TICKS_PER_SECOND) {
        debug!("Server time: {}", time.0);
    }
    time.0 += 1;
}
