use bevy::prelude::*;

use crate::state::GameState;
use buffered_client::{CurrentFrameInputs, PlayerTickInputsBuffer, SyncTime};

pub mod buffered_client;
mod chat;
mod cleanup;
pub mod extensions;
mod inputs;
pub mod save;
mod setup;
mod world;

pub use chat::*;
pub use cleanup::*;
pub use extensions::SendGameMessageExtension;
pub use inputs::*;
pub use setup::*;

/// The connection to the server: the client buffers, the polling of its messages, the upload of
/// the player inputs and the stop of the embedded server. The transport itself is set up by
/// `add_base_netcode`.
pub fn network_plugin(app: &mut App) {
    app.init_resource::<CurrentPlayerProfile>()
        .init_resource::<PlayerTickInputsBuffer>()
        .init_resource::<CurrentFrameInputs>()
        .init_resource::<SyncTime>()
        .init_resource::<UnacknowledgedInputs>()
        .add_observer(network_failure_handler)
        .add_systems(
            Update,
            establish_authenticated_connection_to_server
                .run_if(in_state(GameState::PreGameLoading)),
        )
        .add_systems(
            FixedPreUpdate,
            poll_network_messages.run_if(in_state(GameState::Game)),
        )
        .add_systems(
            FixedUpdate,
            (upload_player_inputs_system).run_if(in_state(GameState::Game)),
        )
        .add_systems(
            Update,
            finish_stopping_local_server.run_if(in_state(GameState::Menu)),
        );
}
