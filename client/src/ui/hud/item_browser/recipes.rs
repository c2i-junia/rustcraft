//! What the recipe view says: its title, its back button, its tabs, the station of a recipe or why
//! it cannot be made now, and what it says when there is no recipe to show.

// First used by the recipe view (Task 4)
#![allow(dead_code)]

use shared::players::Inventory;
use shared::world::{craftable, missing, recipes_for, uses_of, ItemId, RecipeView};

use super::{ItemView, RecipeTab};
use crate::ui::hud::item_name;

/// The line of the tooltip of an item craftable now, in Survival.
pub const CRAFTABLE_NOTE: &str = "You have the ingredients";
/// The keys of the tooltip of an item of the browser.
pub const ITEM_KEYS_HINT: &str = "R Recipes  U Uses";

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

/// Where `recipe` is made: in the inventory's 2×2 grid when it fits, else on a crafting table.
pub fn station_label(recipe: &RecipeView) -> &'static str {
    if recipe.fits(2) {
        "Inventory 2×2"
    } else {
        "Crafting Table 3×3"
    }
}

/// Why `inventory` cannot make `recipe` now with a grid of `grid_size`: the ingredients it lacks,
/// else the crafting table it needs. `None` when it can.
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
    (!recipe.fits(grid_size)).then(|| "Needs a Crafting Table".to_string())
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
            uses_sentence(uses_of(view.item).count())
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
        assert_eq!(station_label(&stick), "Inventory 2×2");
        assert_eq!(station_label(&pickaxe), "Crafting Table 3×3");
        assert_eq!(CRAFTABLE_NOTE, "You have the ingredients");
        assert_eq!(ITEM_KEYS_HINT, "R Recipes  U Uses");
    }

    #[test]
    fn the_problem_of_a_recipe_is_what_is_missing_then_the_table() {
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
    }

    #[test]
    fn an_item_without_recipe_or_use_says_so() {
        assert_eq!(
            empty_view_message(view(ItemId::Cobblestone, RecipeTab::Recipes)),
            "Cobblestone is gathered, not crafted. It goes into 1 recipe: see Uses."
        );
        assert_eq!(
            empty_view_message(view(ItemId::SpruceLog, RecipeTab::Recipes)),
            "Spruce Log is gathered, not crafted. It goes into 1 recipe: see Uses."
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
            empty_view_message(view(ItemId::WoodenSword, RecipeTab::Uses)),
            "No recipe uses Wooden Sword yet."
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
    }
}
