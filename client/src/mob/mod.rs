use bevy::prelude::*;

use crate::state::GameState;

mod models;
mod sync;

pub use sync::*;

/// The entity of a mob, which the maps draw.
#[derive(Debug, Component, Clone)]
pub struct MobRoot;

/// The mobs shown by the client: drawn from the server updates, moving smoothly between them,
/// and playing the clip of what they do.
pub fn mob_plugin(app: &mut App) {
    app.init_resource::<ClientMobs>()
        .init_resource::<MobEntities>()
        .init_resource::<models::MobAnimations>()
        .add_systems(
            Update,
            (
                apply_mob_updates,
                smooth_mob_motion,
                setup_mob_animators,
                play_mob_clips,
            )
                .chain()
                .run_if(in_state(GameState::Game)),
        )
        .add_systems(OnExit(GameState::Game), forget_mobs)
        .add_systems(
            OnTransition {
                exited: GameState::PreGameLoading,
                entered: GameState::Menu,
            },
            forget_mobs,
        );
}
