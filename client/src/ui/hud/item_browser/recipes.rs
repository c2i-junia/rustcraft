//! What the recipe view says: its title, its back button, its tabs, its cards, the station of a
//! recipe or why it cannot be made now, and what it says when there is no recipe to show.

use shared::players::Inventory;
use shared::world::{
    craftable, fuel_ticks, missing, recipes_for, smelting_inputs, smelting_result, uses_of, ItemId,
    RecipeView, COOK_TICKS,
};
use shared::TICKS_PER_SECOND;

use super::catalog::category_detail;
use super::{ItemView, RecipeTab};
use crate::ui::hud::item_name;

/// The line of the tooltip of an item craftable now, in Survival.
pub const CRAFTABLE_NOTE: &str = "You have the ingredients";
/// The keys of the tooltip of an item of the browser; A removes a bookmarked item from the
/// bookmarks.
pub fn item_keys_hint(bookmarked: bool) -> String {
    let bookmark = if bookmarked { "Unbookmark" } else { "Bookmark" };
    format!("R Recipes  U Uses  A {bookmark}")
}

/// The line under the name of the item of the view: its category, and whether it is bookmarked.
pub fn bookmarked_detail(item: ItemId, bookmarked: bool) -> String {
    let detail = category_detail(item);
    if bookmarked {
        format!("{detail} · bookmarked")
    } else {
        detail
    }
}

/// The title of the browser showing `tab`.
pub fn view_title(tab: RecipeTab) -> &'static str {
    match tab {
        RecipeTab::Recipes => "Recipes",
        RecipeTab::Uses => "Uses",
    }
}

/// The label of the back button: where it goes, the last item of `history` or the catalog.
pub fn back_label(history: &[ItemView]) -> String {
    match history.last() {
        Some(view) => format!("<- {}", item_name(view.item)),
        None => "<- Items".to_string(),
    }
}

/// The label of the chip of `tab`, which has `count` recipes.
pub fn tab_label(tab: RecipeTab, count: usize) -> String {
    format!("{} {count}", view_title(tab))
}

/// Where `recipe` is made: in the inventory's 2x2 grid when it fits, else on a crafting table.
/// An `x`, not a `×`: the game font has no `×`.
pub fn station_label(recipe: &RecipeView) -> &'static str {
    if recipe.fits(2) {
        "Inventory 2x2"
    } else {
        "Crafting Table 3x3"
    }
}

/// Why `inventory` cannot make `recipe` now with a grid of `grid_size`: the ingredients it lacks,
/// else the crafting table it needs, else the chest or the furnace open in place of the grid.
/// `None` when it can, and `+ Fill grid` fills the grid with it.
pub fn recipe_problem(
    inventory: &Inventory,
    recipe: &RecipeView,
    grid_size: u32,
) -> Option<String> {
    let lacking = missing(inventory, recipe);
    if !lacking.is_empty() {
        let items: Vec<String> = lacking
            .into_iter()
            .map(|(item, count)| format!("{count} {}", item_name(item)))
            .collect();
        return Some(format!("Missing {}", items.join(", ")));
    }
    if !recipe.fits(grid_size) {
        return Some("Needs a Crafting Table".to_string());
    }
    // The chest or the furnace takes the place of the crafting grid in the panel
    if inventory.open_chest.is_some() {
        return Some(NOT_BESIDE_A_CHEST.to_string());
    }
    inventory
        .open_furnace
        .is_some()
        .then(|| NOT_BESIDE_A_FURNACE.to_string())
}

/// Why a recipe cannot fill the grid while a chest is open.
pub const NOT_BESIDE_A_CHEST: &str = "Not beside a chest";
/// Why a recipe cannot fill the grid while a furnace is open.
pub const NOT_BESIDE_A_FURNACE: &str = "Not beside a furnace";

/// A card of the view: a recipe of the crafting grid, or one of the furnace.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BrowserCard {
    Crafting(RecipeView),
    /// The furnace makes `output` of `input`.
    Smelting {
        input: ItemId,
        output: ItemId,
    },
    /// `item` burns in the furnace, `ticks` long.
    Fuel {
        item: ItemId,
        ticks: u16,
    },
}

