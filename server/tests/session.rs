//! End-to-end test of a solo session: a real server thread driven over loopback UDP by a
//! renet client set up the same way as `client/src/network/setup.rs`.

use bevy_renet::netcode::{ClientAuthentication, NetcodeClientTransport};
use bevy_renet::renet::RenetClient;
use shared::messages::{
    AuthRegisterRequest, ChatMessageRequest, ClientToServerMessage, PlayerFrameInput,
    ServerToClientMessage,
};
use shared::{
    game_message_to_payload, get_shared_renet_config, payload_to_game_message,
    ChannelResolvableExt, GameFolderPaths, GameServerConfig, PROTOCOL_ID, STC_AUTH_CHANNEL,
    STC_CHUNK_DATA_CHANNEL, STC_STANDARD_CHANNEL,
};
use std::net::{IpAddr, Ipv4Addr, UdpSocket};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant, SystemTime};

const WAIT_TIMEOUT: Duration = Duration::from_secs(10);
const PUMP_INTERVAL: Duration = Duration::from_millis(5);
const PLAYER_ID: u64 = 42;
const WORLD_NAME: &str = "session-test";

struct TestServer {
    tmp: tempfile::TempDir,
    addr: std::net::SocketAddr,
    handle: Option<JoinHandle<()>>,
}

impl TestServer {
    /// Starts a solo server on an ephemeral loopback port, saving into a temporary folder.
    fn start(world_name: &str) -> Self {
        let tmp = tempfile::tempdir().unwrap();
        let paths = GameFolderPaths {
            game_folder_path: tmp.path().to_path_buf(),
            assets_folder_path: tmp.path().join("assets"),
        };
        let socket = server::acquire_local_ephemeral_udp_socket(IpAddr::V4(Ipv4Addr::LOCALHOST));
        let addr = socket.local_addr().unwrap();
        let config = GameServerConfig {
            world_name: world_name.to_string(),
            is_solo: true,
        };
        let handle = thread::spawn(move || server::init(socket, config, paths));
        Self {
            tmp,
            addr,
            handle: Some(handle),
        }
    }

    fn world_dir(&self) -> std::path::PathBuf {
        self.tmp.path().join("saves").join(WORLD_NAME)
    }

    /// Waits for the server thread to stop after an `Exit` message.
    fn join_after_exit(&mut self) {
        let handle = self.handle.take().expect("server already joined");
        let start = Instant::now();
        while !handle.is_finished() {
            assert!(
                start.elapsed() < WAIT_TIMEOUT,
                "server thread did not stop within {WAIT_TIMEOUT:?} after Exit"
            );
            thread::sleep(PUMP_INTERVAL);
        }
        handle.join().expect("server thread panicked");
    }
}

struct TestClient {
    id: u64,
    client: RenetClient,
    transport: NetcodeClientTransport,
    inbox: Vec<ServerToClientMessage>,
    last_pump: Instant,
}

impl TestClient {
    fn connect(server_addr: std::net::SocketAddr, client_id: u64) -> Self {
        let authentication = ClientAuthentication::Unsecure {
            server_addr,
            client_id,
            user_data: None,
            protocol_id: PROTOCOL_ID,
        };
        let socket = UdpSocket::bind("127.0.0.1:0").unwrap();
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap();
        Self {
            id: client_id,
            client: RenetClient::new(get_shared_renet_config()),
            transport: NetcodeClientTransport::new(now, authentication, socket).unwrap(),
            inbox: Vec::new(),
            last_pump: Instant::now(),
        }
    }

    /// Advances renet once and moves every received message into the inbox. Does nothing
    /// more than sleep once the server has disconnected us: check `client.is_disconnected()`.
    fn pump(&mut self) {
        let dt = self.last_pump.elapsed();
        self.last_pump = Instant::now();
        self.client.update(dt);
        if self.transport.update(dt, &mut self.client).is_ok() {
            for channel in [
                STC_STANDARD_CHANNEL,
                STC_CHUNK_DATA_CHANNEL,
                STC_AUTH_CHANNEL,
            ] {
                while let Some(payload) = self.client.receive_message(channel) {
                    self.inbox.push(payload_to_game_message(&payload).unwrap());
                }
            }
            // Fails only when the server disconnected us, which `update` reports next time.
            let _ = self.transport.send_packets(&mut self.client);
        }
        thread::sleep(PUMP_INTERVAL);
    }

    /// Flushes queued messages without reading, for the last message before a shutdown.
    fn send_packets(&mut self) {
        self.transport.send_packets(&mut self.client).unwrap();
    }

    /// Pumps until the netcode handshake is done. Fails the test after `WAIT_TIMEOUT`.
    fn wait_connected(&mut self) {
        let start = Instant::now();
        while !self.client.is_connected() {
            assert!(
                start.elapsed() < WAIT_TIMEOUT,
                "client did not connect within {WAIT_TIMEOUT:?}"
            );
            self.pump();
        }
    }

    fn send(&mut self, message: ClientToServerMessage) {
        let channel = message.get_channel_id();
        self.client
            .send_message(channel, game_message_to_payload(message));
    }

    /// Pumps until a message accepted by `pick` arrives, removes it from the inbox and
    /// returns what `pick` extracted. Fails the test after `WAIT_TIMEOUT`.
    fn wait_for<T>(
        &mut self,
        what: &str,
        mut pick: impl FnMut(&ServerToClientMessage) -> Option<T>,
    ) -> T {
        let start = Instant::now();
        loop {
            if let Some(index) = self.inbox.iter().position(|m| pick(m).is_some()) {
                let message = self.inbox.remove(index);
                return pick(&message).unwrap();
            }
            assert!(
                start.elapsed() < WAIT_TIMEOUT,
                "timed out after {WAIT_TIMEOUT:?} waiting for {what}"
            );
            self.pump();
        }
    }
}

