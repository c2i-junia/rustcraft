use bevy::prelude::*;
use bevy_platform::collections::HashSet;
use serde::{Deserialize, Serialize};

use super::PlayerId;
use crate::players::{Inventory, ViewMode};

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone, Eq, Hash)]
pub enum NetworkAction {
    MoveForward,
    MoveRight,
    MoveBackward,
    MoveLeft,
    JumpOrFlyUp,
    SneakOrFlyDown,
    ToggleFlyMode,
    LeftClick,
    RightClick,
}

#[derive(Serialize, Deserialize, Default, PartialEq, Debug, Clone)]
pub struct PlayerSave {
    pub position: Vec3,
    pub camera_transform: Transform,
    pub is_flying: bool,
}

#[derive(Event, Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct PlayerSpawnEvent {
    pub id: PlayerId,
    pub name: String,
    pub data: PlayerSave,
}

#[derive(Event, Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct PlayerUpdateEvent {
    pub id: PlayerId,
    pub position: Vec3,
    pub orientation: Quat,
    pub last_ack_time: u64,
    pub inventory: Inventory,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct PlayerFrameInput {
    pub time_ms: u64,
    pub delta_ms: u64,
    pub inputs: HashSet<NetworkAction>,
    pub camera: Transform,
    pub hotbar_slot: u32,
    pub view_mode: ViewMode,
    #[serde(skip)]
    pub position: Vec3,
}