/// The cards `view` shows: the crafting recipes first, then the smeltings, then, among the uses
/// of a fuel, its burning.
pub fn cards_of(view: ItemView) -> Vec<BrowserCard> {
    let item = view.item;
    match view.tab {
        RecipeTab::Recipes => recipes_for(item)
            .map(BrowserCard::Crafting)
            .chain(smelting_inputs(item).map(|input| BrowserCard::Smelting {
                input,
                output: item,
            }))
            .collect(),
        RecipeTab::Uses => uses_of(item)
            .map(BrowserCard::Crafting)
            .chain(smelting_result(item).map(|output| BrowserCard::Smelting {
                input: item,
                output,
            }))
            .chain(fuel_ticks(item).map(|ticks| BrowserCard::Fuel { item, ticks }))
            .collect(),
    }
}

/// The accessible name of a card.
pub fn card_label(card: &BrowserCard) -> String {
    match card {
        BrowserCard::Crafting(recipe) => format!("Recipe of {}", item_name(recipe.output.item_id)),
        BrowserCard::Smelting { output, .. } => format!("Smelting of {}", item_name(*output)),
        BrowserCard::Fuel { item, .. } => format!("{} as fuel", item_name(*item)),
    }
}

/// Under a smelting card: where it is made, and how long it takes.
pub fn smelting_note() -> String {
    format!("Furnace · {} s", u64::from(COOK_TICKS) / TICKS_PER_SECOND)
}

/// Under a fuel card: how long one of it burns, `ticks`.
pub fn fuel_note(ticks: u16) -> String {
    format!("Fuel · {} s", u64::from(ticks) / TICKS_PER_SECOND)
}

/// Beside the fuel of a fuel card: how many items one of it smelts, burning `ticks` long.
pub fn burns_for(ticks: u16) -> String {
    let items = f32::from(ticks) / f32::from(COOK_TICKS);
    if items == 1.0 {
        "Burns for 1 item".to_string()
    } else {
        format!("Burns for {items} items")
    }
}

/// After the message of an item without recipe: how many recipes it goes into.
fn uses_sentence(uses: usize) -> String {
    match uses {
        0 => String::new(),
        1 => " It goes into 1 recipe: see Uses.".to_string(),
        uses => format!(" It goes into {uses} recipes: see Uses."),
    }
}

/// What the view says when it has no recipe to show.
pub fn empty_view_message(view: ItemView) -> String {
    let name = item_name(view.item);
    match view.tab {
        RecipeTab::Recipes => format!(
            "{name} is gathered, not crafted.{}",
            uses_sentence(
                cards_of(ItemView {
                    tab: RecipeTab::Uses,
                    ..view
                })
                .len()
            )
        ),
        RecipeTab::Uses => format!("No recipe uses {name} yet."),
    }
}

/// Whether `inventory` holds the ingredients of some recipe of `item`, whatever the grid size.
pub fn craftable_now(inventory: &Inventory, item: ItemId) -> bool {
    recipes_for(item).any(|recipe| craftable(inventory, &recipe))
}

#[cfg(test)]
mod tests {
    use shared::players::Inventory;
    use shared::world::{grid_slot, recipes_for, ItemId, ItemStack};

    use super::*;
    use crate::ui::hud::item_browser::{ItemView, RecipeTab};

    fn put(inventory: &mut Inventory, slot: u32, item_id: ItemId, nb: u32) {
        let stack = ItemStack {
            item_id,
            item_type: item_id.get_default_type(),
            nb,
        };
        inventory.inner.insert(slot, stack);
    }

    fn view(item: ItemId, tab: RecipeTab) -> ItemView {
        ItemView { item, tab }
    }

