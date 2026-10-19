use bevy::prelude::*;
use bevy_renet::RenetClient;

use crate::state::GameState;
use buffered_client::{CurrentFrameInputs, PlayerTickInputsBuffer, SyncTime};

pub mod address;
pub mod buffered_client;
mod chat;
mod cleanup;
pub mod extensions;
mod inputs;
pub mod leave;
pub mod save;
pub mod session;
mod setup;
pub mod status;
mod world;

pub use chat::*;
pub use cleanup::*;
pub use extensions::SendGameMessageExtension;
pub use inputs::*;
pub use setup::*;

/// Whether the server accepted the player: `ConnectionEstablished` or `FullyReady`.
pub fn authenticated(target: Res<TargetServer>) -> bool {
    matches!(
        target.state,
        TargetServerState::ConnectionEstablished | TargetServerState::FullyReady
    )
}

/// The connection to the server: the watch of the session being opened, the client buffers, the
/// polling of its messages, the upload of the player inputs and the stop of the embedded server.
/// The transport itself is set up by `add_base_netcode`.
pub fn network_plugin(app: &mut App) {
    session::session_plugin(app);
    leave::leave_plugin(app);
    app.init_resource::<CurrentPlayerProfile>()
        .init_resource::<PlayerTickInputsBuffer>()
        .init_resource::<CurrentFrameInputs>()
        .init_resource::<SyncTime>()
        .init_resource::<UnacknowledgedInputs>()
        .add_observer(network_failure_handler)
        // a session that ended before it connected (a name no server accepts, a world still
        // closing) has no connection: the last session's was removed when it ended
        .add_systems(
            Update,
            establish_authenticated_connection_to_server.run_if(
                in_state(GameState::PreGameLoading).and_then(resource_exists::<RenetClient>),
            ),
        )
        // the chunks around the spawn come in before the game
        .add_systems(
            FixedPreUpdate,
            poll_network_messages.run_if(
                in_state(GameState::Game)
                    .or_else(in_state(GameState::PreGameLoading).and_then(authenticated)),
            ),
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
