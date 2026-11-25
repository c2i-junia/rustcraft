use bevy::prelude::*;

use crate::state::GameState;

mod hearts;
mod hurt;
mod models;
mod sync;

pub use hurt::MobHurt;
pub use sync::*;

/// The entity of a mob, which the maps draw.
#[derive(Debug, Component, Clone)]
pub struct MobRoot;

/// The mobs shown by the client: drawn from the server updates, moving smoothly between them,
/// playing the clip of what they do, and flashing red when hurt.
pub fn mob_plugin(app: &mut App) {
    app.add_message::<MobHurt>()
        .init_resource::<ClientMobs>()
        .init_resource::<MobEntities>()
        .init_resource::<models::MobAnimations>()
        .init_resource::<hurt::HurtMaterials>()
        .init_resource::<hearts::Hearts>()
        .add_systems(
            Update,
            (
                apply_mob_updates,
                scale_mob_models,
                hearts::show_hearts,
                hurt::flash_hurt_mobs,
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
