//! What an inventory action does: the clicks on the slots of the open inventory, which move
//! items between the slots and the stack held by the cursor (the floating stack), crafting
//! from the grid, using a crafting table, and closing it. Run by the client (prediction) and by
//! the server (authority).

use std::ops::Range;

use bevy::math::IVec3;
use serde::{Deserialize, Serialize};

use crate::{
    messages::PlayerFrameInput,
    players::{
        blocks::{CallerType, INTERACTION_DISTANCE},
        Inventory, Player,
    },
    world::{
        grid_slot, match_recipe, BlockId, ChunkMap, CraftingGrid, ItemId, ItemStack, ItemType,
        CRAFTING_GRID_SIZE,
    },
    HALF_BLOCK, HOTBAR_SLOTS, MAX_INVENTORY_ACTIONS_PER_INPUT, MAX_INVENTORY_SLOTS,
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
    /// Right click on the crafting table at this position: the grid grows to 3x3 while the
    /// table stays within reach.
    OpenCraftingTable(IVec3),
    /// Shift+click: the stack of this slot goes to the other part of the inventory, the hotbar
    /// or the backpack. A crafting grid cell goes back into the inventory.
    QuickMoveSlot(u32),
    /// Number key over a slot: it trades places with this slot of the hotbar.
    SwapWithHotbar {
        slot: u32,
        hotbar: u32,
    },
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
    /// There is no crafting table within reach at this position.
    NoTable(IVec3),
    /// The cursor holds a stack, which a swap with the hotbar would leave out.
    HoldingAStack,
}

/// The slots of the cells of a grid of `size` by `size`, in its top left corner.
pub fn grid_slots(size: u32) -> impl Iterator<Item = u32> {
    (0..size).flat_map(move |row| (0..size).map(move |col| grid_slot(row, col)))
}

impl Inventory {
    /// The items in the cells of the crafting grid.
    pub fn crafting_grid(&self) -> CraftingGrid {
        let mut grid = CraftingGrid::default();
        for (cell, slot) in grid.iter_mut().zip(grid_slots(CRAFTING_GRID_SIZE)) {
            *cell = self.inner.get(&slot).map(|stack| stack.item_id);
        }
        grid
    }
}

/// Width and height of the crafting grid in use: 3 while the open table is a crafting table
/// within reach, 2 otherwise.
pub fn grid_size(inventory: &Inventory, table_in_reach: impl Fn(IVec3) -> bool) -> u32 {
    match inventory.open_table {
        Some(table) if table_in_reach(table) => CRAFTING_GRID_SIZE,
        _ => INVENTORY_GRID_SIZE,
    }
}

/// Applies the inventory actions of `input`, in order, up to `MAX_INVENTORY_ACTIONS_PER_INPUT`.
pub fn simulate_inventory_actions(
    player: &mut Player,
    world_map: &ChunkMap,
    input: &PlayerFrameInput,
    caller_type: CallerType,
) {
    let position = player.position;
    let table_in_reach = |table: IVec3| {
        world_map
            .get_block(table)
            .is_some_and(|block| block.id == BlockId::CraftingTable)
            && (table.as_vec3() + HALF_BLOCK).distance(position) <= INTERACTION_DISTANCE
    };
    // A table the player walked away from, or that was broken, is closed: the dialog shrinks
    if let Some(table) = player.inventory.open_table {
        if !table_in_reach(table) {
            player.inventory.open_table = None;
        }
    }
    let actions = input.inventory_actions.iter();
    for &action in actions.take(MAX_INVENTORY_ACTIONS_PER_INPUT) {
        if let Err(refusal) = apply_inventory_action(&mut player.inventory, action, table_in_reach)
        {
            log::debug!(
                "{} Player {} could not apply {action:?}: {refusal:?}",
                caller_type.as_str(),
                player.id
            );
        }
    }
}

