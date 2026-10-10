use std::{path::PathBuf, time::Duration};

use bevy_ecs::resource::Resource;
use bevy_log::debug;
use bevy_renet::renet::{ChannelConfig, ConnectionConfig, SendType};
use bincode::Options;

pub mod constants;
pub mod messages;
pub mod players;
pub mod utils;
pub mod world;

pub use constants::*;
use messages::{ClientToServerMessage, ServerToClientMessage};
use utils::format_bytes;

#[derive(Resource, Debug, Clone)]
pub struct GameFolderPaths {
    pub game_folder_path: PathBuf,
    pub assets_folder_path: PathBuf,
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

const MAX_MEMORY: usize = 128 * 1024 * 1024;
const RESEND_TIME: Duration = Duration::from_millis(300);
const AVAILABLE_BYTES_PER_TICK: u64 = 5 * 1024 * 1024;

pub const CTS_STANDARD_CHANNEL: u8 = 0;
pub const CTS_AUTH_CHANNEL: u8 = 1;

pub fn get_customized_client_to_server_channels() -> Vec<ChannelConfig> {
    vec![
        ChannelConfig {
            channel_id: CTS_STANDARD_CHANNEL,
            max_memory_usage_bytes: MAX_MEMORY,
            send_type: SendType::ReliableOrdered {
                resend_time: RESEND_TIME,
            },
        },
        ChannelConfig {
            channel_id: CTS_AUTH_CHANNEL,
            max_memory_usage_bytes: MAX_MEMORY,
            send_type: SendType::ReliableOrdered {
                resend_time: RESEND_TIME,
            },
        },
    ]
}

pub const STC_STANDARD_CHANNEL: u8 = 0;
pub const STC_CHUNK_DATA_CHANNEL: u8 = 1;
pub const STC_AUTH_CHANNEL: u8 = 2;

pub fn get_customized_server_to_client_channels() -> Vec<ChannelConfig> {
    vec![
        ChannelConfig {
            channel_id: STC_STANDARD_CHANNEL,
            max_memory_usage_bytes: MAX_MEMORY,
            send_type: SendType::ReliableOrdered {
                resend_time: RESEND_TIME,
            },
        },
        ChannelConfig {
            channel_id: STC_CHUNK_DATA_CHANNEL,
            max_memory_usage_bytes: MAX_MEMORY,
            send_type: SendType::ReliableOrdered {
                resend_time: RESEND_TIME,
            },
        },
        ChannelConfig {
            channel_id: STC_AUTH_CHANNEL,
            max_memory_usage_bytes: MAX_MEMORY,
            send_type: SendType::ReliableOrdered {
                resend_time: RESEND_TIME,
            },
        },
    ]
}

pub fn get_shared_renet_config() -> ConnectionConfig {
    ConnectionConfig {
        client_channels_config: get_customized_client_to_server_channels(),
        server_channels_config: get_customized_server_to_client_channels(),
        available_bytes_per_tick: AVAILABLE_BYTES_PER_TICK,
    }
}

pub fn game_message_to_payload<T: serde::Serialize>(message: T) -> Vec<u8> {
    let payload = bincode::options().serialize(&message).unwrap();
    let output = lz4::block::compress(&payload, None, true).unwrap();
    if payload.len() > 1024 {
        debug!(
            "Original payload size: {}",
            format_bytes(payload.len() as u64)
        );
        debug!(
            "Compressed payload of size: {}",
            format_bytes(output.len() as u64)
        );
    }
    output
}

pub fn payload_to_game_message<T: serde::de::DeserializeOwned>(
    payload: &[u8],
) -> Result<T, bincode::Error> {
    let decompressed_payload = lz4::block::decompress(payload, None)?;
    bincode::options().deserialize(&decompressed_payload)
}

pub fn get_game_folder_paths(
    game_folder_path: Option<String>,
    assets_folder_path: Option<String>,
) -> GameFolderPaths {
    let mut paths = default_game_folder_paths();

    if let Some(game_data) = game_folder_path {
        paths.game_folder_path = game_data.into();
    }
    if let Some(game_assets) = assets_folder_path {
        paths.assets_folder_path = game_assets.into();
    }

    paths
}

#[cfg(target_os = "windows")]
pub fn default_game_folder_paths() -> GameFolderPaths {
    GameFolderPaths {
        game_folder_path: "%AppData/rustcraft".into(),
        assets_folder_path: "%AppData/rustcraft/data".into(),
    }
}

#[cfg(target_os = "linux")]
pub fn default_game_folder_paths() -> GameFolderPaths {
    GameFolderPaths {
        game_folder_path: "$HOME/.local/share/rustcraft".into(),
        assets_folder_path: "$HOME/.config/rustcraft".into(),
    }
}

pub trait ChannelResolvableExt {
    fn get_channel_id(&self) -> u8;
}

impl ChannelResolvableExt for ClientToServerMessage {
    fn get_channel_id(&self) -> u8 {
        match self {
            ClientToServerMessage::AuthRegisterRequest(_) => CTS_AUTH_CHANNEL,
            _ => CTS_STANDARD_CHANNEL,
        }
    }
}

impl ChannelResolvableExt for ServerToClientMessage {
    fn get_channel_id(&self) -> u8 {
        match self {
            ServerToClientMessage::WorldUpdate(_) => STC_CHUNK_DATA_CHANNEL,
            ServerToClientMessage::AuthRegisterResponse(_) => STC_AUTH_CHANNEL,
            _ => STC_STANDARD_CHANNEL,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::messages::{ChatConversation, ChatMessageRequest, FullChatMessage};

    #[test]
    fn client_to_server_message_round_trips() {
        let message = ClientToServerMessage::ChatMessage(ChatMessageRequest {
            content: "hello".into(),
        });
        let payload = game_message_to_payload(message.clone());
        let decoded: ClientToServerMessage = payload_to_game_message(&payload).unwrap();
        assert_eq!(decoded, message);
    }

    #[test]
    fn server_to_client_message_round_trips() {
        let conversation = ChatConversation {
            messages: vec![FullChatMessage {
                content: "hi".into(),
                author: "steve".into(),
                timestamp: 42,
            }],
        };
        let payload = game_message_to_payload(ServerToClientMessage::ChatConversation(
            conversation.clone(),
        ));
        let decoded: ServerToClientMessage = payload_to_game_message(&payload).unwrap();
        match decoded {
            ServerToClientMessage::ChatConversation(decoded) => assert_eq!(decoded, conversation),
            other => panic!("unexpected variant: {other:?}"),
        }
    }

    #[test]
    fn decoding_an_empty_slice_is_an_error() {
        assert!(payload_to_game_message::<ClientToServerMessage>(&[]).is_err());
    }

    #[test]
    fn decoding_garbage_bytes_is_an_error() {
        // Last case: valid lz4 framing around bytes that are not a valid message.
        let valid_lz4_invalid_message = lz4::block::compress(&[0xff; 8], None, true).unwrap();
        let garbage: [&[u8]; 3] = [
            &[1, 2, 3],
            &[0xde, 0xad, 0xbe, 0xef, 0x01, 0x02, 0x03, 0x04],
            &valid_lz4_invalid_message,
        ];
        for bytes in garbage {
            assert!(
                payload_to_game_message::<ClientToServerMessage>(bytes).is_err(),
                "{bytes:?}"
            );
            assert!(
                payload_to_game_message::<ServerToClientMessage>(bytes).is_err(),
                "{bytes:?}"
            );
        }
    }
}
