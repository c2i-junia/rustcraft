use crate::{
    network::{
        cleanup::cleanup_all_players_from_world,
        dispatcher::{self, setup_resources_and_events},
    },
    world::{data::SAVE_PATH, load_from_file::load_world_data},
};
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
use shared::{
    get_shared_renet_config,
    messages::PlayerId,
    world::{get_game_folder, ServerChunkWorldMap, ServerWorldMap},
    GameFolderPaths, GameServerConfig, TICKS_PER_SECOND,
};
use std::fmt::Debug;
use std::time::{Duration, SystemTime};
use std::{collections::HashMap, net::IpAddr};

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
            1.0 / TICKS_PER_SECOND as f64,
        ))),
    );

    let game_folder_paths = GameFolderPaths {
        game_folder_path: game_folder_path.clone(),
        assets_folder_path: format!("{game_folder_path}/data"),
    };

    app.add_plugins(RenetServerPlugin);
    app.add_plugins(FrameTimeDiagnosticsPlugin::default());
    app.add_plugins(LogDiagnosticsPlugin::default());
    app.add_plugins(bevy::log::LogPlugin::default());

    app.insert_resource(ServerLobby::default());
    app.insert_resource(game_folder_paths.clone());

    let world_name = &config.world_name.clone();

    app.insert_resource(config);

    info!("Starting server on {}", socket.local_addr().unwrap());

    add_netcode_network(&mut app, socket);

    setup_resources_and_events(&mut app);

    // Load world from files
    let world_data =
        match load_world_data(world_name, app.world().get_resource::<GameFolderPaths>()) {
            Ok(data) => data,
            Err(err) => {
                error!(
                    "Failed to load world {} & failed to create a default world : {}",
                    world_name, err
                );
                panic!()
            }
        };

    let mut world_map = ServerWorldMap {
        name: world_data.name,
        chunks: ServerChunkWorldMap {
            map: world_data.map,
            chunks_to_update: Vec::new(),
        },
        players: HashMap::new(),
        mobs: world_data.mobs,
        item_stacks: world_data.item_stacks,
        time: world_data.time,
    };

    cleanup_all_players_from_world(&mut world_map);

    // Insert world_map and seed into ressources
    app.insert_resource(world_map);
    app.insert_resource(world_data.seed);
    app.insert_resource(ServerTime(world_data.time));

    // Create save folder if does not already exist
    let save_folder = format!(
        "{}{}/players/",
        get_game_folder(Some(&game_folder_paths))
            .join(SAVE_PATH)
            .display(),
        world_name
    );

    if let Err(err) = std::fs::create_dir_all(save_folder) {
        error!(
            "Could not create save directory for map {} : {}",
            world_name, err
        );
        panic!();
    }

    dispatcher::register_systems(&mut app);

    app.run();
}
