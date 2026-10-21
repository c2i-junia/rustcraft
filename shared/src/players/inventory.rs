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
        fuel_ticks, grid_slot, lacking_cells, match_recipe, plan_fill, recipes, smelting_result,
        BlockId, ChunkMap, Container, CraftingGrid, FillPlan, Furnace, ItemId, ItemStack, ItemType,
        RecipeView, CHEST_SLOTS, CRAFTING_GRID_SIZE, CRAFTING_GRID_START,
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
    /// Release of a drag over at least two slots with a held stack: `slots` has bit `n` set for
    /// slot `n`. `one_each`: right button, one item per slot; else left button, the stack split
    /// evenly.
    Drag {
        slots: u128,
        one_each: bool,
    },
    /// Shift+click on the result: crafts again and again into the inventory.
    CraftAll,
    /// Double click with a held stack: the stacks of its item gather onto the cursor.
    CollectAll,
    /// Shift+double click on a slot that held `item`: every other stack of `item` on that side
    /// moves across, as a shift-click each.
    MoveAllOfType {
        slot: u32,
        item: ItemId,
    },
    /// Number key over the result: crafts once into this hotbar slot.
    CraftToHotbar(u32),
    /// Drop key over a slot, with an empty cursor: one item of its stack, or all of it, is
    /// thrown out of the inventory.
    DropSlot {
        slot: u32,
        all: bool,
    },
    /// A press outside the inventory with a held stack: one item of it, or all of it, is
    /// thrown out of the inventory.
    DropFloating {
        all: bool,
    },
}

/// Why an inventory action changed nothing.
#[derive(Debug, Clone, Copy, PartialEq)]
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
    /// The action needs a stack held by the cursor.
    NothingHeld,
    /// No slot has room for what the action puts there.
    NoRoom,
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

/// What one inventory action of an input did: the client's feedback.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct InventoryActionOutcome {
    pub action: InventoryAction,
    /// The stack held by the cursor before the action, and after it.
    pub held_before: Option<ItemStack>,
    pub held_after: Option<ItemStack>,
    pub result: Result<(), InventoryRefusal>,
    /// What the action threw out of the inventory, for the server to drop ahead of the player.
    pub thrown: Option<ItemStack>,
    /// The item a craft action the simulation accepted crafted: what the grid made before it.
    pub crafted: Option<ItemId>,
}

/// Applies the inventory actions of `input`, in order, up to `MAX_INVENTORY_ACTIONS_PER_INPUT`,
/// and returns what each of those did. The actions on the open chest or furnace change it in
/// `world_map`, when it holds it.
pub fn simulate_inventory_actions(
    player: &mut Player,
    world_map: &mut ChunkMap,
    input: &PlayerFrameInput,
    caller_type: CallerType,
) -> Vec<InventoryActionOutcome> {
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
    let mut outcomes = Vec::new();
    for &action in actions.take(MAX_INVENTORY_ACTIONS_PER_INPUT) {
        let held_before = player.inventory.floating;
        let is_craft = matches!(
            action,
            InventoryAction::TakeCraftResult
                | InventoryAction::CraftAll
                | InventoryAction::CraftToHotbar(_)
        );
        let made = is_craft
            .then(|| {
                let size = grid_size(&player.inventory, |pos, id| {
                    block_in_reach(world_map, position, pos, id)
                });
                craft_result(&player.inventory, size)
            })
            .flatten();
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
        outcomes.push(InventoryActionOutcome {
            action,
            held_before,
            held_after: player.inventory.floating,
            result: applied.map(|_| ()),
            thrown: applied.ok().flatten(),
            crafted: made.filter(|_| applied.is_ok()).map(|made| made.item_id),
        });
    }
    outcomes
}

/// Applies `action` to `inventory`, and returns the stack it threw out of the inventory, if
/// any. `in_reach` tells whether a position holds a block of this kind the player can use.
/// `contents` is what the open chest or furnace holds, `None` when it is unknown: its slots
/// cannot be used then. The actions of the item browser's catalog need `creative`.
pub fn apply_inventory_action(
    inventory: &mut Inventory,
    action: InventoryAction,
    in_reach: impl Fn(IVec3, BlockId) -> bool,
    contents: Option<OpenContents>,
    creative: bool,
) -> Result<Option<ItemStack>, InventoryRefusal> {
    let creative_only = matches!(
        action,
        InventoryAction::TakeFromCatalog { .. } | InventoryAction::DestroyFloating
    );
    if creative_only && !creative {
        return Err(InventoryRefusal::NotInCreative);
    }
    let mut thrown = None;
    with_lent(inventory, contents, |inventory, lent| {
        apply_action(inventory, action, &in_reach, lent, &mut thrown)
    })
    .map(|()| thrown)
}

/// What a drag leaves: the new stack of each slot it puts items into, in ascending order, and
/// what stays on the cursor.
#[derive(Debug, Clone, PartialEq)]
pub struct DragPlan {
    pub slots: Vec<(u32, ItemStack)>,
    pub held: Option<ItemStack>,
}

/// What `InventoryAction::Drag { slots, one_each }` would do to `inventory`, which it leaves
/// alone, as `apply_inventory_action` would apply it with `in_reach` and `contents`: what the
/// action does, and what the client previews while the drag lasts.
pub fn drag_plan(
    inventory: &Inventory,
    slots: u128,
    one_each: bool,
    in_reach: impl Fn(IVec3, BlockId) -> bool,
    contents: Option<OpenContents>,
) -> Result<DragPlan, InventoryRefusal> {
    let mut inventory = inventory.clone();
    let size = grid_size(&inventory, &in_reach);
    with_lent(&mut inventory, contents, |inventory, lent| {
        let floating = inventory.floating;
        plan_drag(inventory, slots, one_each, size, floating, lent)
    })
}

/// Runs `apply` on `inventory` with the stacks of the open chest or furnace lent to it, in its
/// slots, so that every click works the same on them, and tells it which are lent.
fn with_lent<T>(
    inventory: &mut Inventory,
    contents: Option<OpenContents>,
    apply: impl FnOnce(&mut Inventory, Lent) -> T,
) -> T {
    match contents {
        Some(OpenContents::Chest(chest)) if inventory.open_chest.is_some() => {
            let stacks: Vec<_> = chest.slots.iter_mut().collect();
            lend(inventory, stacks, chest_slots(), |inventory| {
                apply(inventory, Lent::Chest)
            })
        }
        Some(OpenContents::Furnace(furnace)) if inventory.open_furnace.is_some() => {
            let cooked = furnace.input.map(|stack| stack.item_id);
            let stacks = vec![&mut furnace.input, &mut furnace.fuel, &mut furnace.output];
            let applied = lend(inventory, stacks, furnace_slots(), |inventory| {
                apply(inventory, Lent::Furnace)
            });
            // As in Minecraft, another item starts from nothing: what cooked was not it
            if furnace.input.map(|stack| stack.item_id) != cooked {
                furnace.cook = 0;
            }
            applied
        }
        _ => apply(inventory, Lent::Nothing),
    }
}

