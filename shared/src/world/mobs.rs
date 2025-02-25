use bevy::math::{Quat, Vec3};
use serde::{Deserialize, Serialize};

use crate::messages::PlayerId;

pub type MobId = u128;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum MobKind {
    Fox,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum MobTarget {
    None,
    Position(Vec3),
    Player(PlayerId),
    Mob(MobId),
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum MobAction {
    Idle,
    Walk,
    Attack,
    Flee,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ServerMob {
    pub kind: MobKind,
    pub target: MobTarget,
    pub action: MobAction,
    pub position: Vec3,
    pub rotation: Quat,
}
