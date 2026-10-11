use bevy::prelude::*;

mod fox;
mod spawn;

pub use fox::*;
pub use spawn::*;

#[derive(Debug, Component, Clone)]
pub struct MobRoot {
    pub id: u128,
}
