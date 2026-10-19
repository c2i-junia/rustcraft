use crate::network::{
    open_connection, CurrentPlayerProfile, LocalServerAddress, LocalServerHandle,
    SendGameMessageExtension, TargetServer, TargetServerState,
};
use bevy::prelude::*;
use bevy_renet::netcode::NetcodeClientTransport;
use bevy_renet::RenetClient;
use shared::messages::ClientToServerMessage;
use std::time::{Duration, Instant};

use super::{buffered_client::PlayerTickInputsBuffer, UnacknowledgedInputs};

/// Ends the session when the player leaves the game or cancels the connection: tells the
/// server, waits for the embedded server (if any) to save and stop, and resets the
/// connection state.
pub fn terminate_server_connection(
    mut commands: Commands,
    mut target: ResMut<TargetServer>,
    mut unacknowledged_inputs: ResMut<UnacknowledgedInputs>,
    mut current_frame: ResMut<PlayerTickInputsBuffer>,
    client: Option<ResMut<RenetClient>>,
    transport: Option<ResMut<NetcodeClientTransport>>,
    local_server: Option<ResMut<LocalServerHandle>>,
    address: Option<Res<LocalServerAddress>>,
) {
    info!("Terminating server connection");
    let can_drop_connection = stop_session(
        client.map(ResMut::into_inner),
        transport.map(ResMut::into_inner),
        local_server.map(ResMut::into_inner),
        goes_to_local_server(&target, address.as_deref()),
    );
    if can_drop_connection {
        drop_connection(&mut commands);
    }

    target.address = None;
    target.username = None;
    target.session_token = None;
    target.state = TargetServerState::Initial;

    unacknowledged_inputs.0.clear();
    current_frame.buffer.clear();
}

/// Removes the connection of an ended session, so that it is not updated any more.
fn drop_connection(commands: &mut Commands) {
    commands.remove_resource::<RenetClient>();
    commands.remove_resource::<NetcodeClientTransport>();
}

/// How long the app waits for the embedded server to save and stop when a session ends.
const LOCAL_SERVER_STOP_TIMEOUT: Duration = Duration::from_secs(5);

/// Closes the session when the app exits (window closed, quit button), which never goes
/// through `OnExit(GameState::Game)`. Runs before the netcode plugin disconnects the
/// transport.
pub fn close_session_on_app_exit(
    client: Option<ResMut<RenetClient>>,
    transport: Option<ResMut<NetcodeClientTransport>>,
    local_server: Option<ResMut<LocalServerHandle>>,
    target: Res<TargetServer>,
    address: Option<Res<LocalServerAddress>>,
) {
    info!("App exiting, closing the session");
    // The connection is kept: the netcode plugin still needs the transport to disconnect.
    stop_session(
        client.map(ResMut::into_inner),
        transport.map(ResMut::into_inner),
        local_server.map(ResMut::into_inner),
        goes_to_local_server(&target, address.as_deref()),
    );
}

/// Whether the connection of the session goes to the embedded server: the session opened a
/// world rather than joined a server.
fn goes_to_local_server(target: &TargetServer, address: Option<&LocalServerAddress>) -> bool {
    matches!((target.address, address), (Some(target), Some(local)) if target == local.0)
}

/// Tells the server that the player leaves, sending the message right away since the regular
/// send system may not run again, then, when that server is the embedded one
/// (`to_local_server`), waits for its thread to finish saving.
///
/// `Exit` cannot be sent before the connection is established. The embedded server is then
/// left running and its handle is kept, so that `finish_stopping_local_server` can stop it
/// later. So is the handle of an embedded server left running by an earlier session while this
/// one joined another server, without waiting for it, and that of a server that does not stop
/// in time.
///
/// Returns whether the client and the transport are no longer needed. They are still needed
/// to send `Exit` once connected to an embedded server that was not told yet.
fn stop_session(
    client: Option<&mut RenetClient>,
    transport: Option<&mut NetcodeClientTransport>,
    local_server: Option<&mut LocalServerHandle>,
    to_local_server: bool,
) -> bool {
    let mut exit_sent = false;
    if let (Some(client), Some(transport)) = (client, transport) {
        if client.is_connected() {
            client.send_game_message(ClientToServerMessage::Exit);
            match transport.send_packets(client) {
                Ok(()) => exit_sent = true,
                Err(err) => error!("Could not send the exit message: {err}"),
            }
        }
    }

    let Some(local_server) = local_server else {
        return true;
    };
    let Some(handle) = local_server.0.take() else {
        return true;
    };
    if (exit_sent && to_local_server) || handle.is_finished() {
        local_server.0 = wait_for_local_server(handle, LOCAL_SERVER_STOP_TIMEOUT);
        true
    } else {
        warn!("The local server was not told to stop, it is still running");
        local_server.0 = Some(handle);
        // a connection to another server cannot tell it
        !to_local_server
    }
}