    #[test]
    fn the_texts_of_the_view() {
        assert_eq!(view_title(RecipeTab::Recipes), "Recipes");
        assert_eq!(view_title(RecipeTab::Uses), "Uses");
        assert_eq!(back_label(&[]), "<- Items");
        let history = [
            view(ItemId::WoodenPickaxe, RecipeTab::Recipes),
            view(ItemId::Stick, RecipeTab::Uses),
        ];
        assert_eq!(back_label(&history), "<- Stick");
        assert_eq!(tab_label(RecipeTab::Recipes, 2), "Recipes 2");
        assert_eq!(tab_label(RecipeTab::Uses, 10), "Uses 10");
        let stick = recipes_for(ItemId::Stick).next().unwrap();
        let pickaxe = recipes_for(ItemId::WoodenPickaxe).next().unwrap();
        assert_eq!(station_label(&stick), "Inventory 2x2");
        assert_eq!(station_label(&pickaxe), "Crafting Table 3x3");
        assert_eq!(CRAFTABLE_NOTE, "You have the ingredients");
        assert_eq!(item_keys_hint(false), "R Recipes  U Uses  A Bookmark");
        assert_eq!(item_keys_hint(true), "R Recipes  U Uses  A Unbookmark");
        assert_eq!(bookmarked_detail(ItemId::Chest, false), "Stations");
        assert_eq!(
            bookmarked_detail(ItemId::Chest, true),
            "Stations · bookmarked"
        );
        assert_eq!(
            bookmarked_detail(ItemId::WoodenAxe, true),
            "Tools · Tool · bookmarked"
        );
    }

    #[test]
    fn the_problem_of_a_recipe_is_what_is_missing_then_the_table_then_the_chest_or_furnace() {
        let pickaxe = recipes_for(ItemId::WoodenPickaxe).next().unwrap();
        let stick = recipes_for(ItemId::Stick).next().unwrap();
        let mut inventory = Inventory::new();
        assert_eq!(
            recipe_problem(&inventory, &pickaxe, 2).as_deref(),
            Some("Missing 3 Oak Planks, 2 Stick")
        );
        put(&mut inventory, 0, ItemId::OakPlanks, 3);
        put(&mut inventory, grid_slot(0, 0), ItemId::Stick, 2);
        assert_eq!(
            recipe_problem(&inventory, &pickaxe, 2).as_deref(),
            Some("Needs a Crafting Table")
        );
        assert_eq!(recipe_problem(&inventory, &pickaxe, 3), None);
        assert_eq!(recipe_problem(&inventory, &stick, 2), None);
        inventory.open_chest = Some(bevy::math::IVec3::ZERO);
        assert_eq!(
            recipe_problem(&inventory, &stick, 2).as_deref(),
            Some("Not beside a chest")
        );
        inventory.open_chest = None;
        inventory.open_furnace = Some(bevy::math::IVec3::ZERO);
        assert_eq!(
            recipe_problem(&inventory, &stick, 2).as_deref(),
            Some("Not beside a furnace")
        );
    }

    #[test]
    fn a_view_lists_the_crafting_then_the_smelting_then_the_burning() {
        let cards = |item, tab| cards_of(view(item, tab));
        let smelting = |input, output| BrowserCard::Smelting { input, output };
        // One smelting per log
        assert_eq!(
            cards(ItemId::Charcoal, RecipeTab::Recipes),
            [
                smelting(ItemId::OakLog, ItemId::Charcoal),
                smelting(ItemId::SpruceLog, ItemId::Charcoal),
                smelting(ItemId::JungleLog, ItemId::Charcoal),
            ]
        );
        assert_eq!(
            cards(ItemId::Glass, RecipeTab::Recipes),
            [smelting(ItemId::Sand, ItemId::Glass)]
        );
        // An iron ingot comes out of its block, then of the furnace
        let ingot = cards(ItemId::IronIngot, RecipeTab::Recipes);
        assert!(
            matches!(ingot[0], BrowserCard::Crafting(recipe) if recipe.cells == [Some(ItemId::IronBlock)])
        );
        assert_eq!(ingot[1..], [smelting(ItemId::IronOre, ItemId::IronIngot)]);
        // An oak log makes planks, smelts into charcoal, and burns
        let log = cards(ItemId::OakLog, RecipeTab::Uses);
        assert!(
            matches!(log[0], BrowserCard::Crafting(recipe) if recipe.output.item_id == ItemId::OakPlanks)
        );
        assert_eq!(
            log[1..],
            [
                smelting(ItemId::OakLog, ItemId::Charcoal),
                BrowserCard::Fuel {
                    item: ItemId::OakLog,
                    ticks: 300
                },
            ]
        );
        assert_eq!(
            cards(ItemId::Charcoal, RecipeTab::Uses)[1..],
            [BrowserCard::Fuel {
                item: ItemId::Charcoal,
                ticks: 1600
            }]
        );
        assert!(cards(ItemId::Bedrock, RecipeTab::Uses).is_empty());
        assert!(cards(ItemId::IronOre, RecipeTab::Recipes).is_empty());
    }

