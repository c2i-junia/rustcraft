use bevy::{prelude::*, utils::HashSet};
use serde::{Deserialize, Serialize};

use super::PlayerId;

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone, Eq, Hash)]
pub enum NetworkAction {
    MoveForward,
    MoveRight,
    MoveBackward,
    MoveLeft,
    JumpOrFlyUp,
    SneakOrFlyDown,
    ToggleFlyMode,
}

#[derive(Event, Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct PlayerSpawnEvent {
    pub id: PlayerId,
    pub name: String,
    pub position: Vec3,
}

#[derive(Event, Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct PlayerUpdateEvent {
    pub id: PlayerId,
    pub position: Vec3,
    pub orientation: Quat,
    pub last_ack_time: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct PlayerFrameInput {
    pub time_ms: u64,
    pub delta_ms: u64,
    pub inputs: HashSet<NetworkAction>,
    pub camera: Quat,
    #[serde(skip)]
    pub position: Vec3,
}
