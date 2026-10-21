//! The gestures of a seasoned Minecraft player in the inventory: a drag of the held stack over
//! the grid, Shift+click on the result and a double click, through the integrated server.

use std::time::Duration;

use bevy::{input::keyboard::KeyCode, prelude::*, time::TimeUpdateStrategy};
use shared::players::inventory::craft_result;
use shared::players::Inventory;
use shared::world::{grid_slot, ItemId, CRAFTING_GRID_SIZE};

use super::game::{enter_world, floating, hold_click, place_ahead, slot};
use super::harness::{Harness, SCREEN, SERVER};
use super::locator::Locator as L;
use super::seed::GameFolder;
use crate::ui::hud::hotbar::Hotbar;
use crate::ui::hud::{
    CraftResultCell, FloatingStack, InventoryRoot, CHEST_TITLE, CRAFTING_TABLE_TITLE,
    INVENTORY_TITLE,
};
use crate::ui::theme::ACCENT;
use crate::ui::widgets::slot::SlotCount;
use crate::world::ClientWorldMap;

/// The ring of eight cells around the centre of the 3x3 grid: a chest, out of planks.
const RING: [(u32, u32); 8] = [
    (0, 0),
    (0, 1),
    (0, 2),
    (1, 2),
    (2, 2),
    (2, 1),
    (2, 0),
    (1, 0),
];

/// What the grid in use makes, as the client sees it.
fn made(w: &World, size: u32) -> Option<ItemId> {
    craft_result(w.resource::<Inventory>(), size).map(|stack| stack.item_id)
}

/// The colour of the count shown by the cell of `slot`.
fn count_color(h: &mut Harness, slot: u32) -> Color {
    let count = h.get(&L::marker::<SlotCount>().within(L::cell(slot)));
    h.world().get::<TextColor>(count).unwrap().0
}

/// Runs `clicks` with a clock that moves 20 ms a frame, whatever a frame takes under load, so
/// that two clicks a few frames apart make a double click.
fn with_steady_clock(h: &mut Harness, clicks: impl FnOnce(&mut Harness)) {
    let step = Duration::from_millis(20);
    h.world_mut()
        .insert_resource(TimeUpdateStrategy::ManualDuration(step));
    clicks(h);
    h.world_mut().insert_resource(TimeUpdateStrategy::Automatic);
}

