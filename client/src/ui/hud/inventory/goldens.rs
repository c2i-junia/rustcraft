//! Golden screenshots of the hotbar and of the inventory panel. See `crate::ui::golden`.
//! They need lavapipe, which is only installed on Linux.

use bevy::prelude::*;
use shared::players::Inventory;
use shared::world::{grid_slot, ItemId, ItemStack};

use crate::constants::MAX_HOTBAR_SLOTS;
use crate::ui::golden::UiGolden;
use crate::ui::hud::{
    display_crafting_system, display_inventory_system, setup_inventory, style_inventory_panel,
    InventoryRoot,
};
use crate::ui::widgets::slot::style_slots;

fn stack(item_id: ItemId, nb: u32) -> ItemStack {
    ItemStack {
        item_id,
        item_type: item_id.get_default_type(),
        nb,
    }
}

/// Renders the hotbar and, if `open`, the inventory panel, as they show `inventory`.
fn assert_inventory_matches(name: &str, inventory: Inventory, open: bool) {
    let mut golden = UiGolden::new();
    golden.world_mut().insert_resource(inventory);
    golden.run_system(setup_inventory);
    if open {
        let mut root = golden
            .world_mut()
            .query_filtered::<&mut Visibility, With<InventoryRoot>>();
        *root.single_mut(golden.world_mut()).unwrap() = Visibility::Visible;
    }
    golden.run_system(display_inventory_system);
    golden.run_system(display_crafting_system);
    golden.run_system(style_inventory_panel);
    golden.run_system(style_slots);
    golden.assert_matches(name);
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn the_hotbar() {
    let mut inventory = Inventory::new();
    inventory.inner.insert(0, stack(ItemId::Dirt, 64));
    inventory.inner.insert(2, stack(ItemId::OakLog, 3));
    inventory
        .inner
        .insert(MAX_HOTBAR_SLOTS - 1, stack(ItemId::Stick, 12));
    assert_inventory_matches("hotbar", inventory, false);
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn the_inventory_crafts_in_a_2x2_grid() {
    let mut inventory = Inventory::new();
    for (row, col) in [(0, 0), (0, 1), (1, 0), (1, 1)] {
        inventory
            .inner
            .insert(grid_slot(row, col), stack(ItemId::OakPlanks, 1));
    }
    inventory
        .inner
        .insert(MAX_HOTBAR_SLOTS, stack(ItemId::OakPlanks, 4));
    assert_inventory_matches("inventory_2x2_grid", inventory, true);
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn a_crafting_table_crafts_in_a_3x3_grid() {
    let mut inventory = Inventory::new();
    inventory.open_table = Some(IVec3::ZERO);
    for row in 0..3 {
        for col in 0..3 {
            if (row, col) != (1, 1) {
                inventory
                    .inner
                    .insert(grid_slot(row, col), stack(ItemId::Cobblestone, 1));
            }
        }
    }
    assert_inventory_matches("crafting_table_3x3_grid", inventory, true);
}

/// Items left beyond the 2x2 grid, once a table is closed, are shown in their own cell.
#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn the_inventory_shows_the_items_left_beyond_its_grid() {
    let mut inventory = Inventory::new();
    inventory
        .inner
        .insert(grid_slot(0, 0), stack(ItemId::OakLog, 1));
    inventory
        .inner
        .insert(grid_slot(2, 2), stack(ItemId::Cobblestone, 1));
    assert_inventory_matches("inventory_leftovers_beyond_the_grid", inventory, true);
}
