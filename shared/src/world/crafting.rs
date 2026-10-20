//! The recipes, and what the crafting grid makes.

use crate::{
    players::Inventory,
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
/// wherever that part sits in the grid, or match it mirrored left to right.
struct Recipe {
    width: usize,
    cells: &'static [Option<ItemId>],
    output: ItemId,
    nb: u32,
}

const OAK_LOG: Option<ItemId> = Some(ItemId::OakLog);
const SPRUCE_LOG: Option<ItemId> = Some(ItemId::SpruceLog);
const JUNGLE_LOG: Option<ItemId> = Some(ItemId::JungleLog);
const OAK: Option<ItemId> = Some(ItemId::OakPlanks);
const SPRUCE: Option<ItemId> = Some(ItemId::SprucePlanks);
const JUNGLE: Option<ItemId> = Some(ItemId::JunglePlanks);
const COBBLE: Option<ItemId> = Some(ItemId::Cobblestone);
const STICK: Option<ItemId> = Some(ItemId::Stick);
const COAL: Option<ItemId> = Some(ItemId::Coal);

#[rustfmt::skip]
const RECIPES: &[Recipe] = &[
    Recipe { width: 1, cells: &[OAK_LOG], output: ItemId::OakPlanks, nb: 4 },
    Recipe { width: 1, cells: &[SPRUCE_LOG], output: ItemId::SprucePlanks, nb: 4 },
    Recipe { width: 1, cells: &[JUNGLE_LOG], output: ItemId::JunglePlanks, nb: 4 },
    Recipe { width: 1, cells: &[OAK, OAK], output: ItemId::Stick, nb: 4 },
    Recipe { width: 1, cells: &[SPRUCE, SPRUCE], output: ItemId::Stick, nb: 4 },
    Recipe { width: 1, cells: &[JUNGLE, JUNGLE], output: ItemId::Stick, nb: 4 },
    Recipe { width: 1, cells: &[COAL, STICK], output: ItemId::Torch, nb: 4 },
    Recipe { width: 2, cells: &[OAK, OAK, OAK, OAK], output: ItemId::CraftingTable, nb: 1 },
    Recipe { width: 2, cells: &[SPRUCE, SPRUCE, SPRUCE, SPRUCE], output: ItemId::CraftingTable, nb: 1 },
    Recipe { width: 2, cells: &[JUNGLE, JUNGLE, JUNGLE, JUNGLE], output: ItemId::CraftingTable, nb: 1 },
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
    Recipe {
        width: 3,
        cells: &[
            OAK, OAK, OAK,
            OAK, None, OAK,
            OAK, OAK, OAK,
        ],
        output: ItemId::Chest,
        nb: 1,
    },
    Recipe {
        width: 3,
        cells: &[
            SPRUCE, SPRUCE, SPRUCE,
            SPRUCE, None, SPRUCE,
            SPRUCE, SPRUCE, SPRUCE,
        ],
        output: ItemId::Chest,
        nb: 1,
    },
    Recipe {
        width: 3,
        cells: &[
            JUNGLE, JUNGLE, JUNGLE,
            JUNGLE, None, JUNGLE,
            JUNGLE, JUNGLE, JUNGLE,
        ],
        output: ItemId::Chest,
        nb: 1,
    },
    Recipe {
        width: 3,
        cells: &[
            OAK, OAK, OAK,
            None, STICK, None,
            None, STICK, None,
        ],
        output: ItemId::WoodenPickaxe,
        nb: 1,
    },
    Recipe {
        width: 3,
        cells: &[
            SPRUCE, SPRUCE, SPRUCE,
            None, STICK, None,
            None, STICK, None,
        ],
        output: ItemId::WoodenPickaxe,
        nb: 1,
    },
    Recipe {
        width: 3,
        cells: &[
            JUNGLE, JUNGLE, JUNGLE,
            None, STICK, None,
            None, STICK, None,
        ],
        output: ItemId::WoodenPickaxe,
        nb: 1,
    },
    Recipe {
        width: 2,
        cells: &[
            OAK, OAK,
            OAK, STICK,
            None, STICK,
        ],
        output: ItemId::WoodenAxe,
        nb: 1,
    },
    Recipe {
        width: 2,
        cells: &[
            SPRUCE, SPRUCE,
            SPRUCE, STICK,
            None, STICK,
        ],
        output: ItemId::WoodenAxe,
        nb: 1,
    },
    Recipe {
        width: 2,
        cells: &[
            JUNGLE, JUNGLE,
            JUNGLE, STICK,
            None, STICK,
        ],
        output: ItemId::WoodenAxe,
        nb: 1,
    },
    Recipe {
        width: 1,
        cells: &[
            OAK,
            STICK,
            STICK,
        ],
        output: ItemId::WoodenShovel,
        nb: 1,
    },
    Recipe {
        width: 1,
        cells: &[
            SPRUCE,
            STICK,
            STICK,
        ],
        output: ItemId::WoodenShovel,
        nb: 1,
    },
    Recipe {
        width: 1,
        cells: &[
            JUNGLE,
            STICK,
            STICK,
        ],
        output: ItemId::WoodenShovel,
        nb: 1,
    },
    Recipe {
        width: 2,
        cells: &[
            OAK, OAK,
            None, STICK,
            None, STICK,
        ],
        output: ItemId::WoodenHoe,
        nb: 1,
    },
    Recipe {
        width: 2,
        cells: &[
            SPRUCE, SPRUCE,
            None, STICK,
            None, STICK,
        ],
        output: ItemId::WoodenHoe,
        nb: 1,
    },
    Recipe {
        width: 2,
        cells: &[
            JUNGLE, JUNGLE,
            None, STICK,
            None, STICK,
        ],
        output: ItemId::WoodenHoe,
        nb: 1,
    },
    Recipe {
        width: 1,
        cells: &[
            OAK,
            OAK,
            STICK,
        ],
        output: ItemId::WoodenSword,
        nb: 1,
    },
    Recipe {
        width: 1,
        cells: &[
            SPRUCE,
            SPRUCE,
            STICK,
        ],
        output: ItemId::WoodenSword,
        nb: 1,
    },
    Recipe {
        width: 1,
        cells: &[
            JUNGLE,
            JUNGLE,
            STICK,
        ],
        output: ItemId::WoodenSword,
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

    let matches = |recipe: &Recipe, mirrored: bool| {
        (0..height).all(|row| {
            (0..width).all(|col| {
                let recipe_col = if mirrored { width - 1 - col } else { col };
                recipe.cells[row * width + recipe_col] == cell(top + row, left + col)
            })
        })
    };
    let recipe = RECIPES.iter().find(|recipe| {
        recipe.width == width
            && recipe.cells.len() == width * height
            && (matches(recipe, false) || matches(recipe, true))
    })?;
    Some(ItemStack {
        item_id: recipe.output,
        item_type: recipe.output.get_default_type(),
        nb: recipe.nb,
    })
}

/// A recipe as the item browser reads it. Its index is its position in the table, stable for a
/// protocol version.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RecipeView {
    pub index: u16,
    pub width: usize,
    pub height: usize,
    /// Its cells, row after row.
    pub cells: &'static [Option<ItemId>],
    pub output: ItemStack,
}

impl RecipeView {
    /// Whether the recipe fits a crafting grid of `grid_size` by `grid_size`.
    pub fn fits(&self, grid_size: u32) -> bool {
        self.width <= grid_size as usize && self.height <= grid_size as usize
    }

    /// The items of the recipe, one per cell, in the order of their first cell.
    pub fn ingredients(&self) -> Vec<(ItemId, u32)> {
        let mut ingredients: Vec<(ItemId, u32)> = Vec::new();
        for item in self.cells.iter().flatten() {
            match ingredients.iter_mut().find(|(other, _)| other == item) {
                Some((_, count)) => *count += 1,
                None => ingredients.push((*item, 1)),
            }
        }
        ingredients
    }
}

/// Every recipe, in the order of the table.
pub fn recipes() -> impl Iterator<Item = RecipeView> {
    RECIPES
        .iter()
        .enumerate()
        .map(|(index, recipe)| RecipeView {
            index: index as u16,
            width: recipe.width,
            height: recipe.cells.len() / recipe.width,
            cells: recipe.cells,
            output: ItemStack {
                item_id: recipe.output,
                item_type: recipe.output.get_default_type(),
                nb: recipe.nb,
            },
        })
}

/// The recipes making `item`.
pub fn recipes_for(item: ItemId) -> impl Iterator<Item = RecipeView> {
    recipes().filter(move |recipe| recipe.output.item_id == item)
}

/// The recipes `item` goes into.
pub fn uses_of(item: ItemId) -> impl Iterator<Item = RecipeView> {
    recipes().filter(move |recipe| recipe.cells.contains(&Some(item)))
}

/// What `inventory` lacks to make `recipe` once, in the order of its ingredients. Its slots and
/// its crafting grid count; a chest, and the stack held by the cursor, do not.
pub fn missing(inventory: &Inventory, recipe: &RecipeView) -> Vec<(ItemId, u32)> {
    let grid = CRAFTING_GRID_START..CRAFTING_GRID_START + CRAFTING_GRID_SIZE * CRAFTING_GRID_SIZE;
    let held = |item: ItemId| -> u32 {
        inventory
            .inner
            .iter()
            .filter(|(slot, stack)| {
                (**slot < MAX_INVENTORY_SLOTS || grid.contains(slot)) && stack.item_id == item
            })
            .map(|(_, stack)| stack.nb)
            .sum()
    };
    recipe
        .ingredients()
        .into_iter()
        .filter_map(|(item, needed)| {
            let lacking = needed.saturating_sub(held(item));
            (lacking > 0).then_some((item, lacking))
        })
        .collect()
}

/// Whether `inventory` holds the ingredients of `recipe`, counted as `missing` does.
pub fn craftable(inventory: &Inventory, recipe: &RecipeView) -> bool {
    missing(inventory, recipe).is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::players::Inventory;

    const LOG: Option<ItemId> = Some(ItemId::OakLog);
    const OAK: Option<ItemId> = Some(ItemId::OakPlanks);
    const SPRUCE: Option<ItemId> = Some(ItemId::SprucePlanks);
    const JUNGLE: Option<ItemId> = Some(ItemId::JunglePlanks);
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
        let jungle_log = Some(ItemId::JungleLog);
        assert_eq!(
            match_recipe(&grid([
                jungle_log, None, None, None, None, None, None, None, None
            ])),
            made(ItemId::JunglePlanks, 4)
        );
    }

    #[test]
    fn two_planks_one_above_the_other_make_four_sticks() {
        for planks in [OAK, SPRUCE, JUNGLE] {
            assert_eq!(
                match_recipe(&grid([
                    planks, None, None, planks, None, None, None, None, None
                ])),
                made(ItemId::Stick, 4)
            );
        }
    }

    #[test]
    fn coal_on_a_stick_makes_four_torches() {
        let coal = Some(ItemId::Coal);
        let stick = Some(ItemId::Stick);
        assert_eq!(
            match_recipe(&grid([
                coal, None, None, stick, None, None, None, None, None
            ])),
            made(ItemId::Torch, 4)
        );
        assert_eq!(
            match_recipe(&grid([
                stick, None, None, coal, None, None, None, None, None
            ])),
            None
        );
    }

    #[test]
    fn four_planks_in_a_square_make_a_crafting_table() {
        for planks in [OAK, SPRUCE, JUNGLE] {
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
    fn a_ring_of_planks_makes_a_chest() {
        for planks in [OAK, SPRUCE, JUNGLE] {
            assert_eq!(
                match_recipe(&grid([
                    planks, planks, planks, planks, None, planks, planks, planks, planks
                ])),
                made(ItemId::Chest, 1)
            );
        }
        assert_eq!(
            match_recipe(&grid([OAK, OAK, OAK, OAK, None, OAK, OAK, OAK, SPRUCE])),
            None
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

    const STICK: Option<ItemId> = Some(ItemId::Stick);

    #[test]
    fn planks_and_sticks_make_the_wooden_tools() {
        for p in [OAK, SPRUCE, JUNGLE] {
            for (cells, tool) in [
                (
                    [p, p, p, None, STICK, None, None, STICK, None],
                    ItemId::WoodenPickaxe,
                ),
                (
                    [p, p, None, p, STICK, None, None, STICK, None],
                    ItemId::WoodenAxe,
                ),
                (
                    [p, None, None, STICK, None, None, STICK, None, None],
                    ItemId::WoodenShovel,
                ),
                (
                    [p, p, None, None, STICK, None, None, STICK, None],
                    ItemId::WoodenHoe,
                ),
                (
                    [p, None, None, p, None, None, STICK, None, None],
                    ItemId::WoodenSword,
                ),
            ] {
                assert_eq!(match_recipe(&grid(cells)), made(tool, 1), "{tool:?}");
            }
        }
    }

    #[test]
    fn a_recipe_also_matches_mirrored_left_to_right() {
        assert_eq!(
            match_recipe(&grid([None, OAK, OAK, None, STICK, OAK, None, STICK, None])),
            made(ItemId::WoodenAxe, 1)
        );
        assert_eq!(
            match_recipe(&grid([
                OAK, OAK, None, STICK, None, None, STICK, None, None
            ])),
            made(ItemId::WoodenHoe, 1)
        );
        // but not upside down
        assert_eq!(
            match_recipe(&grid([None, STICK, None, None, STICK, None, OAK, OAK, OAK])),
            None
        );
    }

    #[test]
    fn every_recipe_is_listed_with_its_index_and_size() {
        let all: Vec<RecipeView> = recipes().collect();
        assert_eq!(all.len(), 29);
        for (position, recipe) in all.iter().enumerate() {
            assert_eq!(recipe.index as usize, position);
            assert_eq!(recipe.cells.len(), recipe.width * recipe.height);
            // what the grid makes when it holds the recipe in its top left corner
            let mut grid = [None; 9];
            for row in 0..recipe.height {
                for col in 0..recipe.width {
                    grid[row * 3 + col] = recipe.cells[row * recipe.width + col];
                }
            }
            assert_eq!(match_recipe(&grid), Some(recipe.output), "{recipe:?}");
        }
        let furnace = all.iter().find(|r| r.output.item_id == ItemId::Furnace);
        assert_eq!(furnace.map(|r| (r.width, r.height)), Some((3, 3)));
        assert_eq!((all[3].width, all[3].height), (1, 2));
        assert_eq!(all[3].output, made(ItemId::Stick, 4).unwrap());
    }

    #[test]
    fn recipes_for_and_uses_of_find_the_recipes_of_an_item() {
        let counts = |item| (recipes_for(item).count(), uses_of(item).count());
        assert_eq!(counts(ItemId::WoodenPickaxe), (3, 0));
        assert_eq!(counts(ItemId::Stick), (3, 16));
        assert_eq!(counts(ItemId::OakPlanks), (1, 8));
        assert_eq!(counts(ItemId::Cobblestone), (0, 1));
        assert_eq!(counts(ItemId::Bedrock), (0, 0));
        assert!(recipes_for(ItemId::Chest).all(|r| r.output.item_id == ItemId::Chest));
        assert!(uses_of(ItemId::Stick).all(|r| r.cells.contains(&STICK)));
    }

    /// The first recipe making `item`.
    fn recipe_of(item: ItemId) -> RecipeView {
        recipes_for(item).next().unwrap()
    }

    #[test]
    fn a_recipe_fits_a_grid_as_large_as_it() {
        assert!(recipe_of(ItemId::Stick).fits(2));
        assert!(recipe_of(ItemId::WoodenPickaxe).fits(3));
        assert!(!recipe_of(ItemId::WoodenPickaxe).fits(2));
        assert!(recipe_of(ItemId::WoodenAxe).fits(3));
        assert!(!recipe_of(ItemId::WoodenAxe).fits(2));
    }

    #[test]
    fn the_ingredients_of_a_recipe_are_counted_by_cell() {
        assert_eq!(
            recipe_of(ItemId::WoodenPickaxe).ingredients(),
            [(ItemId::OakPlanks, 3), (ItemId::Stick, 2)]
        );
        assert_eq!(
            recipe_of(ItemId::Furnace).ingredients(),
            [(ItemId::Cobblestone, 8)]
        );
    }

    fn put(inventory: &mut Inventory, slot: u32, item_id: ItemId, nb: u32) {
        inventory.inner.insert(slot, made(item_id, nb).unwrap());
    }

    #[test]
    fn missing_counts_the_inventory_and_the_grid() {
        let pickaxe = recipe_of(ItemId::WoodenPickaxe);
        let mut inventory = Inventory::new();
        assert_eq!(
            missing(&inventory, &pickaxe),
            [(ItemId::OakPlanks, 3), (ItemId::Stick, 2)]
        );
        assert!(!craftable(&inventory, &pickaxe));
        put(&mut inventory, 0, ItemId::OakPlanks, 1);
        put(&mut inventory, grid_slot(1, 1), ItemId::OakPlanks, 2);
        put(&mut inventory, 20, ItemId::Stick, 1);
        assert_eq!(missing(&inventory, &pickaxe), [(ItemId::Stick, 1)]);
        assert!(!craftable(&inventory, &pickaxe));
        put(&mut inventory, MAX_INVENTORY_SLOTS - 1, ItemId::Stick, 1);
        assert!(missing(&inventory, &pickaxe).is_empty());
        assert!(craftable(&inventory, &pickaxe));
    }

    #[test]
    fn the_chest_and_the_floating_stack_do_not_count() {
        let pickaxe = recipe_of(ItemId::WoodenPickaxe);
        let mut inventory = Inventory::new();
        let chest = grid_slot(2, 2) + 1;
        put(&mut inventory, chest, ItemId::OakPlanks, 64);
        put(&mut inventory, chest + 1, ItemId::Stick, 64);
        inventory.floating = made(ItemId::Stick, 64);
        assert_eq!(
            missing(&inventory, &pickaxe),
            [(ItemId::OakPlanks, 3), (ItemId::Stick, 2)]
        );
    }

    #[test]
    fn a_tool_needs_planks_of_one_kind() {
        assert_eq!(
            match_recipe(&grid([
                OAK, OAK, SPRUCE, None, STICK, None, None, STICK, None
            ])),
            None
        );
        assert_eq!(
            match_recipe(&grid([
                JUNGLE, JUNGLE, OAK, None, STICK, None, None, STICK, None
            ])),
            None
        );
    }
}
