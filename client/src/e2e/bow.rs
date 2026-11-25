//! The bow: drawn in first person and by the model, its arrow in flight, stuck and picked up.

use bevy::input::{mouse::MouseButtonInput, ButtonState};
use bevy::prelude::*;
use bevy::window::{PrimaryWindow, WindowEvent};
use shared::players::{bow::FULL_DRAW_MS, Inventory, Player};
use shared::world::{ItemId, MobAction, MobKind};

use super::game::enter_world;
use super::harness::{Harness, SERVER};
use super::locator::Locator as L;
use super::seed::GameFolder;
use crate::camera::CameraController;
use crate::mob::Mob;
use crate::player::hand::HeldItemMesh;
use crate::player::CurrentPlayerMarker;
use crate::ui::hud::hotbar::Hotbar;

fn player(w: &World) -> Player {
    let mut players = w
        .try_query_filtered::<&Player, With<CurrentPlayerMarker>>()
        .unwrap();
    players.single(w).unwrap().clone()
}

fn arrows(w: &World) -> Vec<(MobAction, Vec3)> {
    let Some(mut mobs) = w.try_query::<(&Mob, &Transform)>() else {
        return Vec::new();
    };
    mobs.iter(w)
        .filter(|(mob, _)| mob.kind == MobKind::Arrow)
        .map(|(mob, transform)| (mob.action, transform.translation))
        .collect()
}

/// The arrows in the inventory.
fn quiver(w: &World) -> u32 {
    let inventory = w.resource::<Inventory>();
    let arrows = inventory.inner.values();
    arrows
        .filter(|stack| stack.item_id == ItemId::Arrow)
        .map(|stack| stack.nb)
        .sum()
}

fn right_button(h: &mut Harness, state: ButtonState) {
    let w = h.world_mut();
    let mut windows = w.query_filtered::<Entity, With<PrimaryWindow>>();
    let input = MouseButtonInput {
        button: MouseButton::Right,
        state,
        window: windows.single(w).unwrap(),
    };
    w.write_message(input);
    w.write_message(WindowEvent::MouseButtonInput(input));
}

fn draw(h: &mut Harness) {
    let center = h.center_of(&L::marker::<Hotbar>());
    h.move_cursor(center);
    h.frames(1);
    right_button(h, ButtonState::Pressed);
    h.wait_until("the bow fully drawn", SERVER, |w| {
        player(w).bow_draw_ms == FULL_DRAW_MS
    });
    h.frames(5);
}

fn aim(h: &mut Harness, pitch: f32) {
    let w = h.world_mut();
    let mut cameras = w.query::<&mut CameraController>();
    cameras.single_mut(w).unwrap().angle_y = pitch;
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn a_bow_is_drawn_seen_drawn_and_its_arrow_flies_sticks_and_is_picked_up() {
    let folder = GameFolder::new().with_world("Archery").with_items(
        "Archery",
        "Tester",
        &[(0, ItemId::Bow, 1), (9, ItemId::Arrow, 5)],
    );
    let mut h = enter_world(folder, "Archery");
    h.wait_until("the player on the ground", SERVER, |w| player(w).on_ground);
    h.set_time("day");
    h.wait_until("the arrows", SERVER, |w| quiver(w) == 5);
    aim(&mut h, 0.0);
    h.frames(40);
    h.snapshot("at_rest");

    draw(&mut h);
    let stages = |w: &World| {
        let mut meshes = w.try_query::<&HeldItemMesh>().unwrap();
        meshes
            .iter(w)
            .map(|mesh| (mesh.0, mesh.1))
            .collect::<Vec<_>>()
    };
    assert_eq!(
        stages(h.world()),
        [(ItemId::Bow, Some(3)); 2],
        "in the hand and on the model"
    );
    h.snapshot("drawn");
    right_button(&mut h, ButtonState::Released);
    h.wait_until("an arrow in flight", SERVER, |w| {
        arrows(w)
            .iter()
            .any(|(action, _)| *action == MobAction::Walk)
    });
    h.frames(6);
    h.snapshot("in_flight");
    h.wait_until("the arrow spent", SERVER, |w| quiver(w) == 4);

    // At the ground ahead, then picked up on the way
    aim(&mut h, 0.45);
    draw(&mut h);
    let before = arrows(h.world()).len();
    right_button(&mut h, ButtonState::Released);
    h.wait_until("another arrow stuck", SERVER, |w| {
        let arrows = arrows(w);
        arrows.len() > before && arrows.iter().all(|(action, _)| *action == MobAction::Idle)
    });
    h.frames(30);
    h.snapshot("stuck");
    h.wait_until("it is spent", SERVER, |w| quiver(w) == 3);

    // The model draws it too
    h.press(KeyCode::F5);
    draw(&mut h);
    h.snapshot("third_person");
    right_button(&mut h, ButtonState::Released);

    // The stuck arrow from its side, the camera freed of the player
    let (_, at) = *arrows(h.world())
        .iter()
        .min_by(|a, b| {
            let eye = player(h.world()).position;
            a.1.distance(eye).total_cmp(&b.1.distance(eye))
        })
        .unwrap();
    let w = h.world_mut();
    let mut cameras = w.query_filtered::<Entity, With<CameraController>>();
    let camera = cameras.single(w).unwrap();
    w.entity_mut(camera)
        .remove::<CameraController>()
        .insert(Transform::from_translation(at + Vec3::new(1.2, 0.4, 0.3)).looking_at(at, Vec3::Y));
    h.frames(5);
    h.snapshot("stuck_side");
}
