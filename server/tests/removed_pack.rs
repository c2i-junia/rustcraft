//! A world that holds a block and its item only a content file defines, opened without that file,
//! as when a pack is removed: its `world.ron` and its player files still read, the block and
//! the item are placeholders under their names, they are saved back under them, and they are
//! whole again once the file is back. It installs content other than the game's, so it is alone
//! in its file and its process.

use std::net::{IpAddr, Ipv4Addr, UdpSocket};
use std::path::Path;
use std::thread;
use std::time::{Duration, Instant, SystemTime};

use bevy_renet::netcode::{ClientAuthentication, NetcodeClientTransport};
use bevy_renet::RenetClient;
use shared::content::{Content, ItemProps};
use shared::messages::{
    AuthRegisterRequest, ChatMessageRequest, ClientToServerMessage, ServerToClientMessage,
};
use shared::world::{BlockId, ItemId, ItemType};
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
/// block `glow_moss`, whose item needs no entry.
fn write_content_files(folder: &Path) {
    std::fs::create_dir_all(folder).unwrap();
    let game = Path::new(env!("CARGO_MANIFEST_DIR")).join("../data/content/rustcraft");
    for entry in std::fs::read_dir(game).unwrap() {
        let path = entry.unwrap().path();
        std::fs::copy(&path, folder.join(path.file_name().unwrap())).unwrap();
    }
    std::fs::write(
        folder.join("moss.ron"),
        r#"(blocks: {"glow_moss": (light: 7, break: 3)})"#,
    )
    .unwrap();
}

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

/// A stack of glow moss lying on the ground, as `world.ron` writes it.
const DROPPED_MOSS: &str = r#"(
            stack: (item_id: GlowMoss, item_type: Block(GlowMoss), nb: 2),
            position: (0.5, 90, 0.5),
            velocity: (0, 0, 0),
            on_ground: false,
            age_ticks: 0,
            passes: 0,
            experience: 0,
        )"#;

/// Where the test below keeps the game folder for its first part, `with_the_file`.
const FOLDER_VAR: &str = "RUSTCRAFT_REMOVED_PACK_FOLDER";

/// The paths of a game in `folder`, which reads its content files from its assets.
fn paths_in(folder: &Path) -> GameFolderPaths {
    GameFolderPaths {
        game_folder_path: folder.to_path_buf(),
        assets_folder_path: folder.join("assets"),
    }
}

/// The first part of the test below, in a process of its own, whose content has the moss: the
/// player is given moss, then the world is saved.
#[test]
#[ignore = "the first part of the test below, which runs it in a process of its own"]
fn with_the_file() {
    let Ok(folder) = std::env::var(FOLDER_VAR) else {
        return;
    };
    let paths = paths_in(Path::new(&folder));
    let (addr, server) = start_server(&paths);
    let mut client = TestClient::join(addr, "gardener");
    let joined = client.wait_for_content();
    assert!(
        joined.item_named("glow_moss").is_some(),
        "the item of the block"
    );
    client.chat("/give glow_moss 5");
    assert_eq!(client.wait_for_chat(), "Gave 5 GlowMoss");
    client.exit(server);
}

#[test]
fn a_world_opened_without_the_file_of_its_block_and_item_reads_and_keeps_them() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = paths_in(tmp.path());
    let folder = paths.assets_folder_path.join("content").join("rustcraft");
    write_content_files(&folder);
    let world_dir = tmp.path().join("saves").join("moss");
    let world_file = world_dir.join("world.ron");
    let players_dir = world_dir.join("players");

    // 1. With the file, in another process: the player is given moss
    let status = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "with_the_file", "--ignored", "--quiet"])
        .env(FOLDER_VAR, tmp.path())
        .status()
        .unwrap();
    assert!(status.success(), "the first part failed");
    // and a stack of it lies on the ground
    let text = std::fs::read_to_string(&world_file).unwrap();
    std::fs::write(
        &world_file,
        text.replacen(
            "dropped_items: [",
            &format!("dropped_items: [{DROPPED_MOSS},"),
            1,
        ),
    )
    .unwrap();
    let player_files = || -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(&players_dir)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    };
    let player_text = |name: &str| std::fs::read_to_string(players_dir.join(name)).unwrap();
    let files = player_files();
    assert_eq!(files.len(), 1, "{files:?}");
    assert!(player_text(&files[0]).contains("GlowMoss"));

    // 2. Without the file: everything reads, the moss is a placeholder under its name
    std::fs::remove_file(folder.join("moss.ron")).unwrap();
    let (addr, server) = start_server(&paths);
    let mut client = TestClient::join(addr, "gardener");
    let content: Content = client.wait_for_content();
    let item = content.item_named("glow_moss").expect("kept in its place");
    assert_eq!(*content.item(item), ItemProps::PLACEHOLDER);
    let block = content.block_named("glow_moss").expect("kept in its place");
    assert_eq!(
        content.block(block).look,
        content.block(BlockId::Debug).look,
        "it looks like the debug block"
    );
    client.exit(server);
    assert_eq!(player_files(), files, "the player file is not set aside");
    let player = player_text(&files[0]);
    assert!(
        player.contains("GlowMoss"),
        "the moss is still held: {player}"
    );
    let text = std::fs::read_to_string(&world_file).unwrap();
    let meta: server::WorldMeta = ron::de::from_str(&text).unwrap();
    let lying: Vec<ItemId> = meta
        .dropped_items
        .iter()
        .map(|dropped| dropped.stack.item_id)
        .collect();
    assert!(
        lying.contains(&item),
        "the moss still lies on the ground: {text}"
    );

    // 3. The file back: the moss is whole again
    write_content_files(&folder);
    let (addr, server) = start_server(&paths);
    let mut client = TestClient::join(addr, "gardener");
    let content: Content = client.wait_for_content();
    assert_eq!(
        content.item_named("glow_moss"),
        Some(item),
        "under the same number"
    );
    assert_eq!(content.item(item).default_type, ItemType::Block(block));
    assert_eq!(content.block(block).light_emission, 7);
    client.exit(server);
    assert!(player_text(&files[0]).contains("GlowMoss"));
}
