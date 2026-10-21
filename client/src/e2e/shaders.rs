//! Pictures of a varied landscape at several hours, to compare how the world is shaded: a lake
//! between a forest and hills in the world of seed 1, seen from above and from the shore, at
//! noon, in the golden hour, at sunset and at midnight, and from under the water. Saved with
//! `UI_SNAPSHOTS=1` as `<SHADER_TAG>_<scene>`, the tag `baseline` by default. A tag that names a
//! quality of the shaders, such as `high`, draws with it; any other without them.

use std::time::{Duration, Instant};

use bevy::ecs::system::RunSystemOnce;
use bevy::prelude::*;
use shared::players::Player;
use shared::world::BlockId;
use shared::DAY_DURATION_IN_TICKS;

use super::game::enter_world_with;
use super::harness::Options;
use super::seed::GameFolder;
use crate::camera::{underwater_fog_system, CameraController};
use crate::constants::SAVE_PATH;
use crate::network::hash_string_to_u64;
use crate::player::PlayerBody;
use crate::world::clouds::Clouds;
use crate::world::rendering::shaders::{Antialiasing, ShaderFeatures};
use crate::world::time::ClientTime;
use crate::world::{ClientWorldMap, GraphicsSettings, ShaderQuality};
use bevy::anti_alias::taa::TemporalAntiAliasing;
use bevy::camera::Hdr;
use bevy::light::{Atmosphere, VolumetricFog};

const SEED: u32 = 1;
/// The middle of the lake, forest and hills that `scout` found in the world of `SEED`.
pub(super) const SITE: Vec3 = Vec3::new(-64., 110., -176.);
const PLAYER: &str = "Tester";

/// A world of `SEED` whose player `PLAYER` flies at `SITE`.
pub(super) fn folder() -> GameFolder {
    let folder = GameFolder::new()
        .with_file("video.ron", "(render_distance: 16)")
        .with_seeded_world("Shaders", SEED, 0, Duration::ZERO);
    let id = hash_string_to_u64(PLAYER);
    let mut player = Player::new(id, PLAYER.to_string(), SITE, Transform::default());
    player.is_flying = true;
    let players = folder
        .path()
        .join(SAVE_PATH)
        .join("Shaders")
        .join("players");
    std::fs::create_dir_all(&players).unwrap();
    std::fs::write(
        players.join(format!("{id}.ron")),
        ron::ser::to_string(&player).unwrap(),
    )
    .unwrap();
    folder
}

/// The tick at `fraction` of the tenth day: noon is at 0.75, the sun sets at 1.0.
pub(super) fn hour(fraction: f64) -> u64 {
    10 * DAY_DURATION_IN_TICKS + (DAY_DURATION_IN_TICKS as f64 * fraction) as u64
}

/// The height of the highest block of the column `(x, z)` that is not air, and that block.
pub(super) fn surface(map: &ClientWorldMap, x: i32, z: i32) -> Option<(i32, BlockId)> {
    (0..200)
        .rev()
        .find_map(|y| map.blocks.get_block(IVec3::new(x, y, z)).map(|b| (y, b.id)))
}

/// The rotation of a camera at `from` looking at `to`.
pub(super) fn looking(from: Vec3, to: Vec3) -> Quat {
    Transform::from_translation(from)
        .looking_at(to, Vec3::Y)
        .rotation
}

