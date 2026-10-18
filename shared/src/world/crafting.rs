//! The recipes, and what the crafting grid makes.

use crate::{
    world::{ItemId, ItemStack},
    MAX_INVENTORY_SLOTS,
};

/// The crafting grid is held in the inventory, in the slots after those of the inventory.
pub const CRAFTING_GRID_START: u32 = MAX_INVENTORY_SLOTS;
/// Width and height of the largest grid, that of a crafting table. The inventory alone only
/// uses its top left 2x2 corner.
pub const CRAFTING_GRID_SIZE: u32 = 3;

/// What the cells of the grid hold, row after row.
pub type CraftingGrid = [Option<ItemId>; (CRAFTING_GRID_SIZE * CRAFTING_GRID_SIZE) as usize];

/// The inventory slot of the cell of the grid at `row`, `col`.
pub const fn grid_slot(row: u32, col: u32) -> u32 {
    CRAFTING_GRID_START + row * CRAFTING_GRID_SIZE + col
}

/// A shaped recipe: its cells, row after row, must match the filled part of the grid exactly,
/// wherever that part sits in the grid.
struct Recipe {
    width: usize,
    cells: &'static [Option<ItemId>],
    output: ItemId,
    nb: u32,
}

const OAK_LOG: Option<ItemId> = Some(ItemId::OakLog);
const SPRUCE_LOG: Option<ItemId> = Some(ItemId::SpruceLog);
const OAK: Option<ItemId> = Some(ItemId::OakPlanks);
const SPRUCE: Option<ItemId> = Some(ItemId::SprucePlanks);
const COBBLE: Option<ItemId> = Some(ItemId::Cobblestone);

#[rustfmt::skip]
const RECIPES: &[Recipe] = &[
    Recipe { width: 1, cells: &[OAK_LOG], output: ItemId::OakPlanks, nb: 4 },
    Recipe { width: 1, cells: &[SPRUCE_LOG], output: ItemId::SprucePlanks, nb: 4 },
    Recipe { width: 1, cells: &[OAK, OAK], output: ItemId::Stick, nb: 4 },
    Recipe { width: 1, cells: &[SPRUCE, SPRUCE], output: ItemId::Stick, nb: 4 },
    Recipe { width: 2, cells: &[OAK, OAK, OAK, OAK], output: ItemId::CraftingTable, nb: 1 },
    Recipe { width: 2, cells: &[SPRUCE, SPRUCE, SPRUCE, SPRUCE], output: ItemId::CraftingTable, nb: 1 },
    Recipe {
        width: 3,
        cells: &[
            COBBLE, COBBLE, COBBLE,
            COBBLE, None, COBBLE,
            COBBLE, COBBLE, COBBLE,
        ],
        output: ItemId::Furnace,
        nb: 1,
    },
];

