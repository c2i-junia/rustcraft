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

impl SendGameMessageExtension for RenetServer {
    fn send_game_message(&mut self, client_id: ClientId, message: ServerToClientMessage) {
        let channel = message.get_channel_id();
        let payload = shared::game_message_to_payload(message);
        let size = payload.len() as u64;
        if size > (10 * 1024) {
            info!("Sending game message of size: {}", format_bytes(size));
        }
        self.send_message(client_id, channel, payload);
    }

    fn broadcast_game_message(&mut self, message: ServerToClientMessage) {
        let channel = message.get_channel_id();
        let payload = shared::game_message_to_payload(message);
        let size = payload.len() as u64;
        if size > (10 * 1024) {
            info!("Broadcasting game message of size: {}", format_bytes(size));
        }
        self.broadcast_message(channel, payload);
    }

    fn receive_game_message_by_channel(
        &mut self,
        client_id: ClientId,
        channel: u8,
    ) -> Option<Result<ClientToServerMessage, Box<bincode::ErrorKind>>> {
        let payload = self.receive_message(client_id, channel);
        if let Some(payload) = payload {
            // debug!("Received payload: {:?}", payload);
            let msg = shared::payload_to_game_message::<shared::messages::ClientToServerMessage>(
                &payload,
            );
            match msg {
                Ok(msg) => {
                    // info!("Received message: {:?}", msg);
                    return Some(Ok(msg));
                }
                Err(e) => {
                    warn!("Error deserializing message: {:?}", e);
                    return Some(Err(e));
                }
            }
        }
        None
    }

    fn receive_game_message(
        &mut self,
        client_id: ClientId,
    ) -> Option<Result<ClientToServerMessage, Box<bincode::ErrorKind>>> {
        let channels = get_customized_client_to_server_channels();
        for channel in channels {
            let res = self.receive_game_message_by_channel(client_id, channel.channel_id);
            if res.is_some() {
                return res;
            }
        }
        None
    }
}
