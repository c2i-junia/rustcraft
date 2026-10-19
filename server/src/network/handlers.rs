//! One handler per kind of client message, called by `handle_client_messages_system`.

use std::ops::ControlFlow;

use bevy::prelude::*;
use bevy_renet::renet::{ClientId, DisconnectReason};
use bevy_renet::RenetServer;
use shared::messages::{
    sanitize_username, AuthRegisterRequest, AuthRegisterResponse, ChatConversation,
    ChatMessageRequest, FullChatMessage, PlayerFrameInput, PlayerSave, PlayerSpawnEvent,
    ServerToClientMessage,
};
use shared::players::Player;
use shared::utils::unix_time_ms;
use shared::world::WorldSeed;
use shared::{GameFolderPaths, GameServerConfig};

use crate::init::{LobbyPlayer, ServerLobby, ServerTime};
use crate::network::broadcast_chat::record_chat_message;
use crate::network::cleanup::cleanup_player_from_world;
use crate::network::commands::{command_line, parse_command, run_command, SERVER_AUTHOR};
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
    seed: WorldSeed,
    game_folder_paths: &GameFolderPaths,
    max_render_distance: u32,
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
    if lobby.players.values().any(|player| player.name == username) {
        warn!("Username already in use: {username}, disconnecting client {client_id}");
        server.disconnect(client_id);
        return ControlFlow::Break(());
    }

    if !world_map.players.contains_key(&client_id) {
        match load_player_data(&world_map.name, &client_id, username, game_folder_paths) {
            Ok(player) => world_map.players.insert(client_id, player),
            Err(err) => {
                error!("Refusing player {client_id}: {err}");
                server.disconnect(client_id);
                return ControlFlow::Break(());
            }
        };
    }
    world_map
        .render_distances
        .set(client_id, auth_req.render_distance, max_render_distance);
    lobby
        .players
        .insert(client_id, LobbyPlayer::new(username.to_string()));
    info!("Player {username} authenticated as client {client_id}");

    let players: Vec<PlayerSpawnEvent> = world_map
        .players
        .iter()
        .map(|(id, player)| spawn_event(*id, player))
        .collect();
    let joiner = players.iter().find(|spawn| spawn.id == client_id).cloned();

    let auth_res = AuthRegisterResponse {
        username: username.to_string(),
        session_token: client_id,
        tick: time.0,
        timestamp_ms: unix_time_ms(),
        players,
        max_render_distance,
        seed: seed.0,
    };
    server.send_game_message(client_id, auth_res.into());

    for message in &chat_conversation.messages {
        server.send_game_message(
            client_id,
            ServerToClientMessage::ChatMessage(message.clone()),
        );
    }

    // The players already there learn about the new one
    if let Some(joiner) = joiner {
        server.broadcast_game_message(ServerToClientMessage::PlayerSpawn(joiner));
    }
    ControlFlow::Continue(())
}

/// What a client needs to show `player`.
fn spawn_event(id: ClientId, player: &Player) -> PlayerSpawnEvent {
    PlayerSpawnEvent {
        id,
        name: player.name.clone(),
        data: PlayerSave {
            position: player.position,
            camera_transform: player.camera_transform,
            is_flying: player.is_flying,
        },
    }
}

pub(super) fn chat_message(
    clients: Clients,
    client_id: ClientId,
    chat_msg: ChatMessageRequest,
    chat_conversation: &mut ChatConversation,
    world_map: &mut ServerWorldMap,
    time: &mut ServerTime,
) {
    let Clients { server, lobby } = clients;
    let Some(current_author) = lobby.players.get(&client_id) else {
        return;
    };

    if let Some(line) = command_line(&chat_msg.content) {
        // An authenticated client always has its player in the world
        let Some(player) = world_map.players.get_mut(&client_id) else {
            return;
        };
        let clock = time.0;
        let answer = match parse_command(line) {
            Ok(command) => run_command(command, time, &mut player.inventory),
            Err(error) => error,
        };
        if time.0 != clock {
            for &id in lobby.players.keys() {
                server.send_game_message(id, ServerToClientMessage::TimeSet(time.0));
            }
        }
        let answer = FullChatMessage {
            author: SERVER_AUTHOR.to_string(),
            content: answer,
            timestamp: unix_time_ms(),
        };
        server.send_game_message(client_id, ServerToClientMessage::ChatMessage(answer));
        return;
    }

    let Some(message) = record_chat_message(
        chat_conversation,
        &current_author.name,
        chat_msg.content,
        unix_time_ms(),
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

    // The solo player owns its server: leaving stops it. On a dedicated server a player
    // only leaves.
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

pub(super) fn set_render_distance(
    client_id: ClientId,
    asked: u32,
    world_map: &mut ServerWorldMap,
    max_render_distance: u32,
) {
    world_map
        .render_distances
        .set(client_id, asked, max_render_distance);
    debug!(
        "Client {client_id} asked for a render distance of {asked}, granted {}",
        world_map.render_distances.get(client_id)
    );
}

pub(super) fn save_world_request(
    client_id: ClientId,
    config: &GameServerConfig,
    ev_save_request: &mut MessageWriter<SaveRequestEvent>,
) {
    debug!("Save request received from client with session token");

    // Only the solo player may ask for a save: a dedicated server has no notion of admin
    // yet and relies on its autosave.
    if config.is_solo {
        ev_save_request.write(SaveRequestEvent::World);
        ev_save_request.write(SaveRequestEvent::Player(client_id));
    }
}