/// Applies `action` to `inventory`. `table_in_reach` tells whether a position holds a crafting
/// table the player can use.
pub fn apply_inventory_action(
    inventory: &mut Inventory,
    action: InventoryAction,
    table_in_reach: impl Fn(IVec3) -> bool,
) -> Result<(), InventoryRefusal> {
    let size = grid_size(inventory, &table_in_reach);
    let mut floating = inventory.floating.take();
    let applied = match action {
        InventoryAction::LeftClickSlot(slot) => check_slot(inventory, slot, size, floating)
            .map(|()| left_click_slot(inventory, slot, &mut floating)),
        InventoryAction::RightClickSlot(slot) => check_slot(inventory, slot, size, floating)
            .map(|()| right_click_slot(inventory, slot, &mut floating)),
        InventoryAction::QuickMoveSlot(slot) => {
            check_slot(inventory, slot, size, floating).map(|()| quick_move_slot(inventory, slot))
        }
        InventoryAction::SwapWithHotbar { slot, hotbar } => {
            swap_with_hotbar(inventory, slot, hotbar, size, floating)
        }
        InventoryAction::Close => {
            inventory.open_table = None;
            if let Some(stack) = floating {
                floating = put_back(inventory, stack);
            }
            for slot in grid_slots(CRAFTING_GRID_SIZE) {
                if let Some(stack) = inventory.inner.remove(&slot) {
                    if let Some(kept) = put_back(inventory, stack) {
                        inventory.inner.insert(slot, kept);
                    }
                }
            }
            Ok(())
        }
        InventoryAction::TakeCraftResult => take_craft_result(inventory, &mut floating, size),
        InventoryAction::OpenCraftingTable(table) => {
            if table_in_reach(table) {
                inventory.open_table = Some(table);
                Ok(())
            } else {
                Err(InventoryRefusal::NoTable(table))
            }
        }
    };
    inventory.floating = floating;
    applied
}

/// Adds `stack` to the inventory and returns what did not fit.
fn put_back(inventory: &mut Inventory, stack: ItemStack) -> Option<ItemStack> {
    let kept = inventory.add_item_to_inventory(stack);
    (kept > 0).then_some(ItemStack { nb: kept, ..stack })
}

