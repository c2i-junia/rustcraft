//! Recipes and smelting edited in the content files of a running server: after `/reload`, the
//! grid makes what the new recipe says and the furnaces smelt by the new table, and a bad recipe
//! is refused with its file and its entry. It installs content other than the game's, so it is
//! alone in its file and its process.

use std::net::{IpAddr, Ipv4Addr, UdpSocket};
use std::path::Path;
use std::thread;
use std::time::{Duration, Instant, SystemTime};

use bevy_renet::netcode::{ClientAuthentication, NetcodeClientTransport};
use bevy_renet::RenetClient;
use shared::content::Content;
use shared::messages::{
    AuthRegisterRequest, ChatMessageRequest, ClientToServerMessage, PlayerFrameInput,
    ServerToClientMessage,
};
use shared::players::inventory::InventoryAction;
use shared::players::Inventory;
use shared::world::{recipes_for, smelting_result, ItemId, ItemStack};
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

    /// Sends `actions` in a frame of `time_ms`, and returns the inventory the server answers
    /// with once it applied them.
    fn act(&mut self, time_ms: u64, actions: Vec<InventoryAction>) -> Inventory {
        self.send(ClientToServerMessage::PlayerInputs(vec![
            PlayerFrameInput {
                time_ms,
                delta_ms: 16,
                inventory_actions: actions,
                ..Default::default()
            },
        ]));
        self.wait_for("the actions applied", |m| match m {
            ServerToClientMessage::PlayerUpdate(update) if update.last_ack_time == time_ms => {
                update.inventory.clone()
            }
            _ => None,
        })
    }
}

/// Writes the content files of the game into `folder`, with each `(from, to)` of `edits`
/// replaced in the one file that holds it once.
fn write_content_files(folder: &Path, edits: &[(&str, &str)]) {
    std::fs::create_dir_all(folder).unwrap();
    let game = Path::new(env!("CARGO_MANIFEST_DIR")).join("../data/content/rustcraft");
    let mut replaced = vec![0; edits.len()];
    for entry in std::fs::read_dir(game).unwrap() {
        let path = entry.unwrap().path();
        let mut text = std::fs::read_to_string(&path).unwrap();
        for ((from, to), replaced) in edits.iter().zip(&mut replaced) {
            *replaced += text.matches(from).count();
            text = text.replace(from, to);
        }
        std::fs::write(folder.join(path.file_name().unwrap()), text).unwrap();
    }
    assert!(replaced.iter().all(|&count| count == 1), "{replaced:?}");
}

/// Starts a solo server in `paths`, which reads its content files.
fn start_server(paths: &GameFolderPaths) -> (std::net::SocketAddr, thread::JoinHandle<()>) {
    let socket =
        server::acquire_local_ephemeral_udp_socket(IpAddr::V4(Ipv4Addr::LOCALHOST)).unwrap();
    let addr = socket.local_addr().unwrap();
    let config = GameServerConfig {
        world_name: "recipes".to_string(),
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

/// The recipe of the planks of an oak log, as `content` has it.
fn oak_planks_made(content: &Content) -> u32 {
    content
        .crafting
        .recipes
        .iter()
        .find(|recipe| recipe.output == ItemId::OakPlanks)
        .unwrap()
        .nb
}

const OAK_PLANKS: &str = r#"(out: "oak_planks", count: 4,"#;
const SAND: &str = r#"("sand", "glass"),"#;

#[test]
fn a_recipe_and_a_smelting_edited_in_the_files_of_a_running_server_apply_after_a_reload() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = GameFolderPaths {
        game_folder_path: tmp.path().to_path_buf(),
        assets_folder_path: tmp.path().join("assets"),
    };
    let folder = paths.assets_folder_path.join("content").join("rustcraft");
    write_content_files(&folder, &[]);
    let (addr, server) = start_server(&paths);
    let mut client = TestClient::join(addr, "crafter");
    let joined = client.wait_for_content();
    assert_eq!(oak_planks_made(&joined), 4);
    assert_eq!(smelting_result(ItemId::Sand), Some(ItemId::Glass));

    // 1. A bad recipe: refused with its file and its entry, and nothing changes
    write_content_files(&folder, &[(OAK_PLANKS, r#"(out: "oak_planks", count: 0,"#)]);
    client.chat("/reload");
    let refusal = client.wait_for_chat();
    assert!(
        refusal.contains("crafting.ron")
            && refusal.contains("recipe 1 (out: \"oak_planks\"): count 0"),
        "{refusal}"
    );
    assert_eq!(oak_planks_made(shared::content::content()), 4);

    // 2. Two planks a log, and sand smelted into stone: sent to the player, and what the
    // server crafts and smelts by
    write_content_files(
        &folder,
        &[
            (OAK_PLANKS, r#"(out: "oak_planks", count: 2,"#),
            (SAND, r#"("sand", "stone"),"#),
        ],
    );
    client.chat("/reload");
    assert_eq!(client.wait_for_chat(), "Content files reloaded");
    let reloaded = client.wait_for_content();
    assert_eq!(oak_planks_made(&reloaded), 2);
    assert!(reloaded
        .crafting
        .smelting
        .contains(&(ItemId::Sand, ItemId::Stone)));
    // what a furnace of the server reads on its next tick
    assert_eq!(smelting_result(ItemId::Sand), Some(ItemId::Stone));

    client.chat("/give oak_log 1");
    assert_eq!(client.wait_for_chat(), "Gave 1 OakLog");
    let planks = recipes_for(ItemId::OakPlanks).next().unwrap();
    client.act(
        1,
        vec![InventoryAction::FillCraftingGrid {
            recipe: planks.index,
            times: 1,
        }],
    );
    let crafted = client.act(2, vec![InventoryAction::TakeCraftResult]);
    assert_eq!(
        crafted.floating,
        Some(ItemStack {
            item_id: ItemId::OakPlanks,
            item_type: ItemId::OakPlanks.get_default_type(),
            nb: 2,
        })
    );
    client.exit(server);
}
