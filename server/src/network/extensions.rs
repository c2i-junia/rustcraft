use bevy::prelude::*;
use bevy_renet::renet::{ClientId, RenetServer};
use shared::{
    get_customized_client_to_server_channels,
    messages::{ClientToServerMessage, ServerToClientMessage},
    utils::format_bytes,
    ChannelResolvableExt,
};

pub trait SendGameMessageExtension {
    fn send_game_message(&mut self, client_id: ClientId, message: ServerToClientMessage);
    fn broadcast_game_message(&mut self, message: ServerToClientMessage);
    fn broadcast_game_message_except(
        &mut self,
        client_id: ClientId,
        message: ServerToClientMessage,
    );
    fn receive_game_message(
        &mut self,
        client_id: ClientId,
    ) -> Option<Result<ClientToServerMessage, Box<bincode::ErrorKind>>>;
    fn receive_game_message_by_channel(
        &mut self,
        client_id: ClientId,
        channel: u8,
    ) -> Option<Result<ClientToServerMessage, Box<bincode::ErrorKind>>>;
}

/// Messages above this size are logged when sent: 10 KiB.
const LARGE_MESSAGE_SIZE: u64 = 10 * 1024;

impl SendGameMessageExtension for RenetServer {
    fn send_game_message(&mut self, client_id: ClientId, message: ServerToClientMessage) {
        let channel = message.get_channel_id();
        let payload = shared::game_message_to_payload(message);
        let size = payload.len() as u64;
        if size > LARGE_MESSAGE_SIZE {
            info!("Sending game message of size: {}", format_bytes(size));
        }
        self.send_message(client_id, channel, payload);
    }

    fn broadcast_game_message(&mut self, message: ServerToClientMessage) {
        let channel = message.get_channel_id();
        let payload = shared::game_message_to_payload(message);
        let size = payload.len() as u64;
        if size > LARGE_MESSAGE_SIZE {
            info!("Broadcasting game message of size: {}", format_bytes(size));
        }
        self.broadcast_message(channel, payload);
    }

    fn broadcast_game_message_except(
        &mut self,
        client_id: ClientId,
        message: ServerToClientMessage,
    ) {
        let channel = message.get_channel_id();
        let payload = shared::game_message_to_payload(message);
        self.broadcast_message_except(client_id, channel, payload);
    }

    fn receive_game_message_by_channel(
        &mut self,
        client_id: ClientId,
        channel: u8,
    ) -> Option<Result<ClientToServerMessage, Box<bincode::ErrorKind>>> {
        let payload = self.receive_message(client_id, channel)?;
        let message = shared::payload_to_game_message_limited::<ClientToServerMessage>(
            &payload,
            shared::MAX_CLIENT_MESSAGE_SIZE,
        );
        if let Err(err) = &message {
            warn!("Error deserializing message: {:?}", err);
        }
        Some(message)
    }

    fn receive_game_message(
        &mut self,
        client_id: ClientId,
    ) -> Option<Result<ClientToServerMessage, Box<bincode::ErrorKind>>> {
        get_customized_client_to_server_channels()
            .into_iter()
            .find_map(|channel| self.receive_game_message_by_channel(client_id, channel.channel_id))
    }
}
