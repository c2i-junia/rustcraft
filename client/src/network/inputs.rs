use bevy::prelude::*;
use bevy_renet::renet::RenetClient;
use shared::messages::{ClientToServerMessage, PlayerFrameInput};

use super::buffered_client::PlayerTickInputsBuffer;
use super::SendGameMessageExtension;

// vector of time_ms values for inputs that have not been acknowledged by the server
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

    let mut frames = vec![];
    for input in inputs.buffer.iter() {
        frames.push(input.clone());
        unacknowledged_inputs.0.push(input.clone());
    }
    // for frame in frames.iter() {
    //     debug!(
    //         "Sending input: {:?} | {:?} | {:?}",
    //         frame.time_ms, frame.inputs, frame.position
    //     );
    // }
    client.send_game_message(ClientToServerMessage::PlayerInputs(frames));
    inputs.buffer.clear();
}