/// Clicks the cell of `slot` and waits until the cursor holds `held`.
fn pick_up(h: &mut Harness, slot: u32, held: (ItemId, u32)) {
    h.click(&L::cell(slot));
    h.wait_until("the stack held", SERVER, |w| floating(w) == Some(held));
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn a_drag_splits_planks_over_the_ring_of_a_chest() {
    let planks = ItemId::OakPlanks;
    let folder = GameFolder::new().with_world("Ring").with_items(
        "Ring",
        "Tester",
        &[(0, ItemId::CraftingTable, 1), (9, planks, 18)],
    );
    let mut h = enter_world(folder, "Ring");
    h.wait_until("the saved inventory", SERVER, |w| {
        slot(w, 9) == Some((planks, 18))
    });
    place_ahead(&mut h);
    hold_click(&mut h, &L::marker::<Hotbar>(), MouseButton::Right, 1);
    h.wait_for(&L::text(CRAFTING_TABLE_TITLE), SCREEN);
    pick_up(&mut h, 9, (planks, 18));

    // While the button is down, the cells show what they would get, and the cursor what stays
    let ring = RING.map(|(row, col)| grid_slot(row, col));
    h.mouse_down(&L::cell(ring[0]));
    for cell in &ring[1..] {
        h.hover(&L::cell(*cell));
    }
    h.frames(1);
    for cell in ring {
        assert_eq!(h.text_of(&L::cell(cell)), "2", "cell {cell}");
        assert_eq!(count_color(&mut h, cell), ACCENT, "cell {cell}");
    }
    assert_eq!(h.text_of(&L::marker::<FloatingStack>()), "2");
    h.snapshot("drag_over_the_ring");
    // Nothing is sent before the release
    h.frames(10);
    assert_eq!(floating(h.world()), Some((planks, 18)));
    assert!(ring.iter().all(|&cell| slot(h.world(), cell).is_none()));

    // Released: two planks in each cell, two held, and a chest to take
    h.mouse_up();
    h.wait_until("the planks in the ring", SERVER, |w| {
        ring.iter().all(|&cell| slot(w, cell) == Some((planks, 2)))
            && floating(w) == Some((planks, 2))
    });
    h.frames(30);
    assert!(ring
        .iter()
        .all(|&cell| slot(h.world(), cell) == Some((planks, 2))));
    assert_eq!(made(h.world(), CRAFTING_GRID_SIZE), Some(ItemId::Chest));
    assert_eq!(count_color(&mut h, ring[0]), crate::ui::theme::TEXT);
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn a_right_drag_drops_one_plank_in_each_cell_of_the_grid() {
    let planks = ItemId::OakPlanks;
    let folder =
        GameFolder::new()
            .with_world("Square")
            .with_items("Square", "Tester", &[(9, planks, 6)]);
    let mut h = enter_world(folder, "Square");
    h.wait_until("the saved inventory", SERVER, |w| {
        slot(w, 9) == Some((planks, 6))
    });
    h.press(KeyCode::KeyE);
    h.wait_for(&L::text(INVENTORY_TITLE), SCREEN);
    pick_up(&mut h, 9, (planks, 6));

    let grid = [(0, 0), (0, 1), (1, 1), (1, 0)].map(|(row, col)| grid_slot(row, col));
    h.mouse_down_with(&L::cell(grid[0]), MouseButton::Right);
    for cell in &grid[1..] {
        h.hover(&L::cell(*cell));
    }
    h.frames(1);
    assert_eq!(h.text_of(&L::marker::<FloatingStack>()), "2");
    h.button_up(MouseButton::Right);

    h.wait_until("a plank in each cell", SERVER, |w| {
        grid.iter().all(|&cell| slot(w, cell) == Some((planks, 1)))
            && floating(w) == Some((planks, 2))
    });
    h.frames(30);
    assert!(grid
        .iter()
        .all(|&cell| slot(h.world(), cell) == Some((planks, 1))));
    assert_eq!(made(h.world(), 2), Some(ItemId::CraftingTable));
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn shift_click_on_the_result_crafts_a_stack_of_logs_into_planks() {
    let folder = GameFolder::new().with_world("Logs").with_items(
        "Logs",
        "Tester",
        &[(grid_slot(0, 0), ItemId::OakLog, 64)],
    );
    let mut h = enter_world(folder, "Logs");
    h.wait_until("the saved inventory", SERVER, |w| {
        slot(w, grid_slot(0, 0)) == Some((ItemId::OakLog, 64))
    });
    h.press(KeyCode::KeyE);
    h.wait_for(&L::marker::<InventoryRoot>(), SCREEN);

    h.key_down(KeyCode::ShiftLeft);
    h.click(&L::marker::<CraftResultCell>());
    h.key_up(KeyCode::ShiftLeft);

    let crafted = |w: &World| {
        (0..4).all(|id| slot(w, id) == Some((ItemId::OakPlanks, 64)))
            && slot(w, grid_slot(0, 0)).is_none()
            && floating(w).is_none()
    };
    h.wait_until("four stacks of planks", SERVER, crafted);
    // The server agrees: its updates keep them there
    h.frames(30);
    assert!(crafted(h.world()));
    assert_eq!(h.world().resource::<Inventory>().inner.len(), 4);
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn a_double_click_gathers_the_smallest_stacks_onto_the_cursor() {
    let planks = ItemId::OakPlanks;
    let folder = GameFolder::new().with_world("Gather").with_items(
        "Gather",
        "Tester",
        &[
            (9, planks, 10),
            (12, planks, 40),
            (20, planks, 30),
            (25, planks, 30),
        ],
    );
    let mut h = enter_world(folder, "Gather");
    h.wait_until("the saved inventory", SERVER, |w| {
        slot(w, 25) == Some((planks, 30))
    });
    h.press(KeyCode::KeyE);
    h.wait_for(&L::marker::<InventoryRoot>(), SCREEN);

    // Two clicks 40 ms apart
    h.hover(&L::cell(9));
    with_steady_clock(&mut h, |h| {
        for _ in 0..2 {
            h.button_down(MouseButton::Left);
            h.button_up(MouseButton::Left);
        }
    });

    let gathered = |w: &World| {
        floating(w) == Some((planks, 64))
            && slot(w, 9).is_none()
            && slot(w, 20).is_none()
            && slot(w, 25) == Some((planks, 6))
            && slot(w, 12) == Some((planks, 40))
    };
    h.wait_until("a full stack held", SERVER, gathered);
    h.frames(30);
    assert!(gathered(h.world()));
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn a_number_key_over_the_result_crafts_into_that_hotbar_slot() {
    let folder = GameFolder::new().with_world("Hotkey").with_items(
        "Hotkey",
        "Tester",
        &[(grid_slot(0, 0), ItemId::OakLog, 3)],
    );
    let mut h = enter_world(folder, "Hotkey");
    h.wait_until("the saved inventory", SERVER, |w| {
        slot(w, grid_slot(0, 0)) == Some((ItemId::OakLog, 3))
    });
    h.press(KeyCode::KeyE);
    h.wait_for(&L::marker::<InventoryRoot>(), SCREEN);

    h.hover(&L::marker::<CraftResultCell>());
    h.press(KeyCode::Digit3);

    let crafted = |w: &World| {
        slot(w, 2) == Some((ItemId::OakPlanks, 4))
            && slot(w, grid_slot(0, 0)) == Some((ItemId::OakLog, 2))
            && floating(w).is_none()
    };
    h.wait_until("the planks in the third hotbar slot", SERVER, crafted);
    h.frames(30);
    assert!(crafted(h.world()));
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn a_shift_double_click_moves_every_stack_of_its_item_into_the_chest() {
    let folder = GameFolder::new().with_world("Hoard").with_items(
        "Hoard",
        "Tester",
        &[
            (0, ItemId::Chest, 1),
            (3, ItemId::Dirt, 2),
            (9, ItemId::Dirt, 5),
            (10, ItemId::Sand, 4),
            (20, ItemId::Dirt, 7),
        ],
    );
    let mut h = enter_world(folder, "Hoard");
    h.wait_until("the saved inventory", SERVER, |w| {
        slot(w, 20) == Some((ItemId::Dirt, 7))
    });
    let chest = place_ahead(&mut h);
    hold_click(&mut h, &L::marker::<Hotbar>(), MouseButton::Right, 1);
    h.wait_for(&L::text(CHEST_TITLE), SCREEN);
    let contents = |w: &World| {
        w.resource::<ClientWorldMap>()
            .blocks
            .container(chest)
            .cloned()
    };
    h.wait_until("its empty slots", SERVER, |w| {
        contents(w).is_some_and(|contents| contents.is_empty())
    });

    // Shift held, two clicks 40 ms apart on the dirt of slot 9
    h.key_down(KeyCode::ShiftLeft);
    h.hover(&L::cell(9));
    with_steady_clock(&mut h, |h| {
        for _ in 0..2 {
            h.button_down(MouseButton::Left);
            h.button_up(MouseButton::Left);
        }
    });
    h.key_up(KeyCode::ShiftLeft);

    // The 14 dirt in the chest, the sand left alone
    let moved = |w: &World| {
        let in_chest: Vec<_> = contents(w)
            .map(|contents| contents.slots.iter().flatten().copied().collect())
            .unwrap_or_default();
        let dirt: u32 = in_chest
            .iter()
            .filter(|stack| stack.item_id == ItemId::Dirt)
            .map(|stack| stack.nb)
            .sum();
        dirt == 14
            && in_chest.iter().all(|stack| stack.item_id == ItemId::Dirt)
            && [3, 9, 20].iter().all(|&id| slot(w, id).is_none())
            && slot(w, 10) == Some((ItemId::Sand, 4))
    };
    h.wait_until("every stack of dirt in the chest", SERVER, moved);
    h.frames(30);
    assert!(moved(h.world()));
}