    #[test]
    fn the_texts_of_the_furnace_cards() {
        assert_eq!(smelting_note(), "Furnace · 10 s");
        assert_eq!(fuel_note(1600), "Fuel · 80 s");
        assert_eq!(fuel_note(100), "Fuel · 5 s");
        assert_eq!(burns_for(1600), "Burns for 8 items");
        assert_eq!(burns_for(300), "Burns for 1.5 items");
        assert_eq!(burns_for(200), "Burns for 1 item");
        assert_eq!(burns_for(100), "Burns for 0.5 items");
        let pickaxe = recipes_for(ItemId::WoodenPickaxe).next().unwrap();
        assert_eq!(
            card_label(&BrowserCard::Crafting(pickaxe)),
            "Recipe of Wooden Pickaxe"
        );
        let smelting = BrowserCard::Smelting {
            input: ItemId::IronOre,
            output: ItemId::IronIngot,
        };
        assert_eq!(card_label(&smelting), "Smelting of Iron Ingot");
        let fuel = BrowserCard::Fuel {
            item: ItemId::Coal,
            ticks: 1600,
        };
        assert_eq!(card_label(&fuel), "Coal as fuel");
    }

    #[test]
    fn an_item_without_recipe_or_use_says_so() {
        // Its uses count its smelting and its burning
        assert_eq!(
            empty_view_message(view(ItemId::Cobblestone, RecipeTab::Recipes)),
            "Cobblestone is gathered, not crafted. It goes into 7 recipes: see Uses."
        );
        assert_eq!(
            empty_view_message(view(ItemId::IronOre, RecipeTab::Recipes)),
            "Iron Ore is gathered, not crafted. It goes into 1 recipe: see Uses."
        );
        assert_eq!(
            empty_view_message(view(ItemId::SpruceLog, RecipeTab::Recipes)),
            "Spruce Log is gathered, not crafted. It goes into 3 recipes: see Uses."
        );
        assert_eq!(
            empty_view_message(view(ItemId::Bedrock, RecipeTab::Recipes)),
            "Bedrock is gathered, not crafted."
        );
        assert_eq!(
            empty_view_message(view(ItemId::Bedrock, RecipeTab::Uses)),
            "No recipe uses Bedrock yet."
        );
        assert_eq!(
            empty_view_message(view(ItemId::StoneSword, RecipeTab::Uses)),
            "No recipe uses Stone Sword yet."
        );
        // A plural once there are more
        assert_eq!(uses_sentence(3), " It goes into 3 recipes: see Uses.");
    }

    #[test]
    fn an_item_is_craftable_now_when_some_recipe_is() {
        let mut inventory = Inventory::new();
        assert!(!craftable_now(&inventory, ItemId::OakPlanks));
        put(&mut inventory, 9, ItemId::OakLog, 1);
        assert!(craftable_now(&inventory, ItemId::OakPlanks));
        assert!(!craftable_now(&inventory, ItemId::Stick));
        assert!(!craftable_now(&inventory, ItemId::OakLog));
        // The furnace does not count: it is filled by hand
        put(&mut inventory, 10, ItemId::IronOre, 1);
        assert!(!craftable_now(&inventory, ItemId::IronIngot));
        assert!(!craftable_now(&inventory, ItemId::Charcoal));
    }
}