/// What the grid makes, if it matches a recipe.
pub fn match_recipe(grid: &CraftingGrid) -> Option<ItemStack> {
    let size = CRAFTING_GRID_SIZE as usize;
    let cell = |row: usize, col: usize| grid[row * size + col];

    // The bounding box of the filled cells
    let rows: Vec<usize> = (0..size)
        .filter(|&row| (0..size).any(|col| cell(row, col).is_some()))
        .collect();
    let cols: Vec<usize> = (0..size)
        .filter(|&col| (0..size).any(|row| cell(row, col).is_some()))
        .collect();
    let (&top, &bottom, &left, &right) = (rows.first()?, rows.last()?, cols.first()?, cols.last()?);
    let (height, width) = (bottom - top + 1, right - left + 1);

    let recipe = RECIPES.iter().find(|recipe| {
        recipe.width == width
            && recipe.cells.len() == width * height
            && (0..height).all(|row| {
                (0..width).all(|col| recipe.cells[row * width + col] == cell(top + row, left + col))
            })
    })?;
    Some(ItemStack {
        item_id: recipe.output,
        item_type: recipe.output.get_default_type(),
        nb: recipe.nb,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const LOG: Option<ItemId> = Some(ItemId::OakLog);
    const OAK: Option<ItemId> = Some(ItemId::OakPlanks);
    const SPRUCE: Option<ItemId> = Some(ItemId::SprucePlanks);
    const COBBLE: Option<ItemId> = Some(ItemId::Cobblestone);

    fn grid(cells: [Option<ItemId>; 9]) -> CraftingGrid {
        cells
    }

    fn made(item_id: ItemId, nb: u32) -> Option<ItemStack> {
        Some(ItemStack {
            item_id,
            item_type: item_id.get_default_type(),
            nb,
        })
    }

    #[test]
    fn a_log_makes_four_planks() {
        assert_eq!(
            match_recipe(&grid([LOG, None, None, None, None, None, None, None, None])),
            made(ItemId::OakPlanks, 4)
        );
        let spruce_log = Some(ItemId::SpruceLog);
        assert_eq!(
            match_recipe(&grid([
                spruce_log, None, None, None, None, None, None, None, None
            ])),
            made(ItemId::SprucePlanks, 4)
        );
    }

    #[test]
    fn two_planks_one_above_the_other_make_four_sticks() {
        for planks in [OAK, SPRUCE] {
            assert_eq!(
                match_recipe(&grid([
                    planks, None, None, planks, None, None, None, None, None
                ])),
                made(ItemId::Stick, 4)
            );
        }
    }

    #[test]
    fn four_planks_in_a_square_make_a_crafting_table() {
        for planks in [OAK, SPRUCE] {
            assert_eq!(
                match_recipe(&grid([
                    planks, planks, None, planks, planks, None, None, None, None
                ])),
                made(ItemId::CraftingTable, 1)
            );
        }
    }

    #[test]
    fn a_ring_of_cobblestone_makes_a_furnace() {
        assert_eq!(
            match_recipe(&grid([
                COBBLE, COBBLE, COBBLE, COBBLE, None, COBBLE, COBBLE, COBBLE, COBBLE
            ])),
            made(ItemId::Furnace, 1)
        );
    }

    #[test]
    fn a_recipe_matches_anywhere_in_the_grid() {
        assert_eq!(
            match_recipe(&grid([None, None, None, None, None, None, None, None, LOG])),
            made(ItemId::OakPlanks, 4)
        );
        assert_eq!(
            match_recipe(&grid([None, None, None, None, OAK, OAK, None, OAK, OAK])),
            made(ItemId::CraftingTable, 1)
        );
        assert_eq!(
            match_recipe(&grid([None, None, OAK, None, None, OAK, None, None, None])),
            made(ItemId::Stick, 4)
        );
    }

    #[test]
    fn planks_side_by_side_make_nothing() {
        assert_eq!(
            match_recipe(&grid([OAK, OAK, None, None, None, None, None, None, None])),
            None
        );
    }

    #[test]
    fn mixed_planks_make_nothing() {
        assert_eq!(
            match_recipe(&grid([
                OAK, None, None, SPRUCE, None, None, None, None, None
            ])),
            None
        );
    }

    #[test]
    fn an_extra_item_makes_nothing() {
        assert_eq!(
            match_recipe(&grid([
                LOG, None, None, None, None, None, None, None, COBBLE
            ])),
            None
        );
        assert_eq!(
            match_recipe(&grid([
                COBBLE, COBBLE, COBBLE, COBBLE, COBBLE, COBBLE, COBBLE, COBBLE, COBBLE
            ])),
            None
        );
    }

    #[test]
    fn an_empty_grid_makes_nothing() {
        assert_eq!(match_recipe(&[None; 9]), None);
    }

    #[test]
    fn the_grid_starts_after_the_inventory() {
        assert_eq!(CRAFTING_GRID_START, MAX_INVENTORY_SLOTS);
        assert_eq!(grid_slot(0, 0), MAX_INVENTORY_SLOTS);
        assert_eq!(grid_slot(2, 1), MAX_INVENTORY_SLOTS + 7);
    }
}
