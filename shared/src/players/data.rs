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

    // Ajoute un item à l'inventaire du joueur
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

impl Player {
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

    pub fn toggle_fly_mode(&mut self) {
        self.is_flying = !self.is_flying;
        self.velocity = Vec3::ZERO;
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
