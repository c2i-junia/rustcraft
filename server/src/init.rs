use crate::network::dispatcher::{self, setup_resources_and_events};
use bevy::{
    diagnostic::{FrameTimeDiagnosticsPlugin, LogDiagnosticsPlugin},
    prelude::*,
};
use bevy_app::ScheduleRunnerPlugin;
use bevy_renet::{netcode::NetcodeServerTransport, RenetServerPlugin};
use bevy_renet::{
    netcode::{NetcodeServerPlugin, ServerAuthentication, ServerConfig},
    renet::RenetServer,
};
use serde::{Deserialize, Serialize};
use shared::{get_shared_renet_config, messages::PlayerId, GameFolderPaths, GameServerConfig};
use std::fmt::Debug;
use std::time::{Duration, SystemTime};
use std::{collections::HashMap, net::IpAddr};

use crate::world::load_from_file::{load_world_map, load_world_seed, load_world_time};

use std::net::{SocketAddr, UdpSocket};

#[derive(Resource, Serialize, Deserialize, Debug, Clone)]
pub struct ServerTime(pub u64);

#[derive(Debug, Default, Resource)]
pub struct ServerLobby {
    pub players: HashMap<PlayerId, String>,
}

#[allow(dead_code)]
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

pub fn init(socket: UdpSocket, config: GameServerConfig, game_folder_path: String) {
    let mut app = App::new();
    app.add_plugins(
        MinimalPlugins.set(ScheduleRunnerPlugin::run_loop(Duration::from_secs_f64(
            1.0 / 60.0,
        ))),
    );

    app.add_plugins(RenetServerPlugin);
    app.add_plugins(FrameTimeDiagnosticsPlugin);
    app.add_plugins(LogDiagnosticsPlugin::default());
    app.add_plugins(bevy::log::LogPlugin::default());

    app.insert_resource(ServerLobby::default());
    app.insert_resource(GameFolderPaths {
        game_folder_path: game_folder_path.clone(),
        assets_folder_path: format!("{}/data", game_folder_path),
    });

    let world_name = &config.world_name.clone();

    app.insert_resource(config);

    info!("Starting server on {}", socket.local_addr().unwrap());

    add_netcode_network(&mut app, socket);

    setup_resources_and_events(&mut app);

    // Load world from files
    let world_map = match load_world_map(world_name, &app) {
        Ok(world) => world,
        Err(e) => {
            error!("Error loading world: {}. Generating a new one.", e);
            panic!();
        }
    };

    let world_seed = match load_world_seed(world_name, &app) {
        Ok(seed) => {
            info!("World seed loaded successfully: {}", seed.0); // Affiche la seed chargée
            seed
        }
        Err(e) => {
            error!("Error loading seed: {}. Generating a new one.", e);
            panic!();
        }
    };

    let server_time = match load_world_time(world_name, &app) {
        Ok(time) => time,
        Err(e) => {
            error!("Error loading time: {}. Defaulting to 0.", e);
            0
        }
    };

    // Insert world_map and seed into ressources
    app.insert_resource(world_map);
    app.insert_resource(world_seed);
    app.insert_resource(ServerTime(server_time));

    dispatcher::register_systems(&mut app);

    setup_heartbeat(&mut app);

    app.run();
}

#[derive(Resource)]
pub struct TickCounter {
    pub(crate) tick: u64,
}

#[derive(Resource)]
struct HeartbeatTimer {
    timer: Timer,
}

fn setup_heartbeat(app: &mut App) {
    app.insert_resource(HeartbeatTimer {
        timer: Timer::new(Duration::from_secs(1), TimerMode::Repeating),
    });
    app.insert_resource(TickCounter { tick: 0 });
    app.add_systems(Update, heartbeat_system);
}

fn heartbeat_system(
    time: Res<Time>,
    mut ticker: ResMut<TickCounter>,
    mut timer: ResMut<HeartbeatTimer>,
) {
    ticker.tick += 1;
    timer.timer.tick(time.delta());
    if timer.timer.finished() {
        trace!("Server heartbeat, tick={}", ticker.tick);
    }
}
