use bevy::prelude::*;
use bevy_platform::collections::HashSet;
use serde::{Deserialize, Serialize};

use super::PlayerId;
use crate::players::game_mode::{DeathCause, GameMode};
use crate::players::{inventory::InventoryAction, Inventory, MovementState, ViewMode};
use crate::world::Container;

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
    /// Sent while the player sprints. It starts a sprint while moving forward, which the
    /// simulation keeps as long as the player moves forward and does not sneak.
    Sprint,
}

#[derive(Serialize, Deserialize, Default, PartialEq, Debug, Clone)]
pub struct PlayerSave {
    pub position: Vec3,
    pub camera_transform: Transform,
    pub is_flying: bool,
}

#[derive(Message, Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct PlayerSpawnEvent {
    pub id: PlayerId,
    pub name: String,
    pub data: PlayerSave,
}

#[derive(Message, Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct PlayerUpdateEvent {
    pub id: PlayerId,
    /// The state of the player once the server has simulated the input `last_ack_time`.
    pub movement: MovementState,
    pub orientation: Quat,
    pub last_ack_time: u64,
    /// The inventory of the player, sent to that player only: the others never show it.
    pub inventory: Option<Inventory>,
    /// What the chest `inventory.open_chest` holds once the server has simulated the input
    /// `last_ack_time`: the client takes it with the inventory. Boxed, as it is larger than
    /// every other message. Sent to that player only, as the inventory.
    pub open_chest: Option<Box<Container>>,
    pub health: u32,
    pub game_mode: GameMode,
    /// What killed the player, while it is dead.
    pub death_cause: Option<DeathCause>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct PlayerFrameInput {
    pub time_ms: u64,
    pub delta_ms: u64,
    pub inputs: HashSet<NetworkAction>,
    pub camera: Transform,
    pub hotbar_slot: u32,
    pub view_mode: ViewMode,
    pub inventory_actions: Vec<InventoryAction>,
    /// Client only: the state it predicted after this input.
    #[serde(skip)]
    pub predicted: MovementState,
}

impl PlayerFrameInput {
    pub fn is_pressed(&self, action: NetworkAction) -> bool {
        self.inputs.contains(&action)
    }
}
