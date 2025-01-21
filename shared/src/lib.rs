use std::time::Duration;

use bevy::{math::Vec3, prelude::Resource};
use bevy_renet::renet::{ChannelConfig, ConnectionConfig, SendType};

pub mod messages;
pub mod world;

#[derive(Resource, Debug, Clone)]
pub struct GameFolderPaths {
    pub game_folder_path: String,
    pub assets_folder_path: String,
}

#[derive(Resource, Debug, Clone)]
pub struct SpecialFlag {
    pub special_flag: bool,
}

#[derive(Resource)]
pub struct GameServerConfig {
    pub world_name: String,
    pub is_solo: bool,
}

pub const PROTOCOL_ID: u64 = 0;
pub const CHUNK_SIZE: i32 = 16;
pub const HALF_BLOCK: Vec3 = Vec3 {
    x: 0.5,
    y: 0.5,
    z: 0.5,
};

fn get_customized_default_channels() -> Vec<ChannelConfig> {
    let memory = 128 * 1024 * 1024;
    vec![
        ChannelConfig {
            channel_id: 0,
            max_memory_usage_bytes: memory,
            send_type: SendType::Unreliable,
        },
        ChannelConfig {
            channel_id: 1,
            max_memory_usage_bytes: memory,
            send_type: SendType::ReliableUnordered {
                resend_time: Duration::from_millis(300),
            },
        },
        ChannelConfig {
            channel_id: 2,
            max_memory_usage_bytes: memory,
            send_type: SendType::ReliableOrdered {
                resend_time: Duration::from_millis(300),
            },
        },
    ]
}

pub fn get_shared_renet_config() -> ConnectionConfig {
    ConnectionConfig {
        client_channels_config: get_customized_default_channels(),
        server_channels_config: get_customized_default_channels(),
        ..Default::default()
    }
}
