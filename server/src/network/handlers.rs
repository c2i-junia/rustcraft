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
use uuid::Uuid;

use shared::{GameFolderPaths, GameServerConfig, ServerAuth};

use crate::content::ContentFiles;
use crate::init::{LobbyPlayer, ServerLobby, ServerTime};
use crate::mods::Mods;
use crate::network::broadcast_chat::record_chat_message;
use crate::network::cleanup::cleanup_player_from_world;
use crate::network::commands::{
    command_line, list_players, parse_command, refusal, run_command, Command, Place, SERVER_AUTHOR,
};
use crate::network::extensions::SendGameMessageExtension;
use crate::network::join::{check_proof, BAD_NAME};
use crate::network::permissions::{same_name, Admin, Dismissals, Permissions, HOSTED_REFUSAL};
use crate::network::profiles::ProfileLookups;
use crate::world::data::ServerWorldMap;
use crate::world::generation::WorldGeneration;
use crate::world::load_from_file::load_player_data;
use crate::world::locate::Locator;
use crate::world::portals::all_portals;
use crate::world::save::SaveRequestEvent;
use crate::world::simulation::PlayerInputsEvent;
use crate::world::weather::{send_to_players, ServerWeather};

/// The transport and the authenticated players, which most handlers need.
pub(super) struct Clients<'a> {
    pub server: &'a mut RenetServer,
    pub lobby: &'a mut ServerLobby,
}

/// What a player proves its account against, in hub mode.
pub(super) struct Join<'a> {
    pub auth: &'a ServerAuth,
    /// The challenge of this connection.
    pub nonce: Option<&'a [u8; 32]>,
    pub server_key: &'a [u8; 32],
}

/// Who may do what, and the clients being dismissed.
pub(super) struct Access<'a> {
    pub permissions: &'a mut Permissions,
    pub dismissals: &'a mut Dismissals,
    pub is_solo: bool,
    /// How the players are known: by their account in hub mode, whose names are asked to the hub.
    pub auth: &'a ServerAuth,
    pub lookups: &'a mut ProfileLookups,
}

/// Runs `admin`, on the account `account` of its player in hub mode, and dismisses the players
/// it bans or kicks. Returns the answer to its author.
pub(super) fn run_admin(
    server: &mut RenetServer,
    lobby: &ServerLobby,
    (permissions, dismissals): (&mut Permissions, &mut Dismissals),
    admin: Admin,
    account: Option<Uuid>,
) -> String {
    let outcome = permissions.apply(
        admin,
        |name| lobby.players.values().any(|p| same_name(&p.name, name)),
        account,
    );
    if let Some((name, message)) = outcome.dismiss {
        let dismissed = lobby
            .players
            .iter()
            .filter(|(_, p)| match outcome.account {
                Some(account) => p.account == Some(account),
                None => same_name(&p.name, &name),
            })
            .map(|(id, _)| *id)
            .collect::<Vec<_>>();
        for id in dismissed {
            dismissals.dismiss(server, id, message.clone());
        }
    }
    outcome.answer
}

pub(super) fn client_connected(client_id: ClientId) {
    info!("Player {} connected.", client_id);
}

pub(super) fn client_disconnected(
    client_id: &ClientId,
    reason: &DisconnectReason,
    lobby: &mut ServerLobby,
    world_map: &mut ServerWorldMap,
    ev_save_request: &mut MessageWriter<SaveRequestEvent>,
) {
    info!("Player {} disconnected: {}", client_id, reason);
    lobby.players.remove(client_id);
    cleanup_player_from_world(world_map, client_id, ev_save_request);
}

