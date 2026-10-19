use bevy::prelude::*;
use bevy::window::ExitSystems;
use bevy_renet::netcode::{
    ClientAuthentication, NetcodeClientPlugin, NetcodeClientTransport, NetcodeDisconnectReason,
    NetcodeErrorEvent,
};
use bevy_renet::{RenetClient, RenetClientPlugin};
use rand::Rng;
use shared::messages::mob::MobUpdateEvent;
use shared::world::BiomeNoises;
use shared::{get_shared_renet_config, GameServerConfig, SaveCounter, STC_AUTH_CHANNEL};

use crate::menus::solo::SelectedWorld;
use crate::network::session::{end_session, ConnectError, SessionOrigin};
use crate::network::world::{update_world_from_network, DecodingChunkMessages};
use crate::network::CachedChatConversation;
use crate::state::{GameState, PlayerNameSupplied};
use crate::ui::hud::chat_completion::ModCommands;
use crate::ui::hud::debug::WorldBiomes;
use crate::world::time::ClientTime;
use crate::world::{
    BlockChanged, ChunkLoadEvent, RenderDistance, ServerRenderDistanceLimit,
    WorldRenderRequestUpdateEvent,
};
use shared::messages::{
    AuthRegisterRequest, DroppedItemsUpdate, PlayerId, PlayerSpawnEvent, PlayerUpdateEvent,
    ServerToClientMessage,
};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::net::{Ipv4Addr, SocketAddr};
use std::thread::JoinHandle;
use std::time::Instant;
use std::{net::UdpSocket, thread, time::SystemTime};

use crate::world::ClientWorldMap;
use shared::GameFolderPaths;

use super::cleanup::{close_session_on_app_exit, join_local_server, ExitSent};
use super::leave::STOP_TIMEOUT;
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

/// The thread of the embedded solo server, with the error it stopped on.
#[derive(Resource)]
pub struct LocalServerHandle(pub Option<JoinHandle<Result<(), String>>>);

/// The address the embedded solo server listens on: a server left running by a session that
/// ended is told to stop there, once the session's own address is forgotten.
#[derive(Resource)]
pub struct LocalServerAddress(pub SocketAddr);

/// The saves the embedded solo server of the session finished writing, until its thread is
/// joined.
#[derive(Resource, Clone)]
pub struct LocalSaves(pub SaveCounter);

/// Forgets the saves of the embedded server once its thread is joined: a server session after a
/// world has no saves to watch. Runs in `First`, before a world launched in the frame joins the
/// last server and starts its own.
pub fn forget_the_saves_of_a_joined_server(
    local_server: Option<Res<LocalServerHandle>>,
    mut commands: Commands,
) {
    if local_server.is_none_or(|local_server| local_server.0.is_none()) {
        commands.remove_resource::<LocalSaves>();
    }
}

/// The world opened waits for the server of the last one, told to stop, to finish saving: since
/// the world was asked for.
#[derive(Resource)]
pub struct LastWorldClosing {
    pub since: Instant,
}

