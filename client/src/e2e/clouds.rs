//! Pictures of the clouds from below, from inside and from above, for a human to look at.

use std::time::Instant;

use bevy::ecs::system::RunSystemOnce;
use bevy::prelude::*;
use shared::{DAY_DURATION_IN_TICKS, MIDNIGHT_IN_DAY, NOON_IN_DAY};

use super::game::enter_world;
use super::seed::GameFolder;
use crate::camera::{underwater_fog_system, CameraController};
use crate::player::CurrentPlayerMarker;
use crate::world::clouds::field::{drift, is_full};
use crate::world::clouds::{camera_cell, ALTITUDE, CELL, THICKNESS};
use crate::world::time::ClientTime;
use crate::world::ClientWorldMap;

/// The camera of a new world at the default render distance, around the player: at noon on the
/// ground looking up, in the middle of the layer of clouds looking ahead (between the clouds
/// and in one), above it looking down, from far and from near the edge of a cloud, then above
/// it again two days later, and on the ground and above the clouds at sunset and at midnight.
/// Saved with `UI_SNAPSHOTS=1`, as `clouds_*`.
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

    let ground = Vec3::new(player.x, player.y + 1.6, player.z);
    let middle = ALTITUDE as f32 + THICKNESS as f32 / 2.;
    let between = Vec3::new(player.x, middle, player.z);
    let above = Vec3::new(player.x, ALTITUDE as f32 + 48., player.z);
    // Whole days later, the hour is the same and the clouds have drifted
    let noon = 10 * DAY_DURATION_IN_TICKS + NOON_IN_DAY;
    let two_days_later = noon + 2 * DAY_DURATION_IN_TICKS;
    // The middle of the nearest full cell with an empty one west of it, at noon
    let drift = drift(noon);
    let under_the_player = camera_cell(player, drift);
    let cell = (-16..=16)
        .flat_map(|z| (-16..=16).map(move |x| under_the_player + IVec2::new(x, z)))
        .filter(|cell| is_full(*cell) && !is_full(*cell - IVec2::X))
        .min_by_key(|cell| (*cell - under_the_player).length_squared())
        .unwrap();
    let half = CELL as f32 / 2.;
    let in_a_cloud = Vec3::new(
        (cell.x * CELL) as f32 + half + drift,
        middle,
        (cell.y * CELL) as f32 + half,
    );
    // Over the west edge of that cloud, near enough for the ground to show through the fog
    let over_the_edge = Vec3::new(
        (cell.x * CELL) as f32 + drift,
        (ALTITUDE + THICKNESS) as f32 + 12.,
        (cell.y * CELL) as f32 + half,
    );
    for (name, position, pitch, tick) in [
        ("clouds_below", ground, 40_f32, noon),
        ("clouds_between", between, 5., noon),
        ("clouds_in_a_cloud", in_a_cloud, 5., noon),
        ("clouds_above", above, -30., noon),
        ("clouds_above_two_days_later", above, -30., two_days_later),
        ("clouds_over_the_ground", over_the_edge, -75., noon),
        (
            "clouds_below_sunset",
            ground,
            25.,
            10 * DAY_DURATION_IN_TICKS,
        ),
        (
            "clouds_below_midnight",
            ground,
            25.,
            noon - NOON_IN_DAY + MIDNIGHT_IN_DAY,
        ),
        (
            "clouds_above_sunset",
            above,
            -15.,
            10 * DAY_DURATION_IN_TICKS,
        ),
        (
            "clouds_above_midnight",
            above,
            -15.,
            noon - NOON_IN_DAY + MIDNIGHT_IN_DAY,
        ),
    ] {
        let world = h.world_mut();
        world.resource_mut::<ClientTime>().0 = tick;
        *world.get_mut::<Transform>(camera).unwrap() = Transform::from_translation(position)
            .with_rotation(Quat::from_rotation_y(0.6) * Quat::from_rotation_x(pitch.to_radians()));
        // The sky of the hour, which the fog system only gives to the camera of the player
        world.entity_mut(camera).insert(CameraController::default());
        world.run_system_once(underwater_fog_system).unwrap();
        world.entity_mut(camera).remove::<CameraController>();
        h.frames(5);
        h.snapshot(name);
    }
}