/// Stops an embedded server that was still running when its session ended, as soon as the
/// connection to it is established, then removes the connection. A connection that gave up
/// before the server answered (a world slow to load), or that another session replaced and
/// removed, is opened again. Does not block: runs every frame in the menu.
pub fn finish_stopping_local_server(
    mut commands: Commands,
    client: Option<ResMut<RenetClient>>,
    transport: Option<ResMut<NetcodeClientTransport>>,
    local_server: Option<ResMut<LocalServerHandle>>,
    address: Option<Res<LocalServerAddress>>,
    profile: Option<Res<CurrentPlayerProfile>>,
    mut open_failed: Local<bool>,
) {
    let Some(mut local_server) = local_server else {
        return;
    };
    let Some(handle) = local_server.0.as_ref() else {
        return;
    };
    if handle.is_finished() {
        if let Some(handle) = local_server.0.take() {
            join_local_server(handle);
        }
        drop_connection(&mut commands);
        return;
    }

    match (client, transport) {
        (Some(mut client), Some(mut transport)) if !client.is_disconnected() => {
            if client.is_connected() {
                info!("Stopping the local server left running by a cancelled connection");
                client.send_game_message(ClientToServerMessage::Exit);
                if let Err(err) = transport.send_packets(&mut client) {
                    error!("Could not send the exit message: {err}");
                }
                drop_connection(&mut commands);
            }
        }
        _ => {
            let (Some(LocalServerAddress(addr)), Some(profile)) =
                (address.as_deref(), profile.as_deref())
            else {
                return;
            };
            match open_connection(*addr, profile.id) {
                Ok((client, transport)) => {
                    info!("Connecting again to the local server left running, to stop it");
                    commands.insert_resource(client);
                    commands.insert_resource(transport);
                    *open_failed = false;
                }
                // tried again every frame: said once
                Err(err) if !*open_failed => {
                    error!("Could not open a connection to the local server: {err}");
                    *open_failed = true;
                }
                Err(_) => (),
            }
        }
    }
}

/// Joins the thread of an embedded server that finished, and logs why it stopped when it failed.
pub(crate) fn join_local_server(handle: std::thread::JoinHandle<Result<(), String>>) {
    match handle.join() {
        Ok(Ok(())) => (),
        Ok(Err(err)) => error!("Local server stopped on an error: {err}"),
        Err(_) => error!("Local server thread panicked"),
    }
}

