pub mod blocks;
pub mod chunks;
pub mod coords;
pub mod fps;
#[cfg(debug_assertions)]
pub mod inspector;
mod loaded_stats;
pub mod raycast;
pub mod setup;
pub mod targeted_block;

use bevy::prelude::*;

use crate::input::data::GameAction;
use crate::input::keyboard::is_action_just_pressed;
use crate::state::KeyMap;
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
        debug!(
            "Raycast debug mode is now {}",
            self.is_raycast_debug_mode_enabled
        );
    }
}

pub fn toggle_chunk_debug_mode_system(
    mut debug_options: ResMut<DebugOptions>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    key_map: Res<KeyMap>,
) {
    if is_action_just_pressed(GameAction::ToggleChunkDebugMode, &keyboard_input, &key_map) {
        debug_options.toggle_chunk_debug_mode();
    }
}

pub fn toggle_raycast_debug_mode_system(
    mut debug_options: ResMut<DebugOptions>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    key_map: Res<KeyMap>,
) {
    if is_action_just_pressed(
        GameAction::ToggleRaycastDebugMode,
        &keyboard_input,
        &key_map,
    ) {
        debug_options.toggle_raycast_debug_mode();
    }
}
