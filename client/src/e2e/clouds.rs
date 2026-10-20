//! Pictures of the clouds from below, from inside and from above, for a human to look at.

use std::time::Instant;

use bevy::prelude::*;
use shared::NOON_IN_DAY;

use super::game::enter_world;
use super::seed::GameFolder;
use crate::camera::CameraController;
use crate::player::CurrentPlayerMarker;
use crate::world::clouds::{ALTITUDE, THICKNESS};
use crate::world::time::ClientTime;
use crate::world::ClientWorldMap;

/// The camera of a new world, at the default render distance and at noon, over the player: on the ground looking up, in the middle
/// of the layer of clouds looking ahead, and above it looking down. Saved with
/// `UI_SNAPSHOTS=1`, as `clouds_below`, `clouds_inside` and `clouds_above`.
#[test]
#[ignore = "a picture for a human: UI_SNAPSHOTS=1 cargo test -p client e2e::clouds -- --ignored"]
fn snapshot_the_clouds() {
    let folder = GameFolder::new()
        .with_file("video.ron", "(render_distance: 12)")
        .with_world("Sky");
    let mut h = enter_world(folder, "Sky");
    let world = h.world_mut();
    let camera = world
        .query_filtered::<Entity, With<CameraController>>()
        .single(world)
        .unwrap();
    // The camera is placed by hand from now on
    world.entity_mut(camera).remove::<CameraController>();
    let player = world
        .query_filtered::<&Transform, With<CurrentPlayerMarker>>()
        .single(world)
        .unwrap()
        .translation;
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

    let middle = ALTITUDE as f32 + THICKNESS as f32 / 2.;
    for (name, height, pitch) in [
        ("clouds_below", player.y + 1.6, 40_f32),
        ("clouds_inside", middle, 5.),
        ("clouds_above", ALTITUDE as f32 + 48., -30.),
    ] {
        let world = h.world_mut();
        world.resource_mut::<ClientTime>().0 = NOON_IN_DAY;
        *world.get_mut::<Transform>(camera).unwrap() = Transform::from_xyz(
            player.x, height, player.z,
        )
        .with_rotation(Quat::from_rotation_y(0.6) * Quat::from_rotation_x(pitch.to_radians()));
        h.frames(5);
        h.snapshot(name);
    }
}
