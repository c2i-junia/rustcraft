//! What the recipe view says: its title, its back button, its tabs, its cards, the station of a
//! recipe or why it cannot be made now, and what it says when there is no recipe to show.

use crate::i18n::{tr, trf};
use shared::players::Inventory;
use shared::world::{
    craftable, fuel_ticks, missing, recipes_for, smelting_inputs, smelting_result, uses_of,
    Ingredient, ItemId, RecipeView, COOK_TICKS,
};
use shared::TICKS_PER_SECOND;

use super::catalog::category_detail;
use super::{ItemView, RecipeTab};
use crate::ui::hud::item_name;

/// The line of the tooltip of an item craftable now, in Survival.
pub const CRAFTABLE_NOTE: &str = "browser.craftable_note";
/// The keys of the tooltip of an item of the browser; A removes a bookmarked item from the
/// bookmarks.
pub fn item_keys_hint(bookmarked: bool) -> String {
    let bookmark = if bookmarked {
        tr("browser.unbookmark")
    } else {
        tr("browser.bookmark")
    };
    trf("browser.item_keys", &[("bookmark", &bookmark)])
}

/// What Ctrl+click on a catalog slot does, on the key line of its tooltip.
pub const CTRL_CLICK_HINT: &str = "browser.ctrl_click";

/// The keys of the tooltip of a catalog slot (the grid, a bookmark, a recent item): those of an
/// item slot, then Ctrl+click when the item has a recipe of the crafting grid to fill it with.
pub fn catalog_keys_hint(item: ItemId, bookmarked: bool) -> String {
    let keys = item_keys_hint(bookmarked);
    if recipes_for(item).next().is_some() {
        format!("{keys}  {}", crate::i18n::tr(CTRL_CLICK_HINT))
    } else {
        keys
    }
}

/// The line under the name of the item of the view: its category, and whether it is bookmarked.
pub fn bookmarked_detail(item: ItemId, bookmarked: bool) -> String {
    let detail = category_detail(item);
    if bookmarked {
        trf("browser.bookmarked_detail", &[("detail", &detail)])
    } else {
        detail
    }
}

/// The title of the browser showing `tab`.
pub fn view_title(tab: RecipeTab) -> &'static str {
    match tab {
        RecipeTab::Recipes => tr("browser.recipes"),
        RecipeTab::Uses => tr("browser.uses"),
    }
}

/// The label of the back button: where it goes, the last item of `history` or the catalog.
pub fn back_label(history: &[ItemView]) -> String {
    match history.last() {
        Some(view) => format!("<- {}", item_name(view.item)),
        None => tr("browser.back_to_items").to_string(),
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
        tr("browser.grid_inventory")
    } else {
        tr("browser.grid_table")
    }
}

/// The name of `ingredient`: its item's, or its family's, as in `Missing 3 Planks`.
pub fn ingredient_name(ingredient: Ingredient) -> String {
    match ingredient {
        Ingredient::Item(item) => item_name(item),
        Ingredient::Tag(tag) => crate::i18n::tag_name(tag).to_string(),
    }
}

/// The name of a cell of `ingredient` on a card: its item's, or `Any` and its family's.
pub fn ingredient_label(ingredient: Ingredient) -> String {
    match ingredient {
        Ingredient::Item(item) => item_name(item),
        Ingredient::Tag(tag) => trf("browser.any", &[("tag", &crate::i18n::tag_name(tag))]),
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
            .map(|(ingredient, count)| format!("{count} {}", ingredient_name(ingredient)))
            .collect();
        return Some(trf("browser.missing", &[("items", &items.join(", "))]));
    }
    if !recipe.fits(grid_size) {
        return Some(tr("browser.needs_table").to_string());
    }
    // The chest or the furnace takes the place of the crafting grid in the panel
    if inventory.has_chest_open() {
        return Some(crate::i18n::tr(NOT_BESIDE_A_CHEST).to_string());
    }
    if inventory.open_engraver.is_some() {
        return Some(crate::i18n::tr(NOT_BESIDE_AN_ENGRAVING_TABLE).to_string());
    }
    if inventory.open_anvil.is_some() {
        return Some(crate::i18n::tr(NOT_BESIDE_AN_ANVIL).to_string());
    }
    inventory
        .open_furnace
        .is_some()
        .then(|| crate::i18n::tr(NOT_BESIDE_A_FURNACE).to_string())
}

/// What `+ Fill grid` does on a card: fill the grid with its recipe, or lay a ghost of it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FillButton {
    Fill,
    Ghost,
}