/// Starts the embedded server of the selected world, or ends the session: `WorldFailed` without
/// a world, `NoConnection` without a socket. While the server of the last session still runs,
/// the world waits for it (`LastWorldClosing`) when it was told to stop (`ExitSent`), and runs
/// again every frame until that server is joined; `WorldBusy` after `STOP_TIMEOUT`, or at once
/// for a server never told. Stands down when the session already ended this frame.
pub fn launch_local_server_system(
    mut commands: Commands,
    mut target: ResMut<TargetServer>,
    selected_world: Res<SelectedWorld>,
    paths: Res<GameFolderPaths>,
    local_server: Option<ResMut<LocalServerHandle>>,
    origin: Option<Res<SessionOrigin>>,
    exit_sent: Option<Res<ExitSent>>,
    closing: Option<Res<LastWorldClosing>>,
    mut game_state: ResMut<NextState<GameState>>,
) {
    if target.address.is_some() {
        debug!("Skipping launch local server");
        return;
    }
    // the session ended this frame (a cancel, a quit): no server is started that no one would
    // tell to stop
    if !matches!(*game_state, NextState::Unchanged) {
        return;
    }

    // Two servers on the same save folder would corrupt it; one that already stopped is joined.
    if let Some(mut local_server) = local_server {
        if local_server.0.as_ref().is_some_and(JoinHandle::is_finished) {
            if let Some(handle) = local_server.0.take() {
                join_local_server(handle);
            }
        }
        if local_server.0.is_some() {
            let since = closing
                .as_ref()
                .map_or_else(Instant::now, |closing| closing.since);
            if exit_sent.is_some() && since.elapsed() < STOP_TIMEOUT {
                if closing.is_none() {
                    info!("Waiting for the last world to be saved before opening this one");
                    commands.insert_resource(LastWorldClosing { since });
                }
                return;
            }
            error!("The previous local server is still running, not starting another one");
            commands.remove_resource::<LastWorldClosing>();
            fail_launch(
                &mut commands,
                &mut game_state,
                origin.as_deref(),
                ConnectError::WorldBusy,
            );
            return;
        }
    }
    commands.remove_resource::<LastWorldClosing>();

    let mut fail = |error: ConnectError| {
        fail_launch(&mut commands, &mut game_state, origin.as_deref(), error);
    };

    let Some(world_name) = selected_world.name.clone() else {
        error!("No world selected, unable to launch the local server");
        fail(ConnectError::WorldFailed(
            "No world was selected".to_string(),
        ));
        return;
    };
    info!("Launching local server with world: {}", world_name);

    let socket = server::acquire_local_ephemeral_udp_socket(Ipv4Addr::LOCALHOST.into())
        .and_then(|socket| Ok((socket.local_addr()?, socket)));
    let (addr, socket) = match socket {
        Ok(socket) => socket,
        Err(err) => {
            error!("Could not open a socket for the local server: {err}");
            fail(ConnectError::NoConnection(err.to_string()));
            return;
        }
    };
    debug!("Obtained UDP socket: {}", addr);

    let paths = paths.clone();
    let saves = SaveCounter::default();
    commands.insert_resource(LocalSaves(saves.clone()));
    let handle = thread::spawn(move || {
        let config = GameServerConfig {
            world_name,
            is_solo: true,
            saves: Some(saves),
            // the player's own world: whatever the player may choose
            max_render_distance: crate::constants::MAX_CHUNK_RENDER_DISTANCE_RADIUS,
        };
        let result = server::init(socket, config, paths);
        if let Err(err) = &result {
            error!("Local server did not start: {err}");
        }
        result
    });
    commands.insert_resource(LocalServerHandle(Some(handle)));
    commands.insert_resource(LocalServerAddress(addr));
    // told nothing yet, unlike a server of an earlier session joined above
    commands.remove_resource::<ExitSent>();

    target.address = Some(addr);
}

pub fn poll_network_messages(
    mut commands: Commands,
    mut client: ResMut<RenetClient>,
    mut world: ResMut<ClientWorldMap>,
    mut ev_render: MessageWriter<WorldRenderRequestUpdateEvent>,
    mut ev_chunk_load: MessageWriter<ChunkLoadEvent>,
    mut ev_player_spawn: MessageWriter<PlayerSpawnEvent>,
    mut ev_mob_update: MessageWriter<MobUpdateEvent>,
    mut ev_player_update: MessageWriter<PlayerUpdateEvent>,
    mut ev_block_changed: MessageWriter<BlockChanged>,
    mut ev_dropped_items: MessageWriter<DroppedItemsUpdate>,
    mut chat: ResMut<CachedChatConversation>,
    mut client_time: ResMut<ClientTime>,
    mut decoding: ResMut<DecodingChunkMessages>,
) {
    update_world_from_network(
        &mut commands,
        &mut client,
        &mut world,
        &mut ev_render,
        &mut ev_chunk_load,
        &mut ev_player_spawn,
        &mut ev_mob_update,
        &mut ev_player_update,
        &mut ev_block_changed,
        &mut ev_dropped_items,
        &mut chat,
        &mut client_time,
        &mut decoding,
    );
}

