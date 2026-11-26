use bevy::prelude::*;
use bevy_platform::collections::HashSet;
use serde::{Deserialize, Serialize};

use super::PlayerId;
use crate::players::armour::WornArmour;
use crate::players::experience::Experience;
use crate::players::game_mode::{DeathCause, GameMode};
use crate::players::skin::PlayerSkin;
use crate::players::{inventory::InventoryAction, Held, Inventory, MovementState, ViewMode};
use crate::world::{Container, Furnace};

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
    /// Sent while a UI is open: a bow being drawn is let down without shooting.
    LowerBow,
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
    /// The skin the player wears, which the clients cut its model from.
    pub skin: PlayerSkin,
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
    /// What the furnace `inventory.open_furnace` holds once the server has simulated the input
    /// `last_ack_time`, as `open_chest`.
    pub open_furnace: Option<Furnace>,
    pub health: u32,
    /// The ticks of air the player has left (`Player::air`).
    pub air: u16,
    /// The food of the player (`Player::food`): the client's prediction refuses the sprint
    /// with it as the server does.
    pub food: u8,
    /// The levels of the player (`Player::experience`), for its bar.
    pub experience: Experience,
    /// The ticks the player has left to burn (`Player::burning_ticks`).
    pub burning_ticks: u16,
    pub game_mode: GameMode,
    /// What killed the player, while it is dead.
    pub death_cause: Option<DeathCause>,
    /// Whether the player swings its arm (`Player::is_swinging`): the others see it strike.
    pub swinging: bool,
    /// The item in the player's hand, from its selected hotbar slot (`Inventory::in_hand`): the
    /// others see it held, and its glint when it is enchanted.
    pub held: Option<Held>,
    /// The armour the player wears (`Inventory::worn`): the others see it on its model.
    pub worn: WornArmour,
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
