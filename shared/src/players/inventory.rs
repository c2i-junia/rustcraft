//! What an inventory action does: the clicks on the slots of the open inventory, which move
//! items between the slots and the stack held by the cursor (the floating stack), crafting
//! from the grid, using a crafting table, a chest or a furnace, and closing it. Run by the
//! client (prediction) and by the server (authority).

use std::ops::Range;

use bevy::math::{IVec3, Vec3};
use serde::{Deserialize, Serialize};

use crate::{
    messages::PlayerFrameInput,
    players::{
        blocks::{CallerType, INTERACTION_DISTANCE},
        Inventory, Player,
    },
    world::{
        fuel_ticks, grid_slot, match_recipe, recipes, smelting_result, BlockId, ChunkMap,
        Container, CraftingGrid, Furnace, ItemId, ItemStack, ItemType, CHEST_SLOTS,
        CRAFTING_GRID_SIZE, CRAFTING_GRID_START,
    },
    HALF_BLOCK, HOTBAR_SLOTS, MAX_INVENTORY_ACTIONS_PER_INPUT, MAX_INVENTORY_SLOTS,
};

/// Width and height of the crafting grid of the inventory alone.
pub const INVENTORY_GRID_SIZE: u32 = 2;

/// The slots of an open chest follow those of the crafting grid.
pub const CHEST_SLOT_START: u32 = CRAFTING_GRID_START + CRAFTING_GRID_SIZE * CRAFTING_GRID_SIZE;

/// The slots of an open chest, in the numbering of the inventory actions.
pub fn chest_slots() -> Range<u32> {
    CHEST_SLOT_START..CHEST_SLOT_START + CHEST_SLOTS
}

/// The slots of an open furnace follow those of a chest. Its input takes anything.
pub const FURNACE_INPUT_SLOT: u32 = CHEST_SLOT_START + CHEST_SLOTS;
/// The fuel slot of an open furnace takes only a fuel.
pub const FURNACE_FUEL_SLOT: u32 = FURNACE_INPUT_SLOT + 1;
/// The output of an open furnace takes nothing: it only gives.
pub const FURNACE_OUTPUT_SLOT: u32 = FURNACE_INPUT_SLOT + 2;

/// The slots of an open furnace, in the numbering of the inventory actions.
pub fn furnace_slots() -> Range<u32> {
    FURNACE_INPUT_SLOT..FURNACE_OUTPUT_SLOT + 1
}

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
    /// Right click on the chest at this position: its slots can be used while it stays within
    /// reach.
    OpenChest(IVec3),
    /// Creative only, from the item browser: a new stack of `count` `item`, clamped to
    /// `1..=max_stack`, goes into the inventory (hotbar first, as a pick-up) with
    /// `to_inventory`, else to the cursor, which may already hold some of the same item.
    TakeFromCatalog {
        item: ItemId,
        count: u32,
        to_inventory: bool,
    },
    /// Creative only: the stack held by the cursor disappears.
    DestroyFloating,
    /// The crafting grid goes back into the inventory, then the ingredients of the recipe of
    /// this index (`crate::world::recipes`) go from the inventory into the grid, at its top left
    /// corner, `times` times at most: as many as the inventory holds and a stack takes.
    FillCraftingGrid {
        recipe: u16,
        times: u32,
    },
    /// Right click on the furnace, lit or out, at this position: its slots can be used while it
    /// stays within reach.
    OpenFurnace(IVec3),
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
    /// There is no chest within reach at this position.
    NoChest(IVec3),
    /// The cursor holds a stack, which a swap with the hotbar would leave out.
    HoldingAStack,
    /// The action is for a player in Creative.
    NotInCreative,
    /// No recipe has this index.
    UnknownRecipe(u16),
    /// The recipe is larger than the crafting grid in use.
    DoesNotFit,
    /// The inventory lacks the ingredients of one round of the recipe.
    MissingIngredients,
    /// The inventory has no room for what the crafting grid holds.
    GridNotEmptied,
    /// There is no furnace, lit or out, within reach at this position.
    NoFurnace(IVec3),
    /// The slot does not take the item held: the fuel slot of a furnace takes only a fuel, its
    /// output nothing.
    NotTaken(u32),
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
pub fn grid_size(inventory: &Inventory, in_reach: impl Fn(IVec3, BlockId) -> bool) -> u32 {
    match inventory.open_table {
        Some(table) if in_reach(table, BlockId::CraftingTable) => CRAFTING_GRID_SIZE,
        _ => INVENTORY_GRID_SIZE,
    }
}

/// Whether `pos` holds a block `id` within the reach of a player at `position`.
fn block_in_reach(world_map: &ChunkMap, position: Vec3, pos: IVec3, id: BlockId) -> bool {
    world_map.get_block(pos).is_some_and(|block| block.id == id)
        && (pos.as_vec3() + HALF_BLOCK).distance(position) <= INTERACTION_DISTANCE
}

/// Whether `pos` holds a furnace, lit or out, `in_reach`.
fn furnace_in_reach(in_reach: impl Fn(IVec3, BlockId) -> bool, pos: IVec3) -> bool {
    in_reach(pos, BlockId::Furnace) || in_reach(pos, BlockId::LitFurnace)
}