/// Ends the session whose world could not be launched as `error`, or goes back to the menu.
fn fail_launch(
    commands: &mut Commands,
    game_state: &mut NextState<GameState>,
    origin: Option<&SessionOrigin>,
    error: ConnectError,
) {
    match origin {
        Some(origin) => {
            end_session(commands, game_state, origin, Some(error));
        }
        None => game_state.set(GameState::Menu),
    }
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
    shared::enlarge_socket_buffers(&socket);
    let transport = NetcodeClientTransport::new(current_time, authentication, socket)
        .map_err(|err| err.to_string())?;
    Ok((RenetClient::new(get_shared_renet_config()), transport))
}

/// Connects to the server of the session, or ends it with `NoConnection`. Stands down when the
/// session already ended this frame, the launch of its world failed, and while its world waits
/// for the last one to close (`LastWorldClosing`): it connects once that world is launched.
pub fn init_server_connection(
    mut commands: Commands,
    target: Res<TargetServer>,
    current_player_id: Res<CurrentPlayerProfile>,
    origin: Option<Res<SessionOrigin>>,
    closing: Option<Res<LastWorldClosing>>,
    mut game_state: ResMut<NextState<GameState>>,
) {
    if !matches!(*game_state, NextState::Unchanged) || closing.is_some() {
        return;
    }
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
    transport: Option<Res<NetcodeClientTransport>>,
    mut target: ResMut<TargetServer>,
    current_profile: Res<CurrentPlayerProfile>,
    render_distance: Res<RenderDistance>,
    mut commands: Commands,
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

    // A message sent before netcode connects is dropped, and sent again only after the resend
    // time of its channel
    if target.state == TargetServerState::Initial && client.is_connected() {
        let username = target
            .username
            .get_or_insert_with(|| current_profile.name.clone())
            .clone();

        let auth_msg = AuthRegisterRequest {
            username,
            render_distance: render_distance.distance,
        };
        info!("Sending auth request: {:?}", auth_msg);
        client.send_game_message(auth_msg.into());
        target.state = TargetServerState::Establishing;
    }

    // The server drops a client whose name it refuses (invalid or already in use); a connection
    // that gave up on a silent server is no refusal
    let refused = transport.is_some_and(|transport| {
        transport.disconnect_reason() == Some(NetcodeDisconnectReason::DisconnectedByServer)
    });
    if target.state == TargetServerState::Establishing && refused {
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
                commands.insert_resource(ServerRenderDistanceLimit(message.max_render_distance));
                commands.insert_resource(WorldBiomes(BiomeNoises::new(message.seed)));
                commands.insert_resource(ModCommands(message.mod_commands));
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::network::finish_stopping_local_server;
    use bevy::state::app::StatesPlugin;
    use std::sync::mpsc::channel;
    use std::time::Duration;

    /// The saves of a world go with its server: once it is joined in the menus, a server session
    /// entered next finds none.
    #[test]
    fn the_saves_of_a_world_go_once_its_server_is_joined() {
        let (release, wait) = channel::<()>();
        let server = thread::spawn(move || {
            let _ = wait.recv();
            Ok(())
        });
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, StatesPlugin))
            .insert_state(GameState::Menu)
            .insert_resource(LocalServerHandle(Some(server)))
            .insert_resource(LocalSaves(SaveCounter::default()))
            .add_systems(
                First,
                forget_the_saves_of_a_joined_server.run_if(resource_exists::<LocalSaves>),
            )
            .add_systems(Update, finish_stopping_local_server);
        for _ in 0..3 {
            app.update();
            assert!(app.world().contains_resource::<LocalSaves>());
        }

        release.send(()).unwrap();
        let start = Instant::now();
        while app.world().resource::<LocalServerHandle>().0.is_some() {
            assert!(start.elapsed() < Duration::from_secs(10), "never joined");
            app.update();
            thread::sleep(Duration::from_millis(5));
        }
        app.update();
        assert!(!app.world().contains_resource::<LocalSaves>());
    }
}
