//! The renet channels of the game, and which channel each message travels on.

use std::time::Duration;

use bevy_renet::renet::{ChannelConfig, ConnectionConfig, SendType};

use crate::messages::{ClientToServerMessage, ServerToClientMessage};

const MAX_MEMORY: usize = 128 * 1024 * 1024;
const RESEND_TIME: Duration = Duration::from_millis(300);
const AVAILABLE_BYTES_PER_TICK: u64 = 5 * 1024 * 1024;

pub const CTS_STANDARD_CHANNEL: u8 = 0;
pub const CTS_AUTH_CHANNEL: u8 = 1;

pub const STC_STANDARD_CHANNEL: u8 = 0;
pub const STC_CHUNK_DATA_CHANNEL: u8 = 1;
pub const STC_AUTH_CHANNEL: u8 = 2;

/// Every channel of the game is reliable and ordered; they only differ by their id.
fn reliable_ordered_channel(channel_id: u8) -> ChannelConfig {
    ChannelConfig {
        channel_id,
        max_memory_usage_bytes: MAX_MEMORY,
        send_type: SendType::ReliableOrdered {
            resend_time: RESEND_TIME,
        },
    }
}

pub fn get_customized_client_to_server_channels() -> Vec<ChannelConfig> {
    [CTS_STANDARD_CHANNEL, CTS_AUTH_CHANNEL]
        .map(reliable_ordered_channel)
        .into()
}

pub fn get_customized_server_to_client_channels() -> Vec<ChannelConfig> {
    [
        STC_STANDARD_CHANNEL,
        STC_CHUNK_DATA_CHANNEL,
        STC_AUTH_CHANNEL,
    ]
    .map(reliable_ordered_channel)
    .into()
}

pub fn get_shared_renet_config() -> ConnectionConfig {
    ConnectionConfig {
        client_channels_config: get_customized_client_to_server_channels(),
        server_channels_config: get_customized_server_to_client_channels(),
        available_bytes_per_tick: AVAILABLE_BYTES_PER_TICK,
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

    #[test]
    fn each_direction_declares_its_channels_in_id_order() {
        let ids = |channels: Vec<ChannelConfig>| -> Vec<u8> {
            channels.iter().map(|channel| channel.channel_id).collect()
        };
        assert_eq!(ids(get_customized_client_to_server_channels()), [0, 1]);
        assert_eq!(ids(get_customized_server_to_client_channels()), [0, 1, 2]);
    }
}
