//! Throwing items out of the inventory, through the integrated server: Q in the game and over a
//! slot of the open inventory, and a click outside the panel with a held stack.

use std::time::{Duration, Instant};

use bevy::{input::keyboard::KeyCode, prelude::*};
use shared::world::ItemId;

use super::game::{enter_world, floating, slot};
use super::harness::{Harness, SCREEN, SERVER};
use super::locator::Locator as L;
use super::seed::GameFolder;
use crate::camera::CameraController;
use crate::dropped_items::DroppedItemRoot;
use crate::player::CurrentPlayerMarker;
use crate::ui::hud::InventoryRoot;

/// Where the items lying in the world are, as the client shows them.
fn lying(w: &World) -> Vec<Vec3> {
    let Some(mut items) = w.try_query::<(&DroppedItemRoot, &Transform)>() else {
        return Vec::new();
    };
    items.iter(w).map(|(_, at)| at.translation).collect()
}

/// Presses `key` with Ctrl held.
fn with_ctrl(h: &mut Harness, key: KeyCode) {
    h.key_down(KeyCode::ControlLeft);
    h.press(key);
    h.key_up(KeyCode::ControlLeft);
}

/// Lets `duration` go by, the frames running.
fn wait(h: &mut Harness, duration: Duration) {
    let start = Instant::now();
    h.wait_until("the time to go by", duration * 2, |_| {
        start.elapsed() >= duration
    });
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn q_in_game_throws_an_item_ahead_that_is_not_picked_up_at_once() {
    let folder = GameFolder::new().with_world("Throw").with_items(
        "Throw",
        "Tester",
        &[(0, ItemId::Dirt, 10)],
    );
    let mut h = enter_world(folder, "Throw");
    h.wait_until("the saved inventory", SERVER, |w| {
        slot(w, 0) == Some((ItemId::Dirt, 10))
    });
    // Settled on the ground
    h.frames(60);
    let world = h.world_mut();
    let player = world
        .query_filtered::<&Transform, With<CurrentPlayerMarker>>()
        .single(world)
        .unwrap()
        .translation;
    let ahead = world
        .query_filtered::<&Transform, With<CameraController>>()
        .single(world)
        .unwrap()
        .forward()
        .with_y(0.0)
        .normalize();

    // One item flies ahead, out of reach, and stays there
    h.press(KeyCode::KeyQ);
    h.wait_until("the thrown dirt at rest ahead", SERVER, |w| {
        slot(w, 0) == Some((ItemId::Dirt, 9))
            && lying(w).iter().any(|&at| {
                let offset = (at - player).with_y(0.0);
                offset.dot(ahead) > 1.5 && offset.reject_from(ahead).length() < 0.5
            })
    });
    wait(&mut h, Duration::from_secs(1));
    assert_eq!(slot(h.world(), 0), Some((ItemId::Dirt, 9)));
    assert_eq!(lying(h.world()).len(), 1);

    // Ctrl+Q throws the rest of the stack
    with_ctrl(&mut h, KeyCode::KeyQ);
    h.wait_until("the slot emptied", SERVER, |w| slot(w, 0).is_none());
    wait(&mut h, Duration::from_secs(1));
    assert_eq!(slot(h.world(), 0), None);
    assert!(!lying(h.world()).is_empty());
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn q_over_a_slot_of_the_open_inventory_throws_from_it() {
    let folder =
        GameFolder::new()
            .with_world("Slot")
            .with_items("Slot", "Tester", &[(10, ItemId::Dirt, 5)]);
    let mut h = enter_world(folder, "Slot");
    h.wait_until("the saved inventory", SERVER, |w| {
        slot(w, 10) == Some((ItemId::Dirt, 5))
    });
    h.press(KeyCode::KeyE);
    h.wait_for(&L::marker::<InventoryRoot>(), SCREEN);

    h.hover(&L::cell(10));
    h.press(KeyCode::KeyQ);
    h.wait_until("one dirt thrown", SERVER, |w| {
        slot(w, 10) == Some((ItemId::Dirt, 4)) && lying(w).len() == 1
    });
    with_ctrl(&mut h, KeyCode::KeyQ);
    h.wait_until("the stack thrown", SERVER, |w| slot(w, 10).is_none());
    assert!(h.is_visible(&L::marker::<InventoryRoot>()));
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn a_click_outside_the_panel_throws_the_held_stack() {
    let folder = GameFolder::new().with_world("Outside").with_items(
        "Outside",
        "Tester",
        &[(10, ItemId::Dirt, 5)],
    );
    let mut h = enter_world(folder, "Outside");
    h.wait_until("the saved inventory", SERVER, |w| {
        slot(w, 10) == Some((ItemId::Dirt, 5))
    });
    h.press(KeyCode::KeyE);
    h.wait_for(&L::marker::<InventoryRoot>(), SCREEN);
    h.click(&L::cell(10));
    h.wait_until("the dirt held", SERVER, |w| {
        floating(w) == Some((ItemId::Dirt, 5))
    });

    // Top left, beside neither the panel nor the item browser: a right click throws one
    let outside = Vec2::new(10.0, 10.0);
    h.move_cursor(outside);
    h.frames(1);
    h.button_down(MouseButton::Right);
    h.button_up(MouseButton::Right);
    h.wait_until("one dirt thrown", SERVER, |w| {
        floating(w) == Some((ItemId::Dirt, 4)) && lying(w).len() == 1
    });

    // A left click throws the rest; the inventory stays open
    h.click_at(outside);
    h.wait_until("the held stack thrown", SERVER, |w| {
        floating(w).is_none() && !lying(w).is_empty()
    });
    h.frames(10);
    assert_eq!(slot(h.world(), 10), None);
    assert!(h.is_visible(&L::marker::<InventoryRoot>()));
}
