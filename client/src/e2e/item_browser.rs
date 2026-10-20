//! The item browser beside the open inventory: the catalog, its search and its chips, its layout
//! by window size and its drawer, and the keyboard.

use accesskit::Role;
use bevy::{input::keyboard::KeyCode, input_focus::InputFocus, prelude::*, ui::Checked};
use shared::world::ItemId;

use super::game::{enter_world, floating, slot};
use super::harness::{Harness, SCREEN, SERVER};
use super::locator::Locator as L;
use super::seed::GameFolder;
use crate::ui::hud::item_browser::{
    CatalogSlot, DrawerHandle, ItemBrowser, ItemCount, ItemSearchField, SEARCH_LABEL,
};
use crate::ui::hud::{InventoryDialog, InventoryRoot};
use crate::ui::menus::pause::PauseMenu;
use crate::ui::widgets::tooltip::TooltipRoot;

/// The search field of the browser.
fn search_field() -> L {
    L::role(Role::TextInput, SEARCH_LABEL)
}

/// The slot of `name` in the catalog.
fn catalog_slot(name: &str) -> L {
    L::role(Role::GridCell, name).within(L::marker::<ItemBrowser>())
}

impl Harness {
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
    assert_eq!(h.item_count(), "33 / 33");
    assert_eq!(h.shown_items().len(), 33);
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
    assert_eq!(h.item_count(), "1 / 33");
    assert_eq!(h.shown_items(), [ItemId::SprucePlanks]);
    h.erase(5);
    h.type_text("#tools");
    h.frames(1);
    assert_eq!(h.item_count(), "5 / 33");

    // Nothing matches
    h.type_text("zzz");
    h.wait_for(&L::text("No item matches \"#toolszzz\"."), SCREEN);
    assert!(h.is_visible(&L::text("Esc clears the search.")));
    assert!(h.shown_items().is_empty());
    h.erase(9);
    h.frames(1);
    assert_eq!(h.item_count(), "33 / 33");
    assert!(!h.is_visible(&L::text("Esc clears the search.")));

    // A chip keeps one category
    h.click(&L::role(Role::Button, "Wood"));
    h.frames(1);
    assert_eq!(h.item_count(), "4 / 33");
    let wood = h.get(&L::role(Role::Button, "Wood"));
    assert!(h.world().get::<Checked>(wood).is_some());
    h.click(&L::role(Role::Button, "All"));
    h.frames(1);
    assert_eq!(h.item_count(), "33 / 33");
    assert!(h.world().get::<Checked>(wood).is_none());

    // The tooltip names the item and its category
    let tooltip = L::marker::<TooltipRoot>;
    h.hover(&catalog_slot("Oak Log"));
    for text in ["Oak Log", "Wood"] {
        h.wait_for(&L::text(text).within(tooltip()), SCREEN);
    }
    h.hover(&catalog_slot("Wooden Pickaxe"));
    h.wait_for(&L::text("Tools · Tool").within(tooltip()), SCREEN);

    // A click on an item focuses it, and takes nothing
    h.click(&catalog_slot("Oak Log"));
    h.frames(10);
    let oak_log = h.get(&catalog_slot("Oak Log"));
    assert_eq!(h.world().resource::<InputFocus>().get(), Some(oak_log));
    assert_eq!(slot(h.world(), 0), Some((ItemId::Dirt, 5)));
    assert_eq!(floating(h.world()), None);

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
    assert_eq!(h.item_count(), "3 / 33");
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

    // Wide again, it docks, and the handle goes
    h.resize(1280, 720);
    h.frames(2);
    assert!(h.is_visible(&L::marker::<ItemBrowser>()));
    assert!(!h.is_visible(&L::marker::<DrawerHandle>()));
}
