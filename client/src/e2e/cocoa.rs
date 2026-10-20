//! Pictures of the pods of cocoa on two jungle trees of the seed 42, for a human to look at.

use std::time::Instant;

use bevy::ecs::system::RunSystemOnce;
use bevy::prelude::*;
use shared::{DAY_DURATION_IN_TICKS, NOON_IN_DAY};

use super::game::enter_world;
use super::seed::GameFolder;
use crate::camera::{underwater_fog_system, CameraController};
use crate::world::time::ClientTime;
use crate::world::ClientWorldMap;

/// The camera of a world of the seed 42 at noon, beside two jungle trees: the one whose trunk
/// stands at (-204, 51) holds a pod on its south side and one on its east side, seen from
/// afar, close, and from the side, where the stem and the gap show, the one at (-191, 86) two
/// pods one above the other on its north side, near the ground. Saved with
/// `UI_SNAPSHOTS=1`, as `cocoa_*`.
#[test]
#[ignore = "a picture for a human: UI_SNAPSHOTS=1 cargo test -p client e2e::cocoa -- --ignored"]
fn snapshot_the_cocoa() {
    let folder = GameFolder::new().with_world("Jungle").with_player_at(
        "Jungle",
        "Tester",
        Vec3::new(-198.5, 75., 66.5),
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
            "cocoa_on_a_jungle_tree",
            Vec3::new(-199.5, 73.8, 57.5),
            Vec3::new(-203.5, 73.5, 51.5),
        ),
        (
            "cocoa_close",
            Vec3::new(-200.5, 73.6, 54.5),
            Vec3::new(-203., 73.5, 52.),
        ),
        (
            "cocoa_stem",
            Vec3::new(-205.2, 73.95, 52.9),
            Vec3::new(-203.5, 73.6, 52.25),
        ),
        (
            "cocoa_near_the_ground",
            Vec3::new(-189.2, 70.3, 82.8),
            Vec3::new(-190.5, 69.8, 85.5),
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
