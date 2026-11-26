//! End-to-end test of a solo session: a real server thread driven over loopback UDP by a
//! renet client set up the same way as `client/src/network/setup.rs`.

use bevy::math::{IVec3, Vec3};
use bevy::prelude::Transform;
use bevy_renet::netcode::{ClientAuthentication, NetcodeClientTransport};
use bevy_renet::RenetClient;
use ed25519_dalek::SigningKey;
use rustcraft_protocol::api::{AccountSummary, MemberRole, Profile, RealmConfig, RealmMember};
use rustcraft_protocol::identity::{
    player_id, sign, sign_join, Certificate, CERTIFICATE_LIFETIME_SECS, CERTIFICATE_VERSION,
};
use server::hub::{RealmEvent, RealmWatch};
use server::{read_chunk, region_file_name, region_of, write_chunk_in_region};
use shared::messages::{
    AuthRegisterRequest, ChatMessageRequest, ClientToServerMessage, FullChatMessage, JoinProof,
    NetworkAction, PlayerFrameInput, ServerToClientMessage,
};
use shared::players::armour::{ArmourPiece, WornArmour};
use shared::players::blocks::HIT_MS;
use shared::players::game_mode::GameMode;
use shared::players::inventory::InventoryAction;
use shared::players::progress::Achievement;
use shared::players::skin::{PlayerSkin, SKIN_BYTES};
use shared::players::{Inventory, Player};
use shared::status::{StatusReply, StatusRequest};
use shared::world::{
    chunk_distance, global_block_to_chunk_pos, grid_slot, lair, raycast, to_local_pos,
    world_position_to_chunk_position, BlockData, BlockDirection, BlockId, ChunkMap, Enchantment,
    FaceDirection, FaceDirectionExt, ItemId, ItemStack, ItemType, MobId, MobKind, Portal,
    PortalColor, ServerMob, TerrainNoises, WeatherKind,
};
use shared::{
    game_message_to_payload, get_shared_renet_config, payload_to_game_message,
    ChannelResolvableExt, GameFolderPaths, GameServerConfig, HubTrust, ServerAuth,
    DAY_DURATION_IN_TICKS, MAX_INVENTORY_SLOTS, MIDNIGHT_IN_DAY, NOON_IN_DAY, PROTOCOL_ID,
    STC_AUTH_CHANNEL, STC_CHUNK_DATA_CHANNEL, STC_STANDARD_CHANNEL,
};
use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, UdpSocket};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant, SystemTime};
use uuid::Uuid;

