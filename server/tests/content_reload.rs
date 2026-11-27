//! The one test that installs content other than the game's: a real server reads its content
//! files, sends them to its player at join, and reads them again on `/reload`. It is
//! alone in its file, so its process (and the content of that process) is its own.

use std::net::{IpAddr, Ipv4Addr, UdpSocket};
use std::path::Path;
use std::thread;
use std::time::{Duration, Instant, SystemTime};

use bevy_renet::netcode::{ClientAuthentication, NetcodeClientTransport};
use bevy_renet::RenetClient;
use shared::content::{content, Content};
use shared::messages::{
    AuthRegisterRequest, ChatMessageRequest, ClientToServerMessage, ServerToClientMessage,
};
use shared::world::{BlockId, ItemId};
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

/// Writes the content files of the game into `folder`, with `from` replaced by `to` in the
/// entry of `name` for each `(name, from, to)` of `edits`.
fn write_game_files(folder: &Path, edits: &[(&str, &str, &str)]) {
    let game = Path::new(env!("CARGO_MANIFEST_DIR")).join("../data/content/rustcraft");
    let mut done = 0;
    for entry in std::fs::read_dir(game).unwrap() {
        let path = entry.unwrap().path();
        let text = std::fs::read_to_string(&path).unwrap();
        let mut lines: Vec<String> = text.lines().map(str::to_string).collect();
        for (name, from, to) in edits {
            let key = format!("\"{name}\": ");
            if let Some(line) = lines.iter_mut().find(|l| l.trim_start().starts_with(&key)) {
                assert!(line.contains(from), "{line}");
                *line = line.replace(from, to);
                done += 1;
            }
        }
        let name = path.file_name().unwrap();
        std::fs::write(folder.join(name), lines.join("\n") + "\n").unwrap();
    }
    assert_eq!(done, edits.len(), "an entry to edit is missing");
}

fn torch_light(content: &Content) -> u8 {
    content.block(BlockId::Torch).light_emission
}

#[test]
fn a_server_sends_its_content_files_at_join_and_again_on_reload() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = GameFolderPaths {
        game_folder_path: tmp.path().to_path_buf(),
        assets_folder_path: tmp.path().join("assets"),
    };
    let folder = paths.assets_folder_path.join("content").join("rustcraft");
    std::fs::create_dir_all(&folder).unwrap();
    let write = |name: &str, text: &str| std::fs::write(folder.join(name), text).unwrap();
    write_game_files(
        &folder,
        &[
            ("torch", "light: 14", "light: 9"),
            ("apple", "(food_value", "(stack: 16, food_value"),
        ],
    );
    let builtin = content().clone();
    assert_eq!(torch_light(&builtin), 14, "the test changes the torch");

    let socket =
        server::acquire_local_ephemeral_udp_socket(IpAddr::V4(Ipv4Addr::LOCALHOST)).unwrap();
    let addr = socket.local_addr().unwrap();
    let config = GameServerConfig {
        world_name: "content".to_string(),
        is_solo: true,
        saves: None,
        max_render_distance: 2,
        max_players: shared::MAX_PLAYERS,
        new_world: Default::default(),
        auth: Default::default(),
        // the watcher is never started in the tests: `/reload` reads the files
        content: ContentSource::Files,
    };
    let server = thread::spawn(move || server::init(socket, config, paths).unwrap());

    // 1. At join: the content of the files
    let mut client = TestClient::connect(addr);
    let start = Instant::now();
    while !client.client.is_connected() {
        assert!(start.elapsed() < WAIT_TIMEOUT, "the client did not connect");
        client.pump();
    }
    client.send(ClientToServerMessage::AuthRegisterRequest(
        AuthRegisterRequest {
            username: "tuner".to_string(),
            render_distance: 2,
            skin: Default::default(),
            proof: None,
        },
    ));
    let joined = client.wait_for_content();
    assert_eq!(torch_light(&joined), 9);
    assert_eq!(joined.item(ItemId::Apple).max_stack, 16);
    // what the getters of the process read, the server's
    assert_eq!(BlockId::Torch.light_emission(), 9);
    assert_eq!(ItemId::Apple.get_max_stack(), 16);
    assert!(joined.generation > 0);

    // 2. A bad file: nothing changes, and the player is told the file and the block
    write("broken.ron", r#"(blocks: {"stone": (light: 99)})"#);
    client.chat("/reload");
    let refusal = client.wait_for_chat();
    assert!(
        refusal.contains("broken.ron") && refusal.contains("\"stone\""),
        "{refusal}"
    );
    assert_eq!(BlockId::Torch.light_emission(), 9);
    assert_eq!(BlockId::Stone.light_emission(), 0);

    // 3. Fixed and changed: installed and sent again
    std::fs::remove_file(folder.join("broken.ron")).unwrap();
    write_game_files(&folder, &[("torch", "light: 14", "light: 5")]);
    client.chat("/reload");
    assert_eq!(client.wait_for_chat(), "Content files reloaded");
    let reloaded = client.wait_for_content();
    assert_eq!(torch_light(&reloaded), 5);
    assert_eq!(
        reloaded.item(ItemId::Apple).max_stack,
        builtin.item(ItemId::Apple).max_stack
    );
    assert!(reloaded.generation > joined.generation);
    assert_eq!(BlockId::Torch.light_emission(), 5);
    assert!(content().same_properties(&reloaded));

    // 4. Unchanged files: nothing to send
    client.chat("/reload");
    assert_eq!(client.wait_for_chat(), "The content files changed nothing");

    client.send(ClientToServerMessage::Exit);
    let _ = client.transport.send_packets(&mut client.client);
    let start = Instant::now();
    while !server.is_finished() {
        assert!(start.elapsed() < WAIT_TIMEOUT, "the server did not stop");
        thread::sleep(Duration::from_millis(5));
    }
    server.join().unwrap();
}
