use crate::network::{
    LocalServerHandle, SendGameMessageExtension, TargetServer, TargetServerState,
};
use bevy::prelude::*;
use bevy_renet::netcode::NetcodeClientTransport;
use bevy_renet::renet::RenetClient;
use shared::messages::ClientToServerMessage;
use std::time::{Duration, Instant};

use super::{buffered_client::PlayerTickInputsBuffer, UnacknowledgedInputs};

/// Ends the session when the player leaves the game or cancels the connection: tells the
/// server, waits for the embedded server (if any) to save and stop, and resets the
/// connection state.
pub fn terminate_server_connection(
    mut target: ResMut<TargetServer>,
    mut unacknowledged_inputs: ResMut<UnacknowledgedInputs>,
    mut current_frame: ResMut<PlayerTickInputsBuffer>,
    client: Option<ResMut<RenetClient>>,
    transport: Option<ResMut<NetcodeClientTransport>>,
    local_server: Option<ResMut<LocalServerHandle>>,
) {
    info!("Terminating server connection");
    stop_session(
        client.map(ResMut::into_inner),
        transport.map(ResMut::into_inner),
        local_server.map(ResMut::into_inner),
    );

    target.address = None;
    target.username = None;
    target.session_token = None;
    target.state = TargetServerState::Initial;

    unacknowledged_inputs.0.clear();
    current_frame.buffer.clear();
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
) {
    info!("App exiting, closing the session");
    stop_session(
        client.map(ResMut::into_inner),
        transport.map(ResMut::into_inner),
        local_server.map(ResMut::into_inner),
    );
}

/// Tells the server that the player leaves, sending the message right away since the regular
/// send system may not run again, then waits for the embedded server thread to finish saving.
///
/// `Exit` cannot be sent before the connection is established. The embedded server is then
/// left running and its handle is kept, so that `finish_stopping_local_server` can stop it
/// later. The handle is also kept when the server does not stop in time.
fn stop_session(
    client: Option<&mut RenetClient>,
    transport: Option<&mut NetcodeClientTransport>,
    local_server: Option<&mut LocalServerHandle>,
) {
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
        return;
    };
    let Some(handle) = local_server.0.take() else {
        return;
    };
    if exit_sent || handle.is_finished() {
        local_server.0 = wait_for_local_server(handle, LOCAL_SERVER_STOP_TIMEOUT);
    } else {
        warn!("The local server was not told to stop, it is still running");
        local_server.0 = Some(handle);
    }
}

/// Stops an embedded server that was still running when its session ended, as soon as the
/// connection to it is established. Does not block: runs every frame in the menu.
pub fn finish_stopping_local_server(
    client: Option<ResMut<RenetClient>>,
    transport: Option<ResMut<NetcodeClientTransport>>,
    local_server: Option<ResMut<LocalServerHandle>>,
) {
    let Some(mut local_server) = local_server else {
        return;
    };
    let Some(handle) = local_server.0.as_ref() else {
        return;
    };
    if handle.is_finished() {
        if let Some(handle) = local_server.0.take() {
            if handle.join().is_err() {
                error!("Local server thread panicked");
            }
        }
        return;
    }

    if let (Some(mut client), Some(mut transport)) = (client, transport) {
        if client.is_connected() {
            info!("Stopping the local server left running by a cancelled connection");
            client.send_game_message(ClientToServerMessage::Exit);
            if let Err(err) = transport.send_packets(&mut client) {
                error!("Could not send the exit message: {err}");
            }
        }
    }
}

/// Waits for the server thread to stop. Gives the handle back when it did not stop in time.
fn wait_for_local_server(
    handle: std::thread::JoinHandle<()>,
    timeout: Duration,
) -> Option<std::thread::JoinHandle<()>> {
    let deadline = Instant::now() + timeout;
    while !handle.is_finished() {
        if Instant::now() >= deadline {
            warn!("Local server did not stop within {timeout:?}, keeping its handle");
            return Some(handle);
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    if handle.join().is_err() {
        error!("Local server thread panicked");
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::menus::solo::SelectedWorld;
    use crate::network::add_base_netcode;
    use crate::network::launch_local_server_system;
    use crate::GameState;
    use bevy::state::app::StatesPlugin;
    use bevy_renet::netcode::ClientAuthentication;
    use shared::messages::AuthRegisterRequest;
    use shared::{
        get_shared_renet_config, GameFolderPaths, GameServerConfig, PROTOCOL_ID, STC_AUTH_CHANNEL,
    };
    use std::net::{IpAddr, Ipv4Addr, UdpSocket};
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
            let game_folder =
                std::env::temp_dir().join(format!("rustcraft-{name}-{}", std::process::id()));
            let paths = GameFolderPaths {
                game_folder_path: game_folder.clone(),
                assets_folder_path: game_folder.join("assets"),
            };
            let socket =
                server::acquire_local_ephemeral_udp_socket(IpAddr::V4(Ipv4Addr::LOCALHOST));
            let addr = socket.local_addr().unwrap();
            let config = GameServerConfig {
                world_name: name.to_string(),
                is_solo: true,
            };
            let server = std::thread::spawn(move || server::init(socket, config, paths).unwrap());

            let mut app = App::new();
            app.add_plugins(MinimalPlugins);
            add_base_netcode(&mut app);
            app.init_resource::<UnacknowledgedInputs>()
                .init_resource::<PlayerTickInputsBuffer>();
            app.insert_resource(LocalServerHandle(Some(server)));
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

            update_until(&mut app, "the connection", |app| {
                app.world().resource::<RenetClient>().is_connected()
            });
            app.world_mut()
                .resource_mut::<RenetClient>()
                .send_game_message(
                    AuthRegisterRequest {
                        username: name.to_string(),
                    }
                    .into(),
                );
            update_until(&mut app, "the authentication", |app| {
                let mut client = app.world_mut().resource_mut::<RenetClient>();
                client
                    .receive_game_message_by_channel(STC_AUTH_CHANNEL)
                    .is_some()
            });
            app.world_mut().resource_mut::<TargetServer>().session_token = Some(client_id);

            Self {
                app,
                game_folder,
                world_name: name.to_string(),
            }
        }

        fn assert_saved_and_stopped(self, client_id: u64) {
            assert!(self.app.world().resource::<LocalServerHandle>().0.is_none());
            let world_dir = self.game_folder.join("saves").join(&self.world_name);
            assert!(world_dir.join("world.ron").is_file(), "world not saved");
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

    #[test]
    fn a_second_local_server_is_refused_while_the_first_one_runs() {
        let (release, wait) = std::sync::mpsc::channel::<()>();
        let running = std::thread::spawn(move || {
            let _ = wait.recv();
        });

        let mut app = App::new();
        app.add_plugins((MinimalPlugins, StatesPlugin))
            .init_state::<GameState>()
            .insert_resource(LocalServerHandle(Some(running)))
            .insert_resource(SelectedWorld {
                name: Some("world".to_string()),
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
        let handle = app.world().resource::<LocalServerHandle>().0.as_ref();
        assert!(handle.is_some_and(|handle| !handle.is_finished()));

        release.send(()).unwrap();
    }
}
