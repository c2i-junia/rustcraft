//! Golden screenshots of the item browser beside the open inventory, at the sizes where it docks
//! and where it is a drawer. See `crate::ui::golden`. They need lavapipe, which is only installed
//! on Linux.

use bevy::prelude::*;
use shared::players::Inventory;
use shared::world::{ItemId, ItemStack};

use super::layout::BrowserLayout;
use super::{
    filter_catalog, layout_item_browser, mark_craftable_items, setup_item_browser,
    show_catalog_icons, show_item_browser, show_mode_footer, style_chips, update_craftable_now,
    CraftableNow, ItemBrowserState,
};
use crate::ui::golden::UiGolden;
use crate::ui::hud::{
    display_crafting_system, display_inventory_system, setup_inventory, style_inventory_panel,
    InventoryRoot,
};
use crate::ui::widgets::list::hide_idle_scrollbars;
use crate::ui::widgets::slot::style_slots;
use crate::world::ClientWorldMap;

/// Builds the panel and the browser in `golden`, opens the inventory, and runs the systems that
/// show them, twice: the browser is laid out from the size of the window and of the panel.
pub(crate) fn show_open_inventory(golden: &mut UiGolden, open: bool) {
    golden.world_mut().init_resource::<ItemBrowserState>();
    golden.world_mut().init_resource::<BrowserLayout>();
    golden.world_mut().init_resource::<CraftableNow>();
    golden.run_system(setup_inventory);
    golden.run_system(setup_item_browser);
    if open {
        let mut root = golden
            .world_mut()
            .query_filtered::<&mut Visibility, With<InventoryRoot>>();
        *root.single_mut(golden.world_mut()).unwrap() = Visibility::Visible;
    }
    for _ in 0..2 {
        golden.frames(1);
        golden.run_system(display_inventory_system);
        golden.run_system(display_crafting_system);
        golden.run_system(style_inventory_panel);
        golden.run_system(layout_item_browser);
        golden.run_system(show_catalog_icons);
        golden.run_system(update_craftable_now);
        golden.run_system(filter_catalog);
        golden.run_system(mark_craftable_items);
        golden.run_system(show_mode_footer);
        golden.run_system(show_item_browser);
        golden.run_system(style_chips);
        golden.run_system(style_slots);
        golden.run_system(hide_idle_scrollbars);
    }
}

fn stack(item_id: ItemId, nb: u32) -> ItemStack {
    ItemStack {
        item_id,
        item_type: item_id.get_default_type(),
        nb,
    }
}

/// Renders the open inventory, holding a few stacks, and the browser beside it, in a window of
/// `width` by `height`, with the browser in `state`.
fn assert_browser_matches(name: &str, (width, height): (u32, u32), state: ItemBrowserState) {
    let mut golden = UiGolden::with_size(width, height);
    let mut inventory = Inventory::new();
    inventory.inner.insert(0, stack(ItemId::WoodenPickaxe, 1));
    inventory.inner.insert(1, stack(ItemId::Cobblestone, 40));
    inventory.inner.insert(12, stack(ItemId::OakLog, 12));
    golden.world_mut().insert_resource(inventory);
    golden.world_mut().init_resource::<ClientWorldMap>();
    golden.world_mut().insert_resource(state);
    show_open_inventory(&mut golden, true);
    golden.assert_matches(name);
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn the_item_browser_docks_eight_columns_at_1280x720() {
    assert_browser_matches(
        "item_browser_1280x720",
        (1280, 720),
        ItemBrowserState::default(),
    );
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn the_item_browser_docks_four_columns_at_854x480() {
    assert_browser_matches(
        "item_browser_854x480",
        (854, 480),
        ItemBrowserState::default(),
    );
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn the_drawer_is_a_handle_at_640x480() {
    assert_browser_matches(
        "item_browser_drawer_closed_640x480",
        (640, 480),
        ItemBrowserState::default(),
    );
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn the_open_drawer_covers_the_right_of_the_inventory_at_640x480() {
    let state = ItemBrowserState {
        drawer_open: true,
        ..default()
    };
    assert_browser_matches("item_browser_drawer_open_640x480", (640, 480), state);
}
