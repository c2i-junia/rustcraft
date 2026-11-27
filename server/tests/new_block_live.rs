//! A placeable block defined only in a content file, with no Rust, added while the server runs:
//! `/reload` reads it, the player is sent it with its item, is given the item and places the
//! block. It installs content other than the game's, so it is alone in its file and its process.

use std::net::{IpAddr, Ipv4Addr, UdpSocket};
use std::path::Path;
use std::thread;
use std::time::{Duration, Instant, SystemTime};

use bevy::math::{IVec3, Vec3};
use bevy::transform::components::Transform;
use bevy_renet::netcode::{ClientAuthentication, NetcodeClientTransport};
use bevy_renet::RenetClient;
use shared::content::Content;
use shared::messages::{
    AuthRegisterRequest, ChatMessageRequest, ClientToServerMessage, NetworkAction,
    PlayerFrameInput, ServerToClientMessage,
};
use shared::world::{raycast, ChunkMap, FaceDirectionExt, ItemType};
use shared::{
    game_message_to_payload, get_shared_renet_config, payload_to_game_message,
    ChannelResolvableExt, ContentSource, GameFolderPaths, GameServerConfig, PROTOCOL_ID,
    STC_AUTH_CHANNEL, STC_CHUNK_DATA_CHANNEL, STC_STANDARD_CHANNEL,
};

const WAIT_TIMEOUT: Duration = Duration::from_secs(20);

struct TestClient {
    client: RenetClient,
    transport: NetcodeClientTransport,
    inbox: Vec<ServerToClientMessage>,
    last_pump: Instant,
    /// The chunks the server sent, kept up to date by its updates.
    world: ChunkMap,
}

