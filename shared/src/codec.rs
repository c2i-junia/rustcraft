//! How a message becomes bytes on the wire and back: bincode, then lz4 with a size prefix.
//! The decoders bound the decompressed size before allocating anything.

use bevy_log::debug;
use bincode::Options;

use crate::utils::format_bytes;

pub fn game_message_to_payload<T: serde::Serialize>(message: T) -> Vec<u8> {
    // Neither step can fail on a message of the game: they write to memory, and lz4 only
    // refuses inputs of about 2 GiB
    let payload = bincode::options()
        .serialize(&message)
        .expect("a game message serializes to memory");
    let output = lz4::block::compress(&payload, None, true)
        .expect("a game message is far below the lz4 size limit");
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::channels::{ChannelResolvableExt, STC_CHUNK_DATA_CHANNEL};
    use crate::messages::{
        ChatMessageRequest, ClientToServerMessage, FullChatMessage, ServerToClientMessage,
    };

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
    fn chunk_unloads_round_trip_on_the_world_channel() {
        use bevy::math::IVec3;

        let chunks = vec![IVec3::new(-4, 0, 7), IVec3::new(3, -1, 2)];
        let message = ServerToClientMessage::ChunkUnloads(chunks.clone());
        assert_eq!(message.get_channel_id(), STC_CHUNK_DATA_CHANNEL);

        let payload = game_message_to_payload(message);
        match payload_to_game_message::<ServerToClientMessage>(&payload).unwrap() {
            ServerToClientMessage::ChunkUnloads(decoded) => assert_eq!(decoded, chunks),
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
}
