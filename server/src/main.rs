use std::net::Ipv4Addr;
use std::process::ExitCode;

use bevy::log::{error, tracing_subscriber::EnvFilter};
use clap::Parser;
use server::acquire_socket_by_port;
use shared::{get_game_folder_paths, GameServerConfig};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(short, long, default_value_t = 8000)]
    port: u16,

    #[arg(short, long, default_value = "default")]
    world: String,

    #[arg(short, long)]
    game_folder_path: Option<String>,
}

fn main() -> ExitCode {
    let args = Args::parse();
    bevy::log::tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();
    let socket = acquire_socket_by_port(std::net::IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), args.port);

    match server::init(
        socket,
        GameServerConfig {
            world_name: args.world,
            is_solo: false,
        },
        get_game_folder_paths(args.game_folder_path, None),
    ) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            error!("{err}");
            ExitCode::FAILURE
        }
    }
}
