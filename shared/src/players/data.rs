use std::ops::Range;

use bevy::{
    math::{IVec3, Vec3},
    prelude::{Component, Resource, Transform},
};
use bevy_platform::collections::HashMap;
use serde::{Deserialize, Serialize};

use super::game_mode::{DeathCause, GameMode, MAX_HEALTH};
use crate::{
    messages::PlayerId,
    world::{ItemId, ItemStack, ItemType},
    MAX_INVENTORY_SLOTS,
};

#[derive(Debug, Resource, Clone, Serialize, Deserialize, PartialEq)]
pub struct Inventory {
    pub inner: HashMap<u32, ItemStack>,
    /// The stack held by the cursor while the inventory is open.
    #[serde(default)]
    pub floating: Option<ItemStack>,
    /// The crafting table the player is using, if any.
    #[serde(default)]
    pub open_table: Option<IVec3>,
    /// The chest the player is using, if any. Never at the same time as a table.
    #[serde(default)]
    pub open_chest: Option<IVec3>,
    /// The furnace, lit or out, the player is using, if any. Never at the same time as a table
    /// or a chest.
    #[serde(default)]
    pub open_furnace: Option<IVec3>,
}

impl Default for Inventory {
    fn default() -> Self {
        Self::new()
    }
}

impl Inventory {
    pub fn new() -> Self {
        Self {
            inner: HashMap::new(),
            floating: None,
            open_table: None,
            open_chest: None,
            open_furnace: None,
        }
    }

    /// Adds `stack` to the inventory: first on the stacks of the same item, then in empty slots,
    /// in slot order. Returns how many items did not fit; those are lost.
    pub fn add_item_to_inventory(&mut self, stack: ItemStack) -> u32 {
        self.add_item_to_range(stack, 0..MAX_INVENTORY_SLOTS)
    }

    /// Adds `stack` to the slots of `range`: first on stacks of the same item, then in empty
    /// slots, in slot order. Returns how many items did not fit.
    pub fn add_item_to_range(&mut self, mut stack: ItemStack, range: Range<u32>) -> u32 {
        let max_stack = stack.item_id.get_max_stack();
        for tops_up in [true, false] {
            for slot in range.clone() {
                if stack.nb == 0 {
                    return 0;
                }
                let matches = match self.inner.get(&slot) {
                    Some(held) => tops_up && held.item_id == stack.item_id,
                    None => !tops_up,
                };
                if !matches {
                    continue;
                }
                let held = self.inner.get(&slot).map_or(0, |held| held.nb);
                let added = stack.nb.min(max_stack.saturating_sub(held));
                if added > 0 {
                    self.inner.insert(
                        slot,
                        ItemStack {
                            nb: held + added,
                            ..stack
                        },
                    );
                    stack.nb -= added;
                }
            }
        }
        stack.nb
    }

    /// Add items to stack at specified position\
    /// Stacks cannot exceed the item's `get_max_stack()` number of items\
    /// Returns number of items really added to the stack
    pub fn add_item_to_stack(
        &mut self,
        stack: u32,
        mut nb: u32,
        id: ItemId,
        item_type: ItemType,
    ) -> u32 {
        let item_option = self.inner.get(&stack);
        let mut new_item = ItemStack {
            item_id: id,
            nb,
            item_type,
        };

        if let Some(item) = item_option {
            if nb + item.nb > item.item_id.get_max_stack() {
                nb = item.item_id.get_max_stack() - item.nb;
            }
            // The stack keeps its own type: a tool its wear
            new_item = ItemStack {
                nb: nb + item.nb,
                ..*item
            };
        }
        self.inner.insert(stack, new_item);
        nb
    }

    /// Adds `wear` to the tool in `slot`. A tool worn down to its durability breaks: it leaves
    /// the slot, and `true` is returned. Anything else in the slot is left alone.
    pub fn wear_tool(&mut self, slot: u32, wear: u32) -> bool {
        let Some(stack) = self.inner.get_mut(&slot) else {
            return false;
        };
        let (Some(tool), ItemType::Tool { wear: worn }) = (stack.item_id.tool(), stack.item_type)
        else {
            return false;
        };
        let worn = worn.saturating_add(wear);
        if worn < tool.durability {
            stack.item_type = ItemType::Tool { wear: worn };
            return false;
        }
        self.inner.remove(&slot);
        true
    }

    /// Removes items from stack at specified position\
    /// Stacks cannot have < 0 number of items\
    /// Returns number of items really removed from the stack
    pub fn remove_item_from_stack(&mut self, stack: u32, mut nb: u32) -> u32 {
        let item_option = self.inner.get(&stack);

        if let Some(&item) = item_option {
            if nb >= item.nb {
                nb = item.nb;
                self.inner.remove(&stack);
            } else {
                self.inner.insert(
                    stack,
                    ItemStack {
                        item_id: item.item_id,
                        nb: item.nb - nb,
                        item_type: item.item_type,
                    },
                );
            }
            return nb;
        }
        0
    }
}

