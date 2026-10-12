use bevy::prelude::*;
use bevy_renet::RenetClient;
use shared::messages::{ClientToServerMessage, PlayerFrameInput};

use super::buffered_client::PlayerTickInputsBuffer;
use super::SendGameMessageExtension;

/// The inputs sent to the server that it has not acknowledged yet, oldest first, each with
/// the state the client predicted after it.
#[derive(Debug, Default, Resource)]
pub struct UnacknowledgedInputs(pub Vec<PlayerFrameInput>);

pub fn upload_player_inputs_system(
    mut client: ResMut<RenetClient>,
    mut inputs: ResMut<PlayerTickInputsBuffer>,
    mut unacknowledged_inputs: ResMut<UnacknowledgedInputs>,
) {
    if client.is_disconnected() {
        inputs.buffer.clear();
        unacknowledged_inputs.0.clear();
        return;
    }

    let frames = std::mem::take(&mut inputs.buffer);
    unacknowledged_inputs.0.extend(frames.iter().cloned());
    client.send_game_message(ClientToServerMessage::PlayerInputs(frames));
}
