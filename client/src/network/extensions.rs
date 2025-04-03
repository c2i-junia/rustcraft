use bevy::prelude::*;
use bevy_renet::renet::RenetClient;
use bincode::ErrorKind;
use shared::{
    game_message_to_payload, get_customized_server_to_client_channels,
    messages::{ClientToServerMessage, ServerToClientMessage},
    ChannelResolvableExt,
};

pub trait SendGameMessageExtension {
    fn send_game_message(&mut self, message: ClientToServerMessage);
    fn receive_game_message_by_channel(
        &mut self,
        channel: u8,
    ) -> Option<Result<ServerToClientMessage, Box<ErrorKind>>>;
    fn receive_game_message_except_channel(
        &mut self,
        channel: u8,
    ) -> Option<Result<ServerToClientMessage, Box<ErrorKind>>>;
    // fn receive_game_message(&mut self) -> Option<Result<ServerToClientMessage, Box<ErrorKind>>>;
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
            // debug!("Received payload: {:?}", payload);
            let res = shared::payload_to_game_message::<ServerToClientMessage>(&payload);
            match res {
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

    fn receive_game_message_except_channel(
        &mut self,
        excluded_channel_id: u8,
    ) -> Option<Result<ServerToClientMessage, Box<ErrorKind>>> {
        let channels = get_customized_server_to_client_channels();
        for channel in channels {
            if channel.channel_id == excluded_channel_id {
                continue;
            }
            let res = self.receive_game_message_by_channel(channel.channel_id);
            if let Some(res) = res {
                return Some(res);
            }
        }
        None
    }

    // fn receive_game_message(&mut self) -> Option<Result<ServerToClientMessage, Box<ErrorKind>>> {
    //     let channels = get_customized_server_to_client_channels();
    //     for channel in channels {
    //         let res = self.receive_game_message_by_channel(channel.channel_id);
    //         if let Some(res) = res {
    //             return Some(res);
    //         }
    //     }
    //     None
    // }
}