#[derive(Component, Clone, Serialize, Deserialize, Debug)]
pub struct Player {
    pub id: PlayerId,
    pub name: String,
    pub position: Vec3,
    pub camera_transform: Transform,
    pub velocity: Vec3,
    pub on_ground: bool,
    pub is_flying: bool,
    #[serde(default)]
    pub is_sprinting: bool,
    #[serde(default)]
    pub is_sneaking: bool,
    #[serde(default)]
    pub is_swimming: bool,
    /// Blocks fallen since the player last stood on the ground, swam or flew.
    #[serde(default)]
    pub fall_distance: f32,
    #[serde(default)]
    pub game_mode: GameMode,
    /// From 0, dead, to `MAX_HEALTH`. Only the server changes it.
    #[serde(default = "full_health")]
    pub health: u32,
    /// What killed the player, while it is dead.
    #[serde(default)]
    pub death_cause: Option<DeathCause>,
    pub inventory: Inventory,
    pub height: f32,
    pub width: f32,
    pub last_input_processed: u64,
    /// Time the left button was held that did not add up to a mining hit yet, in ms.
    #[serde(skip)]
    pub mining_carry_ms: u64,
    /// Time the right button must still be held before the next block is placed, in ms.
    #[serde(skip)]
    pub place_wait_ms: u64,
}

/// What the movement simulation carries over from one input to the next. The server sends it
/// with each acknowledgement, so that a client resumes its prediction from the very same state.
#[derive(Serialize, Deserialize, PartialEq, Debug, Clone, Copy, Default)]
pub struct MovementState {
    pub position: Vec3,
    pub velocity: Vec3,
    pub on_ground: bool,
    pub is_flying: bool,
    pub is_sprinting: bool,
    pub is_sneaking: bool,
    pub is_swimming: bool,
    pub fall_distance: f32,
}

fn full_health() -> u32 {
    MAX_HEALTH
}

/// Size of a player's collision box, in blocks.
pub const PLAYER_HEIGHT: f32 = 1.8;
pub const PLAYER_WIDTH: f32 = 0.8;

impl Player {
    pub fn movement_state(&self) -> MovementState {
        MovementState {
            position: self.position,
            velocity: self.velocity,
            on_ground: self.on_ground,
            is_flying: self.is_flying,
            is_sprinting: self.is_sprinting,
            is_sneaking: self.is_sneaking,
            is_swimming: self.is_swimming,
            fall_distance: self.fall_distance,
        }
    }

    pub fn set_movement_state(&mut self, state: MovementState) {
        self.position = state.position;
        self.velocity = state.velocity;
        self.on_ground = state.on_ground;
        self.is_flying = state.is_flying;
        self.is_sprinting = state.is_sprinting;
        self.is_sneaking = state.is_sneaking;
        self.is_swimming = state.is_swimming;
        self.fall_distance = state.fall_distance;
    }

    pub fn is_dead(&self) -> bool {
        self.health == 0
    }

    /// Takes `damage` points of health, if the game mode lets it, and dies of `cause` when none
    /// is left. A dead player takes no more.
    pub fn hurt(&mut self, damage: u32, cause: DeathCause) {
        if damage == 0 || self.is_dead() || !self.game_mode.takes_damage() {
            return;
        }
        self.health = self.health.saturating_sub(damage);
        if self.is_dead() {
            self.death_cause = Some(cause);
        }
    }

    /// Brings a dead player back to life at `spawn`, standing still and in full health.
    pub fn respawn(&mut self, spawn: Vec3) {
        self.set_movement_state(MovementState {
            position: spawn,
            on_ground: true,
            is_flying: self.is_flying && self.game_mode.can_fly(),
            ..Default::default()
        });
        self.health = MAX_HEALTH;
        self.death_cause = None;
    }

    pub fn new(id: PlayerId, name: String, position: Vec3, camera_transform: Transform) -> Self {
        Self {
            id,
            name,
            position,
            camera_transform,
            velocity: Vec3::ZERO,
            on_ground: true,
            is_flying: false,
            is_sprinting: false,
            is_sneaking: false,
            is_swimming: false,
            fall_distance: 0.0,
            game_mode: GameMode::default(),
            health: MAX_HEALTH,
            death_cause: None,
            inventory: Inventory::new(),
            height: PLAYER_HEIGHT,
            width: PLAYER_WIDTH,
            last_input_processed: 0,
            mining_carry_ms: 0,
            place_wait_ms: 0,
        }
    }
}

