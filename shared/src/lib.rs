//! What the client and the server have in common: the messages and their encoding, the
//! world containers, and the player simulation both sides run.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

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

/// The saves of a world its embedded server finished writing, for the client of the same process.
#[derive(Clone, Default, Debug)]
pub struct SaveCounter(Arc<AtomicU64>);

impl SaveCounter {
    pub fn get(&self) -> u64 {
        self.0.load(Ordering::Acquire)
    }

    pub fn bump(&self) {
        self.0.fetch_add(1, Ordering::Release);
    }
}

#[derive(Resource)]
pub struct GameServerConfig {
    pub world_name: String,
    pub is_solo: bool,
    /// Counts the saves written, for the client that runs this server in its own process.
    pub saves: Option<SaveCounter>,
}
