//! What the client and the server have in common: the messages and their encoding, the
//! world containers, and the player simulation both sides run.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use bevy_ecs::resource::Resource;

pub mod channels;
pub mod codec;
pub mod commands;
pub mod constants;
pub mod messages;
pub mod mods;
pub mod paths;
pub mod players;
pub mod status;
pub mod utils;
pub mod world;

pub use channels::*;
pub use codec::*;
pub use constants::*;
pub use paths::*;

/// The saves of a world its embedded server finished writing, and the ones it failed to write
/// in full, for the client of the same process.
#[derive(Clone, Default, Debug)]
pub struct SaveCounter {
    saved: Arc<AtomicU64>,
    failed: Arc<AtomicU64>,
}

impl SaveCounter {
    /// The saves written in full.
    pub fn get(&self) -> u64 {
        self.saved.load(Ordering::Acquire)
    }

    pub fn bump(&self) {
        self.saved.fetch_add(1, Ordering::Release);
    }

    /// The saves of which something could not be written.
    pub fn failures(&self) -> u64 {
        self.failed.load(Ordering::Acquire)
    }

    pub fn bump_failure(&self) {
        self.failed.fetch_add(1, Ordering::Release);
    }
}

#[derive(Resource)]
pub struct GameServerConfig {
    pub world_name: String,
    pub is_solo: bool,
    /// Counts the saves written and failed, for the client that runs this server in its own
    /// process.
    pub saves: Option<SaveCounter>,
    /// The largest render distance a player may ask for, in chunks.
    pub max_render_distance: u32,
}
