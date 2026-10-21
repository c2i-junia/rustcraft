//! Golden screenshots of the hotbar and of the inventory panel. See `crate::ui::golden`.
//! They need lavapipe, which is only installed on Linux.

use bevy::prelude::*;
use shared::players::Inventory;
use shared::world::{grid_slot, Furnace, ItemId, ItemStack};

use crate::constants::MAX_HOTBAR_SLOTS;
use crate::ui::golden::UiGolden;
use crate::ui::hud::item_browser::goldens::show_open_inventory;
use crate::world::ClientWorldMap;

fn stack(item_id: ItemId, nb: u32) -> ItemStack {
    ItemStack {
        item_id,
        item_type: item_id.get_default_type(),
        nb,
    }
}

/// Renders the hotbar and, if `open`, the inventory panel, as they show `inventory`.
fn assert_inventory_matches(name: &str, inventory: Inventory, open: bool) {
    assert_panel_matches(name, inventory, ClientWorldMap::default(), open);
}

/// `assert_inventory_matches`, with the chests the client knows in `world_map`.
fn assert_panel_matches(name: &str, inventory: Inventory, world_map: ClientWorldMap, open: bool) {
    let mut golden = UiGolden::new();
    golden.world_mut().insert_resource(inventory);
    golden.world_mut().insert_resource(world_map);
    // as in the game, with the item browser beside the open panel
    show_open_inventory(&mut golden, open);
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

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn an_open_chest_shows_its_slots_above_the_backpack() {
    use shared::world::{ChunkMap, Container};

    let chest = IVec3::new(1, 2, 3);
    let mut blocks = ChunkMap::with_loaded_region(IVec3::ZERO, IVec3::splat(15));
    blocks.fill_box(chest, chest, shared::world::BlockId::Chest);
    let mut contents = Container::default();
    contents.slots[0] = Some(stack(ItemId::Cobblestone, 64));
    contents.slots[10] = Some(stack(ItemId::OakLog, 12));
    contents.slots[26] = Some(stack(ItemId::Stick, 3));
    blocks.insert_container(chest, contents);
    let mut inventory = Inventory::new();
    inventory.open_chest = Some(chest);
    inventory.inner.insert(0, stack(ItemId::Dirt, 20));
    inventory
        .inner
        .insert(MAX_HOTBAR_SLOTS, stack(ItemId::OakPlanks, 4));
    let world_map = ClientWorldMap {
        blocks,
        ..default()
    };
    assert_panel_matches("chest", inventory, world_map, true);
}

/// An open furnace at (1, 2, 3) holding `furnace`, with stacks in the inventory to put in it.
fn assert_furnace_matches(name: &str, block: shared::world::BlockId, furnace: Furnace) {
    use shared::world::ChunkMap;

    let pos = IVec3::new(1, 2, 3);
    let mut blocks = ChunkMap::with_loaded_region(IVec3::ZERO, IVec3::splat(15));
    blocks.fill_box(pos, pos, block);
    blocks.insert_furnace(pos, furnace);
    let mut inventory = Inventory::new();
    inventory.open_furnace = Some(pos);
    inventory.inner.insert(0, stack(ItemId::Sand, 20));
    inventory.inner.insert(1, stack(ItemId::Coal, 7));
    inventory
        .inner
        .insert(MAX_HOTBAR_SLOTS, stack(ItemId::Cobblestone, 64));
    let world_map = ClientWorldMap {
        blocks,
        ..default()
    };
    assert_panel_matches(name, inventory, world_map, true);
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn an_open_furnace_shows_its_slots_flame_and_arrow_above_the_backpack() {
    assert_furnace_matches(
        "furnace",
        shared::world::BlockId::Furnace,
        Furnace::default(),
    );
}

/// Half its coal burnt and half its sand cooked: the flame and the arrow half lit.
#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn a_lit_furnace_shows_its_fire_and_how_far_it_cooked() {
    assert_furnace_matches(
        "furnace_smelting",
        shared::world::BlockId::LitFurnace,
        Furnace {
            input: Some(stack(ItemId::Sand, 12)),
            fuel: Some(stack(ItemId::Coal, 3)),
            output: Some(stack(ItemId::Glass, 5)),
            burn_left: 800,
            burn_total: 1600,
            cook: 100,
        },
    );
}
