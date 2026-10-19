use crate::{
    mods::Mods,
    network::{
        cleanup::cleanup_all_players_from_world,
        dispatcher::{self, setup_resources_and_events},
        status::{publish_player_count, spawn_status_responder, StatusCounter},
    },
    world::{
        container_files::insert_with_chests,
        data::{world_dir, ServerWorldMap},
        dropped_items::DroppedItems,
        load_from_file::open_world,
        save::Autosave,
        writer::WorldWriter,
    },
};
use bevy::prelude::*;
use bevy_app::ScheduleRunnerPlugin;
use bevy_renet::netcode::{NetcodeServerPlugin, ServerAuthentication, ServerConfig};
use bevy_renet::{netcode::NetcodeServerTransport, RenetServer, RenetServerPlugin};
use serde::{Deserialize, Serialize};
use shared::{
    get_shared_renet_config,
    messages::PlayerId,
    status::status_port,
    world::{ChunkMap, WorldSeed},
    GameFolderPaths, GameServerConfig, TICKS_PER_SECOND,
};
use std::fmt::Debug;
use std::sync::{atomic::AtomicU32, Arc};
use std::time::{Duration, SystemTime};
use std::{
    collections::{HashMap, HashSet},
    net::IpAddr,
};

use std::net::{SocketAddr, UdpSocket};

#[derive(Resource, Serialize, Deserialize, Debug, Clone)]
pub struct ServerTime(pub u64);

#[derive(Debug)]
pub struct LobbyPlayer {
    pub name: String,
}

impl LobbyPlayer {
    pub fn new(name: String) -> Self {
        Self { name }
    }
}

/// The authenticated clients. A client is added here by its authentication, together with its
/// `Player` in `ServerWorldMap::players`, and both are removed when it disconnects; a client
/// that is connected but absent from the lobby may only ask to authenticate.
#[derive(Debug, Default, Resource)]
pub struct ServerLobby {
    pub players: HashMap<PlayerId, LobbyPlayer>,
}

pub fn acquire_local_ephemeral_udp_socket(ip: IpAddr) -> std::io::Result<UdpSocket> {
    acquire_socket_by_port(ip, 0)
}

pub fn acquire_socket_by_port(ip: IpAddr, port: u16) -> std::io::Result<UdpSocket> {
    UdpSocket::bind(SocketAddr::new(ip, port))
}

/// A game socket on an ephemeral port of `ip` and its status socket on the next port; retries
/// up to 16 times when that port cannot be bound.
pub fn acquire_local_socket_pair(ip: IpAddr) -> std::io::Result<(UdpSocket, UdpSocket)> {
    const ATTEMPTS: usize = 16;
    let mut last_error = None;
    for _ in 0..ATTEMPTS {
        let game = acquire_local_ephemeral_udp_socket(ip)?;
        // The last port has no neighbour: draw another one
        let Some(port) = status_port(game.local_addr()?.port()) else {
            continue;
        };
        match acquire_socket_by_port(ip, port) {
            Ok(status) => return Ok((game, status)),
            Err(err) => last_error = Some(err),
        }
    }
    Err(last_error.unwrap_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::AddrInUse,
            "no ephemeral UDP port had a free port next to it",
        )
    }))
}

/// The period of the loop of the server: the transport sends and receives on every turn, and a
/// tick runs (`FixedUpdate`) on the turns its time has come. A connection takes several round
/// trips: at the tick rate, each would wait up to a tick.
const LOOP_PERIOD: Duration = Duration::from_millis(4);

/// Largest number of clients connected at once.
const MAX_CLIENTS: usize = 64;