/// Lends `stacks` to `inventory` in `slots` while `apply` runs, and takes back what the slots
/// hold then.
fn lend<T>(
    inventory: &mut Inventory,
    mut stacks: Vec<&mut Option<ItemStack>>,
    slots: Range<u32>,
    apply: impl FnOnce(&mut Inventory) -> T,
) -> T {
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
/// `inventory`, as `lent` says. What the action throws out goes to `thrown`.
fn apply_action(
    inventory: &mut Inventory,
    action: InventoryAction,
    in_reach: impl Fn(IVec3, BlockId) -> bool,
    lent: Lent,
    thrown: &mut Option<ItemStack>,
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
        InventoryAction::Drag { slots, one_each } => {
            plan_drag(inventory, slots, one_each, size, floating, lent).map(|plan| {
                inventory.inner.extend(plan.slots);
                floating = plan.held;
            })
        }
        InventoryAction::CraftAll => craft_all(inventory, size),
        InventoryAction::CraftToHotbar(hotbar) => craft_to_hotbar(inventory, hotbar, size),
        InventoryAction::CollectAll => collect_all(inventory, &mut floating, lent),
        InventoryAction::MoveAllOfType { slot, item } => {
            move_all_of_type(inventory, slot, item, size, floating, lent)
        }
        InventoryAction::DropSlot { slot, all } => {
            check_slot(inventory, slot, size, floating, lent).and_then(|()| {
                if floating.is_some() {
                    return Err(InventoryRefusal::HoldingAStack);
                }
                *thrown = take_from_slot(inventory, slot, all);
                Ok(())
            })
        }
        InventoryAction::DropFloating { all } => match floating {
            Some(held) => {
                let nb = remove_item_floating_stack(&mut floating, if all { held.nb } else { 1 });
                *thrown = Some(ItemStack { nb, ..held });
                Ok(())
            }
            None => Err(InventoryRefusal::NothingHeld),
        },
    };
    inventory.floating = floating;
    applied
}

