use bevy::prelude::*;
use bevy_renet::renet::RenetClient;
use bincode::ErrorKind;
use shared::{
    game_message_to_payload,
    messages::{ClientToServerMessage, ServerToClientMessage},
    ChannelResolvableExt,
};

pub trait SendGameMessageExtension {
    fn send_game_message(&mut self, message: ClientToServerMessage);
    fn receive_game_message_by_channel(
        &mut self,
        channel: u8,
    ) -> Option<Result<ServerToClientMessage, Box<ErrorKind>>>;
}

impl SendGameMessageExtension for RenetClient {
    fn send_game_message(&mut self, message: ClientToServerMessage) {
        let ch = message.get_channel_id();
        let payload = game_message_to_payload(message);
        self.send_message(ch, payload);
    }

    fn receive_game_message_by_channel(
        &mut self,
        channel: u8,
    ) -> Option<Result<ServerToClientMessage, Box<ErrorKind>>> {
        let payload = self.receive_message(channel);
        if let Some(payload) = payload {
            let res = shared::payload_to_game_message::<ServerToClientMessage>(&payload);
            match res {
                Ok(msg) => {
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
}
