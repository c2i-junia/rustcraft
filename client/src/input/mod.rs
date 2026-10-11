use bevy::prelude::*;

use crate::state::{GameSet, GameState};

pub mod data;
pub mod keyboard;
pub mod mouse;

pub use mouse::*;

/// Mouse and keyboard handling that is not tied to one domain (the key bindings are a resource).
pub fn input_plugin(app: &mut App) {
    app.add_systems(
        Update,
        handle_mouse_system
            .after(GameSet::UiMode)
            .run_if(in_state(GameState::Game)),
    );
}
