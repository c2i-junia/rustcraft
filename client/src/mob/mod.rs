use bevy::prelude::*;
use shared::messages::mob::MobUpdateEvent;

use crate::state::GameState;

mod fox;
mod spawn;

pub use fox::*;
pub use spawn::*;

#[derive(Debug, Component, Clone)]
pub struct MobRoot {
    pub id: u128,
}

/// The mobs shown by the client: spawn from the server updates, and the fox animation.
pub fn mob_plugin(app: &mut App) {
    app.init_resource::<ParticleAssets>()
        .init_resource::<FoxFeetTargets>()
        .init_resource::<Animations>()
        .add_message::<MobUpdateEvent>()
        .add_observer(observe_on_step)
        .add_systems(
            Update,
            (setup_fox_once_loaded, simulate_particles).run_if(in_state(GameState::Game)),
        )
        .add_systems(Update, spawn_mobs_system.run_if(in_state(GameState::Game)));
}
