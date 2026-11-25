//! Pictures of the lair, the Ember Mother, her bar, the compass and the credits, for a human to
//! look at.

use std::time::{Duration, Instant};

use bevy::ecs::system::RunSystemOnce;
use bevy::prelude::*;
use shared::players::game_mode::GameMode;
use shared::world::{lair, ItemId, ItemStack, TerrainNoises};
use shared::{DAY_DURATION_IN_TICKS, NOON_IN_DAY};

use super::game::enter_world;
use super::harness::SERVER;
use super::locator::Locator as L;
use super::seed::GameFolder;
use super::shaders::looking;
use crate::camera::{underwater_fog_system, CameraController};
use crate::constants::SAVE_PATH;
use crate::ui::credits::{Credits, CreditsDue};
use crate::ui::hud::boss::BossBarRoot;
use crate::world::ClientWorldMap;

const SEED: u32 = 42;

/// Saved with `UI_SNAPSHOTS=1`.
#[test]
#[ignore = "pictures for a human: UI_SNAPSHOTS=1 cargo test -p client e2e::lair -- --ignored"]
fn snapshot_the_lair_the_boss_and_the_credits() {
    let noon = 10 * DAY_DURATION_IN_TICKS + NOON_IN_DAY;
    let heart = lair::heart(lair::centre(SEED, &TerrainNoises::new(SEED)));
    let stand = heart + Vec3::new(0., 0.95, 18.);
    let folder = GameFolder::new()
        .with_file("video.ron", "(render_distance: 6)")
        .with_seeded_world("Lair", SEED, noon, Duration::ZERO)
        .with_player("Lair", "Tester", move |player| {
            player.position = stand;
            player.game_mode = GameMode::Survival;
            player.health = 5000;
            for (slot, item_id) in [(0, ItemId::DepthsCompass), (1, ItemId::EmberHeart)] {
                let item_type = item_id.get_default_type();
                let stack = ItemStack {
                    item_id,
                    item_type,
                    nb: 1,
                };
                player.inventory.inner.insert(slot, stack);
            }
        });
    let meta = format!(
        "(format_version: 5, name: \"Lair\", seed: ({SEED}), time: {noon}, mobs: {{}}, \
         generation: (preset: Default, villages: true, dungeons: true))"
    );
    let world = folder.path().join(SAVE_PATH).join("Lair");
    std::fs::write(world.join("world.ron"), meta).unwrap();
    let mut h = enter_world(folder, "Lair");
    let world = h.world_mut();
    let camera = world
        .query_filtered::<Entity, With<CameraController>>()
        .single(world)
        .unwrap();
    world.entity_mut(camera).remove::<CameraController>();
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
    h.wait_for(&L::marker::<BossBarRoot>(), SERVER);
    for (name, from, to) in [
        ("lair", heart + Vec3::new(17., 7., 17.), heart),
        ("shore", stand + Vec3::Y, heart + Vec3::Y),
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
    h.world_mut().resource_mut::<CreditsDue>().0 = true;
    h.wait_for(&L::marker::<Credits>(), SERVER);
    let start = Instant::now();
    while start.elapsed().as_secs() < 8 {
        h.frames(5);
    }
    h.snapshot("credits");
    h.press(KeyCode::Escape);
    h.frames(3);
    let world = h.world_mut();
    assert!(world.query::<&Credits>().iter(world).next().is_none());
}
