use crate::{
    network::{
        cleanup::cleanup_all_players_from_world,
        dispatcher::{self, setup_resources_and_events},
    },
    world::{
        data::{world_dir, ServerWorldMap},
        load_from_file::load_world_data,
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
    get_shared_renet_config, messages::PlayerId, world::ChunkMap, GameFolderPaths,
    GameServerConfig, TICKS_PER_SECOND,
};
use std::fmt::Debug;
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

// ServerLobby represents the list of players connected to the server
// (who may or may not be in the game world yet)
#[derive(Debug, Default, Resource)]
pub struct ServerLobby {
    pub players: HashMap<PlayerId, LobbyPlayer>,
}

pub fn acquire_local_ephemeral_udp_socket(ip: IpAddr) -> UdpSocket {
    acquire_socket_by_port(ip, 0)
}

pub fn acquire_socket_by_port(ip: IpAddr, port: u16) -> UdpSocket {
    let addr = SocketAddr::new(ip, port);
    UdpSocket::bind(addr).unwrap()
}

pub fn add_netcode_network(app: &mut App, socket: UdpSocket) {
    app.add_plugins(NetcodeServerPlugin);

    let server = RenetServer::new(get_shared_renet_config());

    let granted_addr = &socket.local_addr().unwrap();

    let current_time: Duration = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap();
    let server_config = ServerConfig {
        current_time,
        max_clients: 64,
        protocol_id: shared::PROTOCOL_ID,
        public_addresses: vec![*granted_addr],
        authentication: ServerAuthentication::Unsecure,
    };

    let transport = NetcodeServerTransport::new(server_config, socket).unwrap();
    app.insert_resource(server);
    app.insert_resource(transport);
}

/// One loop turn of the server is one tick: the fixed-step systems (`FixedUpdate`) run once per
/// tick, with the tick duration as delta, instead of at Bevy's default 64 Hz.
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
    let mut app = App::new();
    app.add_plugins(
        MinimalPlugins.set(ScheduleRunnerPlugin::run_loop(Duration::from_secs_f64(
            1.0 / TICKS_PER_SECOND as f64,
        ))),
    );
    set_fixed_timestep(&mut app);

    app.add_plugins(RenetServerPlugin);

    app.insert_resource(ServerLobby::default());
    app.insert_resource(game_folder_paths.clone());

    let world_name = &config.world_name.clone();
    // Every path of the world is built from this name: refuse one that is not a plain folder name
    let world_dir = world_dir(&game_folder_paths, world_name)?;

    app.insert_resource(config);

    info!("Starting server on {}", socket.local_addr().unwrap());

    add_netcode_network(&mut app, socket);

    setup_resources_and_events(&mut app);

    // Load world from files
    let world_data = match load_world_data(world_name, &game_folder_paths) {
        Ok(data) => data,
        Err(err) => return Err(format!("Failed to load world {world_name}: {err}")),
    };

    let mut chunks = ChunkMap::default();
    for (chunk_pos, chunk) in world_data.map {
        chunks.insert_chunk(chunk_pos, chunk);
    }

    let mut world_map = ServerWorldMap {
        // the folder the world was loaded from, whatever name its file claims
        name: world_name.clone(),
        chunks,
        sent_chunks: HashMap::new(),
        players: HashMap::new(),
        mobs: world_data.mobs,
        dirty_chunks: HashSet::new(),
    };

    cleanup_all_players_from_world(&mut world_map);

    // Insert world_map and seed into ressources
    app.insert_resource(world_map);
    app.insert_resource(world_data.seed);
    app.insert_resource(ServerTime(world_data.time));

    // Create save folder if does not already exist
    let players_dir = world_dir.join("players");
    if let Err(err) = std::fs::create_dir_all(&players_dir) {
        return Err(format!(
            "Could not create the save directory {} for world {world_name}: {err}",
            players_dir.display()
        ));
    }

    // The writer is dropped with the app, which waits for the last save to be written
    let writer = WorldWriter::spawn(world_dir)
        .map_err(|err| format!("Could not start the world writer: {err}"))?;
    app.insert_resource(writer);
    app.insert_resource(Autosave {
        last_save_tick: world_data.time,
    });

    dispatcher::register_systems(&mut app);

    app.run();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::time::TimeUpdateStrategy;

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