impl TestClient {
    fn connect(server_addr: std::net::SocketAddr) -> Self {
        let authentication = ClientAuthentication::Unsecure {
            server_addr,
            client_id: 7,
            user_data: None,
            protocol_id: PROTOCOL_ID,
        };
        let socket = UdpSocket::bind("127.0.0.1:0").unwrap();
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap();
        Self {
            client: RenetClient::new(get_shared_renet_config()),
            transport: NetcodeClientTransport::new(now, authentication, socket).unwrap(),
            inbox: Vec::new(),
            last_pump: Instant::now(),
            world: ChunkMap::default(),
        }
    }

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
            let _ = self.transport.send_packets(&mut self.client);
        }
        thread::sleep(Duration::from_millis(5));
    }

    fn apply_to_world(&mut self, message: &ServerToClientMessage) {
        match message {
            ServerToClientMessage::WorldUpdate(update) => {
                for (pos, chunk) in &update.new_map {
                    self.world.insert_chunk(*pos, chunk.clone());
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

    fn send(&mut self, message: ClientToServerMessage) {
        let channel = message.get_channel_id();
        self.client
            .send_message(channel, game_message_to_payload(message));
    }

    /// Pumps until a message accepted by `pick` arrives, and removes it from the inbox.
    fn wait_for<T>(
        &mut self,
        what: &str,
        mut pick: impl FnMut(&ServerToClientMessage) -> Option<T>,
    ) -> T {
        let start = Instant::now();
        loop {
            if let Some(index) = self.inbox.iter().position(|m| pick(m).is_some()) {
                return pick(&self.inbox.remove(index)).unwrap();
            }
            assert!(
                start.elapsed() < WAIT_TIMEOUT,
                "timed out waiting for {what}"
            );
            self.pump();
        }
    }

    fn wait_for_content(&mut self) -> Content {
        self.wait_for("the content", |m| match m {
            ServerToClientMessage::Content(content) => Some(content.clone()),
            _ => None,
        })
    }

    /// Connects, then joins as `username`.
    fn join(server_addr: std::net::SocketAddr, username: &str) -> Self {
        let mut client = Self::connect(server_addr);
        let start = Instant::now();
        while !client.client.is_connected() {
            assert!(start.elapsed() < WAIT_TIMEOUT, "the client did not connect");
            client.pump();
        }
        client.send(ClientToServerMessage::AuthRegisterRequest(
            AuthRegisterRequest {
                username: username.to_string(),
                render_distance: 2,
                skin: Default::default(),
                proof: None,
            },
        ));
        client
    }

    /// Leaves, which stops the solo server once it saved the world.
    fn exit(mut self, server: thread::JoinHandle<()>) {
        self.send(ClientToServerMessage::Exit);
        let _ = self.transport.send_packets(&mut self.client);
        let start = Instant::now();
        while !server.is_finished() {
            assert!(start.elapsed() < WAIT_TIMEOUT, "the server did not stop");
            thread::sleep(Duration::from_millis(5));
        }
        server.join().unwrap();
    }

    /// The next chat message of the server, the answers to the commands.
    fn wait_for_chat(&mut self) -> String {
        self.wait_for("a chat message of the server", |m| match m {
            ServerToClientMessage::ChatMessage(message) if message.author == "Server" => {
                Some(message.content.clone())
            }
            _ => None,
        })
    }

    fn chat(&mut self, line: &str) {
        self.send(ClientToServerMessage::ChatMessage(ChatMessageRequest {
            content: line.to_string(),
        }));
    }
}

/// The content files of the game, copied into `folder`.
fn write_content_files(folder: &Path) {
    std::fs::create_dir_all(folder).unwrap();
    let game = Path::new(env!("CARGO_MANIFEST_DIR")).join("../data/content/rustcraft");
    for entry in std::fs::read_dir(game).unwrap() {
        let path = entry.unwrap().path();
        std::fs::copy(&path, folder.join(path.file_name().unwrap())).unwrap();
    }
}

/// Starts a solo server of the world `amber` in `paths`, which reads its content files.
fn start_server(paths: &GameFolderPaths) -> (std::net::SocketAddr, thread::JoinHandle<()>) {
    let socket =
        server::acquire_local_ephemeral_udp_socket(IpAddr::V4(Ipv4Addr::LOCALHOST)).unwrap();
    let addr = socket.local_addr().unwrap();
    let config = GameServerConfig {
        world_name: "amber".to_string(),
        is_solo: true,
        saves: None,
        max_render_distance: 2,
        max_players: shared::MAX_PLAYERS,
        new_world: Default::default(),
        auth: Default::default(),
        content: ContentSource::Files,
    };
    let paths = paths.clone();
    (
        addr,
        thread::spawn(move || server::init(socket, config, paths).unwrap()),
    )
}

#[test]
fn a_block_added_to_the_files_of_a_running_server_is_given_and_placed_after_a_reload() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = GameFolderPaths {
        game_folder_path: tmp.path().to_path_buf(),
        assets_folder_path: tmp.path().join("assets"),
    };
    let folder = paths.assets_folder_path.join("content").join("rustcraft");
    write_content_files(&folder);
    let (addr, server) = start_server(&paths);
    let mut client = TestClient::join(addr, "builder");
    let joined = client.wait_for_content();
    assert_eq!(joined.block_named("amber"), None);

    // 1. A file of one line, and /reload: the block and its item reach the player
    std::fs::write(
        folder.join("amber.ron"),
        r#"(blocks: {"amber": (break: 4)})"#,
    )
    .unwrap();
    client.chat("/reload");
    assert_eq!(client.wait_for_chat(), "Content files reloaded");
    assert_eq!(
        client.wait_for_chat(),
        "block \"amber\" in amber.ron is new"
    );
    let reloaded = client.wait_for_content();
    let block = reloaded.block_named("amber").expect("the new block");
    let item = reloaded.item_named("amber").expect("the item of the block");
    assert_eq!(reloaded.item(item).default_type, ItemType::Block(block));
    assert_eq!(reloaded.block(block).drops, vec![(1, item, 1)]);
    assert_eq!(
        reloaded.block_idents[block.index()],
        "Amber",
        "its texture: Amber.png"
    );

    // 2. Given, then placed two blocks ahead on the ground
    client.chat("/give amber 3");
    assert_eq!(client.wait_for_chat(), "Gave 3 Amber");
    let last_time_ms = 40;
    let landing = (1..=last_time_ms)
        .map(|time_ms| PlayerFrameInput {
            time_ms,
            delta_ms: 100,
            ..Default::default()
        })
        .collect();
    client.send(ClientToServerMessage::PlayerInputs(landing));
    let position = client.wait_for("the player on the ground", |m| match m {
        ServerToClientMessage::PlayerUpdate(update) if update.last_ack_time == last_time_ms => {
            Some(update.movement.position)
        }
        _ => None,
    });
    let camera = Transform::from_translation(position)
        .looking_at(position + Vec3::new(0., -1.6, -2.), Vec3::Y);
    let start = Instant::now();
    let ground = loop {
        if let Some(hit) = raycast::raycast(&client.world, &camera, &position, Default::default()) {
            break hit;
        }
        assert!(start.elapsed() < WAIT_TIMEOUT, "the ground never arrived");
        client.pump();
    };
    let placed: IVec3 = ground.position + ground.face.to_ivec3();
    client.send(ClientToServerMessage::PlayerInputs(vec![
        PlayerFrameInput {
            time_ms: last_time_ms + 1,
            delta_ms: 16,
            inputs: [NetworkAction::RightClick].into(),
            camera,
            hotbar_slot: 0,
            ..Default::default()
        },
    ]));
    let start = Instant::now();
    while client.world.get_block(placed).map(|cell| cell.id) != Some(block) {
        assert!(
            start.elapsed() < WAIT_TIMEOUT,
            "the amber was not placed at {placed}"
        );
        client.pump();
    }
    client.exit(server);
}