/// The plan of a drag of `floating` over `slots` on `inventory`, whose lent slots are `lent`.
/// The slots it fills are those a click may reach that take the held item and are empty or
/// hold it with room, at most one per held item, in ascending order. Each gets the held count
/// shared between them, or one item with `one_each`, clamped to its room.
fn plan_drag(
    inventory: &Inventory,
    slots: u128,
    one_each: bool,
    size: u32,
    floating: Option<ItemStack>,
    lent: Lent,
) -> Result<DragPlan, InventoryRefusal> {
    let held = floating.ok_or(InventoryRefusal::NothingHeld)?;
    let max_stack = held.item_id.get_max_stack();
    let filled: Vec<(u32, Option<ItemStack>)> = (0..u128::BITS)
        .filter(|slot| slots & 1 << slot != 0)
        .filter(|&slot| {
            check_slot(inventory, slot, size, floating, lent).is_ok() && takes(slot, held.item_id)
        })
        .filter_map(|slot| match inventory.inner.get(&slot) {
            None => Some((slot, None)),
            Some(&in_slot) if in_slot.item_id == held.item_id && in_slot.nb < max_stack => {
                Some((slot, Some(in_slot)))
            }
            Some(_) => None,
        })
        .take(held.nb as usize)
        .collect();
    if filled.is_empty() {
        return Err(InventoryRefusal::NoRoom);
    }
    let share = if one_each {
        1
    } else {
        held.nb / filled.len() as u32
    };
    let mut left = held.nb;
    let mut plan = Vec::new();
    for (slot, in_slot) in filled {
        let room = max_stack - in_slot.map_or(0, |in_slot| in_slot.nb);
        let put = share.min(room).min(left);
        if put == 0 {
            break;
        }
        left -= put;
        let stack = match in_slot {
            Some(in_slot) => ItemStack {
                nb: in_slot.nb + put,
                ..in_slot
            },
            None => ItemStack { nb: put, ..held },
        };
        plan.push((slot, stack));
    }
    Ok(DragPlan {
        slots: plan,
        held: (left > 0).then_some(ItemStack { nb: left, ..held }),
    })
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

/// What filling the crafting grid with a recipe would do, planned on a copy of the inventory.
#[derive(Debug, Clone, PartialEq)]
pub struct GridFill {
    pub recipe: RecipeView,
    /// The inventory once the grid is back into it, from which the plan takes the ingredients.
    pub emptied: Inventory,
    /// What goes into the grid, `None` when the inventory lacks one round of the recipe.
    pub plan: Option<FillPlan>,
    /// The cells of the recipe, row after row, that one round leaves without their ingredient
    /// (`lacking_cells`).
    pub lacking: Vec<bool>,
}

/// What `InventoryAction::FillCraftingGrid { recipe, times }` would do with a grid of `size` in
/// use, on a copy of `inventory`: the grid put back into the inventory, then the fill planned
/// by `plan_fill`. Refused when no recipe has the index, when it does not fit the grid, or when
/// the inventory has no room for the grid; without one round of the recipe, the plan is `None`
/// and the action refused with `MissingIngredients`. The action applies it, and the client
/// previews it, so that both agree.
pub fn plan_grid_fill(
    inventory: &Inventory,
    recipe: u16,
    times: u32,
    size: u32,
) -> Result<GridFill, InventoryRefusal> {
    let recipe = recipes()
        .nth(recipe as usize)
        .ok_or(InventoryRefusal::UnknownRecipe(recipe))?;
    if !recipe.fits(size) {
        return Err(InventoryRefusal::DoesNotFit);
    }
    let mut emptied = inventory.clone();
    for slot in grid_slots(CRAFTING_GRID_SIZE) {
        if let Some(stack) = emptied.inner.remove(&slot) {
            if put_back(&mut emptied, stack).is_some() {
                return Err(InventoryRefusal::GridNotEmptied);
            }
        }
    }
    Ok(GridFill {
        plan: plan_fill(&emptied, &recipe, times),
        lacking: lacking_cells(&emptied, &recipe),
        recipe,
        emptied,
    })
}

/// Puts the crafting grid back into the inventory, then the ingredients of `recipe` into the grid
/// of `size`, unmirrored, at its top left corner, as `plan_grid_fill` plans it: `times` rounds at
/// most, as many as the inventory holds and a stack takes, any planks in a cell of planks. All or
/// nothing.
fn fill_crafting_grid(
    inventory: &mut Inventory,
    recipe: u16,
    times: u32,
    size: u32,
) -> Result<(), InventoryRefusal> {
    let GridFill {
        recipe,
        mut emptied,
        plan,
        ..
    } = plan_grid_fill(inventory, recipe, times, size)?;
    let plan = plan.ok_or(InventoryRefusal::MissingIngredients)?;
    for (index, cell) in plan.cells.into_iter().enumerate() {
        let Some(item) = cell else {
            continue;
        };
        let item_type = take_from_inventory(&mut emptied, item, plan.rounds);
        let (row, col) = (index / recipe.width, index % recipe.width);
        let stack = ItemStack {
            item_id: item,
            item_type,
            nb: plan.rounds,
        };
        emptied
            .inner
            .insert(grid_slot(row as u32, col as u32), stack);
    }
    *inventory = emptied;
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

/// Double click with a held stack: takes the stacks of its item from the inventory and from an
/// open chest, the smallest first and then in slot order, until the cursor holds a full stack.
/// Never from the crafting grid or a furnace.
fn collect_all(
    inventory: &mut Inventory,
    floating: &mut Option<ItemStack>,
    lent: Lent,
) -> Result<(), InventoryRefusal> {
    let held = floating.as_mut().ok_or(InventoryRefusal::NothingHeld)?;
    let max_stack = held.item_id.get_max_stack();
    if held.nb >= max_stack {
        return Err(InventoryRefusal::CursorFull);
    }
    let chest = match lent {
        Lent::Chest => chest_slots(),
        _ => 0..0,
    };
    let mut sources: Vec<(u32, u32)> = (0..MAX_INVENTORY_SLOTS)
        .chain(chest)
        .filter_map(|slot| {
            let stack = inventory.inner.get(&slot)?;
            (stack.item_id == held.item_id).then_some((stack.nb, slot))
        })
        .collect();
    sources.sort_unstable();
    for (_, slot) in sources {
        let room = max_stack - held.nb;
        if room == 0 {
            break;
        }
        held.nb += inventory.remove_item_from_stack(slot, room);
    }
    Ok(())
}

/// The slots a Shift+double click on `slot` moves from: the grid, the chest or the furnace it is
/// part of; with a chest or a furnace open, the whole inventory; else the hotbar or the backpack.
fn zone_of(slot: u32, lent: Lent) -> Vec<u32> {
    let grid: Vec<u32> = grid_slots(CRAFTING_GRID_SIZE).collect();
    if grid.contains(&slot) {
        grid
    } else if chest_slots().contains(&slot) {
        chest_slots().collect()
    } else if furnace_slots().contains(&slot) {
        furnace_slots().collect()
    } else if lent != Lent::Nothing {
        (0..MAX_INVENTORY_SLOTS).collect()
    } else if slot < HOTBAR_SLOTS {
        (0..HOTBAR_SLOTS).collect()
    } else {
        (HOTBAR_SLOTS..MAX_INVENTORY_SLOTS).collect()
    }
}

/// Shift+double click on `slot`: every stack of `item` in its zone is quick-moved, in slot order.
fn move_all_of_type(
    inventory: &mut Inventory,
    slot: u32,
    item: ItemId,
    size: u32,
    floating: Option<ItemStack>,
    lent: Lent,
) -> Result<(), InventoryRefusal> {
    check_slot(inventory, slot, size, floating, lent)?;
    for slot in zone_of(slot, lent) {
        let holds_it = inventory
            .inner
            .get(&slot)
            .is_some_and(|stack| stack.item_id == item);
        if holds_it && check_slot(inventory, slot, size, floating, lent).is_ok() {
            quick_move_slot(inventory, slot, lent);
        }
    }
    Ok(())
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

/// Takes one item of the stack in `slot`, or all of it, and returns what it took.
fn take_from_slot(inventory: &mut Inventory, slot: u32, all: bool) -> Option<ItemStack> {
    let stack = *inventory.inner.get(&slot)?;
    let nb = if all { stack.nb } else { 1 };
    let nb = inventory.remove_item_from_stack(slot, nb);
    Some(ItemStack { nb, ..stack })
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
    take_one_from_each_cell(inventory);
    Ok(())
}

/// The most rounds a Shift+click on the result crafts.
pub const MAX_CRAFT_ALL_ROUNDS: u32 = 64;

/// How many `item` the slots of `range` take: the room of its stacks, then of the empty slots.
fn room_for(inventory: &Inventory, item: ItemId, range: Range<u32>) -> u32 {
    let max_stack = item.get_max_stack();
    range
        .map(|slot| match inventory.inner.get(&slot) {
            None => max_stack,
            Some(stack) if stack.item_id == item => max_stack.saturating_sub(stack.nb),
            Some(_) => 0,
        })
        .sum()
}

/// Shift+click on the result: crafts round after round into the inventory, while the grid makes
/// the item of the first round and the inventory has room for a whole result, up to
/// `MAX_CRAFT_ALL_ROUNDS`. The floating stack is left alone.
fn craft_all(inventory: &mut Inventory, size: u32) -> Result<(), InventoryRefusal> {
    let first = craft_result(inventory, size).ok_or(InventoryRefusal::NoRecipe)?;
    let mut rounds = 0;
    while rounds < MAX_CRAFT_ALL_ROUNDS {
        let Some(result) =
            craft_result(inventory, size).filter(|result| result.item_id == first.item_id)
        else {
            break;
        };
        if room_for(inventory, result.item_id, 0..MAX_INVENTORY_SLOTS) < result.nb {
            break;
        }
        inventory.add_item_to_inventory(result);
        take_one_from_each_cell(inventory);
        rounds += 1;
    }
    if rounds == 0 {
        return Err(InventoryRefusal::NoRoom);
    }
    Ok(())
}

/// Number key over the result: crafts once into the slot `hotbar` of the hotbar, which must be
/// empty or hold the same item with room for the whole result.
fn craft_to_hotbar(
    inventory: &mut Inventory,
    hotbar: u32,
    size: u32,
) -> Result<(), InventoryRefusal> {
    if hotbar >= HOTBAR_SLOTS {
        return Err(InventoryRefusal::InvalidSlot(hotbar));
    }
    let result = craft_result(inventory, size).ok_or(InventoryRefusal::NoRecipe)?;
    if room_for(inventory, result.item_id, hotbar..hotbar + 1) < result.nb {
        return Err(InventoryRefusal::NoRoom);
    }
    inventory.add_item_to_stack(hotbar, result.nb, result.item_id, result.item_type);
    take_one_from_each_cell(inventory);
    Ok(())
}

/// What a craft takes from the grid: one item of each cell.
fn take_one_from_each_cell(inventory: &mut Inventory) {
    for slot in grid_slots(CRAFTING_GRID_SIZE) {
        inventory.remove_item_from_stack(slot, 1);
    }
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
    fn the_simulation_reports_what_each_action_did() {
        let mut player = Player::default();
        player.inventory.inner.insert(0, stack(ItemId::Dirt, 5));
        let input = PlayerFrameInput {
            inventory_actions: vec![
                InventoryAction::LeftClickSlot(0),
                InventoryAction::TakeCraftResult,
                InventoryAction::DropFloating { all: false },
            ],
            ..Default::default()
        };

        let outcomes = simulate_inventory_actions(
            &mut player,
            &mut ChunkMap::default(),
            &input,
            CallerType::Client,
        );

        let dirt = Some(stack(ItemId::Dirt, 5));
        assert_eq!(
            outcomes,
            [
                InventoryActionOutcome {
                    action: InventoryAction::LeftClickSlot(0),
                    held_before: None,
                    held_after: dirt,
                    result: Ok(()),
                    thrown: None,
                    crafted: None,
                },
                InventoryActionOutcome {
                    action: InventoryAction::TakeCraftResult,
                    held_before: dirt,
                    held_after: dirt,
                    result: Err(InventoryRefusal::NoRecipe),
                    thrown: None,
                    crafted: None,
                },
                InventoryActionOutcome {
                    action: InventoryAction::DropFloating { all: false },
                    held_before: dirt,
                    held_after: Some(stack(ItemId::Dirt, 4)),
                    result: Ok(()),
                    thrown: Some(stack(ItemId::Dirt, 1)),
                    crafted: None,
                },
            ]
        );
    }

    #[test]
    fn an_outcome_names_what_a_craft_action_crafted() {
        let crafted = |actions: Vec<InventoryAction>, logs: u32| {
            let mut player = Player::default();
            player
                .inventory
                .inner
                .insert(grid_slot(0, 0), stack(ItemId::OakLog, logs));
            let input = PlayerFrameInput {
                inventory_actions: actions,
                ..Default::default()
            };
            simulate_inventory_actions(
                &mut player,
                &mut ChunkMap::default(),
                &input,
                CallerType::Client,
            )
            .into_iter()
            .map(|outcome| outcome.crafted)
            .collect::<Vec<_>>()
        };
        let planks = Some(ItemId::OakPlanks);
        assert_eq!(crafted(vec![InventoryAction::TakeCraftResult], 1), [planks]);
        assert_eq!(crafted(vec![InventoryAction::CraftAll], 3), [planks]);
        assert_eq!(
            crafted(vec![InventoryAction::CraftToHotbar(4)], 1),
            [planks]
        );
        // A refused craft, or an action that crafts nothing, names nothing
        assert_eq!(
            crafted(
                vec![
                    InventoryAction::TakeCraftResult,
                    InventoryAction::TakeCraftResult,
                    InventoryAction::CraftToHotbar(9),
                ],
                1
            ),
            [planks, None, None]
        );
        assert_eq!(
            crafted(vec![InventoryAction::LeftClickSlot(grid_slot(0, 0))], 1),
            [None]
        );
    }

    #[test]
    fn the_simulation_reports_only_the_actions_it_applied() {
        let mut player = Player::default();
        let input = PlayerFrameInput {
            inventory_actions: vec![InventoryAction::LeftClickSlot(1); 40],
            ..Default::default()
        };

        let outcomes = simulate_inventory_actions(
            &mut player,
            &mut ChunkMap::default(),
            &input,
            CallerType::Client,
        );

        assert_eq!(outcomes.len(), MAX_INVENTORY_ACTIONS_PER_INPUT);
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
    fn every_slot_is_below_128() {
        // A drag carries its slots as the bits of a u128
        let last = (0..MAX_INVENTORY_SLOTS)
            .chain(grid_slots(CRAFTING_GRID_SIZE))
            .chain(chest_slots())
            .chain(furnace_slots())
            .max()
            .unwrap();
        assert_eq!(last, FURNACE_OUTPUT_SLOT);
        assert!(last < u128::BITS);
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
    ) -> Result<Option<ItemStack>, InventoryRefusal> {
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
    fn filling_the_grid_takes_one_species_of_planks_when_it_is_enough() {
        let chest = crate::world::recipes_for(ItemId::Chest).next().unwrap();
        let mut inventory = inventory_with(0, stack(ItemId::SprucePlanks, 3));
        inventory.inner.insert(1, stack(ItemId::OakPlanks, 10));
        inventory.open_table = Some(TABLE);
        apply_inventory_action(&mut inventory, fill(chest, 1), |_, _| true, None, false).unwrap();
        for slot in grid_slots(CRAFTING_GRID_SIZE) {
            let expected = (slot != grid_slot(1, 1)).then_some(stack(ItemId::OakPlanks, 1));
            assert_eq!(inventory.inner.get(&slot).copied(), expected, "{slot}");
        }
        assert_eq!(inventory.inner[&0], stack(ItemId::SprucePlanks, 3));
        assert_eq!(inventory.inner[&1], stack(ItemId::OakPlanks, 2));
    }

    #[test]
    fn filling_the_grid_mixes_species_of_planks_when_none_is_enough() {
        let chest = crate::world::recipes_for(ItemId::Chest).next().unwrap();
        let mut inventory = inventory_with(0, stack(ItemId::OakPlanks, 5));
        inventory.inner.insert(1, stack(ItemId::SprucePlanks, 4));
        inventory.open_table = Some(TABLE);
        apply_inventory_action(&mut inventory, fill(chest, 1), |_, _| true, None, false).unwrap();
        let oak = [(0, 0), (0, 1), (0, 2), (1, 0), (1, 2)];
        let spruce = [(2, 0), (2, 1), (2, 2)];
        for (row, col) in oak {
            assert_eq!(
                inventory.inner[&grid_slot(row, col)],
                stack(ItemId::OakPlanks, 1)
            );
        }
        for (row, col) in spruce {
            assert_eq!(
                inventory.inner[&grid_slot(row, col)],
                stack(ItemId::SprucePlanks, 1)
            );
        }
        assert!(!inventory.inner.contains_key(&0));
        assert_eq!(inventory.inner[&1], stack(ItemId::SprucePlanks, 1));
        assert_eq!(
            craft_result(&inventory, CRAFTING_GRID_SIZE).map(|made| made.item_id),
            Some(ItemId::Chest)
        );
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
    fn planning_a_fill_puts_the_grid_back_first() {
        let mut inventory = inventory_with(0, planks(1));
        inventory.inner.insert(grid_slot(1, 1), planks(1));
        let sticks = oak_sticks();
        let fill = plan_grid_fill(&inventory, sticks.index, 1, INVENTORY_GRID_SIZE).unwrap();
        assert_eq!(fill.recipe, sticks);
        assert_eq!(fill.emptied.inner[&0], planks(2));
        assert_eq!(fill.emptied.inner.len(), 1);
        assert_eq!(
            fill.plan,
            Some(FillPlan {
                rounds: 1,
                cells: vec![Some(ItemId::OakPlanks); 2]
            })
        );
        assert_eq!(fill.lacking, [false, false]);
        // The inventory is left alone
        assert_eq!(inventory.inner[&grid_slot(1, 1)], planks(1));
    }

    #[test]
    fn planning_a_fill_is_refused_as_the_action_is() {
        let refusal = |inventory: &Inventory, recipe: u16| {
            plan_grid_fill(inventory, recipe, 1, INVENTORY_GRID_SIZE).err()
        };
        let axe = crate::world::recipes_for(ItemId::WoodenAxe).next().unwrap();
        let inventory = inventory_with(0, planks(1));
        assert_eq!(
            refusal(&inventory, u16::MAX),
            Some(InventoryRefusal::UnknownRecipe(u16::MAX))
        );
        assert_eq!(
            refusal(&inventory, axe.index),
            Some(InventoryRefusal::DoesNotFit)
        );
        let mut full = Inventory::new();
        for slot in 0..MAX_INVENTORY_SLOTS {
            full.inner.insert(slot, stack(ItemId::Dirt, 64));
        }
        full.inner.insert(grid_slot(0, 0), planks(2));
        assert_eq!(
            refusal(&full, oak_sticks().index),
            Some(InventoryRefusal::GridNotEmptied)
        );
        // Short of one round: no plan, and the cells one round leaves without
        let fill = plan_grid_fill(&inventory, oak_sticks().index, 1, INVENTORY_GRID_SIZE).unwrap();
        assert_eq!(fill.plan, None);
        assert_eq!(fill.lacking, [false, true]);
    }

    #[test]
    fn the_fill_applies_what_was_planned() {
        let chest = crate::world::recipes_for(ItemId::Chest).next().unwrap();
        let pickaxe = crate::world::recipes_for(ItemId::WoodenPickaxe)
            .next()
            .unwrap();
        let mut with_grid = inventory_with(0, planks(1));
        with_grid.inner.insert(grid_slot(0, 1), planks(1));
        let mut mixed = inventory_with(3, stack(ItemId::OakPlanks, 5));
        mixed.inner.insert(4, stack(ItemId::SprucePlanks, 4));
        mixed.open_table = Some(TABLE);
        let mut pickaxe_kit = inventory_with(9, planks(7));
        pickaxe_kit.inner.insert(10, stack(ItemId::Stick, 5));
        let mut at_table = pickaxe_kit.clone();
        at_table.open_table = Some(TABLE);
        let cases = [
            (with_grid, oak_sticks(), 1),
            (inventory_with(0, planks(1)), oak_sticks(), 1),
            (mixed.clone(), chest, 1),
            (mixed, chest, u32::MAX),
            (pickaxe_kit, pickaxe, 1),
            (at_table, pickaxe, u32::MAX),
            (Inventory::new(), chest, 1),
        ];
        for (inventory, recipe, times) in cases {
            let size = if inventory.open_table.is_some() {
                CRAFTING_GRID_SIZE
            } else {
                INVENTORY_GRID_SIZE
            };
            let planned = plan_grid_fill(&inventory, recipe.index, times, size);
            let mut filled = inventory.clone();
            let applied =
                apply_inventory_action(&mut filled, fill(recipe, times), |_, _| true, None, false);
            let plan = planned.ok().and_then(|fill| fill.plan);
            assert_eq!(applied.is_ok(), plan.is_some(), "{recipe:?} {times}");
            let Some(plan) = plan else {
                assert_eq!(filled, inventory);
                continue;
            };
            for (index, cell) in plan.cells.iter().enumerate() {
                let slot = grid_slot((index / recipe.width) as u32, (index % recipe.width) as u32);
                let in_grid = filled
                    .inner
                    .get(&slot)
                    .map(|stack| (stack.item_id, stack.nb));
                assert_eq!(in_grid, cell.map(|item| (item, plan.rounds)), "{index}");
            }
        }
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

    fn bits(slots: &[u32]) -> u128 {
        slots.iter().fold(0, |bits, slot| bits | 1 << slot)
    }

    fn drag(slots: &[u32], one_each: bool) -> InventoryAction {
        InventoryAction::Drag {
            slots: bits(slots),
            one_each,
        }
    }

    fn try_apply(
        inventory: &mut Inventory,
        action: InventoryAction,
    ) -> Result<Option<ItemStack>, InventoryRefusal> {
        apply_inventory_action(inventory, action, |_, _| false, None, false)
    }

    #[test]
    fn a_left_drag_splits_the_held_stack_evenly_and_keeps_the_rest() {
        let mut inventory = Inventory::new();
        inventory.floating = Some(stack(ItemId::Dirt, 10));

        apply(&mut inventory, drag(&[0, 1, 2], false));

        for slot in 0..3 {
            assert_eq!(inventory.inner[&slot], stack(ItemId::Dirt, 3));
        }
        assert_eq!(inventory.floating, Some(stack(ItemId::Dirt, 1)));
    }

    #[test]
    fn a_drag_clamps_each_share_to_the_room_of_its_slot() {
        let mut inventory = inventory_with(0, stack(ItemId::Dirt, 62));
        inventory.floating = Some(stack(ItemId::Dirt, 20));

        apply(&mut inventory, drag(&[0, 1], false));

        assert_eq!(inventory.inner[&0], stack(ItemId::Dirt, 64));
        assert_eq!(inventory.inner[&1], stack(ItemId::Dirt, 10));
        assert_eq!(inventory.floating, Some(stack(ItemId::Dirt, 8)));
    }

    #[test]
    fn a_drag_over_more_slots_than_items_puts_one_in_each_of_the_first_slots() {
        let mut inventory = Inventory::new();
        inventory.floating = Some(stack(ItemId::Dirt, 3));

        apply(&mut inventory, drag(&[9, 5, 1, 7, 2], false));

        assert_eq!(inventory.inner.len(), 3);
        for slot in [1, 2, 5] {
            assert_eq!(inventory.inner[&slot], stack(ItemId::Dirt, 1));
        }
        assert_eq!(inventory.floating, None);
    }

    #[test]
    fn a_drag_skips_the_slots_it_cannot_fill() {
        let mut inventory = inventory_with(0, stack(ItemId::Sand, 1));
        inventory.inner.insert(1, stack(ItemId::Dirt, max_dirt()));
        inventory.inner.insert(4, stack(ItemId::Dirt, 1));
        inventory.floating = Some(stack(ItemId::Dirt, 8));
        let before = inventory.inner.clone();

        // Another item, a full stack, a cell beyond the 2x2 grid, a chest that is not open
        let skipped = [0, 1, grid_slot(2, 2), CHEST_SLOT_START];
        apply(
            &mut inventory,
            drag(&[skipped.as_slice(), &[3, 4]].concat(), false),
        );

        for slot in [0, 1] {
            assert_eq!(inventory.inner[&slot], before[&slot]);
        }
        assert_eq!(inventory.inner[&3], stack(ItemId::Dirt, 4));
        assert_eq!(inventory.inner[&4], stack(ItemId::Dirt, 5));
        assert_eq!(inventory.inner.len(), 4);
        assert_eq!(inventory.floating, None);
    }

    #[test]
    fn a_drag_puts_only_a_fuel_in_the_fuel_slot_and_nothing_in_the_output() {
        let (mut player, mut world) = by_an_open_furnace(Furnace::default());
        let furnace_slots = [FURNACE_INPUT_SLOT, FURNACE_FUEL_SLOT, FURNACE_OUTPUT_SLOT];

        player.inventory.floating = Some(stack(ItemId::Sand, 4));
        act(
            &mut player,
            &mut world,
            vec![drag(&[furnace_slots.as_slice(), &[9]].concat(), false)],
        );
        assert_eq!(
            world.furnace(FURNACE),
            Some(&furnace_holding(Some(stack(ItemId::Sand, 2)), None, None))
        );
        assert_eq!(player.inventory.inner[&9], stack(ItemId::Sand, 2));
        assert_eq!(player.inventory.floating, None);

        player.inventory.floating = Some(stack(ItemId::Coal, 4));
        act(
            &mut player,
            &mut world,
            vec![drag(&[FURNACE_FUEL_SLOT, FURNACE_OUTPUT_SLOT, 10], false)],
        );
        assert_eq!(
            world.furnace(FURNACE).unwrap().fuel,
            Some(stack(ItemId::Coal, 2))
        );
        assert_eq!(world.furnace(FURNACE).unwrap().output, None);
        assert_eq!(player.inventory.inner[&10], stack(ItemId::Coal, 2));
    }

    #[test]
    fn a_right_drag_puts_one_item_per_slot_until_the_cursor_is_empty() {
        let mut inventory = Inventory::new();
        inventory.floating = Some(stack(ItemId::Dirt, 3));
        apply(&mut inventory, drag(&[0, 1, 2, 3, 4], true));
        assert_eq!(inventory.inner.len(), 3);
        for slot in 0..3 {
            assert_eq!(inventory.inner[&slot], stack(ItemId::Dirt, 1));
        }
        assert_eq!(inventory.floating, None);

        let mut inventory = inventory_with(0, stack(ItemId::Dirt, 1));
        inventory.floating = Some(stack(ItemId::Dirt, 5));
        apply(&mut inventory, drag(&[0, 1], true));
        assert_eq!(inventory.inner[&0], stack(ItemId::Dirt, 2));
        assert_eq!(inventory.inner[&1], stack(ItemId::Dirt, 1));
        assert_eq!(inventory.floating, Some(stack(ItemId::Dirt, 3)));
    }

    #[test]
    fn a_drag_without_a_held_stack_is_refused() {
        let mut inventory = inventory_with(0, stack(ItemId::Dirt, 5));
        let before = inventory.clone();
        assert_eq!(
            try_apply(&mut inventory, drag(&[0, 1], false)),
            Err(InventoryRefusal::NothingHeld)
        );
        assert_eq!(inventory, before);
    }

    #[test]
    fn a_drag_with_no_slot_to_fill_is_refused_and_changes_nothing() {
        let mut inventory = inventory_with(0, stack(ItemId::Sand, 1));
        inventory.inner.insert(1, stack(ItemId::Dirt, max_dirt()));
        inventory.floating = Some(stack(ItemId::Dirt, 5));
        let before = inventory.clone();
        for slots in [&[0, 1][..], &[]] {
            for one_each in [false, true] {
                assert_eq!(
                    try_apply(&mut inventory, drag(slots, one_each)),
                    Err(InventoryRefusal::NoRoom),
                    "{slots:?}"
                );
                assert_eq!(inventory, before);
            }
        }
    }

    #[test]
    fn the_drag_plan_is_what_the_drag_does_with_a_chest_open() {
        let (mut player, mut world) = by_an_open_chest(holding(0, stack(ItemId::Dirt, 60)));
        player.inventory.floating = Some(stack(ItemId::Dirt, 10));
        let slots = bits(&[chest_slot(0), chest_slot(1), 9]);

        let mut contents = world.container(CHEST).unwrap().clone();
        let plan = drag_plan(
            &player.inventory,
            slots,
            false,
            |pos, id| block_in_reach(&world, NEAR, pos, id),
            Some(OpenContents::Chest(&mut contents)),
        );
        // The share of 3 fills the chest's first slot up to 63
        let expected = DragPlan {
            slots: vec![
                (9, stack(ItemId::Dirt, 3)),
                (chest_slot(0), stack(ItemId::Dirt, 63)),
                (chest_slot(1), stack(ItemId::Dirt, 3)),
            ],
            held: Some(stack(ItemId::Dirt, 1)),
        };
        assert_eq!(plan, Ok(expected));
        // A plan changes nothing
        assert_eq!(world.container(CHEST), Some(&contents));

        act(
            &mut player,
            &mut world,
            vec![InventoryAction::Drag {
                slots,
                one_each: false,
            }],
        );
        let mut expected = holding(0, stack(ItemId::Dirt, 63));
        expected.slots[1] = Some(stack(ItemId::Dirt, 3));
        assert_eq!(world.container(CHEST), Some(&expected));
        assert_eq!(player.inventory.inner[&9], stack(ItemId::Dirt, 3));
        assert_eq!(player.inventory.floating, Some(stack(ItemId::Dirt, 1)));
    }

    #[test]
    fn a_dragged_tool_keeps_its_wear() {
        let worn = ItemStack {
            item_type: ItemType::Tool { wear: 7 },
            ..stack(ItemId::WoodenAxe, 1)
        };
        let mut inventory = Inventory::new();
        inventory.floating = Some(worn);

        apply(&mut inventory, drag(&[4, 3], false));

        assert_eq!(inventory.inner[&3], worn);
        assert_eq!(inventory.inner.len(), 1);
        assert_eq!(inventory.floating, None);
    }

    /// The items of `item` in the slots of the inventory, `0..36`, by slot.
    fn held_in_inventory(inventory: &Inventory, item: ItemId) -> Vec<(u32, u32)> {
        let mut held: Vec<_> = inventory
            .inner
            .iter()
            .filter(|(slot, stack)| **slot < MAX_INVENTORY_SLOTS && stack.item_id == item)
            .map(|(slot, stack)| (*slot, stack.nb))
            .collect();
        held.sort();
        held
    }

    #[test]
    fn craft_all_crafts_a_stack_of_logs_into_the_inventory() {
        let mut inventory = inventory_with(grid_slot(0, 0), stack(ItemId::OakLog, 64));

        apply(&mut inventory, InventoryAction::CraftAll);

        assert_eq!(
            held_in_inventory(&inventory, ItemId::OakPlanks),
            [(0, 64), (1, 64), (2, 64), (3, 64)]
        );
        assert_eq!(inventory.inner.len(), 4);
        assert_eq!(inventory.floating, None);
    }

    #[test]
    fn craft_all_stops_when_the_inventory_has_no_room_for_a_round() {
        let full = |inventory: &mut Inventory| {
            for slot in 0..MAX_INVENTORY_SLOTS {
                inventory.inner.insert(slot, stack(ItemId::Stone, 64));
            }
        };
        // Room for two rounds, in a stack of planks topped up
        let mut inventory = inventory_with(grid_slot(0, 0), stack(ItemId::OakLog, 10));
        full(&mut inventory);
        inventory.inner.insert(5, planks(56));
        apply(&mut inventory, InventoryAction::CraftAll);
        assert_eq!(inventory.inner[&5], planks(64));
        assert_eq!(inventory.inner[&grid_slot(0, 0)], stack(ItemId::OakLog, 8));

        // Room for less than a round: nothing is crafted
        let mut inventory = inventory_with(grid_slot(0, 0), stack(ItemId::OakLog, 10));
        full(&mut inventory);
        inventory.inner.insert(5, planks(62));
        let before = inventory.clone();
        assert_eq!(
            try_apply(&mut inventory, InventoryAction::CraftAll),
            Err(InventoryRefusal::NoRoom)
        );
        assert_eq!(inventory, before);
    }

    #[test]
    fn craft_all_stops_when_the_grid_makes_another_item() {
        // A crafting table, then two planks one above the other: sticks
        let mut inventory = Inventory::new();
        for (row, col, nb) in [(0, 0, 2), (0, 1, 1), (1, 0, 2), (1, 1, 1)] {
            inventory.inner.insert(grid_slot(row, col), planks(nb));
        }

        apply(&mut inventory, InventoryAction::CraftAll);

        assert_eq!(
            held_in_inventory(&inventory, ItemId::CraftingTable),
            [(0, 1)]
        );
        assert_eq!(inventory.inner[&grid_slot(0, 0)], planks(1));
        assert_eq!(inventory.inner[&grid_slot(1, 0)], planks(1));
        assert_eq!(inventory.inner.len(), 3);
    }

    #[test]
    fn craft_all_stops_after_64_rounds() {
        // More than a stack, which no click can put there
        let mut inventory = inventory_with(grid_slot(0, 0), stack(ItemId::OakLog, 100));

        apply(&mut inventory, InventoryAction::CraftAll);

        assert_eq!(MAX_CRAFT_ALL_ROUNDS, 64);
        let made: u32 = held_in_inventory(&inventory, ItemId::OakPlanks)
            .iter()
            .map(|(_, nb)| nb)
            .sum();
        assert_eq!(made, 4 * 64);
        assert_eq!(inventory.inner[&grid_slot(0, 0)], stack(ItemId::OakLog, 36));
    }

    #[test]
    fn craft_all_leaves_the_floating_stack_alone() {
        for held in [stack(ItemId::Dirt, 5), planks(3)] {
            let mut inventory = inventory_with(grid_slot(0, 0), stack(ItemId::OakLog, 2));
            inventory.floating = Some(held);

            apply(&mut inventory, InventoryAction::CraftAll);

            assert_eq!(inventory.floating, Some(held));
            assert_eq!(held_in_inventory(&inventory, ItemId::OakPlanks), [(0, 8)]);
        }
    }

    #[test]
    fn craft_all_without_a_recipe_is_refused() {
        let mut inventory = inventory_with(grid_slot(0, 0), stack(ItemId::Dirt, 3));
        let before = inventory.clone();
        assert_eq!(
            try_apply(&mut inventory, InventoryAction::CraftAll),
            Err(InventoryRefusal::NoRecipe)
        );
        assert_eq!(inventory, before);
    }

    #[test]
    fn a_number_key_over_the_result_crafts_once_into_that_hotbar_slot() {
        let mut inventory = inventory_with(grid_slot(0, 0), stack(ItemId::OakLog, 2));
        inventory.floating = Some(stack(ItemId::Dirt, 1));

        apply(&mut inventory, InventoryAction::CraftToHotbar(3));
        assert_eq!(inventory.inner[&3], planks(4));
        assert_eq!(inventory.inner[&grid_slot(0, 0)], stack(ItemId::OakLog, 1));
        assert_eq!(inventory.floating, Some(stack(ItemId::Dirt, 1)));

        // Onto the same item, up to a full stack
        inventory.inner.insert(3, planks(60));
        apply(&mut inventory, InventoryAction::CraftToHotbar(3));
        assert_eq!(inventory.inner[&3], planks(64));
        assert!(!inventory.inner.contains_key(&grid_slot(0, 0)));
    }

    #[test]
    fn a_number_key_over_the_result_is_refused_without_a_recipe_or_room() {
        let logs = || inventory_with(grid_slot(0, 0), stack(ItemId::OakLog, 2));
        let mut another_item = logs();
        another_item.inner.insert(3, stack(ItemId::Dirt, 1));
        let mut no_room = logs();
        no_room.inner.insert(3, planks(61));
        for (mut inventory, hotbar, refusal) in [
            (
                logs(),
                HOTBAR_SLOTS,
                InventoryRefusal::InvalidSlot(HOTBAR_SLOTS),
            ),
            (
                inventory_with(grid_slot(0, 0), stack(ItemId::Dirt, 1)),
                3,
                InventoryRefusal::NoRecipe,
            ),
            (another_item, 3, InventoryRefusal::NoRoom),
            (no_room, 3, InventoryRefusal::NoRoom),
        ] {
            let before = inventory.clone();
            assert_eq!(
                try_apply(&mut inventory, InventoryAction::CraftToHotbar(hotbar)),
                Err(refusal)
            );
            assert_eq!(inventory, before);
        }
    }

    #[test]
    fn collect_all_takes_the_smallest_stacks_first_until_a_full_stack() {
        let mut inventory = Inventory::new();
        for (slot, item, nb) in [
            (12, ItemId::OakPlanks, 40),
            (25, ItemId::OakPlanks, 30),
            (20, ItemId::OakPlanks, 30),
            (3, ItemId::Dirt, 5),
        ] {
            inventory.inner.insert(slot, stack(item, nb));
        }
        inventory.floating = Some(planks(10));

        apply(&mut inventory, InventoryAction::CollectAll);

        assert_eq!(inventory.floating, Some(planks(64)));
        // Of the two smallest, the first slot first
        assert!(!inventory.inner.contains_key(&20));
        assert_eq!(inventory.inner[&25], planks(6));
        assert_eq!(inventory.inner[&12], planks(40));
        assert_eq!(inventory.inner[&3], stack(ItemId::Dirt, 5));
    }

    #[test]
    fn collect_all_takes_from_an_open_chest_too() {
        let (mut player, mut world) = by_an_open_chest(holding(0, stack(ItemId::Dirt, 3)));
        player.inventory.inner.insert(9, stack(ItemId::Dirt, 5));
        player.inventory.floating = Some(stack(ItemId::Dirt, 1));

        act(&mut player, &mut world, vec![InventoryAction::CollectAll]);

        assert_eq!(player.inventory.floating, Some(stack(ItemId::Dirt, 9)));
        assert!(world.container(CHEST).unwrap().is_empty());
        assert!(player.inventory.inner.is_empty());
    }

    #[test]
    fn collect_all_never_takes_from_the_grid_or_a_furnace() {
        let mut inventory = inventory_with(grid_slot(0, 0), stack(ItemId::Dirt, 3));
        inventory.floating = Some(stack(ItemId::Dirt, 1));
        let before = inventory.clone();
        apply(&mut inventory, InventoryAction::CollectAll);
        assert_eq!(inventory, before);

        let furnace = furnace_holding(
            Some(stack(ItemId::Sand, 3)),
            None,
            Some(stack(ItemId::Glass, 2)),
        );
        let (mut player, mut world) = by_an_open_furnace(furnace.clone());
        for held in [ItemId::Sand, ItemId::Glass] {
            player.inventory.floating = Some(stack(held, 1));
            act(&mut player, &mut world, vec![InventoryAction::CollectAll]);
            assert_eq!(player.inventory.floating, Some(stack(held, 1)));
            assert_eq!(world.furnace(FURNACE), Some(&furnace));
        }
    }

    #[test]
    fn collect_all_needs_a_held_stack_with_room() {
        let mut inventory = inventory_with(0, stack(ItemId::Dirt, 5));
        for (held, refusal) in [
            (None, InventoryRefusal::NothingHeld),
            (
                Some(stack(ItemId::Dirt, max_dirt())),
                InventoryRefusal::CursorFull,
            ),
        ] {
            inventory.floating = held;
            let before = inventory.clone();
            assert_eq!(
                try_apply(&mut inventory, InventoryAction::CollectAll),
                Err(refusal)
            );
            assert_eq!(inventory, before);
        }
    }

    fn move_all(slot: u32, item: ItemId) -> InventoryAction {
        InventoryAction::MoveAllOfType { slot, item }
    }

    #[test]
    fn move_all_of_type_moves_every_stack_of_the_item_in_the_hotbar_or_the_backpack() {
        let mut inventory = Inventory::new();
        for (slot, item, nb) in [
            (0, ItemId::Dirt, 5),
            (2, ItemId::Dirt, 7),
            (4, ItemId::Sand, 1),
            (12, ItemId::Dirt, 3),
        ] {
            inventory.inner.insert(slot, stack(item, nb));
        }
        apply(&mut inventory, move_all(0, ItemId::Dirt));
        assert_eq!(inventory.inner[&12], stack(ItemId::Dirt, 15));
        assert_eq!(inventory.inner[&4], stack(ItemId::Sand, 1));
        assert_eq!(inventory.inner.len(), 2);

        // From the backpack, to the hotbar
        inventory.inner.insert(20, stack(ItemId::Dirt, 6));
        inventory.inner.insert(1, stack(ItemId::Dirt, 1));
        apply(&mut inventory, move_all(12, ItemId::Dirt));
        assert_eq!(inventory.inner[&1], stack(ItemId::Dirt, 22));
        assert_eq!(inventory.inner[&4], stack(ItemId::Sand, 1));
        assert_eq!(inventory.inner.len(), 2);
    }

    #[test]
    fn move_all_of_type_moves_between_the_whole_inventory_and_the_chest() {
        let (mut player, mut world) = by_an_open_chest(holding(5, stack(ItemId::Sand, 1)));
        player.inventory.inner.insert(0, stack(ItemId::Dirt, 5));
        player.inventory.inner.insert(20, stack(ItemId::Dirt, 6));

        act(&mut player, &mut world, vec![move_all(0, ItemId::Dirt)]);
        let mut expected = holding(5, stack(ItemId::Sand, 1));
        expected.slots[0] = Some(stack(ItemId::Dirt, 11));
        assert_eq!(world.container(CHEST), Some(&expected));
        assert!(player.inventory.inner.is_empty());

        act(
            &mut player,
            &mut world,
            vec![move_all(chest_slot(5), ItemId::Dirt)],
        );
        assert_eq!(
            world.container(CHEST),
            Some(&holding(5, stack(ItemId::Sand, 1)))
        );
        assert_eq!(player.inventory.inner[&0], stack(ItemId::Dirt, 11));
    }

    #[test]
    fn move_all_of_type_empties_the_grid_or_the_furnace_of_the_item() {
        let mut inventory = inventory_with(grid_slot(0, 0), stack(ItemId::Dirt, 2));
        inventory
            .inner
            .insert(grid_slot(1, 1), stack(ItemId::Dirt, 3));
        inventory
            .inner
            .insert(grid_slot(0, 1), stack(ItemId::Sand, 1));
        apply(&mut inventory, move_all(grid_slot(0, 0), ItemId::Dirt));
        assert_eq!(inventory.inner[&0], stack(ItemId::Dirt, 5));
        assert_eq!(inventory.inner[&grid_slot(0, 1)], stack(ItemId::Sand, 1));
        assert_eq!(inventory.inner.len(), 2);

        let (mut player, mut world) = by_an_open_furnace(furnace_holding(
            Some(stack(ItemId::Sand, 3)),
            None,
            Some(stack(ItemId::Glass, 2)),
        ));
        act(
            &mut player,
            &mut world,
            vec![move_all(FURNACE_OUTPUT_SLOT, ItemId::Glass)],
        );
        assert_eq!(
            world.furnace(FURNACE),
            Some(&furnace_holding(Some(stack(ItemId::Sand, 3)), None, None))
        );
        assert_eq!(player.inventory.inner[&0], stack(ItemId::Glass, 2));
    }

    #[test]
    fn move_all_of_type_on_a_slot_out_of_reach_is_refused() {
        let mut inventory = inventory_with(0, stack(ItemId::Dirt, 5));
        let before = inventory.clone();
        for slot in [1000, CHEST_SLOT_START, grid_slot(2, 2)] {
            assert_eq!(
                try_apply(&mut inventory, move_all(slot, ItemId::Dirt)),
                Err(InventoryRefusal::InvalidSlot(slot))
            );
        }
        assert_eq!(inventory, before);
    }

    fn drop_slot(slot: u32, all: bool) -> InventoryAction {
        InventoryAction::DropSlot { slot, all }
    }

    /// Applies `action`, which must be accepted, and returns what it threw.
    fn throw(inventory: &mut Inventory, action: InventoryAction) -> Option<ItemStack> {
        apply_inventory_action(inventory, action, |_, _| false, None, false).unwrap()
    }

    /// Applies `action` as the server does, which must be accepted, and returns what it threw.
    fn throw_by(
        player: &mut Player,
        world: &mut ChunkMap,
        action: InventoryAction,
    ) -> Option<ItemStack> {
        let input = PlayerFrameInput {
            inventory_actions: vec![action],
            ..Default::default()
        };
        let outcomes = simulate_inventory_actions(player, world, &input, CallerType::Server);
        assert_eq!(outcomes[0].result, Ok(()), "{action:?}");
        outcomes[0].thrown
    }

    #[test]
    fn dropping_from_a_slot_throws_one_item_or_the_stack() {
        let mut inventory = inventory_with(12, stack(ItemId::Dirt, 5));

        let thrown = throw(&mut inventory, drop_slot(12, false));
        assert_eq!(thrown, Some(stack(ItemId::Dirt, 1)));
        assert_eq!(inventory.inner[&12], stack(ItemId::Dirt, 4));

        let thrown = throw(&mut inventory, drop_slot(12, true));
        assert_eq!(thrown, Some(stack(ItemId::Dirt, 4)));
        assert!(inventory.inner.is_empty());
        assert_eq!(inventory.floating, None);
    }

    #[test]
    fn dropping_from_an_empty_slot_throws_nothing() {
        let mut inventory = Inventory::new();
        for all in [false, true] {
            assert_eq!(throw(&mut inventory, drop_slot(3, all)), None);
        }
        assert_eq!(inventory, Inventory::new());
    }

    #[test]
    fn dropping_from_a_chest_a_furnace_output_or_a_leftover_beyond_the_grid() {
        let (mut player, mut world) = by_an_open_chest(holding(4, stack(ItemId::Stone, 12)));
        let thrown = throw_by(&mut player, &mut world, drop_slot(chest_slot(4), true));
        assert_eq!(thrown, Some(stack(ItemId::Stone, 12)));
        assert_eq!(world.container(CHEST).unwrap().slots[4], None);

        let glass = Some(stack(ItemId::Glass, 5));
        let (mut player, mut world) = by_an_open_furnace(furnace_holding(None, None, glass));
        let thrown = throw_by(
            &mut player,
            &mut world,
            drop_slot(FURNACE_OUTPUT_SLOT, false),
        );
        assert_eq!(thrown, Some(stack(ItemId::Glass, 1)));
        assert_eq!(
            world.furnace(FURNACE).unwrap().output,
            Some(stack(ItemId::Glass, 4))
        );

        // Left beyond the 2x2 grid when a table was closed with a full inventory
        let leftover = grid_slot(2, 2);
        let mut inventory = inventory_with(leftover, stack(ItemId::Cobblestone, 3));
        let thrown = throw(&mut inventory, drop_slot(leftover, true));
        assert_eq!(thrown, Some(stack(ItemId::Cobblestone, 3)));
        assert!(inventory.inner.is_empty());
    }

    #[test]
    fn dropping_from_a_slot_is_refused_out_of_reach_or_with_a_held_stack() {
        let mut inventory = inventory_with(0, stack(ItemId::Dirt, 5));
        let before = inventory.clone();
        for slot in [grid_slot(2, 2), chest_slot(0), FURNACE_INPUT_SLOT, 1000] {
            assert_eq!(
                try_apply(&mut inventory, drop_slot(slot, false)),
                Err(InventoryRefusal::InvalidSlot(slot))
            );
        }
        assert_eq!(inventory, before);

        inventory.floating = Some(stack(ItemId::Stone, 1));
        let before = inventory.clone();
        for all in [false, true] {
            assert_eq!(
                try_apply(&mut inventory, drop_slot(0, all)),
                Err(InventoryRefusal::HoldingAStack)
            );
        }
        assert_eq!(inventory, before);
    }

    #[test]
    fn dropping_the_held_stack_throws_one_item_or_all() {
        let mut inventory = Inventory::new();
        inventory.floating = Some(stack(ItemId::Dirt, 5));

        let thrown = throw(&mut inventory, InventoryAction::DropFloating { all: false });
        assert_eq!(thrown, Some(stack(ItemId::Dirt, 1)));
        assert_eq!(inventory.floating, Some(stack(ItemId::Dirt, 4)));

        let thrown = throw(&mut inventory, InventoryAction::DropFloating { all: true });
        assert_eq!(thrown, Some(stack(ItemId::Dirt, 4)));
        assert_eq!(inventory.floating, None);
        assert!(inventory.inner.is_empty());
    }

    #[test]
    fn dropping_the_held_stack_without_one_is_refused() {
        let mut inventory = inventory_with(0, stack(ItemId::Dirt, 5));
        let before = inventory.clone();
        for all in [false, true] {
            assert_eq!(
                try_apply(&mut inventory, InventoryAction::DropFloating { all }),
                Err(InventoryRefusal::NothingHeld)
            );
        }
        assert_eq!(inventory, before);
    }

    #[test]
    fn a_thrown_tool_keeps_its_wear() {
        let worn = ItemStack {
            item_type: ItemType::Tool { wear: 9 },
            ..stack(ItemId::WoodenShovel, 1)
        };
        let mut inventory = inventory_with(0, worn);
        assert_eq!(throw(&mut inventory, drop_slot(0, false)), Some(worn));

        inventory.floating = Some(worn);
        let thrown = throw(&mut inventory, InventoryAction::DropFloating { all: false });
        assert_eq!(thrown, Some(worn));
    }
}