fn add_netcode_network(app: &mut App, socket: UdpSocket) -> Result<(), String> {
    app.add_plugins(NetcodeServerPlugin);

    let address = socket
        .local_addr()
        .map_err(|err| format!("The server socket has no local address: {err}"))?;
    info!("Starting server on {address}");

    let server_config = ServerConfig {
        current_time: SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default(),
        max_clients: MAX_CLIENTS,
        protocol_id: shared::PROTOCOL_ID,
        public_addresses: vec![address],
        // Nothing proves who a client is: it announces its own id. Fine among friends, not
        // for a server open to anyone.
        authentication: ServerAuthentication::Unsecure,
    };
    shared::enlarge_socket_buffers(&socket);
    let transport = NetcodeServerTransport::new(server_config, socket)
        .map_err(|err| format!("Could not start the network transport: {err}"))?;

    app.insert_resource(RenetServer::new(get_shared_renet_config()));
    app.insert_resource(transport);
    Ok(())
}

/// A tick is one run of the fixed-step systems (`FixedUpdate`), `TICKS_PER_SECOND` times a
/// second with the tick duration as delta, instead of at Bevy's default 64 Hz.
fn set_fixed_timestep(app: &mut App) {
    app.insert_resource(Time::<Fixed>::from_hz(TICKS_PER_SECOND as f64));
}

/// Runs the server until it exits. Fails without starting when the saved world cannot be
/// loaded: a fresh world generated over it would destroy the save.
///
/// Installs no logger: the caller owns that, as the server may be embedded in a process that
/// already has one.
pub fn init(
    socket: UdpSocket,
    config: GameServerConfig,
    game_folder_paths: GameFolderPaths,
) -> Result<(), String> {
    run(socket, None, config, game_folder_paths)
}

/// Like `init`, and answers status requests on `status` while the server runs.
pub fn init_with_status(
    socket: UdpSocket,
    status: UdpSocket,
    config: GameServerConfig,
    game_folder_paths: GameFolderPaths,
) -> Result<(), String> {
    run(socket, Some(status), config, game_folder_paths)
}

fn run(
    socket: UdpSocket,
    status: Option<UdpSocket>,
    config: GameServerConfig,
    game_folder_paths: GameFolderPaths,
) -> Result<(), String> {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins.set(ScheduleRunnerPlugin::run_loop(LOOP_PERIOD)));
    set_fixed_timestep(&mut app);

    app.add_plugins(RenetServerPlugin);

    app.insert_resource(ServerLobby::default());
    app.insert_resource(game_folder_paths.clone());

    let world_name = config.world_name.clone();
    // Every path of the world is built from this name: refuse one that is not a plain folder name
    let world_dir = world_dir(&game_folder_paths, &world_name)?;
    let saves = config.saves.clone();

    app.insert_resource(config);

    add_netcode_network(&mut app, socket)?;

    setup_resources_and_events(&mut app);

    let (world_map, seed, time) = load_world(&world_name, &game_folder_paths)?;
    app.insert_resource(world_map);
    app.insert_resource(seed);
    app.insert_resource(ServerTime(time));

    let mods = Mods::new().map_err(|err| format!("Could not start the mods: {err}"))?;
    mods.load_folder(&game_folder_paths.game_folder_path.join("mods"));
    app.insert_resource(mods);

    // Create save folder if does not already exist
    let players_dir = world_dir.join("players");
    if let Err(err) = std::fs::create_dir_all(&players_dir) {
        return Err(format!(
            "Could not create the save directory {} for world {world_name}: {err}",
            players_dir.display()
        ));
    }

    // The writer is dropped with the app, which waits for the last save to be written
    let writer = WorldWriter::spawn(world_dir, saves)
        .map_err(|err| format!("Could not start the world writer: {err}"))?;
    app.insert_resource(writer);
    app.insert_resource(Autosave {
        last_save_tick: time,
    });

    dispatcher::register_systems(&mut app);

    if let Some(status) = status {
        let players = Arc::new(AtomicU32::new(0));
        // The responder holds a weak reference: it stops once the app is dropped
        if let Err(err) =
            spawn_status_responder(status, Arc::downgrade(&players), MAX_CLIENTS as u32)
        {
            warn!("Could not answer status requests: {err}; the server list will show this server offline");
        }
        app.insert_resource(StatusCounter(players));
        app.add_systems(Update, publish_player_count);
    }

    app.run();
    Ok(())
}

