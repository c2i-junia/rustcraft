use bevy::{
    math::Vec3,
    prelude::{Component, Resource, Transform},
};
use bevy_platform::collections::HashMap;
use serde::{Deserialize, Serialize};

use crate::{
    messages::PlayerId,
    world::{ItemId, ItemStack, ItemType},
    MAX_INVENTORY_SLOTS,
};

#[derive(Debug, Resource, Clone, Serialize, Deserialize, PartialEq)]
pub struct Inventory {
    pub inner: HashMap<u32, ItemStack>,
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
        }
    }

    // Adds an item to the player's inventory
    pub fn add_item_to_inventory(&mut self, mut stack: ItemStack) {
        for i in 0..MAX_INVENTORY_SLOTS {
            let item_option = self.inner.get(&i);

            if let Some(existing_item) = item_option {
                // If not item of right type or stack already full : pass
                if existing_item.item_id != stack.item_id
                    || existing_item.nb >= stack.item_id.get_max_stack()
                {
                    continue;
                }

                stack.nb += existing_item.nb;
            }

            let inserted_stack = ItemStack {
                item_id: stack.item_id,
                item_type: stack.item_type,
                nb: if stack.nb >= stack.item_id.get_max_stack() {
                    stack.item_id.get_max_stack()
                } else {
                    stack.nb
                },
            };
            stack.nb -= inserted_stack.nb;

            // Push inserted items in right inventory slot
            self.inner.insert(i, inserted_stack);

            // If no more items to add, end loop
            if stack.nb == 0 {
                break;
            }
        }

        // Problem : if inventory full, items disappear
    }

    /// Add items to stack at specified position\
    /// Stacks cannot exceed MAX_ITEM_STACK number of items\
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
            new_item.nb = nb + item.nb;
        }
        self.inner.insert(stack, new_item);
        nb
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
    pub inventory: Inventory,
    pub height: f32,
    pub width: f32,
    pub last_input_processed: u64,
}

/// What the movement simulation carries over from one input to the next. The server sends it
/// with each acknowledgement, so that a client resumes its prediction from the very same state.
#[derive(Serialize, Deserialize, PartialEq, Debug, Clone, Copy, Default)]
pub struct MovementState {
    pub position: Vec3,
    pub velocity: Vec3,
    pub on_ground: bool,
    pub is_flying: bool,
}

impl Player {
    pub fn movement_state(&self) -> MovementState {
        MovementState {
            position: self.position,
            velocity: self.velocity,
            on_ground: self.on_ground,
            is_flying: self.is_flying,
        }
    }

    pub fn set_movement_state(&mut self, state: MovementState) {
        self.position = state.position;
        self.velocity = state.velocity;
        self.on_ground = state.on_ground;
        self.is_flying = state.is_flying;
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
            inventory: Inventory::new(),
            height: 1.8,
            width: 0.8,
            last_input_processed: 0,
        }
    }
}

impl Default for Player {
    fn default() -> Self {
        Self {
            id: 0,
            name: "Default".into(),
            position: Vec3::ZERO,
            camera_transform: Transform::default(),
            velocity: Vec3::ZERO,
            on_ground: true,
            is_flying: false,
            inventory: Inventory::new(),
            height: 1.8,
            width: 0.8,
            last_input_processed: 0,
        }
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
        inventory.add_item_to_inventory(stack(ItemId::Dirt, 10));
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
}
