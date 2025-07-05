use crate::init::{LobbyPlayer, ServerLobby, ServerTime};
use crate::mob::behavior::mob_behavior_system;
use crate::network::broadcast_chat::*;
use crate::network::cleanup::cleanup_player_from_world;
use crate::world;
use crate::world::background_generation::background_world_generation_system;
use crate::world::broadcast_world::broadcast_world_state;
use crate::world::save::SaveRequestEvent;
use crate::world::simulation::{handle_player_inputs_system, PlayerInputsEvent};
use crate::world::BlockInteractionEvent;
use bevy::prelude::*;
use bevy_renet::renet::{RenetServer, ServerEvent};
use shared::messages::{
    AuthRegisterResponse, ChatConversation, ClientToServerMessage, FullChatMessage,
    PlayerSpawnEvent, ServerToClientMessage,
};
use shared::players::Player;
use shared::world::ServerWorldMap;
use shared::{GameServerConfig, TICKS_PER_SECOND};

use super::extensions::SendGameMessageExtension;

pub fn setup_resources_and_events(app: &mut App) {
    app.add_event::<SaveRequestEvent>()
        .add_event::<BlockInteractionEvent>()
        .add_event::<PlayerInputsEvent>();

    setup_chat_resources(app);
}

pub fn register_systems(app: &mut App) {
    app.add_systems(Update, server_update_system);

    app.add_systems(Update, broadcast_world_state);

    app.add_systems(Update, world::save::save_world_system);
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
    (
        mut ev_chat,
        mut ev_app_exit,
        // mut ev_world_update_request,
        mut ev_save_request,
        mut ev_block_interaction,
        mut ev_player_inputs,
    ): (
        EventWriter<ChatMessageEvent>,
        EventWriter<AppExit>,
        // EventWriter<WorldUpdateRequestEvent>,
        EventWriter<SaveRequestEvent>,
        EventWriter<BlockInteractionEvent>,
        EventWriter<PlayerInputsEvent>,
    ),
    config: Res<GameServerConfig>,
    mut world_map: ResMut<ServerWorldMap>,
    time: Res<ServerTime>,
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
                cleanup_player_from_world(&mut world_map, client_id);
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

                    let maybe_existing_player = world_map.players.get(&client_id);

                    let new_position = match maybe_existing_player {
                        Some(existing_player) => existing_player.position,
                        None => Vec3::new(0.0, 80.0, 0.0),
                    };

                    let camera_transform = match maybe_existing_player {
                        Some(existing_player) => existing_player.camera_transform,
                        None => Transform::default(),
                    };

                    let player_data = Player::new(
                        client_id,
                        auth_req.username.clone(),
                        new_position,
                        camera_transform,
                    );

                    world_map.players.insert(client_id, player_data);

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
                            position: player.position,
                            camera_transform: player.camera_transform,
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

                    for (id, player) in lobby.players.iter() {
                        let spawn_message = PlayerSpawnEvent {
                            id: *id,
                            name: player.name.clone(),
                            position: Vec3::new(0.0, 80.0, 0.0),
                            camera_transform: Transform::default(),
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
                ClientToServerMessage::PlayerInputs(inputs) => {
                    // info!("Received {} player inputs", inputs.len());
                    for input in inputs.iter() {
                        ev_player_inputs.send(PlayerInputsEvent {
                            client_id,
                            input: input.clone(),
                        });
                    }
                }
                ClientToServerMessage::SaveWorldRequest => {
                    debug!("Save request received from client with session token");

                    ev_save_request.send(SaveRequestEvent);
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
