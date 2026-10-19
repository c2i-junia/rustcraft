//! What the client and the server have in common: the messages and their encoding, the
//! world containers, and the player simulation both sides run.

use bevy_ecs::resource::Resource;

pub mod channels;
pub mod codec;
pub mod constants;
pub mod messages;
pub mod paths;
pub mod players;
pub mod status;
pub mod utils;
pub mod world;

pub use channels::*;
pub use codec::*;
pub use constants::*;
pub use paths::*;

#[derive(Resource)]
pub struct GameServerConfig {
    pub world_name: String,
    pub is_solo: bool,
}
