//! A world that holds a block its content no longer defines, opened without the file that
//! defined it: the block is kept as a placeholder, and the world saves it back under its name.
//! It installs content other than the game's, so it is alone in its file and its process.

use std::net::{IpAddr, Ipv4Addr, UdpSocket};
use std::thread;
use std::time::{Duration, Instant, SystemTime};

use bevy::math::IVec3;
use bevy_renet::netcode::{ClientAuthentication, NetcodeClientTransport};
use bevy_renet::RenetClient;
use shared::content::{content, BlockProps, Content};
use shared::messages::{
    AuthRegisterRequest, ChatMessageRequest, ClientToServerMessage, ServerToClientMessage,
};
use shared::world::{
    global_block_to_chunk_pos, to_local_pos, BlockData, BlockDirection, BlockId, LEGACY_BLOCK_NAMES,
};
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

/// A mod: `/where` answers the block its author stands in, `/put x y z name` puts a block.
const BUILDER_MOD: &str = r#"
    rustcraft.register_command("where", function(ctx, args)
        local p = rustcraft.player_position(ctx.player)
        return p.x .. " " .. p.y .. " " .. p.z
    end)
    rustcraft.register_command("put", function(ctx, args)
        rustcraft.set_block({ x = tonumber(args[1]), y = tonumber(args[2]), z = tonumber(args[3]) }, args[4])
        return "put"
    end)
"#;

/// Starts a solo server of the world `kept` in `paths`, on the content files of the game.
fn start_server(paths: &GameFolderPaths) -> (std::net::SocketAddr, thread::JoinHandle<()>) {
    let socket =
        server::acquire_local_ephemeral_udp_socket(IpAddr::V4(Ipv4Addr::LOCALHOST)).unwrap();
    let addr = socket.local_addr().unwrap();
    let config = GameServerConfig {
        world_name: "kept".to_string(),
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

fn read_meta(world_dir: &std::path::Path) -> server::WorldMeta {
    ron::de::from_str(&std::fs::read_to_string(world_dir.join("world.ron")).unwrap()).unwrap()
}

#[test]
fn a_block_whose_file_is_gone_is_kept_under_its_name() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = GameFolderPaths {
        game_folder_path: tmp.path().to_path_buf(),
        assets_folder_path: tmp.path().join("assets"),
    };
    let mod_dir = tmp.path().join("mods").join("builder");
    std::fs::create_dir_all(&mod_dir).unwrap();
    std::fs::write(mod_dir.join("init.luau"), BUILDER_MOD).unwrap();
    assert_eq!(
        BlockId::from_name("glow_moss"),
        None,
        "the game has no glow moss"
    );

    // 1. A world is made and saved, around where its player stands
    let (addr, server) = start_server(&paths);
    let mut client = TestClient::join(addr, "keeper");
    client.wait_for_content();
    client.chat("/where");
    let at: Vec<i32> = client
        .wait_for_chat()
        .split_whitespace()
        .map(|word| word.parse().unwrap())
        .collect();
    client.exit(server);
    // the ground under the player, and another cell of its chunk
    let moss_at = IVec3::new(at[0], at[1], at[2]) - IVec3::Y;
    let chunk_pos = global_block_to_chunk_pos(&moss_at);
    let local = to_local_pos(&moss_at);
    let stone_at = moss_at - local + IVec3::new((local.x + 1) % 16, local.y, local.z);

    // 2. As saved by a game that had a glow moss there, from a content file now gone
    let world_dir = tmp.path().join("saves").join("kept");
    let mut meta = read_meta(&world_dir);
    let mut names = meta.block_names.take().expect("version 6 lists the blocks");
    assert_eq!(names, LEGACY_BLOCK_NAMES);
    names.push("glow_moss".to_string());
    let saved_moss = BlockId::from_index((names.len() - 1) as u16);
    meta.block_names = Some(names);
    server::write_meta(&world_dir, &meta).unwrap();
    let chunks_dir = world_dir.join("chunks");
    let mut chunk = server::read_chunk(&chunks_dir, chunk_pos).expect("the ground is saved");
    chunk.set(local, BlockData::new(saved_moss, BlockDirection::Front));
    server::write_chunk_in_region(&chunks_dir, chunk_pos, &chunk).unwrap();

    // 3. Opened again: the moss is a placeholder of its name, and the player is sent it
    let (addr, server) = start_server(&paths);
    let mut client = TestClient::join(addr, "keeper");
    let joined: Content = client.wait_for_content();
    let moss = BlockId::from_name("glow_moss").expect("a placeholder holds its place");
    assert_eq!(joined.block_named("glow_moss"), Some(moss));
    let debug = content().block(BlockId::Debug);
    assert_eq!(*content().block(moss), BlockProps::placeholder(debug));
    // a change to its chunk, so that the chunk is saved again
    client.chat(&format!(
        "/put {} {} {} stone",
        stone_at.x, stone_at.y, stone_at.z
    ));
    assert_eq!(client.wait_for_chat(), "put");
    client.exit(server);

    // 4. Saved back under its name, in the chunk written again
    let names = read_meta(&world_dir).block_names.unwrap();
    assert_eq!(names[LEGACY_BLOCK_NAMES.len()..], ["glow_moss"]);
    let chunk = server::read_chunk(&chunks_dir, chunk_pos).unwrap();
    assert_eq!(
        chunk.get(to_local_pos(&stone_at)).map(|block| block.id),
        Some(BlockId::Stone),
        "the chunk was written again"
    );
    let kept = chunk.get(local).expect("the moss is kept").id;
    assert_eq!(names[kept.index()], "glow_moss");
}
