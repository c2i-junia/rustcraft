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

/// A family of items any of which a recipe takes in a cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemTag {
    Planks,
}

impl ItemTag {
    /// The items of the family, in the order of `ItemId::ALL`. A new species of wood joins
    /// `Planks` by being listed here.
    pub fn members(self) -> &'static [ItemId] {
        match self {
            Self::Planks => &[
                ItemId::OakPlanks,
                ItemId::SprucePlanks,
                ItemId::JunglePlanks,
            ],
        }
    }

    /// The name of the family, as the UI writes it.
    pub fn name(self) -> &'static str {
        match self {
            Self::Planks => "Planks",
        }
    }
}

/// What a cell of a recipe takes: an item, or any item of a family.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ingredient {
    Item(ItemId),
    Tag(ItemTag),
}

impl Ingredient {
    /// Whether a cell of this ingredient takes `item`.
    pub fn matches(self, item: ItemId) -> bool {
        self.members().contains(&item)
    }

    /// The items a cell of this ingredient takes: the item alone, or the family's.
    pub fn members(&self) -> &[ItemId] {
        match self {
            Self::Item(item) => std::slice::from_ref(item),
            Self::Tag(tag) => tag.members(),
        }
    }
}

/// A shaped recipe: its cells, row after row, must match the filled part of the grid exactly,
/// wherever that part sits in the grid, or match it mirrored left to right.
struct Recipe {
    width: usize,
    cells: &'static [Option<Ingredient>],
    output: ItemId,
    nb: u32,
}

/// A cell of a recipe taking `item`.
const fn item(item: ItemId) -> Option<Ingredient> {
    Some(Ingredient::Item(item))
}

const OAK_LOG: Option<Ingredient> = item(ItemId::OakLog);
const SPRUCE_LOG: Option<Ingredient> = item(ItemId::SpruceLog);
const JUNGLE_LOG: Option<Ingredient> = item(ItemId::JungleLog);
const PLANKS: Option<Ingredient> = Some(Ingredient::Tag(ItemTag::Planks));
const COBBLE: Option<Ingredient> = item(ItemId::Cobblestone);
const STICK: Option<Ingredient> = item(ItemId::Stick);
const COAL: Option<Ingredient> = item(ItemId::Coal);
const CHARCOAL: Option<Ingredient> = item(ItemId::Charcoal);
const IRON: Option<Ingredient> = item(ItemId::IronIngot);
const GOLD: Option<Ingredient> = item(ItemId::GoldIngot);
const DIAMOND: Option<Ingredient> = item(ItemId::Diamond);

/// The tools' shapes, Minecraft's, made of `m` and sticks.
const fn pickaxe(m: Option<Ingredient>) -> [Option<Ingredient>; 9] {
    [m, m, m, None, STICK, None, None, STICK, None]
}
const fn axe(m: Option<Ingredient>) -> [Option<Ingredient>; 6] {
    [m, m, m, STICK, None, STICK]
}
const fn shovel(m: Option<Ingredient>) -> [Option<Ingredient>; 3] {
    [m, STICK, STICK]
}
const fn hoe(m: Option<Ingredient>) -> [Option<Ingredient>; 6] {
    [m, m, None, STICK, None, STICK]
}
const fn sword(m: Option<Ingredient>) -> [Option<Ingredient>; 3] {
    [m, m, STICK]
}

