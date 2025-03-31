pub mod blocks;
pub mod chunks;
pub mod coords;
pub mod fps;
pub mod inspector;
mod loaded_stats;
pub mod raycast;
pub mod setup;
pub mod targeted_block;

use bevy::prelude::Resource;
pub use blocks::*;
pub use chunks::*;
pub use coords::*;
pub use fps::*;
pub use loaded_stats::*;
pub use raycast::*;
pub use setup::*;

#[derive(Resource, Default)]
pub struct DebugOptions {
    is_chunk_debug_mode_enabled: bool,
    is_raycast_debug_mode_enabled: bool,
}

impl DebugOptions {
    pub fn toggle_chunk_debug_mode(&mut self) {
        self.is_chunk_debug_mode_enabled = !self.is_chunk_debug_mode_enabled;
    }

    pub fn toggle_raycast_debug_mode(&mut self) {
        self.is_raycast_debug_mode_enabled = !self.is_raycast_debug_mode_enabled;
        println!(
            "Raycast debug mode is now {}",
            self.is_raycast_debug_mode_enabled
        );
    }
}