/// Waits for the server thread to stop. Gives the handle back when it did not stop in time.
fn wait_for_local_server(
    handle: std::thread::JoinHandle<Result<(), String>>,
    timeout: Duration,
) -> Option<std::thread::JoinHandle<Result<(), String>>> {
    let deadline = Instant::now() + timeout;
    while !handle.is_finished() {
        if Instant::now() >= deadline {
            warn!("Local server did not stop within {timeout:?}, keeping its handle");
            return Some(handle);
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    join_local_server(handle);
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::menus::solo::SelectedWorld;
    use crate::network::add_base_netcode;
    use crate::network::establish_authenticated_connection_to_server;
    use crate::network::init_server_connection;
    use crate::network::launch_local_server_system;
    use crate::network::session::{session_plugin, ConnectError, SessionEnd, SessionOrigin};
    use crate::network::status::StatusProbe;
    use crate::state::GameState;
    use crate::world::time::ClientTime;
    use crate::world::ClientWorldMap;
    use bevy::ecs::system::RunSystemOnce;
    use bevy::state::app::StatesPlugin;
    use bevy_renet::netcode::{
        ClientAuthentication, NetcodeServerTransport, ServerAuthentication, ServerConfig,
    };
    use bevy_renet::RenetServer;
    use shared::messages::AuthRegisterRequest;
    use shared::messages::PlayerSpawnEvent;
    use shared::{
        get_shared_renet_config, GameFolderPaths, GameServerConfig, PROTOCOL_ID, STC_AUTH_CHANNEL,
    };
    use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};
    use std::path::PathBuf;
    use std::time::SystemTime;

    const TIMEOUT: Duration = Duration::from_secs(10);

    /// Runs frames of `app` until `done` holds. Fails the test after `TIMEOUT`.
    fn update_until(app: &mut App, what: &str, done: impl Fn(&mut App) -> bool) {
        let start = Instant::now();
        while !done(app) {
            assert!(start.elapsed() < TIMEOUT, "timed out waiting for {what}");
            app.update();
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    /// A real embedded server and the real client networking (`add_base_netcode`), without a
    /// window, with the player connected and authenticated.
    struct SoloSession {
        app: App,
        game_folder: PathBuf,
        world_name: String,
    }

    impl SoloSession {
        fn start(name: &str, client_id: u64) -> Self {
            let mut session = Self::connecting(name, client_id);
            session.finish_connecting(client_id);
            session
        }

        /// The embedded server is running and the client has started to connect.
        fn connecting(name: &str, client_id: u64) -> Self {
            let game_folder =
                std::env::temp_dir().join(format!("rustcraft-{name}-{}", std::process::id()));
            let paths = GameFolderPaths {
                game_folder_path: game_folder.clone(),
                assets_folder_path: game_folder.join("assets"),
            };
            let socket =
                server::acquire_local_ephemeral_udp_socket(IpAddr::V4(Ipv4Addr::LOCALHOST))
                    .unwrap();
            let addr = socket.local_addr().unwrap();
            let config = GameServerConfig {
                world_name: name.to_string(),
                is_solo: true,
            };
            let server = std::thread::spawn(move || server::init(socket, config, paths));

            let mut app = App::new();
            app.add_plugins(MinimalPlugins);
            add_base_netcode(&mut app);
            app.init_resource::<UnacknowledgedInputs>()
                .init_resource::<PlayerTickInputsBuffer>();
            app.insert_resource(LocalServerHandle(Some(server)))
                .insert_resource(LocalServerAddress(addr))
                .insert_resource(CurrentPlayerProfile {
                    id: client_id,
                    name: name.to_string(),
                });
            app.world_mut().resource_mut::<TargetServer>().address = Some(addr);
            let now = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap();
            let authentication = ClientAuthentication::Unsecure {
                server_addr: addr,
                client_id,
                user_data: None,
                protocol_id: PROTOCOL_ID,
            };
            let transport = NetcodeClientTransport::new(
                now,
                authentication,
                UdpSocket::bind("127.0.0.1:0").unwrap(),
            )
            .unwrap();
            app.insert_resource(transport);
            app.insert_resource(RenetClient::new(get_shared_renet_config()));

            Self {
                app,
                game_folder,
                world_name: name.to_string(),
            }
        }

        /// Connects and authenticates the player.
        fn finish_connecting(&mut self, client_id: u64) {
            let username = self.world_name.clone();
            update_until(&mut self.app, "the connection", |app| {
                app.world().resource::<RenetClient>().is_connected()
            });
            self.app
                .world_mut()
                .resource_mut::<RenetClient>()
                .send_game_message(AuthRegisterRequest { username }.into());
            update_until(&mut self.app, "the authentication", |app| {
                let mut client = app.world_mut().resource_mut::<RenetClient>();
                client
                    .receive_game_message_by_channel(STC_AUTH_CHANNEL)
                    .is_some()
            });
            self.app
                .world_mut()
                .resource_mut::<TargetServer>()
                .session_token = Some(client_id);
        }

        fn assert_saved_and_stopped(self, client_id: u64) {
            assert!(self.app.world().resource::<LocalServerHandle>().0.is_none());
            let world_dir = self.game_folder.join("saves").join(&self.world_name);
            assert!(world_dir.join("world.ron").is_file(), "world not saved");
            assert!(
                std::fs::read_dir(world_dir.join("chunks"))
                    .is_ok_and(|mut chunks| chunks.next().is_some()),
                "no chunk saved"
            );
            assert!(
                world_dir.join(format!("players/{client_id}.ron")).is_file(),
                "player not saved"
            );
            std::fs::remove_dir_all(&self.game_folder).unwrap();
        }
    }

    #[test]
    fn app_exit_saves_the_session_and_stops_the_local_server() {
        let mut session = SoloSession::start("exit-test", 7);

        // The window is closed: nothing but `AppExit` reaches the app.
        session.app.world_mut().write_message(AppExit::Success);
        let start = Instant::now();
        session.app.update();

        assert!(
            start.elapsed() < LOCAL_SERVER_STOP_TIMEOUT,
            "the local server took too long to stop"
        );
        session.assert_saved_and_stopped(7);
    }

    #[test]
    fn cancelling_the_loading_screen_saves_the_session_and_stops_the_local_server() {
        let mut session = SoloSession::start("cancel-test", 8);
        session
            .app
            .add_plugins(StatesPlugin)
            .insert_state(GameState::PreGameLoading)
            .add_systems(
                OnTransition {
                    exited: GameState::PreGameLoading,
                    entered: GameState::Menu,
                },
                terminate_server_connection,
            );
        session.app.update();

        session
            .app
            .world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Menu);
        let start = Instant::now();
        session.app.update();

        assert!(
            start.elapsed() < LOCAL_SERVER_STOP_TIMEOUT,
            "the local server took too long to stop"
        );
        let target = session.app.world().resource::<TargetServer>();
        assert!(target.address.is_none() && target.session_token.is_none());
        session.assert_saved_and_stopped(8);
    }

    /// The cancelled connection could not tell the server to stop: it is stopped once the
    /// connection is up, and the handle is released.
    #[test]
    fn a_server_left_running_is_stopped_once_the_client_is_connected() {
        let mut session = SoloSession::start("pending-test", 9);
        session
            .app
            .add_systems(Update, finish_stopping_local_server);

        update_until(&mut session.app, "the local server to stop", |app| {
            app.world().resource::<LocalServerHandle>().0.is_none()
        });
        session.assert_saved_and_stopped(9);
    }

    /// The connection gave up before the server answered, a world slow to load: a new one is
    /// opened to tell the server to stop.
    #[test]
    fn a_server_left_running_is_stopped_even_after_the_connection_gave_up() {
        let mut session = SoloSession::connecting("gave-up-test", 12);
        session
            .app
            .world_mut()
            .resource_mut::<NetcodeClientTransport>()
            .disconnect();
        session
            .app
            .add_systems(Update, finish_stopping_local_server);

        update_until(&mut session.app, "the local server to stop", |app| {
            app.world().resource::<LocalServerHandle>().0.is_none()
        });
        let world_dir = session.game_folder.join("saves").join("gave-up-test");
        assert!(world_dir.join("world.ron").is_file(), "world not saved");
        std::fs::remove_dir_all(&session.game_folder).unwrap();
    }

    /// A server join replaced the connection of a world still starting, then ended: a
    /// connection is opened to tell the world's server to stop.
    #[test]
    fn a_server_left_running_is_stopped_even_without_a_connection() {
        let mut session = SoloSession::connecting("no-client-test", 14);
        session.app.world_mut().remove_resource::<RenetClient>();
        session
            .app
            .world_mut()
            .remove_resource::<NetcodeClientTransport>();
        session
            .app
            .add_systems(Update, finish_stopping_local_server);

        update_until(&mut session.app, "the local server to stop", |app| {
            app.world().resource::<LocalServerHandle>().0.is_none()
        });
        let world_dir = session.game_folder.join("saves").join("no-client-test");
        assert!(world_dir.join("world.ron").is_file(), "world not saved");
        std::fs::remove_dir_all(&session.game_folder).unwrap();
    }

    /// The first connection to a world gave up before its server answered, a world slower to
    /// load than netcode waits: the new connection authenticates.
    #[test]
    fn a_world_whose_first_connection_gave_up_still_authenticates() {
        let mut session = SoloSession::connecting("slow-test", 15);
        session
            .app
            .add_plugins(StatesPlugin)
            .insert_state(GameState::PreGameLoading)
            .add_message::<PlayerSpawnEvent>()
            .insert_resource(ClientTime(0))
            .init_resource::<ClientWorldMap>()
            .init_resource::<StatusProbe>()
            .insert_resource(SessionOrigin::World {
                name: "slow-test".to_string(),
            })
            .add_systems(Update, establish_authenticated_connection_to_server);
        session_plugin(&mut session.app);
        // the authentication request is queued on the first connection, which then gives up
        session.app.update();
        assert_eq!(
            session.app.world().resource::<TargetServer>().state,
            TargetServerState::Establishing
        );
        session
            .app
            .world_mut()
            .resource_mut::<NetcodeClientTransport>()
            .disconnect();

        update_until(&mut session.app, "the authentication", |app| {
            app.world().resource::<TargetServer>().state == TargetServerState::ConnectionEstablished
        });
        assert!(!session.app.world().contains_resource::<SessionEnd>());
        session
            .app
            .world_mut()
            .run_system_once(terminate_server_connection)
            .unwrap();
        assert!(session
            .app
            .world()
            .resource::<LocalServerHandle>()
            .0
            .is_none());
        std::fs::remove_dir_all(&session.game_folder).unwrap();
    }

    #[test]
    fn a_local_server_that_already_stopped_is_not_busy() {
        let stopped = std::thread::spawn(|| Err("saves/world/world.ron: bad".to_string()));
        while !stopped.is_finished() {
            std::thread::sleep(Duration::from_millis(1));
        }
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, StatesPlugin))
            .init_state::<GameState>()
            .insert_resource(LocalServerHandle(Some(stopped)))
            // no world: the launch ends there, after the busy check
            .insert_resource(SelectedWorld { name: None })
            .insert_resource(SessionOrigin::World {
                name: "world".to_string(),
            })
            .insert_resource(GameFolderPaths {
                game_folder_path: std::env::temp_dir().join("rustcraft-never-used"),
                assets_folder_path: std::env::temp_dir().join("rustcraft-never-used"),
            })
            .insert_resource(TargetServer {
                address: None,
                username: None,
                session_token: None,
                state: TargetServerState::Initial,
            })
            .add_systems(Update, launch_local_server_system);
        app.update();

        assert_eq!(
            app.world().resource::<SessionEnd>().error,
            Some(ConnectError::WorldFailed("No world was selected".into()))
        );
        assert!(app.world().resource::<LocalServerHandle>().0.is_none());
    }

    fn has_connection(app: &App) -> bool {
        app.world().get_resource::<RenetClient>().is_some()
            || app
                .world()
                .get_resource::<NetcodeClientTransport>()
                .is_some()
    }

    #[test]
    fn terminating_an_established_session_removes_the_connection() {
        let mut session = SoloSession::start("drop-test", 10);
        assert!(has_connection(&session.app));

        session
            .app
            .world_mut()
            .run_system_once(terminate_server_connection)
            .unwrap();

        assert!(!has_connection(&session.app));
        session.assert_saved_and_stopped(10);
    }

    /// `Exit` cannot be sent before the connection is established: the connection is kept
    /// until `finish_stopping_local_server` has sent it.
    #[test]
    fn the_connection_of_a_cancelled_session_is_kept_until_the_server_is_told_to_stop() {
        let mut session = SoloSession::connecting("cancelled-drop-test", 11);
        session
            .app
            .world_mut()
            .run_system_once(terminate_server_connection)
            .unwrap();
        assert!(!session.app.world().resource::<RenetClient>().is_connected());
        assert!(has_connection(&session.app));
        assert!(session
            .app
            .world()
            .resource::<LocalServerHandle>()
            .0
            .is_some());

        session
            .app
            .add_systems(Update, finish_stopping_local_server);
        update_until(&mut session.app, "the local server to stop", |app| {
            app.world().resource::<LocalServerHandle>().0.is_none()
        });
        session.app.update();

        assert!(!has_connection(&session.app));
        // The player never authenticated, so only the world is saved.
        assert!(session
            .app
            .world()
            .resource::<LocalServerHandle>()
            .0
            .is_none());
        std::fs::remove_dir_all(&session.game_folder).unwrap();
    }

    /// A client connected to a bare server of the test, at the address returned, which is no
    /// local server. The server is dropped: the client stays connected until it times out.
    fn connected_to_another_server() -> (RenetClient, NetcodeClientTransport, SocketAddr) {
        let socket = UdpSocket::bind("127.0.0.1:0").unwrap();
        let addr = socket.local_addr().unwrap();
        let config = ServerConfig {
            current_time: SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap(),
            max_clients: 1,
            protocol_id: PROTOCOL_ID,
            public_addresses: vec![addr],
            authentication: ServerAuthentication::Unsecure,
        };
        let mut server = RenetServer::new(get_shared_renet_config());
        let mut server_transport = NetcodeServerTransport::new(config, socket).unwrap();
        let (mut client, mut transport) = open_connection(addr, 16).unwrap();
        let step = Duration::from_millis(5);
        let start = Instant::now();
        while !client.is_connected() {
            assert!(start.elapsed() < TIMEOUT, "never connected");
            client.update(step);
            transport.update(step, &mut client).unwrap();
            transport.send_packets(&mut client).unwrap();
            server.update(step);
            server_transport.update(step, &mut server).unwrap();
            server_transport.send_packets(&mut server);
            std::thread::sleep(step);
        }
        (client, transport, addr)
    }

    /// A server joined while the world of an earlier session still closes: leaving it tells
    /// that server, not the world's, and so does not wait for the world.
    #[test]
    fn a_session_to_another_server_ends_without_waiting_for_an_earlier_world() {
        let (release, wait) = std::sync::mpsc::channel::<()>();
        let running = std::thread::spawn(move || {
            let _ = wait.recv();
            Ok(())
        });
        let (client, transport, addr) = connected_to_another_server();
        let mut app = App::new();
        app.init_resource::<UnacknowledgedInputs>()
            .init_resource::<PlayerTickInputsBuffer>()
            .insert_resource(client)
            .insert_resource(transport)
            .insert_resource(LocalServerHandle(Some(running)))
            .insert_resource(LocalServerAddress("127.0.0.1:9".parse().unwrap()))
            .insert_resource(TargetServer {
                address: Some(addr),
                username: Some("Tester".into()),
                session_token: Some(16),
                state: TargetServerState::FullyReady,
            });

        let start = Instant::now();
        app.world_mut()
            .run_system_once(terminate_server_connection)
            .unwrap();

        assert!(
            start.elapsed() < Duration::from_secs(1),
            "waited {:?}",
            start.elapsed()
        );
        let handle = app.world().resource::<LocalServerHandle>().0.as_ref();
        assert!(handle.is_some_and(|handle| !handle.is_finished()));
        // the world's server is told to stop on a connection of its own
        assert!(!has_connection(&app));
        release.send(()).unwrap();
    }

    /// The launch of the world ended the session (`WorldBusy`, no socket): the connection that
    /// follows it in `OnEnter(PreGameLoading)` stands down.
    #[test]
    fn no_connection_is_opened_once_the_session_ended() {
        let mut app = App::new();
        app.init_resource::<NextState<GameState>>()
            .insert_resource(CurrentPlayerProfile {
                id: 17,
                name: "Tester".into(),
            })
            .insert_resource(TargetServer {
                address: Some("127.0.0.1:9".parse().unwrap()),
                username: None,
                session_token: None,
                state: TargetServerState::Initial,
            });
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Menu);

        app.world_mut()
            .run_system_once(init_server_connection)
            .unwrap();

        assert!(!has_connection(&app));
    }

    #[test]
    fn a_second_local_server_is_refused_while_the_first_one_runs() {
        let (release, wait) = std::sync::mpsc::channel::<()>();
        let running = std::thread::spawn(move || {
            let _ = wait.recv();
            Ok(())
        });

        let mut app = App::new();
        app.add_plugins((MinimalPlugins, StatesPlugin))
            .init_state::<GameState>()
            .insert_resource(LocalServerHandle(Some(running)))
            .insert_resource(SelectedWorld {
                name: Some("world".to_string()),
            })
            .insert_resource(SessionOrigin::World {
                name: "world".to_string(),
            })
            .insert_resource(GameFolderPaths {
                game_folder_path: std::env::temp_dir().join("rustcraft-never-used"),
                assets_folder_path: std::env::temp_dir().join("rustcraft-never-used"),
            })
            .insert_resource(TargetServer {
                address: None,
                username: None,
                session_token: None,
                state: TargetServerState::Initial,
            })
            .add_systems(Update, launch_local_server_system);
        app.update();

        assert!(app.world().resource::<TargetServer>().address.is_none());
        assert!(matches!(
            app.world().resource::<NextState<GameState>>(),
            NextState::Pending(GameState::Menu)
        ));
        assert_eq!(
            app.world().resource::<SessionEnd>().error,
            Some(ConnectError::WorldBusy)
        );
        let handle = app.world().resource::<LocalServerHandle>().0.as_ref();
        assert!(handle.is_some_and(|handle| !handle.is_finished()));

        release.send(()).unwrap();
    }
}
