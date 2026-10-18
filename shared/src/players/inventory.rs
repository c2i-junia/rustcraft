//! What an inventory action does: the clicks on the slots of the open inventory, which move
//! items between the slots and the stack held by the cursor (the floating stack), and closing
//! it. Run by the client (prediction) and by the server (authority).

use serde::{Deserialize, Serialize};

use crate::{
    messages::PlayerFrameInput,
    players::{blocks::CallerType, Inventory, Player},
    world::{ItemId, ItemStack, ItemType},
    MAX_INVENTORY_SLOTS,
};

/// Append only, like `NetworkAction`: the position of the variant is what an input carries on
/// the network. Bump `PROTOCOL_ID` with any change.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum InventoryAction {
    LeftClickSlot(u32),
    RightClickSlot(u32),
    /// The inventory was closed: the floating stack goes back into it.
    Close,
}

/// Why an inventory action changed nothing.
#[derive(Debug, PartialEq)]
pub enum InventoryRefusal {
    /// The slot is not one of the inventory.
    InvalidSlot(u32),
}

/// Applies the inventory actions of `input`, in order.
pub fn simulate_inventory_actions(
    player: &mut Player,
    input: &PlayerFrameInput,
    caller_type: CallerType,
) {
    for &action in &input.inventory_actions {
        if let Err(refusal) = apply_inventory_action(&mut player.inventory, action) {
            log::debug!(
                "{} Player {} could not apply {action:?}: {refusal:?}",
                caller_type.as_str(),
                player.id
            );
        }
    }
}

pub fn apply_inventory_action(
    inventory: &mut Inventory,
    action: InventoryAction,
) -> Result<(), InventoryRefusal> {
    let mut floating = inventory.floating.take();
    let applied = match action {
        InventoryAction::LeftClickSlot(slot) => {
            check_slot(slot).map(|()| left_click_slot(inventory, slot, &mut floating))
        }
        InventoryAction::RightClickSlot(slot) => {
            check_slot(slot).map(|()| right_click_slot(inventory, slot, &mut floating))
        }
        InventoryAction::Close => {
            if let Some(stack) = floating {
                let kept = inventory.add_item_to_inventory(stack);
                floating = (kept > 0).then_some(ItemStack { nb: kept, ..stack });
            }
            Ok(())
        }
    };
    inventory.floating = floating;
    applied
}

fn check_slot(slot: u32) -> Result<(), InventoryRefusal> {
    if slot < MAX_INVENTORY_SLOTS {
        Ok(())
    } else {
        Err(InventoryRefusal::InvalidSlot(slot))
    }
}

/// Removes `nb` items from the floating stack\
/// Cannot go lower than `0` items\
/// Returns number of items _actually_ removed
fn remove_item_floating_stack(floating_stack: &mut Option<ItemStack>, nb: u32) -> u32 {
    let Some(item) = floating_stack else {
        return 0;
    };
    if nb >= item.nb {
        let removed = item.nb;
        *floating_stack = None;
        return removed;
    }
    item.nb -= nb;
    nb
}

/// Adds `nb` items to the floating stack\
/// Cannot go higher than the item's maximum stack size\
/// Parameter `item_type` will **ONLY BE USED** if no items are present in the floating stack\
/// Returns number of items _actually_ added
fn add_item_floating_stack(
    floating_stack: &mut Option<ItemStack>,
    nb: u32,
    item_id: ItemId,
    item_type: ItemType,
) -> u32 {
    if nb == 0 {
        return 0;
    }
    match floating_stack {
        Some(item) => {
            let added = nb.min(item.item_id.get_max_stack().saturating_sub(item.nb));
            item.nb += added;
            added
        }
        None => {
            let added = nb.min(item_id.get_max_stack());
            *floating_stack = Some(ItemStack {
                item_id,
                item_type,
                nb: added,
            });
            added
        }
    }
}

/// Left click on `slot`: tops the slot up from the floating stack when both hold the same
/// item and the slot has room, and swaps the two otherwise.
fn left_click_slot(inventory: &mut Inventory, slot: u32, floating_stack: &mut Option<ItemStack>) {
    let in_slot = inventory.inner.get(&slot).copied();

    if let (Some(in_slot), Some(floating)) = (in_slot, *floating_stack) {
        let room = in_slot.item_id.get_max_stack().saturating_sub(in_slot.nb);
        if in_slot.item_id == floating.item_id && room > 0 {
            let moved = remove_item_floating_stack(floating_stack, room);
            inventory.add_item_to_stack(slot, moved, floating.item_id, in_slot.item_type);
            return;
        }
    }

    match floating_stack.take() {
        Some(floating) => inventory.inner.insert(slot, floating),
        None => inventory.inner.remove(&slot),
    };
    *floating_stack = in_slot;
}

