//! What the player hears and sees of the inventory actions: the sounds of a pick, a place and a
//! craft, and the shake of a refused slot, through the integrated server.

use std::time::Duration;

use bevy::{input::keyboard::KeyCode, prelude::*, time::TimeUpdateStrategy};
use shared::world::{grid_slot, ItemId};

use super::game::{enter_world, floating, slot};
use super::harness::{Harness, SCREEN, SERVER};
use super::locator::Locator as L;
use super::seed::GameFolder;
use crate::ui::hud::{CraftResultCell, InventoryRoot};
use crate::ui::theme::DANGER;
use crate::ui::widgets::slot::RefusalShake;

/// The files of the sounds played since `listen`, in order.
#[derive(Resource, Default)]
struct Heard(Vec<String>);

/// Records the file of every sound the game plays from now on.
fn listen(h: &mut Harness) {
    let world = h.world_mut();
    world.init_resource::<Heard>();
    world.add_observer(
        |added: On<Add, AudioPlayer>,
         players: Query<&AudioPlayer>,
         assets: Res<AssetServer>,
         mut heard: ResMut<Heard>| {
            let path = players
                .get(added.entity)
                .ok()
                .and_then(|player| assets.get_path(player.0.id()));
            if let Some(path) = path {
                heard.0.push(path.to_string());
            }
        },
    );
}

/// How many times the sound of `file` played.
fn times_heard(w: &World, file: &str) -> usize {
    w.resource::<Heard>()
        .0
        .iter()
        .filter(|heard| *heard == file)
        .count()
}

/// The horizontal move of the result cell, and its border.
fn result_look(h: &mut Harness) -> (Val, BorderColor) {
    let result = h.get(&L::marker::<CraftResultCell>());
    let world = h.world();
    let x = world.get::<UiTransform>(result).unwrap().translation.x;
    (x, *world.get::<BorderColor>(result).unwrap())
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn a_click_on_the_result_of_an_empty_grid_shakes_it_red_with_a_thud() {
    let mut h = enter_world(GameFolder::new().with_world("Refused"), "Refused");
    h.press(KeyCode::KeyE);
    h.wait_for(&L::marker::<InventoryRoot>(), SCREEN);
    listen(&mut h);
    // A clock of 20 ms a frame: the shake lasts twelve frames, however long they take
    h.world_mut()
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
            20,
        )));

    h.click(&L::marker::<CraftResultCell>());
    let result = h.get(&L::marker::<CraftResultCell>());
    h.wait_until("the result shaking", SCREEN, |w| {
        w.get::<RefusalShake>(result).is_some()
    });
    let mut moves = Vec::new();
    while h.world().get::<RefusalShake>(result).is_some() {
        let (x, border) = result_look(&mut h);
        assert_eq!(border, BorderColor::all(DANGER));
        moves.push(x);
        h.frames(1);
    }

    assert!(moves.iter().any(|x| *x != Val::Px(0.0)), "{moves:?}");
    assert!(
        moves
            .iter()
            .all(|x| matches!(x, Val::Px(x) if x.abs() <= 3.0)),
        "{moves:?}"
    );
    let (x, border) = result_look(&mut h);
    assert_eq!(x, Val::Px(0.0));
    assert_ne!(border, BorderColor::all(DANGER));
    // Heard once, though the server's updates replay the click until it acknowledges it
    h.world_mut().insert_resource(TimeUpdateStrategy::Automatic);
    h.frames(30);
    assert_eq!(times_heard(h.world(), "sounds/ui/refused.ogg"), 1);
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn a_pick_a_place_and_a_craft_are_each_heard_once() {
    let folder = GameFolder::new().with_world("Heard").with_items(
        "Heard",
        "Tester",
        &[(9, ItemId::OakLog, 2)],
    );
    let mut h = enter_world(folder, "Heard");
    h.wait_until("the saved inventory", SERVER, |w| {
        slot(w, 9) == Some((ItemId::OakLog, 2))
    });
    h.press(KeyCode::KeyE);
    h.wait_for(&L::marker::<InventoryRoot>(), SCREEN);
    listen(&mut h);

    h.click(&L::cell(9));
    h.wait_until("the pick", SCREEN, |w| {
        times_heard(w, "sounds/ui/item_pick.ogg") == 1
    });
    h.click(&L::cell(grid_slot(0, 0)));
    h.wait_until("the place", SCREEN, |w| {
        times_heard(w, "sounds/ui/item_place.ogg") == 1
    });
    h.wait_until("the logs in the grid", SERVER, |w| {
        slot(w, grid_slot(0, 0)) == Some((ItemId::OakLog, 2)) && floating(w).is_none()
    });
    h.key_down(KeyCode::ShiftLeft);
    h.click(&L::marker::<CraftResultCell>());
    h.key_up(KeyCode::ShiftLeft);
    h.wait_until("the knock", SCREEN, |w| {
        times_heard(w, "sounds/ui/craft.ogg") == 1
    });
    h.wait_until("the planks", SERVER, |w| {
        slot(w, 0) == Some((ItemId::OakPlanks, 8))
    });

    // The server's updates replay the actions it has not acknowledged: they are not heard again
    h.frames(30);
    for file in [
        "sounds/ui/item_pick.ogg",
        "sounds/ui/item_place.ogg",
        "sounds/ui/craft.ogg",
    ] {
        assert_eq!(times_heard(h.world(), file), 1, "{file}");
    }
    assert_eq!(times_heard(h.world(), "sounds/ui/refused.ogg"), 0);
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn a_burst_of_refused_notches_shakes_its_cell_once_with_one_thud() {
    // A full hotbar: the dirt of the backpack has nowhere to go
    let mut items: Vec<_> = (0..9).map(|slot| (slot, ItemId::Stone, 64)).collect();
    items.push((9, ItemId::Dirt, 5));
    let folder = GameFolder::new()
        .with_world("Notches")
        .with_items("Notches", "Tester", &items);
    let mut h = enter_world(folder, "Notches");
    h.wait_until("the saved inventory", SERVER, |w| {
        slot(w, 9) == Some((ItemId::Dirt, 5))
    });
    h.press(KeyCode::KeyE);
    h.wait_for(&L::marker::<InventoryRoot>(), SCREEN);
    listen(&mut h);
    h.world_mut()
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
            20,
        )));
    h.hover(&L::cell(9));
    let cell = h.get(&L::cell(9));

    h.scroll(-1.);
    h.wait_until("the cell shaking", SCREEN, |w| {
        w.get::<RefusalShake>(cell).is_some()
    });
    // Three more notches a frame apart, while it shakes
    let mut shaking = 0;
    for _ in 0..3 {
        h.scroll(-1.);
        shaking += 1;
    }
    while h.world().get::<RefusalShake>(cell).is_some() {
        h.frames(1);
        shaking += 1;
    }

    // One shake of 0.24 s, twelve frames of 20 ms, not started again by the later notches
    assert!(shaking <= 13, "{shaking} frames");
    h.world_mut().insert_resource(TimeUpdateStrategy::Automatic);
    h.frames(30);
    assert_eq!(times_heard(h.world(), "sounds/ui/refused.ogg"), 1);
    assert_eq!(slot(h.world(), 9), Some((ItemId::Dirt, 5)));
}
