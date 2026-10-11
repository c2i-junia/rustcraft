use std::{ffi::OsString, path::PathBuf, time::Duration};

use bevy_ecs::resource::Resource;
use bevy_log::{debug, warn};
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

/// Largest decompressed message accepted from the network.
pub const MAX_DECOMPRESSED_MESSAGE_SIZE: usize = 64 * 1024 * 1024;

/// Largest decompressed message accepted from a client, an unauthenticated one included.
///
/// The largest legitimate one is a batch of inputs: at most about 75 bytes each (two varint
/// times, a transform of 40 bytes, a handful of actions), and a client sends one per rendered
/// frame, so around 2000 after a two second stall at 1000 frames per second, that is about
/// 150 KiB. 1 MiB leaves a margin of about seven. Chat and authentication messages are far smaller.
pub const MAX_CLIENT_MESSAGE_SIZE: usize = 1024 * 1024;

pub fn payload_to_game_message<T: serde::de::DeserializeOwned>(
    payload: &[u8],
) -> Result<T, bincode::Error> {
    payload_to_game_message_limited(payload, MAX_DECOMPRESSED_MESSAGE_SIZE)
}

/// Decodes a message whose decompressed size is at most `max_size`.
pub fn payload_to_game_message_limited<T: serde::de::DeserializeOwned>(
    payload: &[u8],
    max_size: usize,
) -> Result<T, bincode::Error> {
    // lz4 trusts the 4-byte little-endian size prefix to size its allocation: check it first
    let Some(prefix) = payload.first_chunk::<4>() else {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "payload shorter than its size prefix",
        )
        .into());
    };
    let announced_size = u32::from_le_bytes(*prefix) as usize;
    if announced_size > max_size {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("announced decompressed size {announced_size} is too large"),
        )
        .into());
    }
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

#[derive(Clone, Copy)]
enum Platform {
    Linux,
    Windows,
    MacOs,
}

/// Default folders of the current platform, resolved from the process environment.
pub fn default_game_folder_paths() -> GameFolderPaths {
    let platform = if cfg!(target_os = "windows") {
        Platform::Windows
    } else if cfg!(target_os = "macos") {
        Platform::MacOs
    } else {
        Platform::Linux
    };
    game_folder_paths_for(platform, |name| std::env::var_os(name))
}