/// Adds `stack` to the slots of `range`: first on stacks of the same item, then in empty slots,
/// in slot order. Returns how many items did not fit.
fn add_to_range(inventory: &mut Inventory, mut stack: ItemStack, range: Range<u32>) -> u32 {
    let max_stack = stack.item_id.get_max_stack();
    for tops_up in [true, false] {
        for slot in range.clone() {
            if stack.nb == 0 {
                return 0;
            }
            let matches = match inventory.inner.get(&slot) {
                Some(held) => tops_up && held.item_id == stack.item_id,
                None => !tops_up,
            };
            if !matches {
                continue;
            }
            let held = inventory.inner.get(&slot).map_or(0, |held| held.nb);
            let added = stack.nb.min(max_stack.saturating_sub(held));
            if added > 0 {
                inventory.inner.insert(
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

/// Shift+click on `slot`: moves its stack from the hotbar to the backpack or the other way
/// round, and from a crafting grid cell into the whole inventory. What does not fit stays.
fn quick_move_slot(inventory: &mut Inventory, slot: u32) {
    let Some(stack) = inventory.inner.remove(&slot) else {
        return;
    };
    let kept = if slot < HOTBAR_SLOTS {
        let kept = add_to_range(inventory, stack, HOTBAR_SLOTS..MAX_INVENTORY_SLOTS);
        (kept > 0).then_some(ItemStack { nb: kept, ..stack })
    } else if slot < MAX_INVENTORY_SLOTS {
        let kept = add_to_range(inventory, stack, 0..HOTBAR_SLOTS);
        (kept > 0).then_some(ItemStack { nb: kept, ..stack })
    } else {
        put_back(inventory, stack)
    };
    if let Some(kept) = kept {
        inventory.inner.insert(slot, kept);
    }
}

/// Number key over `slot`: exchanges it with the slot `hotbar` of the hotbar, without merging.
fn swap_with_hotbar(
    inventory: &mut Inventory,
    slot: u32,
    hotbar: u32,
    size: u32,
    floating: Option<ItemStack>,
) -> Result<(), InventoryRefusal> {
    if hotbar >= HOTBAR_SLOTS {
        return Err(InventoryRefusal::InvalidSlot(hotbar));
    }
    if floating.is_some() {
        return Err(InventoryRefusal::HoldingAStack);
    }
    if slot >= MAX_INVENTORY_SLOTS && !grid_slots(size).any(|cell| cell == slot) {
        return Err(InventoryRefusal::InvalidSlot(slot));
    }
    let in_slot = inventory.inner.remove(&slot);
    let in_hotbar = inventory.inner.remove(&hotbar);
    if let Some(stack) = in_slot {
        inventory.inner.insert(hotbar, stack);
    }
    if let Some(stack) = in_hotbar {
        inventory.inner.insert(slot, stack);
    }
    Ok(())
}

/// Whether `slot` is a cell of the 3x3 grid outside the grid of `size` in use.
fn is_beyond_the_grid(slot: u32, size: u32) -> bool {
    grid_slots(CRAFTING_GRID_SIZE).any(|cell| cell == slot)
        && !grid_slots(size).any(|usable| usable == slot)
}

/// What the grid of `size` in use makes. Nothing while items lie in cells beyond it, left there
/// when a table was closed with a full inventory.
pub fn craft_result(inventory: &Inventory, size: u32) -> Option<ItemStack> {
    let beyond_the_grid = grid_slots(CRAFTING_GRID_SIZE)
        .any(|slot| is_beyond_the_grid(slot, size) && inventory.inner.contains_key(&slot));
    if beyond_the_grid {
        return None;
    }
    match_recipe(&inventory.crafting_grid())
}

fn take_craft_result(
    inventory: &mut Inventory,
    floating: &mut Option<ItemStack>,
    size: u32,
) -> Result<(), InventoryRefusal> {
    let result = craft_result(inventory, size).ok_or(InventoryRefusal::NoRecipe)?;
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
    for slot in grid_slots(CRAFTING_GRID_SIZE) {
        inventory.remove_item_from_stack(slot, 1);
    }
    Ok(())
}

/// A click may reach the inventory and the grid in use. Cells beyond it may only be emptied,
/// with nothing in the cursor.
fn check_slot(
    inventory: &Inventory,
    slot: u32,
    size: u32,
    floating: Option<ItemStack>,
) -> Result<(), InventoryRefusal> {
    let takes_a_leftover =
        is_beyond_the_grid(slot, size) && floating.is_none() && inventory.inner.contains_key(&slot);
    if slot < MAX_INVENTORY_SLOTS || grid_slots(size).any(|cell| cell == slot) || takes_a_leftover {
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
    use crate::world::{grid_slot, BlockId, ChunkMap};
    use bevy::math::{IVec3, Vec3};
    use bevy::prelude::Transform;

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
        apply_inventory_action(inventory, action, |_| false).unwrap();
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
                InventoryAction::LeftClickSlot(MAX_INVENTORY_SLOTS + 50),
                |_| false
            ),
            Err(InventoryRefusal::InvalidSlot(MAX_INVENTORY_SLOTS + 50))
        );
        assert_eq!(
            apply_inventory_action(
                &mut inventory,
                InventoryAction::RightClickSlot(1000),
                |_| false
            ),
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
                apply_inventory_action(&mut inventory, InventoryAction::TakeCraftResult, |_| false),
                Err(InventoryRefusal::CursorFull)
            );
            assert_eq!(inventory, before);
        }
    }

    #[test]
    fn taking_the_result_of_no_recipe_is_refused() {
        let mut inventory = inventory_with(grid_slot(0, 0), stack(ItemId::Dirt, 1));
        assert_eq!(
            apply_inventory_action(&mut inventory, InventoryAction::TakeCraftResult, |_| false),
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
                InventoryAction::LeftClickSlot(grid_slot(0, 2)),
                |_| false
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

    const TABLE: IVec3 = IVec3::new(0, 0, -3);
    /// Three blocks from the centre of `TABLE`, and thirteen.
    const NEAR: Vec3 = Vec3::new(0.5, 0.5, 0.5);
    const FAR: Vec3 = Vec3::new(0.5, 0.5, 10.5);

    /// A player at `position`, and a world holding `block` at `TABLE`.
    fn player_by(block: BlockId, position: Vec3) -> (Player, ChunkMap) {
        let player = Player::new(1, "tester".into(), position, Transform::default());
        let mut world = ChunkMap::with_loaded_region(IVec3::splat(-16), IVec3::splat(15));
        world.fill_box(TABLE, TABLE, block);
        (player, world)
    }

    fn act(player: &mut Player, world: &ChunkMap, actions: Vec<InventoryAction>) {
        let input = PlayerFrameInput {
            inventory_actions: actions,
            ..Default::default()
        };
        simulate_inventory_actions(player, world, &input, CallerType::Server);
    }

    /// Fills the grid with a ring of cobblestone, a furnace, but for its bottom right cell.
    fn almost_a_furnace(inventory: &mut Inventory) {
        for (row, col) in [(0, 0), (0, 1), (0, 2), (1, 0), (1, 2), (2, 0), (2, 1)] {
            inventory
                .inner
                .insert(grid_slot(row, col), stack(ItemId::Cobblestone, 1));
        }
        inventory.floating = Some(stack(ItemId::Cobblestone, 1));
    }

    fn furnace_from_a_table(player: &mut Player, world: &ChunkMap) -> Option<ItemStack> {
        act(
            player,
            world,
            vec![
                InventoryAction::LeftClickSlot(grid_slot(2, 2)),
                InventoryAction::TakeCraftResult,
            ],
        );
        player.inventory.floating
    }

    #[test]
    fn a_table_opens_only_within_reach() {
        let (mut player, world) = player_by(BlockId::CraftingTable, NEAR);
        act(
            &mut player,
            &world,
            vec![InventoryAction::OpenCraftingTable(TABLE)],
        );
        assert_eq!(player.inventory.open_table, Some(TABLE));

        let (mut player, world) = player_by(BlockId::CraftingTable, FAR);
        act(
            &mut player,
            &world,
            vec![InventoryAction::OpenCraftingTable(TABLE)],
        );
        assert_eq!(player.inventory.open_table, None);
    }

    #[test]
    fn a_block_that_is_not_a_table_does_not_open() {
        let (mut player, world) = player_by(BlockId::Stone, NEAR);
        act(
            &mut player,
            &world,
            vec![InventoryAction::OpenCraftingTable(TABLE)],
        );
        assert_eq!(player.inventory.open_table, None);
    }

    #[test]
    fn the_3x3_grid_is_usable_while_a_table_is_open() {
        let (mut player, world) = player_by(BlockId::CraftingTable, NEAR);
        almost_a_furnace(&mut player.inventory);
        act(
            &mut player,
            &world,
            vec![InventoryAction::OpenCraftingTable(TABLE)],
        );

        assert_eq!(
            furnace_from_a_table(&mut player, &world),
            Some(stack(ItemId::Furnace, 1))
        );
        assert!(player.inventory.inner.is_empty());
    }

    #[test]
    fn a_ring_of_cobblestone_makes_nothing_without_a_table() {
        let (mut player, world) = player_by(BlockId::CraftingTable, NEAR);
        almost_a_furnace(&mut player.inventory);
        player
            .inventory
            .inner
            .insert(grid_slot(2, 2), stack(ItemId::Cobblestone, 1));
        player.inventory.floating = None;

        act(&mut player, &world, vec![InventoryAction::TakeCraftResult]);

        assert_eq!(player.inventory.floating, None);
        assert_eq!(player.inventory.inner.len(), 8);
    }

    #[test]
    fn a_broken_table_takes_the_3x3_grid_away() {
        let (mut player, mut world) = player_by(BlockId::CraftingTable, NEAR);
        almost_a_furnace(&mut player.inventory);
        act(
            &mut player,
            &world,
            vec![InventoryAction::OpenCraftingTable(TABLE)],
        );
        world.remove_block(TABLE);

        assert_eq!(
            furnace_from_a_table(&mut player, &world),
            Some(stack(ItemId::Cobblestone, 1))
        );
    }

    #[test]
    fn a_table_out_of_reach_takes_the_3x3_grid_away() {
        let (mut player, world) = player_by(BlockId::CraftingTable, NEAR);
        almost_a_furnace(&mut player.inventory);
        act(
            &mut player,
            &world,
            vec![InventoryAction::OpenCraftingTable(TABLE)],
        );
        player.position = FAR;

        assert_eq!(
            furnace_from_a_table(&mut player, &world),
            Some(stack(ItemId::Cobblestone, 1))
        );
    }

    #[test]
    fn nothing_is_crafted_while_items_lie_beyond_the_grid_in_use() {
        let mut inventory = Inventory::new();
        almost_a_furnace(&mut inventory);
        inventory
            .inner
            .insert(grid_slot(2, 2), stack(ItemId::Cobblestone, 1));

        assert_eq!(craft_result(&inventory, INVENTORY_GRID_SIZE), None);
        assert_eq!(
            craft_result(&inventory, CRAFTING_GRID_SIZE),
            Some(stack(ItemId::Furnace, 1))
        );
    }

    #[test]
    fn items_left_beyond_the_grid_can_be_taken_out_but_not_put_back() {
        let mut inventory = inventory_with(grid_slot(2, 2), stack(ItemId::Cobblestone, 3));

        apply(
            &mut inventory,
            InventoryAction::LeftClickSlot(grid_slot(2, 2)),
        );
        assert_eq!(inventory.floating, Some(stack(ItemId::Cobblestone, 3)));

        assert_eq!(
            apply_inventory_action(
                &mut inventory,
                InventoryAction::LeftClickSlot(grid_slot(2, 2)),
                |_| false
            ),
            Err(InventoryRefusal::InvalidSlot(grid_slot(2, 2)))
        );
        assert!(inventory.inner.is_empty());
    }

    #[test]
    fn a_table_out_of_reach_is_closed_at_the_next_input() {
        let (mut player, world) = player_by(BlockId::CraftingTable, NEAR);
        act(
            &mut player,
            &world,
            vec![InventoryAction::OpenCraftingTable(TABLE)],
        );
        player.position = FAR;

        act(&mut player, &world, vec![]);

        assert_eq!(player.inventory.open_table, None);
    }

    #[test]
    fn close_forgets_the_open_table() {
        let (mut player, world) = player_by(BlockId::CraftingTable, NEAR);
        act(
            &mut player,
            &world,
            vec![
                InventoryAction::OpenCraftingTable(TABLE),
                InventoryAction::Close,
            ],
        );
        assert_eq!(player.inventory.open_table, None);
    }

    #[test]
    fn an_input_applies_at_most_its_share_of_inventory_actions() {
        let mut player = Player::default();
        player.inventory.floating = Some(stack(ItemId::Dirt, max_dirt()));
        let input = PlayerFrameInput {
            inventory_actions: vec![InventoryAction::RightClickSlot(1); 40],
            ..Default::default()
        };

        simulate_inventory_actions(
            &mut player,
            &ChunkMap::default(),
            &input,
            CallerType::Server,
        );

        assert_eq!(
            player.inventory.inner[&1].nb,
            MAX_INVENTORY_ACTIONS_PER_INPUT as u32
        );
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

        simulate_inventory_actions(
            &mut player,
            &ChunkMap::default(),
            &input,
            CallerType::Server,
        );

        assert_eq!(player.inventory.inner.len(), 1);
        assert_eq!(player.inventory.inner[&7], stack(ItemId::Dirt, 5));
        assert_eq!(player.inventory.floating, None);
    }

    #[test]
    fn old_player_saves_without_a_floating_stack_still_load() {
        let inventory: Inventory = ron::from_str("(inner: {})").unwrap();
        assert_eq!(inventory, Inventory::new());
    }

    #[test]
    fn shift_click_moves_a_hotbar_stack_into_the_backpack() {
        let mut inventory = inventory_with(0, stack(ItemId::Dirt, 5));

        apply(&mut inventory, InventoryAction::QuickMoveSlot(0));

        assert_eq!(inventory.inner.get(&0), None);
        assert_eq!(inventory.inner[&HOTBAR_SLOTS], stack(ItemId::Dirt, 5));
    }

    #[test]
    fn shift_click_moves_a_backpack_stack_into_the_hotbar() {
        let mut inventory = inventory_with(20, stack(ItemId::Dirt, 5));

        apply(&mut inventory, InventoryAction::QuickMoveSlot(20));

        assert_eq!(inventory.inner.get(&20), None);
        assert_eq!(inventory.inner[&0], stack(ItemId::Dirt, 5));
    }

    #[test]
    fn shift_click_tops_up_stacks_of_the_same_item_first() {
        let mut inventory = inventory_with(3, stack(ItemId::Dirt, max_dirt() - 4));
        inventory.inner.insert(20, stack(ItemId::Dirt, 10));

        apply(&mut inventory, InventoryAction::QuickMoveSlot(20));

        assert_eq!(inventory.inner[&3], stack(ItemId::Dirt, max_dirt()));
        assert_eq!(inventory.inner[&0], stack(ItemId::Dirt, 6));
        assert_eq!(inventory.inner.get(&20), None);
    }

    #[test]
    fn shift_click_keeps_what_does_not_fit() {
        let mut inventory = inventory_with(20, stack(ItemId::Dirt, 5));
        for slot in 0..HOTBAR_SLOTS {
            inventory.inner.insert(slot, stack(ItemId::Stone, 64));
        }

        apply(&mut inventory, InventoryAction::QuickMoveSlot(20));

        assert_eq!(inventory.inner[&20], stack(ItemId::Dirt, 5));
    }

    #[test]
    fn shift_click_on_a_grid_cell_puts_it_back_into_the_inventory() {
        let cell = grid_slot(0, 0);
        let mut inventory = inventory_with(cell, stack(ItemId::OakPlanks, 4));

        apply(&mut inventory, InventoryAction::QuickMoveSlot(cell));

        assert_eq!(inventory.inner.get(&cell), None);
        let (slot, moved) = inventory.inner.iter().next().unwrap();
        assert!(*slot < MAX_INVENTORY_SLOTS);
        assert_eq!(*moved, stack(ItemId::OakPlanks, 4));
    }

    #[test]
    fn shift_click_leaves_the_held_stack_alone() {
        let mut inventory = inventory_with(0, stack(ItemId::Dirt, 5));
        inventory.floating = Some(stack(ItemId::Stick, 3));

        apply(&mut inventory, InventoryAction::QuickMoveSlot(0));

        assert_eq!(inventory.floating, Some(stack(ItemId::Stick, 3)));
    }

    #[test]
    fn a_number_key_swaps_a_slot_with_that_hotbar_slot() {
        let mut inventory = inventory_with(12, stack(ItemId::Dirt, 5));
        inventory.inner.insert(2, stack(ItemId::Stick, 3));

        let swap = InventoryAction::SwapWithHotbar {
            slot: 12,
            hotbar: 2,
        };
        apply(&mut inventory, swap);
        assert_eq!(inventory.inner[&12], stack(ItemId::Stick, 3));
        assert_eq!(inventory.inner[&2], stack(ItemId::Dirt, 5));

        let mut inventory = inventory_with(12, stack(ItemId::Dirt, 5));
        apply(
            &mut inventory,
            InventoryAction::SwapWithHotbar {
                slot: 12,
                hotbar: 4,
            },
        );
        assert_eq!(inventory.inner.get(&12), None);
        assert_eq!(inventory.inner[&4], stack(ItemId::Dirt, 5));
    }

    #[test]
    fn a_number_key_is_refused_while_the_cursor_holds_a_stack() {
        let mut inventory = inventory_with(12, stack(ItemId::Dirt, 5));
        inventory.floating = Some(stack(ItemId::Dirt, 1));
        let before = inventory.clone();

        let swap = InventoryAction::SwapWithHotbar {
            slot: 12,
            hotbar: 2,
        };
        let refusal = apply_inventory_action(&mut inventory, swap, |_| false);

        assert_eq!(refusal, Err(InventoryRefusal::HoldingAStack));
        assert_eq!(inventory, before);
    }

    #[test]
    fn a_number_key_on_a_cell_beyond_the_grid_is_refused() {
        let mut inventory = Inventory::new();
        let cell = grid_slot(2, 2);

        let swap = InventoryAction::SwapWithHotbar {
            slot: cell,
            hotbar: 0,
        };
        let refusal = apply_inventory_action(&mut inventory, swap, |_| false);

        assert_eq!(refusal, Err(InventoryRefusal::InvalidSlot(cell)));
    }

    #[test]
    fn a_number_key_beyond_the_hotbar_is_refused() {
        let mut inventory = inventory_with(12, stack(ItemId::Dirt, 5));

        let swap = InventoryAction::SwapWithHotbar {
            slot: 12,
            hotbar: HOTBAR_SLOTS,
        };
        let refusal = apply_inventory_action(&mut inventory, swap, |_| false);

        assert_eq!(refusal, Err(InventoryRefusal::InvalidSlot(HOTBAR_SLOTS)));
    }
}
