use bevy::prelude::*;

use crate::state::GameState;

mod fishing;
mod hearts;
mod hurt;
mod leads;
mod models;
mod potions;
mod strikes;
mod sync;

pub use hurt::MobHurt;
pub use sync::*;

/// The entity of a mob, which the maps draw.
#[derive(Debug, Component, Clone)]
pub struct MobRoot;

/// The mobs shown by the client: drawn from the server updates, moving smoothly between them,
/// playing the clip of what they do, flashing red when hurt, and sparking at a special blow.
pub fn mob_plugin(app: &mut App) {
    app.add_message::<MobHurt>()
        .add_message::<fishing::BobberBite>()
        .init_resource::<fishing::Bubbles>()
        .add_message::<potions::PotionBurst>()
        .init_resource::<potions::Drops>()
        .init_resource::<ClientMobs>()
        .init_resource::<MobEntities>()
        .init_resource::<models::MobAnimations>()
        .init_resource::<hurt::HurtMaterials>()
        .init_resource::<hearts::Hearts>()
        .init_resource::<strikes::StrikeSparks>()
        .add_systems(
            Update,
            (
                apply_mob_updates,
                strikes::show_strikes,
                scale_mob_models,
                hearts::show_hearts,
                hurt::flash_hurt_mobs,
                smooth_mob_motion,
                place_ridden_mount,
                show_saddles,
                setup_mob_animators,
                play_mob_clips,
                fishing::show_bites,
                potions::show_bursts,
                fishing::draw_lines,
                leads::draw_leads,
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
