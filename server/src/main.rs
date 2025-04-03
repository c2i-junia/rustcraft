use std::net::Ipv4Addr;

use crate::init::acquire_socket_by_port;
use clap::Parser;
use shared::GameServerConfig;

mod init;
mod mob;
mod network;
mod world;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(short, long, default_value_t = 8000)]
    port: u16,

    #[arg(short, long, default_value = "default")]
    world: String,

    #[arg(short, long, default_value = "../")]
    game_folder_path: String,
}

fn main() {
    let args = Args::parse();
    let socket = acquire_socket_by_port(std::net::IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), args.port);

    let game_folder_path = args.game_folder_path.clone();

    init::init(
        socket,
        GameServerConfig {
            world_name: args.world,
            is_solo: false,
        },
        game_folder_path,
    );
}