#[rustfmt::skip]
const RECIPES: &[Recipe] = &[
    Recipe { width: 1, cells: &[OAK_LOG], output: ItemId::OakPlanks, nb: 4 },
    Recipe { width: 1, cells: &[SPRUCE_LOG], output: ItemId::SprucePlanks, nb: 4 },
    Recipe { width: 1, cells: &[JUNGLE_LOG], output: ItemId::JunglePlanks, nb: 4 },
    Recipe { width: 1, cells: &[PLANKS, PLANKS], output: ItemId::Stick, nb: 4 },
    Recipe { width: 1, cells: &[COAL, STICK], output: ItemId::Torch, nb: 4 },
    Recipe { width: 2, cells: &[PLANKS; 4], output: ItemId::CraftingTable, nb: 1 },
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
            PLANKS, PLANKS, PLANKS,
            PLANKS, None, PLANKS,
            PLANKS, PLANKS, PLANKS,
        ],
        output: ItemId::Chest,
        nb: 1,
    },
    Recipe { width: 3, cells: &pickaxe(PLANKS), output: ItemId::WoodenPickaxe, nb: 1 },
    Recipe { width: 2, cells: &axe(PLANKS), output: ItemId::WoodenAxe, nb: 1 },
    Recipe { width: 1, cells: &shovel(PLANKS), output: ItemId::WoodenShovel, nb: 1 },
    Recipe { width: 2, cells: &hoe(PLANKS), output: ItemId::WoodenHoe, nb: 1 },
    Recipe { width: 1, cells: &sword(PLANKS), output: ItemId::WoodenSword, nb: 1 },
    Recipe { width: 3, cells: &pickaxe(COBBLE), output: ItemId::StonePickaxe, nb: 1 },
    Recipe { width: 2, cells: &axe(COBBLE), output: ItemId::StoneAxe, nb: 1 },
    Recipe { width: 1, cells: &shovel(COBBLE), output: ItemId::StoneShovel, nb: 1 },
    Recipe { width: 2, cells: &hoe(COBBLE), output: ItemId::StoneHoe, nb: 1 },
    Recipe { width: 1, cells: &sword(COBBLE), output: ItemId::StoneSword, nb: 1 },
    Recipe { width: 3, cells: &pickaxe(IRON), output: ItemId::IronPickaxe, nb: 1 },
    Recipe { width: 2, cells: &axe(IRON), output: ItemId::IronAxe, nb: 1 },
    Recipe { width: 1, cells: &shovel(IRON), output: ItemId::IronShovel, nb: 1 },
    Recipe { width: 2, cells: &hoe(IRON), output: ItemId::IronHoe, nb: 1 },
    Recipe { width: 1, cells: &sword(IRON), output: ItemId::IronSword, nb: 1 },
    Recipe { width: 3, cells: &pickaxe(GOLD), output: ItemId::GoldenPickaxe, nb: 1 },
    Recipe { width: 2, cells: &axe(GOLD), output: ItemId::GoldenAxe, nb: 1 },
    Recipe { width: 1, cells: &shovel(GOLD), output: ItemId::GoldenShovel, nb: 1 },
    Recipe { width: 2, cells: &hoe(GOLD), output: ItemId::GoldenHoe, nb: 1 },
    Recipe { width: 1, cells: &sword(GOLD), output: ItemId::GoldenSword, nb: 1 },
    Recipe { width: 3, cells: &pickaxe(DIAMOND), output: ItemId::DiamondPickaxe, nb: 1 },
    Recipe { width: 2, cells: &axe(DIAMOND), output: ItemId::DiamondAxe, nb: 1 },
    Recipe { width: 1, cells: &shovel(DIAMOND), output: ItemId::DiamondShovel, nb: 1 },
    Recipe { width: 2, cells: &hoe(DIAMOND), output: ItemId::DiamondHoe, nb: 1 },
    Recipe { width: 1, cells: &sword(DIAMOND), output: ItemId::DiamondSword, nb: 1 },
    Recipe { width: 3, cells: &[IRON; 9], output: ItemId::IronBlock, nb: 1 },
    Recipe { width: 3, cells: &[GOLD; 9], output: ItemId::GoldBlock, nb: 1 },
    Recipe { width: 3, cells: &[DIAMOND; 9], output: ItemId::DiamondBlock, nb: 1 },
    Recipe { width: 1, cells: &[item(ItemId::IronBlock)], output: ItemId::IronIngot, nb: 9 },
    Recipe { width: 1, cells: &[item(ItemId::GoldBlock)], output: ItemId::GoldIngot, nb: 9 },
    Recipe { width: 1, cells: &[item(ItemId::DiamondBlock)], output: ItemId::Diamond, nb: 9 },
    Recipe { width: 1, cells: &[CHARCOAL, STICK], output: ItemId::Torch, nb: 4 },
    Recipe { width: 3, cells: &[IRON, None, IRON, None, IRON, None], output: ItemId::Bucket, nb: 1 },
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
                match (
                    recipe.cells[row * width + recipe_col],
                    cell(top + row, left + col),
                ) {
                    (Some(ingredient), Some(item)) => ingredient.matches(item),
                    (None, None) => true,
                    _ => false,
                }
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
    pub cells: &'static [Option<Ingredient>],
    pub output: ItemStack,
}