/// Breaks when the client was refused: the rest of its messages of this frame are dropped.
pub(super) fn auth_register_request(
    clients: Clients,
    client_id: ClientId,
    auth_req: AuthRegisterRequest,
    world_map: &mut ServerWorldMap,
    chat_conversation: &ChatConversation,
    time: &ServerTime,
    (seed, generation): (WorldSeed, &WorldGeneration),
    game_folder_paths: &GameFolderPaths,
    max_render_distance: u32,
    mods: &Mods,
    weather: &ServerWeather,
    access: Access,
    join: Join,
) -> ControlFlow<()> {
    let Clients { server, lobby } = clients;

    if lobby.players.contains_key(&client_id) {
        debug!("Ignoring a second authentication request from client {client_id}");
        return ControlFlow::Continue(());
    }
    // In hub mode the player is the account of its certificate, whatever name it asks for
    let (username, certificate) = match join.auth {
        ServerAuth::Offline => {
            let Some(username) = sanitize_username(&auth_req.username) else {
                warn!("Invalid username, disconnecting client {client_id}");
                server.disconnect(client_id);
                return ControlFlow::Break(());
            };
            (username.to_string(), None)
        }
        ServerAuth::Hub(trust) => {
            let now = unix_time_ms() / 1000;
            let checked = check_proof(
                trust,
                auth_req.proof.as_ref(),
                client_id,
                join.nonce,
                join.server_key,
                now,
            )
            .and_then(|certificate| match sanitize_username(&certificate.name) {
                Some(name) => Ok((name.to_string(), certificate)),
                None => Err(BAD_NAME.to_string()),
            });
            match checked {
                Ok((name, certificate)) => (name, Some(certificate)),
                Err(reason) => {
                    warn!("Refusing client {client_id}: {reason}");
                    access.dismissals.dismiss(server, client_id, reason);
                    return ControlFlow::Break(());
                }
            }
        }
    };
    let username = username.as_str();
    let account = certificate.as_ref().map(|certificate| certificate.account);
    // Whatever the case, as the operators and the bans are known; by account in hub mode
    if lobby.players.values().any(|player| match account {
        Some(account) => player.account == Some(account),
        None => same_name(&player.name, username),
    }) {
        warn!("Username already in use: {username}, disconnecting client {client_id}");
        server.disconnect(client_id);
        return ControlFlow::Break(());
    }
    // A banned player, or one off the whitelist, is told why before it is disconnected
    if let Some(account) = account {
        access.permissions.identify(username, account);
    }
    if let Err(message) = access.permissions.admission(username, account) {
        warn!("Refusing {username}, client {client_id}: {message}");
        access.dismissals.dismiss(server, client_id, message);
        return ControlFlow::Break(());
    }

    // a player file holding content that is gone gives it placeholders
    let generation_before = shared::content::content().generation;
    if !world_map.players.contains_key(&client_id) {
        let spawn = world_map.spawn;
        match load_player_data(
            &world_map.name,
            &client_id,
            username,
            account,
            spawn,
            world_map.game_mode,
            game_folder_paths,
        ) {
            Ok(player) => world_map.players.insert(client_id, player),
            Err(err) => {
                error!("Refusing player {client_id}: {err}");
                server.disconnect(client_id);
                return ControlFlow::Break(());
            }
        };
    }
    if let Some(account) = account {
        world_map.accounts.insert(client_id, account);
    }
    if let Some(player) = world_map.players.get_mut(&client_id) {
        player.skin = auth_req.skin.validated();
    }
    world_map
        .render_distances
        .set(client_id, auth_req.render_distance, max_render_distance);
    lobby.players.insert(
        client_id,
        LobbyPlayer {
            account,
            certificate: certificate.as_ref().map(|certificate| certificate.id),
            ..LobbyPlayer::new(username.to_string())
        },
    );
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
        generation: generation.settings.clone(),
        biomes: generation.biomes.defs().to_vec(),
        mod_commands: mods.command_names(),
        weather: weather.kind,
    };
    server.send_game_message(client_id, auth_res.into());
    let content = ServerToClientMessage::Content(shared::content::content().clone());
    if shared::content::content().generation == generation_before {
        server.send_game_message(client_id, content);
    } else {
        // the other players learn the placeholders too
        for player in lobby.players.keys() {
            server.send_game_message(*player, content.clone());
        }
    }

    for message in &chat_conversation.messages {
        server.send_game_message(
            client_id,
            ServerToClientMessage::ChatMessage(message.clone()),
        );
    }
    server.send_game_message(client_id, all_portals(&world_map.chunks));

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
        skin: player.skin.clone(),
    }
}

