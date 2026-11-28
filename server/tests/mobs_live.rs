//! The numbers of a kind of mob edited in the content files of a running server: after
//! `/reload`, a mob of that kind already there takes its new box and health, one born from a
//! spawn egg has them too, and it drops by its new loot; a mob naming an item that does not exist is refused with its file and its
//! entry. It installs content other than the game's, so it is alone in its file and its process.

use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, UdpSocket};
use std::path::Path;
use std::thread;
use std::time::{Duration, Instant, SystemTime};

use bevy::math::Vec3;
use bevy::transform::components::Transform;
use bevy_renet::netcode::{ClientAuthentication, NetcodeClientTransport};
use bevy_renet::RenetClient;
use shared::content::Content;
use shared::messages::{
    AuthRegisterRequest, ChatMessageRequest, ClientToServerMessage, NetworkAction,
    PlayerFrameInput, ServerToClientMessage,
};
use shared::world::{raycast, ChunkMap, ItemId, MobId, MobKind, ServerMob};
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

/// Starts a solo server of the world `herd` in `paths`, which reads its content files.
fn start_server(paths: &GameFolderPaths) -> (std::net::SocketAddr, thread::JoinHandle<()>) {
    let socket =
        server::acquire_local_ephemeral_udp_socket(IpAddr::V4(Ipv4Addr::LOCALHOST)).unwrap();
    let addr = socket.local_addr().unwrap();
    let config = GameServerConfig {
        world_name: "herd".to_string(),
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

const SHEEP_HEALTH: &str = "health: 8,\n            size: (0.9, 1.25, 0.9),";
const SHEEP_DROPS: &str = r#"(item: "raw_mutton", count: Range(1, 2)),"#;

/// The mobs the client was told of, as last told, but those removed.
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

/// Pumps until a sheep `pick` accepts is known, and returns its id.
fn wait_for_sheep(
    client: &mut TestClient,
    what: &str,
    pick: impl Fn(MobId, &ServerMob) -> bool,
) -> MobId {
    let start = Instant::now();
    loop {
        if let Some((id, _)) = known_mobs(client)
            .into_iter()
            .find(|(id, mob)| mob.kind == MobKind::Sheep && pick(*id, mob))
        {
            return id;
        }
        assert!(
            start.elapsed() < WAIT_TIMEOUT,
            "timed out waiting for {what}"
        );
        client.pump();
    }
}

#[test]
fn a_mob_edited_in_the_files_of_a_running_server_takes_its_new_numbers_after_a_reload() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = GameFolderPaths {
        game_folder_path: tmp.path().to_path_buf(),
        assets_folder_path: tmp.path().join("assets"),
    };
    let folder = paths.assets_folder_path.join("content").join("rustcraft");
    write_content_files(&folder, &[]);
    let (addr, server) = start_server(&paths);
    let mut client = TestClient::join(addr, "shepherd");
    let joined = client.wait_for_content();
    assert_eq!(joined.mob(MobKind::Sheep).health, 8);

    // 1. A sheep born from an egg two blocks ahead on the ground, of the numbers of the game
    client.chat("/give sheep_spawn_egg 2");
    assert_eq!(client.wait_for_chat(), "Gave 2 SheepSpawnEgg");
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
    while raycast::raycast(&client.world, &camera, &position, Default::default()).is_none() {
        assert!(start.elapsed() < WAIT_TIMEOUT, "the ground never arrived");
        client.pump();
    }
    let hatch = |client: &mut TestClient, time_ms| {
        client.send(ClientToServerMessage::PlayerInputs(vec![
            PlayerFrameInput {
                time_ms,
                delta_ms: 16,
                inputs: [NetworkAction::RightClick].into(),
                camera,
                hotbar_slot: 0,
                ..Default::default()
            },
        ]));
    };
    hatch(&mut client, last_time_ms + 1);
    let first = wait_for_sheep(&mut client, "a sheep born of the egg", |_, mob| {
        mob.height == 1.25
    });

    // 2. A drop of an item that does not exist: refused with its file and its entry, and
    // nothing changes
    write_content_files(&folder, &[(SHEEP_DROPS, r#"(item: "golden_fleece"),"#)]);
    client.chat("/reload");
    let refusal = client.wait_for_chat();
    assert!(
        refusal.contains("mobs.ron")
            && refusal.contains("mob \"sheep\": drops: no item is named \"golden_fleece\""),
        "{refusal}"
    );
    assert_eq!(MobKind::Sheep.health(), 8);

    // 3. A sheep of 30 points, taller, that drops sticks: sent to the player, and what the
    // server reads
    write_content_files(
        &folder,
        &[
            (
                SHEEP_HEALTH,
                "health: 30,\n            size: (0.9, 1.5, 0.9),",
            ),
            (SHEEP_DROPS, r#"(item: "stick", count: Range(2, 2)),"#),
        ],
    );
    client.chat("/reload");
    assert_eq!(client.wait_for_chat(), "Content files reloaded");
    let reloaded = client.wait_for_content();
    assert_eq!(reloaded.mob(MobKind::Sheep).health, 30);
    assert_eq!(reloaded.mob(MobKind::Sheep).size, [0.9, 1.5, 0.9]);
    // what the server rolls when a sheep dies
    assert_eq!(MobKind::Sheep.loot()[0].item, ItemId::Stick);

    // 4. The sheep already there takes the new box, on its feet in the open, and the new health
    wait_for_sheep(&mut client, "the sheep grown to its new box", |id, mob| {
        id == first && mob.height == 1.5
    });
    let sheep = &known_mobs(&client)[&first];
    assert_eq!(
        Vec3::new(sheep.width, sheep.height, sheep.depth),
        MobKind::Sheep.size()
    );
    assert_eq!(sheep.max_health(), 30);

    // 5. One born since has the new numbers too
    hatch(&mut client, last_time_ms + 2);
    let born = wait_for_sheep(&mut client, "a second sheep born of the egg", |id, mob| {
        id != first && mob.height == 1.5
    });
    let born = &known_mobs(&client)[&born];
    assert_eq!((born.width, born.depth), (0.9, 0.9));
    assert_eq!(born.max_health(), 30);
    client.exit(server);
}