impl RecipeView {
    /// Whether the recipe fits a crafting grid of `grid_size` by `grid_size`.
    pub fn fits(&self, grid_size: u32) -> bool {
        self.width <= grid_size as usize && self.height <= grid_size as usize
    }

    /// The ingredients of the recipe, one per cell, in the order of their first cell.
    pub fn ingredients(&self) -> Vec<(Ingredient, u32)> {
        let mut ingredients: Vec<(Ingredient, u32)> = Vec::new();
        for ingredient in self.cells.iter().flatten() {
            match ingredients
                .iter_mut()
                .find(|(other, _)| other == ingredient)
            {
                Some((_, count)) => *count += 1,
                None => ingredients.push((*ingredient, 1)),
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

/// The recipes `item` goes into: those with a cell taking it.
pub fn uses_of(item: ItemId) -> impl Iterator<Item = RecipeView> {
    recipes().filter(move |recipe| {
        recipe
            .cells
            .iter()
            .flatten()
            .any(|ingredient| ingredient.matches(item))
    })
}

/// What `inventory` lacks to make `recipe` once, in the order of its ingredients, the items of
/// a family counted together. Its slots and its crafting grid count; a chest, and the stack
/// held by the cursor, do not.
pub fn missing(inventory: &Inventory, recipe: &RecipeView) -> Vec<(Ingredient, u32)> {
    let grid = CRAFTING_GRID_START..CRAFTING_GRID_START + CRAFTING_GRID_SIZE * CRAFTING_GRID_SIZE;
    let held = |ingredient: Ingredient| -> u32 {
        inventory
            .inner
            .iter()
            .filter(|(slot, stack)| {
                (**slot < MAX_INVENTORY_SLOTS || grid.contains(slot))
                    && ingredient.matches(stack.item_id)
            })
            .map(|(_, stack)| stack.nb)
            .sum()
    };
    recipe
        .ingredients()
        .into_iter()
        .filter_map(|(ingredient, needed)| {
            let lacking = needed.saturating_sub(held(ingredient));
            (lacking > 0).then_some((ingredient, lacking))
        })
        .collect()
}

/// Whether `inventory` holds the ingredients of `recipe`, counted as `missing` does.
pub fn craftable(inventory: &Inventory, recipe: &RecipeView) -> bool {
    missing(inventory, recipe).is_empty()
}

/// What filling the grid with a recipe puts into it: `rounds` items of one item in each cell.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FillPlan {
    /// How many items each cell gets.
    pub rounds: u32,
    /// The item each cell of the recipe gets, row after row, `None` for its empty cells.
    pub cells: Vec<Option<ItemId>>,
}

/// The most rounds a fill makes: a stack in each cell.
const MAX_FILL_ROUNDS: u32 = 64;

/// How `inventory` fills the grid with `recipe`, `times` rounds at most, from its slots alone
/// (not the grid, which the fill puts back first, nor a chest or the cursor): the most rounds,
/// up to a stack, for which every cell can get that many of one item. A cell of a family takes
/// the item the previous cell of that family took while it holds enough, else the one the
/// inventory holds the most of after the cells already served, the first of the family on a
/// tie: one species is used whenever it is enough. `None` without one round.
pub fn plan_fill(inventory: &Inventory, recipe: &RecipeView, times: u32) -> Option<FillPlan> {
    let held = held_items(inventory);
    (1..=times.min(MAX_FILL_ROUNDS)).rev().find_map(|rounds| {
        let cells = serve_cells(held.clone(), recipe, rounds)
            .into_iter()
            .map(|cell| match cell {
                Cell::Empty => Some(None),
                Cell::Served(item) => Some(Some(item)),
                Cell::Lacking => None,
            })
            .collect::<Option<Vec<_>>>()?;
        Some(FillPlan { rounds, cells })
    })
}

/// The cells of `recipe` that one round leaves without their ingredient, row after row, served
/// from the slots of `inventory` as `plan_fill` serves them: `true` for a cell the inventory has
/// nothing left for once the cells before it are served.
pub fn lacking_cells(inventory: &Inventory, recipe: &RecipeView) -> Vec<bool> {
    serve_cells(held_items(inventory), recipe, 1)
        .into_iter()
        .map(|cell| cell == Cell::Lacking)
        .collect()
}

/// How many of each item the slots `0..36` of `inventory` hold, in the order first met.
fn held_items(inventory: &Inventory) -> Vec<(ItemId, u32)> {
    let mut held: Vec<(ItemId, u32)> = Vec::new();
    for stack in (0..MAX_INVENTORY_SLOTS).filter_map(|slot| inventory.inner.get(&slot)) {
        match held.iter_mut().find(|(item, _)| *item == stack.item_id) {
            Some((_, nb)) => *nb += stack.nb,
            None => held.push((stack.item_id, stack.nb)),
        }
    }
    held
}

/// What a cell of a recipe gets in a fill.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Cell {
    /// The recipe leaves it empty.
    Empty,
    /// It gets the rounds of this item.
    Served(ItemId),
    /// Nothing left serves it.
    Lacking,
}

/// How the items `left` serve each cell of `recipe` with `rounds` items of one item, in the
/// order of the cells; a cell they cannot serve takes nothing.
fn serve_cells(mut left: Vec<(ItemId, u32)>, recipe: &RecipeView, rounds: u32) -> Vec<Cell> {
    let count = |left: &[(ItemId, u32)], item: ItemId| {
        left.iter()
            .find(|(other, _)| *other == item)
            .map_or(0, |(_, nb)| *nb)
    };
    // The item the last cell of each ingredient took
    let mut last: Vec<(Ingredient, ItemId)> = Vec::new();
    let mut cells = Vec::with_capacity(recipe.cells.len());
    for cell in recipe.cells {
        let Some(ingredient) = *cell else {
            cells.push(Cell::Empty);
            continue;
        };
        let previous = last
            .iter()
            .find(|(other, _)| *other == ingredient)
            .map(|(_, item)| *item)
            .filter(|&item| count(&left, item) >= rounds);
        // the first of the most held
        let most = ingredient.members().iter().copied().reduce(|best, item| {
            if count(&left, item) > count(&left, best) {
                item
            } else {
                best
            }
        });
        let Some(item) = previous.or(most) else {
            cells.push(Cell::Lacking);
            continue;
        };
        if count(&left, item) < rounds || item.get_max_stack() < rounds {
            cells.push(Cell::Lacking);
            continue;
        }
        if let Some((_, nb)) = left.iter_mut().find(|(other, _)| *other == item) {
            *nb -= rounds;
        }
        match last.iter_mut().find(|(other, _)| *other == ingredient) {
            Some((_, taken)) => *taken = item,
            None => last.push((ingredient, item)),
        }
        cells.push(Cell::Served(item));
    }
    cells
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
            match_recipe(&grid([OAK, OAK, OAK, JUNGLE, None, OAK, OAK, OAK, SPRUCE])),
            made(ItemId::Chest, 1)
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
    fn planks_of_any_kind_one_above_the_other_make_four_sticks() {
        assert_eq!(
            match_recipe(&grid([
                OAK, None, None, SPRUCE, None, None, None, None, None
            ])),
            made(ItemId::Stick, 4)
        );
        assert_eq!(
            match_recipe(&grid([
                None, None, None, None, JUNGLE, None, None, OAK, None
            ])),
            made(ItemId::Stick, 4)
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
    fn cobblestone_ingots_and_diamonds_make_the_tools_of_their_tier() {
        for (m, [pickaxe, axe, shovel, hoe, sword]) in [
            (
                COBBLE,
                [
                    ItemId::StonePickaxe,
                    ItemId::StoneAxe,
                    ItemId::StoneShovel,
                    ItemId::StoneHoe,
                    ItemId::StoneSword,
                ],
            ),
            (
                Some(ItemId::IronIngot),
                [
                    ItemId::IronPickaxe,
                    ItemId::IronAxe,
                    ItemId::IronShovel,
                    ItemId::IronHoe,
                    ItemId::IronSword,
                ],
            ),
            (
                Some(ItemId::GoldIngot),
                [
                    ItemId::GoldenPickaxe,
                    ItemId::GoldenAxe,
                    ItemId::GoldenShovel,
                    ItemId::GoldenHoe,
                    ItemId::GoldenSword,
                ],
            ),
            (
                Some(ItemId::Diamond),
                [
                    ItemId::DiamondPickaxe,
                    ItemId::DiamondAxe,
                    ItemId::DiamondShovel,
                    ItemId::DiamondHoe,
                    ItemId::DiamondSword,
                ],
            ),
        ] {
            for (cells, tool) in [
                ([m, m, m, None, STICK, None, None, STICK, None], pickaxe),
                ([m, m, None, m, STICK, None, None, STICK, None], axe),
                (
                    [m, None, None, STICK, None, None, STICK, None, None],
                    shovel,
                ),
                ([m, m, None, None, STICK, None, None, STICK, None], hoe),
                ([m, None, None, m, None, None, STICK, None, None], sword),
            ] {
                assert_eq!(match_recipe(&grid(cells)), made(tool, 1), "{tool:?}");
            }
        }
    }

    #[test]
    fn nine_ingots_or_diamonds_make_a_block_which_gives_them_back() {
        for (material, block) in [
            (ItemId::IronIngot, ItemId::IronBlock),
            (ItemId::GoldIngot, ItemId::GoldBlock),
            (ItemId::Diamond, ItemId::DiamondBlock),
        ] {
            assert_eq!(match_recipe(&grid([Some(material); 9])), made(block, 1));
            let alone = [Some(block), None, None, None, None, None, None, None, None];
            assert_eq!(match_recipe(&grid(alone)), made(material, 9));
            // eight are not enough
            let mut eight = [Some(material); 9];
            eight[4] = None;
            assert_eq!(match_recipe(&grid(eight)), None, "{material:?}");
        }
    }

    #[test]
    fn charcoal_on_a_stick_makes_four_torches_as_coal() {
        let charcoal = Some(ItemId::Charcoal);
        assert_eq!(
            match_recipe(&grid([
                charcoal, None, None, STICK, None, None, None, None, None
            ])),
            made(ItemId::Torch, 4)
        );
    }

    #[test]
    fn three_iron_ingots_in_a_v_make_a_bucket() {
        let iron = Some(ItemId::IronIngot);
        for v in [
            [iron, None, iron, None, iron, None, None, None, None],
            [None, None, None, iron, None, iron, None, iron, None],
        ] {
            assert_eq!(match_recipe(&grid(v)), made(ItemId::Bucket, 1));
        }
        // upside down, it is nothing
        assert_eq!(
            match_recipe(&grid([
                None, iron, None, iron, None, iron, None, None, None
            ])),
            None
        );
        // three wide, it needs a crafting table
        assert!(!recipe_of(ItemId::Bucket).fits(2));
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
        assert_eq!(all.len(), 41);
        for (position, recipe) in all.iter().enumerate() {
            assert_eq!(recipe.index as usize, position);
            assert_eq!(recipe.cells.len(), recipe.width * recipe.height);
            // what the grid makes when it holds the recipe in its top left corner, each cell
            // holding the first item of its ingredient
            let grid = grid_of(recipe, false, |ingredient| ingredient.members()[0]);
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
        assert_eq!(counts(ItemId::WoodenPickaxe), (1, 0));
        assert_eq!(counts(ItemId::Stick), (1, 27));
        assert_eq!(counts(ItemId::OakPlanks), (1, 8));
        assert_eq!(counts(ItemId::SprucePlanks), (1, 8));
        assert_eq!(counts(ItemId::JunglePlanks), (1, 8));
        // the same recipes for each species
        let uses = |item| uses_of(item).map(|r| r.index).collect::<Vec<_>>();
        assert_eq!(uses(ItemId::OakPlanks), uses(ItemId::SprucePlanks));
        assert_eq!(uses(ItemId::OakPlanks), uses(ItemId::JunglePlanks));
        assert_eq!(counts(ItemId::Cobblestone), (0, 6));
        assert_eq!(counts(ItemId::IronIngot), (1, 7));
        assert_eq!(counts(ItemId::Torch), (2, 0));
        assert_eq!(counts(ItemId::Bedrock), (0, 0));
        assert!(recipes_for(ItemId::Chest).all(|r| r.output.item_id == ItemId::Chest));
        assert!(uses_of(ItemId::Stick)
            .all(|r| r.cells.contains(&Some(Ingredient::Item(ItemId::Stick)))));
        assert!(uses_of(ItemId::SprucePlanks).all(|r| r.cells.contains(&Some(PLANKS))));
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
            [(PLANKS, 3), (Ingredient::Item(ItemId::Stick), 2)]
        );
        assert_eq!(
            recipe_of(ItemId::Furnace).ingredients(),
            [(Ingredient::Item(ItemId::Cobblestone), 8)]
        );
    }

    fn put(inventory: &mut Inventory, slot: u32, item_id: ItemId, nb: u32) {
        inventory.inner.insert(slot, made(item_id, nb).unwrap());
    }

    #[test]
    fn missing_counts_the_inventory_and_the_grid() {
        let pickaxe = recipe_of(ItemId::WoodenPickaxe);
        let mut inventory = Inventory::new();
        let stick = Ingredient::Item(ItemId::Stick);
        assert_eq!(missing(&inventory, &pickaxe), [(PLANKS, 3), (stick, 2)]);
        assert!(!craftable(&inventory, &pickaxe));
        put(&mut inventory, 0, ItemId::OakPlanks, 1);
        put(&mut inventory, grid_slot(1, 1), ItemId::OakPlanks, 2);
        put(&mut inventory, 20, ItemId::Stick, 1);
        assert_eq!(missing(&inventory, &pickaxe), [(stick, 1)]);
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
            [(PLANKS, 3), (Ingredient::Item(ItemId::Stick), 2)]
        );
    }

    #[test]
    fn a_tool_takes_planks_of_any_kind() {
        assert_eq!(
            match_recipe(&grid([
                OAK, OAK, SPRUCE, None, STICK, None, None, STICK, None
            ])),
            made(ItemId::WoodenPickaxe, 1)
        );
        assert_eq!(
            match_recipe(&grid([
                JUNGLE, JUNGLE, OAK, None, STICK, None, None, STICK, None
            ])),
            made(ItemId::WoodenPickaxe, 1)
        );
        // mirrored, too
        assert_eq!(
            match_recipe(&grid([
                None, SPRUCE, JUNGLE, None, STICK, OAK, None, STICK, None
            ])),
            made(ItemId::WoodenAxe, 1)
        );
    }

    const PLANKS: Ingredient = Ingredient::Tag(ItemTag::Planks);

    /// The grid holding `recipe` in its top left corner, each cell holding the item `pick`
    /// chooses for its ingredient, mirrored left to right if `mirrored`.
    fn grid_of(
        recipe: &RecipeView,
        mirrored: bool,
        pick: impl Fn(Ingredient) -> ItemId,
    ) -> CraftingGrid {
        let mut grid = [None; 9];
        for row in 0..recipe.height {
            for col in 0..recipe.width {
                let recipe_col = if mirrored {
                    recipe.width - 1 - col
                } else {
                    col
                };
                grid[row * 3 + col] = recipe.cells[row * recipe.width + recipe_col].map(&pick);
            }
        }
        grid
    }

    #[test]
    fn every_recipe_with_planks_is_made_by_each_species() {
        let with_planks: Vec<RecipeView> = recipes()
            .filter(|recipe| recipe.cells.contains(&Some(PLANKS)))
            .collect();
        let outputs: Vec<ItemId> = with_planks.iter().map(|r| r.output.item_id).collect();
        assert_eq!(
            outputs,
            [
                ItemId::Stick,
                ItemId::CraftingTable,
                ItemId::Chest,
                ItemId::WoodenPickaxe,
                ItemId::WoodenAxe,
                ItemId::WoodenShovel,
                ItemId::WoodenHoe,
                ItemId::WoodenSword,
            ]
        );
        for recipe in &with_planks {
            for species in [
                ItemId::OakPlanks,
                ItemId::SprucePlanks,
                ItemId::JunglePlanks,
            ] {
                let pick = |ingredient: Ingredient| match ingredient {
                    Ingredient::Item(item) => item,
                    Ingredient::Tag(_) => species,
                };
                for mirrored in [false, true] {
                    assert_eq!(
                        match_recipe(&grid_of(recipe, mirrored, pick)),
                        Some(recipe.output),
                        "{species:?} {recipe:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn the_planks_tag_holds_every_planks_item() {
        let planks: Vec<ItemId> = ItemId::ALL
            .into_iter()
            .filter(|item| item.name().ends_with("_planks"))
            .collect();
        assert_eq!(ItemTag::Planks.members(), planks);
        assert_eq!(
            planks,
            [
                ItemId::OakPlanks,
                ItemId::SprucePlanks,
                ItemId::JunglePlanks
            ]
        );
        assert_eq!(ItemTag::Planks.name(), "Planks");
    }

    #[test]
    fn an_ingredient_matches_its_items() {
        for planks in ItemTag::Planks.members() {
            assert!(PLANKS.matches(*planks));
        }
        assert!(!PLANKS.matches(ItemId::OakLog));
        assert!(!PLANKS.matches(ItemId::Stick));
        let stick = Ingredient::Item(ItemId::Stick);
        assert!(stick.matches(ItemId::Stick));
        assert!(!stick.matches(ItemId::OakPlanks));
        assert_eq!(stick.members(), [ItemId::Stick]);
        assert_eq!(PLANKS.members(), ItemTag::Planks.members());
    }

    #[test]
    fn no_recipe_has_two_ingredients_sharing_an_item() {
        for recipe in recipes() {
            let ingredients = recipe.ingredients();
            for (index, (first, _)) in ingredients.iter().enumerate() {
                for (second, _) in &ingredients[index + 1..] {
                    assert!(
                        !first.members().iter().any(|item| second.matches(*item)),
                        "{recipe:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn missing_counts_every_member_of_a_tag_together() {
        let pickaxe = recipe_of(ItemId::WoodenPickaxe);
        let stick = Ingredient::Item(ItemId::Stick);
        let mut inventory = Inventory::new();
        put(&mut inventory, 0, ItemId::OakPlanks, 1);
        put(&mut inventory, 30, ItemId::SprucePlanks, 1);
        assert_eq!(missing(&inventory, &pickaxe), [(PLANKS, 1), (stick, 2)]);
        put(&mut inventory, grid_slot(2, 2), ItemId::JunglePlanks, 1);
        assert_eq!(missing(&inventory, &pickaxe), [(stick, 2)]);
        put(&mut inventory, 1, ItemId::Stick, 2);
        assert!(craftable(&inventory, &pickaxe));
    }

    /// An inventory holding `planks` in its slots from 0, a stack each.
    fn holding(planks: &[(ItemId, u32)]) -> Inventory {
        let mut inventory = Inventory::new();
        for (slot, &(item, nb)) in planks.iter().enumerate() {
            put(&mut inventory, slot as u32, item, nb);
        }
        inventory
    }

    /// The items of the cells of the chest's ring, in the order of the recipe.
    fn ring(oak: usize, spruce: usize) -> Vec<Option<ItemId>> {
        let mut species = std::iter::repeat_n(ItemId::OakPlanks, oak)
            .chain(std::iter::repeat_n(ItemId::SprucePlanks, spruce));
        (0..9)
            .map(|cell| if cell == 4 { None } else { species.next() })
            .collect()
    }

    #[test]
    fn a_fill_takes_one_species_when_it_is_enough() {
        let chest = recipe_of(ItemId::Chest);
        let plan = |planks: &[(ItemId, u32)]| plan_fill(&holding(planks), &chest, 1);
        // the most held
        let only_oak = plan(&[(ItemId::OakPlanks, 10), (ItemId::SprucePlanks, 3)]);
        assert_eq!(
            only_oak,
            Some(FillPlan {
                rounds: 1,
                cells: ring(8, 0)
            })
        );
        let only_spruce = plan(&[(ItemId::OakPlanks, 8), (ItemId::SprucePlanks, 9)]);
        assert_eq!(
            only_spruce,
            Some(FillPlan {
                rounds: 1,
                cells: ring(0, 8)
            })
        );
        // the first of the family on a tie
        let tie = plan(&[(ItemId::SprucePlanks, 8), (ItemId::OakPlanks, 8)]);
        assert_eq!(
            tie,
            Some(FillPlan {
                rounds: 1,
                cells: ring(8, 0)
            })
        );
    }

    #[test]
    fn a_fill_mixes_species_when_none_is_enough() {
        let chest = recipe_of(ItemId::Chest);
        let inventory = holding(&[(ItemId::OakPlanks, 5), (ItemId::SprucePlanks, 4)]);
        assert_eq!(
            plan_fill(&inventory, &chest, 1),
            Some(FillPlan {
                rounds: 1,
                cells: ring(5, 3)
            })
        );
        let sticks = recipe_of(ItemId::Stick);
        let inventory = holding(&[(ItemId::JunglePlanks, 1), (ItemId::OakPlanks, 1)]);
        assert_eq!(
            plan_fill(&inventory, &sticks, 1),
            Some(FillPlan {
                rounds: 1,
                cells: vec![Some(ItemId::OakPlanks), Some(ItemId::JunglePlanks)]
            })
        );
    }

    #[test]
    fn lacking_cells_serves_one_round_cell_by_cell() {
        let pickaxe = recipe_of(ItemId::WoodenPickaxe);
        // Two planks for three cells, one stick for two: the last of each lacks
        let mut inventory = holding(&[(ItemId::OakPlanks, 2)]);
        put(&mut inventory, 5, ItemId::Stick, 1);
        assert_eq!(
            lacking_cells(&inventory, &pickaxe),
            [false, false, true, false, false, false, false, true, false]
        );
        // Any species serves a cell of planks
        let mut inventory = holding(&[
            (ItemId::OakPlanks, 1),
            (ItemId::SprucePlanks, 1),
            (ItemId::JunglePlanks, 1),
        ]);
        put(&mut inventory, 5, ItemId::Stick, 2);
        assert_eq!(lacking_cells(&inventory, &pickaxe), [false; 9]);
        assert!(plan_fill(&inventory, &pickaxe, 1).is_some());
        // Nothing serves anything; the grid does not count
        let mut inventory = Inventory::new();
        put(&mut inventory, grid_slot(0, 0), ItemId::Coal, 1);
        let torch = recipe_of(ItemId::Torch);
        assert_eq!(lacking_cells(&inventory, &torch), [true, true]);
    }

    #[test]
    fn a_fill_makes_as_many_rounds_as_every_cell_can_get_of_one_item() {
        let chest = recipe_of(ItemId::Chest);
        // 3 rounds: 6 cells of oak and 2 of spruce; 4 would need 8 cells of 4
        let inventory = holding(&[(ItemId::OakPlanks, 20), (ItemId::SprucePlanks, 10)]);
        assert_eq!(
            plan_fill(&inventory, &chest, u32::MAX),
            Some(FillPlan {
                rounds: 3,
                cells: ring(6, 2)
            })
        );
        // as many as asked
        assert_eq!(
            plan_fill(&inventory, &chest, 2).map(|plan| plan.rounds),
            Some(2)
        );
        // a stack at most
        let pickaxe = recipe_of(ItemId::WoodenPickaxe);
        let mut inventory = Inventory::new();
        for slot in 0..10 {
            put(&mut inventory, slot, ItemId::OakPlanks, 64);
        }
        for slot in 10..14 {
            put(&mut inventory, slot, ItemId::Stick, 64);
        }
        assert_eq!(
            plan_fill(&inventory, &pickaxe, u32::MAX).map(|plan| plan.rounds),
            Some(64)
        );
        // the grid and a chest do not count: the fill puts the grid back first
        let mut inventory = holding(&[(ItemId::OakPlanks, 7)]);
        put(&mut inventory, grid_slot(0, 0), ItemId::OakPlanks, 1);
        put(&mut inventory, grid_slot(2, 2) + 1, ItemId::OakPlanks, 64);
        assert_eq!(plan_fill(&inventory, &chest, 1), None);
        assert_eq!(plan_fill(&Inventory::new(), &chest, 1), None);
        assert_eq!(
            plan_fill(&holding(&[(ItemId::OakPlanks, 8)]), &chest, 0),
            None
        );
    }
}
