//! End-to-end test of a solo session: a real server thread driven over loopback UDP by a
//! renet client set up the same way as `client/src/network/setup.rs`.

use bevy::math::{IVec3, Vec3};
use bevy::prelude::Transform;
use bevy_renet::netcode::{ClientAuthentication, NetcodeClientTransport};
use bevy_renet::RenetClient;
use shared::messages::{
    AuthRegisterRequest, ChatMessageRequest, ClientToServerMessage, FullChatMessage, NetworkAction,
    PlayerFrameInput, ServerToClientMessage,
};
use shared::players::blocks::HIT_MS;
use shared::players::inventory::InventoryAction;
use shared::players::{Inventory, Player};
use shared::status::{StatusReply, StatusRequest};
use shared::world::{
    global_block_to_chunk_pos, grid_slot, raycast, to_local_pos, BlockData, BlockDirection,
    BlockId, Chunk, ChunkMap, FaceDirectionExt, ItemId, ItemStack,
};
use shared::{
    game_message_to_payload, get_shared_renet_config, payload_to_game_message,
    ChannelResolvableExt, GameFolderPaths, GameServerConfig, MAX_INVENTORY_SLOTS, PROTOCOL_ID,
    STC_AUTH_CHANNEL, STC_CHUNK_DATA_CHANNEL, STC_STANDARD_CHANNEL,
};
use std::net::{IpAddr, Ipv4Addr, UdpSocket};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant, SystemTime};

const WAIT_TIMEOUT: Duration = Duration::from_secs(10);
const PUMP_INTERVAL: Duration = Duration::from_millis(5);
const PLAYER_ID: u64 = 42;
const WORLD_NAME: &str = "session-test";
/// A seed whose terrain is bare grass around the spawn column, at y = 64.
const FLAT_SPAWN_SEED: u32 = 69;

struct TestServer {
    tmp: tempfile::TempDir,
    addr: std::net::SocketAddr,
    /// Where the server answers status requests, when it does.
    status_addr: Option<std::net::SocketAddr>,
    handle: Option<JoinHandle<()>>,
}

impl TestServer {
    /// Starts a solo server on an ephemeral loopback port, saving into a temporary folder.
    fn start(world_name: &str) -> Self {
        Self::start_in(tempfile::tempdir().unwrap(), world_name)
    }

    /// Same, saving into `tmp`, which may already hold the saves of an earlier server.
    fn start_in(tmp: tempfile::TempDir, world_name: &str) -> Self {
        Self::start_with(tmp, world_name, true)
    }

    /// Starts a server like the `server` binary does: it serves several players and outlives
    /// their `Exit`. Nothing stops it, so its thread lasts until the test process ends.
    fn start_dedicated(world_name: &str) -> Self {
        Self::start_with(tempfile::tempdir().unwrap(), world_name, false)
    }

    /// Same, and it answers status requests on the port next to its game port.
    fn start_dedicated_with_status(world_name: &str) -> Self {
        let (socket, status) =
            server::acquire_local_socket_pair(IpAddr::V4(Ipv4Addr::LOCALHOST)).unwrap();
        Self::start_on(
            tempfile::tempdir().unwrap(),
            world_name,
            false,
            socket,
            Some(status),
        )
    }

    fn start_with(tmp: tempfile::TempDir, world_name: &str, is_solo: bool) -> Self {
        let socket =
            server::acquire_local_ephemeral_udp_socket(IpAddr::V4(Ipv4Addr::LOCALHOST)).unwrap();
        Self::start_on(tmp, world_name, is_solo, socket, None)
    }

    fn start_on(
        tmp: tempfile::TempDir,
        world_name: &str,
        is_solo: bool,
        socket: UdpSocket,
        status: Option<UdpSocket>,
    ) -> Self {
        // A new world gets a fixed seed: a random one may put a tree or the sea at the spawn
        let world_dir = tmp.path().join("saves").join(world_name);
        if !world_dir.join("world.ron").exists() {
            std::fs::create_dir_all(&world_dir).unwrap();
            std::fs::write(
                world_dir.join("world.ron"),
                format!(
                    r#"(format_version: 2, name: "{world_name}", seed: ({FLAT_SPAWN_SEED}), time: 0, mobs: {{}})"#
                ),
            )
            .unwrap();
        }
        let paths = GameFolderPaths {
            game_folder_path: tmp.path().to_path_buf(),
            assets_folder_path: tmp.path().join("assets"),
        };
        let addr = socket.local_addr().unwrap();
        let status_addr = status.as_ref().map(|status| status.local_addr().unwrap());
        let config = GameServerConfig {
            world_name: world_name.to_string(),
            is_solo,
            saves: None,
        };
        let handle = thread::spawn(move || match status {
            Some(status) => server::init_with_status(socket, status, config, paths).unwrap(),
            None => server::init(socket, config, paths).unwrap(),
        });
        Self {
            tmp,
            addr,
            status_addr,
            handle: Some(handle),
        }
    }

    fn world_dir(&self) -> std::path::PathBuf {
        self.tmp.path().join("saves").join(WORLD_NAME)
    }

