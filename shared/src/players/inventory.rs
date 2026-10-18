//! What an inventory action does: the clicks on the slots of the open inventory, which move
//! items between the slots and the stack held by the cursor (the floating stack), crafting
//! from the grid, and closing it. Run by the client (prediction) and by the server (authority).

use serde::{Deserialize, Serialize};

use crate::{
    messages::PlayerFrameInput,
    players::{blocks::CallerType, Inventory, Player},
    world::{grid_slot, match_recipe, CraftingGrid, ItemId, ItemStack, ItemType},
    MAX_INVENTORY_SLOTS,
};

/// Width and height of the crafting grid of the inventory alone.
pub const INVENTORY_GRID_SIZE: u32 = 2;

/// Append only, like `NetworkAction`: the position of the variant is what an input carries on
/// the network. Bump `PROTOCOL_ID` with any change.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum InventoryAction {
    LeftClickSlot(u32),
    RightClickSlot(u32),
    /// The inventory was closed: the floating stack and the crafting grid go back into it.
    Close,
    /// Click on the result of the crafting grid: the result goes to the cursor, and every cell
    /// of the grid gives one item.
    TakeCraftResult,
}

/// Why an inventory action changed nothing.
#[derive(Debug, PartialEq)]
pub enum InventoryRefusal {
    /// The slot is neither one of the inventory nor one of the usable crafting grid.
    InvalidSlot(u32),
    /// The crafting grid matches no recipe.
    NoRecipe,
    /// The cursor holds another item, or has no room for the result.
    CursorFull,
}

/// The slots of the cells of a grid of `size` by `size`, in its top left corner.
pub fn grid_slots(size: u32) -> impl Iterator<Item = u32> {
    (0..size).flat_map(move |row| (0..size).map(move |col| grid_slot(row, col)))
}

impl Inventory {
    /// The items in the cells of the crafting grid.
    pub fn crafting_grid(&self) -> CraftingGrid {
        let mut grid = CraftingGrid::default();
        for (cell, slot) in grid.iter_mut().zip(grid_slots(3)) {
            *cell = self.inner.get(&slot).map(|stack| stack.item_id);
        }
        grid
    }
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
                floating = put_back(inventory, stack);
            }
            for slot in grid_slots(3) {
                if let Some(stack) = inventory.inner.remove(&slot) {
                    if let Some(kept) = put_back(inventory, stack) {
                        inventory.inner.insert(slot, kept);
                    }
                }
            }
            Ok(())
        }
        InventoryAction::TakeCraftResult => take_craft_result(inventory, &mut floating),
    };
    inventory.floating = floating;
    applied
}

/// Adds `stack` to the inventory and returns what did not fit.
fn put_back(inventory: &mut Inventory, stack: ItemStack) -> Option<ItemStack> {
    let kept = inventory.add_item_to_inventory(stack);
    (kept > 0).then_some(ItemStack { nb: kept, ..stack })
}

fn take_craft_result(
    inventory: &mut Inventory,
    floating: &mut Option<ItemStack>,
) -> Result<(), InventoryRefusal> {
    let result = match_recipe(&inventory.crafting_grid()).ok_or(InventoryRefusal::NoRecipe)?;
    let held = match *floating {
        None => 0,
        Some(held) if held.item_id == result.item_id => held.nb,
        Some(_) => return Err(InventoryRefusal::CursorFull),
    };
    if held + result.nb > result.item_id.get_max_stack() {
        return Err(InventoryRefusal::CursorFull);
    }

    *floating = Some(ItemStack {
        nb: held + result.nb,
        ..result
    });
    for slot in grid_slots(3) {
        inventory.remove_item_from_stack(slot, 1);
    }
    Ok(())
}

