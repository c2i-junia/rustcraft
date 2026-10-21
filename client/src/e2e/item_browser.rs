//! The item browser beside the open inventory: the catalog, its search and its chips, its layout
//! by window size and its drawer, the keyboard, its recipes, its Creative gestures, its
//! bookmarks, and the express recipe book: Ctrl+click, the ghost of a recipe, the recent crafts.

use std::time::Duration;

use accesskit::Role;
use bevy::{
    input::keyboard::KeyCode,
    input_focus::InputFocus,
    prelude::*,
    ui::{Checked, InteractionDisabled},
};
use shared::players::{inventory::craft_result, Inventory};
use shared::world::grid_slot;
use shared::world::{Ingredient, ItemId, CRAFTING_GRID_SIZE};

use super::game::{enter_world, floating, hold_click, place_ahead, slot};
use super::harness::{Harness, SCREEN, SERVER};
use super::helpers::{ctrl, is_in};
use super::locator::Locator as L;
use super::seed::GameFolder;
use crate::constants::ITEM_BROWSER_PATH;
use crate::state::GameState;
use crate::ui::hud::hotbar::Hotbar;
use crate::ui::hud::item_browser::bookmarks::ItemBrowserFile;
use crate::ui::hud::item_browser::creative::TRASH_HINT;
use crate::ui::hud::item_browser::FILL_GRID;
use crate::ui::hud::item_browser::{
    BookmarkBar, BookmarkRow, BookmarkSlot, CatalogGrid, CatalogList, CatalogSlot, DrawerHandle,
    ItemBrowser, ItemBrowserState, ItemCount, ItemSearchField, RecentBar, RecentRow, RecentSlot,
    RecipeCard, RecipeCards, RecipePanel, RecipeSlot, TrashSlot, SEARCH_LABEL,
};
use crate::ui::hud::{
    CraftResultCell, CraftingGridCell, GhostIcon, GhostTint, GridGhost, HudBottom, InventoryCell,
    InventoryDialog, InventoryRoot, CRAFTING_TABLE_TITLE,
};
use crate::ui::menus::pause::PauseMenu;
use crate::ui::menus::MenuState;
use crate::ui::theme::ACCENT;
use crate::ui::widgets::slot::{RefusalShake, SlotIcon};
use crate::ui::widgets::tooltip::TooltipRoot;
use crate::world::MaterialResource;
use shared::players::game_mode::GameMode;

/// The search field of the browser.
fn search_field() -> L {
    L::role(Role::TextInput, SEARCH_LABEL)
}

/// The slot of `name` in the catalog.
fn catalog_slot(name: &str) -> L {
    L::role(Role::GridCell, name).within(L::marker::<ItemBrowser>())
}

/// The slots of `name` in the cards of the recipe view.
fn card_slot(name: &str) -> L {
    L::role(Role::GridCell, name).within(L::marker::<RecipeCard>())
}

/// The slot of `name` in the grid of the catalog.
fn catalog_grid_slot(name: &str) -> L {
    L::role(Role::GridCell, name).within(L::marker::<CatalogGrid>())
}

/// The slot of `name` in the row of bookmarks.
fn bookmark_slot(name: &str) -> L {
    L::role(Role::GridCell, name).within(L::marker::<BookmarkRow>())
}

/// A button of the browser, a chip or the back button, by its label.
fn button(label: &str) -> L {
    L::role(Role::Button, label)
}

impl Harness {
    /// The first slot of `name` in the cards of the recipe view.
    fn first_card_slot(&mut self, name: &str) -> Entity {
        self.find(&card_slot(name))[0]
    }

    /// What the count of the browser says.
    fn item_count(&mut self) -> String {
        self.text_of(&L::marker::<ItemCount>())
    }

    /// The items the catalog shows.
    fn shown_items(&mut self) -> Vec<ItemId> {
        let found = self.find(&L::marker::<CatalogSlot>());
        found
            .into_iter()
            .map(|entity| self.world().get::<CatalogSlot>(entity).unwrap().0)
            .collect()
    }

    /// How many columns the catalog shows: the distinct lefts of its slots.
    fn catalog_columns(&mut self) -> usize {
        let found = self.find(&L::marker::<CatalogSlot>());
        let mut lefts: Vec<i32> = found
            .into_iter()
            .map(|entity| {
                let world = self.world();
                let center = world.get::<UiGlobalTransform>(entity).unwrap().translation;
                let size = world.get::<ComputedNode>(entity).unwrap().size();
                (center.x - size.x / 2.0).round() as i32
            })
            .collect();
        lefts.sort_unstable();
        lefts.dedup();
        lefts.len()
    }