/// What `+ Fill grid` does on the card of `recipe` with a grid of `grid_size`: it fills when the
/// recipe can be made now, lays a ghost when ingredients are its only problem, and is disabled
/// (`None`) otherwise.
pub fn fill_button(
    inventory: &Inventory,
    recipe: &RecipeView,
    grid_size: u32,
) -> Option<FillButton> {
    if recipe_problem(inventory, recipe, grid_size).is_none() {
        return Some(FillButton::Fill);
    }
    let only_missing = !missing(inventory, recipe).is_empty()
        && recipe.fits(grid_size)
        && !inventory.has_chest_open()
        && inventory.open_furnace.is_none()
        && inventory.open_engraver.is_none()
        && inventory.open_anvil.is_none();
    only_missing.then_some(FillButton::Ghost)
}

/// Why a recipe cannot fill the grid while an engraving table is open.
pub const NOT_BESIDE_AN_ENGRAVING_TABLE: &str = "browser.not_beside_engraving";
/// Why a recipe cannot fill the grid while an anvil is open.
pub const NOT_BESIDE_AN_ANVIL: &str = "browser.not_beside_anvil";
/// Why a recipe cannot fill the grid while a chest is open.
pub const NOT_BESIDE_A_CHEST: &str = "browser.not_beside_chest";
/// Why a recipe cannot fill the grid while a furnace is open.
pub const NOT_BESIDE_A_FURNACE: &str = "browser.not_beside_furnace";

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
    /// The alembic makes `output` of `vial` with `ingredient` (`shared::world::alchemy`); with a
    /// modifier, `output` is `vial` made longer or stronger.
    Distilling {
        vial: ItemId,
        ingredient: ItemId,
        output: ItemId,
    },
}

/// The cards of the alembic about `item`: what makes it, with `recipes`, else what it makes or
/// works, the modifiers shown on the potion of swiftness.
fn distilling_cards(item: ItemId, recipes: bool) -> Vec<BrowserCard> {
    use shared::world::alchemy::{distilling_inputs, LONGER, RECIPES, STRONGER};
    let card = |vial, ingredient, output| BrowserCard::Distilling {
        vial,
        ingredient,
        output,
    };
    if recipes {
        let mut cards: Vec<_> = distilling_inputs(item)
            .map(|(vial, ingredient)| card(vial, ingredient, item))
            .collect();
        if let Some(potion) = item.potion() {
            if potion.lengthens() {
                cards.push(card(item, LONGER, item));
            }
            if potion.strengthens() {
                cards.push(card(item, STRONGER, item));
            }
        }
        return cards;
    }
    let mut cards: Vec<_> = RECIPES
        .iter()
        .filter(|(vial, with, _)| *vial == item || *with == item)
        .map(|&(vial, with, output)| card(vial, with, output))
        .collect();
    if item == LONGER || item == STRONGER {
        cards.push(card(ItemId::SwiftnessPotion, item, ItemId::SwiftnessPotion));
    }
    cards
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
            .chain(distilling_cards(item, true))
            .collect(),
        RecipeTab::Uses => uses_of(item)
            .map(BrowserCard::Crafting)
            .chain(smelting_result(item).map(|output| BrowserCard::Smelting {
                input: item,
                output,
            }))
            .chain(fuel_ticks(item).map(|ticks| BrowserCard::Fuel { item, ticks }))
            .chain(distilling_cards(item, false))
            .collect(),
    }
}

/// The accessible name of a card.
pub fn card_label(card: &BrowserCard) -> String {
    match card {
        BrowserCard::Crafting(recipe) => trf(
            "browser.recipe_of",
            &[("item", &item_name(recipe.output.item_id))],
        ),
        BrowserCard::Smelting { output, .. } => {
            trf("browser.smelting_of", &[("item", &item_name(*output))])
        }
        BrowserCard::Fuel { item, .. } => trf("browser.as_fuel", &[("item", &item_name(*item))]),
        BrowserCard::Distilling { output, .. } => {
            trf("browser.distilling_of", &[("item", &item_name(*output))])
        }
    }
}

/// Under a card of the alembic: where it is made and how long it takes, and what a modifier
/// `ingredient` does.
pub fn distilling_note(ingredient: ItemId) -> String {
    use shared::world::alchemy::{DISTILL_TICKS, LONGER, STRONGER};
    let time = trf(
        "browser.alembic_time",
        &[("s", &(u64::from(DISTILL_TICKS) / TICKS_PER_SECOND))],
    );
    match ingredient {
        LONGER => format!("{time} · {}", tr("browser.alembic_longer")),
        STRONGER => format!("{time} · {}", tr("browser.alembic_stronger")),
        _ => time,
    }
}

/// Under a smelting card: where it is made, and how long it takes.
pub fn smelting_note() -> String {
    trf(
        "browser.furnace_time",
        &[("s", &(u64::from(COOK_TICKS) / TICKS_PER_SECOND))],
    )
}

