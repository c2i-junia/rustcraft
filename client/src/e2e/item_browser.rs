//! The item browser beside the open inventory: the catalog, its search and its chips, its layout
//! by window size and its drawer, the keyboard, its recipes, its Creative gestures and its
//! bookmarks.

use accesskit::Role;
use bevy::{
    input::keyboard::KeyCode,
    input_focus::InputFocus,
    prelude::*,
    ui::{Checked, InteractionDisabled},
};
use shared::players::{inventory::craft_result, Inventory};
use shared::world::grid_slot;
use shared::world::ItemId;

use super::game::{enter_world, floating, slot};
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
    ItemBrowser, ItemCount, ItemSearchField, RecipeCard, RecipeCards, RecipePanel, TrashSlot,
    SEARCH_LABEL,
};
use crate::ui::hud::{CraftResultCell, HudBottom, InventoryCell, InventoryDialog, InventoryRoot};
use crate::ui::menus::pause::PauseMenu;
use crate::ui::menus::MenuState;
use crate::ui::theme::ACCENT;
use crate::ui::widgets::tooltip::TooltipRoot;
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
    assert!(h.is_visible(&L::text("R Recipes  U Uses  A Bookmark").within(tooltip())));

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
    h.wait_for(&button("Recipes 3"), SCREEN);
    for shown in [
        L::text("Recipes"),
        button("Uses 1"),
        button("<- Items"),
        L::text("Missing 2 Stick"),
        L::text("Missing 3 Spruce Planks, 2 Stick"),
        L::text("Missing 3 Jungle Planks, 2 Stick"),
    ] {
        assert!(h.is_visible(&shown), "{shown:?}");
    }
    assert!(!h.is_visible(&L::text("72 / 72")));

    // An ingredient shows its recipes, a right click its uses, each going on the history
    let stick = h.first_card_slot("Stick");
    h.click_entity(stick, MouseButton::Left);
    h.wait_for(&button("<- Wooden Pickaxe"), SCREEN);
    assert!(h.is_visible(&button("Uses 38")));
    let planks = h.first_card_slot("Oak Planks");
    h.click_entity(planks, MouseButton::Right);
    h.wait_for(&button("<- Stick"), SCREEN);
    assert!(h.is_visible(&L::text("Uses")));
    assert!(h.is_visible(&button("Uses 9")));
    // scrolled down, its cards are left at the top of the next view
    let first = h.first_card_slot("Oak Planks");
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
    assert!(h.is_visible(&button("Recipes 3")));

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
    h.wait_for(&button("Uses 38"), SCREEN);
    h.press(KeyCode::Escape);
    h.press(KeyCode::Escape);
    h.press(KeyCode::Escape);
    h.wait_for(&L::text("72 / 72"), SCREEN);
    h.hover(&catalog_slot("Chest"));
    h.press(KeyCode::KeyR);
    h.wait_for(&button("Recipes 3"), SCREEN);
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

    // The sticks of oak can be made: their button is the only one enabled
    h.click(&catalog_slot("Stick"));
    h.wait_for(&button("Recipes 3"), SCREEN);
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
        &L::text("R Recipes  U Uses  A Bookmark").within(tooltip()),
        SCREEN,
    );
    h.press(KeyCode::KeyA);
    h.wait_for(&L::text("Bookmarked Chest"), SCREEN);
    // the row of bookmarks shows above the grid, which moves down
    h.frames(2);
    h.hover(&catalog_grid_slot("Chest"));
    h.wait_for(
        &L::text("R Recipes  U Uses  A Unbookmark").within(tooltip()),
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
    h.wait_for(&button("Recipes 3"), SCREEN);
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
