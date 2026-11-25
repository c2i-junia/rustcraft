use std::net::Ipv4Addr;
use std::process::ExitCode;

use bevy::log::{error, tracing_subscriber::EnvFilter, warn};
use clap::Parser;
use server::acquire_socket_by_port;
use shared::status::status_port;
use shared::{get_game_folder_paths, GameServerConfig, DEFAULT_MAX_RENDER_DISTANCE, DEFAULT_PORT};

/// The exit code of a process interrupted twice: 128 + SIGINT.
const INTERRUPTED_EXIT_CODE: i32 = 130;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(short, long, default_value_t = DEFAULT_PORT)]
    port: u16,

    #[arg(short, long, default_value = "default")]
    world: String,

    #[arg(short, long)]
    game_folder_path: Option<String>,

    /// The largest render distance a player may ask for, in chunks
    #[arg(long, default_value_t = DEFAULT_MAX_RENDER_DISTANCE)]
    max_render_distance: u32,

    /// Makes this player an operator before the server starts, in `ops.ron` of the game
    /// folder: how a server gets its first operator. May be given several times
    #[arg(long = "op", value_name = "NAME")]
    ops: Vec<String>,
}

fn main() -> ExitCode {
    let args = Args::parse();
    bevy::log::tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();
    // SIGTERM (`docker stop`) and Ctrl-C save the world and exit; a second signal exits at once
    if let Err(err) = ctrlc::set_handler(|| {
        if server::request_shutdown() {
            std::process::exit(INTERRUPTED_EXIT_CODE);
        }
    }) {
        warn!("Could not catch the stop signals, the world will not be saved on them: {err}");
    }
    let ip = Ipv4Addr::UNSPECIFIED.into();
    let socket = match acquire_socket_by_port(ip, args.port) {
        Ok(socket) => socket,
        Err(err) => {
            error!("Could not listen on UDP port {}: {err}", args.port);
            return ExitCode::FAILURE;
        }
    };
    // The port actually bound, which `--port 0` leaves to the system
    let game_port = socket.local_addr().map_or(args.port, |addr| addr.port());
    // Without a status port the server still runs: the server list shows it offline
    let status = match status_port(game_port) {
        Some(port) => match acquire_socket_by_port(ip, port) {
            Ok(status) => Some(status),
            Err(err) => {
                warn!("Could not listen on UDP port {port} for the status: {err}; the server list will show this server offline");
                None
            }
        },
        None => {
            warn!("UDP port {game_port} has no port next to it for the status; the server list will show this server offline");
            None
        }
    };

    let config = GameServerConfig {
        world_name: args.world,
        is_solo: false,
        saves: None,
        max_render_distance: args.max_render_distance,
    };
    let game_folder_paths = get_game_folder_paths(args.game_folder_path, None);
    if !args.ops.is_empty() {
        let folder = &game_folder_paths.game_folder_path;
        if let Err(err) = server::Permissions::add_operators(folder, &args.ops) {
            error!("{err}");
            return ExitCode::FAILURE;
        }
    }
    let result = match status {
        Some(status) => server::init_with_status(socket, status, config, game_folder_paths),
        None => server::init(socket, config, game_folder_paths),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            error!("{err}");
            ExitCode::FAILURE
        }
    }
}