/// Under a fuel card: how long one of it burns, `ticks`.
pub fn fuel_note(ticks: u16) -> String {
    trf(
        "browser.fuel_time",
        &[("s", &(u64::from(ticks) / TICKS_PER_SECOND))],
    )
}

/// Beside the fuel of a fuel card: how many items one of it smelts, burning `ticks` long.
pub fn burns_for(ticks: u16) -> String {
    let items = f32::from(ticks) / f32::from(COOK_TICKS);
    if items == 1.0 {
        tr("browser.burns_one").to_string()
    } else {
        trf("browser.burns", &[("items", &items)])
    }
}

/// After the message of an item without recipe: how many recipes it goes into.
fn uses_sentence(uses: usize) -> String {
    match uses {
        0 => String::new(),
        1 => tr("browser.uses_one").to_string(),
        uses => trf("browser.uses_many", &[("uses", &uses)]),
    }
}

/// What the view says when it has no recipe to show.
pub fn empty_view_message(view: ItemView) -> String {
    let name = item_name(view.item);
    match view.tab {
        RecipeTab::Recipes => {
            let uses = uses_sentence(
                cards_of(ItemView {
                    tab: RecipeTab::Uses,
                    ..view
                })
                .len(),
            );
            trf("browser.gathered", &[("name", &name), ("uses", &uses)])
        }
        RecipeTab::Uses => trf("browser.no_uses", &[("name", &name)]),
    }
}

/// Whether `inventory` holds the ingredients of some recipe of `item`, whatever the grid size.
pub fn craftable_now(inventory: &Inventory, item: ItemId) -> bool {
    recipes_for(item).any(|recipe| craftable(inventory, &recipe))
}

#[cfg(test)]
mod tests {
    use shared::players::Inventory;
    use shared::world::{grid_slot, recipes_for, Ingredient, ItemId, ItemStack, ItemTag};

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
        assert_eq!(crate::i18n::tr(CRAFTABLE_NOTE), "You have the ingredients");
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
    fn the_keys_of_a_catalog_slot_name_ctrl_click_when_the_item_is_crafted() {
        assert_eq!(
            catalog_keys_hint(ItemId::Chest, false),
            "R Recipes  U Uses  A Bookmark  Ctrl+click fill"
        );
        assert_eq!(
            catalog_keys_hint(ItemId::Torch, true),
            "R Recipes  U Uses  A Unbookmark  Ctrl+click fill"
        );
        // Gathered or smelted, not crafted
        assert_eq!(
            catalog_keys_hint(ItemId::Dirt, false),
            "R Recipes  U Uses  A Bookmark"
        );
        assert_eq!(
            catalog_keys_hint(ItemId::Glass, false),
            "R Recipes  U Uses  A Bookmark"
        );
    }

