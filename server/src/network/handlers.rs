//! One handler per kind of client message, called by `server_update_system`.

use std::ops::ControlFlow;

use bevy::prelude::*;
use bevy_renet::renet::{ClientId, DisconnectReason};
use bevy_renet::RenetServer;
use shared::messages::{
    AuthRegisterRequest, AuthRegisterResponse, ChatConversation, ChatMessageRequest,
    PlayerFrameInput, PlayerSave, PlayerSpawnEvent, ServerToClientMessage,
};
use shared::messages::sanitize_username;
use shared::{GameFolderPaths, GameServerConfig};

use crate::init::{LobbyPlayer, ServerLobby, ServerTime};
use crate::network::broadcast_chat::record_chat_message;
use crate::network::cleanup::cleanup_player_from_world;
use crate::network::extensions::SendGameMessageExtension;
use crate::world::data::ServerWorldMap;
use crate::world::load_from_file::load_player_data;
use crate::world::save::SaveRequestEvent;
use crate::world::simulation::PlayerInputsEvent;

/// The transport and the authenticated players, which most handlers need.
pub(super) struct Clients<'a> {
    pub server: &'a mut RenetServer,
    pub lobby: &'a mut ServerLobby,
}

pub(super) fn client_connected(client_id: ClientId) {
    info!("Player {} connected.", client_id);
}

pub(super) fn client_disconnected(
    client_id: &ClientId,
    reason: &DisconnectReason,
    lobby: &mut ServerLobby,
    world_map: &mut ServerWorldMap,
    game_folder_paths: &GameFolderPaths,
    ev_save_request: &mut MessageWriter<SaveRequestEvent>,
) {
    info!("Player {} disconnected: {}", client_id, reason);
    lobby.players.remove(client_id);
    cleanup_player_from_world(world_map, client_id, game_folder_paths, ev_save_request);
}

/// Breaks when the client was refused: the rest of its messages of this frame are dropped.
pub(super) fn auth_register_request(
    clients: Clients,
    client_id: ClientId,
    auth_req: AuthRegisterRequest,
    world_map: &mut ServerWorldMap,
    chat_conversation: &ChatConversation,
    time: &ServerTime,
    game_folder_paths: &GameFolderPaths,
) -> ControlFlow<()> {
    let Clients { server, lobby } = clients;

    if lobby.players.contains_key(&client_id) {
        debug!("Ignoring a second authentication request from client {client_id}");
        return ControlFlow::Continue(());
    }
    let Some(username) = sanitize_username(&auth_req.username) else {
        warn!("Invalid username, disconnecting client {client_id}");
        server.disconnect(client_id);
        return ControlFlow::Break(());
    };
    let auth_req = AuthRegisterRequest {
        username: username.to_string(),
    };
    info!("Auth request received {:?}", auth_req);

    if lobby.players.values().any(|v| v.name == auth_req.username) {
        warn!(
            "Username already in use: {}, disconnecting client {}",
            &auth_req.username, client_id
        );
        server.disconnect(client_id);
        return ControlFlow::Break(());
    }

    lobby
        .players
        .insert(client_id, LobbyPlayer::new(auth_req.username.clone()));
    debug!("New lobby : {:?}", lobby);

    // Load player data if it doesn't already exist
    let registered_player = if let Some(player) = world_map.players.get(&client_id) {
        player
    } else {
        let player = match load_player_data(
            &world_map.name,
            &client_id,
            &auth_req.username,
            game_folder_paths,
        ) {
            Ok(player) => player,
            Err(err) => {
                error!("Refusing player {client_id}: {err}");
                lobby.players.remove(&client_id);
                server.disconnect(client_id);
                return ControlFlow::Break(());
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

        let spawn_message_wrapped = ServerToClientMessage::PlayerSpawn(spawn_message);

        info!("Sending spawn order {:?}", spawn_message_wrapped);
        server.broadcast_game_message(spawn_message_wrapped);
    }
    ControlFlow::Continue(())
}

pub(super) fn chat_message(
    clients: Clients,
    client_id: ClientId,
    chat_msg: ChatMessageRequest,
    chat_conversation: &mut ChatConversation,
) {
    let Clients { server, lobby } = clients;
    let current_timestamp: u64 = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;

    let Some(current_author) = lobby.players.get(&client_id) else {
        return;
    };

    let Some(message) = record_chat_message(
        chat_conversation,
        &current_author.name,
        chat_msg.content,
        current_timestamp,
    ) else {
        warn!("Rejected chat message from client {}", client_id);
        return;
    };
    // Only authenticated clients: the others are not in the lobby yet.
    for &id in lobby.players.keys() {
        server.send_game_message(id, ServerToClientMessage::ChatMessage(message.clone()));
    }
}

pub(super) fn exit(
    clients: Clients,
    client_id: ClientId,
    config: &GameServerConfig,
    ev_save_request: &mut MessageWriter<SaveRequestEvent>,
    ev_app_exit: &mut MessageWriter<AppExit>,
) {
    let Clients { server, lobby } = clients;
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

/// Forwards the inputs to the simulation while `budget`, the inputs this client may still send
/// in this tick, lasts. Returns how many were dropped.
pub(super) fn player_inputs(
    client_id: ClientId,
    inputs: Vec<PlayerFrameInput>,
    budget: &mut usize,
    ev_player_inputs: &mut MessageWriter<PlayerInputsEvent>,
) -> usize {
    let accepted = inputs.len().min(*budget);
    *budget -= accepted;
    let dropped = inputs.len() - accepted;
    for input in inputs.into_iter().take(accepted) {
        ev_player_inputs.write(PlayerInputsEvent { client_id, input });
    }
    dropped
}

pub(super) fn save_world_request(
    client_id: ClientId,
    config: &GameServerConfig,
    ev_save_request: &mut MessageWriter<SaveRequestEvent>,
) {
    debug!("Save request received from client with session token");

    // TODO : Check for permissions on multiplayer mode (server admin)

    // If in solo mode, save both world and player data
    if config.is_solo {
        ev_save_request.write(SaveRequestEvent::World);
        ev_save_request.write(SaveRequestEvent::Player(client_id));
    }
}
