use bevy::{
    prelude::{Component, Resource},
    utils::HashMap,
};

use crate::{
    messages::PlayerId,
    world::{ItemId, ItemStack, ItemType},
    MAX_INVENTORY_SLOTS,
};

#[derive(Debug, Resource, Clone)]
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

            if item_option.is_some() {
                let existing_item = item_option.expect("Error : empty item");
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

#[derive(Component, Clone)]
pub struct Player {
    pub id: PlayerId,
    pub name: String,
    pub vertical_velocity: f32,
    pub on_ground: bool,
    // pub view_mode: ViewMode,
    // pub is_chunk_debug_mode_enabled: bool,
    pub is_flying: bool,
    // pub inventory: HashMap<RegistryId, items::Item>,
    pub height: f32,
    pub width: f32,
}

impl Player {
    pub fn new(id: PlayerId, name: String) -> Self {
        Self {
            id,
            name,
            vertical_velocity: 0.0,
            on_ground: true,
            is_flying: false,
            height: 1.8,
            width: 0.8,
        }
    }

    pub fn toggle_fly_mode(&mut self) {
        self.is_flying = !self.is_flying;
        self.vertical_velocity = 0.0; // Réinitialisation de la vélocité
    }
}