#[test]
#[ignore = "pictures for a human: BENCH_GPU=1 UI_SNAPSHOTS=1 cargo test -p client --release e2e::shaders -- --ignored"]
fn snapshot_the_shaders() {
    let tag = std::env::var("SHADER_TAG").unwrap_or_else(|_| "baseline".to_string());
    let quality = ShaderQuality::ALL
        .into_iter()
        .find(|quality| format!("{quality:?}").to_lowercase() == tag)
        .unwrap_or(ShaderQuality::Off);
    let mut h = enter_world_with(
        folder(),
        "Shaders",
        Options {
            width: 1600,
            height: 900,
            shaders: Some(quality),
            ..Options::default()
        },
    );
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

    // The UI and the body of the player are out of the pictures
    let world = h.world_mut();
    let mut nodes = world.query_filtered::<&mut Node, Without<ChildOf>>();
    for mut node in nodes.iter_mut(world) {
        node.display = Display::None;
    }
    let bodies: Vec<Entity> = world
        .query_filtered::<Entity, With<PlayerBody>>()
        .iter(world)
        .collect();
    for body in bodies {
        world.entity_mut(body).insert(Visibility::Hidden);
    }

    // The nearest water to the site, a shore point beside it, and the ground under the site
    let map = h.world().resource::<ClientWorldMap>();
    let (cx, cz) = (SITE.x as i32, SITE.z as i32);
    let columns = (-60..=60).flat_map(|dz| (-60..=60).map(move |dx| (cx + dx, cz + dz)));
    let water = columns
        .clone()
        .filter(|&(x, z)| surface(map, x, z).is_some_and(|(_, b)| b == BlockId::Water))
        .min_by_key(|&(x, z)| (x - cx).pow(2) + (z - cz).pow(2))
        .expect("water near the site");
    let water_y = surface(map, water.0, water.1).unwrap().0 as f32;
    let water_at = Vec3::new(water.0 as f32, water_y, water.1 as f32);
    // A dry column 12 to 20 blocks from that water, the nearest to the site
    let shore = columns
        .filter(|&(x, z)| {
            let d = (x - water.0).pow(2) + (z - water.1).pow(2);
            (144..=400).contains(&d)
                && surface(map, x, z)
                    .is_some_and(|(y, b)| b != BlockId::Water && y as f32 <= water_y + 4.)
        })
        .min_by_key(|&(x, z)| (x - cx).pow(2) + (z - cz).pow(2))
        .expect("a shore near the water");
    let shore_y = surface(map, shore.0, shore.1).unwrap().0 as f32;
    let eye = Vec3::new(shore.0 as f32 + 0.5, shore_y + 2.7, shore.1 as f32 + 0.5);
    let beyond = water_at + (water_at - eye).with_y(0.).normalize() * 40.;
    let above = water_at + (water_at - eye).with_y(0.).normalize() * -60. + Vec3::Y * 45.;
    let under = water_at + Vec3::new(0.5, -1.6, 0.5);
    let west = above + Vec3::new(0., -30., 100.);
    // Up the path of the sun at noon and of the moon at midnight, which lean toward -X, and
    // toward where the sun sets
    let high_sky = above + Vec3::new(-100., 90., 0.);
    let setting = above + Vec3::new(0., 8., 100.);

    let scenes = [
        ("vista_noon", above, beyond, hour(0.75)),
        ("vista_morning", above, beyond, hour(0.58)),
        // The sun rises and sets toward +Z, also with the path of the sun tilted
        ("vista_golden", above, west, hour(0.97)),
        ("vista_sunset", above, west, hour(0.99)),
        ("vista_midnight", above, beyond, hour(0.25)),
        ("sky_noon", above, high_sky, hour(0.75)),
        ("sky_sunset", above, setting, hour(0.985)),
        ("sky_dusk", above, setting, hour(0.01)),
        ("sky_midnight", above, high_sky, hour(0.25)),
        ("shore_noon", eye, beyond + Vec3::Y * 4., hour(0.75)),
        ("shore_golden", eye, beyond + Vec3::Y * 4., hour(0.97)),
        (
            "underwater_noon",
            under,
            under + (beyond - eye).with_y(0.) + Vec3::Y * 3.,
            hour(0.75),
        ),
    ];
    for (name, from, to, tick) in scenes {
        let world = h.world_mut();
        world.resource_mut::<ClientTime>().0 = tick;
        *world.get_mut::<Transform>(camera).unwrap() =
            Transform::from_translation(from).with_rotation(looking(from, to));
        // The sky of the hour, which the fog system only gives to the camera of the player
        world.entity_mut(camera).insert(CameraController::default());
        world.run_system_once(underwater_fog_system).unwrap();
        world.entity_mut(camera).remove::<CameraController>();
        // The sky itself, without the clouds that would hide it
        let clouds: Vec<Entity> = world
            .query_filtered::<Entity, With<Clouds>>()
            .iter(world)
            .collect();
        let shown = if name.starts_with("sky_") {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        };
        for cloud in clouds {
            world.entity_mut(cloud).insert(shown);
        }
        // Long enough for effects that accumulate over frames to settle
        h.frames(10);
        h.snapshot(&format!("{tag}_{name}"));
    }
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn the_shaders_change_in_game_as_soon_as_they_are_chosen() {
    let mut h = super::game::enter_world(GameFolder::new().with_world("Shaders"), "Shaders");
    let camera_has = |h: &mut super::harness::Harness| {
        let world = h.world_mut();
        let camera = world
            .query_filtered::<Entity, With<CameraController>>()
            .single(world)
            .unwrap();
        let camera = world.entity(camera);
        (
            camera.contains::<Hdr>(),
            camera.contains::<TemporalAntiAliasing>(),
            camera.contains::<VolumetricFog>(),
        )
    };
    assert_eq!(camera_has(&mut h), (false, false, false));
    for quality in ShaderQuality::ALL.into_iter().chain([ShaderQuality::Off]) {
        h.world_mut().resource_mut::<GraphicsSettings>().shaders = quality;
        h.frames(5);
        let features = ShaderFeatures::of(quality);
        assert_eq!(
            camera_has(&mut h),
            (
                features.sky,
                features.antialiasing == Antialiasing::Taa,
                features.god_rays.is_some()
            ),
            "{quality:?}"
        );
        let atmospheres = h
            .world_mut()
            .query_filtered::<(), With<Atmosphere>>()
            .iter(h.world())
            .count();
        assert_eq!(atmospheres, features.sky as usize, "{quality:?}");
    }
}