    /// The names of the files in `chunks/` of the world, sorted.
    fn chunk_files(&self) -> Vec<String> {
        let Ok(entries) = std::fs::read_dir(self.world_dir().join("chunks")) else {
            return Vec::new();
        };
        let mut names: Vec<String> = entries
            .map(|entry| entry.unwrap().file_name().into_string().unwrap())
            .collect();
        names.sort();
        names
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
    /// The blocks this client knows, built like the game client does: from the chunks it
    /// receives and the block updates applied on top.
    world: ChunkMap,
    /// Every chunk received, in order, one entry per time it was sent.
    chunk_arrivals: Vec<IVec3>,
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
            world: ChunkMap::default(),
            chunk_arrivals: Vec::new(),
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
                    let message: ServerToClientMessage = payload_to_game_message(&payload).unwrap();
                    self.apply_to_world(&message);
                    self.inbox.push(message);
                }
            }
            // Fails only when the server disconnected us, which `update` reports next time.
            let _ = self.transport.send_packets(&mut self.client);
        }
        thread::sleep(PUMP_INTERVAL);
    }

    fn apply_to_world(&mut self, message: &ServerToClientMessage) {
        match message {
            ServerToClientMessage::WorldUpdate(update) => {
                for (pos, chunk) in &update.new_map {
                    self.world.insert_chunk(*pos, chunk.clone());
                    self.chunk_arrivals.push(*pos);
                }
            }
            ServerToClientMessage::BlockUpdates(updates) => {
                for update in updates {
                    match update.block {
                        Some(block) => self.world.set_block(update.position, block),
                        None => {
                            self.world.remove_block(update.position);
                        }
                    }
                }
            }
            _ => {}
        }
    }

    /// The block updates for `position` received and not yet taken from the inbox.
    fn take_updates_for(&mut self, position: IVec3) -> Vec<Option<BlockData>> {
        let mut updates = Vec::new();
        self.inbox.retain(|message| {
            let ServerToClientMessage::BlockUpdates(batch) = message else {
                return true;
            };
            updates.extend(
                batch
                    .iter()
                    .filter(|u| u.position == position)
                    .map(|u| u.block),
            );
            false
        });
        updates
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
    while !(world_dir.join("world.ron").is_file()
        && player_file.is_file()
        && !server.chunk_files().is_empty())
    {
        assert!(
            start.elapsed() < WAIT_TIMEOUT,
            "world.ron, the chunk files and the player file were not saved within {WAIT_TIMEOUT:?}"
        );
        client.pump();
    }
    let names = server.chunk_files();
    assert!(names.iter().any(|name| name.ends_with(".bin")), "{names:?}");

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
fn solo_server_stops_on_exit_from_a_client_that_never_authenticated() {
    let mut server = TestServer::start(WORLD_NAME);
    let mut client = TestClient::connect(server.addr, PLAYER_ID + 2);
    client.wait_connected();

    client.send(ClientToServerMessage::Exit);
    client.send_packets();
    server.join_after_exit();
}

#[test]
fn duplicate_username_is_disconnected_without_blocking_others() {
    let mut server = TestServer::start(WORLD_NAME);
    let mut first = TestClient::connect(server.addr, PLAYER_ID);
    let first_id = first.id;
    authenticate(&mut first, "same");

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

#[test]
fn invalid_usernames_are_refused_and_valid_ones_are_trimmed() {
    let mut server = TestServer::start(WORLD_NAME);
    let refused = [
        String::new(),
        "   \t ".to_string(),
        "x".repeat(33),
        "bad\nname".to_string(),
        "nul\0".to_string(),
    ];
    for (n, name) in refused.iter().enumerate() {
        let mut client = TestClient::connect(server.addr, PLAYER_ID + 10 + n as u64);
        client.wait_connected();
        client.send(auth_request(name));
        let start = Instant::now();
        while !client.client.is_disconnected() {
            assert!(
                start.elapsed() < WAIT_TIMEOUT,
                "the name {name:?} was not refused within {WAIT_TIMEOUT:?}"
            );
            client.pump();
        }
        assert!(
            !client
                .inbox
                .iter()
                .any(|m| matches!(m, ServerToClientMessage::AuthRegisterResponse(_))),
            "the name {name:?} was answered"
        );
    }

    // The longest name is accepted, and the stored name has no surrounding spaces
    let mut long = TestClient::connect(server.addr, PLAYER_ID);
    authenticate(&mut long, &"y".repeat(32));
    let mut padded = TestClient::connect(server.addr, PLAYER_ID + 1);
    padded.wait_connected();
    padded.send(auth_request("  Verif \t"));
    let auth = padded.wait_for("AuthRegisterResponse", |m| match m {
        ServerToClientMessage::AuthRegisterResponse(r) => Some(r.clone()),
        _ => None,
    });
    assert_eq!(auth.username, "Verif");

    long.send(ClientToServerMessage::Exit);
    long.send_packets();
    server.join_after_exit();
}

#[test]
fn a_message_with_too_many_inputs_is_only_simulated_up_to_the_cap() {
    let mut server = TestServer::start(WORLD_NAME);
    let mut client = TestClient::connect(server.addr, PLAYER_ID);
    let id = client.id;
    authenticate(&mut client, "flooder");

    let cap = shared::MAX_INPUTS_PER_TICK as u64;
    let frames: Vec<_> = (1..=cap + 50)
        .map(|time_ms| PlayerFrameInput {
            time_ms,
            delta_ms: 16,
            ..Default::default()
        })
        .collect();
    client.send(ClientToServerMessage::PlayerInputs(frames));
    client.wait_for("PlayerUpdate acknowledging the cap-th input", |m| match m {
        ServerToClientMessage::PlayerUpdate(u) if u.id == id && u.last_ack_time == cap => Some(()),
        _ => None,
    });
    // A later input is simulated, so the excess was dropped, not delayed
    client.send(inputs_at(100_000));
    client.wait_for("PlayerUpdate acknowledging the later input", |m| match m {
        ServerToClientMessage::PlayerUpdate(u) if u.id == id && u.last_ack_time == 100_000 => {
            Some(())
        }
        _ => None,
    });
    assert!(
        !client.inbox.iter().any(|m| matches!(
            m,
            ServerToClientMessage::PlayerUpdate(u)
                if u.id == id && (cap + 1..=cap + 50).contains(&u.last_ack_time)
        )),
        "an input beyond the cap was simulated"
    );

    client.send(ClientToServerMessage::Exit);
    client.send_packets();
    server.join_after_exit();
}

#[test]
fn a_second_authentication_request_changes_nothing() {
    let mut server = TestServer::start(WORLD_NAME);
    let mut client = TestClient::connect(server.addr, PLAYER_ID);
    let id = client.id;
    authenticate(&mut client, "orig");

    client.send(auth_request("renamed"));
    client.send(inputs_at(5));
    client.wait_for("PlayerUpdate acknowledging the input", |m| match m {
        ServerToClientMessage::PlayerUpdate(u) if u.id == id && u.last_ack_time == 5 => Some(()),
        _ => None,
    });

    // No second answer, and a newcomer sees the original name only
    let mut newcomer = TestClient::connect(server.addr, PLAYER_ID + 1);
    newcomer.wait_connected();
    newcomer.send(auth_request("newcomer"));
    let auth = newcomer.wait_for("AuthRegisterResponse", |m| match m {
        ServerToClientMessage::AuthRegisterResponse(r) => Some(r.clone()),
        _ => None,
    });
    let names: Vec<_> = auth.players.iter().map(|p| p.name.as_str()).collect();
    assert!(names.contains(&"orig"), "{names:?}");
    assert!(!names.contains(&"renamed"), "{names:?}");
    assert!(
        !client
            .inbox
            .iter()
            .any(|m| matches!(m, ServerToClientMessage::AuthRegisterResponse(_))),
        "the client was answered a second time"
    );
    // The name "renamed" is still free: the request did not register it
    let mut other = TestClient::connect(server.addr, PLAYER_ID + 2);
    authenticate(&mut other, "renamed");

    client.send(ClientToServerMessage::Exit);
    client.send_packets();
    server.join_after_exit();
}

/// Flies the player forward and up, then returns the position the server acknowledged.
fn fly_away(client: &mut TestClient, first_time_ms: u64) -> Vec3 {
    let frame = |time_ms, actions: &[NetworkAction]| PlayerFrameInput {
        time_ms,
        delta_ms: 100,
        inputs: actions.iter().cloned().collect(),
        ..Default::default()
    };
    let last_time_ms = first_time_ms + 5;
    let mut frames = vec![frame(first_time_ms, &[NetworkAction::ToggleFlyMode])];
    for time_ms in first_time_ms + 1..=last_time_ms {
        frames.push(frame(
            time_ms,
            &[NetworkAction::MoveForward, NetworkAction::JumpOrFlyUp],
        ));
    }
    client.send(ClientToServerMessage::PlayerInputs(frames));

    let id = client.id;
    client.wait_for(
        "PlayerUpdate acknowledging the last flight input",
        |m| match m {
            ServerToClientMessage::PlayerUpdate(u)
                if u.id == id && u.last_ack_time == last_time_ms =>
            {
                Some(u.movement.position)
            }
            _ => None,
        },
    )
}

fn authenticate(client: &mut TestClient, username: &str) {
    client.wait_connected();
    client.send(auth_request(username));
    client.wait_for("AuthRegisterResponse", |m| match m {
        ServerToClientMessage::AuthRegisterResponse(_) => Some(()),
        _ => None,
    });
}

#[test]
fn player_is_saved_when_it_disconnects_without_exit() {
    let mut server = TestServer::start(WORLD_NAME);
    let mut leaver = TestClient::connect(server.addr, PLAYER_ID);
    authenticate(&mut leaver, "leaver");
    let position = fly_away(&mut leaver, 1);

    // The connection just goes away: no `Exit` message.
    leaver.transport.disconnect();

    let player_file = server
        .world_dir()
        .join("players")
        .join(format!("{}.ron", PLAYER_ID));
    let start = Instant::now();
    while !player_file.is_file() {
        assert!(
            start.elapsed() < WAIT_TIMEOUT,
            "player file not written within {WAIT_TIMEOUT:?} of the disconnection"
        );
        thread::sleep(PUMP_INTERVAL);
    }
    let saved: Player = ron::de::from_str(&std::fs::read_to_string(&player_file).unwrap()).unwrap();
    assert_eq!(saved.position, position);

    // Another client stops the solo server.
    let mut closer = TestClient::connect(server.addr, PLAYER_ID + 1);
    authenticate(&mut closer, "closer");
    closer.send(ClientToServerMessage::Exit);
    closer.send_packets();
    server.join_after_exit();
}

#[test]
fn reconnecting_player_gets_back_its_saved_state() {
    let mut first = TestServer::start(WORLD_NAME);
    let mut client = TestClient::connect(first.addr, PLAYER_ID);
    authenticate(&mut client, "returning");
    let position = fly_away(&mut client, 1);
    assert_ne!(position, Vec3::new(0., 80., 0.), "the player did not move");

    client.send(ClientToServerMessage::Exit);
    client.send_packets();
    first.join_after_exit();

    let second = TestServer::start_in(first.tmp, WORLD_NAME);
    let mut client = TestClient::connect(second.addr, PLAYER_ID);
    client.wait_connected();
    client.send(auth_request("returning"));
    let id = client.id;
    let spawn = client.wait_for("PlayerSpawn for our id", |m| match m {
        ServerToClientMessage::PlayerSpawn(s) if s.id == id => Some(s.clone()),
        _ => None,
    });
    assert_eq!(spawn.data.position, position);
    assert!(spawn.data.is_flying);

    let mut second = second;
    client.send(ClientToServerMessage::Exit);
    client.send_packets();
    second.join_after_exit();
}

#[test]
fn corrupt_world_file_stops_the_server_without_touching_it() {
    let tmp = tempfile::tempdir().unwrap();
    let world_dir = tmp.path().join("saves").join(WORLD_NAME);
    std::fs::create_dir_all(&world_dir).unwrap();
    let world_file = world_dir.join("world.ron");
    std::fs::write(&world_file, "not a world").unwrap();

    let paths = GameFolderPaths {
        game_folder_path: tmp.path().to_path_buf(),
        assets_folder_path: tmp.path().join("assets"),
    };
    let socket =
        server::acquire_local_ephemeral_udp_socket(IpAddr::V4(Ipv4Addr::LOCALHOST)).unwrap();
    let config = GameServerConfig {
        world_name: WORLD_NAME.to_string(),
        is_solo: true,
        saves: None,
    };

    let err = server::init(socket, config, paths).unwrap_err();

    // only the cause: the caller names the world
    assert!(
        err.starts_with(&format!("{}: ", world_file.display())),
        "error does not start with the file: {err}"
    );
    assert_eq!(std::fs::read_to_string(world_file).unwrap(), "not a world");
}

#[test]
fn new_player_falls_onto_the_terrain_instead_of_through_it() {
    let mut server = TestServer::start(WORLD_NAME);
    let mut client = TestClient::connect(server.addr, PLAYER_ID);
    authenticate(&mut client, "faller");

    // 40 frames of 100 ms in one batch: far more than the fall from the y = 80 spawn
    let last_time_ms = 40;
    let frames = (1..=last_time_ms)
        .map(|time_ms| PlayerFrameInput {
            time_ms,
            delta_ms: 100,
            ..Default::default()
        })
        .collect();
    client.send(ClientToServerMessage::PlayerInputs(frames));
    let id = client.id;
    let position = client.wait_for("PlayerUpdate acknowledging the last frame", |m| match m {
        ServerToClientMessage::PlayerUpdate(u) if u.id == id && u.last_ack_time == last_time_ms => {
            Some(u.movement.position)
        }
        _ => None,
    });

    // The terrain surface is around y = 62-70; falling through it ended around y = 28
    assert!(
        (55.0..80.0).contains(&position.y),
        "player ended at y = {}",
        position.y
    );

    client.send(ClientToServerMessage::Exit);
    client.send_packets();
    server.join_after_exit();
}

fn chat_message(content: &str) -> ClientToServerMessage {
    ClientToServerMessage::ChatMessage(ChatMessageRequest {
        content: content.to_string(),
    })
}

fn received_chat(client: &mut TestClient, content: &str) -> FullChatMessage {
    let content = content.to_string();
    client.wait_for("ChatMessage", |m| match m {
        ServerToClientMessage::ChatMessage(c) if c.content == content => Some(c.clone()),
        _ => None,
    })
}

#[test]
fn chat_message_reaches_every_client_and_empty_ones_are_dropped() {
    let mut server = TestServer::start(WORLD_NAME);
    let mut alice = TestClient::connect(server.addr, PLAYER_ID);
    authenticate(&mut alice, "alice");
    let mut bob = TestClient::connect(server.addr, PLAYER_ID + 1);
    authenticate(&mut bob, "bob");

    // Rejected messages come first: the next accepted one proves they were not relayed.
    alice.send(chat_message(""));
    alice.send(chat_message("   "));
    alice.send(chat_message(&"x".repeat(257)));
    alice.send(chat_message("hello bob"));

    for client in [&mut alice, &mut bob] {
        let message = received_chat(client, "hello bob");
        assert_eq!(message.author, "alice");
        assert!(message.timestamp > 0);
    }
    for client in [&alice, &bob] {
        let relayed = client
            .inbox
            .iter()
            .filter(|m| matches!(m, ServerToClientMessage::ChatMessage(_)))
            .count();
        assert_eq!(relayed, 0, "a rejected message was relayed");
    }

    // A client authenticating later receives the stored history once.
    let mut carol = TestClient::connect(server.addr, PLAYER_ID + 2);
    authenticate(&mut carol, "carol");
    let replayed = received_chat(&mut carol, "hello bob");
    assert_eq!(replayed.author, "alice");
    carol.send(chat_message("hi all"));
    received_chat(&mut carol, "hi all");
    assert!(
        !carol.inbox.iter().any(
            |m| matches!(m, ServerToClientMessage::ChatMessage(c) if c.content == "hello bob")
        ),
        "the history was sent twice"
    );

    alice.send(ClientToServerMessage::Exit);
    alice.send_packets();
    server.join_after_exit();
}

/// A player standing on the terrain at the spawn column, looking straight down. Returns the
/// frames' camera, the player's position and the block the camera points at, once the client
/// has received the chunk that holds it.
fn land_and_aim_down(client: &mut TestClient) -> (Transform, IVec3) {
    let last_time_ms = 40;
    let frames = (1..=last_time_ms)
        .map(|time_ms| PlayerFrameInput {
            time_ms,
            delta_ms: 100,
            ..Default::default()
        })
        .collect();
    client.send(ClientToServerMessage::PlayerInputs(frames));
    let id = client.id;
    let position = client.wait_for("PlayerUpdate acknowledging the landing", |m| match m {
        ServerToClientMessage::PlayerUpdate(u) if u.id == id && u.last_ack_time == last_time_ms => {
            Some(u.movement.position)
        }
        _ => None,
    });

    let camera = Transform::from_translation(position).looking_to(Vec3::NEG_Y, Vec3::Z);
    let start = Instant::now();
    loop {
        if let Some(hit) = raycast::raycast(&client.world, &camera, &position, Default::default()) {
            return (camera, hit.position);
        }
        assert!(
            start.elapsed() < WAIT_TIMEOUT,
            "the block under the player never arrived"
        );
        client.pump();
    }
}

fn left_click_frame(time_ms: u64, camera: Transform) -> ClientToServerMessage {
    ClientToServerMessage::PlayerInputs(vec![PlayerFrameInput {
        time_ms,
        delta_ms: HIT_MS,
        inputs: [NetworkAction::LeftClick].into(),
        camera,
        ..Default::default()
    }])
}

#[test]
fn a_broken_block_reaches_every_client_as_updates_and_never_as_a_whole_chunk() {
    let mut server = TestServer::start(WORLD_NAME);
    let mut a = TestClient::connect(server.addr, PLAYER_ID);
    authenticate(&mut a, "miner");
    let mut b = TestClient::connect(server.addr, PLAYER_ID + 1);
    authenticate(&mut b, "watcher");

    let (camera, target) = land_and_aim_down(&mut a);
    let target_chunk = shared::world::global_block_to_chunk_pos(&target);
    let start = Instant::now();
    while !b.world.is_chunk_loaded(target_chunk) {
        assert!(
            start.elapsed() < WAIT_TIMEOUT,
            "the watcher never received the chunk of the target"
        );
        b.pump();
    }
    let block = *a.world.get_block(target).unwrap();
    let break_time = block.id.get_break_time().expect("the target is breakable");

    // One hit per message: each one holds the button for `HIT_MS`.
    a.inbox.clear();
    b.inbox.clear();
    let mut sequence = Vec::new();
    let breaking_started = Instant::now();
    for time_ms in 1000.. {
        assert!(
            breaking_started.elapsed() < WAIT_TIMEOUT,
            "the block was not broken within {WAIT_TIMEOUT:?}: {} updates, camera {camera:?}, target {target}",
            sequence.len()
        );
        a.send(left_click_frame(time_ms, camera));
        let update = a.wait_for("a BlockUpdates for the target", |m| match m {
            ServerToClientMessage::BlockUpdates(batch) => batch
                .iter()
                .find(|u| u.position == target)
                .map(|u| (u.block, game_message_to_payload(m.clone()).len())),
            _ => None,
        });
        sequence.push(update);
        if update.0.is_none() {
            break;
        }
    }

    let progress: Vec<u8> = sequence
        .iter()
        .filter_map(|(block, _)| block.map(|b| b.breaking_progress))
        .collect();
    assert_eq!(progress, (1..break_time).collect::<Vec<u8>>());
    assert_eq!(sequence.last().unwrap().0, None);
    assert!(a.world.get_block(target).is_none());

    // The watcher sees exactly the same states, in the same order.
    let start = Instant::now();
    let mut seen_by_b = b.take_updates_for(target);
    while seen_by_b.last() != Some(&None) {
        assert!(
            start.elapsed() < WAIT_TIMEOUT,
            "the watcher did not see the block break"
        );
        b.pump();
        seen_by_b.extend(b.take_updates_for(target));
    }
    let seen_by_a: Vec<Option<BlockData>> = sequence.iter().map(|(block, _)| *block).collect();
    assert_eq!(seen_by_b, seen_by_a);
    assert!(b.world.get_block(target).is_none());

    // No chunk was ever sent twice to either client, and none was re-sent after the change.
    for client in [&a, &b] {
        let mut arrivals = client.chunk_arrivals.clone();
        arrivals.sort_by_key(|p| (p.x, p.y, p.z));
        let total = arrivals.len();
        arrivals.dedup();
        assert_eq!(
            arrivals.len(),
            total,
            "client {} got a chunk twice",
            client.id
        );
    }

    // Size of a progress update against the whole chunk the old protocol re-sent each tick.
    let whole_chunk = ServerToClientMessage::WorldUpdate(shared::messages::WorldUpdate {
        new_map: [(target_chunk, a.world.chunk(target_chunk).unwrap().clone())].into(),
        ..Default::default()
    });
    let chunk_bytes = game_message_to_payload(whole_chunk).len();
    let largest_update = sequence.iter().map(|(_, bytes)| *bytes).max().unwrap();
    assert!(
        largest_update < chunk_bytes,
        "update of {largest_update} bytes against a chunk of {chunk_bytes} bytes"
    );

    a.send(ClientToServerMessage::Exit);
    a.send_packets();
    server.join_after_exit();
}

#[test]
fn a_block_broken_in_one_session_is_still_broken_after_a_restart() {
    let mut first = TestServer::start(WORLD_NAME);
    let mut client = TestClient::connect(first.addr, PLAYER_ID);
    authenticate(&mut client, "miner");
    let (camera, target) = land_and_aim_down(&mut client);
    let target_chunk = shared::world::global_block_to_chunk_pos(&target);

    let breaking_started = Instant::now();
    for time_ms in 1000.. {
        assert!(
            breaking_started.elapsed() < WAIT_TIMEOUT,
            "the block was not broken within {WAIT_TIMEOUT:?}: camera {camera:?}, target {target}"
        );
        client.send(left_click_frame(time_ms, camera));
        let broken = client.wait_for("a BlockUpdates for the target", |m| match m {
            ServerToClientMessage::BlockUpdates(batch) => batch
                .iter()
                .find(|u| u.position == target)
                .map(|u| u.block.is_none()),
            _ => None,
        });
        if broken {
            break;
        }
    }

    client.send(ClientToServerMessage::SaveWorldRequest);
    client.send_packets();
    let start = Instant::now();
    while first.chunk_files().is_empty() {
        assert!(start.elapsed() < WAIT_TIMEOUT, "no chunk file was written");
        client.pump();
    }
    assert!(first.world_dir().join("world.ron").is_file());

    // The server stops: once its thread has ended, everything is on disk, nothing to wait for.
    client.send(ClientToServerMessage::Exit);
    client.send_packets();
    first.join_after_exit();
    let chunk_file = format!(
        "{}_{}_{}.bin",
        target_chunk.x, target_chunk.y, target_chunk.z
    );
    assert!(first.chunk_files().contains(&chunk_file));
    assert!(!first
        .chunk_files()
        .iter()
        .any(|name| name.ends_with(".tmp")));

    let mut second = TestServer::start_in(first.tmp, WORLD_NAME);
    let mut client = TestClient::connect(second.addr, PLAYER_ID);
    authenticate(&mut client, "miner");
    let start = Instant::now();
    while !client.world.is_chunk_loaded(target_chunk) {
        assert!(
            start.elapsed() < WAIT_TIMEOUT,
            "the chunk of the target never arrived"
        );
        client.pump();
    }
    assert!(
        client.world.get_block(target).is_none(),
        "the block is back after the restart"
    );

    client.send(ClientToServerMessage::Exit);
    client.send_packets();
    second.join_after_exit();
}

/// Hits the block at `target` with `camera` until it breaks, one hit per message.
fn break_aimed_block(client: &mut TestClient, camera: Transform, target: IVec3) {
    let breaking_started = Instant::now();
    for time_ms in 1000.. {
        assert!(
            breaking_started.elapsed() < WAIT_TIMEOUT,
            "the block was not broken within {WAIT_TIMEOUT:?}: camera {camera:?}, target {target}"
        );
        client.send(left_click_frame(time_ms, camera));
        let broken = client.wait_for("a BlockUpdates for the target", |m| match m {
            ServerToClientMessage::BlockUpdates(batch) => batch
                .iter()
                .find(|u| u.position == target)
                .map(|u| u.block.is_none()),
            _ => None,
        });
        if broken {
            return;
        }
    }
}

/// Pumps until `client` knows of water at `position`, and returns its level.
fn wait_for_water(client: &mut TestClient, position: IVec3) -> u8 {
    let start = Instant::now();
    loop {
        if let Some(block) = client
            .world
            .get_block(position)
            .filter(|block| block.id == BlockId::Water)
        {
            return block.level;
        }
        assert!(
            start.elapsed() < WAIT_TIMEOUT,
            "client {} never saw water at {position}",
            client.id
        );
        client.pump();
    }
}

#[test]
fn water_flows_into_a_broken_block_next_to_it_for_every_client() {
    // A first session finds the block under the player and saves its chunk
    let mut first = TestServer::start(WORLD_NAME);
    let mut client = TestClient::connect(first.addr, PLAYER_ID);
    authenticate(&mut client, "miner");
    let (_, target) = land_and_aim_down(&mut client);
    client.send(ClientToServerMessage::SaveWorldRequest);
    client.send_packets();
    let start = Instant::now();
    while first.chunk_files().is_empty() {
        assert!(start.elapsed() < WAIT_TIMEOUT, "no chunk file was written");
        client.pump();
    }
    client.send(ClientToServerMessage::Exit);
    client.send_packets();
    first.join_after_exit();

    // A source next to that block, in the saved chunk
    let source = target + IVec3::X;
    let source_chunk = global_block_to_chunk_pos(&source);
    let chunk_file = first.world_dir().join("chunks").join(format!(
        "{}_{}_{}.bin",
        source_chunk.x, source_chunk.y, source_chunk.z
    ));
    let mut chunk: Chunk = payload_to_game_message(&std::fs::read(&chunk_file).unwrap()).unwrap();
    chunk.set(
        to_local_pos(&source),
        BlockData::new(BlockId::Water, BlockDirection::Front),
    );
    std::fs::write(&chunk_file, game_message_to_payload(&chunk)).unwrap();

    let mut second = TestServer::start_in(first.tmp, WORLD_NAME);
    let mut miner = TestClient::connect(second.addr, PLAYER_ID);
    authenticate(&mut miner, "miner");
    let mut watcher = TestClient::connect(second.addr, PLAYER_ID + 1);
    authenticate(&mut watcher, "watcher");
    let (camera, aimed) = land_and_aim_down(&mut miner);
    assert_eq!(aimed, target);

    break_aimed_block(&mut miner, camera, target);

    assert_eq!(wait_for_water(&mut miner, target), 1);
    assert_eq!(wait_for_water(&mut watcher, target), 1);

    miner.send(ClientToServerMessage::Exit);
    miner.send_packets();
    second.join_after_exit();
}

/// Authenticates and returns where the server spawned this client's player.
fn join(client: &mut TestClient, username: &str) -> Vec3 {
    client.wait_connected();
    client.send(auth_request(username));
    let id = client.id;
    client.wait_for("AuthRegisterResponse", |m| match m {
        ServerToClientMessage::AuthRegisterResponse(response) => response
            .players
            .iter()
            .find(|player| player.id == id)
            .map(|player| player.data.position),
        _ => None,
    })
}

/// Sends an input and waits for the server to acknowledge it: every message the server sent
/// before on the standard channel has then arrived.
fn sync_with_server(client: &mut TestClient, time_ms: u64) {
    let id = client.id;
    client.send(inputs_at(time_ms));
    client.wait_for("PlayerUpdate acknowledging the input", |m| match m {
        ServerToClientMessage::PlayerUpdate(u) if u.id == id && u.last_ack_time == time_ms => {
            Some(())
        }
        _ => None,
    });
}

fn spawns_received(client: &TestClient) -> Vec<u64> {
    client
        .inbox
        .iter()
        .filter_map(|message| match message {
            ServerToClientMessage::PlayerSpawn(spawn) => Some(spawn.id),
            _ => None,
        })
        .collect()
}

#[test]
fn a_joining_player_is_announced_once_with_its_own_position() {
    let server = TestServer::start_dedicated(WORLD_NAME);
    let mut first = TestClient::connect(server.addr, PLAYER_ID);
    join(&mut first, "first");
    // Away from the spawn point, where the second player will appear
    fly_away(&mut first, 1);
    sync_with_server(&mut first, 100);
    first
        .inbox
        .retain(|message| !matches!(message, ServerToClientMessage::PlayerSpawn(_)));

    let mut second = TestClient::connect(server.addr, PLAYER_ID + 1);
    let second_id = second.id;
    let second_spawn = join(&mut second, "second");

    let announced = first.wait_for("PlayerSpawn of the second player", |m| match m {
        ServerToClientMessage::PlayerSpawn(spawn) if spawn.id == second_id => Some(spawn.clone()),
        _ => None,
    });
    assert_eq!(announced.name, "second");
    assert_eq!(announced.data.position, second_spawn);

    // Nobody else is announced again: the first player keeps its own position everywhere
    sync_with_server(&mut first, 101);
    assert_eq!(spawns_received(&first), Vec::<u64>::new());
    sync_with_server(&mut second, 1);
    assert_eq!(spawns_received(&second), vec![second_id]);
}

#[test]
fn a_dedicated_server_outlives_the_exit_of_a_player_and_saves_it() {
    let server = TestServer::start_dedicated(WORLD_NAME);
    let mut leaving = TestClient::connect(server.addr, PLAYER_ID);
    let mut staying = TestClient::connect(server.addr, PLAYER_ID + 1);
    join(&mut leaving, "leaving");
    join(&mut staying, "staying");

    // A save request is the privilege of the solo player: a dedicated server ignores it
    staying.send(ClientToServerMessage::SaveWorldRequest);
    leaving.send(ClientToServerMessage::Exit);
    let start = Instant::now();
    while !leaving.client.is_disconnected() {
        assert!(
            start.elapsed() < WAIT_TIMEOUT,
            "the leaving client was not disconnected within {WAIT_TIMEOUT:?}"
        );
        leaving.pump();
    }

    sync_with_server(&mut staying, 1);
    let player_file = server
        .world_dir()
        .join("players")
        .join(format!("{PLAYER_ID}.ron"));
    let start = Instant::now();
    while !player_file.exists() {
        assert!(
            start.elapsed() < WAIT_TIMEOUT,
            "{} was not written within {WAIT_TIMEOUT:?}",
            player_file.display()
        );
        staying.pump();
    }
}

/// Starts a solo server whose save already holds player `PLAYER_ID` with `inventory`.
fn start_with_inventory(inventory: Inventory) -> TestServer {
    let tmp = tempfile::tempdir().unwrap();
    let players_dir = tmp.path().join("saves").join(WORLD_NAME).join("players");
    std::fs::create_dir_all(&players_dir).unwrap();
    let mut player = Player::new(
        PLAYER_ID,
        "crafter".into(),
        Vec3::new(0., 80., 0.),
        Transform::default(),
    );
    player.inventory = inventory;
    std::fs::write(
        players_dir.join(format!("{PLAYER_ID}.ron")),
        ron::ser::to_string(&player).unwrap(),
    )
    .unwrap();
    TestServer::start_in(tmp, WORLD_NAME)
}

/// Sends one input holding `actions` and returns the inventory the server acknowledges it with.
fn send_inventory_actions(
    client: &mut TestClient,
    time_ms: u64,
    actions: Vec<InventoryAction>,
) -> Inventory {
    client.send(ClientToServerMessage::PlayerInputs(vec![
        PlayerFrameInput {
            time_ms,
            delta_ms: 16,
            inventory_actions: actions,
            ..Default::default()
        },
    ]));
    let id = client.id;
    client.wait_for("PlayerUpdate acknowledging the actions", |m| match m {
        ServerToClientMessage::PlayerUpdate(u) if u.id == id && u.last_ack_time == time_ms => {
            Some(u.inventory.clone())
        }
        _ => None,
    })
}

fn stack(item_id: ItemId, nb: u32) -> ItemStack {
    ItemStack {
        item_id,
        item_type: item_id.get_default_type(),
        nb,
    }
}

#[test]
fn inventory_clicks_are_applied_by_the_server() {
    let mut inventory = Inventory::new();
    inventory.inner.insert(0, stack(ItemId::Dirt, 5));
    let server = start_with_inventory(inventory);
    let mut client = TestClient::connect(server.addr, PLAYER_ID);
    join(&mut client, "crafter");

    let inventory = send_inventory_actions(
        &mut client,
        1,
        vec![
            InventoryAction::LeftClickSlot(0),
            InventoryAction::LeftClickSlot(7),
            InventoryAction::LeftClickSlot(MAX_INVENTORY_SLOTS + 50),
        ],
    );

    let mut expected = Inventory::new();
    expected.inner.insert(7, stack(ItemId::Dirt, 5));
    assert_eq!(inventory, expected);
}

#[test]
fn quick_moves_and_hotbar_swaps_are_applied_by_the_server() {
    let mut inventory = Inventory::new();
    inventory.inner.insert(20, stack(ItemId::Dirt, 5));
    inventory.inner.insert(2, stack(ItemId::Stick, 3));
    let server = start_with_inventory(inventory);
    let mut client = TestClient::connect(server.addr, PLAYER_ID);
    join(&mut client, "crafter");

    let inventory = send_inventory_actions(
        &mut client,
        1,
        vec![
            // The dirt goes to the first free slot of the hotbar
            InventoryAction::QuickMoveSlot(20),
            // The stick trades places with the empty slot 9
            InventoryAction::SwapWithHotbar { slot: 9, hotbar: 2 },
        ],
    );

    let mut expected = Inventory::new();
    expected.inner.insert(0, stack(ItemId::Dirt, 5));
    expected.inner.insert(9, stack(ItemId::Stick, 3));
    assert_eq!(inventory, expected);
}

#[test]
fn a_log_is_crafted_into_planks() {
    let mut inventory = Inventory::new();
    inventory.inner.insert(0, stack(ItemId::OakLog, 1));
    let server = start_with_inventory(inventory);
    let mut client = TestClient::connect(server.addr, PLAYER_ID);
    join(&mut client, "crafter");

    let inventory = send_inventory_actions(
        &mut client,
        1,
        vec![
            InventoryAction::LeftClickSlot(0),
            InventoryAction::LeftClickSlot(grid_slot(0, 0)),
            InventoryAction::TakeCraftResult,
            InventoryAction::LeftClickSlot(1),
        ],
    );

    let mut expected = Inventory::new();
    expected.inner.insert(1, stack(ItemId::OakPlanks, 4));
    assert_eq!(inventory, expected);
}

#[test]
fn a_furnace_is_crafted_at_a_table() {
    let mut inventory = Inventory::new();
    inventory.inner.insert(0, stack(ItemId::CraftingTable, 1));
    inventory.inner.insert(1, stack(ItemId::Cobblestone, 8));
    let server = start_with_inventory(inventory);
    let mut client = TestClient::connect(server.addr, PLAYER_ID);
    join(&mut client, "crafter");

    // Place the table on the ground, two blocks ahead
    let (down, _) = land_and_aim_down(&mut client);
    let position = down.translation;
    let camera = Transform::from_translation(position)
        .looking_at(position + Vec3::new(0., -1.6, 2.), Vec3::Y);
    let ground = raycast::raycast(&client.world, &camera, &position, Default::default())
        .expect("the player aims at the ground");
    let table = ground.position + ground.face.to_ivec3();
    let place = PlayerFrameInput {
        time_ms: 100,
        delta_ms: 16,
        inputs: [NetworkAction::RightClick].into(),
        camera,
        hotbar_slot: 0,
        ..Default::default()
    };

    let ring = [
        (0, 0),
        (0, 1),
        (0, 2),
        (1, 0),
        (1, 2),
        (2, 0),
        (2, 1),
        (2, 2),
    ];
    let mut actions = vec![
        InventoryAction::OpenCraftingTable(table),
        InventoryAction::LeftClickSlot(1),
    ];
    actions.extend(
        ring.iter()
            .map(|&(row, col)| InventoryAction::RightClickSlot(grid_slot(row, col))),
    );
    actions.extend([
        InventoryAction::TakeCraftResult,
        InventoryAction::LeftClickSlot(2),
        InventoryAction::Close,
    ]);
    let craft = PlayerFrameInput {
        time_ms: 101,
        delta_ms: 16,
        camera,
        inventory_actions: actions,
        ..Default::default()
    };
    client.send(ClientToServerMessage::PlayerInputs(vec![place, craft]));
    let id = client.id;
    let inventory = client.wait_for("PlayerUpdate acknowledging the craft", |m| match m {
        ServerToClientMessage::PlayerUpdate(u) if u.id == id && u.last_ack_time == 101 => {
            Some(u.inventory.clone())
        }
        _ => None,
    });

    let mut expected = Inventory::new();
    expected.inner.insert(2, stack(ItemId::Furnace, 1));
    assert_eq!(inventory, expected);
}

/// The answer of the status port at `addr` to a request carrying `nonce`, asked from a fresh
/// socket, or nothing within 500 ms.
fn ask_status(addr: std::net::SocketAddr, nonce: u64) -> Option<StatusReply> {
    let socket = UdpSocket::bind("127.0.0.1:0").unwrap();
    socket.connect(addr).unwrap();
    socket
        .set_read_timeout(Some(Duration::from_millis(500)))
        .unwrap();
    socket.send(&StatusRequest { nonce }.encode()).unwrap();
    let mut buffer = [0; 64];
    let len = socket.recv(&mut buffer).ok()?;
    StatusReply::decode(&buffer[..len])
}

/// Asks the status port at `addr` until a reply satisfies `done`, and returns that reply.
/// Fails the test after `WAIT_TIMEOUT`.
fn wait_for_status(
    addr: std::net::SocketAddr,
    nonce: u64,
    what: &str,
    done: impl Fn(&StatusReply) -> bool,
) -> StatusReply {
    let start = Instant::now();
    loop {
        assert!(
            start.elapsed() < WAIT_TIMEOUT,
            "no status reply {what} within {WAIT_TIMEOUT:?}"
        );
        if let Some(reply) = ask_status(addr, nonce).filter(&done) {
            return reply;
        }
    }
}

#[test]
fn the_status_port_counts_the_players_and_ignores_garbage() {
    let server = TestServer::start_dedicated_with_status(WORLD_NAME);
    let status_addr = server.status_addr.unwrap();

    let request = StatusRequest { nonce: 1 }.encode();
    let mut longer = request.to_vec();
    longer.push(0);
    let reply = StatusReply {
        nonce: 1,
        protocol: PROTOCOL_ID,
        players: 0,
        max_players: 64,
    }
    .encode();
    let garbage = UdpSocket::bind("127.0.0.1:0").unwrap();
    garbage.connect(status_addr).unwrap();
    for packet in [
        &b""[..],
        b"hello",
        &[0; 32],
        &request[..31],
        &longer,
        &reply,
    ] {
        garbage.send(packet).unwrap();
    }

    // The garbage queued before this request was read first: its answers would be waiting
    let first = wait_for_status(status_addr, 42, "from the started server", |_| true);
    assert_eq!(first.nonce, 42);
    assert_eq!(first.protocol, PROTOCOL_ID);
    assert_eq!(first.players, 0);
    assert_eq!(first.max_players, 64);
    garbage
        .set_read_timeout(Some(Duration::from_millis(300)))
        .unwrap();
    let mut buffer = [0; 64];
    assert!(
        garbage.recv(&mut buffer).is_err(),
        "a malformed status request was answered"
    );

    let mut client = TestClient::connect(server.addr, PLAYER_ID);
    authenticate(&mut client, "player");
    wait_for_status(status_addr, 43, "counting the player", |reply| {
        reply.players == 1
    });
}