/// The world saved under `world_name`, or a new one when nothing was saved yet, with its
/// seed and its time in ticks. Its chunks are read from their files as the players need them.
fn load_world(
    world_name: &str,
    game_folder_paths: &GameFolderPaths,
) -> Result<(ServerWorldMap, WorldSeed, u64), String> {
    // the cause alone, which names the file: the caller knows the world it asked for
    let world_data = open_world(world_name, game_folder_paths).map_err(|err| err.to_string())?;

    // A world converted from a single file still holds its chunks, and had no chests
    let mut chunks = ChunkMap::default();
    for (chunk_pos, chunk) in world_data.map {
        insert_with_chests(&mut chunks, chunk_pos, chunk, Vec::new());
    }
    let mut world_map = ServerWorldMap {
        // the folder the world was loaded from, whatever name its file claims
        name: world_name.to_string(),
        chunks,
        sent_chunks: HashMap::new(),
        render_distances: Default::default(),
        players: HashMap::new(),
        mobs: world_data.mobs,
        dropped_items: DroppedItems::from_saved(world_data.dropped_items),
        sent_items: HashMap::new(),
        dirty_chunks: HashSet::new(),
        saved: Some(world_dir(game_folder_paths, world_name)?),
    };
    cleanup_all_players_from_world(&mut world_map);

    Ok((world_map, world_data.seed, world_data.time))
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::time::TimeUpdateStrategy;
    use std::net::Ipv4Addr;

    #[derive(Resource, Default)]
    struct FixedRuns(Vec<Duration>);

    #[test]
    fn fixed_update_runs_once_per_tick_with_the_tick_duration() {
        let tick = Duration::from_secs_f64(1.0 / TICKS_PER_SECOND as f64);
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .insert_resource(TimeUpdateStrategy::ManualDuration(tick))
            .init_resource::<FixedRuns>()
            .add_systems(
                FixedUpdate,
                |time: Res<Time<Fixed>>, mut runs: ResMut<FixedRuns>| runs.0.push(time.delta()),
            );
        set_fixed_timestep(&mut app);

        // the first update only starts the clock
        for _ in 0..=TICKS_PER_SECOND {
            app.update();
        }

        let runs = &app.world().resource::<FixedRuns>().0;
        assert_eq!(runs.len() as u64, TICKS_PER_SECOND);
        assert!(runs.iter().all(|delta| *delta == tick));
    }

    #[test]
    fn the_socket_pair_is_two_neighbouring_ports() {
        let (game, status) = acquire_local_socket_pair(Ipv4Addr::LOCALHOST.into()).unwrap();

        assert_eq!(
            status.local_addr().unwrap().port(),
            game.local_addr().unwrap().port() + 1
        );
    }

    #[test]
    fn a_save_folder_that_cannot_be_created_is_an_error_naming_the_path() {
        let tmp = tempfile::tempdir().unwrap();
        let world = tmp.path().join("saves").join("w");
        std::fs::create_dir_all(&world).unwrap();
        // `players` is a regular file, so the folder cannot be created
        std::fs::write(world.join("players"), b"").unwrap();

        let err = init(
            UdpSocket::bind("127.0.0.1:0").unwrap(),
            GameServerConfig {
                world_name: "w".into(),
                is_solo: true,
                saves: None,
                max_render_distance: shared::DEFAULT_MAX_RENDER_DISTANCE,
            },
            GameFolderPaths {
                game_folder_path: tmp.path().into(),
                assets_folder_path: tmp.path().into(),
            },
        )
        .unwrap_err();

        assert!(err.contains("players"), "{err}");
    }
}
