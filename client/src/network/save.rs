use crate::network::SendGameMessageExtension;
use bevy::prelude::*;
use bevy_renet::renet::RenetClient;

// Send save request to server
pub fn send_save_request_to_server(client: &mut ResMut<RenetClient>) {
    client.send_game_message(shared::messages::ClientToServerMessage::SaveWorldRequest);
    debug!("Save request sent to server.");
}