const WAIT_TIMEOUT: Duration = Duration::from_secs(10);
const PUMP_INTERVAL: Duration = Duration::from_millis(5);
const PLAYER_ID: u64 = 42;
const WORLD_NAME: &str = "session-test";
/// A seed whose terrain is bare and level around the spawn column: a snow plain, at y = 67.
const FLAT_SPAWN_SEED: u32 = 147;

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

    /// A dedicated server whose game folder holds `ops`, the RON list of its operators.
    fn start_dedicated_with_ops(world_name: &str, ops: &str) -> Self {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join(server::OPS_FILE), ops).unwrap();
        Self::start_with(tmp, world_name, false)
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
            shared::MAX_PLAYERS,
            ServerAuth::Offline,
            None,
        )
    }

    /// A dedicated server that holds at most `max_players` players.
    fn start_dedicated_with_max_players(world_name: &str, max_players: usize) -> Self {
        let socket =
            server::acquire_local_ephemeral_udp_socket(IpAddr::V4(Ipv4Addr::LOCALHOST)).unwrap();
        Self::start_on(
            tempfile::tempdir().unwrap(),
            world_name,
            false,
            socket,
            None,
            max_players,
            ServerAuth::Offline,
            None,
        )
    }

    fn start_with(tmp: tempfile::TempDir, world_name: &str, is_solo: bool) -> Self {
        let socket =
            server::acquire_local_ephemeral_udp_socket(IpAddr::V4(Ipv4Addr::LOCALHOST)).unwrap();
        Self::start_on(
            tmp,
            world_name,
            is_solo,
            socket,
            None,
            shared::MAX_PLAYERS,
            ServerAuth::Offline,
            None,
        )
    }

    /// A dedicated server whose players join with the accounts of the hub `trust` describes.
    fn start_hub(world_name: &str, trust: HubTrust) -> Self {
        Self::start_hub_in(tempfile::tempdir().unwrap(), world_name, trust)
    }

    /// Same, in the game folder `tmp`, which may hold its lists and the saves of a server.
    fn start_hub_in(tmp: tempfile::TempDir, world_name: &str, trust: HubTrust) -> Self {
        server::set_profile_lookup(hub_profile);
        let socket =
            server::acquire_local_ephemeral_udp_socket(IpAddr::V4(Ipv4Addr::LOCALHOST)).unwrap();
        Self::start_on(
            tmp,
            world_name,
            false,
            socket,
            None,
            shared::MAX_PLAYERS,
            ServerAuth::Hub(trust),
            None,
        )
    }

    /// A hosted world of the hub `trust` describes, with the members of `initial`: the events
    /// the hub would send are those of the sender.
    fn start_hosted(
        trust: HubTrust,
        initial: RealmConfig,
    ) -> (Self, std::sync::mpsc::Sender<RealmEvent>) {
        server::set_profile_lookup(hub_profile);
        let socket =
            server::acquire_local_ephemeral_udp_socket(IpAddr::V4(Ipv4Addr::LOCALHOST)).unwrap();
        let (sender, events) = std::sync::mpsc::channel();
        let server = Self::start_on(
            tempfile::tempdir().unwrap(),
            WORLD_NAME,
            false,
            socket,
            None,
            shared::MAX_PLAYERS,
            ServerAuth::Hub(trust),
            Some(RealmWatch { initial, events }),
        );
        (server, sender)
    }

    fn start_on(
        tmp: tempfile::TempDir,
        world_name: &str,
        is_solo: bool,
        socket: UdpSocket,
        status: Option<UdpSocket>,
        max_players: usize,
        auth: ServerAuth,
        realm: Option<RealmWatch>,
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
            max_render_distance: shared::DEFAULT_MAX_RENDER_DISTANCE,
            max_players,
            new_world: Default::default(),
            auth,
        };
        let handle = thread::spawn(move || match (realm, status) {
            (Some(realm), status) => {
                server::init_hosted(socket, status, config, paths, realm).unwrap()
            }
            (None, Some(status)) => {
                server::init_with_status(socket, status, config, paths).unwrap()
            }
            (None, None) => server::init(socket, config, paths).unwrap(),
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
            ServerToClientMessage::ChunkUnloads(chunks) => {
                for pos in chunks {
                    self.world.remove_chunk(*pos);
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
fn a_server_with_two_places_refuses_a_third_client() {
    let server = TestServer::start_dedicated_with_max_players(WORLD_NAME, 2);
    let mut clients: Vec<TestClient> = (1..=2)
        .map(|id| TestClient::connect(server.addr, id))
        .collect();
    for client in &mut clients {
        client.wait_connected();
    }

    let mut third = TestClient::connect(server.addr, 3);
    let start = Instant::now();
    while !third.client.is_disconnected() {
        assert!(
            start.elapsed() < WAIT_TIMEOUT,
            "the third client was neither accepted nor refused"
        );
        assert!(!third.client.is_connected(), "the third client got in");
        third.pump();
    }
    // the two first are still there
    for client in &mut clients {
        client.pump();
        assert!(client.client.is_connected());
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
            render_distance: 2,
            skin: Default::default(),
            proof: None,
        }
        .into(),
    );
    let auth = client.wait_for("AuthRegisterResponse", |m| match m {
        ServerToClientMessage::AuthRegisterResponse(r) => Some(r.clone()),
        _ => None,
    });
    assert_eq!(auth.username, "tester");
    assert_eq!(auth.session_token, client.id);
    assert_eq!(auth.seed, FLAT_SPAWN_SEED);

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

    // 4. Save: the files must exist before we ask the server to exit. The chunk files are
    // written aside, `.tmp`, then all put in place at once
    client.send(ClientToServerMessage::SaveWorldRequest);
    client.send_packets();
    let world_dir = server.world_dir();
    let player_file = world_dir.join("players").join(format!("{}.ron", client.id));
    let start = Instant::now();
    while !(world_dir.join("world.ron").is_file()
        && player_file.is_file()
        && server
            .chunk_files()
            .iter()
            .any(|name| name.ends_with(".region")))
    {
        assert!(
            start.elapsed() < WAIT_TIMEOUT,
            "world.ron, the chunk files and the player file were not saved within {WAIT_TIMEOUT:?}"
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
        render_distance: 2,
        skin: Default::default(),
        proof: None,
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
    set_game_mode(client, "creative");
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

/// Runs `/gamemode <mode>` and waits for its answer, so that the inputs sent next are
/// simulated in that mode.
fn set_game_mode(client: &mut TestClient, mode: &str) {
    client.send(chat_message(&format!("/gamemode {mode}")));
    let expected = format!("Game mode set to {mode}");
    client.wait_for("the answer to /gamemode", |m| match m {
        ServerToClientMessage::ChatMessage(message) if message.content == expected => Some(()),
        _ => None,
    });
}

/// Sends one input of 100 ms per action set of `frames`, from `first_time_ms` on, and returns
/// the update that acknowledges the last one.
fn play(
    client: &mut TestClient,
    first_time_ms: u64,
    frames: &[&[NetworkAction]],
) -> shared::messages::PlayerUpdateEvent {
    let inputs: Vec<PlayerFrameInput> = frames
        .iter()
        .zip(first_time_ms..)
        .map(|(actions, time_ms)| PlayerFrameInput {
            time_ms,
            delta_ms: 100,
            inputs: actions.iter().cloned().collect(),
            ..Default::default()
        })
        .collect();
    let last_time_ms = first_time_ms + frames.len() as u64 - 1;
    client.send(ClientToServerMessage::PlayerInputs(inputs));
    let id = client.id;
    client.wait_for("PlayerUpdate acknowledging the last input", |m| match m {
        ServerToClientMessage::PlayerUpdate(u) if u.id == id && u.last_ack_time == last_time_ms => {
            Some(u.clone())
        }
        _ => None,
    })
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
        max_render_distance: shared::DEFAULT_MAX_RENDER_DISTANCE,
        max_players: shared::MAX_PLAYERS,
        new_world: Default::default(),
        auth: Default::default(),
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
fn the_chunks_follow_the_render_distance_the_player_asks_for() {
    let server = TestServer::start(WORLD_NAME);
    let mut client = TestClient::connect(server.addr, PLAYER_ID);
    client.wait_connected();
    client.send(
        AuthRegisterRequest {
            username: "viewer".to_string(),
            render_distance: 1,
            skin: Default::default(),
            proof: None,
        }
        .into(),
    );
    let id = client.id;
    let spawn = client.wait_for("PlayerSpawn for our id", |m| match m {
        ServerToClientMessage::PlayerSpawn(s) if s.id == id => Some(s.data.position),
        _ => None,
    });
    let center = world_position_to_chunk_position(spawn);
    // Pumps until the client holds `count` chunks, and returns the farthest one's distance
    let hold = |client: &mut TestClient, count: usize| {
        let start = Instant::now();
        while client.world.chunk_positions().count() != count {
            assert!(
                start.elapsed() < WAIT_TIMEOUT,
                "the client holds {} chunks, not {count}",
                client.world.chunk_positions().count()
            );
            client.pump();
        }
        client
            .world
            .chunk_positions()
            .map(|chunk| chunk_distance(chunk, center))
            .max()
    };

    assert_eq!(hold(&mut client, 27), Some(1));
    // a few more ticks bring nothing farther
    let start = Instant::now();
    while start.elapsed() < Duration::from_millis(300) {
        client.pump();
    }
    assert_eq!(hold(&mut client, 27), Some(1));

    client.send(ClientToServerMessage::SetRenderDistance(3));
    assert_eq!(hold(&mut client, 7 * 7 * 7), Some(3));

    // down to 1: the chunks at 3 are unloaded, the margin keeps those at 2
    client.send(ClientToServerMessage::SetRenderDistance(1));
    let unloads = client.wait_for("ChunkUnloads", |m| match m {
        ServerToClientMessage::ChunkUnloads(chunks) => Some(chunks.len()),
        _ => None,
    });
    assert_eq!(unloads, 7 * 7 * 7 - 5 * 5 * 5);
    assert_eq!(hold(&mut client, 5 * 5 * 5), Some(2));
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

#[test]
fn a_joining_player_receives_the_skins_of_the_others_and_they_receive_its_own() {
    let mut server = TestServer::start(WORLD_NAME);
    let pixels: Vec<u8> = (0..SKIN_BYTES).map(|i| (i % 251) as u8).collect();
    let mut painter = TestClient::connect(server.addr, PLAYER_ID);
    painter.wait_connected();
    painter.send(
        AuthRegisterRequest {
            username: "painter".to_string(),
            render_distance: 2,
            skin: PlayerSkin::Custom(pixels.clone()),
            proof: None,
        }
        .into(),
    );
    painter.wait_for("AuthRegisterResponse", |m| match m {
        ServerToClientMessage::AuthRegisterResponse(_) => Some(()),
        _ => None,
    });
    // A skin of the wrong size is replaced by the default one
    let mut sloppy = TestClient::connect(server.addr, PLAYER_ID + 1);
    sloppy.wait_connected();
    sloppy.send(
        AuthRegisterRequest {
            username: "sloppy".to_string(),
            render_distance: 2,
            skin: PlayerSkin::Custom(vec![1; 64 * 32 * 4]),
            proof: None,
        }
        .into(),
    );
    sloppy.wait_for("AuthRegisterResponse", |m| match m {
        ServerToClientMessage::AuthRegisterResponse(_) => Some(()),
        _ => None,
    });

    let mut miner = TestClient::connect(server.addr, PLAYER_ID + 2);
    miner.wait_connected();
    miner.send(
        AuthRegisterRequest {
            username: "miner".to_string(),
            render_distance: 2,
            skin: PlayerSkin::BuiltIn("Miner".to_string()),
            proof: None,
        }
        .into(),
    );
    let auth = miner.wait_for("AuthRegisterResponse", |m| match m {
        ServerToClientMessage::AuthRegisterResponse(r) => Some(r.clone()),
        _ => None,
    });
    let skin_of = |name: &str| {
        auth.players
            .iter()
            .find(|p| p.name == name)
            .map(|p| p.skin.clone())
    };
    assert_eq!(skin_of("painter"), Some(PlayerSkin::Custom(pixels)));
    assert_eq!(skin_of("sloppy"), Some(PlayerSkin::default()));
    assert_eq!(
        skin_of("miner"),
        Some(PlayerSkin::BuiltIn("Miner".to_string()))
    );

    // The players already there are told of the newcomer with its skin
    let spawn = painter.wait_for("PlayerSpawn of the miner", |m| match m {
        ServerToClientMessage::PlayerSpawn(s) if s.name == "miner" => Some(s.clone()),
        _ => None,
    });
    assert_eq!(spawn.skin, PlayerSkin::BuiltIn("Miner".to_string()));

    painter.send(ClientToServerMessage::Exit);
    painter.send_packets();
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
        // Water next to the block may flow into it within the tick that breaks it: the update
        // then carries the water instead of air
        let broken = client.wait_for("a BlockUpdates for the target", |m| match m {
            ServerToClientMessage::BlockUpdates(batch) => batch
                .iter()
                .find(|u| u.position == target)
                .map(|u| u.block.is_none_or(|block| block.id == BlockId::Water)),
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
    let region_file = region_file_name(region_of(target_chunk));
    assert!(first.chunk_files().contains(&region_file));
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

#[test]
fn a_player_is_sent_its_own_inventory_and_not_the_others() {
    let mut server = TestServer::start(WORLD_NAME);
    let mut a = TestClient::connect(server.addr, PLAYER_ID);
    authenticate(&mut a, "first");
    let mut b = TestClient::connect(server.addr, PLAYER_ID + 1);
    authenticate(&mut b, "second");

    let inventory_of = |client: &mut TestClient, id| {
        client.wait_for("a PlayerUpdate", |m| match m {
            ServerToClientMessage::PlayerUpdate(u) if u.id == id => Some(u.inventory.clone()),
            _ => None,
        })
    };
    assert!(inventory_of(&mut a, PLAYER_ID).is_some());
    assert_eq!(inventory_of(&mut a, PLAYER_ID + 1), None);
    assert!(inventory_of(&mut b, PLAYER_ID + 1).is_some());
    assert_eq!(inventory_of(&mut b, PLAYER_ID), None);

    a.send(ClientToServerMessage::Exit);
    a.send_packets();
    server.join_after_exit();
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
        // Water next to the block may flow into it within the tick that breaks it: the update
        // then carries the water instead of air
        let broken = client.wait_for("a BlockUpdates for the target", |m| match m {
            ServerToClientMessage::BlockUpdates(batch) => batch
                .iter()
                .find(|u| u.position == target)
                .map(|u| u.block.is_none_or(|block| block.id == BlockId::Water)),
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
    let chunks_dir = first.world_dir().join("chunks");
    let mut chunk = read_chunk(&chunks_dir, source_chunk).unwrap();
    chunk.set(
        to_local_pos(&source),
        BlockData::new(BlockId::Water, BlockDirection::Front),
    );
    write_chunk_in_region(&chunks_dir, source_chunk, &chunk).unwrap();

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
    // the first player flies away with /gamemode, an operator's command
    let server = TestServer::start_dedicated_with_ops(WORLD_NAME, r#"["first"]"#);
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
            u.inventory.clone()
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
fn the_others_see_the_item_of_the_selected_hotbar_slot_in_a_players_hand() {
    let mut inventory = Inventory::new();
    inventory.inner.insert(0, stack(ItemId::Dirt, 5));
    inventory.inner.insert(2, stack(ItemId::Stick, 3));
    let server = start_with_inventory(inventory);
    let mut holder = TestClient::connect(server.addr, PLAYER_ID);
    join(&mut holder, "crafter");
    let mut watcher = TestClient::connect(server.addr, PLAYER_ID + 1);
    join(&mut watcher, "watcher");

    // The holder selects a slot, then another: the watcher sees each item in its hand, and an
    // empty slot leaves the hand empty
    for (time_ms, slot, held) in [
        (1, 2, Some(ItemId::Stick)),
        (2, 0, Some(ItemId::Dirt)),
        (3, 5, None),
    ] {
        holder.send(ClientToServerMessage::PlayerInputs(vec![
            PlayerFrameInput {
                time_ms,
                delta_ms: 16,
                hotbar_slot: slot,
                ..Default::default()
            },
        ]));
        holder.send_packets();
        watcher.wait_for("the holder's update with that slot", |m| match m {
            ServerToClientMessage::PlayerUpdate(u)
                if u.id == PLAYER_ID && u.last_ack_time == time_ms =>
            {
                assert_eq!(u.held.map(|held| held.item), held, "slot {slot}");
                Some(())
            }
            _ => None,
        });
    }
}

#[test]
fn a_piece_of_armour_is_put_on_saved_and_seen_by_the_others() {
    let helmet = ArmourPiece::Helmet.slot();
    let mut inventory = Inventory::new();
    inventory.inner.insert(0, stack(ItemId::IronHelmet, 1));
    inventory
        .inner
        .insert(ArmourPiece::Boots.slot(), stack(ItemId::LeatherBoots, 1));
    // The boots were worn when the player was saved
    let server = start_with_inventory(inventory);
    let mut wearer = TestClient::connect(server.addr, PLAYER_ID);
    join(&mut wearer, "crafter");
    let mut watcher = TestClient::connect(server.addr, PLAYER_ID + 1);
    join(&mut watcher, "watcher");

    // The helmet goes to its slot, refused in that of the leggings
    let inventory = send_inventory_actions(
        &mut wearer,
        1,
        vec![
            InventoryAction::SwapWithHotbar {
                slot: ArmourPiece::Leggings.slot(),
                hotbar: 0,
            },
            InventoryAction::QuickMoveSlot(0),
        ],
    );
    assert_eq!(inventory.inner[&helmet], stack(ItemId::IronHelmet, 1));
    assert_eq!(inventory.inner.len(), 2);
    let expected = WornArmour([
        Some(ItemId::IronHelmet),
        None,
        None,
        Some(ItemId::LeatherBoots),
    ]);
    watcher.wait_for("the wearer in its armour", |m| match m {
        ServerToClientMessage::PlayerUpdate(u) if u.id == PLAYER_ID && u.worn == expected => {
            Some(())
        }
        _ => None,
    });
}

#[test]
fn a_player_who_joins_later_sees_the_armour_and_the_held_item_of_those_there() {
    let mut inventory = Inventory::new();
    inventory.inner.insert(2, stack(ItemId::IronSword, 1));
    inventory
        .inner
        .insert(ArmourPiece::Helmet.slot(), stack(ItemId::IronHelmet, 1));
    let server = start_with_inventory(inventory);
    let mut first = TestClient::connect(server.addr, PLAYER_ID);
    join(&mut first, "crafter");
    let mut second = TestClient::connect(server.addr, PLAYER_ID + 1);
    join(&mut second, "watcher");

    // Both ways: the first in its armour, its sword in hand, and the second with neither
    for (sees, seen, slot, held, helmet) in [
        (0, 1, 2, Some(ItemId::IronSword), Some(ItemId::IronHelmet)),
        (1, 0, 0, None, None),
    ] {
        let clients = [&mut second, &mut first];
        let id = clients[seen].id;
        clients[seen].send(ClientToServerMessage::PlayerInputs(vec![
            PlayerFrameInput {
                time_ms: 1,
                delta_ms: 16,
                hotbar_slot: slot,
                ..Default::default()
            },
        ]));
        clients[seen].send_packets();
        clients[sees].wait_for("the other player as it is", |m| match m {
            ServerToClientMessage::PlayerUpdate(u)
                if u.id == id && u.last_ack_time == 1 && u.held.map(|held| held.item) == held =>
            {
                assert_eq!(u.worn, WornArmour([helmet, None, None, None]));
                Some(())
            }
            _ => None,
        });
    }
}

#[test]
fn a_player_killed_drops_its_armour_and_what_its_engraving_table_holds() {
    use shared::players::inventory::ENGRAVING_SLOT;
    let carried = [
        (0, ItemId::Dirt),
        (ArmourPiece::Helmet.slot(), ItemId::IronHelmet),
        (ArmourPiece::Boots.slot(), ItemId::LeatherBoots),
        (ENGRAVING_SLOT, ItemId::IronSword),
    ];
    let mut inventory = Inventory::new();
    for (slot, item) in carried {
        inventory.inner.insert(slot, stack(item, 1));
    }
    let server = start_with_inventory(inventory);
    let mut victim = TestClient::connect(server.addr, PLAYER_ID);
    join(&mut victim, "crafter");
    let mut time_ms = 1;
    let mut update = play(&mut victim, time_ms, &[&[]]);
    // It falls from 30 blocks above where it stands
    for mode in ["creative", "survival"] {
        set_game_mode(&mut victim, mode);
        let mut frames: Vec<&[NetworkAction]> = vec![&[]; 40];
        if mode == "creative" {
            frames = vec![&[NetworkAction::ToggleFlyMode]];
            frames.extend([&[NetworkAction::JumpOrFlyUp][..]; 15]);
        }
        time_ms += 100;
        update = play(&mut victim, time_ms, &frames);
    }
    assert_eq!(update.health, 0);

    let mut missing: Vec<ItemId> = carried.map(|(_, item)| item).to_vec();
    while !missing.is_empty() {
        let items: Vec<ItemId> = victim.wait_for("the items it dropped", |m| match m {
            ServerToClientMessage::DroppedItems(items) if !items.upserts.is_empty() => {
                let items = items.upserts.iter();
                Some(items.map(|(_, item)| item.stack.item_id).collect())
            }
            _ => None,
        });
        missing.retain(|item| !items.contains(item));
    }
    victim.send(ClientToServerMessage::Respawn);
    let id = victim.id;
    victim.wait_for("the respawned player, with nothing on it", |m| match m {
        ServerToClientMessage::PlayerUpdate(u) if u.id == id && u.health > 0 => {
            (u.worn == WornArmour::default()).then_some(())
        }
        _ => None,
    });
}

/// `world.ron` and the file of a player as commit c0e3a7c7 wrote them: without the game mode, the
/// boss and the dungeons of the world, nor the growth, the owner and the tier of its mobs, nor the
/// experience, the armour and the progress of the player.
const FORMER_WORLD: &str = r#"(
    format_version: 5,
    name: "session-test",
    seed: (147),
    time: 1200,
    mobs: {
        7: (
            kind: Sheep,
            target: None,
            action: Idle,
            position: (4.5, 80.0, 4.5),
            rotation: (0.0, 0.0, 0.0, 1.0),
            height: 1.3,
            width: 0.9,
            on_ground: true,
            velocity: (0.0, 0.0, 0.0),
            depth: 0.9,
            action_ticks: 0,
            damage: 2,
            sheared: true,
            passes: 0,
        ),
    },
    dropped_items: [],
    weather: Some((kind: Rain, ticks_left: 5000)),
    generation: (preset: Default, villages: true),
    difficulty: Peaceful,
)"#;
const FORMER_PLAYER: &str = r#"(
    id: 42,
    name: "crafter",
    position: (0.5, 90.0, 0.5),
    camera_transform: (
        translation: (0.5, 90.6, 0.5),
        rotation: (0.0, 0.0, 0.0, 1.0),
        scale: (1.0, 1.0, 1.0),
    ),
    velocity: (0.0, 0.0, 0.0),
    on_ground: false,
    is_flying: true,
    is_sprinting: false,
    is_sneaking: false,
    is_swimming: false,
    fall_distance: 0.0,
    game_mode: Creative,
    health: 13,
    death_cause: None,
    food: 11,
    saturation: 2.5,
    exhaustion: 0.5,
    inventory: (
        inner: {
            0: (item_id: IronPickaxe, item_type: Tool(wear: 12), nb: 1),
            9: (item_id: Cobblestone, item_type: Block(Cobblestone), nb: 33),
            30: (item_id: Stick, item_type: Generic, nb: 4),
        },
        floating: None,
        open_table: None,
        open_chest: None,
        open_furnace: None,
    ),
    height: 1.8,
    width: 0.8,
    last_input_processed: 987654,
)"#;

#[test]
fn a_world_and_a_player_saved_before_the_progression_still_load() {
    let tmp = tempfile::tempdir().unwrap();
    let world_dir = tmp.path().join("saves").join(WORLD_NAME);
    std::fs::create_dir_all(world_dir.join("players")).unwrap();
    std::fs::write(world_dir.join("world.ron"), FORMER_WORLD).unwrap();
    let player_file = world_dir.join("players").join(format!("{PLAYER_ID}.ron"));
    std::fs::write(&player_file, FORMER_PLAYER).unwrap();

    let mut server = TestServer::start_in(tmp, WORLD_NAME);
    let mut client = TestClient::connect(server.addr, PLAYER_ID);
    let position = join(&mut client, "crafter");
    assert_eq!(position, Vec3::new(0.5, 90.0, 0.5));
    let update = play(&mut client, 1, &[&[]]);
    assert_eq!(update.game_mode, GameMode::Creative);
    // A peaceful world gives health and food back meanwhile
    assert!((13..20).contains(&update.health), "{}", update.health);
    assert_eq!(update.worn, WornArmour::default());
    client.send(ClientToServerMessage::Exit);
    client.send_packets();
    server.join_after_exit();

    // Saved again by this version, with all it held
    let saved = std::fs::read_to_string(player_file).unwrap();
    let saved: Player = ron::de::from_str(&saved).unwrap();
    assert!((11..20).contains(&saved.food), "{}", saved.food);
    let pickaxe = saved.inventory.inner[&0];
    assert_eq!(pickaxe.item_id, ItemId::IronPickaxe);
    assert_eq!(pickaxe.item_type, ItemType::Tool { wear: 12 });
    assert_eq!(saved.inventory.inner[&9], stack(ItemId::Cobblestone, 33));
    assert_eq!(saved.inventory.inner[&30], stack(ItemId::Stick, 4));
    let world = std::fs::read_to_string(server.world_dir().join("world.ron")).unwrap();
    for kept in ["Peaceful", "Sheep", "sheared: true"] {
        assert!(world.contains(kept), "{kept} is gone: {world}");
    }
    assert!(
        !world.contains("dungeons: true"),
        "dungeons in an old world"
    );
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
    assert_eq!(inventory, Some(expected));
}

#[test]
fn a_furnace_fed_through_its_slots_lights_for_every_client_and_its_player_sees_it_burn() {
    let mut inventory = Inventory::new();
    inventory.inner.insert(0, stack(ItemId::Furnace, 1));
    inventory.inner.insert(1, stack(ItemId::Sand, 2));
    inventory.inner.insert(2, stack(ItemId::Coal, 1));
    let server = start_with_inventory(inventory);
    let mut client = TestClient::connect(server.addr, PLAYER_ID);
    join(&mut client, "crafter");

    // Place the furnace on the ground, two blocks ahead toward -Z
    let (down, _) = land_and_aim_down(&mut client);
    let position = down.translation;
    let camera = Transform::from_translation(position)
        .looking_at(position + Vec3::new(0., -1.6, -2.), Vec3::Y);
    let ground = raycast::raycast(&client.world, &camera, &position, Default::default())
        .expect("the player aims at the ground");
    let furnace = ground.position + ground.face.to_ivec3();
    let place = PlayerFrameInput {
        time_ms: 100,
        delta_ms: 16,
        inputs: [NetworkAction::RightClick].into(),
        camera,
        hotbar_slot: 0,
        ..Default::default()
    };
    // The sand smelts and goes to the input, the coal burns and goes to the fuel slot
    let fill = PlayerFrameInput {
        time_ms: 101,
        delta_ms: 16,
        camera,
        inventory_actions: vec![
            InventoryAction::OpenFurnace(furnace),
            InventoryAction::QuickMoveSlot(1),
            InventoryAction::QuickMoveSlot(2),
        ],
        ..Default::default()
    };
    client.send(ClientToServerMessage::PlayerInputs(vec![place, fill]));

    let id = client.id;
    let burning = client.wait_for("a PlayerUpdate with the furnace burning", |m| match m {
        ServerToClientMessage::PlayerUpdate(u) if u.id == id => {
            u.open_furnace.clone().filter(|furnace| furnace.is_lit())
        }
        _ => None,
    });
    assert_eq!(burning.input.map(|input| input.item_id), Some(ItemId::Sand));
    assert_eq!(burning.fuel, None);
    assert!(burning.cook > 0);

    // Lit, it still faces the player, toward +Z
    let start = Instant::now();
    while client.world.get_block(furnace).map(|block| block.id) != Some(BlockId::LitFurnace) {
        assert!(
            start.elapsed() < WAIT_TIMEOUT,
            "the lit furnace never reached the client"
        );
        client.pump();
    }
    assert_eq!(
        client.world.get_block(furnace).copied(),
        Some(BlockData::new(BlockId::LitFurnace, BlockDirection::Back))
    );
}

#[test]
fn a_bucket_pours_water_that_spreads_and_takes_it_back_dropping_what_does_not_fit() {
    // A full inventory: a water bucket, two buckets, and dirt
    let mut inventory = Inventory::new();
    inventory.inner.insert(0, stack(ItemId::WaterBucket, 1));
    inventory.inner.insert(1, stack(ItemId::Bucket, 2));
    for slot in 2..MAX_INVENTORY_SLOTS {
        inventory.inner.insert(slot, stack(ItemId::Dirt, 64));
    }
    let server = start_with_inventory(inventory);
    let mut client = TestClient::connect(server.addr, PLAYER_ID);
    join(&mut client, "crafter");

    // Pour the water on the ground, two blocks ahead
    let (down, _) = land_and_aim_down(&mut client);
    let position = down.translation;
    let camera = Transform::from_translation(position)
        .looking_at(position + Vec3::new(0., -1.6, -2.), Vec3::Y);
    let ground = raycast::raycast(&client.world, &camera, &position, Default::default())
        .expect("the player aims at the ground");
    let water = ground.position + ground.face.to_ivec3();
    let right_click = |time_ms, hotbar_slot| PlayerFrameInput {
        time_ms,
        delta_ms: 16,
        inputs: [NetworkAction::RightClick].into(),
        camera,
        hotbar_slot,
        ..Default::default()
    };
    client.send(ClientToServerMessage::PlayerInputs(vec![right_click(
        100, 0,
    )]));
    assert_eq!(wait_for_water(&mut client, water), 0);
    // The flow spreads it into the cells beside it that are free
    let beside = [IVec3::X, IVec3::NEG_X, IVec3::Z, IVec3::NEG_Z].map(|side| water + side);
    let free: Vec<IVec3> = beside
        .into_iter()
        .filter(|&cell| client.world.get_block(cell).is_none())
        .collect();
    assert!(!free.is_empty(), "the water is walled in");
    for cell in free {
        assert_eq!(wait_for_water(&mut client, cell), 1, "{cell}");
    }

    // Released, then the stack of buckets takes the source back
    let released = PlayerFrameInput {
        time_ms: 101,
        delta_ms: 16,
        camera,
        ..Default::default()
    };
    client.send(ClientToServerMessage::PlayerInputs(vec![
        released,
        right_click(102, 1),
    ]));
    let id = client.id;
    let inventory = client.wait_for("PlayerUpdate acknowledging the bucket", |m| match m {
        ServerToClientMessage::PlayerUpdate(u) if u.id == id && u.last_ack_time == 102 => {
            u.inventory.clone()
        }
        _ => None,
    });
    assert_eq!(inventory.inner[&0], stack(ItemId::Bucket, 1));
    assert_eq!(inventory.inner[&1], stack(ItemId::Bucket, 1));
    assert!(inventory
        .inner
        .values()
        .all(|held| held.item_id != ItemId::WaterBucket));

    // The water bucket, with no room for it, lands on the ground
    let thrown = client.wait_for("the water bucket dropped", |m| match m {
        ServerToClientMessage::DroppedItems(update) => update
            .upserts
            .iter()
            .find(|(_, item)| item.stack == stack(ItemId::WaterBucket, 1))
            .map(|(_, item)| item.position),
        _ => None,
    });
    assert!(thrown.distance(position) < 2.0, "dropped at {thrown}");
    let start = Instant::now();
    while client
        .world
        .get_block(water)
        .is_some_and(|block| block.id == BlockId::Water && block.level == 0)
    {
        assert!(start.elapsed() < WAIT_TIMEOUT, "the source stayed");
        client.pump();
    }
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

/// Waits for the answer of the server to a command.
fn command_answer(client: &mut TestClient, content: &str) {
    let answer = received_chat(client, content);
    assert_eq!(answer.author, "Server");
}

fn time_set(client: &mut TestClient) -> u64 {
    client.wait_for("TimeSet", |m| match m {
        ServerToClientMessage::TimeSet(tick) => Some(*tick),
        _ => None,
    })
}

#[test]
fn time_set_moves_every_client_to_midnight_or_noon_and_answers_its_author_only() {
    let mut server = TestServer::start(WORLD_NAME);
    let mut alice = TestClient::connect(server.addr, PLAYER_ID);
    authenticate(&mut alice, "alice");
    let mut bob = TestClient::connect(server.addr, PLAYER_ID + 1);
    authenticate(&mut bob, "bob");

    alice.send(chat_message("/time set night"));
    let night = time_set(&mut alice);
    assert_eq!(night % DAY_DURATION_IN_TICKS, MIDNIGHT_IN_DAY);
    assert_eq!(time_set(&mut bob), night);
    command_answer(&mut alice, "Time set to night");

    alice.send(chat_message("/time set day"));
    let day = time_set(&mut alice);
    assert_eq!(day % DAY_DURATION_IN_TICKS, NOON_IN_DAY);
    assert!(day > night, "the clock went back from {night} to {day}");
    assert_eq!(time_set(&mut bob), day);
    command_answer(&mut alice, "Time set to day");

    alice.send(chat_message("/time set noon"));
    command_answer(&mut alice, "Usage: /time set <day|night>");

    // The chat is ordered: once this message arrived, the earlier ones would have too
    alice.send(chat_message("hello bob"));
    received_chat(&mut alice, "hello bob");
    received_chat(&mut bob, "hello bob");
    assert!(
        !bob.inbox
            .iter()
            .any(|m| matches!(m, ServerToClientMessage::ChatMessage(_))),
        "bob received a command or its answer: {:?}",
        bob.inbox
    );

    alice.send(ClientToServerMessage::Exit);
    alice.send_packets();
    server.join_after_exit();
}

fn weather_set(client: &mut TestClient) -> WeatherKind {
    client.wait_for("WeatherSet", |m| match m {
        ServerToClientMessage::WeatherSet(kind) => Some(*kind),
        _ => None,
    })
}

/// Authenticates `client` as `username` and returns the weather the answer carries.
fn join_weather(client: &mut TestClient, username: &str) -> WeatherKind {
    client.wait_connected();
    client.send(auth_request(username));
    client.wait_for("AuthRegisterResponse", |m| match m {
        ServerToClientMessage::AuthRegisterResponse(response) => Some(response.weather),
        _ => None,
    })
}

#[test]
fn weather_set_changes_every_client_and_answers_its_author_only() {
    let mut server = TestServer::start(WORLD_NAME);
    let mut alice = TestClient::connect(server.addr, PLAYER_ID);
    // a new world starts clear
    assert_eq!(join_weather(&mut alice, "alice"), WeatherKind::Clear);
    let mut bob = TestClient::connect(server.addr, PLAYER_ID + 1);
    authenticate(&mut bob, "bob");

    alice.send(chat_message("/weather thunder"));
    assert_eq!(weather_set(&mut alice), WeatherKind::Thunder);
    assert_eq!(weather_set(&mut bob), WeatherKind::Thunder);
    command_answer(&mut alice, "Weather set to thunder");

    alice.send(chat_message("/weather snow"));
    command_answer(&mut alice, "Usage: /weather <clear|rain|thunder>");

    // A player who joins now learns the weather with its answer
    let mut carol = TestClient::connect(server.addr, PLAYER_ID + 2);
    assert_eq!(join_weather(&mut carol, "carol"), WeatherKind::Thunder);

    // The chat is ordered: once this message arrived, the earlier ones would have too
    alice.send(chat_message("hello bob"));
    received_chat(&mut alice, "hello bob");
    received_chat(&mut bob, "hello bob");
    assert!(
        !bob.inbox
            .iter()
            .any(|m| matches!(m, ServerToClientMessage::ChatMessage(_))),
        "bob received a command or its answer: {:?}",
        bob.inbox
    );

    alice.send(ClientToServerMessage::Exit);
    alice.send_packets();
    server.join_after_exit();
}

#[test]
fn the_weather_is_still_there_after_a_restart() {
    let mut first = TestServer::start(WORLD_NAME);
    let mut client = TestClient::connect(first.addr, PLAYER_ID);
    authenticate(&mut client, "player");
    client.send(chat_message("/weather rain"));
    assert_eq!(weather_set(&mut client), WeatherKind::Rain);
    client.send(ClientToServerMessage::Exit);
    client.send_packets();
    first.join_after_exit();
    let meta = std::fs::read_to_string(first.world_dir().join("world.ron")).unwrap();
    assert!(meta.contains("kind: Rain"), "{meta}");

    let mut second = TestServer::start_in(first.tmp, WORLD_NAME);
    let mut client = TestClient::connect(second.addr, PLAYER_ID);
    assert_eq!(join_weather(&mut client, "player"), WeatherKind::Rain);
    client.send(ClientToServerMessage::Exit);
    client.send_packets();
    second.join_after_exit();
}

#[test]
fn a_survival_player_cannot_take_from_the_catalog_or_destroy_its_stack() {
    let mut inventory = Inventory::new();
    inventory.inner.insert(0, stack(ItemId::Dirt, 5));
    let server = start_with_inventory(inventory);
    let mut client = TestClient::connect(server.addr, PLAYER_ID);
    join(&mut client, "crafter");
    let catalog = |item, count, to_inventory| InventoryAction::TakeFromCatalog {
        item,
        count,
        to_inventory,
    };

    // A new player is in Survival: the dirt is picked up, nothing else happens
    let inventory = send_inventory_actions(
        &mut client,
        1,
        vec![
            InventoryAction::LeftClickSlot(0),
            catalog(ItemId::Stone, 64, true),
            InventoryAction::DestroyFloating,
            catalog(ItemId::Dirt, 1, false),
        ],
    );
    let mut expected = Inventory::new();
    expected.floating = Some(stack(ItemId::Dirt, 5));
    assert_eq!(inventory, expected);

    set_game_mode(&mut client, "creative");
    let inventory = send_inventory_actions(
        &mut client,
        2,
        vec![
            InventoryAction::DestroyFloating,
            catalog(ItemId::Stone, 64, true),
            catalog(ItemId::Sand, 3, false),
        ],
    );
    let mut expected = Inventory::new();
    expected.inner.insert(0, stack(ItemId::Stone, 64));
    expected.floating = Some(stack(ItemId::Sand, 3));
    assert_eq!(inventory, expected);
}

#[test]
fn a_survival_player_fills_the_grid() {
    let mut inventory = Inventory::new();
    inventory.inner.insert(3, stack(ItemId::OakLog, 2));
    let server = start_with_inventory(inventory);
    let mut client = TestClient::connect(server.addr, PLAYER_ID);
    join(&mut client, "crafter");
    let planks = shared::world::recipes_for(ItemId::OakPlanks)
        .next()
        .unwrap();

    let inventory = send_inventory_actions(
        &mut client,
        1,
        vec![InventoryAction::FillCraftingGrid {
            recipe: planks.index,
            times: 1,
        }],
    );
    let mut expected = Inventory::new();
    expected.inner.insert(3, stack(ItemId::OakLog, 1));
    expected
        .inner
        .insert(grid_slot(0, 0), stack(ItemId::OakLog, 1));
    assert_eq!(inventory, expected);

    let inventory = send_inventory_actions(&mut client, 2, vec![InventoryAction::TakeCraftResult]);
    expected.inner.remove(&grid_slot(0, 0));
    expected.floating = Some(stack(ItemId::OakPlanks, 4));
    assert_eq!(inventory, expected);
}

#[test]
fn give_adds_the_items_to_the_inventory_of_its_author() {
    let mut inventory = Inventory::new();
    inventory.inner.insert(0, stack(ItemId::Dirt, 5));
    let server = start_with_inventory(inventory);
    let mut client = TestClient::connect(server.addr, PLAYER_ID);
    join(&mut client, "crafter");

    client.send(chat_message("/give emerald 1"));
    command_answer(&mut client, "Unknown item: emerald");
    client.send(chat_message("/give dirt 70"));
    command_answer(&mut client, "Gave 70 Dirt");
    let inventory = send_inventory_actions(&mut client, 1, vec![]);
    let mut expected = Inventory::new();
    expected.inner.insert(0, stack(ItemId::Dirt, 64));
    expected.inner.insert(1, stack(ItemId::Dirt, 11));
    assert_eq!(inventory, expected);

    // What does not fit falls at the feet of the player, a backpack of it at most; the rest
    // is lost, and the answer says so
    let room = (MAX_INVENTORY_SLOTS - 2) * 64;
    let fell = MAX_INVENTORY_SLOTS * 64;
    client.send(chat_message("/give stone 99999"));
    command_answer(
        &mut client,
        &format!(
            "Gave {} Stone, {} did not fit in the inventory",
            room + fell,
            99999 - room - fell
        ),
    );
    let inventory = send_inventory_actions(&mut client, 2, vec![]);
    for slot in 2..MAX_INVENTORY_SLOTS {
        expected.inner.insert(slot, stack(ItemId::Stone, 64));
    }
    assert_eq!(inventory, expected);
}

#[test]
fn a_survival_player_is_hurt_by_a_fall_dies_of_a_higher_one_and_respawns_at_the_spawn() {
    use shared::players::game_mode::{DeathCause, GameMode, INVULNERABILITY_TICKS, MAX_HEALTH};
    use shared::TICKS_PER_SECOND;

    let mut server = TestServer::start(WORLD_NAME);
    let mut client = TestClient::connect(server.addr, PLAYER_ID);
    client.wait_connected();
    client.send(auth_request("faller"));
    let id = client.id;
    let spawn = client.wait_for("PlayerSpawn for our id", |m| match m {
        ServerToClientMessage::PlayerSpawn(s) if s.id == id => Some(s.data.position),
        _ => None,
    });
    let idle: &[NetworkAction] = &[];
    let fly_up: &[NetworkAction] = &[NetworkAction::JumpOrFlyUp];
    let mut time_ms = 1;
    let mut play_frames = |client: &mut TestClient, frames: Vec<&[NetworkAction]>| {
        let update = play(client, time_ms, &frames);
        time_ms += frames.len() as u64;
        update
    };

    // A new player is in survival, standing on the spawn in full health
    let update = play_frames(&mut client, vec![idle; 10]);
    assert_eq!(update.game_mode, GameMode::Survival);
    assert_eq!(update.health, MAX_HEALTH);
    assert!((update.movement.position - spawn).length() < 0.05);
    let ground = update.movement.position.y;

    // 10 blocks up: a flight of 20 blocks/s takes 5 frames of 100 ms. A survival player
    // cannot fly: it has to be in creative
    let mut toggle_and_fly = vec![&[NetworkAction::ToggleFlyMode][..]];
    toggle_and_fly.extend([fly_up; 5]);
    let update = play_frames(&mut client, toggle_and_fly.clone());
    assert!(!update.movement.is_flying, "flew in survival");
    // It only jumped: let it land
    let update = play_frames(&mut client, vec![idle; 20]);
    assert!((update.movement.position.y - ground).abs() < 0.05);
    set_game_mode(&mut client, "creative");
    let update = play_frames(&mut client, toggle_and_fly);
    assert!(update.movement.is_flying);
    assert!((update.movement.position.y - ground - 10.0).abs() < 0.05);

    // Back in survival, it falls and lands 10 blocks below: 7 points of health
    set_game_mode(&mut client, "survival");
    let update = play_frames(&mut client, vec![idle; 30]);
    assert!(update.movement.on_ground && !update.movement.is_flying);
    // The health may have grown back by a point meanwhile
    assert!(
        (MAX_HEALTH - 7..=MAX_HEALTH - 6).contains(&update.health),
        "health {}",
        update.health
    );
    assert_eq!(update.death_cause, None);
    // The frames go in bursts: a second fall would land within the invulnerability of the first
    let invulnerability = INVULNERABILITY_TICKS as u64 * 1000 / TICKS_PER_SECOND;
    thread::sleep(Duration::from_millis(2 * invulnerability));

    // 30 blocks kill it
    set_game_mode(&mut client, "creative");
    let mut flight = vec![&[NetworkAction::ToggleFlyMode][..]];
    flight.extend([fly_up; 15]);
    play_frames(&mut client, flight);
    set_game_mode(&mut client, "survival");
    let update = play_frames(&mut client, vec![idle; 40]);
    assert_eq!(update.health, 0);
    assert_eq!(update.death_cause, Some(DeathCause::Fall));
    let died_at = update.movement.position;

    // A dead player does not move, and the health it gets back waits for its respawn
    let update = play_frames(&mut client, vec![&[NetworkAction::MoveForward][..]; 10]);
    assert_eq!(update.movement.position, died_at);
    assert_eq!(update.health, 0);

    client.send(ClientToServerMessage::Respawn);
    let respawned = client.wait_for("PlayerUpdate after the respawn", |m| match m {
        ServerToClientMessage::PlayerUpdate(u) if u.id == id && u.health > 0 => Some(u.clone()),
        _ => None,
    });
    assert_eq!(respawned.health, MAX_HEALTH);
    assert_eq!(respawned.death_cause, None);
    assert_eq!(respawned.movement.position, spawn);

    // A living player asking again stays where it is
    let update = play_frames(&mut client, vec![&[NetworkAction::MoveForward][..]; 5]);
    client.send(ClientToServerMessage::Respawn);
    let after = play_frames(&mut client, vec![idle; 1]);
    let (after, before) = (after.movement.position, update.movement.position);
    assert_eq!((after.x, after.z), (before.x, before.z));

    client.send(ClientToServerMessage::Exit);
    client.send_packets();
    server.join_after_exit();
}

/// A mod whose `/wall` builds a stone wall 4 wide and 4 high, facing +Z, 6 blocks north of its
/// author, from 2 blocks above its centre.
const WALL_MOD: &str = r#"
    rustcraft.register_command("wall", function(ctx, args)
        local p = rustcraft.player_position(ctx.player)
        for x = -2, 1 do
            for y = 2, 5 do
                rustcraft.set_block({ x = p.x + x, y = p.y + y, z = p.z - 6 }, "stone")
            end
        end
        return "Wall built"
    end)
"#;

/// The portals of the next `PortalUpdates` the client receives.
fn portal_updates(client: &mut TestClient) -> Vec<Portal> {
    client.wait_for("PortalUpdates", |m| match m {
        ServerToClientMessage::PortalUpdates(portals) => Some(portals.clone()),
        _ => None,
    })
}

#[test]
fn a_portal_reaches_every_player_and_goes_with_its_owner() {
    let tmp = tempfile::tempdir().unwrap();
    let wall_mod = tmp.path().join("mods").join("wall");
    std::fs::create_dir_all(&wall_mod).unwrap();
    std::fs::write(wall_mod.join("init.luau"), WALL_MOD).unwrap();
    // alice runs /wall and /give, an operator's commands
    std::fs::write(tmp.path().join(server::OPS_FILE), r#"["alice"]"#).unwrap();
    let server = TestServer::start_with(tmp, WORLD_NAME, false);

    // A joining player is told the portals of the world: none yet
    let mut alice = TestClient::connect(server.addr, PLAYER_ID);
    join(&mut alice, "alice");
    assert_eq!(portal_updates(&mut alice), []);
    let (landed, _) = land_and_aim_down(&mut alice);
    alice.send(chat_message("/wall"));
    command_answer(&mut alice, "Wall built");
    alice.send(chat_message("/give portal_gun 1"));
    command_answer(&mut alice, "Gave 1 PortalGun");

    let position = landed.translation;
    let aimed = position.floor().as_ivec3() + IVec3::new(0, 3, -6);
    let start = Instant::now();
    while alice.world.get_block(aimed).is_none() {
        assert!(start.elapsed() < WAIT_TIMEOUT, "the wall never arrived");
        alice.pump();
    }
    let camera = Transform::from_translation(position + Vec3::Y * 0.8)
        .looking_at(aimed.as_vec3() + Vec3::new(0.5, 0.5, 1.0), Vec3::Y);
    alice.send(ClientToServerMessage::PlayerInputs(vec![
        PlayerFrameInput {
            time_ms: 41,
            delta_ms: 16,
            inputs: [NetworkAction::LeftClick].into(),
            camera,
            ..Default::default()
        },
    ]));
    let portals = portal_updates(&mut alice);
    assert_eq!(portals.len(), 1, "{portals:?}");
    let blue = portals[0];
    assert_eq!((blue.owner, blue.color), (PLAYER_ID, PortalColor::Blue));
    assert_eq!(blue.face, FaceDirection::PlusZ);
    assert!(blue.cells().any(|cell| cell == aimed));

    // Another player joining is told of it, and sees it go when its owner leaves
    let mut bob = TestClient::connect(server.addr, PLAYER_ID + 1);
    join(&mut bob, "bob");
    assert_eq!(portal_updates(&mut bob), [blue]);
    alice.send(ClientToServerMessage::Exit);
    alice.send_packets();
    assert_eq!(portal_updates(&mut bob), []);
}

#[test]
fn a_shot_reaches_every_other_player_before_its_portal() {
    let tmp = tempfile::tempdir().unwrap();
    let wall_mod = tmp.path().join("mods").join("wall");
    std::fs::create_dir_all(&wall_mod).unwrap();
    std::fs::write(wall_mod.join("init.luau"), WALL_MOD).unwrap();
    // alice runs /wall and /give, an operator's commands
    std::fs::write(tmp.path().join(server::OPS_FILE), r#"["alice"]"#).unwrap();
    let server = TestServer::start_with(tmp, WORLD_NAME, false);
    let mut alice = TestClient::connect(server.addr, PLAYER_ID);
    join(&mut alice, "alice");
    assert_eq!(portal_updates(&mut alice), []);
    let (landed, _) = land_and_aim_down(&mut alice);
    alice.send(chat_message("/wall"));
    command_answer(&mut alice, "Wall built");
    alice.send(chat_message("/give portal_gun 1"));
    command_answer(&mut alice, "Gave 1 PortalGun");
    let position = landed.translation;
    let aimed = position.floor().as_ivec3() + IVec3::new(0, 3, -6);
    let start = Instant::now();
    while alice.world.get_block(aimed).is_none() {
        assert!(start.elapsed() < WAIT_TIMEOUT, "the wall never arrived");
        alice.pump();
    }
    let mut bob = TestClient::connect(server.addr, PLAYER_ID + 1);
    join(&mut bob, "bob");
    assert_eq!(portal_updates(&mut bob), []);

    let eye = position + Vec3::Y * 0.8;
    let camera = Transform::from_translation(eye)
        .looking_at(aimed.as_vec3() + Vec3::new(0.5, 0.5, 1.0), Vec3::Y);
    let shoot = |time_ms, button, camera| {
        ClientToServerMessage::PlayerInputs(vec![PlayerFrameInput {
            time_ms,
            delta_ms: 16,
            inputs: [button].into(),
            camera,
            ..Default::default()
        }])
    };
    alice.send(shoot(41, NetworkAction::LeftClick, camera));
    // The shooter predicted its own shot, and is only told of the portal
    let blue = portal_updates(&mut alice);
    assert_eq!(blue.len(), 1, "{blue:?}");
    // Bob hears of the shot first, then of its portal
    let first = bob.wait_for("the shot or its portal", |m| match m {
        ServerToClientMessage::PortalShot { .. } | ServerToClientMessage::PortalUpdates(_) => {
            Some(m.clone())
        }
        _ => None,
    });
    let ServerToClientMessage::PortalShot {
        shooter,
        from,
        to,
        color,
        placed,
    } = first
    else {
        panic!("the portal came before its shot: {first:?}");
    };
    assert_eq!(
        (shooter, color, placed),
        (PLAYER_ID, PortalColor::Blue, true)
    );
    assert_eq!(from, eye);
    assert!((to.z - (aimed.z + 1) as f32).abs() < 1e-4, "{to}");
    assert_eq!(portal_updates(&mut bob), blue);
    sync_with_server(&mut alice, 42);
    assert!(
        !alice
            .inbox
            .iter()
            .any(|m| matches!(m, ServerToClientMessage::PortalShot { .. })),
        "the shooter was told of its own shot"
    );

    // A shot at the sky flies too, and places nothing
    let up = Transform::from_translation(eye).looking_to(Vec3::Y, Vec3::Z);
    alice.send(shoot(43, NetworkAction::RightClick, up));
    sync_with_server(&mut alice, 44);
    let refused = bob.wait_for("the refused shot", |m| match m {
        ServerToClientMessage::PortalShot { color, placed, .. } => Some((*color, *placed)),
        _ => None,
    });
    assert_eq!(refused, (PortalColor::Orange, false));

    // A shot from a camera far from alice's eyes, or nowhere, is not told: the camera comes
    // from the network. Bob is told of the next honest one
    let far =
        Transform::from_translation(eye + Vec3::new(0.0, 1.0e6, 0.0)).looking_to(Vec3::Y, Vec3::Z);
    alice.send(shoot(45, NetworkAction::LeftClick, far));
    let nowhere = Transform::from_translation(Vec3::NAN).looking_to(Vec3::Y, Vec3::Z);
    alice.send(shoot(46, NetworkAction::RightClick, nowhere));
    alice.send(shoot(47, NetworkAction::LeftClick, up));
    sync_with_server(&mut alice, 48);
    let told = bob.wait_for("the honest shot", |m| match m {
        ServerToClientMessage::PortalShot { from, color, .. } => Some((*from, *color)),
        _ => None,
    });
    assert_eq!(told, (eye, PortalColor::Blue));
}

/// Pumps until the server tells the client why it is dismissed, then until it disconnects it.
fn dismissed(client: &mut TestClient) -> String {
    let reason = client.wait_for("Dismissed", |m| match m {
        ServerToClientMessage::Dismissed(reason) => Some(reason.clone()),
        _ => None,
    });
    let start = Instant::now();
    while !client.client.is_disconnected() {
        assert!(
            start.elapsed() < WAIT_TIMEOUT,
            "the dismissed client was not disconnected within {WAIT_TIMEOUT:?}"
        );
        client.pump();
    }
    reason
}

/// Connects a client that asks to join as `username`, and returns why it was refused.
fn refused(server: &TestServer, client_id: u64, username: &str) -> String {
    let mut client = TestClient::connect(server.addr, client_id);
    client.wait_connected();
    client.send(auth_request(username));
    let reason = dismissed(&mut client);
    assert!(
        !client
            .inbox
            .iter()
            .any(|m| matches!(m, ServerToClientMessage::AuthRegisterResponse(_))),
        "{username} was let in"
    );
    reason
}

#[test]
fn on_a_dedicated_server_only_an_operator_may_give_and_make_operators() {
    let server = TestServer::start_dedicated_with_ops(WORLD_NAME, r#"["Alice"]"#);
    let mut alice = TestClient::connect(server.addr, PLAYER_ID);
    join(&mut alice, "alice");
    let mut bob = TestClient::connect(server.addr, PLAYER_ID + 1);
    join(&mut bob, "bob");

    bob.send(chat_message("/give dirt 1"));
    command_answer(&mut bob, "You must be an operator to use /give");
    bob.send(chat_message("/gamemode creative"));
    command_answer(&mut bob, "You must be an operator to use /gamemode");
    bob.send(chat_message("/op bob"));
    command_answer(&mut bob, "You must be an operator to use /op");
    // an unknown command is still unknown, and /list is everyone's
    bob.send(chat_message("/fly"));
    command_answer(&mut bob, "Unknown command: /fly");
    bob.send(chat_message("/list"));
    command_answer(&mut bob, "2 players online: alice, bob");

    alice.send(chat_message("/give dirt 1"));
    command_answer(&mut alice, "Gave 1 Dirt");
    alice.send(chat_message("/op bob"));
    command_answer(&mut alice, "Made bob an operator");
    bob.send(chat_message("/give dirt 2"));
    command_answer(&mut bob, "Gave 2 Dirt");
    let ops = std::fs::read_to_string(server.tmp.path().join(server::OPS_FILE)).unwrap();
    assert!(ops.contains("\"bob\""), "{ops}");

    alice.send(chat_message("/deop bob"));
    command_answer(&mut alice, "bob is no longer an operator");
    bob.send(chat_message("/time set day"));
    command_answer(&mut bob, "You must be an operator to use /time");
}

#[test]
fn a_kicked_player_is_told_why_and_disconnected() {
    let server = TestServer::start_dedicated_with_ops(WORLD_NAME, r#"["alice"]"#);
    let mut alice = TestClient::connect(server.addr, PLAYER_ID);
    join(&mut alice, "alice");
    let mut bob = TestClient::connect(server.addr, PLAYER_ID + 1);
    join(&mut bob, "Bob");

    alice.send(chat_message("/kick carol"));
    command_answer(&mut alice, "No player named carol is online");
    alice.send(chat_message("/kick bob  too   loud"));
    command_answer(&mut alice, "Kicked bob");
    assert_eq!(
        dismissed(&mut bob),
        "You were kicked from the server: too   loud"
    );

    // a kick is no ban
    let mut back = TestClient::connect(server.addr, PLAYER_ID + 2);
    join(&mut back, "Bob");
}

#[test]
fn a_dismissed_operator_is_no_longer_listened_to_and_its_name_may_hold_spaces() {
    let server = TestServer::start_dedicated_with_ops(WORLD_NAME, r#"["alice", "bad guy"]"#);
    let mut alice = TestClient::connect(server.addr, PLAYER_ID);
    join(&mut alice, "alice");
    let mut bad = TestClient::connect(server.addr, PLAYER_ID + 1);
    join(&mut bad, "Bad Guy");

    alice.send(chat_message(r#"/ban "bad guy" griefing"#));
    command_answer(&mut alice, "Banned bad guy");
    bad.send(chat_message(r#"/pardon "bad guy""#));
    dismissed(&mut bad);
    assert_eq!(
        refused(&server, PLAYER_ID + 2, "bad guy"),
        "You are banned from this server: griefing"
    );
}

#[test]
fn a_name_in_use_is_refused_whatever_its_case() {
    let server = TestServer::start_dedicated_with_ops(WORLD_NAME, r#"["alice"]"#);
    let mut alice = TestClient::connect(server.addr, PLAYER_ID);
    join(&mut alice, "alice");
    let mut other = TestClient::connect(server.addr, PLAYER_ID + 1);
    other.wait_connected();
    other.send(auth_request("ALICE"));
    let start = Instant::now();
    while !other.client.is_disconnected() {
        assert!(start.elapsed() < WAIT_TIMEOUT, "ALICE was not refused");
        other.pump();
    }
    assert!(!other
        .inbox
        .iter()
        .any(|m| matches!(m, ServerToClientMessage::AuthRegisterResponse(_))));
}

#[test]
fn a_banned_player_is_refused_with_the_reason_until_pardoned() {
    let server = TestServer::start_dedicated_with_ops(WORLD_NAME, r#"["alice"]"#);
    let mut alice = TestClient::connect(server.addr, PLAYER_ID);
    join(&mut alice, "alice");
    let mut bob = TestClient::connect(server.addr, PLAYER_ID + 1);
    join(&mut bob, "bob");

    alice.send(chat_message("/ban bob griefing"));
    command_answer(&mut alice, "Banned bob");
    assert_eq!(
        dismissed(&mut bob),
        "You are banned from this server: griefing"
    );
    assert_eq!(
        refused(&server, PLAYER_ID + 2, "BOB"),
        "You are banned from this server: griefing"
    );
    let banned = std::fs::read_to_string(server.tmp.path().join(server::BANNED_FILE)).unwrap();
    assert!(banned.contains("griefing"), "{banned}");

    alice.send(chat_message("/pardon bob"));
    command_answer(&mut alice, "Pardoned bob");
    let mut back = TestClient::connect(server.addr, PLAYER_ID + 3);
    join(&mut back, "bob");
}

#[test]
fn with_the_whitelist_on_only_the_players_on_it_and_the_operators_join() {
    let server = TestServer::start_dedicated_with_ops(WORLD_NAME, r#"["alice"]"#);
    let mut alice = TestClient::connect(server.addr, PLAYER_ID);
    join(&mut alice, "alice");

    alice.send(chat_message("/whitelist on"));
    command_answer(&mut alice, "The whitelist is on");
    assert_eq!(
        refused(&server, PLAYER_ID + 1, "carol"),
        "You are not on the whitelist of this server."
    );

    alice.send(chat_message("/whitelist add carol"));
    command_answer(&mut alice, "Added carol to the whitelist");
    let mut carol = TestClient::connect(server.addr, PLAYER_ID + 2);
    join(&mut carol, "carol");
    let whitelist =
        std::fs::read_to_string(server.tmp.path().join(server::WHITELIST_FILE)).unwrap();
    assert!(whitelist.contains("carol"), "{whitelist}");
}

#[test]
fn a_solo_player_is_an_operator_without_any_list() {
    let mut server = TestServer::start(WORLD_NAME);
    let mut client = TestClient::connect(server.addr, PLAYER_ID);
    join(&mut client, "solo");
    client.send(chat_message("/give dirt 1"));
    command_answer(&mut client, "Gave 1 Dirt");
    assert!(!server.tmp.path().join(server::OPS_FILE).exists());
    client.send(ClientToServerMessage::Exit);
    client.send_packets();
    server.join_after_exit();
}

/// The saved player of the world of `tmp`.
fn saved_player(tmp: &tempfile::TempDir) -> Player {
    let file = tmp.path().join("saves").join(WORLD_NAME);
    let file = file.join("players").join(format!("{PLAYER_ID}.ron"));
    ron::from_str(&std::fs::read_to_string(file).unwrap()).unwrap()
}

/// Stops `server` from `client`, and starts it again with its player changed by `edit`.
fn restart_with(
    mut server: TestServer,
    client: &mut TestClient,
    edit: impl FnOnce(&mut Player),
) -> (TestServer, TestClient) {
    client.send(ClientToServerMessage::Exit);
    client.send_packets();
    server.join_after_exit();
    let mut player = saved_player(&server.tmp);
    edit(&mut player);
    let file = server.tmp.path().join("saves").join(WORLD_NAME);
    let file = file.join("players").join(format!("{PLAYER_ID}.ron"));
    std::fs::write(file, ron::ser::to_string(&player).unwrap()).unwrap();
    let TestServer { tmp, .. } = server;
    let server = TestServer::start_in(tmp, WORLD_NAME);
    let mut client = TestClient::connect(server.addr, PLAYER_ID);
    client.wait_connected();
    join(&mut client, "crafter");
    (server, client)
}

/// Every mob the client was told of, as last told, but those removed.
fn known_mobs(client: &TestClient) -> HashMap<MobId, ServerMob> {
    let mut mobs = HashMap::new();
    for message in &client.inbox {
        match message {
            ServerToClientMessage::WorldUpdate(update) => {
                mobs.extend(update.mobs.iter().map(|(id, mob)| (*id, mob.clone())));
            }
            ServerToClientMessage::MobUpdate(update) => {
                mobs.insert(update.id, update.mob.clone());
            }
            ServerToClientMessage::MobRemoved(id) => {
                mobs.remove(id);
            }
            _ => {}
        }
    }
    mobs
}

/// From a new world to the credits, through the network, the moves from a place to the next made
/// in the save: craft, dress, trade, enchant, beat the Ember Mother, reload.
#[test]
#[ignore = "a minute: cargo test -p server --test session -- --ignored"]
fn a_world_is_played_from_its_first_tool_to_the_credits_and_reloaded() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("saves").join(WORLD_NAME);
    std::fs::create_dir_all(&dir).unwrap();
    let meta = format!(
        "(format_version: 5, name: \"{WORLD_NAME}\", seed: ({FLAT_SPAWN_SEED}), time: 15000, \
         mobs: {{}}, generation: (preset: Default, villages: true, dungeons: true, deep: true))"
    );
    std::fs::write(dir.join("world.ron"), meta).unwrap();
    let server = TestServer::start_in(tmp, WORLD_NAME);
    let mut client = TestClient::connect(server.addr, PLAYER_ID);
    join(&mut client, "crafter");
    for give in [
        "oak_log 2",
        "diamond_chestplate 1",
        "diamond_helmet 1",
        "diamond_leggings 1",
        "diamond_boots 1",
        "bow 1",
        "arrow 64",
        "rye 24",
        "engraving_table 1",
    ] {
        client.send(chat_message(&format!("/give {give}")));
    }
    sync_with_server(&mut client, 1);
    // Planks of a log, then the armour on
    let inventory = send_inventory_actions(
        &mut client,
        2,
        vec![
            InventoryAction::LeftClickSlot(0),
            InventoryAction::RightClickSlot(grid_slot(0, 0)),
            InventoryAction::CraftAll,
            InventoryAction::Close,
            InventoryAction::QuickMoveSlot(1),
            InventoryAction::QuickMoveSlot(2),
            InventoryAction::QuickMoveSlot(3),
            InventoryAction::QuickMoveSlot(4),
        ],
    );
    assert_eq!(inventory.worn().0.iter().flatten().count(), 4);
    assert!(inventory
        .inner
        .values()
        .any(|s| s.item_id == ItemId::OakPlanks));

    // The bow enchanted at the table, put down ahead
    let (server, mut client) = restart_with(server, &mut client, |player| {
        player.experience.level = 40;
    });
    let (down, _) = land_and_aim_down(&mut client);
    let camera = down.looking_to(Vec3::new(1.0, -1.0, 0.0), Vec3::Y);
    let hit = raycast::raycast(
        &client.world,
        &camera,
        &camera.translation,
        Default::default(),
    );
    let table = hit.unwrap().position + IVec3::Y;
    let inventory = saved_player(&server.tmp).inventory;
    let slot_of = |item_id| {
        let slots = &inventory.inner;
        let found = slots.iter().find(|(_, stack)| stack.item_id == item_id);
        *found.unwrap().0
    };
    let bow = slot_of(ItemId::Bow);
    client.send(ClientToServerMessage::PlayerInputs(vec![
        PlayerFrameInput {
            time_ms: 100,
            delta_ms: 16,
            inputs: [NetworkAction::RightClick].into_iter().collect(),
            camera,
            hotbar_slot: slot_of(ItemId::EngravingTable),
            ..Default::default()
        },
    ]));
    let inventory = send_inventory_actions(
        &mut client,
        101,
        vec![
            InventoryAction::OpenEngravingTable(table),
            InventoryAction::QuickMoveSlot(bow),
            InventoryAction::Enchant(Enchantment::BowPower),
            InventoryAction::Enchant(Enchantment::BowPower),
            InventoryAction::Enchant(Enchantment::BowPower),
            InventoryAction::Close,
        ],
    );
    let enchanted = inventory.inner.values().find(|s| s.is_enchanted()).unwrap();
    assert_eq!(enchanted.bonus_arrow_damage(), 3);

    // The village: where, then by its farmer
    client.send(chat_message("/locate village"));
    let answer = client.wait_for("the village", |m| match m {
        ServerToClientMessage::ChatMessage(m) if m.content.starts_with("The nearest village") => {
            Some(m.content.clone())
        }
        _ => None,
    });
    let numbers: Vec<f32> = answer
        .split(|c: char| !(c.is_ascii_digit() || c == '-'))
        .filter_map(|word| word.parse().ok())
        .collect();
    let (x, z) = (numbers[0], numbers[1]);
    let (server, mut client) = restart_with(server, &mut client, |player| {
        player.position = Vec3::new(x, 85.0, z);
        player.game_mode = GameMode::Creative;
        player.is_flying = true;
    });
    let start = Instant::now();
    let farmer = loop {
        assert!(start.elapsed() < 6 * WAIT_TIMEOUT, "no farmer");
        let farmers = known_mobs(&client);
        if let Some((id, mob)) = farmers.iter().find(|(_, m)| m.kind == MobKind::Farmer) {
            break (*id, mob.position);
        }
        client.send(inputs_at(1 + start.elapsed().as_millis() as u64));
        client.pump();
    };
    let (server, mut client) = restart_with(server, &mut client, |player| {
        player.position = farmer.1 + Vec3::new(1.2, 0.2, 0.0);
        player.game_mode = GameMode::Survival;
        player.is_flying = false;
    });
    let start = Instant::now();
    let mut opened = None;
    while opened.is_none() {
        assert!(start.elapsed() < 3 * WAIT_TIMEOUT, "no trade");
        let time_ms = 1 + start.elapsed().as_millis() as u64;
        let me = player_position(&client).unwrap_or(farmer.1 + Vec3::X);
        let eye = me + Vec3::Y * 0.6;
        let at = known_mobs(&client)
            .get(&farmer.0)
            .map_or(farmer.1, |mob| mob.position);
        let held = (start.elapsed().as_millis() / 100).is_multiple_of(2);
        client.send(ClientToServerMessage::PlayerInputs(vec![
            PlayerFrameInput {
                time_ms,
                delta_ms: 5,
                inputs: held
                    .then_some(NetworkAction::RightClick)
                    .into_iter()
                    .collect(),
                camera: Transform::from_translation(eye).looking_at(at, Vec3::Y),
                hotbar_slot: 8,
                ..Default::default()
            },
        ]));
        client.pump();
        opened = client.inbox.iter().find_map(|m| match m {
            ServerToClientMessage::TradeUpdate(Some(view)) => Some(view.clone()),
            _ => None,
        });
    }
    let view = opened.unwrap();
    client.send(ClientToServerMessage::Trade {
        mob: view.mob,
        offer: 0,
    });
    client.wait_for("the coin", |m| match m {
        ServerToClientMessage::PlayerUpdate(u) if u.id == PLAYER_ID => u
            .inventory
            .as_ref()
            .filter(|i| i.inner.values().any(|s| s.item_id == ItemId::Coin))
            .map(|_| ()),
        _ => None,
    });
    client.send(ClientToServerMessage::EndTrade);

    // The lair
    let heart = lair::heart(lair::centre(
        FLAT_SPAWN_SEED,
        &TerrainNoises::new(FLAT_SPAWN_SEED),
        shared::world::WorldGenSettings::default().underground(),
    ));
    let stand = heart + Vec3::new(0.0, 0.95, 18.0);
    let (server, mut client) = restart_with(server, &mut client, |player| {
        assert!(player.progress.has(Achievement::Trade));
        assert!(player.progress.has(Achievement::FullSuit));
        player.position = stand;
        assert!(player.progress.has(Achievement::Enchantment));
        assert_eq!(player.experience.level, 24);
        player.health = 5000;
    });
    let bow = saved_player(&server.tmp)
        .inventory
        .inner
        .iter()
        .find(|(_, s)| s.item_id == ItemId::Bow)
        .map(|(slot, _)| *slot)
        .unwrap();
    let start = Instant::now();
    let mut time_ms = 1;
    let mut won = false;
    while !won {
        assert!(start.elapsed() < 12 * WAIT_TIMEOUT, "she is not beaten");
        let mother = known_mobs(&client)
            .into_values()
            .find(|m| m.kind == MobKind::EmberMother)
            .map_or(heart, |m| m.position);
        let me = player_position(&client).unwrap_or(stand);
        let eye = me + Vec3::Y * 0.6;
        // Over it, for the fall of the arrow
        let aim = mother + Vec3::Y * 0.02 * eye.distance_squared(mother) / 2.0;
        for step in 0..12 {
            let drawing = step < 11;
            client.send(ClientToServerMessage::PlayerInputs(vec![
                PlayerFrameInput {
                    time_ms,
                    delta_ms: 100,
                    inputs: drawing
                        .then_some(NetworkAction::RightClick)
                        .into_iter()
                        .collect(),
                    camera: Transform::from_translation(eye).looking_at(aim, Vec3::Y),
                    hotbar_slot: bow,
                    ..Default::default()
                },
            ]));
            time_ms += 1;
            let sent = Instant::now();
            while sent.elapsed() < Duration::from_millis(40) {
                client.pump();
            }
        }
        won = client
            .inbox
            .iter()
            .any(|m| matches!(m, ServerToClientMessage::Victory));
    }
    sync_with_server(&mut client, time_ms + 1);
    let (_server, mut client) = restart_with(server, &mut client, |player| {
        assert!(player.progress.has(Achievement::Boss));
        assert!(player.progress.has(Achievement::Lair));
        assert!(player.experience.level >= 24);
        assert!(!player.is_dead());
        assert!(player
            .inventory
            .inner
            .values()
            .any(|s| s.is_enchanted() && s.item_type.wear() > Some(0)));
    });
    // Beaten for good
    for time_ms in 1..60 {
        sync_with_server(&mut client, time_ms);
    }
    assert!(known_mobs(&client)
        .values()
        .all(|mob| mob.kind != MobKind::EmberMother));
    let meta = std::fs::read_to_string(_server.world_dir().join("world.ron")).unwrap();
    assert!(meta.contains("boss_defeated: true"), "{meta}");
}

/// Where the server last said the player of `client` is.
fn player_position(client: &TestClient) -> Option<Vec3> {
    client.inbox.iter().rev().find_map(|m| match m {
        ServerToClientMessage::PlayerUpdate(u) if u.id == client.id => Some(u.movement.position),
        _ => None,
    })
}

/// A hub of the tests: its key, and the device of an account it signs certificates for.
struct TestHub {
    key: SigningKey,
    device: SigningKey,
    certificate: Certificate,
    trust: HubTrust,
}

const TEST_HUB: &str = "https://hub.test";
/// The address the players of the hub servers of the tests join them at.
const PUBLIC_ADDRESS: &str = "play.test:8000";

fn random_key() -> SigningKey {
    SigningKey::from_bytes(&rand::random())
}

impl TestHub {
    fn new() -> Self {
        let key = random_key();
        let device = random_key();
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let certificate = Certificate {
            version: CERTIFICATE_VERSION,
            hub: TEST_HUB.into(),
            id: Uuid::from_u128(rand::random()),
            account: Uuid::from_u128(rand::random()),
            name: "Alex".into(),
            device_key: device.verifying_key().to_bytes(),
            issued_at: now - 60,
            expires_at: now + CERTIFICATE_LIFETIME_SECS,
            hub_key_id: 1,
        };
        let trust = HubTrust {
            url: TEST_HUB.into(),
            addresses: vec![PUBLIC_ADDRESS.into()],
            keys: vec![(1, key.verifying_key())],
            revoked: Default::default(),
        };
        Self {
            key,
            device,
            certificate,
            trust,
        }
    }

    fn player_id(&self) -> u64 {
        player_id(&self.certificate.account)
    }

    /// The proof of its certificate for this challenge, at `PUBLIC_ADDRESS`.
    fn proof(&self, nonce: &[u8; 32], server_key: &[u8; 32]) -> JoinProof {
        proof_of(
            &self.certificate,
            &self.key,
            &self.device,
            nonce,
            server_key,
            PUBLIC_ADDRESS,
        )
    }
}

/// What the server asks a client as it connects: its nonce, its key and its hub.
fn challenge(client: &mut TestClient) -> ([u8; 32], [u8; 32], Option<String>) {
    client.wait_connected();
    client.wait_for("AuthChallenge", |m| match m {
        ServerToClientMessage::AuthChallenge {
            nonce,
            server_key,
            hub,
        } => Some((*nonce, *server_key, hub.clone())),
        _ => None,
    })
}

/// A request to join as `username` with `proof`.
fn request_with(username: &str, proof: Option<JoinProof>) -> ClientToServerMessage {
    AuthRegisterRequest {
        username: username.to_string(),
        render_distance: 2,
        skin: Default::default(),
        proof,
    }
    .into()
}

/// The proof of `certificate` signed by `hub_key`, its device signing `nonce`, `server_key`
/// and `address`.
fn proof_of(
    certificate: &Certificate,
    hub_key: &SigningKey,
    device: &SigningKey,
    nonce: &[u8; 32],
    server_key: &[u8; 32],
    address: &str,
) -> JoinProof {
    let signed = sign(certificate, hub_key);
    JoinProof {
        certificate: signed.bytes,
        certificate_signature: signed.signature.to_vec(),
        address: address.into(),
        signature: sign_join(device, nonce, server_key, address).to_vec(),
    }
}

fn accepted_name(client: &mut TestClient) -> String {
    client.wait_for("AuthRegisterResponse", |m| match m {
        ServerToClientMessage::AuthRegisterResponse(r) => Some(r.username.clone()),
        _ => None,
    })
}

#[test]
fn an_offline_server_challenges_without_a_hub_and_ignores_any_proof() {
    let hub = TestHub::new();
    let server = TestServer::start_dedicated(WORLD_NAME);
    let mut client = TestClient::connect(server.addr, PLAYER_ID);
    let (_, _, announced) = challenge(&mut client);
    assert_eq!(announced, None);
    client.send(request_with("steve", None));
    assert_eq!(accepted_name(&mut client), "steve");

    // a proof that proves nothing changes nothing either
    let mut bogus = TestClient::connect(server.addr, PLAYER_ID + 1);
    challenge(&mut bogus);
    let proof = proof_of(
        &hub.certificate,
        &random_key(),
        &hub.device,
        &[0; 32],
        &[0; 32],
        "x",
    );
    bogus.send(request_with("bob", Some(proof)));
    assert_eq!(accepted_name(&mut bogus), "bob");
    // an offline server has no key file
    assert!(!server
        .tmp
        .path()
        .join(server::join::SERVER_KEY_FILE)
        .exists());
}

#[test]
fn a_hub_server_takes_a_valid_proof_and_names_the_player_after_its_certificate() {
    let hub = TestHub::new();
    let server = TestServer::start_hub(WORLD_NAME, hub.trust.clone());
    let mut client = TestClient::connect(server.addr, hub.player_id());
    let (nonce, server_key, announced) = challenge(&mut client);
    assert_eq!(announced.as_deref(), Some(TEST_HUB));

    client.send(request_with(
        "impostor",
        Some(hub.proof(&nonce, &server_key)),
    ));
    let auth = client.wait_for("AuthRegisterResponse", |m| match m {
        ServerToClientMessage::AuthRegisterResponse(r) => Some(r.clone()),
        _ => None,
    });
    assert_eq!(auth.username, "Alex");
    assert_eq!(auth.session_token, hub.player_id());
    assert!(server
        .tmp
        .path()
        .join(server::join::SERVER_KEY_FILE)
        .is_file());
}

/// The reason a hub server gives a client of id `client_id` that answers its challenge with
/// what `answer` makes of the nonce and the key of the server.
fn hub_refusal(
    server: &TestServer,
    client_id: u64,
    answer: impl FnOnce(&[u8; 32], &[u8; 32]) -> Option<JoinProof>,
) -> String {
    let mut client = TestClient::connect(server.addr, client_id);
    let (nonce, server_key, _) = challenge(&mut client);
    client.send(request_with("Alex", answer(&nonce, &server_key)));
    let reason = dismissed(&mut client);
    assert!(
        !client
            .inbox
            .iter()
            .any(|m| matches!(m, ServerToClientMessage::AuthRegisterResponse(_))),
        "the player was let in"
    );
    reason
}

#[test]
fn a_hub_server_dismisses_each_bad_proof_with_its_own_reason() {
    let hub = TestHub::new();
    let server = TestServer::start_hub(WORLD_NAME, hub.trust.clone());
    let id = hub.player_id();
    let (cert, key, device) = (&hub.certificate, &hub.key, &hub.device);
    let proof = |cert: &Certificate, key: &SigningKey, nonce: &[u8; 32], server_key: &[u8; 32]| {
        proof_of(cert, key, device, nonce, server_key, PUBLIC_ADDRESS)
    };

    assert_eq!(
        hub_refusal(&server, id, |_, _| None),
        server::join::no_proof(TEST_HUB)
    );

    let mut expired = cert.clone();
    expired.issued_at -= 2 * CERTIFICATE_LIFETIME_SECS;
    expired.expires_at -= 2 * CERTIFICATE_LIFETIME_SECS;
    let reason = hub_refusal(&server, id, |nonce, server_key| {
        Some(proof(&expired, key, nonce, server_key))
    });
    assert_eq!(reason, server::join::EXPIRED);

    let other_hub = random_key();
    let reason = hub_refusal(&server, id, |nonce, server_key| {
        Some(proof(cert, &other_hub, nonce, server_key))
    });
    assert_eq!(reason, server::join::UNKNOWN_HUB);

    let reason = hub_refusal(&server, id, |_, server_key| {
        Some(proof(cert, key, &[7; 32], server_key))
    });
    assert_eq!(reason, server::join::WRONG_SIGNATURE);

    let reason = hub_refusal(&server, id, |nonce, _| {
        Some(proof(cert, key, nonce, &[7; 32]))
    });
    assert_eq!(reason, server::join::WRONG_SIGNATURE);

    // a client that announces another id than its account's
    let reason = hub_refusal(&server, id ^ 1, |nonce, server_key| {
        Some(proof(cert, key, nonce, server_key))
    });
    assert_eq!(reason, server::join::WRONG_ID);

    let mut unnamed = cert.clone();
    unnamed.name = " ".into();
    let reason = hub_refusal(&server, id, |nonce, server_key| {
        Some(proof(&unnamed, key, nonce, server_key))
    });
    assert_eq!(reason, server::join::BAD_NAME);

    hub.trust.revoked.write().unwrap().insert(cert.id);
    let reason = hub_refusal(&server, id, |nonce, server_key| {
        Some(proof(cert, key, nonce, server_key))
    });
    assert_eq!(reason, server::join::REVOKED);
}

/// A server that passes the challenge of this one to its own players, and their proofs back,
/// gets proofs made for its own address: they do not open this one.
#[test]
fn a_proof_relayed_from_another_address_is_refused() {
    let hub = TestHub::new();
    let server = TestServer::start_hub(WORLD_NAME, hub.trust.clone());
    let relayed = "relay.test:8000";
    let reason = hub_refusal(&server, hub.player_id(), |nonce, server_key| {
        Some(proof_of(
            &hub.certificate,
            &hub.key,
            &hub.device,
            nonce,
            server_key,
            relayed,
        ))
    });
    assert_eq!(reason, server::join::wrong_address(relayed));
}

#[test]
fn a_proof_of_one_connection_does_not_open_the_next() {
    let hub = TestHub::new();
    let server = TestServer::start_hub(WORLD_NAME, hub.trust.clone());
    let mut first = TestClient::connect(server.addr, hub.player_id());
    let (nonce, server_key, _) = challenge(&mut first);
    let proof = hub.proof(&nonce, &server_key);
    first.transport.disconnect();

    // the id is free once the server saw the first connection go
    let start = Instant::now();
    let reason = loop {
        let mut second = TestClient::connect(server.addr, hub.player_id());
        second.pump();
        let deadline = Instant::now() + Duration::from_secs(1);
        while !second.client.is_connected()
            && !second.client.is_disconnected()
            && Instant::now() < deadline
        {
            second.pump();
        }
        if second.client.is_connected() {
            challenge(&mut second);
            second.send(request_with("Alex", Some(proof.clone())));
            break dismissed(&mut second);
        }
        assert!(
            start.elapsed() < WAIT_TIMEOUT,
            "the second connection was never accepted"
        );
    };
    assert_eq!(reason, server::join::WRONG_SIGNATURE);
}

#[test]
fn a_player_whose_certificate_is_revoked_is_dismissed() {
    let hub = TestHub::new();
    let server = TestServer::start_hub(WORLD_NAME, hub.trust.clone());
    let mut client = TestClient::connect(server.addr, hub.player_id());
    let (nonce, server_key, _) = challenge(&mut client);
    client.send(request_with("Alex", Some(hub.proof(&nonce, &server_key))));
    assert_eq!(accepted_name(&mut client), "Alex");

    // as the thread that reads the list of the hub does
    hub.trust
        .revoked
        .write()
        .unwrap()
        .insert(hub.certificate.id);
    assert_eq!(dismissed(&mut client), server::join::REVOKED);
}

/// The accounts the hub of the tests knows: Bob, whose account is `BOB`. Another name is not
/// one of the hub, but "Unreachable", which the hub fails to answer.
fn hub_profile(_hub: &str, name: &str) -> Result<Option<Profile>, String> {
    match name {
        "Bob" => Ok(Some(Profile {
            id: BOB,
            username: "Bob".into(),
        })),
        "Unreachable" => Err("down".into()),
        "Slow" => {
            // the first question is the slowest
            if SLOW_ASKED.fetch_add(1, std::sync::atomic::Ordering::SeqCst) == 0 {
                thread::sleep(Duration::from_millis(400));
            }
            Ok(Some(Profile {
                id: account(0x510),
                username: "Slow".into(),
            }))
        }
        _ => Ok(None),
    }
}

static SLOW_ASKED: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

const BOB: Uuid = account(0xb0b);

/// The account number `n`: its first bytes, from which the id of its player comes, are not zero.
const fn account(n: u128) -> Uuid {
    Uuid::from_u128(n << 64 | n)
}

impl TestHub {
    /// The certificate of the account `account`, named `name`, for the device of this hub.
    fn certificate_of(&self, account: Uuid, name: &str) -> Certificate {
        Certificate {
            account,
            name: name.into(),
            id: Uuid::from_u128(rand::random()),
            ..self.certificate.clone()
        }
    }

    /// Joins `server` with `certificate`, and returns the client once it is in.
    fn join(&self, server: &TestServer, certificate: &Certificate) -> TestClient {
        let mut client = self.connect(server, certificate);
        accepted_name(&mut client);
        client
    }

    /// Asks to join `server` with `certificate`.
    fn connect(&self, server: &TestServer, certificate: &Certificate) -> TestClient {
        let mut client = TestClient::connect(server.addr, player_id(&certificate.account));
        let (nonce, server_key, _) = challenge(&mut client);
        let proof = proof_of(
            certificate,
            &self.key,
            &self.device,
            &nonce,
            &server_key,
            PUBLIC_ADDRESS,
        );
        client.send(request_with(&certificate.name, Some(proof)));
        client
    }

    /// A server of this hub whose game folder holds the lists `files`.
    fn start_with_files(&self, files: &[(&str, String)]) -> TestServer {
        let tmp = tempfile::tempdir().unwrap();
        for (name, text) in files {
            std::fs::write(tmp.path().join(name), text).unwrap();
        }
        TestServer::start_hub_in(tmp, WORLD_NAME, self.trust.clone())
    }
}

const ALEX: Uuid = account(0xa1e);

fn account_entry(id: Uuid, name: &str) -> String {
    format!(r#"(id: "{id}", name: "{name}")"#)
}

/// Whether the list `text`, as the server writes it, has the entry of `id` named `name`.
fn has_entry(text: &str, id: Uuid, name: &str) -> bool {
    let squashed: String = text.split_whitespace().collect();
    squashed.contains(&format!(r#"(id:"{id}",name:"{name}",)"#))
}

/// Whether the operator `client` may still run `/give`.
fn may_give(client: &mut TestClient) -> bool {
    client.send(chat_message("/give dirt 1"));
    let answer = client.wait_for("the answer to /give", |m| match m {
        ServerToClientMessage::ChatMessage(c) if c.author == "Server" => Some(c.content.clone()),
        _ => None,
    });
    answer == "Gave 1 Dirt"
}

#[test]
fn an_operator_of_a_hub_is_its_account_and_stays_one_when_renamed() {
    let hub = TestHub::new();
    let ops = format!("[{}]", account_entry(ALEX, "Alex"));
    let server = hub.start_with_files(&[(server::OPS_FILE, ops)]);

    let mut renamed = hub.join(&server, &hub.certificate_of(ALEX, "Alexandra"));
    assert!(may_give(&mut renamed));
    // another account that has the name of the operator now is not one
    let mut other = hub.join(&server, &hub.certificate_of(account(2), "Alex"));
    assert!(!may_give(&mut other));
    // the entry has the new name, for whoever reads the file
    let ops = std::fs::read_to_string(server.tmp.path().join(server::OPS_FILE)).unwrap();
    assert!(
        ops.contains("Alexandra") && !ops.contains("\"Alex\""),
        "{ops}"
    );
}

#[test]
fn a_name_in_a_list_of_a_hub_is_rewritten_with_its_account_when_it_joins() {
    let hub = TestHub::new();
    let server = hub.start_with_files(&[
        (server::OPS_FILE, r#"["alex", "carol"]"#.into()),
        (
            server::WHITELIST_FILE,
            r#"(enabled: true, players: ["ALEX"])"#.into(),
        ),
    ]);
    let mut alex = hub.join(&server, &hub.certificate_of(ALEX, "Alex"));
    assert!(may_give(&mut alex));
    let read = |file| std::fs::read_to_string(server.tmp.path().join(file)).unwrap();
    let ops = read(server::OPS_FILE);
    assert!(has_entry(&ops, ALEX, "Alex"), "{ops}");
    // the name of a player that did not join yet stays a name
    assert!(ops.contains("\"carol\""), "{ops}");
    let whitelist = read(server::WHITELIST_FILE);
    assert!(has_entry(&whitelist, ALEX, "Alex"), "{whitelist}");
}

#[test]
fn a_banned_account_is_refused_whatever_its_name() {
    let hub = TestHub::new();
    let banned =
        format!(r#"[(id: "{ALEX}", name: "Alex", reason: Some("griefing")), (name: "Eve")]"#);
    let server = hub.start_with_files(&[(server::BANNED_FILE, banned)]);
    let mut client = hub.connect(&server, &hub.certificate_of(ALEX, "Somebody else"));
    assert_eq!(
        dismissed(&mut client),
        "You are banned from this server: griefing"
    );
    // a name only is banned by its name
    let mut eve = hub.connect(&server, &hub.certificate_of(account(5), "eve"));
    assert_eq!(dismissed(&mut eve), "You are banned from this server.");
    hub.join(&server, &hub.certificate_of(account(6), "Alex"));
}

#[test]
fn the_commands_of_a_hub_find_the_account_of_a_name_online_or_on_the_hub() {
    let hub = TestHub::new();
    let ops = format!("[{}]", account_entry(ALEX, "Alex"));
    let server = hub.start_with_files(&[(server::OPS_FILE, ops)]);
    let mut alex = hub.join(&server, &hub.certificate_of(ALEX, "Alex"));
    let carol = account(0xca401);
    let mut carol_client = hub.join(&server, &hub.certificate_of(carol, "Carol"));

    // Carol is online: the hub, which does not know her, is not asked
    alex.send(chat_message("/op Carol"));
    command_answer(&mut alex, "Made Carol an operator");
    assert!(may_give(&mut carol_client));

    alex.send(chat_message("/op Nobody"));
    command_answer(&mut alex, "The hub does not know a player named Nobody");
    alex.send(chat_message("/ban Unreachable"));
    command_answer(&mut alex, "Could not ask the hub for Unreachable: down");

    // Bob is not online: the hub tells who he is, and the game goes on meanwhile
    alex.send(chat_message("/op Bob"));
    command_answer(&mut alex, "Made Bob an operator");
    let ops = std::fs::read_to_string(server.tmp.path().join(server::OPS_FILE)).unwrap();
    assert!(has_entry(&ops, BOB, "Bob"), "{ops}");
    let mut bob = hub.join(&server, &hub.certificate_of(BOB, "Robert"));
    assert!(may_give(&mut bob));

    // banned by the account the hub gave, whatever he calls himself, and pardoned by it
    alex.send(chat_message("/ban Robert"));
    command_answer(&mut alex, "Banned Robert");
    assert_eq!(dismissed(&mut bob), "You are banned from this server.");
    let mut again = hub.connect(&server, &hub.certificate_of(BOB, "Bobby"));
    assert_eq!(dismissed(&mut again), "You are banned from this server.");
    alex.send(chat_message("/pardon Bob"));
    command_answer(&mut alex, "Pardoned Bob");
    hub.join(&server, &hub.certificate_of(BOB, "Bobby"));

    alex.send(chat_message("/whitelist add Bob"));
    command_answer(&mut alex, "Added Bob to the whitelist");
    let whitelist =
        std::fs::read_to_string(server.tmp.path().join(server::WHITELIST_FILE)).unwrap();
    assert!(has_entry(&whitelist, BOB, "Bob"), "{whitelist}");
}

#[test]
fn a_player_of_a_hub_is_saved_under_its_account_and_finds_its_old_file() {
    let hub = TestHub::new();
    let certificate = hub.certificate_of(ALEX, "Alex");
    let id = player_id(&ALEX);
    let tmp = tempfile::tempdir().unwrap();
    let players_dir = tmp.path().join("saves").join(WORLD_NAME).join("players");
    std::fs::create_dir_all(&players_dir).unwrap();
    // the file that a server of before named after the id of the player
    let old = Player::new(
        id,
        "Alex".into(),
        Vec3::new(0., 80., 0.),
        Transform::default(),
    );
    let old_file = players_dir.join(format!("{id}.ron"));
    std::fs::write(&old_file, ron::ser::to_string(&old).unwrap()).unwrap();
    let server = TestServer::start_hub_in(tmp, WORLD_NAME, hub.trust.clone());

    let mut client = hub.connect(&server, &certificate);
    let spawn = client.wait_for("AuthRegisterResponse", |m| match m {
        ServerToClientMessage::AuthRegisterResponse(r) => r
            .players
            .iter()
            .find(|player| player.id == id)
            .map(|player| player.data.position),
        _ => None,
    });
    assert_eq!(spawn, Vec3::new(0., 80., 0.));
    let new_file = players_dir.join(format!("{ALEX}.ron"));
    assert!(new_file.is_file() && !old_file.exists());

    // leaving saves it under its account only
    std::fs::remove_file(&new_file).unwrap();
    client.transport.disconnect();
    let start = Instant::now();
    while !new_file.is_file() {
        assert!(start.elapsed() < WAIT_TIMEOUT, "the player was not saved");
        thread::sleep(PUMP_INTERVAL);
    }
    assert!(!old_file.exists());
}

#[test]
fn a_player_banned_through_the_hub_is_dismissed_whatever_its_name_online() {
    let hub = TestHub::new();
    let ops = format!("[{}]", account_entry(ALEX, "Alex"));
    let server = hub.start_with_files(&[(server::OPS_FILE, ops)]);
    let mut alex = hub.join(&server, &hub.certificate_of(ALEX, "Alex"));
    // the name of its certificate is not the one the hub gives it now
    let mut bob = hub.join(&server, &hub.certificate_of(BOB, "Robert"));
    alex.send(chat_message("/ban Bob"));
    command_answer(&mut alex, "Asking the hub for Bob...");
    command_answer(&mut alex, "Banned Bob");
    assert_eq!(dismissed(&mut bob), "You are banned from this server.");
}

#[test]
fn the_commands_of_a_player_run_in_the_order_it_typed_them_whatever_the_hub_speed() {
    let hub = TestHub::new();
    let ops = format!("[{}]", account_entry(ALEX, "Alex"));
    let server = hub.start_with_files(&[(server::OPS_FILE, ops)]);
    let mut alex = hub.join(&server, &hub.certificate_of(ALEX, "Alex"));
    alex.send(chat_message("/ban Slow"));
    alex.send(chat_message("/pardon Slow"));
    command_answer(&mut alex, "Banned Slow");
    command_answer(&mut alex, "Pardoned Slow");
    let banned = std::fs::read_to_string(server.tmp.path().join(server::BANNED_FILE)).unwrap();
    assert!(!banned.contains("Slow"), "{banned}");
}

const OWNER: Uuid = account(0x0123);

/// A hosted world whose members are Alex (a member) and Bob (an operator), owned by `OWNER`.
fn realm_config(members: &[(Uuid, &str, MemberRole)]) -> RealmConfig {
    RealmConfig {
        members: members
            .iter()
            .map(|&(id, username, role)| RealmMember {
                id,
                username: username.into(),
                role,
            })
            .collect(),
        owner: AccountSummary {
            id: OWNER,
            username: "Olivia".into(),
        },
    }
}

fn alex_and_bob() -> RealmConfig {
    realm_config(&[
        (ALEX, "Alex", MemberRole::Member),
        (BOB, "Bob", MemberRole::Operator),
    ])
}

#[test]
fn a_hosted_world_lets_in_its_members_and_its_owner_and_its_operators_are_the_hubs() {
    let hub = TestHub::new();
    let (server, _hub_events) = TestServer::start_hosted(hub.trust.clone(), alex_and_bob());

    let mut alex = hub.join(&server, &hub.certificate_of(ALEX, "Alex"));
    assert!(!may_give(&mut alex));
    let mut bob = hub.join(&server, &hub.certificate_of(BOB, "Bob"));
    assert!(may_give(&mut bob));
    // the owner need not be a member
    let mut owner = hub.join(&server, &hub.certificate_of(OWNER, "Olivia"));
    assert!(may_give(&mut owner));

    let mut stranger = hub.connect(&server, &hub.certificate_of(account(7), "Eve"));
    assert_eq!(
        dismissed(&mut stranger),
        "You are not on the whitelist of this server."
    );
}

#[test]
fn a_hosted_world_leaves_its_members_to_the_hub() {
    let hub = TestHub::new();
    let (server, _hub_events) = TestServer::start_hosted(hub.trust.clone(), alex_and_bob());
    let mut bob = hub.join(&server, &hub.certificate_of(BOB, "Bob"));
    for command in [
        "/op Alex",
        "/deop Bob",
        "/ban Alex",
        "/pardon Alex",
        "/whitelist add Eve",
        "/whitelist list",
    ] {
        bob.send(chat_message(command));
        command_answer(
            &mut bob,
            "Manage the members of a hosted world from the game's Hosted worlds tab.",
        );
    }
    // it is still the hub's list, and nothing is written
    assert!(may_give(&mut bob));
    for file in [
        server::OPS_FILE,
        server::BANNED_FILE,
        server::WHITELIST_FILE,
    ] {
        assert!(!server.tmp.path().join(file).exists(), "{file} was written");
    }
}

#[test]
fn a_member_removed_from_a_hosted_world_is_dismissed_and_a_demoted_one_is_not_an_operator() {
    let hub = TestHub::new();
    let (server, hub_events) = TestServer::start_hosted(hub.trust.clone(), alex_and_bob());
    let mut alex = hub.join(&server, &hub.certificate_of(ALEX, "Alex"));
    let mut bob = hub.join(&server, &hub.certificate_of(BOB, "Bob"));
    assert!(may_give(&mut bob));

    hub_events
        .send(RealmEvent::Config(realm_config(&[
            (ALEX, "Alex", MemberRole::Operator),
            (BOB, "Bob", MemberRole::Member),
        ])))
        .unwrap();
    // the hub promoted Alex and demoted Bob, who is still a member
    let start = Instant::now();
    while !may_give(&mut alex) {
        assert!(start.elapsed() < WAIT_TIMEOUT, "Alex was not promoted");
    }
    assert!(!may_give(&mut bob));

    hub_events
        .send(RealmEvent::Config(realm_config(&[(
            ALEX,
            "Alex",
            MemberRole::Operator,
        )])))
        .unwrap();
    assert_eq!(dismissed(&mut bob), "You were removed from this world.");
    // and cannot come back
    let mut again = hub.connect(&server, &hub.certificate_of(BOB, "Bob"));
    assert_eq!(
        dismissed(&mut again),
        "You are not on the whitelist of this server."
    );
    assert!(may_give(&mut alex));
}

#[test]
fn a_hosted_world_stops_when_its_hub_no_longer_knows_it() {
    let hub = TestHub::new();
    let (mut server, hub_events) = TestServer::start_hosted(hub.trust.clone(), alex_and_bob());
    let _bob = hub.join(&server, &hub.certificate_of(BOB, "Bob"));
    hub_events.send(RealmEvent::Gone).unwrap();
    server.join_after_exit();
    // saved on the way out
    assert!(server.world_dir().join("players").is_dir());
}
