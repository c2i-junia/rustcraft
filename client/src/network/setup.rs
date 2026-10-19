use bevy::prelude::*;
use bevy::window::ExitSystems;
use bevy_renet::netcode::{
    ClientAuthentication, NetcodeClientPlugin, NetcodeClientTransport, NetcodeErrorEvent,
};
use bevy_renet::{RenetClient, RenetClientPlugin};
use rand::Rng;
use shared::messages::mob::MobUpdateEvent;
use shared::{get_shared_renet_config, GameServerConfig, STC_AUTH_CHANNEL};

use crate::menus::solo::SelectedWorld;
use crate::network::session::{end_session, ConnectError, SessionOrigin};
use crate::network::world::update_world_from_network;
use crate::network::CachedChatConversation;
use crate::state::{GameState, PlayerNameSupplied};
use crate::world::time::ClientTime;
use crate::world::{BlockChanged, WorldRenderRequestUpdateEvent};
use shared::messages::{
    AuthRegisterRequest, PlayerId, PlayerSpawnEvent, PlayerUpdateEvent, ServerToClientMessage,
};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::net::{Ipv4Addr, SocketAddr};
use std::thread::JoinHandle;
use std::{net::UdpSocket, thread, time::SystemTime};

use crate::world::ClientWorldMap;
use shared::GameFolderPaths;

use super::cleanup::close_session_on_app_exit;
use super::SendGameMessageExtension;

#[derive(Debug, Clone, PartialEq)]
pub enum TargetServerState {
    Initial,
    Establishing,
    ConnectionEstablished,
    FullyReady, // player has spawned
}

#[derive(Resource, Clone)]
pub struct CurrentPlayerProfile {
    pub id: PlayerId,
    pub name: String,
}

impl CurrentPlayerProfile {
    pub(crate) fn new() -> Self {
        let mut rng = rand::thread_rng();
        let id: u64 = rng.gen();
        Self {
            id,
            name: format!("Player-{id}"),
        }
    }
}

/// The id of a player is the hash of its name, and its save file is named after it. Rust does
/// not promise that `DefaultHasher` gives the same values in every release: replacing it would
/// need a migration of the saved players.
pub(crate) fn hash_string_to_u64(input: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    input.hash(&mut hasher);
    hasher.finish()
}

impl FromWorld for CurrentPlayerProfile {
    fn from_world(world: &mut World) -> Self {
        let player_name = world.get_resource::<PlayerNameSupplied>();
        match player_name {
            Some(player_name) => {
                if shared::messages::sanitize_username(&player_name.name).is_none() {
                    warn!(
                        "The player name {:?} will be refused by servers: 1 to {} characters, no control character",
                        player_name.name,
                        shared::messages::MAX_USERNAME_LENGTH
                    );
                }
                Self {
                    id: hash_string_to_u64(&player_name.name),
                    name: player_name.name.clone(),
                }
            }
            None => CurrentPlayerProfile::new(),
        }
    }
}

#[derive(Resource, Debug, Clone)]
pub struct TargetServer {
    pub address: Option<SocketAddr>,
    pub username: Option<String>,
    pub session_token: Option<u64>,
    pub state: TargetServerState,
}

pub fn add_base_netcode(app: &mut App) {
    app.add_plugins(RenetClientPlugin);

    let client = RenetClient::new(get_shared_renet_config());
    app.insert_resource(client);

    // Setup the transport layer
    app.add_plugins(NetcodeClientPlugin);
    // The netcode plugin disconnects the transport on `AppExit` in `Last`: the session must
    // be closed before that. The window plugin writes `AppExit` in `Last` too (`ExitSystems`),
    // so the session must be closed after that, in the very frame the app exits.
    app.add_systems(
        Last,
        close_session_on_app_exit
            .run_if(on_message::<AppExit>)
            .after(ExitSystems)
            .before(NetcodeClientPlugin::disconnect_on_exit),
    );

    // The username is taken from the player profile when the connection starts
    app.insert_resource(TargetServer {
        address: None,
        username: None,
        session_token: None,
        state: TargetServerState::Initial,
    });
}

/// The thread of the embedded solo server, kept to wait for it when the app exits.
#[derive(Resource)]
pub struct LocalServerHandle(pub Option<JoinHandle<()>>);

pub fn launch_local_server_system(
    mut commands: Commands,
    mut target: ResMut<TargetServer>,
    selected_world: Res<SelectedWorld>,
    paths: Res<GameFolderPaths>,
    local_server: Option<Res<LocalServerHandle>>,
    mut game_state: ResMut<NextState<GameState>>,
) {
    if target.address.is_some() {
        debug!("Skipping launch local server");
        return;
    }

    // Two servers on the same save folder would corrupt it.
    if local_server.is_some_and(|local_server| local_server.0.is_some()) {
        error!("The previous local server is still running, not starting another one");
        game_state.set(GameState::Menu);
        return;
    }

    let Some(world_name) = selected_world.name.clone() else {
        error!("No world selected, unable to launch the local server");
        game_state.set(GameState::Menu);
        return;
    };
    info!("Launching local server with world: {}", world_name);

    let socket = server::acquire_local_ephemeral_udp_socket(Ipv4Addr::LOCALHOST.into())
        .and_then(|socket| Ok((socket.local_addr()?, socket)));
    let (addr, socket) = match socket {
        Ok(socket) => socket,
        Err(err) => {
            error!("Could not open a socket for the local server: {err}");
            game_state.set(GameState::Menu);
            return;
        }
    };
    debug!("Obtained UDP socket: {}", addr);

    let paths = paths.clone();
    let handle = thread::spawn(move || {
        let config = GameServerConfig {
            world_name,
            is_solo: true,
        };
        if let Err(err) = server::init(socket, config, paths) {
            error!("Local server did not start: {err}");
        }
    });
    commands.insert_resource(LocalServerHandle(Some(handle)));

    target.address = Some(addr);
}

