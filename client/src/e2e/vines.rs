//! Pictures of the vines hanging from two giant trees of the seed 42, for a human to look at.

use std::time::Instant;

use bevy::ecs::system::RunSystemOnce;
use bevy::prelude::*;
use shared::{DAY_DURATION_IN_TICKS, NOON_IN_DAY};

use super::game::enter_world;
use super::seed::GameFolder;
use crate::camera::{underwater_fog_system, CameraController};
use crate::world::time::ClientTime;
use crate::world::ClientWorldMap;

/// The camera of a world of the seed 42 at noon, north of two giant trees whose trunks stand
/// at (-213, 59) and (-199, 59): from the ground, looking up at the vines hanging from the
/// north rims of their crowns, then close to one of them, from its open side and along it.
/// Saved with `UI_SNAPSHOTS=1`, as `vines_*`.
#[test]
#[ignore = "a picture for a human: UI_SNAPSHOTS=1 cargo test -p client e2e::vines -- --ignored"]
fn snapshot_the_vines() {
    let folder = GameFolder::new().with_world("Jungle").with_player_at(
        "Jungle",
        "Tester",
        Vec3::new(-205.5, 75., 44.5),
    );
    let mut h = enter_world(folder, "Jungle");
    let world = h.world_mut();
    let camera = world
        .query_filtered::<Entity, With<CameraController>>()
        .single(world)
        .unwrap();
    // The camera is placed by hand from now on
    world.entity_mut(camera).remove::<CameraController>();
    // Drawn once the chunks held did not change for two seconds
    let mut last = (usize::MAX, Instant::now());
    while last.1.elapsed().as_secs_f64() < 2. {
        h.frames(1);
        let held = h
            .world()
            .resource::<ClientWorldMap>()
            .blocks
            .chunk_positions()
            .count();
        if held != last.0 {
            last = (held, Instant::now());
        }
    }
    for (name, position, target) in [
        (
            "vines_under_the_crowns",
            Vec3::new(-205.5, 74., 40.5),
            Vec3::new(-205.5, 86., 56.),
        ),
        (
            "vines_close",
            Vec3::new(-208.5, 89.5, 51.),
            Vec3::new(-208.5, 88.5, 56.),
        ),
        (
            "vines_along",
            Vec3::new(-212., 89.5, 55.5),
            Vec3::new(-208.5, 88.5, 55.9),
        ),
    ] {
        let world = h.world_mut();
        world.resource_mut::<ClientTime>().0 = 10 * DAY_DURATION_IN_TICKS + NOON_IN_DAY;
        *world.get_mut::<Transform>(camera).unwrap() =
            Transform::from_translation(position).looking_at(target, Vec3::Y);
        // The sky of the hour, which the fog system only gives to the camera of the player
        world.entity_mut(camera).insert(CameraController::default());
        world.run_system_once(underwater_fog_system).unwrap();
        world.entity_mut(camera).remove::<CameraController>();
        h.frames(5);
        h.snapshot(name);
    }
}
