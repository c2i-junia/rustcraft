use bevy::prelude::*;

mod fox;
mod spawn;

pub use fox::*;
pub use spawn::*;

#[derive(Debug, Component, Clone)]
pub struct MobRoot {
    #[allow(dead_code)]
    pub name: String,
    #[allow(dead_code)]
    pub id: u128,
}

#[derive(Debug, Component, Clone)]
pub struct MobMarker {
    #[allow(dead_code)]
    pub name: String,
    pub id: u128,
}

#[derive(Debug, Clone)]
pub struct TargetedMobData {
    #[allow(dead_code)]
    pub name: String,
    pub id: u128,
    //pub entity: Entity,
}

#[derive(Debug, Resource, Clone, Default)]
pub struct TargetedMob {
    pub target: Option<TargetedMobData>,
}
