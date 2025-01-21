use crate::network::{SendGameMessageExtension, TargetServer, TargetServerState};
use bevy::prelude::*;
use bevy_renet::renet::RenetClient;
use shared::messages::ClientToServerMessage;

pub fn terminate_server_connection(
    mut client: ResMut<RenetClient>,
    mut target: ResMut<TargetServer>,
) {
    info!("Terminating server connection");
    client.send_game_message(ClientToServerMessage::Exit);

    target.address = None;
    target.username = None;
    target.session_token = None;
    target.state = TargetServerState::Initial;
}
