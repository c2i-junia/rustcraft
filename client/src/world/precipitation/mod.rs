//! The rain and the snow: where they fall (`occlusion.rs`, and the climate map, which tells
//! rain from snow), and how they are drawn (`render.rs`, `precipitation.wgsl`).

use bevy::prelude::*;

use crate::state::GameState;
use occlusion::{forget_occlusion_map, update_occlusion_map_system, OcclusionMap};

pub mod occlusion;

/// The rain and the snow, drawn while in game.
pub fn precipitation_plugin(app: &mut App) {
    app.init_resource::<OcclusionMap>()
        .add_systems(OnEnter(GameState::Game), forget_occlusion_map)
        .add_systems(
            Update,
            update_occlusion_map_system.run_if(in_state(GameState::Game)),
        );
}
