use crate::network::{
    LocalServerHandle, SendGameMessageExtension, TargetServer, TargetServerState,
};
use bevy::prelude::*;
use bevy_renet::netcode::NetcodeClientTransport;
use bevy_renet::renet::RenetClient;
use shared::messages::ClientToServerMessage;
use std::time::{Duration, Instant};

use super::{buffered_client::PlayerTickInputsBuffer, UnacknowledgedInputs};

pub fn terminate_server_connection(
    mut client: ResMut<RenetClient>,
    mut target: ResMut<TargetServer>,
    mut unacknowledged_inputs: ResMut<UnacknowledgedInputs>,
    mut current_frame: ResMut<PlayerTickInputsBuffer>,
) {
    info!("Terminating server connection");
    client.send_game_message(ClientToServerMessage::Exit);

    target.address = None;
    target.username = None;
    target.session_token = None;
    target.state = TargetServerState::Initial;

    unacknowledged_inputs.0.clear();
    current_frame.buffer.clear();
}

/// How long the app waits for the embedded server to save and stop when it exits.
const LOCAL_SERVER_STOP_TIMEOUT: Duration = Duration::from_secs(5);

/// Closes the session when the app exits (window closed, quit button), which never goes
/// through `OnExit(GameState::Game)`: tells the server, sending the message right away
/// since the regular send system may already have run this frame, then waits for the
/// embedded server thread to finish saving.
pub fn close_session_on_app_exit(
    target: Res<TargetServer>,
    client: Option<ResMut<RenetClient>>,
    transport: Option<ResMut<NetcodeClientTransport>>,
    local_server: Option<ResMut<LocalServerHandle>>,
) {
    // `terminate_server_connection` already sent `Exit` and reset the address when the
    // player had left the game before closing the app.
    let mut server_told_to_exit = target.address.is_none();

    if target.address.is_some() {
        if let (Some(mut client), Some(mut transport)) = (client, transport) {
            if client.is_connected() {
                info!("App exiting, closing the session");
                client.send_game_message(ClientToServerMessage::Exit);
                match transport.send_packets(&mut client) {
                    Ok(()) => server_told_to_exit = true,
                    Err(err) => error!("Could not send the exit message: {err}"),
                }
            }
        }
    }

    let handle = local_server.and_then(|mut local_server| local_server.0.take());
    if let (Some(handle), true) = (handle, server_told_to_exit) {
        wait_for_local_server(handle, LOCAL_SERVER_STOP_TIMEOUT);
    }
}

fn wait_for_local_server(handle: std::thread::JoinHandle<()>, timeout: Duration) {
    let deadline = Instant::now() + timeout;
    while !handle.is_finished() {
        if Instant::now() >= deadline {
            warn!("Local server did not stop within {timeout:?}, giving up");
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    if handle.join().is_err() {
        error!("Local server thread panicked");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::network::add_base_netcode;
    use bevy_renet::netcode::ClientAuthentication;
    use shared::messages::AuthRegisterRequest;
    use shared::{
        get_shared_renet_config, GameFolderPaths, GameServerConfig, PROTOCOL_ID, STC_AUTH_CHANNEL,
    };
    use std::net::{IpAddr, Ipv4Addr, UdpSocket};
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

    /// Wires the real client networking (`add_base_netcode`) to a real embedded server,
    /// without a window, and exits the app in the middle of a session.
    #[test]
    fn app_exit_saves_the_session_and_stops_the_local_server() {
        let game_folder =
            std::env::temp_dir().join(format!("rustcraft-exit-{}", std::process::id()));
        let paths = GameFolderPaths {
            game_folder_path: game_folder.clone(),
            assets_folder_path: game_folder.join("assets"),
        };
        let socket = server::acquire_local_ephemeral_udp_socket(IpAddr::V4(Ipv4Addr::LOCALHOST));
        let addr = socket.local_addr().unwrap();
        let config = GameServerConfig {
            world_name: "exit-test".to_string(),
            is_solo: true,
        };
        let server = std::thread::spawn(move || server::init(socket, config, paths).unwrap());

        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        add_base_netcode(&mut app);
        app.insert_resource(LocalServerHandle(Some(server)));
        app.world_mut().resource_mut::<TargetServer>().address = Some(addr);
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap();
        let authentication = ClientAuthentication::Unsecure {
            server_addr: addr,
            client_id: 7,
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
                    username: "quitter".to_string(),
                }
                .into(),
            );
        update_until(&mut app, "the authentication", |app| {
            let mut client = app.world_mut().resource_mut::<RenetClient>();
            client
                .receive_game_message_by_channel(STC_AUTH_CHANNEL)
                .is_some()
        });

        // The window is closed: nothing but `AppExit` reaches the app.
        app.world_mut().send_event(AppExit::Success);
        let start = Instant::now();
        app.update();

        assert!(
            start.elapsed() < LOCAL_SERVER_STOP_TIMEOUT,
            "the local server took too long to stop"
        );
        assert!(app.world().resource::<LocalServerHandle>().0.is_none());
        let world_dir = game_folder.join("saves/exit-test");
        assert!(world_dir.join("world.ron").is_file(), "world not saved");
        assert!(
            world_dir.join("players/7.ron").is_file(),
            "player not saved"
        );
        std::fs::remove_dir_all(game_folder).unwrap();
    }
}