/// What the block a player has open holds, lent to its inventory while an action applies.
pub enum OpenContents<'a> {
    Chest(&'a mut Container),
    Furnace(&'a mut Furnace),
}

/// The slots of an open block lent to the inventory while an action applies.
#[derive(Clone, Copy, PartialEq)]
enum Lent {
    Nothing,
    Chest,
    Furnace,
}

/// Applies the inventory actions of `input`, in order, up to `MAX_INVENTORY_ACTIONS_PER_INPUT`.
/// The actions on the open chest or furnace change it in `world_map`, when it holds it.
pub fn simulate_inventory_actions(
    player: &mut Player,
    world_map: &mut ChunkMap,
    input: &PlayerFrameInput,
    caller_type: CallerType,
) {
    let position = player.position;
    // A table, a chest or a furnace the player walked away from, or that was broken, is
    // closed: the dialog shrinks
    let inventory = &mut player.inventory;
    if let Some(table) = inventory.open_table {
        if !block_in_reach(world_map, position, table, BlockId::CraftingTable) {
            inventory.open_table = None;
        }
    }
    if let Some(chest) = inventory.open_chest {
        if !block_in_reach(world_map, position, chest, BlockId::Chest) {
            inventory.open_chest = None;
        }
    }
    if let Some(furnace) = inventory.open_furnace {
        if !furnace_in_reach(
            |pos, id| block_in_reach(world_map, position, pos, id),
            furnace,
        ) {
            inventory.open_furnace = None;
        }
    }
    let creative = player.game_mode.is_creative();
    let actions = input.inventory_actions.iter();
    for &action in actions.take(MAX_INVENTORY_ACTIONS_PER_INPUT) {
        // A client learns what a chest or a furnace holds from the server, after opening it
        let inventory = &player.inventory;
        let mut chest = inventory
            .open_chest
            .and_then(|pos| Some((pos, world_map.container(pos)?.clone())));
        let mut furnace = inventory
            .open_furnace
            .and_then(|pos| Some((pos, world_map.furnace(pos)?.clone())));
        let contents = match (&mut chest, &mut furnace) {
            (Some((_, contents)), _) => Some(OpenContents::Chest(contents)),
            (None, Some((_, furnace))) => Some(OpenContents::Furnace(furnace)),
            (None, None) => None,
        };
        let applied = apply_inventory_action(
            &mut player.inventory,
            action,
            |pos, id| block_in_reach(world_map, position, pos, id),
            contents,
            creative,
        );
        if let Some((pos, contents)) = chest {
            if world_map.container(pos) != Some(&contents) {
                if let Some(stored) = world_map.container_mut(pos) {
                    *stored = contents;
                }
            }
        }
        if let Some((pos, furnace)) = furnace {
            if world_map.furnace(pos) != Some(&furnace) {
                if let Some(stored) = world_map.furnace_mut(pos) {
                    *stored = furnace;
                }
            }
        }
        if let Err(refusal) = applied {
            log::debug!(
                "{} Player {} could not apply {action:?}: {refusal:?}",
                caller_type.as_str(),
                player.id
            );
        }
    }
}

/// Applies `action` to `inventory`. `in_reach` tells whether a position holds a block of this
/// kind the player can use. `contents` is what the open chest or furnace holds, `None` when it
/// is unknown: its slots cannot be used then. The actions of the item browser's catalog need
/// `creative`.
pub fn apply_inventory_action(
    inventory: &mut Inventory,
    action: InventoryAction,
    in_reach: impl Fn(IVec3, BlockId) -> bool,
    contents: Option<OpenContents>,
    creative: bool,
) -> Result<(), InventoryRefusal> {
    let creative_only = matches!(
        action,
        InventoryAction::TakeFromCatalog { .. } | InventoryAction::DestroyFloating
    );
    if creative_only && !creative {
        return Err(InventoryRefusal::NotInCreative);
    }
    // The stacks of the chest or the furnace are lent to the inventory, in its slots, while
    // the action is applied: every click works the same on them
    match contents {
        Some(OpenContents::Chest(chest)) if inventory.open_chest.is_some() => {
            let stacks: Vec<_> = chest.slots.iter_mut().collect();
            lend(inventory, stacks, chest_slots(), |inventory| {
                apply_action(inventory, action, in_reach, Lent::Chest)
            })
        }
        Some(OpenContents::Furnace(furnace)) if inventory.open_furnace.is_some() => {
            let cooked = furnace.input.map(|stack| stack.item_id);
            let stacks = vec![&mut furnace.input, &mut furnace.fuel, &mut furnace.output];
            let applied = lend(inventory, stacks, furnace_slots(), |inventory| {
                apply_action(inventory, action, in_reach, Lent::Furnace)
            });
            // As in Minecraft, another item starts from nothing: what cooked was not it
            if furnace.input.map(|stack| stack.item_id) != cooked {
                furnace.cook = 0;
            }
            applied
        }
        _ => apply_action(inventory, action, in_reach, Lent::Nothing),
    }
}

/// Lends `stacks` to `inventory` in `slots` while `apply` runs, and takes back what the slots
/// hold then.
fn lend(
    inventory: &mut Inventory,
    mut stacks: Vec<&mut Option<ItemStack>>,
    slots: Range<u32>,
    apply: impl FnOnce(&mut Inventory) -> Result<(), InventoryRefusal>,
) -> Result<(), InventoryRefusal> {
    debug_assert!(slots
        .clone()
        .all(|slot| !inventory.inner.contains_key(&slot)));
    for (slot, stack) in slots.clone().zip(&stacks) {
        if let Some(stack) = stack {
            inventory.inner.insert(slot, *stack);
        }
    }
    let applied = apply(inventory);
    for (slot, stack) in slots.zip(&mut stacks) {
        **stack = inventory.inner.remove(&slot);
    }
    applied
}

/// `apply_inventory_action`, with the stacks of the open chest or furnace in its slots of
/// `inventory`, as `lent` says.
fn apply_action(
    inventory: &mut Inventory,
    action: InventoryAction,
    in_reach: impl Fn(IVec3, BlockId) -> bool,
    lent: Lent,
) -> Result<(), InventoryRefusal> {
    let size = grid_size(inventory, &in_reach);
    let mut floating = inventory.floating.take();
    let applied = match action {
        InventoryAction::LeftClickSlot(slot) => check_slot(inventory, slot, size, floating, lent)
            .and_then(|()| left_click_slot(inventory, slot, &mut floating)),
        InventoryAction::RightClickSlot(slot) => check_slot(inventory, slot, size, floating, lent)
            .and_then(|()| right_click_slot(inventory, slot, &mut floating)),
        InventoryAction::QuickMoveSlot(slot) => check_slot(inventory, slot, size, floating, lent)
            .map(|()| quick_move_slot(inventory, slot, lent)),
        InventoryAction::SwapWithHotbar { slot, hotbar } => {
            swap_with_hotbar(inventory, slot, hotbar, size, floating, lent)
        }
        InventoryAction::Close => {
            inventory.open_table = None;
            inventory.open_chest = None;
            inventory.open_furnace = None;
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
            if in_reach(table, BlockId::CraftingTable) {
                inventory.open_table = Some(table);
                inventory.open_chest = None;
                inventory.open_furnace = None;
                Ok(())
            } else {
                Err(InventoryRefusal::NoTable(table))
            }
        }
        InventoryAction::OpenChest(chest) => {
            if in_reach(chest, BlockId::Chest) {
                inventory.open_chest = Some(chest);
                inventory.open_table = None;
                inventory.open_furnace = None;
                Ok(())
            } else {
                Err(InventoryRefusal::NoChest(chest))
            }
        }
        InventoryAction::OpenFurnace(furnace) => {
            if furnace_in_reach(&in_reach, furnace) {
                inventory.open_furnace = Some(furnace);
                inventory.open_table = None;
                inventory.open_chest = None;
                Ok(())
            } else {
                Err(InventoryRefusal::NoFurnace(furnace))
            }
        }
        InventoryAction::TakeFromCatalog {
            item,
            count,
            to_inventory,
        } => take_from_catalog(inventory, &mut floating, item, count, to_inventory),
        InventoryAction::DestroyFloating => {
            floating = None;
            Ok(())
        }
        InventoryAction::FillCraftingGrid { recipe, times } => {
            fill_crafting_grid(inventory, recipe, times, size)
        }
    };
    inventory.floating = floating;
    applied
}

/// A new stack of `count` `item`, clamped to a stack, into the inventory or onto the cursor,
/// which takes no other item.
fn take_from_catalog(
    inventory: &mut Inventory,
    floating: &mut Option<ItemStack>,
    item: ItemId,
    count: u32,
    to_inventory: bool,
) -> Result<(), InventoryRefusal> {
    let max_stack = item.get_max_stack();
    let stack = ItemStack {
        item_id: item,
        item_type: item.get_default_type(),
        nb: count.clamp(1, max_stack),
    };
    if to_inventory {
        // What does not fit is lost: the catalog has more
        inventory.add_item_to_inventory(stack);
        return Ok(());
    }
    match floating {
        None => *floating = Some(stack),
        Some(held) if held.item_id == item => held.nb = (held.nb + stack.nb).min(max_stack),
        Some(_) => return Err(InventoryRefusal::CursorFull),
    }
    Ok(())
}

/// Puts the crafting grid back into the inventory, then the ingredients of `recipe` into the grid
/// of `size`, unmirrored, at its top left corner: `times` rounds at most, as many as the inventory
/// holds and a stack of each ingredient takes. All or nothing.
fn fill_crafting_grid(
    inventory: &mut Inventory,
    recipe: u16,
    times: u32,
    size: u32,
) -> Result<(), InventoryRefusal> {
    let recipe = recipes()
        .nth(recipe as usize)
        .ok_or(InventoryRefusal::UnknownRecipe(recipe))?;
    if !recipe.fits(size) {
        return Err(InventoryRefusal::DoesNotFit);
    }
    let mut filled = inventory.clone();
    for slot in grid_slots(CRAFTING_GRID_SIZE) {
        if let Some(stack) = filled.inner.remove(&slot) {
            if put_back(&mut filled, stack).is_some() {
                return Err(InventoryRefusal::GridNotEmptied);
            }
        }
    }
    let held = |inventory: &Inventory, item: ItemId| -> u32 {
        (0..MAX_INVENTORY_SLOTS)
            .filter_map(|slot| inventory.inner.get(&slot))
            .filter(|stack| stack.item_id == item)
            .map(|stack| stack.nb)
            .sum()
    };
    let rounds = recipe
        .ingredients()
        .into_iter()
        .map(|(item, cells)| (held(&filled, item) / cells).min(item.get_max_stack()))
        .min()
        .unwrap_or(0)
        .min(times);
    if rounds == 0 {
        return Err(InventoryRefusal::MissingIngredients);
    }
    for (index, cell) in recipe.cells.iter().enumerate() {
        let Some(item) = *cell else {
            continue;
        };
        let item_type = take_from_inventory(&mut filled, item, rounds);
        let (row, col) = (index / recipe.width, index % recipe.width);
        let stack = ItemStack {
            item_id: item,
            item_type,
            nb: rounds,
        };
        filled
            .inner
            .insert(grid_slot(row as u32, col as u32), stack);
    }
    *inventory = filled;
    Ok(())
}

/// Takes `nb` `item` from the inventory, which holds them, the last slots of the backpack first
/// and the hotbar last, and returns the type of the stacks they came from.
fn take_from_inventory(inventory: &mut Inventory, item: ItemId, mut nb: u32) -> ItemType {
    let mut item_type = item.get_default_type();
    for slot in (0..MAX_INVENTORY_SLOTS).rev() {
        if nb == 0 {
            break;
        }
        if let Some(stack) = inventory.inner.get(&slot).filter(|s| s.item_id == item) {
            item_type = stack.item_type;
            nb -= inventory.remove_item_from_stack(slot, nb);
        }
    }
    item_type
}

/// Adds `stack` to the inventory and returns what did not fit.
fn put_back(inventory: &mut Inventory, stack: ItemStack) -> Option<ItemStack> {
    let kept = inventory.add_item_to_inventory(stack);
    (kept > 0).then_some(ItemStack { nb: kept, ..stack })
}

/// Shift+click on `slot`: moves its stack from the hotbar to the backpack or the other way
/// round, and from a crafting grid cell into the whole inventory. With a chest open, it moves
/// it from the inventory to the chest and from the chest to the inventory; with a furnace open,
/// from the inventory to the input if it smelts, else to the fuel slot if it burns, and from
/// the furnace to the inventory. What does not fit stays.
fn quick_move_slot(inventory: &mut Inventory, slot: u32, lent: Lent) {
    let Some(stack) = inventory.inner.remove(&slot) else {
        return;
    };
    let range = if chest_slots().contains(&slot) || furnace_slots().contains(&slot) {
        Some(0..MAX_INVENTORY_SLOTS)
    } else if slot >= MAX_INVENTORY_SLOTS {
        None
    } else {
        match lent {
            Lent::Chest => Some(chest_slots()),
            Lent::Furnace if smelting_result(stack.item_id).is_some() => {
                Some(FURNACE_INPUT_SLOT..FURNACE_FUEL_SLOT)
            }
            Lent::Furnace if fuel_ticks(stack.item_id).is_some() => {
                Some(FURNACE_FUEL_SLOT..FURNACE_OUTPUT_SLOT)
            }
            // Neither smelts nor burns: it stays
            Lent::Furnace => Some(slot..slot),
            Lent::Nothing => None,
        }
    };
    let kept = if let Some(range) = range {
        let kept = inventory.add_item_to_range(stack, range);
        (kept > 0).then_some(ItemStack { nb: kept, ..stack })
    } else if slot < HOTBAR_SLOTS {
        let kept = inventory.add_item_to_range(stack, HOTBAR_SLOTS..MAX_INVENTORY_SLOTS);
        (kept > 0).then_some(ItemStack { nb: kept, ..stack })
    } else if slot < MAX_INVENTORY_SLOTS {
        let kept = inventory.add_item_to_range(stack, 0..HOTBAR_SLOTS);
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
    lent: Lent,
) -> Result<(), InventoryRefusal> {
    if hotbar >= HOTBAR_SLOTS {
        return Err(InventoryRefusal::InvalidSlot(hotbar));
    }
    if floating.is_some() {
        return Err(InventoryRefusal::HoldingAStack);
    }
    if slot >= MAX_INVENTORY_SLOTS
        && !grid_slots(size).any(|cell| cell == slot)
        && !is_lent(slot, lent)
    {
        return Err(InventoryRefusal::InvalidSlot(slot));
    }
    if let Some(held) = inventory.inner.get(&hotbar) {
        if !takes(slot, held.item_id) {
            return Err(InventoryRefusal::NotTaken(slot));
        }
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

/// Whether `slot` is one of the open chest or furnace whose stacks are `lent`.
fn is_lent(slot: u32, lent: Lent) -> bool {
    match lent {
        Lent::Nothing => false,
        Lent::Chest => chest_slots().contains(&slot),
        Lent::Furnace => furnace_slots().contains(&slot),
    }
}

/// Whether `item` may be put into `slot`: the output of a furnace takes nothing, its fuel slot
/// only a fuel, every other slot anything.
fn takes(slot: u32, item: ItemId) -> bool {
    match slot {
        FURNACE_OUTPUT_SLOT => false,
        FURNACE_FUEL_SLOT => fuel_ticks(item).is_some(),
        _ => true,
    }
}

/// A click may reach the inventory, the grid in use and the open chest or furnace. Cells
/// beyond the grid may only be emptied, with nothing in the cursor.
fn check_slot(
    inventory: &Inventory,
    slot: u32,
    size: u32,
    floating: Option<ItemStack>,
    lent: Lent,
) -> Result<(), InventoryRefusal> {
    let takes_a_leftover =
        is_beyond_the_grid(slot, size) && floating.is_none() && inventory.inner.contains_key(&slot);
    if slot < MAX_INVENTORY_SLOTS
        || grid_slots(size).any(|cell| cell == slot)
        || takes_a_leftover
        || is_lent(slot, lent)
    {
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
/// item and the slot has room, and swaps the two otherwise. A slot that does not take the
/// floating item only gives: the floating stack takes from it what it can hold when both hold
/// the same item.
fn left_click_slot(
    inventory: &mut Inventory,
    slot: u32,
    floating_stack: &mut Option<ItemStack>,
) -> Result<(), InventoryRefusal> {
    let in_slot = inventory.inner.get(&slot).copied();

    if let Some(floating) = *floating_stack {
        if !takes(slot, floating.item_id) {
            let Some(in_slot) = in_slot.filter(|in_slot| in_slot.item_id == floating.item_id)
            else {
                return Err(InventoryRefusal::NotTaken(slot));
            };
            let room = floating.item_id.get_max_stack().saturating_sub(floating.nb);
            let taken = inventory.remove_item_from_stack(slot, room);
            add_item_floating_stack(floating_stack, taken, in_slot.item_id, in_slot.item_type);
            return Ok(());
        }
    }

    if let (Some(in_slot), Some(floating)) = (in_slot, *floating_stack) {
        let room = in_slot.item_id.get_max_stack().saturating_sub(in_slot.nb);
        if in_slot.item_id == floating.item_id && room > 0 {
            let moved = remove_item_floating_stack(floating_stack, room);
            inventory.add_item_to_stack(slot, moved, floating.item_id, in_slot.item_type);
            return Ok(());
        }
    }

    match floating_stack.take() {
        Some(floating) => inventory.inner.insert(slot, floating),
        None => inventory.inner.remove(&slot),
    };
    *floating_stack = in_slot;
    Ok(())
}

/// Right click on `slot`: with a floating stack, drops one of its items into the slot if the
/// slot takes it and is empty or holds the same item; without one, picks up half of the slot,
/// rounded up.
fn right_click_slot(
    inventory: &mut Inventory,
    slot: u32,
    floating_stack: &mut Option<ItemStack>,
) -> Result<(), InventoryRefusal> {
    let in_slot = inventory.inner.get(&slot).copied();

    match (*floating_stack, in_slot) {
        (Some(floating), _) if !takes(slot, floating.item_id) => {
            return Err(InventoryRefusal::NotTaken(slot));
        }
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
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{grid_slot, BlockId, ChunkMap, Furnace, RecipeView};
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
        apply_inventory_action(inventory, action, |_, _| false, None, false).unwrap();
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
                |_, _| false,
                None,
                false,
            ),
            Err(InventoryRefusal::InvalidSlot(MAX_INVENTORY_SLOTS + 50))
        );
        assert_eq!(
            apply_inventory_action(
                &mut inventory,
                InventoryAction::RightClickSlot(1000),
                |_, _| false,
                None,
                false,
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
                apply_inventory_action(
                    &mut inventory,
                    InventoryAction::TakeCraftResult,
                    |_, _| false,
                    None,
                    false,
                ),
                Err(InventoryRefusal::CursorFull)
            );
            assert_eq!(inventory, before);
        }
    }

    #[test]
    fn taking_the_result_of_no_recipe_is_refused() {
        let mut inventory = inventory_with(grid_slot(0, 0), stack(ItemId::Dirt, 1));
        assert_eq!(
            apply_inventory_action(
                &mut inventory,
                InventoryAction::TakeCraftResult,
                |_, _| false,
                None,
                false,
            ),
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
                |_, _| false,
                None,
                false,
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

    fn act(player: &mut Player, world: &mut ChunkMap, actions: Vec<InventoryAction>) {
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

    fn furnace_from_a_table(player: &mut Player, world: &mut ChunkMap) -> Option<ItemStack> {
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
        let (mut player, mut world) = player_by(BlockId::CraftingTable, NEAR);
        act(
            &mut player,
            &mut world,
            vec![InventoryAction::OpenCraftingTable(TABLE)],
        );
        assert_eq!(player.inventory.open_table, Some(TABLE));

        let (mut player, mut world) = player_by(BlockId::CraftingTable, FAR);
        act(
            &mut player,
            &mut world,
            vec![InventoryAction::OpenCraftingTable(TABLE)],
        );
        assert_eq!(player.inventory.open_table, None);
    }

    #[test]
    fn a_block_that_is_not_a_table_does_not_open() {
        let (mut player, mut world) = player_by(BlockId::Stone, NEAR);
        act(
            &mut player,
            &mut world,
            vec![InventoryAction::OpenCraftingTable(TABLE)],
        );
        assert_eq!(player.inventory.open_table, None);
    }

    #[test]
    fn the_3x3_grid_is_usable_while_a_table_is_open() {
        let (mut player, mut world) = player_by(BlockId::CraftingTable, NEAR);
        almost_a_furnace(&mut player.inventory);
        act(
            &mut player,
            &mut world,
            vec![InventoryAction::OpenCraftingTable(TABLE)],
        );

        assert_eq!(
            furnace_from_a_table(&mut player, &mut world),
            Some(stack(ItemId::Furnace, 1))
        );
        assert!(player.inventory.inner.is_empty());
    }

    #[test]
    fn a_ring_of_cobblestone_makes_nothing_without_a_table() {
        let (mut player, mut world) = player_by(BlockId::CraftingTable, NEAR);
        almost_a_furnace(&mut player.inventory);
        player
            .inventory
            .inner
            .insert(grid_slot(2, 2), stack(ItemId::Cobblestone, 1));
        player.inventory.floating = None;

        act(
            &mut player,
            &mut world,
            vec![InventoryAction::TakeCraftResult],
        );

        assert_eq!(player.inventory.floating, None);
        assert_eq!(player.inventory.inner.len(), 8);
    }

    #[test]
    fn a_broken_table_takes_the_3x3_grid_away() {
        let (mut player, mut world) = player_by(BlockId::CraftingTable, NEAR);
        almost_a_furnace(&mut player.inventory);
        act(
            &mut player,
            &mut world,
            vec![InventoryAction::OpenCraftingTable(TABLE)],
        );
        world.remove_block(TABLE);

        assert_eq!(
            furnace_from_a_table(&mut player, &mut world),
            Some(stack(ItemId::Cobblestone, 1))
        );
    }

    #[test]
    fn a_table_out_of_reach_takes_the_3x3_grid_away() {
        let (mut player, mut world) = player_by(BlockId::CraftingTable, NEAR);
        almost_a_furnace(&mut player.inventory);
        act(
            &mut player,
            &mut world,
            vec![InventoryAction::OpenCraftingTable(TABLE)],
        );
        player.position = FAR;

        assert_eq!(
            furnace_from_a_table(&mut player, &mut world),
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
                |_, _| false,
                None,
                false,
            ),
            Err(InventoryRefusal::InvalidSlot(grid_slot(2, 2)))
        );
        assert!(inventory.inner.is_empty());
    }

    #[test]
    fn a_table_out_of_reach_is_closed_at_the_next_input() {
        let (mut player, mut world) = player_by(BlockId::CraftingTable, NEAR);
        act(
            &mut player,
            &mut world,
            vec![InventoryAction::OpenCraftingTable(TABLE)],
        );
        player.position = FAR;

        act(&mut player, &mut world, vec![]);

        assert_eq!(player.inventory.open_table, None);
    }

    #[test]
    fn close_forgets_the_open_table() {
        let (mut player, mut world) = player_by(BlockId::CraftingTable, NEAR);
        act(
            &mut player,
            &mut world,
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
            &mut ChunkMap::default(),
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
            &mut ChunkMap::default(),
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
        let refusal = apply_inventory_action(&mut inventory, swap, |_, _| false, None, false);

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
        let refusal = apply_inventory_action(&mut inventory, swap, |_, _| false, None, false);

        assert_eq!(refusal, Err(InventoryRefusal::InvalidSlot(cell)));
    }

    #[test]
    fn a_number_key_beyond_the_hotbar_is_refused() {
        let mut inventory = inventory_with(12, stack(ItemId::Dirt, 5));

        let swap = InventoryAction::SwapWithHotbar {
            slot: 12,
            hotbar: HOTBAR_SLOTS,
        };
        let refusal = apply_inventory_action(&mut inventory, swap, |_, _| false, None, false);

        assert_eq!(refusal, Err(InventoryRefusal::InvalidSlot(HOTBAR_SLOTS)));
    }

    const CHEST: IVec3 = TABLE;

    /// A player by an open chest at `CHEST`, which holds `contents`.
    fn by_an_open_chest(contents: Container) -> (Player, ChunkMap) {
        let (mut player, mut world) = player_by(BlockId::Chest, NEAR);
        world.insert_container(CHEST, contents);
        act(
            &mut player,
            &mut world,
            vec![InventoryAction::OpenChest(CHEST)],
        );
        assert_eq!(player.inventory.open_chest, Some(CHEST));
        world.take_changed_containers();
        (player, world)
    }

    fn holding(slot: u32, stack: ItemStack) -> Container {
        let mut contents = Container::default();
        contents.slots[slot as usize] = Some(stack);
        contents
    }

    fn chest_slot(index: u32) -> u32 {
        CHEST_SLOT_START + index
    }

    #[test]
    fn the_chest_slots_follow_the_crafting_grid() {
        assert_eq!(chest_slots(), 45..72);
        assert_eq!(CHEST_SLOT_START, grid_slot(2, 2) + 1);
    }

    #[test]
    fn a_chest_opens_only_within_reach_and_closes_the_table() {
        let (mut player, mut world) = player_by(BlockId::Chest, FAR);
        act(
            &mut player,
            &mut world,
            vec![InventoryAction::OpenChest(CHEST)],
        );
        assert_eq!(player.inventory.open_chest, None);

        let (mut player, mut world) = player_by(BlockId::Stone, NEAR);
        act(
            &mut player,
            &mut world,
            vec![InventoryAction::OpenChest(CHEST)],
        );
        assert_eq!(player.inventory.open_chest, None);

        let (mut player, mut world) = player_by(BlockId::Chest, NEAR);
        player.inventory.open_table = Some(IVec3::ZERO);
        act(
            &mut player,
            &mut world,
            vec![InventoryAction::OpenChest(CHEST)],
        );
        assert_eq!(player.inventory.open_chest, Some(CHEST));
        assert_eq!(player.inventory.open_table, None);
    }

    #[test]
    fn clicks_move_items_between_the_chest_and_the_inventory() {
        let (mut player, mut world) = by_an_open_chest(holding(4, stack(ItemId::Stone, 10)));
        player.inventory.inner.insert(0, stack(ItemId::Dirt, 3));

        act(
            &mut player,
            &mut world,
            vec![
                InventoryAction::RightClickSlot(chest_slot(4)),
                InventoryAction::LeftClickSlot(9),
                InventoryAction::LeftClickSlot(0),
                InventoryAction::LeftClickSlot(chest_slot(26)),
            ],
        );

        let mut expected = holding(4, stack(ItemId::Stone, 5));
        expected.slots[26] = Some(stack(ItemId::Dirt, 3));
        assert_eq!(world.container(CHEST), Some(&expected));
        assert_eq!(player.inventory.inner.len(), 1);
        assert_eq!(player.inventory.inner[&9], stack(ItemId::Stone, 5));
        assert_eq!(player.inventory.floating, None);
        assert_eq!(world.take_changed_containers(), vec![CHEST]);
    }

    #[test]
    fn shift_click_moves_a_stack_into_the_chest_and_back() {
        let (mut player, mut world) = by_an_open_chest(holding(0, stack(ItemId::Dirt, 60)));
        player.inventory.inner.insert(3, stack(ItemId::Dirt, 10));

        act(
            &mut player,
            &mut world,
            vec![InventoryAction::QuickMoveSlot(3)],
        );

        let mut expected = holding(0, stack(ItemId::Dirt, 64));
        expected.slots[1] = Some(stack(ItemId::Dirt, 6));
        assert_eq!(world.container(CHEST), Some(&expected));
        assert!(player.inventory.inner.is_empty());

        act(
            &mut player,
            &mut world,
            vec![InventoryAction::QuickMoveSlot(chest_slot(1))],
        );

        assert_eq!(
            world.container(CHEST),
            Some(&holding(0, stack(ItemId::Dirt, 64)))
        );
        assert_eq!(player.inventory.inner[&0], stack(ItemId::Dirt, 6));
    }

    #[test]
    fn a_number_key_swaps_a_chest_slot_with_the_hotbar() {
        let (mut player, mut world) = by_an_open_chest(holding(2, stack(ItemId::Stone, 1)));
        player.inventory.inner.insert(5, stack(ItemId::Dirt, 3));

        act(
            &mut player,
            &mut world,
            vec![InventoryAction::SwapWithHotbar {
                slot: chest_slot(2),
                hotbar: 5,
            }],
        );

        assert_eq!(
            world.container(CHEST),
            Some(&holding(2, stack(ItemId::Dirt, 3)))
        );
        assert_eq!(player.inventory.inner[&5], stack(ItemId::Stone, 1));
    }

    #[test]
    fn the_chest_slots_are_refused_without_an_open_chest_and_shift_click_stays_in_the_inventory() {
        let (mut player, mut world) = player_by(BlockId::Chest, NEAR);
        world.insert_container(CHEST, holding(0, stack(ItemId::Stone, 1)));
        player.inventory.inner.insert(0, stack(ItemId::Dirt, 3));

        act(
            &mut player,
            &mut world,
            vec![
                InventoryAction::LeftClickSlot(chest_slot(0)),
                InventoryAction::QuickMoveSlot(0),
            ],
        );

        assert_eq!(
            world.container(CHEST),
            Some(&holding(0, stack(ItemId::Stone, 1)))
        );
        assert_eq!(player.inventory.floating, None);
        assert_eq!(
            player.inventory.inner[&HOTBAR_SLOTS],
            stack(ItemId::Dirt, 3)
        );
        assert!(world.take_changed_containers().is_empty());
    }

    #[test]
    fn the_slots_of_a_chest_whose_contents_are_unknown_are_refused() {
        // A client opening a chest, before the server sent what it holds
        let (mut player, mut world) = player_by(BlockId::Chest, NEAR);
        act(
            &mut player,
            &mut world,
            vec![InventoryAction::OpenChest(CHEST)],
        );
        player.inventory.floating = Some(stack(ItemId::Dirt, 3));

        act(
            &mut player,
            &mut world,
            vec![InventoryAction::LeftClickSlot(chest_slot(0))],
        );

        assert_eq!(player.inventory.floating, Some(stack(ItemId::Dirt, 3)));
        assert!(player.inventory.inner.is_empty());
        assert_eq!(player.inventory.open_chest, Some(CHEST));
    }

    #[test]
    fn closing_keeps_the_contents_in_the_chest() {
        let (mut player, mut world) = by_an_open_chest(holding(0, stack(ItemId::Stone, 8)));

        act(
            &mut player,
            &mut world,
            vec![
                InventoryAction::LeftClickSlot(chest_slot(0)),
                InventoryAction::LeftClickSlot(chest_slot(1)),
                InventoryAction::Close,
            ],
        );

        assert_eq!(player.inventory.open_chest, None);
        assert!(player.inventory.inner.is_empty());
        assert_eq!(
            world.container(CHEST),
            Some(&holding(1, stack(ItemId::Stone, 8)))
        );
    }

    #[test]
    fn a_chest_out_of_reach_or_broken_is_closed() {
        let (mut player, mut world) = by_an_open_chest(holding(0, stack(ItemId::Stone, 8)));
        player.position = FAR;
        act(
            &mut player,
            &mut world,
            vec![InventoryAction::LeftClickSlot(chest_slot(0))],
        );
        assert_eq!(player.inventory.open_chest, None);
        assert_eq!(player.inventory.floating, None);

        let (mut player, mut world) = by_an_open_chest(holding(0, stack(ItemId::Stone, 8)));
        world.remove_block(CHEST);
        act(&mut player, &mut world, Vec::new());
        assert_eq!(player.inventory.open_chest, None);
    }

    const FURNACE: IVec3 = TABLE;

    /// A player by an open furnace at `FURNACE`, which holds `furnace`.
    fn by_an_open_furnace(furnace: Furnace) -> (Player, ChunkMap) {
        let (mut player, mut world) = player_by(BlockId::Furnace, NEAR);
        world.insert_furnace(FURNACE, furnace);
        act(
            &mut player,
            &mut world,
            vec![InventoryAction::OpenFurnace(FURNACE)],
        );
        assert_eq!(player.inventory.open_furnace, Some(FURNACE));
        world.take_changed_furnaces();
        (player, world)
    }

    fn furnace_holding(
        input: Option<ItemStack>,
        fuel: Option<ItemStack>,
        output: Option<ItemStack>,
    ) -> Furnace {
        Furnace {
            input,
            fuel,
            output,
            ..Default::default()
        }
    }

    #[test]
    fn the_furnace_slots_follow_the_chest_slots() {
        assert_eq!(furnace_slots(), 72..75);
        assert_eq!(
            [FURNACE_INPUT_SLOT, FURNACE_FUEL_SLOT, FURNACE_OUTPUT_SLOT],
            [72, 73, 74]
        );
        assert_eq!(FURNACE_INPUT_SLOT, chest_slots().end);
    }

    #[test]
    fn a_furnace_lit_or_out_opens_within_reach_and_closes_a_table_or_a_chest() {
        for id in [BlockId::Furnace, BlockId::LitFurnace] {
            let (mut player, mut world) = player_by(id, FAR);
            act(
                &mut player,
                &mut world,
                vec![InventoryAction::OpenFurnace(FURNACE)],
            );
            assert_eq!(player.inventory.open_furnace, None, "{id:?}");

            for (table, chest) in [(Some(IVec3::ZERO), None), (None, Some(IVec3::ZERO))] {
                let (mut player, mut world) = player_by(id, NEAR);
                player.inventory.open_table = table;
                player.inventory.open_chest = chest;
                act(
                    &mut player,
                    &mut world,
                    vec![InventoryAction::OpenFurnace(FURNACE)],
                );
                assert_eq!(player.inventory.open_furnace, Some(FURNACE), "{id:?}");
                assert_eq!(player.inventory.open_table, None);
                assert_eq!(player.inventory.open_chest, None);
            }
        }

        let (mut player, mut world) = player_by(BlockId::Stone, NEAR);
        let open = InventoryAction::OpenFurnace(FURNACE);
        let refusal = apply_inventory_action(
            &mut player.inventory,
            open,
            |pos, id| world.get_block(pos).is_some_and(|block| block.id == id),
            None,
            false,
        );
        assert_eq!(refusal, Err(InventoryRefusal::NoFurnace(FURNACE)));
        act(&mut player, &mut world, vec![open]);
        assert_eq!(player.inventory.open_furnace, None);
    }

    #[test]
    fn opening_a_table_or_a_chest_closes_the_furnace() {
        for (id, open) in [
            (
                BlockId::CraftingTable,
                InventoryAction::OpenCraftingTable(TABLE),
            ),
            (BlockId::Chest, InventoryAction::OpenChest(TABLE)),
        ] {
            let (mut player, mut world) = player_by(id, NEAR);
            player.inventory.open_furnace = Some(IVec3::ZERO);
            act(&mut player, &mut world, vec![open]);
            assert_eq!(player.inventory.open_furnace, None, "{id:?}");
        }
    }

    #[test]
    fn clicks_move_items_between_the_furnace_and_the_inventory() {
        let (mut player, mut world) =
            by_an_open_furnace(furnace_holding(Some(stack(ItemId::Sand, 10)), None, None));
        player.inventory.inner.insert(0, stack(ItemId::Coal, 3));

        act(
            &mut player,
            &mut world,
            vec![
                InventoryAction::RightClickSlot(FURNACE_INPUT_SLOT),
                InventoryAction::LeftClickSlot(9),
                InventoryAction::LeftClickSlot(0),
                InventoryAction::LeftClickSlot(FURNACE_FUEL_SLOT),
            ],
        );

        let expected = furnace_holding(
            Some(stack(ItemId::Sand, 5)),
            Some(stack(ItemId::Coal, 3)),
            None,
        );
        assert_eq!(world.furnace(FURNACE), Some(&expected));
        assert_eq!(player.inventory.inner.len(), 1);
        assert_eq!(player.inventory.inner[&9], stack(ItemId::Sand, 5));
        assert_eq!(player.inventory.floating, None);
        assert_eq!(world.take_changed_furnaces(), vec![FURNACE]);
    }

    #[test]
    fn the_fuel_slot_takes_only_fuels() {
        let (mut player, mut world) = by_an_open_furnace(Furnace::default());
        player.inventory.inner.insert(0, stack(ItemId::Sand, 4));
        player.inventory.inner.insert(1, stack(ItemId::Coal, 2));
        let fuel = |world: &ChunkMap| world.furnace(FURNACE).unwrap().fuel;

        act(
            &mut player,
            &mut world,
            vec![
                InventoryAction::LeftClickSlot(0),
                InventoryAction::LeftClickSlot(FURNACE_FUEL_SLOT),
                InventoryAction::RightClickSlot(FURNACE_FUEL_SLOT),
            ],
        );
        assert_eq!(fuel(&world), None);
        assert_eq!(player.inventory.floating, Some(stack(ItemId::Sand, 4)));

        act(
            &mut player,
            &mut world,
            vec![
                InventoryAction::LeftClickSlot(0),
                InventoryAction::SwapWithHotbar {
                    slot: FURNACE_FUEL_SLOT,
                    hotbar: 0,
                },
            ],
        );
        assert_eq!(fuel(&world), None);
        assert_eq!(player.inventory.inner[&0], stack(ItemId::Sand, 4));

        act(
            &mut player,
            &mut world,
            vec![InventoryAction::SwapWithHotbar {
                slot: FURNACE_FUEL_SLOT,
                hotbar: 1,
            }],
        );
        assert_eq!(fuel(&world), Some(stack(ItemId::Coal, 2)));
        assert!(!player.inventory.inner.contains_key(&1));
    }

    #[test]
    fn the_output_gives_and_takes_nothing() {
        let (mut player, mut world) =
            by_an_open_furnace(furnace_holding(None, None, Some(stack(ItemId::Glass, 10))));
        let output = |world: &ChunkMap| world.furnace(FURNACE).unwrap().output;

        // A cursor holding the same item takes what it can hold
        player.inventory.floating = Some(stack(ItemId::Glass, 60));
        act(
            &mut player,
            &mut world,
            vec![
                InventoryAction::LeftClickSlot(FURNACE_OUTPUT_SLOT),
                InventoryAction::RightClickSlot(FURNACE_OUTPUT_SLOT),
            ],
        );
        assert_eq!(player.inventory.floating, Some(stack(ItemId::Glass, 64)));
        assert_eq!(output(&world), Some(stack(ItemId::Glass, 6)));

        // Another item is neither put there nor swapped
        player.inventory.floating = Some(stack(ItemId::Sand, 1));
        act(
            &mut player,
            &mut world,
            vec![InventoryAction::LeftClickSlot(FURNACE_OUTPUT_SLOT)],
        );
        assert_eq!(player.inventory.floating, Some(stack(ItemId::Sand, 1)));
        assert_eq!(output(&world), Some(stack(ItemId::Glass, 6)));

        player.inventory.floating = None;
        player.inventory.inner.insert(5, stack(ItemId::Sand, 1));
        act(
            &mut player,
            &mut world,
            vec![
                InventoryAction::SwapWithHotbar {
                    slot: FURNACE_OUTPUT_SLOT,
                    hotbar: 5,
                },
                InventoryAction::SwapWithHotbar {
                    slot: FURNACE_OUTPUT_SLOT,
                    hotbar: 6,
                },
            ],
        );
        assert_eq!(player.inventory.inner[&5], stack(ItemId::Sand, 1));
        assert_eq!(player.inventory.inner[&6], stack(ItemId::Glass, 6));
        assert_eq!(output(&world), None);

        // An empty cursor takes the whole stack
        world.furnace_mut(FURNACE).unwrap().output = Some(stack(ItemId::Glass, 3));
        act(
            &mut player,
            &mut world,
            vec![InventoryAction::LeftClickSlot(FURNACE_OUTPUT_SLOT)],
        );
        assert_eq!(player.inventory.floating, Some(stack(ItemId::Glass, 3)));
        assert_eq!(output(&world), None);
    }

    #[test]
    fn shift_click_sends_what_smelts_to_the_input_a_fuel_to_the_fuel_and_keeps_the_rest() {
        let (mut player, mut world) = by_an_open_furnace(Furnace::default());
        for (slot, item, nb) in [
            (0, ItemId::Cobblestone, 10),
            (1, ItemId::Coal, 5),
            (2, ItemId::Dirt, 3),
            (12, ItemId::Sand, 64),
        ] {
            player.inventory.inner.insert(slot, stack(item, nb));
        }

        act(
            &mut player,
            &mut world,
            [0, 1, 2, 12].map(InventoryAction::QuickMoveSlot).to_vec(),
        );

        // the sand finds the input taken by the cobblestone, and the dirt neither smelts nor burns
        let expected = furnace_holding(
            Some(stack(ItemId::Cobblestone, 10)),
            Some(stack(ItemId::Coal, 5)),
            None,
        );
        assert_eq!(world.furnace(FURNACE), Some(&expected));
        assert_eq!(player.inventory.inner.len(), 2);
        assert_eq!(player.inventory.inner[&2], stack(ItemId::Dirt, 3));
        assert_eq!(player.inventory.inner[&12], stack(ItemId::Sand, 64));
    }

    #[test]
    fn shift_click_on_a_furnace_slot_sends_its_stack_to_the_inventory() {
        let (mut player, mut world) = by_an_open_furnace(furnace_holding(
            Some(stack(ItemId::Sand, 3)),
            Some(stack(ItemId::Coal, 2)),
            Some(stack(ItemId::Glass, 5)),
        ));

        act(
            &mut player,
            &mut world,
            [FURNACE_OUTPUT_SLOT, FURNACE_FUEL_SLOT, FURNACE_INPUT_SLOT]
                .map(InventoryAction::QuickMoveSlot)
                .to_vec(),
        );

        assert_eq!(world.furnace(FURNACE), Some(&Furnace::default()));
        assert_eq!(player.inventory.inner[&0], stack(ItemId::Glass, 5));
        assert_eq!(player.inventory.inner[&1], stack(ItemId::Coal, 2));
        assert_eq!(player.inventory.inner[&2], stack(ItemId::Sand, 3));
    }

    #[test]
    fn closing_keeps_the_stacks_in_the_furnace() {
        let (mut player, mut world) =
            by_an_open_furnace(furnace_holding(Some(stack(ItemId::Sand, 3)), None, None));

        act(
            &mut player,
            &mut world,
            vec![
                InventoryAction::RightClickSlot(FURNACE_INPUT_SLOT),
                InventoryAction::Close,
            ],
        );

        assert_eq!(player.inventory.open_furnace, None);
        assert_eq!(player.inventory.inner[&0], stack(ItemId::Sand, 2));
        assert_eq!(
            world.furnace(FURNACE),
            Some(&furnace_holding(Some(stack(ItemId::Sand, 1)), None, None))
        );
    }

    #[test]
    fn another_item_in_the_input_starts_cooking_afresh_and_more_of_it_does_not() {
        let cooking = Furnace {
            cook: crate::world::COOK_TICKS - 1,
            ..furnace_holding(Some(stack(ItemId::Sand, 2)), None, None)
        };
        let (mut player, mut world) = by_an_open_furnace(cooking.clone());
        player.inventory.floating = Some(stack(ItemId::Sand, 1));
        act(
            &mut player,
            &mut world,
            vec![InventoryAction::LeftClickSlot(FURNACE_INPUT_SLOT)],
        );
        assert_eq!(
            world.furnace(FURNACE).unwrap().cook,
            crate::world::COOK_TICKS - 1
        );

        let (mut player, mut world) = by_an_open_furnace(cooking);
        player.inventory.floating = Some(stack(ItemId::Cobblestone, 1));
        act(
            &mut player,
            &mut world,
            vec![InventoryAction::LeftClickSlot(FURNACE_INPUT_SLOT)],
        );
        let furnace = world.furnace(FURNACE).unwrap();
        assert_eq!(furnace.input, Some(stack(ItemId::Cobblestone, 1)));
        assert_eq!(furnace.cook, 0);
    }

    #[test]
    fn a_furnace_stays_open_as_it_lights_and_closes_out_of_reach_or_broken() {
        let (mut player, mut world) = by_an_open_furnace(Furnace::default());
        world.set_block(
            FURNACE,
            crate::world::BlockData::new(BlockId::LitFurnace, crate::world::BlockDirection::Back),
        );
        act(&mut player, &mut world, Vec::new());
        assert_eq!(player.inventory.open_furnace, Some(FURNACE));

        player.position = FAR;
        act(&mut player, &mut world, Vec::new());
        assert_eq!(player.inventory.open_furnace, None);

        let (mut player, mut world) = by_an_open_furnace(Furnace::default());
        world.remove_block(FURNACE);
        act(&mut player, &mut world, Vec::new());
        assert_eq!(player.inventory.open_furnace, None);
    }

    #[test]
    fn the_furnace_slots_are_refused_unless_its_furnace_is_open_and_known() {
        let sand = furnace_holding(Some(stack(ItemId::Sand, 3)), None, None);
        let click = vec![InventoryAction::LeftClickSlot(FURNACE_INPUT_SLOT)];

        // Not open
        let (mut player, mut world) = player_by(BlockId::Furnace, NEAR);
        world.insert_furnace(FURNACE, sand.clone());
        act(&mut player, &mut world, click.clone());
        assert_eq!(player.inventory.floating, None);
        assert_eq!(world.furnace(FURNACE), Some(&sand));

        // Open by a client the server has not sent its state yet
        let (mut player, mut world) = player_by(BlockId::Furnace, NEAR);
        act(
            &mut player,
            &mut world,
            vec![InventoryAction::OpenFurnace(FURNACE)],
        );
        player.inventory.floating = Some(stack(ItemId::Dirt, 3));
        act(&mut player, &mut world, click.clone());
        assert_eq!(player.inventory.floating, Some(stack(ItemId::Dirt, 3)));
        assert!(player.inventory.inner.is_empty());

        // A chest open instead
        let (mut player, mut world) = by_an_open_chest(Container::default());
        player.inventory.floating = Some(stack(ItemId::Dirt, 3));
        act(&mut player, &mut world, click);
        assert_eq!(player.inventory.floating, Some(stack(ItemId::Dirt, 3)));
        assert!(player.inventory.inner.is_empty());
    }

    #[test]
    fn a_worn_tool_keeps_its_wear_through_every_click() {
        let worn = ItemStack {
            item_type: ItemType::Tool { wear: 12 },
            ..stack(ItemId::WoodenAxe, 1)
        };
        let mut inventory = inventory_with(0, worn);
        for action in [
            InventoryAction::LeftClickSlot(0),
            InventoryAction::LeftClickSlot(5),
            InventoryAction::RightClickSlot(5),
            InventoryAction::RightClickSlot(20),
            InventoryAction::QuickMoveSlot(20),
            InventoryAction::SwapWithHotbar { slot: 0, hotbar: 3 },
        ] {
            apply(&mut inventory, action);
            let stacks: Vec<_> = inventory
                .inner
                .values()
                .chain(&inventory.floating)
                .collect();
            assert_eq!(stacks, [&worn], "after {action:?}");
        }
        assert_eq!(inventory.inner[&3], worn);
    }

    #[test]
    fn a_tool_in_the_cursor_takes_no_second_one() {
        let worn = ItemStack {
            item_type: ItemType::Tool { wear: 40 },
            ..stack(ItemId::WoodenAxe, 1)
        };
        let mut inventory = inventory_with(0, worn);
        inventory.floating = Some(stack(ItemId::WoodenAxe, 1));
        apply(&mut inventory, InventoryAction::RightClickSlot(0));
        // Neither tool takes the wear of the other
        assert_eq!(inventory.inner[&0], worn);
        assert_eq!(inventory.floating, Some(stack(ItemId::WoodenAxe, 1)));
    }

    fn creative(
        inventory: &mut Inventory,
        action: InventoryAction,
    ) -> Result<(), InventoryRefusal> {
        apply_inventory_action(inventory, action, |_, _| false, None, true)
    }

    fn take(item: ItemId, count: u32, to_inventory: bool) -> InventoryAction {
        InventoryAction::TakeFromCatalog {
            item,
            count,
            to_inventory,
        }
    }

    #[test]
    fn taking_from_the_catalog_needs_creative() {
        let mut inventory = Inventory::new();
        inventory.floating = Some(stack(ItemId::Dirt, 5));
        let before = inventory.clone();
        for action in [
            take(ItemId::Stone, 64, false),
            take(ItemId::Stone, 64, true),
            InventoryAction::DestroyFloating,
        ] {
            assert_eq!(
                apply_inventory_action(&mut inventory, action, |_, _| false, None, false),
                Err(InventoryRefusal::NotInCreative)
            );
        }
        assert_eq!(inventory, before);

        // The game mode of the player decides, on the server as in the prediction
        let mut player = Player::default();
        act(
            &mut player,
            &mut ChunkMap::default(),
            vec![take(ItemId::Stone, 64, true)],
        );
        assert!(player.inventory.inner.is_empty());
        player.game_mode = crate::players::game_mode::GameMode::Creative;
        act(
            &mut player,
            &mut ChunkMap::default(),
            vec![take(ItemId::Stone, 64, true)],
        );
        assert_eq!(player.inventory.inner[&0], stack(ItemId::Stone, 64));
    }

    #[test]
    fn taking_from_the_catalog_clamps_the_count() {
        let mut inventory = Inventory::new();
        creative(&mut inventory, take(ItemId::Dirt, 0, false)).unwrap();
        assert_eq!(inventory.floating, Some(stack(ItemId::Dirt, 1)));

        let mut inventory = Inventory::new();
        creative(&mut inventory, take(ItemId::Dirt, 100, false)).unwrap();
        assert_eq!(inventory.floating, Some(stack(ItemId::Dirt, 64)));

        // A tool comes one at a time, new
        let mut inventory = Inventory::new();
        creative(&mut inventory, take(ItemId::WoodenPickaxe, 5, false)).unwrap();
        assert_eq!(inventory.floating, Some(stack(ItemId::WoodenPickaxe, 1)));
        assert_eq!(
            inventory.floating.unwrap().item_type,
            ItemType::Tool { wear: 0 }
        );
    }

    #[test]
    fn taking_onto_a_cursor_holding_another_item_is_refused() {
        let mut inventory = Inventory::new();
        inventory.floating = Some(stack(ItemId::Dirt, 5));
        assert_eq!(
            creative(&mut inventory, take(ItemId::Stone, 1, false)),
            Err(InventoryRefusal::CursorFull)
        );
        assert_eq!(inventory.floating, Some(stack(ItemId::Dirt, 5)));
    }

    #[test]
    fn taking_onto_a_cursor_holding_the_same_item_tops_it_up() {
        let mut inventory = Inventory::new();
        inventory.floating = Some(stack(ItemId::Dirt, 63));
        creative(&mut inventory, take(ItemId::Dirt, 1, false)).unwrap();
        assert_eq!(inventory.floating, Some(stack(ItemId::Dirt, 64)));
        creative(&mut inventory, take(ItemId::Dirt, 1, false)).unwrap();
        assert_eq!(inventory.floating, Some(stack(ItemId::Dirt, 64)));
    }

    #[test]
    fn taking_to_the_inventory_fills_the_hotbar_first() {
        let mut inventory = inventory_with(20, stack(ItemId::Dirt, 60));
        inventory.inner.insert(0, stack(ItemId::Stone, 1));
        inventory.floating = Some(stack(ItemId::Sand, 2));
        creative(&mut inventory, take(ItemId::Dirt, 64, true)).unwrap();
        assert_eq!(inventory.inner[&20], stack(ItemId::Dirt, 64));
        assert_eq!(inventory.inner[&1], stack(ItemId::Dirt, 60));
        assert_eq!(inventory.floating, Some(stack(ItemId::Sand, 2)));
    }

    #[test]
    fn destroying_the_floating_stack_empties_the_cursor() {
        let mut inventory = inventory_with(0, stack(ItemId::Dirt, 5));
        inventory.floating = Some(stack(ItemId::Stone, 64));
        creative(&mut inventory, InventoryAction::DestroyFloating).unwrap();
        assert_eq!(inventory.floating, None);
        assert_eq!(inventory.inner[&0], stack(ItemId::Dirt, 5));
    }

    fn fill(recipe: RecipeView, times: u32) -> InventoryAction {
        InventoryAction::FillCraftingGrid {
            recipe: recipe.index,
            times,
        }
    }

    fn oak_sticks() -> RecipeView {
        crate::world::recipes_for(ItemId::Stick).next().unwrap()
    }

    #[test]
    fn filling_the_grid_returns_the_grid_first() {
        let mut inventory = inventory_with(0, planks(2));
        inventory
            .inner
            .insert(grid_slot(1, 1), stack(ItemId::Stone, 3));
        apply(&mut inventory, fill(oak_sticks(), 1));
        assert_eq!(inventory.inner[&1], stack(ItemId::Stone, 3));
        assert_eq!(inventory.inner[&grid_slot(0, 0)], planks(1));
        assert_eq!(inventory.inner[&grid_slot(1, 0)], planks(1));
        assert_eq!(inventory.inner.len(), 3);
    }

    #[test]
    fn filling_the_grid_puts_the_recipe_at_its_top_left_corner_unmirrored() {
        // The axe is not symmetric: its sticks go in the right column
        let axe = crate::world::recipes_for(ItemId::WoodenAxe).next().unwrap();
        let mut inventory = inventory_with(0, planks(3));
        inventory.inner.insert(1, stack(ItemId::Stick, 2));
        inventory.open_table = Some(TABLE);
        apply_inventory_action(&mut inventory, fill(axe, 1), |_, _| true, None, false).unwrap();
        for (row, col) in [(0, 0), (0, 1), (1, 0)] {
            assert_eq!(inventory.inner[&grid_slot(row, col)], planks(1));
        }
        for (row, col) in [(1, 1), (2, 1)] {
            assert_eq!(
                inventory.inner[&grid_slot(row, col)],
                stack(ItemId::Stick, 1)
            );
        }
        assert_eq!(inventory.inner.len(), 5);
        assert_eq!(
            craft_result(&inventory, CRAFTING_GRID_SIZE).map(|made| made.item_id),
            Some(ItemId::WoodenAxe)
        );
    }

    #[test]
    fn filling_the_grid_needs_a_recipe_that_fits_the_grid_in_use() {
        let axe = crate::world::recipes_for(ItemId::WoodenAxe).next().unwrap();
        let mut inventory = inventory_with(0, planks(3));
        inventory.inner.insert(1, stack(ItemId::Stick, 2));
        let before = inventory.clone();
        assert_eq!(
            apply_inventory_action(&mut inventory, fill(axe, 1), |_, _| false, None, false),
            Err(InventoryRefusal::DoesNotFit)
        );
        assert_eq!(inventory, before);
        // At a table it fits
        inventory.open_table = Some(TABLE);
        apply_inventory_action(&mut inventory, fill(axe, 1), |_, _| true, None, false).unwrap();
        // The sticks fit the inventory's grid
        let mut inventory = inventory_with(0, planks(2));
        apply(&mut inventory, fill(oak_sticks(), 1));
        assert_eq!(inventory.inner[&grid_slot(1, 0)], planks(1));
    }

    #[test]
    fn filling_the_grid_clamps_the_rounds_to_the_inventory_and_the_max_stack() {
        let mut inventory = inventory_with(0, planks(4));
        inventory.inner.insert(30, planks(3));
        apply(&mut inventory, fill(oak_sticks(), u32::MAX));
        assert_eq!(inventory.inner[&grid_slot(0, 0)], planks(3));
        assert_eq!(inventory.inner[&grid_slot(1, 0)], planks(3));
        // Taken from the end of the backpack first, the hotbar last
        assert_eq!(inventory.inner[&0], planks(1));
        assert!(!inventory.inner.contains_key(&30));

        let logs = crate::world::recipes_for(ItemId::OakPlanks).next().unwrap();
        let mut inventory = Inventory::new();
        for slot in 0..4 {
            inventory.inner.insert(slot, stack(ItemId::OakLog, 50));
        }
        apply(&mut inventory, fill(logs, u32::MAX));
        assert_eq!(inventory.inner[&grid_slot(0, 0)], stack(ItemId::OakLog, 64));
        // And to what was asked
        apply(&mut inventory, fill(logs, 2));
        assert_eq!(inventory.inner[&grid_slot(0, 0)], stack(ItemId::OakLog, 2));
        let left: u32 = (0..MAX_INVENTORY_SLOTS)
            .filter_map(|slot| inventory.inner.get(&slot))
            .map(|stack| stack.nb)
            .sum();
        assert_eq!(left, 198);
    }

    #[test]
    fn filling_the_grid_without_one_round_is_refused_and_changes_nothing() {
        let mut inventory = inventory_with(0, planks(1));
        inventory
            .inner
            .insert(grid_slot(0, 1), stack(ItemId::Stone, 1));
        let before = inventory.clone();
        assert_eq!(
            apply_inventory_action(
                &mut inventory,
                fill(oak_sticks(), 1),
                |_, _| false,
                None,
                false
            ),
            Err(InventoryRefusal::MissingIngredients)
        );
        assert_eq!(inventory, before);
    }

    #[test]
    fn filling_the_grid_keeps_everything_when_the_grid_cannot_go_back() {
        let mut inventory = Inventory::new();
        for slot in 0..MAX_INVENTORY_SLOTS {
            inventory.inner.insert(slot, stack(ItemId::Dirt, 64));
        }
        inventory.inner.insert(grid_slot(0, 0), planks(2));
        let before = inventory.clone();
        assert_eq!(
            apply_inventory_action(
                &mut inventory,
                fill(oak_sticks(), 1),
                |_, _| false,
                None,
                false
            ),
            Err(InventoryRefusal::GridNotEmptied)
        );
        assert_eq!(inventory, before);
    }

    #[test]
    fn filling_the_grid_works_in_survival() {
        let mut player = Player::default();
        player.inventory.inner.insert(4, planks(2));
        act(
            &mut player,
            &mut ChunkMap::default(),
            vec![fill(oak_sticks(), 1), InventoryAction::TakeCraftResult],
        );
        assert_eq!(player.inventory.floating, Some(stack(ItemId::Stick, 4)));
        assert!(player.inventory.inner.is_empty());
    }

    #[test]
    fn filling_the_grid_with_an_unknown_recipe_is_refused() {
        let mut inventory = inventory_with(0, planks(2));
        assert_eq!(
            apply_inventory_action(
                &mut inventory,
                InventoryAction::FillCraftingGrid {
                    recipe: u16::MAX,
                    times: 1
                },
                |_, _| false,
                None,
                false
            ),
            Err(InventoryRefusal::UnknownRecipe(u16::MAX))
        );
    }
}
