//! A block that only a content file defines, with no Rust: a server reads it from its content
//! files, sends it to its player, a mod command places it, the world saves it under its name,
//! and loads it back. It installs content other than the game's, so it is alone in its file
//! and its process.

use std::net::{IpAddr, Ipv4Addr, UdpSocket};
use std::path::Path;
use std::thread;
use std::time::{Duration, Instant, SystemTime};

use bevy::math::IVec3;
use bevy_renet::netcode::{ClientAuthentication, NetcodeClientTransport};
use bevy_renet::RenetClient;
use shared::content::Content;
use shared::messages::{
    AuthRegisterRequest, ChatMessageRequest, ClientToServerMessage, ServerToClientMessage,
};
use shared::world::{global_block_to_chunk_pos, to_local_pos, BlockId, LEGACY_BLOCK_NAMES};
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
                    self.inbox.push(payload_to_game_message(&payload).unwrap());
                }
            }
            let _ = self.transport.send_packets(&mut self.client);
        }
        thread::sleep(Duration::from_millis(5));
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

/// The content files of the game, copied into `folder`, and `moss.ron`, which defines the
/// block `glow_moss`.
fn write_content_files(folder: &Path) {
    std::fs::create_dir_all(folder).unwrap();
    let game = Path::new(env!("CARGO_MANIFEST_DIR")).join("../data/content/rustcraft");
    for entry in std::fs::read_dir(game).unwrap() {
        let path = entry.unwrap().path();
        std::fs::copy(&path, folder.join(path.file_name().unwrap())).unwrap();
    }
    std::fs::write(
        folder.join("moss.ron"),
        r#"(blocks: {"glow_moss": (light: 7, break: 3, tools: [Shovel])})"#,
    )
    .unwrap();
}

/// A mod whose command `/moss` puts a glow moss two blocks east of its author, and answers where.
const MOSS_MOD: &str = r#"
    rustcraft.register_command("moss", function(ctx, args)
        local p = rustcraft.player_position(ctx.player)
        rustcraft.set_block({ x = p.x + 2, y = p.y, z = p.z }, "glow_moss")
        return (p.x + 2) .. " " .. p.y .. " " .. p.z
    end)
"#;

/// Starts a solo server of the world `moss` in `paths`, which reads its content files.
fn start_server(paths: &GameFolderPaths) -> (std::net::SocketAddr, thread::JoinHandle<()>) {
    let socket =
        server::acquire_local_ephemeral_udp_socket(IpAddr::V4(Ipv4Addr::LOCALHOST)).unwrap();
    let addr = socket.local_addr().unwrap();
    let config = GameServerConfig {
        world_name: "moss".to_string(),
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
fn a_block_only_a_content_file_defines_is_placed_saved_and_loaded_back() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = GameFolderPaths {
        game_folder_path: tmp.path().to_path_buf(),
        assets_folder_path: tmp.path().join("assets"),
    };
    write_content_files(&paths.assets_folder_path.join("content").join("rustcraft"));
    let mod_dir = tmp.path().join("mods").join("moss");
    std::fs::create_dir_all(&mod_dir).unwrap();
    std::fs::write(mod_dir.join("init.luau"), MOSS_MOD).unwrap();

    // 1. The server numbers it after the blocks of the engine, and sends it at join
    let (addr, server) = start_server(&paths);
    let mut client = TestClient::join(addr, "gardener");
    let joined = client.wait_for_content();
    let moss = BlockId::from_index(BlockId::ENGINE.len() as u16);
    assert_eq!(joined.block_named("glow_moss"), Some(moss));
    assert_eq!(joined.block(moss).light_emission, 7);
    // the process is the server's: its getters, names and formats know the block
    assert_eq!(BlockId::from_name("glow_moss"), Some(moss));
    assert_eq!(moss.light_emission(), 7);
    assert_eq!(format!("{moss:?}"), "GlowMoss");
    assert_eq!(ron::ser::to_string(&moss).unwrap(), "GlowMoss");
    assert_eq!(ron::de::from_str::<BlockId>("GlowMoss").unwrap(), moss);

    // 2. A command of a mod places it, and the world saves it under its name
    client.chat("/moss");
    let answer = client.wait_for_chat();
    let at: Vec<i32> = answer
        .split_whitespace()
        .map(|word| word.parse().unwrap())
        .collect();
    let at = IVec3::new(at[0], at[1], at[2]);
    client.exit(server);

    let world_dir = tmp.path().join("saves").join("moss");
    let meta: server::WorldMeta =
        ron::de::from_str(&std::fs::read_to_string(world_dir.join("world.ron")).unwrap()).unwrap();
    let names = meta.block_names.expect("the world lists its blocks");
    assert_eq!(names[..LEGACY_BLOCK_NAMES.len()], LEGACY_BLOCK_NAMES);
    assert_eq!(names[LEGACY_BLOCK_NAMES.len()..], ["glow_moss"]);
    let chunk_pos = global_block_to_chunk_pos(&at);
    let saved = server::read_chunk(&world_dir.join("chunks"), chunk_pos).unwrap();
    let saved = saved.get(to_local_pos(&at)).expect("the moss is saved").id;
    assert_eq!(names[saved.index()], "glow_moss");

    // 3. Loaded back: the player is sent the moss where it was put
    let (addr, server) = start_server(&paths);
    let mut client = TestClient::join(addr, "gardener");
    let content: Content = client.wait_for_content();
    assert_eq!(content.block_named("glow_moss"), Some(moss));
    let chunk = client.wait_for("the chunk of the moss", |m| match m {
        ServerToClientMessage::WorldUpdate(update) => update.new_map.get(&chunk_pos).cloned(),
        _ => None,
    });
    assert_eq!(
        chunk.get(to_local_pos(&at)).map(|block| block.id),
        Some(moss)
    );
    client.exit(server);
}