impl Default for Player {
    fn default() -> Self {
        Self::new(0, "Default".into(), Vec3::ZERO, Transform::default())
    }
}

#[derive(Debug, PartialEq, Clone, Copy, Resource, Serialize, Deserialize, Default)]
pub enum ViewMode {
    #[default]
    FirstPerson,
    ThirdPerson,
}

impl ViewMode {
    pub fn toggle(&mut self) {
        *self = match *self {
            ViewMode::FirstPerson => ViewMode::ThirdPerson,
            ViewMode::ThirdPerson => ViewMode::FirstPerson,
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stack(item_id: ItemId, nb: u32) -> ItemStack {
        ItemStack {
            item_id,
            item_type: item_id.get_default_type(),
            nb,
        }
    }

    fn max_stack() -> u32 {
        ItemId::Dirt.get_max_stack()
    }

    #[test]
    fn add_to_empty_inventory_uses_first_slot() {
        let mut inventory = Inventory::new();
        inventory.add_item_to_inventory(stack(ItemId::Dirt, 5));
        assert_eq!(inventory.inner.len(), 1);
        assert_eq!(inventory.inner[&0], stack(ItemId::Dirt, 5));
    }

    #[test]
    fn add_stacks_onto_existing_partial_stack() {
        let mut inventory = Inventory::new();
        inventory.add_item_to_inventory(stack(ItemId::Dirt, 5));
        inventory.add_item_to_inventory(stack(ItemId::Dirt, 7));
        assert_eq!(inventory.inner.len(), 1);
        assert_eq!(inventory.inner[&0], stack(ItemId::Dirt, 12));
    }

    #[test]
    fn add_different_item_goes_to_next_slot() {
        let mut inventory = Inventory::new();
        inventory.add_item_to_inventory(stack(ItemId::Dirt, 5));
        inventory.add_item_to_inventory(stack(ItemId::Sand, 2));
        assert_eq!(inventory.inner[&0], stack(ItemId::Dirt, 5));
        assert_eq!(inventory.inner[&1], stack(ItemId::Sand, 2));
    }

    #[test]
    fn add_overflowing_stack_spills_into_next_slot() {
        let mut inventory = Inventory::new();
        inventory.add_item_to_inventory(stack(ItemId::Dirt, max_stack() - 4));
        inventory.add_item_to_inventory(stack(ItemId::Dirt, 10));
        assert_eq!(inventory.inner.len(), 2);
        assert_eq!(inventory.inner[&0], stack(ItemId::Dirt, max_stack()));
        assert_eq!(inventory.inner[&1], stack(ItemId::Dirt, 6));
    }

    #[test]
    fn add_more_than_a_stack_to_empty_inventory_spills_into_next_slot() {
        let mut inventory = Inventory::new();
        inventory.add_item_to_inventory(stack(ItemId::Dirt, max_stack() + 36));
        assert_eq!(inventory.inner[&0], stack(ItemId::Dirt, max_stack()));
        assert_eq!(inventory.inner[&1], stack(ItemId::Dirt, 36));
    }

    #[test]
    fn add_returns_what_did_not_fit() {
        let mut inventory = Inventory::new();
        for slot in 1..MAX_INVENTORY_SLOTS {
            inventory.inner.insert(slot, stack(ItemId::Sand, 1));
        }
        assert_eq!(inventory.add_item_to_inventory(stack(ItemId::Dirt, 3)), 0);
        let lost = inventory.add_item_to_inventory(stack(ItemId::Dirt, max_stack()));
        assert_eq!(lost, 3);
        assert_eq!(inventory.inner[&0], stack(ItemId::Dirt, max_stack()));
    }

    #[test]
    fn adding_nothing_takes_no_slot() {
        let mut inventory = Inventory::new();
        assert_eq!(inventory.add_item_to_inventory(stack(ItemId::Dirt, 0)), 0);
        assert!(inventory.inner.is_empty());
    }

    #[test]
    fn add_tops_up_a_later_partial_stack_before_an_earlier_empty_slot() {
        let mut inventory = Inventory::new();
        inventory
            .inner
            .insert(3, stack(ItemId::Dirt, max_stack() - 4));
        inventory.add_item_to_inventory(stack(ItemId::Dirt, 10));
        assert_eq!(inventory.inner[&3], stack(ItemId::Dirt, max_stack()));
        assert_eq!(inventory.inner[&0], stack(ItemId::Dirt, 6));
        assert_eq!(inventory.inner.len(), 2);
    }

    #[test]
    fn add_skips_full_stack_of_same_item() {
        let mut inventory = Inventory::new();
        inventory.add_item_to_inventory(stack(ItemId::Dirt, max_stack()));
        inventory.add_item_to_inventory(stack(ItemId::Dirt, 1));
        assert_eq!(inventory.inner[&0], stack(ItemId::Dirt, max_stack()));
        assert_eq!(inventory.inner[&1], stack(ItemId::Dirt, 1));
    }

    #[test]
    fn filling_every_slot_then_adding_more_changes_nothing() {
        let mut inventory = Inventory::new();
        inventory.add_item_to_inventory(stack(ItemId::Dirt, max_stack() * MAX_INVENTORY_SLOTS));
        assert_eq!(inventory.inner.len(), MAX_INVENTORY_SLOTS as usize);
        for slot in 0..MAX_INVENTORY_SLOTS {
            assert_eq!(inventory.inner[&slot], stack(ItemId::Dirt, max_stack()));
        }

        let full = inventory.clone();
        let lost = inventory.add_item_to_inventory(stack(ItemId::Dirt, 10));
        assert_eq!(lost, 10);
        inventory.add_item_to_inventory(stack(ItemId::Sand, 10));
        assert_eq!(inventory, full);
    }

    #[test]
    fn add_item_to_stack_caps_at_max_stack() {
        let mut inventory = Inventory::new();
        assert_eq!(
            inventory.add_item_to_stack(3, 60, ItemId::Dirt, ItemId::Dirt.get_default_type()),
            60
        );
        assert_eq!(
            inventory.add_item_to_stack(3, 10, ItemId::Dirt, ItemId::Dirt.get_default_type()),
            4
        );
        assert_eq!(inventory.inner[&3], stack(ItemId::Dirt, max_stack()));
    }

    #[test]
    fn remove_partial_leaves_remainder() {
        let mut inventory = Inventory::new();
        inventory.add_item_to_inventory(stack(ItemId::Dirt, 10));
        assert_eq!(inventory.remove_item_from_stack(0, 4), 4);
        assert_eq!(inventory.inner[&0], stack(ItemId::Dirt, 6));
    }

    #[test]
    fn remove_total_empties_the_slot() {
        let mut inventory = Inventory::new();
        inventory.add_item_to_inventory(stack(ItemId::Dirt, 10));
        assert_eq!(inventory.remove_item_from_stack(0, 10), 10);
        assert!(!inventory.inner.contains_key(&0));
    }

    #[test]
    fn remove_more_than_present_returns_only_what_was_there() {
        let mut inventory = Inventory::new();
        inventory.add_item_to_inventory(stack(ItemId::Dirt, 10));
        assert_eq!(inventory.remove_item_from_stack(0, 25), 10);
        assert!(!inventory.inner.contains_key(&0));
    }

    #[test]
    fn remove_from_empty_slot_returns_zero() {
        let mut inventory = Inventory::new();
        assert_eq!(inventory.remove_item_from_stack(5, 3), 0);
        assert!(inventory.inner.is_empty());
    }

    #[test]
    fn a_tool_wears_down_then_breaks_at_its_durability() {
        let durability = ItemId::WoodenAxe.tool().unwrap().durability;
        let mut inventory = Inventory::new();
        inventory.inner.insert(2, stack(ItemId::WoodenAxe, 1));
        for wear in 1..durability {
            assert!(!inventory.wear_tool(2, 1));
            assert_eq!(inventory.inner[&2].item_type, ItemType::Tool { wear });
        }
        assert!(inventory.wear_tool(2, 1));
        assert!(inventory.inner.is_empty());
    }

    #[test]
    fn wearing_anything_but_a_tool_changes_nothing() {
        let mut inventory = Inventory::new();
        inventory.inner.insert(0, stack(ItemId::Dirt, 3));
        let before = inventory.clone();
        assert!(!inventory.wear_tool(0, 1));
        assert!(!inventory.wear_tool(1, 1));
        assert_eq!(inventory, before);
    }

    #[test]
    fn a_worn_tool_keeps_its_wear_when_moved_around() {
        let worn = ItemStack {
            item_type: ItemType::Tool { wear: 30 },
            ..stack(ItemId::WoodenPickaxe, 1)
        };
        let mut inventory = Inventory::new();
        assert_eq!(inventory.add_item_to_inventory(worn), 0);
        assert_eq!(inventory.inner[&0], worn);
        // A second tool never stacks onto the first one
        assert_eq!(
            inventory.add_item_to_inventory(stack(ItemId::WoodenPickaxe, 1)),
            0
        );
        assert_eq!(inventory.inner[&0], worn);
        assert_eq!(inventory.inner[&1], stack(ItemId::WoodenPickaxe, 1));
    }
}
