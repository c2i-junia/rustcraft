//! The animals: those that live in a new world, and those a spawn egg hatches.

use bevy::input::{mouse::MouseButtonInput, ButtonState};
use bevy::prelude::*;
use bevy::window::{PrimaryWindow, WindowEvent};
use shared::world::{ItemId, MobKind};

use super::game::{aimed_cells, enter_world, slot};
use super::harness::{Harness, SERVER};
use super::locator::Locator as L;
use super::seed::GameFolder;
use crate::camera::CameraController;
use crate::mob::Mob;
use crate::player::CurrentPlayerMarker;
use crate::ui::hud::hotbar::Hotbar;

/// The animals drawn, with where they are.
fn animals(w: &World) -> Vec<(MobKind, Vec3)> {
    let Some(mut mobs) = w.try_query::<(&Mob, &Transform)>() else {
        return Vec::new();
    };
    mobs.iter(w)
        .map(|(mob, transform)| (mob.kind, transform.translation))
        .collect()
}

fn player_position(w: &mut World) -> Vec3 {
    w.query_filtered::<&Transform, With<CurrentPlayerMarker>>()
        .single(w)
        .unwrap()
        .translation
}

/// Turns the camera of the player toward `target`.
fn look_at(h: &mut Harness, target: Vec3) {
    let eye = player_position(h.world_mut());
    let direction = (target - eye).normalize();
    let world = h.world_mut();
    let mut controller = world
        .query::<&mut CameraController>()
        .single_mut(world)
        .unwrap();
    controller.angle_x = (-direction.x).atan2(-direction.z);
    controller.angle_y = (-direction.y).atan2(direction.xz().length());
}

/// Clicks the right button over the hotbar, which the world reads as a use of the held item.
fn right_click(h: &mut Harness) {
    let center = h.center_of(&L::marker::<Hotbar>());
    h.move_cursor(center);
    h.frames(1);
    let window = h
        .world_mut()
        .query_filtered::<Entity, With<PrimaryWindow>>()
        .single(h.world())
        .unwrap();
    for (state, held) in [(ButtonState::Pressed, 3), (ButtonState::Released, 1)] {
        let input = MouseButtonInput {
            button: MouseButton::Right,
            state,
            window,
        };
        h.world_mut().write_message(input);
        h.world_mut()
            .write_message(WindowEvent::MouseButtonInput(input));
        h.frames(held);
    }
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn animals_live_in_a_new_world_and_stand_on_the_ground() {
    let folder = GameFolder::new().with_world("Meadow");
    let mut h = enter_world(folder, "Meadow");
    // Standing: the bottom of its hitbox on the top of a block. The animals in sight may walk
    // out of reach, so whichever stands is looked at.
    let standing = |w: &World| {
        animals(w).into_iter().find(|&(kind, at)| {
            let feet = at.y - kind.size().y / 2.0;
            (feet - feet.round()).abs() < 0.05
        })
    };
    h.wait_until("an animal standing on the ground", SERVER, |w| {
        standing(w).is_some()
    });
    let (_, animal) = standing(h.world()).unwrap();

    look_at(&mut h, animal);
    h.frames(10);
    h.snapshot("standing_animal");
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn a_spawn_egg_used_on_the_ground_hatches_its_animal_there() {
    let folder = GameFolder::new().with_world("Eggs").with_items(
        "Eggs",
        "Tester",
        &[(0, ItemId::SheepSpawnEgg, 1)],
    );
    let mut h = enter_world(folder, "Eggs");
    h.wait_until("the saved inventory", SERVER, |w| {
        slot(w, 0) == Some((ItemId::SheepSpawnEgg, 1))
    });
    // Look down, at the ground ahead of the player, once it is within reach
    let world = h.world_mut();
    let camera = world
        .query_filtered::<Entity, With<CameraController>>()
        .single(world)
        .unwrap();
    let player_entity = world
        .query_filtered::<Entity, With<CurrentPlayerMarker>>()
        .single(world)
        .unwrap();
    world.get_mut::<CameraController>(camera).unwrap().angle_y = 0.9;
    h.wait_until("the ground within reach", SERVER, |w| {
        aimed_cells(w, camera, player_entity).is_some_and(|(target, _)| {
            let centre = w.get::<Transform>(player_entity).unwrap().translation;
            target.as_vec3().distance(centre) < 3.
        })
    });
    // Settled on the ground
    h.frames(30);
    let player = player_position(h.world_mut());

    right_click(&mut h);
    h.wait_until("the hatched sheep", SERVER, |w| {
        animals(w)
            .iter()
            .any(|&(kind, at)| kind == MobKind::Sheep && at.distance(player) < 5.0)
    });
    assert_eq!(slot(h.world(), 0), None);
    h.frames(60);
    h.snapshot("hatched_sheep");
}
