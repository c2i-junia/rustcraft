//! Pictures of a dungeon of each kind, for a human to look at.

use std::time::{Duration, Instant};

use bevy::ecs::system::RunSystemOnce;
use bevy::prelude::*;
use shared::{DAY_DURATION_IN_TICKS, NOON_IN_DAY};

use super::game::enter_world;
use super::seed::GameFolder;
use super::shaders::looking;
use crate::camera::{underwater_fog_system, CameraController};
use crate::constants::SAVE_PATH;
use crate::player::PlayerBody;
use crate::world::ClientWorldMap;

const SEED: u32 = 42;
/// The doors of three dungeons of the world of `SEED`, near one another, and the way out of
/// each, as `/locate` finds them.
const RUIN: (Vec3, Vec3) = (Vec3::new(-478., 67., 158.), Vec3::Z);
const CRYPT: (Vec3, Vec3) = (Vec3::new(-428., 21., 187.), Vec3::NEG_X);
const SANCTUARY: (Vec3, Vec3) = (Vec3::new(-457., 11., 178.), Vec3::X);

/// The ruin from two sides, and the crypt and the sanctuary from their doors, saved with
/// `UI_SNAPSHOTS=1`.
#[test]
#[ignore = "pictures for a human: UI_SNAPSHOTS=1 cargo test -p client e2e::dungeons -- --ignored"]
fn snapshot_the_dungeons() {
    let noon = 10 * DAY_DURATION_IN_TICKS + NOON_IN_DAY;
    let folder = GameFolder::new()
        .with_file("video.ron", "(render_distance: 8)")
        .with_seeded_world("Dungeons", SEED, noon, Duration::ZERO)
        .with_player("Dungeons", "Tester", |player| {
            player.position = Vec3::new(-457., 85., 178.);
            player.is_flying = true;
        });
    let meta = format!(
        "(format_version: 5, name: \"Dungeons\", seed: ({SEED}), time: {noon}, mobs: {{}}, \
         generation: (preset: Default, villages: true, dungeons: true))"
    );
    let world = folder.path().join(SAVE_PATH).join("Dungeons");
    std::fs::write(world.join("world.ron"), meta).unwrap();
    let mut h = enter_world(folder, "Dungeons");
    let world = h.world_mut();
    let camera = world
        .query_filtered::<Entity, With<CameraController>>()
        .single(world)
        .unwrap();
    world.entity_mut(camera).remove::<CameraController>();
    // Drawn once the chunks held did not change for three seconds
    let mut last = (usize::MAX, Instant::now());
    while last.1.elapsed().as_secs_f64() < 3. {
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
    h.frames(60);
    let world = h.world_mut();
    let mut nodes = world.query_filtered::<&mut Node, Without<ChildOf>>();
    for mut node in nodes.iter_mut(world) {
        node.display = Display::None;
    }
    let mut bodies = world.query_filtered::<&mut Visibility, With<PlayerBody>>();
    for mut body in bodies.iter_mut(world) {
        *body = Visibility::Hidden;
    }
    h.hide_the_view_model();

    let inside = |(door, out): (Vec3, Vec3)| {
        let eye = door + Vec3::new(0.5, 2.7, 0.5) - out;
        (eye, eye - out * 10. - Vec3::Y * 1.5)
    };
    let middle = RUIN.0 + Vec3::new(0.5, 1., 0.5) - RUIN.1 * 7.;
    for (name, (from, to)) in [
        ("ruin", (middle + Vec3::new(13., 11., 17.), middle)),
        ("ruin_inside", (middle + Vec3::new(0., 6., 9.), middle)),
        ("crypt", inside(CRYPT)),
        ("sanctuary", inside(SANCTUARY)),
    ] {
        let rotation = looking(from, to);
        let world = h.world_mut();
        *world.get_mut::<Transform>(camera).unwrap() =
            Transform::from_translation(from).with_rotation(rotation);
        world.entity_mut(camera).insert(CameraController::default());
        world.run_system_once(underwater_fog_system).unwrap();
        world.entity_mut(camera).remove::<CameraController>();
        h.frames(10);
        h.snapshot(name);
    }
}