#[test]
fn solo_session_from_auth_to_exit() {
    let mut server = TestServer::start(WORLD_NAME);
    let mut client = TestClient::connect(server.addr, PLAYER_ID);

    let id = client.id;

    // 1. Authentication
    client.wait_connected();
    client.send(
        AuthRegisterRequest {
            username: "tester".to_string(),
        }
        .into(),
    );
    let auth = client.wait_for("AuthRegisterResponse", |m| match m {
        ServerToClientMessage::AuthRegisterResponse(r) => Some(r.clone()),
        _ => None,
    });
    assert_eq!(auth.username, "tester");
    assert_eq!(auth.session_token, client.id);

    // 2. World chunks and our own spawn
    let chunk_count = client.wait_for("WorldUpdate with chunks", |m| match m {
        ServerToClientMessage::WorldUpdate(u) if !u.new_map.is_empty() => Some(u.new_map.len()),
        _ => None,
    });
    assert!(chunk_count > 0);
    let spawn = client.wait_for("PlayerSpawn for our id", |m| match m {
        ServerToClientMessage::PlayerSpawn(s) if s.id == id => Some(s.clone()),
        _ => None,
    });
    assert_eq!(spawn.name, "tester");

    // 3. Inputs are acknowledged
    let input = PlayerFrameInput {
        time_ms: 1234,
        delta_ms: 16,
        ..Default::default()
    };
    client.send(ClientToServerMessage::PlayerInputs(vec![input]));
    let update = client.wait_for("PlayerUpdate acknowledging the input", |m| match m {
        ServerToClientMessage::PlayerUpdate(u) if u.id == id && u.last_ack_time == 1234 => {
            Some(u.clone())
        }
        _ => None,
    });
    assert_eq!(update.last_ack_time, 1234);

    // 4. Save: the files must exist before we ask the server to exit
    client.send(ClientToServerMessage::SaveWorldRequest);
    client.send_packets();
    let world_dir = server.world_dir();
    let player_file = world_dir.join("players").join(format!("{}.ron", client.id));
    let start = Instant::now();
    while !(world_dir.join("world.ron").is_file() && player_file.is_file()) {
        assert!(
            start.elapsed() < WAIT_TIMEOUT,
            "world.ron and the player file were not saved within {WAIT_TIMEOUT:?}"
        );
        client.pump();
    }

    // 5. Exit
    client.send(ClientToServerMessage::Exit);
    // Only send: the server disconnects us as it shuts down.
    client.send_packets();
    server.join_after_exit();
}

fn auth_request(username: &str) -> ClientToServerMessage {
    AuthRegisterRequest {
        username: username.to_string(),
    }
    .into()
}

fn inputs_at(time_ms: u64) -> ClientToServerMessage {
    ClientToServerMessage::PlayerInputs(vec![PlayerFrameInput {
        time_ms,
        delta_ms: 16,
        ..Default::default()
    }])
}

#[test]
fn messages_before_authentication_are_ignored() {
    let mut server = TestServer::start(WORLD_NAME);
    let mut client = TestClient::connect(server.addr, PLAYER_ID + 1);
    let id = client.id;
    client.wait_connected();

    // Neither of these may crash the server, nor be acted upon.
    client.send(ClientToServerMessage::ChatMessage(ChatMessageRequest {
        content: "hello before auth".to_string(),
    }));
    client.send(inputs_at(999));
    client.send(auth_request("late"));
    let auth = client.wait_for("AuthRegisterResponse", |m| match m {
        ServerToClientMessage::AuthRegisterResponse(r) => Some(r.clone()),
        _ => None,
    });
    assert_eq!(auth.username, "late");

    // The server is alive and handles inputs sent after authenticating.
    client.send(inputs_at(2000));
    client.wait_for("PlayerUpdate acknowledging the input", |m| match m {
        ServerToClientMessage::PlayerUpdate(u) if u.id == id && u.last_ack_time == 2000 => Some(()),
        _ => None,
    });
    assert!(
        !client.inbox.iter().any(|m| matches!(
            m,
            ServerToClientMessage::PlayerUpdate(u) if u.last_ack_time == 999
        )),
        "the input sent before authentication was processed"
    );

    client.send(ClientToServerMessage::Exit);
    client.send_packets();
    server.join_after_exit();
}

#[test]
fn duplicate_username_is_disconnected_without_blocking_others() {
    let mut server = TestServer::start(WORLD_NAME);
    let mut first = TestClient::connect(server.addr, PLAYER_ID);
    let first_id = first.id;
    first.wait_connected();
    first.send(auth_request("same"));
    first.wait_for("AuthRegisterResponse", |m| match m {
        ServerToClientMessage::AuthRegisterResponse(_) => Some(()),
        _ => None,
    });

    let mut second = TestClient::connect(server.addr, PLAYER_ID + 1);
    second.wait_connected();
    second.send(auth_request("same"));
    let start = Instant::now();
    while !second.client.is_disconnected() {
        assert!(
            start.elapsed() < WAIT_TIMEOUT,
            "duplicate client was not disconnected within {WAIT_TIMEOUT:?}"
        );
        second.pump();
    }

    // The first client is still served.
    first.send(inputs_at(77));
    first.wait_for("PlayerUpdate acknowledging the input", |m| match m {
        ServerToClientMessage::PlayerUpdate(u) if u.id == first_id && u.last_ack_time == 77 => {
            Some(())
        }
        _ => None,
    });

    first.send(ClientToServerMessage::Exit);
    first.send_packets();
    server.join_after_exit();
}