    #[test]
    fn the_problem_of_a_recipe_is_what_is_missing_then_the_table_then_the_chest_or_furnace() {
        let pickaxe = recipes_for(ItemId::WoodenPickaxe).next().unwrap();
        let stick = recipes_for(ItemId::Stick).next().unwrap();
        let mut inventory = Inventory::new();
        assert_eq!(
            recipe_problem(&inventory, &pickaxe, 2).as_deref(),
            Some("Missing 3 Planks, 2 Stick")
        );
        // every species counts
        put(&mut inventory, 0, ItemId::OakPlanks, 1);
        put(&mut inventory, 1, ItemId::JunglePlanks, 1);
        assert_eq!(
            recipe_problem(&inventory, &pickaxe, 2).as_deref(),
            Some("Missing 1 Planks, 2 Stick")
        );
        put(&mut inventory, 2, ItemId::SprucePlanks, 1);
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
    fn fill_grid_fills_a_recipe_made_now_and_lays_the_ghost_of_one_missing_ingredients() {
        let pickaxe = recipes_for(ItemId::WoodenPickaxe).next().unwrap();
        let stick = recipes_for(ItemId::Stick).next().unwrap();
        let mut inventory = Inventory::new();
        assert_eq!(fill_button(&inventory, &stick, 2), Some(FillButton::Ghost));
        assert_eq!(
            fill_button(&inventory, &pickaxe, 3),
            Some(FillButton::Ghost)
        );
        // Missing ingredients and a table: neither
        assert_eq!(fill_button(&inventory, &pickaxe, 2), None);
        put(&mut inventory, 0, ItemId::OakPlanks, 3);
        put(&mut inventory, 1, ItemId::Stick, 2);
        assert_eq!(fill_button(&inventory, &stick, 2), Some(FillButton::Fill));
        assert_eq!(fill_button(&inventory, &pickaxe, 3), Some(FillButton::Fill));
        assert_eq!(fill_button(&inventory, &pickaxe, 2), None);
        // Beside a chest or a furnace: neither
        let empty = Inventory {
            open_chest: Some(bevy::math::IVec3::ZERO),
            ..Inventory::new()
        };
        assert_eq!(fill_button(&empty, &stick, 2), None);
        inventory.open_furnace = Some(bevy::math::IVec3::ZERO);
        assert_eq!(fill_button(&inventory, &stick, 2), None);
    }

    #[test]
    fn an_ingredient_is_named_by_its_item_or_its_family() {
        let planks = Ingredient::Tag(ItemTag::Planks);
        let stick = Ingredient::Item(ItemId::Stick);
        assert_eq!(ingredient_name(planks), "Planks");
        assert_eq!(ingredient_name(stick), "Stick");
        assert_eq!(ingredient_label(planks), "Any Planks");
        assert_eq!(
            ingredient_label(Ingredient::Item(ItemId::OakPlanks)),
            "Oak Planks"
        );
    }

    #[test]
    fn one_card_stands_for_every_species_of_planks() {
        let cards = |item, tab| cards_of(view(item, tab));
        let pickaxe = cards(ItemId::WoodenPickaxe, RecipeTab::Recipes);
        assert_eq!(pickaxe.len(), 1);
        assert!(matches!(pickaxe[0], BrowserCard::Crafting(recipe)
            if recipe.ingredients()[0] == (Ingredient::Tag(ItemTag::Planks), 3)));
        assert_eq!(cards(ItemId::Chest, RecipeTab::Recipes).len(), 1);
        assert_eq!(cards(ItemId::Stick, RecipeTab::Recipes).len(), 1);
        // The uses of any planks are the recipes of planks, then those of the species (its
        // slab, stairs, door, trapdoor and fence gate), then its table and its chair and the
        // bookshelf of any planks, then its burning
        for species in ItemTag::Planks.members() {
            let uses = cards(*species, RecipeTab::Uses);
            assert_eq!(uses.len(), 14 + 5 + 3 + 1, "{species:?}");
            assert_eq!(uses[..14], cards(ItemId::OakPlanks, RecipeTab::Uses)[..14]);
            assert_eq!(
                uses[22],
                BrowserCard::Fuel {
                    item: *species,
                    ticks: 300
                }
            );
        }
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
                smelting(ItemId::WillowLog, ItemId::Charcoal),
                smelting(ItemId::AcaciaLog, ItemId::Charcoal),
            ]
        );
        assert_eq!(
            cards(ItemId::Glass, RecipeTab::Recipes),
            [smelting(ItemId::Sand, ItemId::Glass)]
        );
        // An iron ingot comes out of its block, then of the furnace
        let ingot = cards(ItemId::IronIngot, RecipeTab::Recipes);
        assert!(
            matches!(ingot[0], BrowserCard::Crafting(recipe) if recipe.cells == [Some(Ingredient::Item(ItemId::IronBlock))])
        );
        assert_eq!(ingot[1..], [smelting(ItemId::IronOre, ItemId::IronIngot)]);
        // An oak log makes planks, smelts into charcoal, and burns
        let log = cards(ItemId::OakLog, RecipeTab::Uses);
        assert!(
            matches!(log[0], BrowserCard::Crafting(recipe) if recipe.output.item_id == ItemId::OakPlanks)
        );
        assert_eq!(
            log[2..],
            [
                smelting(ItemId::OakLog, ItemId::Charcoal),
                BrowserCard::Fuel {
                    item: ItemId::OakLog,
                    ticks: 300
                },
            ]
        );
        // charcoal makes torches, black dye and empty maps, then burns
        let charcoal = cards(ItemId::Charcoal, RecipeTab::Uses);
        assert!(
            matches!(charcoal[1], BrowserCard::Crafting(recipe) if recipe.output.item_id == ItemId::BlackDye)
        );
        assert!(
            matches!(charcoal[2], BrowserCard::Crafting(recipe) if recipe.output.item_id == ItemId::EmptyMap)
        );
        assert_eq!(
            charcoal[3..],
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
            "Cobblestone is gathered, not crafted. It goes into 11 recipes: see Uses."
        );
        assert_eq!(
            empty_view_message(view(ItemId::IronOre, RecipeTab::Recipes)),
            "Iron Ore is gathered, not crafted. It goes into 1 recipe: see Uses."
        );
        assert_eq!(
            empty_view_message(view(ItemId::SpruceLog, RecipeTab::Recipes)),
            "Spruce Log is gathered, not crafted. It goes into 4 recipes: see Uses."
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