pub fn poll_network_messages(
    mut client: ResMut<RenetClient>,
    mut world: ResMut<ClientWorldMap>,
    mut ev_render: MessageWriter<WorldRenderRequestUpdateEvent>,
    mut ev_player_spawn: MessageWriter<PlayerSpawnEvent>,
    mut ev_mob_update: MessageWriter<MobUpdateEvent>,
    mut ev_player_update: MessageWriter<PlayerUpdateEvent>,
    mut ev_block_changed: MessageWriter<BlockChanged>,
    mut chat: ResMut<CachedChatConversation>,
) {
    update_world_from_network(
        &mut client,
        &mut world,
        &mut ev_render,
        &mut ev_player_spawn,
        &mut ev_mob_update,
        &mut ev_player_update,
        &mut ev_block_changed,
        &mut chat,
    );
}

/// Opens a connection to the server at `addr` as `client_id`: a new client and its transport.
pub fn open_connection(
    addr: SocketAddr,
    client_id: u64,
) -> Result<(RenetClient, NetcodeClientTransport), String> {
    let authentication = ClientAuthentication::Unsecure {
        server_addr: addr,
        client_id,
        user_data: None,
        protocol_id: shared::PROTOCOL_ID,
    };
    info!(
        "Attempting to connect to: {} with data {:?}",
        addr, authentication
    );

    let current_time = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).map_err(|err| err.to_string())?;
    let transport = NetcodeClientTransport::new(current_time, authentication, socket)
        .map_err(|err| err.to_string())?;
    Ok((RenetClient::new(get_shared_renet_config()), transport))
}

/// Connects to the server of the session, or ends it with `NoConnection`.
pub fn init_server_connection(
    mut commands: Commands,
    target: Res<TargetServer>,
    current_player_id: Res<CurrentPlayerProfile>,
    origin: Option<Res<SessionOrigin>>,
    mut game_state: ResMut<NextState<GameState>>,
) {
    let connection = match target.address {
        Some(addr) => open_connection(addr, current_player_id.id),
        None => Err("no server to connect to".to_string()),
    };
    let (client, transport) = match connection {
        Ok(connection) => connection,
        Err(reason) => {
            error!("Could not open a connection, back to the menu: {reason}");
            match origin {
                Some(origin) => {
                    let error = Some(ConnectError::NoConnection(reason));
                    end_session(&mut commands, &mut game_state, &origin, error);
                }
                None => game_state.set(GameState::Menu),
            }
            return;
        }
    };

    // Each of them replaces the one of the previous session
    commands.insert_resource(client);
    commands.insert_resource(transport);
    commands.insert_resource(CachedChatConversation::default());
    info!("Network subsystem initialized");
}

/// Logs transport errors, only while in game: outside of it the client is expected to be
/// disconnected or connecting.
pub fn network_failure_handler(
    renet_error: On<NetcodeErrorEvent>,
    game_state: Res<State<GameState>>,
) {
    if *game_state.get() != GameState::Game {
        return;
    }
    error!("network error: {}", *renet_error);
}

pub fn establish_authenticated_connection_to_server(
    mut client: ResMut<RenetClient>,
    mut target: ResMut<TargetServer>,
    current_profile: Res<CurrentPlayerProfile>,
    mut ev_spawn: MessageWriter<PlayerSpawnEvent>,
    mut client_time: ResMut<ClientTime>,
    mut refusal_logged: Local<bool>,
) {
    if target.session_token.is_some() {
        trace!(
            "Successfully acquired a session token as {}",
            target.username.as_deref().unwrap_or_default()
        );
        return;
    }

    if target.state == TargetServerState::Initial {
        let username = target
            .username
            .get_or_insert_with(|| current_profile.name.clone())
            .clone();

        let auth_msg = AuthRegisterRequest { username };
        info!("Sending auth request: {:?}", auth_msg);
        client.send_game_message(auth_msg.into());
        target.state = TargetServerState::Establishing;
    }

    // The server drops a client whose name it refuses (invalid or already in use)
    if target.state == TargetServerState::Establishing && client.is_disconnected() {
        if !*refusal_logged {
            warn!("The server closed the connection before authentication: is the player name valid and not already in use?");
            *refusal_logged = true;
        }
    } else {
        *refusal_logged = false;
    }

    while let Some(Ok(message)) = client.receive_game_message_by_channel(STC_AUTH_CHANNEL) {
        match message {
            ServerToClientMessage::AuthRegisterResponse(message) => {
                target.username = Some(message.username);
                target.session_token = Some(message.session_token);
                target.state = TargetServerState::ConnectionEstablished;
                client_time.0 = message.tick;
                // `timestamp_ms` is not used yet: the clock starts at the server's tick as it
                // was when the response left, so it lags by the latency
                for player in message.players {
                    ev_spawn.write(player);
                }
                info!("Connected! {:?}", target);
            }
            other => {
                // Only the variant name: the content comes from a server we do not trust
                let debug = format!("{other:?}");
                let kind = debug.split(['(', ' ']).next().unwrap_or_default();
                warn!("Ignoring an unexpected {kind} message on the authentication channel");
            }
        }
    }
}