pub(super) fn chat_message(
    clients: Clients,
    client_id: ClientId,
    chat_msg: ChatMessageRequest,
    chat_conversation: &mut ChatConversation,
    world_map: &mut ServerWorldMap,
    time: &mut ServerTime,
    weather: &mut ServerWeather,
    mods: &Mods,
    (seed, generation, locator): (WorldSeed, &WorldGeneration, &Locator),
    access: Access,
    content_files: Option<&mut ContentFiles>,
) {
    let Clients { server, lobby } = clients;
    let Some(current_author) = lobby.players.get(&client_id) else {
        return;
    };
    let author = current_author.name.clone();
    let author_account = current_author.account;

    if let Some(line) = command_line(&chat_msg.content) {
        // An authenticated client always has its player in the world
        let Some(player) = world_map.players.get_mut(&client_id) else {
            return;
        };
        let clock = time.0;
        let sky = weather.kind;
        let name = line.split_whitespace().next().unwrap_or_default();
        let known = shared::commands::COMMANDS
            .iter()
            .any(|command| command.name == name)
            || mods.command_names().iter().any(|command| command == name);
        let is_operator = access
            .permissions
            .may_run(&author, author_account, access.is_solo);
        let refused = refusal(name, known, is_operator);
        let mut overflow = Vec::new();
        let answer = match parse_command(line) {
            _ if line.starts_with("sign ") => {
                crate::world::home::write_sign(world_map, client_id, &line[5..])
            }
            _ if refused.is_some() => refused.unwrap_or_default(),
            Ok(Command::List) => {
                list_players(lobby.players.values().map(|p| p.name.as_str()).collect())
            }
            // the outcome is told to every player once the files are read
            Ok(Command::Reload) => match content_files {
                Some(files) => {
                    files.ask_reload();
                    String::new()
                }
                None => "This server reads no content files".to_string(),
            },
            Ok(Command::Admin(admin)) if access.permissions.refuses(&admin) => {
                HOSTED_REFUSAL.to_string()
            }
            Ok(Command::Admin(admin)) => {
                let hub = match access.auth {
                    ServerAuth::Hub(trust) => Some(trust.url.as_str()),
                    ServerAuth::Offline => None,
                };
                let to_resolve = hub.and_then(|hub| {
                    let name = access.permissions.name_to_resolve(&admin)?;
                    Some((hub, name.to_string()))
                });
                match to_resolve {
                    None => run_admin(
                        server,
                        lobby,
                        (access.permissions, access.dismissals),
                        admin,
                        None,
                    ),
                    Some((hub, name)) => {
                        let online = lobby
                            .players
                            .values()
                            .find(|p| same_name(&p.name, &name))
                            .and_then(|p| p.account);
                        if online.is_some() {
                            run_admin(
                                server,
                                lobby,
                                (access.permissions, access.dismissals),
                                admin,
                                online,
                            )
                        } else {
                            access.lookups.start(hub, client_id, admin, &name);
                            format!("Asking the hub for {name}...")
                        }
                    }
                }
            }
            Ok(command) => run_command(
                command,
                (time, weather, &mut world_map.difficulty),
                player,
                |place, from| match place {
                    Place::Biome(key) => locator.start(client_id, (seed.0, generation), &key, from),
                    Place::Dungeon(kind) => {
                        locator.start_dungeon(client_id, (seed.0, generation), kind, from)
                    }
                    Place::Village => locator.start_village(client_id, (seed.0, generation), from),
                    Place::Wreck => locator.start_wreck(client_id, (seed.0, generation), from),
                    Place::Cave(biome) => {
                        locator.start_cave(client_id, (seed.0, generation), biome, from)
                    }
                },
                &mut overflow,
            ),
            Err(error) => mods.run_command(line, &author, world_map).unwrap_or(error),
        };
        if let Some(player) = world_map.players.get(&client_id) {
            for stack in overflow {
                world_map.dropped_items.drop_from_player(stack, player);
            }
        }
        if time.0 != clock {
            // A night cut short is not lived through
            for player in world_map.players.values_mut() {
                player.progress.night = None;
            }
            for &id in lobby.players.keys() {
                server.send_game_message(id, ServerToClientMessage::TimeSet(time.0));
            }
        }
        if weather.kind != sky {
            send_to_players(
                server,
                lobby,
                ServerToClientMessage::WeatherSet(weather.kind),
            );
        }
        if answer.is_empty() {
            return;
        }
        let answer = FullChatMessage {
            author: SERVER_AUTHOR.to_string(),
            content: answer,
            timestamp: unix_time_ms(),
        };
        server.send_game_message(client_id, ServerToClientMessage::ChatMessage(answer));
        return;
    }

    let Some(message) =
        record_chat_message(chat_conversation, &author, chat_msg.content, unix_time_ms())
    else {
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

/// Brings the player of `client_id` back to the spawn, if it is dead.
pub(super) fn respawn(client_id: ClientId, world_map: &mut ServerWorldMap) {
    let Some(player) = world_map.players.get(&client_id) else {
        return;
    };
    let spawn = crate::world::home::respawn_position(world_map, player);
    let Some(player) = world_map.players.get_mut(&client_id) else {
        return;
    };
    if !player.is_dead() {
        debug!("Ignoring a respawn request from client {client_id}, whose player is alive");
        return;
    }
    info!("Player {} respawns at {spawn}", player.name);
    // A death of this very tick, before the messages are read, left nothing behind yet
    world_map
        .dropped_items
        .drop_on_death(player, &mut rand::thread_rng());
    player.respawn(spawn);
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

#[cfg(test)]
mod tests {
    use super::*;
    use shared::players::game_mode::{DamageSource, MAX_HEALTH};
    use shared::players::{air::MAX_AIR, hunger::MAX_FOOD};
    use shared::world::{BlockData, BlockDirection, BlockId, ChunkMap};

    #[test]
    fn a_respawn_in_the_tick_of_the_death_keeps_nothing() {
        use shared::world::{ItemId, ItemStack};
        let mut world_map = world_with_a_bedroll();
        let player = world_map.players.get_mut(&1).unwrap();
        player.experience.gain(100);
        let stack = ItemStack {
            item_id: ItemId::Diamond,
            item_type: ItemId::Diamond.get_default_type(),
            nb: 3,
        };
        player.inventory.inner.insert(0, stack);
        respawn(1, &mut world_map);
        let player = &world_map.players[&1];
        assert!(!player.is_dead() && player.inventory.inner.is_empty());
        assert_eq!(player.experience.total(), 0);
        assert_eq!(world_map.dropped_items.to_saved().len(), 2);
    }

    /// A world whose spawn is at (0, 10, 0), with a bedroll at (20, 1, 0) for the player
    /// "sleeper", who died there starving, out of air and burning.
    fn world_with_a_bedroll() -> ServerWorldMap {
        let bedroll = IVec3::new(20, 1, 0);
        let mut chunks =
            ChunkMap::with_loaded_region(IVec3::new(0, 0, -16), IVec3::new(31, 15, 15));
        chunks.set_block(
            bedroll,
            BlockData::new(BlockId::Bedroll, BlockDirection::Front),
        );
        let mut world_map = ServerWorldMap {
            chunks,
            spawn: Vec3::new(0.5, 10.0, 0.5),
            ..Default::default()
        };
        world_map.places.bedrolls.insert("sleeper".into(), bedroll);
        let mut player = Player::new(1, "sleeper".into(), Vec3::new(5.0, 2.0, 5.0), default());
        player.food = 0;
        player.air = 0;
        player.burning_ticks = 50;
        player.hurt(100, DamageSource::Starvation);
        world_map.players.insert(1, player);
        world_map
    }

    #[test]
    fn a_dead_player_respawns_on_its_bedroll_fed_breathing_and_put_out() {
        let mut world_map = world_with_a_bedroll();
        respawn(1, &mut world_map);
        let player = &world_map.players[&1];
        assert!(!player.is_dead());
        assert_eq!(player.health, MAX_HEALTH);
        assert_eq!(player.food, MAX_FOOD);
        assert_eq!(player.air, MAX_AIR);
        assert!(!player.is_burning());
        assert_eq!(player.death_cause, None);
        let on_bedroll = Vec3::new(20.5, 1.25 + player.height / 2.0, 0.5);
        assert!(
            player.position.distance(on_bedroll) < 1e-5,
            "{}",
            player.position
        );
    }

    #[test]
    fn without_its_bedroll_a_dead_player_respawns_at_the_spawn() {
        let mut world_map = world_with_a_bedroll();
        world_map.chunks.remove_block(IVec3::new(20, 1, 0));
        respawn(1, &mut world_map);
        assert_eq!(world_map.players[&1].position, world_map.spawn);
    }
}