/// Right click on `slot`: with a floating stack, drops one of its items into the slot if the
/// slot is empty or holds the same item; without one, picks up half of the slot, rounded up.
fn right_click_slot(inventory: &mut Inventory, slot: u32, floating_stack: &mut Option<ItemStack>) {
    let in_slot = inventory.inner.get(&slot).copied();

    match (*floating_stack, in_slot) {
        (Some(floating), in_slot) => {
            let accepts = in_slot.is_none_or(|in_slot| in_slot.item_id == floating.item_id);
            if accepts && floating.nb > 0 {
                let dropped =
                    inventory.add_item_to_stack(slot, 1, floating.item_id, floating.item_type);
                remove_item_floating_stack(floating_stack, dropped);
            }
        }
        (None, Some(in_slot)) => {
            let taken = inventory.remove_item_from_stack(slot, in_slot.nb.div_ceil(2));
            add_item_floating_stack(floating_stack, taken, in_slot.item_id, in_slot.item_type);
        }
        (None, None) => {}
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

    fn inventory_with(slot: u32, stack: ItemStack) -> Inventory {
        let mut inventory = Inventory::new();
        inventory.inner.insert(slot, stack);
        inventory
    }

    fn max_dirt() -> u32 {
        ItemId::Dirt.get_max_stack()
    }

    fn apply(inventory: &mut Inventory, action: InventoryAction) {
        apply_inventory_action(inventory, action).unwrap();
    }

    #[test]
    fn left_click_picks_up_a_stack_and_puts_it_down_again() {
        let mut inventory = inventory_with(3, stack(ItemId::Dirt, 5));

        apply(&mut inventory, InventoryAction::LeftClickSlot(3));
        assert_eq!(inventory.floating, Some(stack(ItemId::Dirt, 5)));
        assert!(inventory.inner.is_empty());

        apply(&mut inventory, InventoryAction::LeftClickSlot(8));
        assert_eq!(inventory.floating, None);
        assert_eq!(inventory.inner[&8], stack(ItemId::Dirt, 5));
    }

    #[test]
    fn left_click_on_another_item_swaps_the_stacks() {
        let mut inventory = inventory_with(0, stack(ItemId::Dirt, 5));
        inventory.floating = Some(stack(ItemId::Sand, 2));

        apply(&mut inventory, InventoryAction::LeftClickSlot(0));

        assert_eq!(inventory.inner[&0], stack(ItemId::Sand, 2));
        assert_eq!(inventory.floating, Some(stack(ItemId::Dirt, 5)));
    }

    #[test]
    fn left_click_on_the_same_item_tops_the_slot_up_and_keeps_the_rest() {
        let mut inventory = inventory_with(0, stack(ItemId::Dirt, max_dirt() - 2));
        inventory.floating = Some(stack(ItemId::Dirt, 5));

        apply(&mut inventory, InventoryAction::LeftClickSlot(0));

        assert_eq!(inventory.inner[&0], stack(ItemId::Dirt, max_dirt()));
        assert_eq!(inventory.floating, Some(stack(ItemId::Dirt, 3)));
    }

    #[test]
    fn left_click_on_a_full_slot_of_the_same_item_swaps() {
        let mut inventory = inventory_with(0, stack(ItemId::Dirt, max_dirt()));
        inventory.floating = Some(stack(ItemId::Dirt, 5));

        apply(&mut inventory, InventoryAction::LeftClickSlot(0));

        assert_eq!(inventory.inner[&0], stack(ItemId::Dirt, 5));
        assert_eq!(inventory.floating, Some(stack(ItemId::Dirt, max_dirt())));
    }

    #[test]
    fn left_click_on_an_empty_slot_with_nothing_held_changes_nothing() {
        let mut inventory = Inventory::new();
        apply(&mut inventory, InventoryAction::LeftClickSlot(0));
        assert_eq!(inventory, Inventory::new());
    }

    #[test]
    fn right_click_picks_up_half_of_a_stack_rounded_up() {
        let mut inventory = inventory_with(0, stack(ItemId::Dirt, 5));

        apply(&mut inventory, InventoryAction::RightClickSlot(0));

        assert_eq!(inventory.floating, Some(stack(ItemId::Dirt, 3)));
        assert_eq!(inventory.inner[&0], stack(ItemId::Dirt, 2));
    }

    #[test]
    fn right_click_drops_one_item_into_an_empty_or_matching_slot() {
        let mut inventory = Inventory::new();
        inventory.floating = Some(stack(ItemId::Dirt, 2));

        apply(&mut inventory, InventoryAction::RightClickSlot(4));
        assert_eq!(inventory.inner[&4], stack(ItemId::Dirt, 1));
        assert_eq!(inventory.floating, Some(stack(ItemId::Dirt, 1)));

        apply(&mut inventory, InventoryAction::RightClickSlot(4));
        assert_eq!(inventory.inner[&4], stack(ItemId::Dirt, 2));
        assert_eq!(inventory.floating, None);
    }

    #[test]
    fn right_click_drops_nothing_into_another_item_or_a_full_slot() {
        let mut inventory = inventory_with(0, stack(ItemId::Sand, 1));
        inventory.inner.insert(1, stack(ItemId::Dirt, max_dirt()));
        inventory.floating = Some(stack(ItemId::Dirt, 2));

        apply(&mut inventory, InventoryAction::RightClickSlot(0));
        apply(&mut inventory, InventoryAction::RightClickSlot(1));

        assert_eq!(inventory.inner[&0], stack(ItemId::Sand, 1));
        assert_eq!(inventory.inner[&1], stack(ItemId::Dirt, max_dirt()));
        assert_eq!(inventory.floating, Some(stack(ItemId::Dirt, 2)));
    }

    #[test]
    fn the_floating_stack_never_exceeds_a_full_stack() {
        let mut floating = Some(stack(ItemId::Dirt, max_dirt() - 1));
        assert_eq!(
            add_item_floating_stack(
                &mut floating,
                5,
                ItemId::Dirt,
                ItemId::Dirt.get_default_type()
            ),
            1
        );
        assert_eq!(
            remove_item_floating_stack(&mut floating, u32::MAX),
            max_dirt()
        );
        assert_eq!(floating, None);
        assert_eq!(remove_item_floating_stack(&mut floating, 1), 0);
    }

    #[test]
    fn a_click_on_a_slot_outside_the_inventory_is_refused() {
        let mut inventory = inventory_with(0, stack(ItemId::Dirt, 5));
        inventory.floating = Some(stack(ItemId::Sand, 2));
        let before = inventory.clone();

        assert_eq!(
            apply_inventory_action(
                &mut inventory,
                InventoryAction::LeftClickSlot(MAX_INVENTORY_SLOTS)
            ),
            Err(InventoryRefusal::InvalidSlot(MAX_INVENTORY_SLOTS))
        );
        assert_eq!(
            apply_inventory_action(&mut inventory, InventoryAction::RightClickSlot(1000)),
            Err(InventoryRefusal::InvalidSlot(1000))
        );
        assert_eq!(inventory, before);
    }

    #[test]
    fn close_puts_the_floating_stack_back_in_the_inventory() {
        let mut inventory = inventory_with(0, stack(ItemId::Dirt, 5));
        inventory.floating = Some(stack(ItemId::Dirt, 3));

        apply(&mut inventory, InventoryAction::Close);

        assert_eq!(inventory.floating, None);
        assert_eq!(inventory.inner[&0], stack(ItemId::Dirt, 8));
    }

    #[test]
    fn close_keeps_what_does_not_fit_in_a_full_inventory() {
        let mut inventory = Inventory::new();
        for slot in 0..MAX_INVENTORY_SLOTS {
            inventory
                .inner
                .insert(slot, stack(ItemId::Dirt, max_dirt()));
        }
        inventory.floating = Some(stack(ItemId::Sand, 5));

        apply(&mut inventory, InventoryAction::Close);

        assert_eq!(inventory.floating, Some(stack(ItemId::Sand, 5)));
    }

    #[test]
    fn the_actions_of_an_input_are_applied_in_order() {
        let mut player = Player::default();
        player.inventory.inner.insert(0, stack(ItemId::Dirt, 5));
        let input = PlayerFrameInput {
            inventory_actions: vec![
                InventoryAction::LeftClickSlot(0),
                InventoryAction::LeftClickSlot(MAX_INVENTORY_SLOTS),
                InventoryAction::LeftClickSlot(7),
            ],
            ..Default::default()
        };

        simulate_inventory_actions(&mut player, &input, CallerType::Server);

        assert_eq!(player.inventory.inner.len(), 1);
        assert_eq!(player.inventory.inner[&7], stack(ItemId::Dirt, 5));
        assert_eq!(player.inventory.floating, None);
    }

    #[test]
    fn old_player_saves_without_a_floating_stack_still_load() {
        let inventory: Inventory = ron::from_str("(inner: {})").unwrap();
        assert_eq!(inventory, Inventory::new());
    }
}