fn check_slot(slot: u32) -> Result<(), InventoryRefusal> {
    if slot < MAX_INVENTORY_SLOTS || grid_slots(INVENTORY_GRID_SIZE).any(|cell| cell == slot) {
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
    use crate::world::grid_slot;

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
                InventoryAction::LeftClickSlot(MAX_INVENTORY_SLOTS + 50)
            ),
            Err(InventoryRefusal::InvalidSlot(MAX_INVENTORY_SLOTS + 50))
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

    fn planks(nb: u32) -> ItemStack {
        stack(ItemId::OakPlanks, nb)
    }

    #[test]
    fn the_crafting_grid_holds_the_items_of_its_slots() {
        let mut inventory = inventory_with(grid_slot(1, 0), stack(ItemId::OakLog, 2));
        inventory.inner.insert(0, stack(ItemId::Dirt, 1));

        let mut expected = [None; 9];
        expected[3] = Some(ItemId::OakLog);
        assert_eq!(inventory.crafting_grid(), expected);
    }

    #[test]
    fn taking_the_result_consumes_one_item_per_cell() {
        let mut inventory = inventory_with(grid_slot(0, 0), stack(ItemId::OakLog, 2));

        apply(&mut inventory, InventoryAction::TakeCraftResult);

        assert_eq!(inventory.floating, Some(planks(4)));
        assert_eq!(inventory.inner[&grid_slot(0, 0)], stack(ItemId::OakLog, 1));

        apply(&mut inventory, InventoryAction::TakeCraftResult);
        assert_eq!(inventory.floating, Some(planks(8)));
        assert!(inventory.inner.is_empty());
    }

    #[test]
    fn the_result_stacks_on_a_cursor_holding_the_same_item() {
        let mut inventory = inventory_with(grid_slot(0, 0), stack(ItemId::OakLog, 1));
        inventory.floating = Some(planks(3));

        apply(&mut inventory, InventoryAction::TakeCraftResult);

        assert_eq!(inventory.floating, Some(planks(7)));
    }

    #[test]
    fn the_result_is_refused_when_the_cursor_cannot_take_it() {
        for held in [
            stack(ItemId::Dirt, 1),
            planks(ItemId::OakPlanks.get_max_stack() - 2),
        ] {
            let mut inventory = inventory_with(grid_slot(0, 0), stack(ItemId::OakLog, 1));
            inventory.floating = Some(held);
            let before = inventory.clone();

            assert_eq!(
                apply_inventory_action(&mut inventory, InventoryAction::TakeCraftResult),
                Err(InventoryRefusal::CursorFull)
            );
            assert_eq!(inventory, before);
        }
    }

    #[test]
    fn taking_the_result_of_no_recipe_is_refused() {
        let mut inventory = inventory_with(grid_slot(0, 0), stack(ItemId::Dirt, 1));
        assert_eq!(
            apply_inventory_action(&mut inventory, InventoryAction::TakeCraftResult),
            Err(InventoryRefusal::NoRecipe)
        );
    }

    #[test]
    fn a_click_on_a_grid_cell_outside_the_2x2_grid_is_refused() {
        let mut inventory = Inventory::new();
        inventory.floating = Some(planks(1));

        assert_eq!(
            apply_inventory_action(
                &mut inventory,
                InventoryAction::LeftClickSlot(grid_slot(0, 2))
            ),
            Err(InventoryRefusal::InvalidSlot(grid_slot(0, 2)))
        );
        apply(
            &mut inventory,
            InventoryAction::LeftClickSlot(grid_slot(1, 1)),
        );
        assert_eq!(inventory.inner[&grid_slot(1, 1)], planks(1));
    }

    #[test]
    fn close_returns_the_grid_to_the_inventory() {
        let mut inventory = inventory_with(grid_slot(0, 0), stack(ItemId::OakLog, 2));
        inventory.inner.insert(grid_slot(1, 1), planks(3));

        apply(&mut inventory, InventoryAction::Close);

        assert_eq!(inventory.inner.len(), 2);
        assert_eq!(inventory.inner[&0], stack(ItemId::OakLog, 2));
        assert_eq!(inventory.inner[&1], planks(3));
    }

    #[test]
    fn close_keeps_in_the_grid_what_does_not_fit() {
        let mut inventory = Inventory::new();
        for slot in 0..MAX_INVENTORY_SLOTS {
            inventory
                .inner
                .insert(slot, stack(ItemId::Dirt, max_dirt()));
        }
        inventory.inner.insert(grid_slot(0, 0), planks(3));

        apply(&mut inventory, InventoryAction::Close);

        assert_eq!(inventory.inner[&grid_slot(0, 0)], planks(3));
    }

    #[test]
    fn the_actions_of_an_input_are_applied_in_order() {
        let mut player = Player::default();
        player.inventory.inner.insert(0, stack(ItemId::Dirt, 5));
        let input = PlayerFrameInput {
            inventory_actions: vec![
                InventoryAction::LeftClickSlot(0),
                InventoryAction::LeftClickSlot(MAX_INVENTORY_SLOTS + 50),
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