/// Builds the default folders of `platform`. `env` looks an environment variable up; unset
/// and empty variables are treated alike. Without a usable home folder the game falls back
/// to a `rustcraft` folder in the working directory.
fn game_folder_paths_for(
    platform: Platform,
    env: impl Fn(&str) -> Option<OsString>,
) -> GameFolderPaths {
    let var = |name: &str| {
        env(name)
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
    };

    let folders = match platform {
        Platform::Linux => var("HOME").map(|home| {
            let data_home = var("XDG_DATA_HOME").unwrap_or_else(|| home.join(".local/share"));
            (data_home.join("rustcraft"), home.join(".config/rustcraft"))
        }),
        Platform::Windows => var("APPDATA").map(|app_data| {
            let game_folder = app_data.join("rustcraft");
            (game_folder.clone(), game_folder.join("data"))
        }),
        Platform::MacOs => var("HOME").map(|home| {
            let game_folder = home.join("Library/Application Support/rustcraft");
            (game_folder.clone(), game_folder.join("data"))
        }),
    };

    let (game_folder_path, assets_folder_path) = folders.unwrap_or_else(|| {
        warn!("No home folder found in the environment, using ./rustcraft");
        ("rustcraft".into(), "rustcraft/data".into())
    });
    GameFolderPaths {
        game_folder_path,
        assets_folder_path,
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
            ServerToClientMessage::WorldUpdate(_) | ServerToClientMessage::BlockUpdates(_) => {
                STC_CHUNK_DATA_CHANNEL
            }
            ServerToClientMessage::AuthRegisterResponse(_) => STC_AUTH_CHANNEL,
            _ => STC_STANDARD_CHANNEL,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::messages::{ChatMessageRequest, FullChatMessage};

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
        let chat_message = FullChatMessage {
            content: "hi".into(),
            author: "steve".into(),
            timestamp: 42,
        };
        let payload =
            game_message_to_payload(ServerToClientMessage::ChatMessage(chat_message.clone()));
        let decoded: ServerToClientMessage = payload_to_game_message(&payload).unwrap();
        match decoded {
            ServerToClientMessage::ChatMessage(decoded) => assert_eq!(decoded, chat_message),
            other => panic!("unexpected variant: {other:?}"),
        }
    }

    #[test]
    fn block_updates_round_trip_on_the_world_channel() {
        use crate::messages::BlockUpdate;
        use crate::world::{BlockData, BlockDirection, BlockId};
        use bevy::math::IVec3;

        let updates = vec![
            BlockUpdate {
                position: IVec3::new(-3, 70, 12),
                block: Some(BlockData::new(BlockId::Stone, BlockDirection::Front)),
            },
            BlockUpdate {
                position: IVec3::new(4, 5, 6),
                block: None,
            },
        ];
        let message = ServerToClientMessage::BlockUpdates(updates.clone());
        assert_eq!(message.get_channel_id(), STC_CHUNK_DATA_CHANNEL);

        let payload = game_message_to_payload(message);
        match payload_to_game_message::<ServerToClientMessage>(&payload).unwrap() {
            ServerToClientMessage::BlockUpdates(decoded) => assert_eq!(decoded, updates),
            other => panic!("unexpected variant: {other:?}"),
        }
    }

    #[test]
    fn decoding_an_empty_slice_is_an_error() {
        assert!(payload_to_game_message::<ClientToServerMessage>(&[]).is_err());
    }

    #[test]
    fn decoding_a_forged_size_prefix_is_rejected_before_allocating() {
        for announced in [
            MAX_DECOMPRESSED_MESSAGE_SIZE as u32 + 1,
            i32::MAX as u32,
            u32::MAX,
        ] {
            let mut payload = announced.to_le_bytes().to_vec();
            payload.extend_from_slice(&[0x10, 0x00]);
            let error = payload_to_game_message::<ClientToServerMessage>(&payload).unwrap_err();
            assert!(
                error.to_string().contains("too large"),
                "{announced}: {error}"
            );
        }
    }

    #[test]
    fn a_payload_over_the_client_limit_is_rejected_by_the_client_decoder() {
        let mut payload = (MAX_CLIENT_MESSAGE_SIZE as u32 + 1).to_le_bytes().to_vec();
        payload.extend_from_slice(&[0x10, 0x00]);
        let error = payload_to_game_message_limited::<ClientToServerMessage>(
            &payload,
            MAX_CLIENT_MESSAGE_SIZE,
        )
        .unwrap_err();
        assert!(error.to_string().contains("too large"), "{error}");
        // The limit for what the server sends is larger: not rejected for its size
        let error = payload_to_game_message::<ClientToServerMessage>(&payload).unwrap_err();
        assert!(!error.to_string().contains("too large"), "{error}");
    }

    #[test]
    fn a_batch_of_inputs_far_above_any_real_one_fits_in_the_client_limit() {
        use crate::messages::{NetworkAction::*, PlayerFrameInput};
        let input = PlayerFrameInput {
            time_ms: u64::MAX,
            delta_ms: u64::MAX,
            inputs: [
                MoveForward,
                MoveRight,
                MoveBackward,
                MoveLeft,
                JumpOrFlyUp,
                SneakOrFlyDown,
                ToggleFlyMode,
                LeftClick,
                RightClick,
            ]
            .into(),
            hotbar_slot: u32::MAX,
            ..Default::default()
        };
        let batch = ClientToServerMessage::PlayerInputs(vec![input; 2000]);
        let payload = game_message_to_payload(batch);
        assert!(payload_to_game_message_limited::<ClientToServerMessage>(
            &payload,
            MAX_CLIENT_MESSAGE_SIZE
        )
        .is_ok());
    }

    #[test]
    fn decoding_a_prefix_at_the_limit_is_not_rejected_for_its_size() {
        let mut payload = (MAX_DECOMPRESSED_MESSAGE_SIZE as u32)
            .to_le_bytes()
            .to_vec();
        payload.extend_from_slice(&[0x10, 0x00]);
        // The body is truncated, so decoding still fails, but not on the size check.
        let error = payload_to_game_message::<ClientToServerMessage>(&payload).unwrap_err();
        assert!(!error.to_string().contains("too large"), "{error}");
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

    fn paths_with(platform: Platform, vars: &[(&str, &str)]) -> (PathBuf, PathBuf) {
        let paths = game_folder_paths_for(platform, |name| {
            vars.iter()
                .find(|(key, _)| *key == name)
                .map(|(_, value)| OsString::from(value))
        });
        (paths.game_folder_path, paths.assets_folder_path)
    }

    #[test]
    fn linux_defaults_live_under_home() {
        let (game, assets) = paths_with(Platform::Linux, &[("HOME", "/home/bob")]);
        assert_eq!(game, PathBuf::from("/home/bob/.local/share/rustcraft"));
        assert_eq!(assets, PathBuf::from("/home/bob/.config/rustcraft"));
    }

    #[test]
    fn linux_honours_xdg_data_home_for_the_game_folder() {
        let (game, assets) = paths_with(
            Platform::Linux,
            &[("HOME", "/home/bob"), ("XDG_DATA_HOME", "/data")],
        );
        assert_eq!(game, PathBuf::from("/data/rustcraft"));
        assert_eq!(assets, PathBuf::from("/home/bob/.config/rustcraft"));
    }

    #[test]
    fn empty_variables_count_as_unset() {
        let (game, _) = paths_with(
            Platform::Linux,
            &[("HOME", "/home/bob"), ("XDG_DATA_HOME", "")],
        );
        assert_eq!(game, PathBuf::from("/home/bob/.local/share/rustcraft"));
    }

    #[test]
    fn windows_defaults_live_under_appdata() {
        let (game, assets) = paths_with(Platform::Windows, &[("APPDATA", "C:/Users/bob/AppData")]);
        assert_eq!(game, PathBuf::from("C:/Users/bob/AppData/rustcraft"));
        assert_eq!(assets, PathBuf::from("C:/Users/bob/AppData/rustcraft/data"));
    }

    #[test]
    fn macos_defaults_live_under_application_support() {
        let (game, assets) = paths_with(Platform::MacOs, &[("HOME", "/Users/bob")]);
        let support = PathBuf::from("/Users/bob/Library/Application Support/rustcraft");
        assert_eq!(game, support);
        assert_eq!(assets, support.join("data"));
    }

    #[test]
    fn missing_home_falls_back_to_the_working_directory() {
        for platform in [Platform::Linux, Platform::Windows, Platform::MacOs] {
            let (game, assets) = paths_with(platform, &[]);
            assert_eq!(game, PathBuf::from("rustcraft"));
            assert_eq!(assets, PathBuf::from("rustcraft/data"));
        }
    }
}
