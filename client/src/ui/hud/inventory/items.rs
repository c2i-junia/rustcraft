use shared::world::{ItemId, ItemStack, ItemType};

use crate::ui::hud::inventory::FloatingStack;

/// Removes `nb` items from the floating stack\
/// Cannot go lower than `0` items\
/// Returns number of items _actually_ removed
pub fn remove_item_floating_stack(floating_stack: &mut FloatingStack, nb: u32) -> u32 {
    if let Some(mut item) = floating_stack.items {
        if nb >= item.nb {
            floating_stack.items = None;
            return item.nb;
        }
        item.nb -= nb;
        floating_stack.items = Some(item);
        return nb;
    }
    0
}

/// Adds `nb` items to the floating stack\
/// Cannot go higher than `MAX_ITEM_STACK` items\
/// Parameter `item_type` will **ONLY BE USED** if no items are present in the floating stack\
/// Returns number of items _actually_ added
pub fn add_item_floating_stack(
    floating_stack: &mut FloatingStack,
    mut nb: u32,
    item_id: ItemId,
    item_type: ItemType,
) -> u32 {
    if nb == 0 {
        0
    } else if let Some(mut item) = floating_stack.items {
        if nb + item.nb > item.item_id.get_max_stack() {
            nb = item.item_id.get_max_stack() - item.nb;
        }
        item.nb += nb;
        nb
    } else {
        if nb > item_id.get_max_stack() {
            nb = item_id.get_max_stack();
        }
        floating_stack.items = Some(ItemStack {
            item_id,
            item_type,
            nb,
        });
        nb
    }
}