    /// Erases `n` characters before the caret of the focused field.
    fn erase(&mut self, n: usize) {
        for _ in 0..n {
            self.press(KeyCode::Backspace);
        }
    }
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn the_item_browser_lists_searches_and_filters_the_items() {
    let folder = GameFolder::new().with_world("Catalog").with_items(
        "Catalog",
        "Tester",
        &[(0, ItemId::Dirt, 5)],
    );
    let mut h = enter_world(folder, "Catalog");
    h.wait_until("the saved inventory", SERVER, |w| {
        slot(w, 0) == Some((ItemId::Dirt, 5))
    });
    assert!(!h.is_visible(&L::marker::<ItemBrowser>()));

    // Beside the open inventory, every item, in 8 columns at 1280x720
    h.press(KeyCode::KeyE);
    h.wait_for(&L::text("Items"), SCREEN);
    h.frames(2);
    assert_eq!(h.item_count(), "72 / 72");
    assert_eq!(h.shown_items().len(), 72);
    assert_eq!(h.catalog_columns(), 8);
    let window = Rect::new(0.0, 0.0, 1280.0, 720.0);
    let browser = h.rect_of(&L::marker::<ItemBrowser>());
    let panel = h.rect_of(&L::marker::<InventoryDialog>());
    assert!(window.contains(browser.min) && window.contains(browser.max));
    assert_eq!(window.max.x - browser.max.x, 16.0, "{browser:?}");
    assert!(
        browser.intersect(panel).is_empty(),
        "{browser:?} covers {panel:?}"
    );

    // Each word of the search narrows the list
    h.click(&search_field());
    h.type_text("sp pl");
    h.frames(1);
    assert_eq!(h.item_count(), "1 / 72");
    assert_eq!(h.shown_items(), [ItemId::SprucePlanks]);
    h.erase(5);
    h.type_text("#tools");
    h.frames(1);
    assert_eq!(h.item_count(), "27 / 72");

    // Nothing matches
    h.type_text("zzz");
    h.wait_for(&L::text("No item matches \"#toolszzz\"."), SCREEN);
    assert!(h.is_visible(&L::text("Esc clears the search.")));
    assert!(h.shown_items().is_empty());
    h.erase(9);
    h.frames(1);
    assert_eq!(h.item_count(), "72 / 72");
    assert!(!h.is_visible(&L::text("Esc clears the search.")));

    // A chip keeps one category
    h.click(&L::role(Role::Button, "Wood"));
    h.frames(1);
    assert_eq!(h.item_count(), "6 / 72");
    let wood = h.get(&L::role(Role::Button, "Wood"));
    assert!(h.world().get::<Checked>(wood).is_some());
    h.click(&L::role(Role::Button, "All"));
    h.frames(1);
    assert_eq!(h.item_count(), "72 / 72");
    assert!(h.world().get::<Checked>(wood).is_none());

    // The tooltip names the item and its category
    let tooltip = L::marker::<TooltipRoot>;
    h.hover(&catalog_slot("Oak Log"));
    for text in ["Oak Log", "Wood"] {
        h.wait_for(&L::text(text).within(tooltip()), SCREEN);
    }
    h.hover(&catalog_slot("Wooden Pickaxe"));
    h.wait_for(&L::text("Tools · Tool").within(tooltip()), SCREEN);

    // In Survival, a click on an item opens its recipes, and takes nothing; Escape goes back to
    // it
    let oak_log = h.get(&catalog_slot("Oak Log"));
    h.click(&catalog_slot("Oak Log"));
    h.wait_for(
        &L::text("Oak Log is gathered, not crafted. It goes into 3 recipes: see Uses."),
        SCREEN,
    );
    h.frames(10);
    assert_eq!(slot(h.world(), 0), Some((ItemId::Dirt, 5)));
    assert_eq!(floating(h.world()), None);
    h.press(KeyCode::Escape);
    h.frames(2);
    assert!(h.is_visible(&L::text("72 / 72")));
    assert_eq!(h.world().resource::<InputFocus>().get(), Some(oak_log));

    // The search stays while the inventory is closed
    h.click(&search_field());
    h.type_text("oak");
    h.frames(1);
    h.click(&L::cell(20));
    h.press(KeyCode::KeyE);
    h.frames(2);
    assert!(!h.is_visible(&L::marker::<InventoryRoot>()));
    assert!(!h.is_visible(&L::marker::<ItemBrowser>()));
    h.press(KeyCode::KeyE);
    h.wait_for(&L::text("Items"), SCREEN);
    assert_eq!(h.value_of::<ItemSearchField>(), "oak");
    assert_eq!(h.item_count(), "3 / 72");
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn the_item_browser_becomes_a_drawer_in_a_small_window() {
    let mut h = enter_world(GameFolder::new().with_world("Drawer"), "Drawer");
    h.press(KeyCode::KeyE);
    h.wait_for(&L::text("Items"), SCREEN);

    // Four columns still fit beside the panel at 854x480
    h.resize(854, 480);
    h.frames(2);
    assert!(h.is_visible(&L::marker::<ItemBrowser>()));
    assert_eq!(h.catalog_columns(), 4);
    assert!(!h.is_visible(&L::marker::<DrawerHandle>()));

    // Not at 640x480: a handle at the right edge, at mid-height
    h.resize(640, 480);
    h.frames(2);
    assert!(!h.is_visible(&L::marker::<ItemBrowser>()));
    let handle = h.rect_of(&L::marker::<DrawerHandle>());
    assert_eq!(handle.max.x, 640.0, "{handle:?}");
    assert!((handle.center().y - 240.0).abs() <= 1.0, "{handle:?}");

    // B opens the drawer, flush right, over the panel; Escape closes it, not the inventory
    h.press(KeyCode::KeyB);
    h.frames(2);
    let drawer = h.rect_of(&L::marker::<ItemBrowser>());
    assert_eq!(drawer.max.x, 640.0, "{drawer:?}");
    assert_eq!(h.catalog_columns(), 6);
    assert!(!h.is_visible(&L::marker::<DrawerHandle>()));
    h.press(KeyCode::Escape);
    h.frames(2);
    assert!(!h.is_visible(&L::marker::<ItemBrowser>()));
    assert!(h.is_visible(&L::marker::<InventoryRoot>()));
    assert!(!h.is_visible(&L::marker::<PauseMenu>()));

    // The handle opens it; a click on the inventory closes it, one inside it does not
    h.click(&L::marker::<DrawerHandle>());
    h.wait_for(&L::marker::<ItemBrowser>(), SCREEN);
    h.click(&L::text("Items").within(L::marker::<ItemBrowser>()));
    h.frames(2);
    assert!(h.is_visible(&L::marker::<ItemBrowser>()));
    h.click(&L::cell(0));
    h.frames(2);
    assert!(!h.is_visible(&L::marker::<ItemBrowser>()));
    assert!(h.is_visible(&L::marker::<InventoryRoot>()));

    // Closing the inventory closes the drawer: it opens closed
    h.press(KeyCode::KeyB);
    h.frames(2);
    assert!(h.is_visible(&L::marker::<ItemBrowser>()));
    h.press(KeyCode::KeyE);
    h.press(KeyCode::KeyE);
    h.wait_for(&L::marker::<InventoryRoot>(), SCREEN);
    h.frames(2);
    assert!(!h.is_visible(&L::marker::<ItemBrowser>()));
    assert!(h.is_visible(&L::marker::<DrawerHandle>()));

    // `/` opens it on the search; Escape leaves the search, then closes it
    h.type_on(KeyCode::Slash, '/');
    h.frames(2);
    assert!(h.is_visible(&L::marker::<ItemBrowser>()));
    let field = h.get(&search_field());
    assert_eq!(h.world().resource::<InputFocus>().get(), Some(field));
    h.press(KeyCode::Escape);
    h.frames(1);
    assert!(h.is_visible(&L::marker::<ItemBrowser>()));
    h.press(KeyCode::Escape);
    h.frames(2);
    assert!(!h.is_visible(&L::marker::<ItemBrowser>()));
    assert!(h.is_visible(&L::marker::<InventoryRoot>()));

    // Wide again, it docks, and the handle goes
    h.resize(1280, 720);
    h.frames(2);
    assert!(h.is_visible(&L::marker::<ItemBrowser>()));
    assert!(!h.is_visible(&L::marker::<DrawerHandle>()));
}

/// Whether the focus is on `HudBottom` or inside it: the inventory panel or the item browser.
fn focus_in_hud_bottom(w: &World) -> bool {
    let Some(mut entity) = w.resource::<InputFocus>().get() else {
        return false;
    };
    loop {
        if w.get::<HudBottom>(entity).is_some() {
            return true;
        }
        match w.get::<ChildOf>(entity) {
            Some(parent) => entity = parent.parent(),
            None => return false,
        }
    }
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn the_keyboard_reaches_the_item_browser_and_typing_in_it_moves_nothing() {
    let folder = GameFolder::new().with_world("Keys").with_items(
        "Keys",
        "Tester",
        &[(0, ItemId::Dirt, 5), (12, ItemId::Sand, 3)],
    );
    let mut h = enter_world(folder, "Keys");
    h.wait_until("the saved inventory", SERVER, |w| {
        slot(w, 12) == Some((ItemId::Sand, 3))
    });
    h.press(KeyCode::KeyE);
    h.wait_for(&L::text("Items"), SCREEN);
    let field = h.get(&search_field());
    let focused = |h: &Harness| h.world().resource::<InputFocus>().get();

    // `/` focuses the search, and is not typed in it
    h.type_on(KeyCode::Slash, '/');
    h.frames(1);
    assert_eq!(focused(&h), Some(field));
    assert_eq!(h.value_of::<ItemSearchField>(), "");

    // Over a slot, the game keys are only text: E, a digit, B and Space
    h.hover(&L::cell(12));
    h.type_text("e3b ");
    h.frames(10);
    assert_eq!(h.value_of::<ItemSearchField>(), "e3b ");
    assert!(h.is_visible(&L::marker::<InventoryRoot>()));
    assert_eq!(slot(h.world(), 12), Some((ItemId::Sand, 3)));
    assert_eq!(slot(h.world(), 2), None);

    // Escape clears the search, then leaves it for the inventory
    h.press(KeyCode::Escape);
    h.frames(1);
    assert_eq!(h.value_of::<ItemSearchField>(), "");
    assert_eq!(focused(&h), Some(field));
    h.press(KeyCode::Escape);
    h.frames(1);
    assert_ne!(focused(&h), Some(field));
    assert!(focus_in_hud_bottom(h.world()));
    assert!(h.is_visible(&L::marker::<InventoryRoot>()));

    // Ctrl+F focuses it too; Down goes to the first result, its ring and its tooltip showing
    ctrl(&mut h, KeyCode::KeyF);
    h.frames(1);
    assert_eq!(focused(&h), Some(field));
    h.type_text("planks");
    h.press(KeyCode::ArrowDown);
    h.frames(1);
    let oak_planks = h.get(&catalog_slot("Oak Planks"));
    assert_eq!(focused(&h), Some(oak_planks));
    assert_eq!(h.world().get::<Outline>(oak_planks).unwrap().color, ACCENT);
    h.wait_for(
        &L::text("Oak Planks").within(L::marker::<TooltipRoot>()),
        SCREEN,
    );

    // The arrows move between the results
    h.press(KeyCode::ArrowRight);
    let spruce_planks = h.get(&catalog_slot("Spruce Planks"));
    assert_eq!(focused(&h), Some(spruce_planks));
    h.press(KeyCode::ArrowLeft);
    assert_eq!(focused(&h), Some(oak_planks));

    // Enter in the search acts as a click on the first result: in Survival, its recipes, the
    // focus on the back button; Escape goes back to the result
    h.click(&search_field());
    h.press(KeyCode::Enter);
    h.wait_for(&button("Recipes 1"), SCREEN);
    assert_eq!(focused(&h), Some(h.get(&button("<- Items"))));
    h.press(KeyCode::Escape);
    h.wait_for(&L::text("3 / 72"), SCREEN);
    assert_eq!(focused(&h), Some(oak_planks));

    // Tab walks the inventory and the browser, and stays among them
    let (mut in_browser, mut in_inventory) = (false, false);
    for _ in 0..80 {
        h.press(KeyCode::Tab);
        let w = h.world();
        assert!(
            focus_in_hud_bottom(w),
            "Tab left the inventory and the browser"
        );
        let focus = w.resource::<InputFocus>().get().unwrap();
        in_browser |= w.get::<CatalogSlot>(focus).is_some();
        in_inventory |= w.get::<InventoryCell>(focus).is_some();
    }
    assert!(in_browser && in_inventory);

    // Closing lets go of the focus
    h.press(KeyCode::KeyE);
    h.frames(1);
    assert_eq!(focused(&h), None);
}

/// How far the cards of the recipe view are scrolled down.
fn cards_scrolled(world: &mut World) -> f32 {
    let mut cards = world.query_filtered::<&ChildOf, With<RecipeCards>>();
    let list = cards.single(world).unwrap().parent();
    world.get::<ScrollPosition>(list).unwrap().y
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn recipes_and_uses_open_from_the_browser_and_the_history_walks_back() {
    let folder = GameFolder::new().with_world("Recipes").with_items(
        "Recipes",
        "Tester",
        &[(0, ItemId::OakPlanks, 5), (1, ItemId::OakLog, 2)],
    );
    let mut h = enter_world(folder, "Recipes");
    h.wait_until("the saved inventory", SERVER, |w| {
        slot(w, 1) == Some((ItemId::OakLog, 2))
    });
    h.press(KeyCode::KeyE);
    h.wait_for(&L::text("Items"), SCREEN);
    let tooltip = L::marker::<TooltipRoot>;

    // A new player is in Survival: an item craftable now says so, and the keys of the browser
    h.hover(&catalog_slot("Oak Planks"));
    h.wait_for(
        &L::text("You have the ingredients").within(tooltip()),
        SCREEN,
    );
    assert!(
        h.is_visible(&L::text("R Recipes  U Uses  A Bookmark  Ctrl+click fill").within(tooltip()))
    );

    // The Craftable chip, at the end of the chips, which wrap, is in view and keeps what can be
    // crafted now
    let browser = h.rect_of(&L::marker::<ItemBrowser>());
    let chip = h.rect_of(&button("Craftable"));
    assert!(
        browser.contains(chip.min) && browser.contains(chip.max),
        "{chip:?} out of {browser:?}"
    );
    h.click(&button("Craftable"));
    h.frames(1);
    assert_eq!(h.item_count(), "3 / 72");
    let shown = h.shown_items();
    for item in [ItemId::OakPlanks, ItemId::Stick, ItemId::CraftingTable] {
        assert!(shown.contains(&item), "{item:?} not in {shown:?}");
    }
    h.click(&button("Craftable"));
    h.frames(1);
    assert_eq!(h.item_count(), "72 / 72");

    // A click on an item shows its recipes, with what the inventory lacks
    h.click(&catalog_slot("Wooden Pickaxe"));
    h.wait_for(&button("Recipes 1"), SCREEN);
    for shown in [
        L::text("Recipes"),
        button("Uses 1"),
        button("<- Items"),
        L::text("Missing 2 Stick"),
    ] {
        assert!(h.is_visible(&shown), "{shown:?}");
    }
    assert!(!h.is_visible(&L::text("72 / 72")));

    // An ingredient shows its recipes, a right click its uses, each going on the history
    let stick = h.first_card_slot("Stick");
    h.click_entity(stick, MouseButton::Left);
    h.wait_for(&button("<- Wooden Pickaxe"), SCREEN);
    assert!(h.is_visible(&button("Uses 28")));
    let planks = h.first_card_slot("Any Planks");
    h.click_entity(planks, MouseButton::Right);
    h.wait_for(&button("<- Stick"), SCREEN);
    assert!(h.is_visible(&L::text("Uses")));
    assert!(h.is_visible(&button("Uses 9")));
    // scrolled down, its cards are left at the top of the next view
    let first = h.first_card_slot("Any Planks");
    h.hover_entity(first);
    h.scroll(-5.);
    h.frames(2);
    assert!(cards_scrolled(h.world_mut()) > 0.0);

    // Escape and the back button walk the history back
    h.press(KeyCode::Escape);
    h.wait_for(&button("<- Wooden Pickaxe"), SCREEN);
    h.frames(2);
    assert_eq!(cards_scrolled(h.world_mut()), 0.0);
    h.click(&button("<- Wooden Pickaxe"));
    h.wait_for(&button("<- Items"), SCREEN);
    assert!(h.is_visible(&button("Recipes 1")));

    // A tab shows the uses of the same item, without the history: a wooden tool burns
    h.click(&button("Uses 1"));
    h.wait_for(&L::text("Burns for 1 item"), SCREEN);
    assert!(h.is_visible(&button("<- Items")));
    h.press(KeyCode::Escape);
    h.wait_for(&L::text("72 / 72"), SCREEN);
    assert!(h.is_visible(&L::marker::<InventoryRoot>()));
    assert!(!h.is_visible(&L::marker::<PauseMenu>()));

    // A right click shows the uses; an item without recipe says where it comes from
    h.right_click(&catalog_slot("Cobblestone"));
    h.wait_for(&button("Uses 7"), SCREEN);
    h.click(&button("Recipes 0"));
    h.wait_for(
        &L::text("Cobblestone is gathered, not crafted. It goes into 7 recipes: see Uses."),
        SCREEN,
    );

    // `/` leaves the view for the search; then Escape leaves the search, and closes the inventory
    h.type_on(KeyCode::Slash, '/');
    h.frames(2);
    assert!(h.is_visible(&L::text("72 / 72")));
    let field = h.get(&search_field());
    assert_eq!(h.world().resource::<InputFocus>().get(), Some(field));
    h.press(KeyCode::Escape);
    h.frames(1);
    h.press(KeyCode::Escape);
    h.frames(2);
    assert!(!h.is_visible(&L::marker::<InventoryRoot>()));
    assert!(!h.is_visible(&L::marker::<PauseMenu>()));
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn r_and_u_work_over_any_slot_in_both_modes_and_open_the_drawer() {
    let folder = GameFolder::new().with_world("Keys").with_items(
        "Keys",
        "Tester",
        &[(0, ItemId::OakPlanks, 5)],
    );
    let mut h = enter_world(folder, "Keys");
    h.wait_until("the saved inventory", SERVER, |w| {
        slot(w, 0) == Some((ItemId::OakPlanks, 5))
    });
    h.set_game_mode(GameMode::Creative);
    h.press(KeyCode::KeyE);
    h.wait_for(&L::text("Items"), SCREEN);
    h.frames(2);

    // In Creative, no Craftable chip, and a click on an item takes it rather than showing its
    // recipes; a second one gives it back
    assert!(!h.is_visible(&button("Craftable")));
    h.click(&catalog_slot("Oak Log"));
    h.wait_until("the logs on the cursor", SCREEN, |w| {
        floating(w) == Some((ItemId::OakLog, 64))
    });
    assert!(h.is_visible(&L::text("72 / 72")));
    assert!(!h.is_visible(&L::marker::<RecipePanel>()));
    h.click(&catalog_slot("Oak Log"));
    h.wait_until("an empty cursor", SCREEN, |w| floating(w).is_none());

    // R over a slot of the inventory shows the recipes of its item, U its uses
    h.hover(&L::cell(0));
    h.press(KeyCode::KeyR);
    h.wait_for(&button("Recipes 1"), SCREEN);
    assert!(h.is_visible(&L::text("Oak Planks").within(L::marker::<RecipePanel>())));
    h.press(KeyCode::KeyU);
    h.wait_for(&button("Uses 9"), SCREEN);
    assert!(h.is_visible(&button("<- Oak Planks")));

    // R over a slot of a card, and over one of the catalog
    let stick = h.first_card_slot("Stick");
    h.hover_entity(stick);
    h.press(KeyCode::KeyR);
    h.wait_for(&button("Uses 28"), SCREEN);
    h.press(KeyCode::Escape);
    h.press(KeyCode::Escape);
    h.press(KeyCode::Escape);
    h.wait_for(&L::text("72 / 72"), SCREEN);
    h.hover(&catalog_slot("Chest"));
    h.press(KeyCode::KeyR);
    h.wait_for(&button("Recipes 1"), SCREEN);
    h.press(KeyCode::Escape);
    h.wait_for(&L::text("72 / 72"), SCREEN);

    // In a drawer, U opens the drawer on the uses; Escape then goes back, closes the drawer, and
    // closes the inventory
    h.resize(640, 480);
    h.frames(2);
    assert!(!h.is_visible(&L::marker::<ItemBrowser>()));
    h.hover(&L::cell(0));
    h.press(KeyCode::KeyU);
    h.wait_for(&button("Uses 9"), SCREEN);
    assert!(h.is_visible(&L::marker::<ItemBrowser>()));
    h.press(KeyCode::Escape);
    h.wait_for(&L::text("72 / 72"), SCREEN);
    assert!(h.is_visible(&L::marker::<ItemBrowser>()));
    h.press(KeyCode::Escape);
    h.frames(2);
    assert!(!h.is_visible(&L::marker::<ItemBrowser>()));
    assert!(h.is_visible(&L::marker::<InventoryRoot>()));
    h.press(KeyCode::Escape);
    h.frames(2);
    assert!(!h.is_visible(&L::marker::<InventoryRoot>()));
    assert!(!h.is_visible(&L::marker::<PauseMenu>()));
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn creative_takes_from_the_catalog_and_the_list_destroys_the_held_stack() {
    let folder = GameFolder::new().with_world("Creative").with_items(
        "Creative",
        "Tester",
        &[(0, ItemId::Sand, 3)],
    );
    let mut h = enter_world(folder, "Creative");
    h.wait_until("the saved inventory", SERVER, |w| {
        slot(w, 0) == Some((ItemId::Sand, 3))
    });
    h.set_game_mode(GameMode::Creative);
    h.press(KeyCode::KeyE);
    h.wait_for(&L::text("Items"), SCREEN);
    h.frames(2);
    assert!(h.is_visible(&L::text(TRASH_HINT)));
    let held = |h: &mut Harness, what: &str, expected: Option<(ItemId, u32)>| {
        h.wait_until(what, SERVER, |w| floating(w) == expected);
    };

    // The tooltip says what the gestures take
    h.hover(&catalog_slot("Dirt"));
    h.wait_for(
        &L::text_containing("LMB 64  RMB 1  Shift to inventory").within(L::marker::<TooltipRoot>()),
        SCREEN,
    );

    // A click takes a full stack; a click on the list destroys it, and says so
    h.click(&catalog_slot("Dirt"));
    held(&mut h, "a stack of dirt", Some((ItemId::Dirt, 64)));
    let list = h.rect_of(&L::marker::<CatalogList>());
    h.click_at(Vec2::new(list.center().x, list.max.y - 20.0));
    h.wait_for(&L::text("Destroyed 64 Dirt"), SCREEN);
    held(&mut h, "an empty cursor", None);

    // A right click takes one, and one more each time; on another item it destroys them
    for _ in 0..3 {
        h.right_click(&catalog_slot("Stone"));
        h.frames(1);
    }
    held(&mut h, "three stones", Some((ItemId::Stone, 3)));
    h.right_click(&catalog_slot("Dirt"));
    h.wait_for(&L::text("Destroyed 3 Stone"), SCREEN);
    held(&mut h, "an empty cursor", None);

    // Shift+click puts a full stack into the inventory, the hotbar first: a tool comes alone
    h.key_down(KeyCode::ShiftLeft);
    h.click(&catalog_slot("Wooden Pickaxe"));
    h.key_up(KeyCode::ShiftLeft);
    h.wait_until("the pickaxe in the hotbar", SERVER, |w| {
        slot(w, 1) == Some((ItemId::WoodenPickaxe, 1))
    });
    assert_eq!(floating(h.world()), None);

    // The trash destroys the held stack too
    h.click(&catalog_slot("Dirt"));
    held(&mut h, "a stack of dirt", Some((ItemId::Dirt, 64)));
    h.click(&L::marker::<TrashSlot>());
    held(&mut h, "an empty cursor", None);

    // Enter in the search takes its first result, Shift+Enter puts it into the inventory
    h.type_on(KeyCode::Slash, '/');
    h.type_text("oak l");
    h.press(KeyCode::Enter);
    held(&mut h, "a stack of logs", Some((ItemId::OakLog, 64)));
    h.click(&L::cell(5));
    held(&mut h, "an empty cursor", None);
    h.click(&search_field());
    h.key_down(KeyCode::ShiftLeft);
    h.press(KeyCode::Enter);
    h.key_up(KeyCode::ShiftLeft);
    h.wait_until("a second stack of logs", SERVER, |w| {
        slot(w, 2) == Some((ItemId::OakLog, 64))
    });
    assert_eq!(slot(h.world(), 5), Some((ItemId::OakLog, 64)));

    // In Survival, the list leaves the held stack alone, and the trash is gone
    h.press(KeyCode::Escape);
    h.press(KeyCode::Escape);
    h.press(KeyCode::KeyE);
    h.frames(2);
    h.set_game_mode(GameMode::Survival);
    h.press(KeyCode::KeyE);
    h.wait_for(&L::text("Items"), SCREEN);
    h.frames(2);
    assert!(!h.is_visible(&L::text(TRASH_HINT)));
    h.click(&L::cell(0));
    held(&mut h, "the sand", Some((ItemId::Sand, 3)));
    let list = h.rect_of(&L::marker::<CatalogList>());
    h.click_at(Vec2::new(list.center().x, list.max.y - 20.0));
    h.frames(20);
    assert_eq!(floating(h.world()), Some((ItemId::Sand, 3)));
    assert_eq!(slot(h.world(), 1), Some((ItemId::WoodenPickaxe, 1)));
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn fill_grid_moves_a_recipe_into_the_grid() {
    let folder = GameFolder::new().with_world("Fill").with_items(
        "Fill",
        "Tester",
        &[(0, ItemId::OakLog, 1), (1, ItemId::OakPlanks, 6)],
    );
    let mut h = enter_world(folder, "Fill");
    h.wait_until("the saved inventory", SERVER, |w| {
        slot(w, 1) == Some((ItemId::OakPlanks, 6))
    });
    h.press(KeyCode::KeyE);
    h.wait_for(&L::text("Items"), SCREEN);
    h.frames(2);

    // The sticks can be made: their button is enabled
    h.click(&catalog_slot("Stick"));
    h.wait_for(&button("Recipes 1"), SCREEN);
    h.frames(2);
    let enabled: Vec<Entity> = h
        .find(&button(FILL_GRID))
        .into_iter()
        .filter(|&b| h.world().get::<InteractionDisabled>(b).is_none())
        .collect();
    assert_eq!(enabled.len(), 1);
    let fill = enabled[0];

    // A click fills the grid once, at its top left corner, in Survival
    h.click_entity(fill, MouseButton::Left);
    let planks = |n| Some((ItemId::OakPlanks, n));
    h.wait_until("one round in the grid", SERVER, |w| {
        slot(w, grid_slot(0, 0)) == planks(1)
            && slot(w, grid_slot(1, 0)) == planks(1)
            && slot(w, 1) == planks(4)
    });
    let made = craft_result(h.world().resource::<Inventory>(), 2);
    assert_eq!(made.map(|stack| stack.item_id), Some(ItemId::Stick));

    // Shift: as many rounds as the inventory holds, the grid back into it first
    h.key_down(KeyCode::ShiftLeft);
    h.click_entity(fill, MouseButton::Left);
    h.key_up(KeyCode::ShiftLeft);
    h.wait_until("three rounds in the grid", SERVER, |w| {
        slot(w, grid_slot(0, 0)) == planks(3) && slot(w, grid_slot(1, 0)) == planks(3)
    });
    assert_eq!(slot(h.world(), 1), None);
    assert_eq!(slot(h.world(), 0), Some((ItemId::OakLog, 1)));

    // The result is crafted as usual, one item from each cell
    h.click(&L::marker::<CraftResultCell>());
    h.wait_until("the sticks on the cursor", SERVER, |w| {
        floating(w) == Some((ItemId::Stick, 4))
            && slot(w, grid_slot(0, 0)) == planks(2)
            && slot(w, grid_slot(1, 0)) == planks(2)
    });
}

/// The indices in the items atlas of the icons of `cells`, and the index of the icon of `item`.
fn icons_of(w: &World, cells: &[Entity], item: ItemId) -> (Vec<Option<usize>>, Option<usize>) {
    let atlas = w.resource::<MaterialResource>().items.as_ref();
    let shown = cells
        .iter()
        .map(|&cell| {
            let children = w.get::<Children>(cell)?;
            let icon = children
                .iter()
                .find(|&child| w.get::<SlotIcon>(child).is_some())?;
            Some(w.get::<ImageNode>(icon)?.texture_atlas.as_ref()?.index)
        })
        .collect();
    let expected = atlas.and_then(|atlas| Some(atlas.sprite(&format!("{item:?}"))?.index));
    (shown, expected)
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn fill_grid_mixes_the_species_of_planks_at_a_table() {
    let folder = GameFolder::new().with_world("Species").with_items(
        "Species",
        "Tester",
        &[
            (0, ItemId::CraftingTable, 1),
            (9, ItemId::OakPlanks, 5),
            (10, ItemId::SprucePlanks, 4),
        ],
    );
    let mut h = enter_world(folder, "Species");
    h.wait_until("the saved inventory", SERVER, |w| {
        slot(w, 10) == Some((ItemId::SprucePlanks, 4))
    });
    place_ahead(&mut h);
    hold_click(&mut h, &L::marker::<Hotbar>(), MouseButton::Right, 1);
    h.wait_for(&L::text(CRAFTING_TABLE_TITLE), SCREEN);
    h.frames(2);

    // One card makes the chest, its ring any planks, the species counted together
    h.click(&catalog_slot("Chest"));
    h.wait_for(&button("Recipes 1"), SCREEN);
    h.frames(2);
    let ring = h.find(&card_slot("Any Planks"));
    assert_eq!(ring.len(), 8);
    assert!(h.is_visible(&L::text("Crafting Table 3x3")));
    let tooltip = L::marker::<TooltipRoot>;
    h.hover_entity(ring[0]);
    h.wait_for(&L::text("Any Planks").within(tooltip()), SCREEN);

    // The planks of the ring show each species in turn, all in step
    for species in [
        ItemId::SprucePlanks,
        ItemId::JunglePlanks,
        ItemId::OakPlanks,
    ] {
        // a frame under load may skip a second: the cycle comes round again
        h.wait_until(
            "the ring showing the next species",
            Duration::from_secs(10),
            |w| {
                let (shown, expected) = icons_of(w, &ring, species);
                expected.is_some() && shown.iter().all(|icon| *icon == expected)
            },
        );
        for &cell in &ring {
            assert_eq!(h.world().get::<RecipeSlot>(cell).unwrap().item, species);
        }
    }

    // Filled, the grid takes all the oak, then the spruce
    let fill = h.get(&button(FILL_GRID));
    assert!(h.world().get::<InteractionDisabled>(fill).is_none());
    h.click_entity(fill, MouseButton::Left);
    let oak = [(0, 0), (0, 1), (0, 2), (1, 0), (1, 2)];
    let spruce = [(2, 0), (2, 1), (2, 2)];
    h.wait_until("the ring in the grid", SERVER, |w| {
        oak.iter()
            .all(|&(row, col)| slot(w, grid_slot(row, col)) == Some((ItemId::OakPlanks, 1)))
            && spruce
                .iter()
                .all(|&(row, col)| slot(w, grid_slot(row, col)) == Some((ItemId::SprucePlanks, 1)))
            && slot(w, 9).is_none()
            && slot(w, 10) == Some((ItemId::SprucePlanks, 1))
    });
    let made = craft_result(h.world().resource::<Inventory>(), CRAFTING_GRID_SIZE);
    assert_eq!(made.map(|stack| stack.item_id), Some(ItemId::Chest));

    // And the mixed ring makes a chest
    h.click(&L::marker::<CraftResultCell>());
    h.wait_until("the chest on the cursor", SERVER, |w| {
        floating(w) == Some((ItemId::Chest, 1))
            && oak
                .iter()
                .chain(&spruce)
                .all(|&(row, col)| slot(w, grid_slot(row, col)).is_none())
    });
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn a_bookmarks_any_item_and_the_bookmarks_are_kept_per_world() {
    let folder = GameFolder::new()
        .with_world("Bookmarks")
        .with_world("Elsewhere")
        .with_items("Bookmarks", "Tester", &[(0, ItemId::WoodenPickaxe, 1)]);
    let mut h = enter_world(folder, "Bookmarks");
    h.wait_until("the saved inventory", SERVER, |w| {
        slot(w, 0) == Some((ItemId::WoodenPickaxe, 1))
    });
    h.press(KeyCode::KeyE);
    h.wait_for(&L::text("Items"), SCREEN);
    h.frames(2);
    let tooltip = L::marker::<TooltipRoot>;
    let saved = |h: &Harness| -> ItemBrowserFile {
        let text = std::fs::read_to_string(h.folder().join(ITEM_BROWSER_PATH)).unwrap();
        ron::from_str(&text).unwrap()
    };

    // A over a slot of the catalog bookmarks its item, says so, and saves it for this world
    h.hover(&catalog_grid_slot("Chest"));
    h.wait_for(
        &L::text("R Recipes  U Uses  A Bookmark  Ctrl+click fill").within(tooltip()),
        SCREEN,
    );
    h.press(KeyCode::KeyA);
    h.wait_for(&L::text("Bookmarked Chest"), SCREEN);
    // the row of bookmarks shows above the grid, which moves down
    h.frames(2);
    h.hover(&catalog_grid_slot("Chest"));
    h.wait_for(
        &L::text("R Recipes  U Uses  A Unbookmark  Ctrl+click fill").within(tooltip()),
        SCREEN,
    );
    assert_eq!(saved(&h).worlds["Bookmarks"], [ItemId::Chest]);

    // The recipe view says so under the item's name
    h.press(KeyCode::KeyR);
    h.wait_for(
        &L::text("Stations · bookmarked").within(L::marker::<RecipePanel>()),
        SCREEN,
    );
    h.press(KeyCode::Escape);
    h.wait_for(&L::text("72 / 72"), SCREEN);

    // A over a slot of the inventory too
    h.hover(&L::cell(0));
    h.press(KeyCode::KeyA);
    h.wait_for(&L::text("Bookmarked Wooden Pickaxe"), SCREEN);
    assert_eq!(
        saved(&h).worlds["Bookmarks"],
        [ItemId::Chest, ItemId::WoodenPickaxe]
    );
    h.press(KeyCode::KeyA);
    h.wait_for(&L::text("Removed Wooden Pickaxe from bookmarks"), SCREEN);
    assert_eq!(saved(&h).worlds["Bookmarks"], [ItemId::Chest]);

    // The bookmarks show between the chips and the grid, and act as the catalog's slots
    h.wait_for(&bookmark_slot("Chest"), SCREEN);
    assert!(h.is_visible(&L::text("Bookmarks").within(L::marker::<BookmarkBar>())));
    assert_eq!(h.find(&L::marker::<BookmarkSlot>()).len(), 1);
    h.click(&bookmark_slot("Chest"));
    h.wait_for(&button("Recipes 1"), SCREEN);
    assert!(!h.is_visible(&L::marker::<BookmarkBar>()));
    h.press(KeyCode::Escape);
    h.wait_for(&L::text("72 / 72"), SCREEN);
    assert!(h.is_visible(&bookmark_slot("Chest")));

    // The arrows reach them from the grid. A on the focused one removes it, and the focus goes
    // to the slot of its item in the grid, where A bookmarks it again
    let focused = |h: &Harness| h.world().resource::<InputFocus>().get();
    h.type_on(KeyCode::Slash, '/');
    h.press(KeyCode::ArrowDown);
    h.frames(1);
    h.press(KeyCode::ArrowUp);
    h.frames(1);
    assert_eq!(focused(&h), Some(h.get(&bookmark_slot("Chest"))));
    h.press(KeyCode::KeyA);
    h.wait_for(&L::text("Removed Chest from bookmarks"), SCREEN);
    h.frames(2);
    assert!(!h.is_visible(&L::marker::<BookmarkBar>()));
    assert_eq!(focused(&h), Some(h.get(&catalog_grid_slot("Chest"))));
    h.press(KeyCode::KeyA);
    h.wait_for(&bookmark_slot("Chest"), SCREEN);
    assert_eq!(saved(&h).worlds["Bookmarks"], [ItemId::Chest]);

    // They are there again when the world is entered again, and only in that world
    leave_and_enter(&mut h, "Bookmarks");
    h.press(KeyCode::KeyE);
    h.wait_for(&bookmark_slot("Chest"), SCREEN);
    leave_and_enter(&mut h, "Elsewhere");
    h.press(KeyCode::KeyE);
    h.wait_for(&L::text("Items"), SCREEN);
    h.frames(2);
    assert!(!h.is_visible(&L::marker::<BookmarkBar>()));
    assert!(h.find(&L::marker::<BookmarkSlot>()).is_empty());
    let file = saved(&h);
    assert_eq!(file.worlds.keys().collect::<Vec<_>>(), ["Bookmarks"]);
    assert!(file.servers.is_empty());
}

/// Closes the inventory, saves and quits to the title, and plays the world `world`.
fn leave_and_enter(h: &mut Harness, world: &str) {
    h.press(KeyCode::KeyE);
    h.frames(2);
    h.press(KeyCode::Escape);
    h.click(&button("Save and quit to title"));
    h.wait_until("the title", SERVER, |w| {
        is_in(w, GameState::Menu) && *w.resource::<State<MenuState>>().get() == MenuState::Main
    });
    h.click(&L::text("Singleplayer"));
    h.play_world(world);
    h.wait_for_game_state(GameState::Game, SERVER);
    h.wait_for(&L::marker::<Hotbar>(), SCREEN);
}

/// The slot of `name` in the row of recent crafts.
fn recent_slot(name: &str) -> L {
    L::role(Role::GridCell, name).within(L::marker::<RecentRow>())
}

impl Harness {
    /// Clicks `locator` with the left button while `keys` are held.
    fn click_holding(&mut self, keys: &[KeyCode], locator: &L) {
        for &key in keys {
            self.key_down(key);
        }
        self.click(locator);
        for &key in keys {
            self.key_up(key);
        }
    }

    /// The items of the row of recent crafts, in order.
    fn recent_items(&mut self) -> Vec<ItemId> {
        let found = self.find(&L::marker::<RecentSlot>());
        found
            .into_iter()
            .map(|entity| self.world().get::<CatalogSlot>(entity).unwrap().0)
            .collect()
    }
}

/// The slots of the crafting grid whose ghost tint shows, and those whose ghost icon shows.
fn ghost_shown(w: &World) -> (Vec<u32>, Vec<u32>) {
    let mut cells = w
        .try_query_filtered::<(&InventoryCell, &Children), With<CraftingGridCell>>()
        .unwrap();
    let (mut tinted, mut iconed) = (Vec::new(), Vec::new());
    for (cell, children) in cells.iter(w) {
        for child in children.iter() {
            let shown = w.get::<Visibility>(child) != Some(&Visibility::Hidden);
            if shown && w.get::<GhostTint>(child).is_some() {
                tinted.push(cell.id);
            }
            if shown && w.get::<GhostIcon>(child).is_some() {
                iconed.push(cell.id);
            }
        }
    }
    tinted.sort_unstable();
    iconed.sort_unstable();
    (tinted, iconed)
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn ctrl_click_fills_the_grid_from_the_browser() {
    let folder = GameFolder::new().with_world("Express").with_items(
        "Express",
        "Tester",
        &[(9, ItemId::OakPlanks, 6)],
    );
    let mut h = enter_world(folder, "Express");
    h.wait_until("the saved inventory", SERVER, |w| {
        slot(w, 9) == Some((ItemId::OakPlanks, 6))
    });
    h.press(KeyCode::KeyE);
    h.wait_for(&L::text("Items"), SCREEN);
    h.frames(2);
    let planks = |n| Some((ItemId::OakPlanks, n));
    let ctrl = [KeyCode::ControlLeft];

    // The tooltip says so
    h.hover(&catalog_slot("Stick"));
    h.wait_for(
        &L::text("R Recipes  U Uses  A Bookmark  Ctrl+click fill")
            .within(L::marker::<TooltipRoot>()),
        SCREEN,
    );

    // Ctrl+click fills one round of the sticks, and opens no view
    h.click_holding(&ctrl, &catalog_slot("Stick"));
    h.wait_until("one round of sticks in the grid", SERVER, |w| {
        slot(w, grid_slot(0, 0)) == planks(1)
            && slot(w, grid_slot(1, 0)) == planks(1)
            && slot(w, 9) == planks(4)
    });
    assert!(!h.is_visible(&L::marker::<RecipePanel>()));
    assert_eq!(h.world().resource::<ItemBrowserState>().view, None);

    // Ctrl+Shift: as many rounds as the inventory holds, the grid put back first
    h.click_holding(
        &[KeyCode::ControlLeft, KeyCode::ShiftLeft],
        &catalog_slot("Stick"),
    );
    h.wait_until("three rounds of sticks in the grid", SERVER, |w| {
        slot(w, grid_slot(0, 0)) == planks(3)
            && slot(w, grid_slot(1, 0)) == planks(3)
            && slot(w, 9).is_none()
    });

    // Nothing makes dirt: its slot shakes, and nothing is sent
    let dirt = h.get(&catalog_slot("Dirt"));
    h.click_holding(&ctrl, &catalog_slot("Dirt"));
    h.wait_until("the dirt shaking", SCREEN, |w| {
        w.get::<RefusalShake>(dirt).is_some()
    });
    h.frames(20);
    assert_eq!(slot(h.world(), grid_slot(0, 0)), planks(3));
    assert_eq!(h.world().resource::<ItemBrowserState>().view, None);

    // In Creative, Ctrl+click fills too, and takes nothing out of the catalog
    h.press(KeyCode::KeyE);
    h.frames(2);
    h.set_game_mode(GameMode::Creative);
    h.press(KeyCode::KeyE);
    h.wait_for(&L::text("Items"), SCREEN);
    h.frames(2);
    h.click_holding(&ctrl, &catalog_slot("Crafting Table"));
    h.wait_until("the square of planks in the grid", SERVER, |w| {
        [(0, 0), (0, 1), (1, 0), (1, 1)]
            .iter()
            .all(|&(row, col)| slot(w, grid_slot(row, col)) == planks(1))
    });
    h.frames(10);
    assert_eq!(floating(h.world()), None);
    let made = craft_result(h.world().resource::<Inventory>(), 2);
    assert_eq!(made.map(|stack| stack.item_id), Some(ItemId::CraftingTable));
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn a_card_missing_ingredients_lays_a_ghost_until_the_grid_changes() {
    let folder = GameFolder::new().with_world("Ghost").with_items(
        "Ghost",
        "Tester",
        &[
            (9, ItemId::Stick, 1),
            (10, ItemId::OakPlanks, 4),
            (11, ItemId::Dirt, 3),
        ],
    );
    let mut h = enter_world(folder, "Ghost");
    h.wait_until("the saved inventory", SERVER, |w| {
        slot(w, 11) == Some((ItemId::Dirt, 3))
    });
    let open_torches = |h: &mut Harness| {
        h.press(KeyCode::KeyE);
        h.wait_for(&L::text("Items"), SCREEN);
        h.frames(2);
        h.click(&catalog_slot("Torch"));
        h.wait_for(&button("Recipes 2"), SCREEN);
        h.frames(2);
    };
    let fill_buttons = |h: &mut Harness| -> Vec<Entity> {
        let buttons = h.find(&button(FILL_GRID));
        for &b in &buttons {
            assert!(h.world().get::<InteractionDisabled>(b).is_none());
        }
        buttons
    };
    let (coal, stick) = (grid_slot(0, 0), grid_slot(1, 0));
    let laid = (vec![coal], vec![coal, stick]);
    let no_ghost = (vec![], vec![]);
    open_torches(&mut h);
    assert!(h.is_visible(&L::text("Missing 1 Coal")));

    // The button of a card missing its coal is enabled: it lays the ghost and sends nothing
    let buttons = fill_buttons(&mut h);
    h.click_entity(buttons[0], MouseButton::Left);
    h.wait_until("the ghost of the torch", SCREEN, |w| ghost_shown(w) == laid);
    h.frames(20);
    assert_eq!(ghost_shown(h.world()), laid);
    assert_eq!(slot(h.world(), 9), Some((ItemId::Stick, 1)));
    assert_eq!(slot(h.world(), coal), None);

    // A stack put into the grid takes it away
    h.click(&L::cell(11));
    h.click(&L::cell(grid_slot(1, 1)));
    h.wait_until("the ghost gone with the dirt", SERVER, |w| {
        slot(w, grid_slot(1, 1)) == Some((ItemId::Dirt, 3)) && ghost_shown(w) == no_ghost
    });

    // Laid again, closing the inventory takes it away; with the dirt in the grid, the stick's
    // cell is still empty
    h.click_entity(buttons[0], MouseButton::Left);
    h.wait_until("the ghost again", SCREEN, |w| ghost_shown(w) == laid);
    h.press(KeyCode::KeyE);
    h.frames(2);
    assert_eq!(h.world().resource::<GridGhost>().0, None);
    assert_eq!(ghost_shown(h.world()), no_ghost);

    // Another ghost replaces it: the charcoal's
    open_torches(&mut h);
    let buttons = fill_buttons(&mut h);
    h.click_entity(buttons[0], MouseButton::Left);
    h.wait_until("the ghost of the torch of coal", SCREEN, |w| {
        ghost_shown(w) == laid
    });
    h.click_entity(buttons[1], MouseButton::Left);
    h.wait_until("the ghost of the torch of charcoal", SCREEN, |w| {
        w.resource::<GridGhost>()
            .0
            .as_ref()
            .is_some_and(|ghost| ghost.cells[0].ingredient == Ingredient::Item(ItemId::Charcoal))
    });
    assert_eq!(ghost_shown(h.world()), laid);

    // And a fill replaces it: Ctrl+click on the crafting table puts the grid back and fills it
    h.press(KeyCode::Escape);
    h.wait_for(&L::text("72 / 72"), SCREEN);
    h.click_holding(&[KeyCode::ControlLeft], &catalog_slot("Crafting Table"));
    h.wait_until("the square of planks in the grid", SERVER, |w| {
        [(0, 0), (0, 1), (1, 0), (1, 1)]
            .iter()
            .all(|&(row, col)| slot(w, grid_slot(row, col)) == Some((ItemId::OakPlanks, 1)))
    });
    assert_eq!(h.world().resource::<GridGhost>().0, None);
    assert_eq!(ghost_shown(h.world()), no_ghost);
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn the_items_crafted_go_into_recent() {
    let folder = GameFolder::new().with_world("Recent").with_items(
        "Recent",
        "Tester",
        &[(9, ItemId::OakLog, 2)],
    );
    let mut h = enter_world(folder, "Recent");
    h.wait_until("the saved inventory", SERVER, |w| {
        slot(w, 9) == Some((ItemId::OakLog, 2))
    });
    h.press(KeyCode::KeyE);
    h.wait_for(&L::text("Items"), SCREEN);
    h.frames(2);
    assert!(!h.is_visible(&L::marker::<RecentBar>()));
    let ctrl = [KeyCode::ControlLeft];
    let craft_all = |h: &mut Harness| {
        h.key_down(KeyCode::ShiftLeft);
        h.click(&L::marker::<CraftResultCell>());
        h.key_up(KeyCode::ShiftLeft);
    };

    // Planks from a log, then sticks: the newest first
    h.click_holding(&ctrl, &catalog_grid_slot("Oak Planks"));
    h.wait_until("a log in the grid", SERVER, |w| {
        slot(w, grid_slot(0, 0)) == Some((ItemId::OakLog, 1))
    });
    craft_all(&mut h);
    h.wait_for(&recent_slot("Oak Planks"), SCREEN);
    assert!(h.is_visible(&L::text("Recent").within(L::marker::<RecentBar>())));
    h.wait_until("four planks", SERVER, |w| {
        (0..36).any(|n| slot(w, n) == Some((ItemId::OakPlanks, 4)))
    });
    h.click_holding(&ctrl, &catalog_grid_slot("Stick"));
    h.wait_until("planks in the grid", SERVER, |w| {
        slot(w, grid_slot(1, 0)) == Some((ItemId::OakPlanks, 1))
    });
    craft_all(&mut h);
    h.wait_for(&recent_slot("Stick"), SCREEN);
    assert_eq!(h.recent_items(), [ItemId::Stick, ItemId::OakPlanks]);

    // Planks again: once, and first
    h.wait_until("an empty grid", SERVER, |w| {
        slot(w, grid_slot(0, 0)).is_none() && slot(w, grid_slot(1, 0)).is_none()
    });
    h.click_holding(&ctrl, &catalog_grid_slot("Oak Planks"));
    h.wait_until("the other log in the grid", SERVER, |w| {
        slot(w, grid_slot(0, 0)) == Some((ItemId::OakLog, 1))
    });
    craft_all(&mut h);
    h.wait_until("the planks first again", SCREEN, |w| {
        let mut slots = w
            .try_query_filtered::<&CatalogSlot, With<RecentSlot>>()
            .unwrap();
        let items: Vec<ItemId> = slots.iter(w).map(|slot| slot.0).collect();
        items == [ItemId::OakPlanks, ItemId::Stick]
    });
    h.frames(2);
    assert_eq!(h.recent_items(), [ItemId::OakPlanks, ItemId::Stick]);

    // A recent slot acts as the catalog's: Ctrl+click fills the grid with its recipe
    h.click_holding(&ctrl, &recent_slot("Stick"));
    h.wait_until("planks for sticks in the grid", SERVER, |w| {
        slot(w, grid_slot(0, 0)) == Some((ItemId::OakPlanks, 1))
            && slot(w, grid_slot(1, 0)) == Some((ItemId::OakPlanks, 1))
    });
}
