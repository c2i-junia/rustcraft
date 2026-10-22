//! Pictures of the weather, for a human to look at: the lake, forest and hills of
//! `e2e::shaders` from the shore at noon under a clear sky, in rain and in a thunderstorm,
//! looking across the water and looking up, then in rain at sunset and at midnight, from above,
//! with the rain halfway in, and without the shaders. Then the rain and the snow themselves:
//! the drops over the lake, the splashes at the feet, the rain seen from under a roof, at
//! midnight, without the shaders and in Ultra, and the snow plain of seed 147 while it snows.
//! Then the wet world, and the lightning: a bolt across the lake at its brightest stroke, at
//! noon and at midnight, in High, Ultra and without the shaders, a strike too far for its bolt,
//! and a bolt over the wet terrace. Saved with `UI_SNAPSHOTS=1` as `weather_*`.

use std::time::Instant;

use bevy::ecs::system::RunSystemOnce;
use bevy::prelude::*;
use shared::world::{BlockId, WeatherKind};

use super::game::enter_world_with;
use super::harness::{Harness, Options};
use super::seed::GameFolder;
use super::shaders::{folder, hour, looking, surface, SITE};
use crate::camera::{underwater_fog_system, CameraController};
use crate::player::{CurrentPlayerMarker, PlayerBody};
use crate::world::lightning::{
    advance_strikes_system, bolt::Strokes, shown_reach, Lightning, LiveStrike, Strike,
};
use crate::world::time::ClientTime;
use crate::world::weather::{ease_weather_system, ClientWeather};
use crate::world::{BlockChanged, ClientWorldMap, ShaderQuality, WorldRenderRequestUpdateEvent};
use crate::world::{RenderDistance, ServerRenderDistanceLimit};
use shared::world::{BlockData, BlockDirection, BlockTransparency};

/// Where the pictures are taken from and looked at.
struct Views {
    /// On the shore, at the height of the eyes.
    eye: Vec3,
    /// Across the water, a little above the horizon.
    across: Vec3,
    /// Up into the sky, over the water.
    up: Vec3,
    /// High above the shore, looking across.
    above: Vec3,
    /// Toward where the sun sets, from above.
    west: Vec3,
    /// A patch of level ground near the shore, and an eye above it.
    feet: Vec3,
    over_feet: Vec3,
}

/// The weather the pictures are taken in, held whatever the levels would ease to.
#[derive(Resource, Default)]
struct HeldWeather(ClientWeather);

fn hold_weather(held: Res<HeldWeather>, mut weather: ResMut<ClientWeather>) {
    *weather = held.0.clone();
}

/// The strikes the pictures are taken with, held at their age whatever the clock.
#[derive(Resource, Default)]
struct HeldLightning(Vec<LiveStrike>);

fn hold_lightning(held: Res<HeldLightning>, mut lightning: ResMut<Lightning>) {
    lightning.strikes = held.0.clone();
}

/// A client with the shaders `quality` in the world `world` of `folder`, its chunks loaded, the
/// UI and the body of the player hidden and the weather held as set; and the camera, placed
/// by hand.
fn enter(folder: GameFolder, world: &str, quality: ShaderQuality) -> (Harness, Entity) {
    let mut h = enter_world_with(
        folder,
        world,
        Options {
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
    h.frames(30);

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
    // Nor the outline of the block aimed at
    world
        .resource_mut::<GizmoConfigStore>()
        .config_mut::<DefaultGizmoConfigGroup>()
        .0
        .enabled = false;
    h.app
        .init_resource::<HeldWeather>()
        .init_resource::<HeldLightning>()
        .add_systems(Update, hold_weather.after(ease_weather_system))
        .add_systems(PreUpdate, hold_lightning.after(advance_strikes_system));
    (h, camera)
}

/// A client with the shaders `quality` in the world of `e2e::shaders`, as `enter`, and the
/// views of its lake.
fn start(quality: ShaderQuality) -> (Harness, Entity, Views) {
    let (h, camera) = enter(folder(), "Shaders", quality);

    // The shore of the nearest water to the site, as in `e2e::shaders`
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
    // A patch of level ground out of the water, the nearest to the eye: the splashes are seen
    // on it from a little above, looking down
    let ground = |x: i32, z: i32| {
        (0..200).rev().find_map(|y| {
            let block = map.blocks.get_block(IVec3::new(x, y, z))?;
            (block.id.get_visibility() != BlockTransparency::Decoration).then_some((y, block.id))
        })
    };
    let level = |x: i32, z: i32| {
        let (y, block) = ground(x, z)?;
        let flat = (-1..=1).all(|dx| {
            (-1..=1).all(|dz| {
                ground(x + dx, z + dz).is_some_and(|(other, b)| {
                    other == y && b.get_visibility() == BlockTransparency::Solid
                })
            })
        });
        (flat && block != BlockId::Water).then_some(y)
    };
    let (patch, patch_y) = (-30..=30)
        .flat_map(|dz| (-30..=30).map(move |dx| (shore.0 + dx, shore.1 + dz)))
        .filter_map(|(x, z)| Some(((x, z), level(x, z)?)))
        .min_by_key(|((x, z), _)| (x - shore.0).pow(2) + (z - shore.1).pow(2))
        .expect("level ground near the shore");
    let patch = Vec3::new(
        patch.0 as f32 + 0.5,
        patch_y as f32 + 1.,
        patch.1 as f32 + 0.5,
    );
    let eye = Vec3::new(shore.0 as f32 + 0.5, shore_y + 2.7, shore.1 as f32 + 0.5);
    let ahead = (water_at - eye).with_y(0.).normalize();
    let above = water_at - ahead * 60. + Vec3::Y * 45.;
    let views = Views {
        eye,
        across: water_at + ahead * 40. + Vec3::Y * 4.,
        up: eye + ahead * 30. + Vec3::Y * 45.,
        above,
        west: above + Vec3::new(0., -30., 100.),
        feet: patch,
        over_feet: patch + Vec3::new(-3., 2.2, -1.),
    };
    (h, camera, views)
}

/// Takes the picture `weather_<name>` from `from` looking at `to`, at the world clock `tick`
/// under `weather`.
fn shoot(
    h: &mut Harness,
    camera: Entity,
    name: &str,
    (from, to): (Vec3, Vec3),
    tick: u64,
    weather: ClientWeather,
) {
    let world = h.world_mut();
    world.resource_mut::<ClientTime>().0 = tick;
    world.insert_resource(weather.clone());
    world.insert_resource(HeldWeather(weather));
    *world.get_mut::<Transform>(camera).unwrap() =
        Transform::from_translation(from).with_rotation(looking(from, to));
    // The sky of the hour, which the fog system only gives to the camera of the player
    world.entity_mut(camera).insert(CameraController::default());
    world.run_system_once(underwater_fog_system).unwrap();
    world.entity_mut(camera).remove::<CameraController>();
    // Long enough for effects that accumulate over frames to settle
    h.frames(10);
    h.snapshot(&format!("weather_{name}"));
}

#[test]
#[ignore = "pictures for a human: UI_SNAPSHOTS=1 cargo test -p client e2e::weather -- --ignored"]
fn snapshot_the_weather() {
    let (mut h, camera, v) = start(ShaderQuality::High);
    let noon = hour(0.75);
    let [clear, rain, thunder] =
        [WeatherKind::Clear, WeatherKind::Rain, WeatherKind::Thunder].map(ClientWeather::settled);
    // A ramp of 0.3264 is a rain level of 0.25: the sky half overcast
    let halfway = ClientWeather::part_way(WeatherKind::Rain, 0.3264);
    let scenes = [
        ("noon_clear", (v.eye, v.across), noon, clear.clone()),
        ("noon_rain", (v.eye, v.across), noon, rain.clone()),
        ("noon_thunder", (v.eye, v.across), noon, thunder.clone()),
        ("noon_clear_up", (v.eye, v.up), noon, clear.clone()),
        ("noon_rain_up", (v.eye, v.up), noon, rain.clone()),
        ("noon_thunder_up", (v.eye, v.up), noon, thunder),
        ("noon_halfway", (v.eye, v.across), noon, halfway),
        ("vista_noon_rain", (v.above, v.across), noon, rain.clone()),
        (
            "sunset_clear",
            (v.above, v.west),
            hour(0.985),
            clear.clone(),
        ),
        ("sunset_rain", (v.above, v.west), hour(0.985), rain.clone()),
        ("midnight_clear", (v.eye, v.across), hour(0.25), clear),
        ("midnight_rain", (v.eye, v.across), hour(0.25), rain),
    ];
    for (name, view, tick, weather) in scenes {
        shoot(&mut h, camera, name, view, tick, weather);
    }
}

#[test]
#[ignore = "pictures for a human: UI_SNAPSHOTS=1 cargo test -p client e2e::weather -- --ignored"]
fn snapshot_the_weather_without_the_shaders() {
    let (mut h, camera, v) = start(ShaderQuality::Off);
    for kind in [WeatherKind::Clear, WeatherKind::Rain, WeatherKind::Thunder] {
        let name = format!("off_noon_{}", kind.name());
        let weather = ClientWeather::settled(kind);
        shoot(
            &mut h,
            camera,
            &name,
            (v.eye, v.across),
            hour(0.75),
            weather,
        );
    }
}

/// Sets the blocks `blocks` in the world the client holds, `None` for air: meshed again and
/// told to the rain as blocks placed or broken would be.
fn place(h: &mut Harness, blocks: Vec<(IVec3, Option<BlockId>)>) {
    let world = h.world_mut();
    let mut chunks = Vec::new();
    for (position, id) in blocks {
        let block = id.map(|id| BlockData::new(id, BlockDirection::Front));
        let mut map = world.resource_mut::<ClientWorldMap>();
        let before = match block {
            Some(block) => {
                let before = map.blocks.get_block(position).copied();
                map.blocks.set_block(position, block);
                before
            }
            None => map.blocks.remove_block(position),
        };
        world.write_message(BlockChanged {
            position,
            before,
            after: block,
            by_local_player: false,
        });
        chunks.push(position.div_euclid(IVec3::splat(shared::CHUNK_SIZE)));
    }
    chunks.sort_by_key(|chunk| chunk.to_array());
    chunks.dedup();
    for chunk in chunks {
        world.write_message(WorldRenderRequestUpdateEvent::ChunkToReload(chunk));
    }
    h.frames(30);
}

/// Covers the eye at `eye` with a roof of planks on four posts, its edge two blocks ahead of
/// the eye along `ahead` (a direction along x or z), in the world the client holds: meshed and
/// told to the rain as a block placed would be.
fn build_roof(h: &mut Harness, eye: Vec3, ahead: IVec3) {
    let eye = eye.floor().as_ivec3();
    let top = eye.y + 2;
    let side = IVec3::new(ahead.z, 0, ahead.x);
    let mut blocks = Vec::new();
    for along in -5..=2 {
        for across in -3..=3 {
            blocks.push(eye + ahead * along + side * across + IVec3::Y * (top - eye.y));
        }
    }
    for (along, across) in [(-5, -3), (-5, 3), (2, -3), (2, 3)] {
        for y in eye.y - 3..top {
            blocks.push(eye + ahead * along + side * across + IVec3::Y * (y - eye.y));
        }
    }
    let world = h.world_mut();
    let mut chunks = Vec::new();
    for position in blocks {
        let block = BlockData::new(BlockId::OakPlanks, BlockDirection::Front);
        world
            .resource_mut::<ClientWorldMap>()
            .blocks
            .set_block(position, block);
        world.write_message(BlockChanged {
            position,
            before: None,
            after: Some(block),
            by_local_player: false,
        });
        chunks.push(position.div_euclid(IVec3::splat(shared::CHUNK_SIZE)));
    }
    chunks.dedup();
    for chunk in chunks {
        world.write_message(WorldRenderRequestUpdateEvent::ChunkToReload(chunk));
    }
    h.frames(30);
}

/// The axis along x or z nearest to `direction`.
fn nearest_axis(direction: Vec3) -> IVec3 {
    if direction.x.abs() > direction.z.abs() {
        IVec3::new(direction.x.signum() as i32, 0, 0)
    } else {
        IVec3::new(0, 0, direction.z.signum() as i32)
    }
}

#[test]
#[ignore = "pictures for a human: UI_SNAPSHOTS=1 cargo test -p client e2e::weather -- --ignored"]
fn snapshot_the_rain() {
    let (mut h, camera, v) = start(ShaderQuality::High);
    let noon = hour(0.75);
    let [rain, thunder] = [WeatherKind::Rain, WeatherKind::Thunder].map(ClientWeather::settled);
    let scenes = [
        ("rain_lake", (v.eye, v.across), noon, rain.clone()),
        ("rain_thunder", (v.eye, v.across), noon, thunder.clone()),
        ("rain_splashes", (v.over_feet, v.feet), noon, rain.clone()),
        (
            "rain_ripples",
            (
                v.eye,
                v.eye + (v.across - v.eye).normalize() * 4. - Vec3::Y * 3.,
            ),
            noon,
            rain.clone(),
        ),
        ("rain_vista", (v.above, v.across), noon, rain.clone()),
        ("rain_midnight", (v.eye, v.across), hour(0.25), rain.clone()),
    ];
    for (name, view, tick, weather) in scenes {
        shoot(&mut h, camera, name, view, tick, weather);
    }
    // Under a roof, looking out over the lake
    let ahead = nearest_axis(v.across - v.eye);
    build_roof(&mut h, v.eye, ahead);
    let out = v.eye + ahead.as_vec3() * 10. + Vec3::Y * 2.;
    shoot(&mut h, camera, "rain_roof", (v.eye, out), noon, rain);
}

#[test]
#[ignore = "pictures for a human: UI_SNAPSHOTS=1 cargo test -p client e2e::weather -- --ignored"]
fn snapshot_the_storm_in_motion() {
    let (mut h, camera, v) = start(ShaderQuality::High);
    let thunder = ClientWeather::settled(WeatherKind::Thunder);
    shoot(
        &mut h,
        camera,
        "storm_motion_0",
        (v.eye, v.across),
        hour(0.75),
        thunder,
    );
    for frame in 1..6 {
        h.frames(1);
        h.snapshot(&format!("weather_storm_motion_{frame}"));
    }
}

#[test]
#[ignore = "pictures for a human: UI_SNAPSHOTS=1 cargo test -p client e2e::weather -- --ignored"]
fn snapshot_the_rain_without_the_shaders() {
    let (mut h, camera, v) = start(ShaderQuality::Off);
    let rain = ClientWeather::settled(WeatherKind::Rain);
    shoot(
        &mut h,
        camera,
        "off_rain_lake",
        (v.eye, v.across),
        hour(0.75),
        rain.clone(),
    );
    shoot(
        &mut h,
        camera,
        "off_rain_splashes",
        (v.over_feet, v.feet),
        hour(0.75),
        rain,
    );
}

#[test]
#[ignore = "pictures for a human: UI_SNAPSHOTS=1 cargo test -p client e2e::weather -- --ignored"]
fn snapshot_the_rain_in_ultra() {
    let (mut h, camera, v) = start(ShaderQuality::Ultra);
    let thunder = ClientWeather::settled(WeatherKind::Thunder);
    shoot(
        &mut h,
        camera,
        "ultra_rain_lake",
        (v.eye, v.across),
        hour(0.75),
        thunder,
    );
}

#[test]
#[ignore = "pictures for a human: UI_SNAPSHOTS=1 cargo test -p client e2e::weather -- --ignored"]
fn snapshot_the_snow() {
    let folder = GameFolder::new()
        .with_file("video.ron", "(render_distance: 12)")
        .with_seeded_world("Snow", 147, 0, std::time::Duration::ZERO);
    let (mut h, camera) = enter(folder, "Snow", ShaderQuality::High);
    let world = h.world_mut();
    let player = world
        .query_filtered::<&Transform, With<CurrentPlayerMarker>>()
        .single(world)
        .unwrap()
        .translation;
    let eye = player + Vec3::Y * 1.6;
    let snow = ClientWeather::settled(WeatherKind::Rain);
    let ahead = eye + Vec3::new(30., -2., 12.);
    shoot(
        &mut h,
        camera,
        "snow_plain",
        (eye, ahead),
        hour(0.75),
        snow.clone(),
    );
    shoot(
        &mut h,
        camera,
        "snow_up",
        (eye, eye + Vec3::new(10., 8., 4.)),
        hour(0.75),
        snow.clone(),
    );
    shoot(&mut h, camera, "snow_dusk", (eye, ahead), hour(0.97), snow);
}

/// A terrace by the lake for the pictures of the wet world, and where it is looked at from.
struct Terrace {
    /// Standing at its back, at the height of the eyes, and what is looked at: its middle.
    eye: Vec3,
    look: Vec3,
    /// Its ground, the top of the block at its back in the middle, and the axes along it,
    /// toward the water, and across it.
    origin: IVec3,
    ahead: IVec3,
    side: IVec3,
}

/// The height of the highest block of the column `(x, z)` that is not air nor a decoration.
fn ground_height(map: &ClientWorldMap, x: i32, z: i32) -> Option<i32> {
    (0..200).rev().find(|&y| {
        map.blocks
            .get_block(IVec3::new(x, y, z))
            .is_some_and(|b| b.id.get_visibility() != BlockTransparency::Decoration)
    })
}

/// Builds a terrace on the shore of `v`, open to the sky, toward the water: a dirt path between
/// grass, a plaza of cobblestone around stone, a low stone wall and a patch of sand, and a pier
/// of planks a step up running out over the water.
fn build_terrace(h: &mut Harness, v: &Views) -> Terrace {
    let ahead = nearest_axis(v.across - v.eye);
    let side = IVec3::new(ahead.z, 0, ahead.x);
    let shore = v.eye.floor().as_ivec3();
    let map = h.world().resource::<ClientWorldMap>();
    let g = ground_height(map, shore.x, shore.z).expect("the ground of the shore");
    let origin = IVec3::new(shore.x, g, shore.z) - ahead * 3;
    let mut blocks = Vec::new();
    for along in -4..=18 {
        for across in -6..=6 {
            let column = origin + ahead * along + side * across;
            let pier = along >= 11 && across.abs() <= 2;
            let ground = match (along, across.abs()) {
                (_, 5..) => Some(BlockId::Grass),
                (..=3, 0..=1) => Some(BlockId::Dirt),
                (..=3, _) => Some(BlockId::Grass),
                (4..=10, 0..=1) if (5..=8).contains(&along) => Some(BlockId::Stone),
                (4..=10, _) if across >= 3 && (5..=7).contains(&along) => Some(BlockId::Sand),
                (4..=10, _) => Some(BlockId::Cobblestone),
                _ if pier => Some(BlockId::Cobblestone),
                _ => None,
            };
            let Some(ground) = ground else {
                continue;
            };
            for y in 1..=12 {
                blocks.push((column + IVec3::Y * y, None));
            }
            blocks.push((column, Some(ground)));
            for y in 1..=3 {
                blocks.push((column - IVec3::Y * y, Some(BlockId::Dirt)));
            }
            if pier {
                blocks.push((column + IVec3::Y, Some(BlockId::OakPlanks)));
            }
            if across == -3 && (6..=9).contains(&along) {
                blocks.push((column + IVec3::Y, Some(BlockId::Stone)));
            }
            if across.abs() >= 5 && (along + across).rem_euclid(3) == 0 {
                blocks.push((column + IVec3::Y, Some(BlockId::TallGrass)));
            }
        }
    }
    place(h, blocks);
    let top = origin.as_vec3() + Vec3::new(0.5, 1., 0.5);
    Terrace {
        eye: top + Vec3::Y * 1.62,
        look: top + ahead.as_vec3() * 10.,
        origin,
        ahead,
        side,
    }
}

/// `kind`, settled, with the world as wet as `wetness`.
fn wet(kind: WeatherKind, wetness: f32) -> ClientWeather {
    let mut weather = ClientWeather::settled(kind);
    weather.wetness = wetness;
    weather
}

#[test]
#[ignore = "pictures for a human: UI_SNAPSHOTS=1 cargo test -p client e2e::weather -- --ignored"]
fn snapshot_the_wet_world() {
    let (mut h, camera, v) = start(ShaderQuality::High);
    let t = build_terrace(&mut h, &v);
    let noon = hour(0.75);
    let sunset = hour(0.985);
    let ahead = t.ahead.as_vec3();
    let pier = t.origin.as_vec3() + ahead * 11. + Vec3::new(0.5, 2., 0.5);
    // Close, looking down at the middle of the plaza
    let plaza = t.origin.as_vec3() + Vec3::new(0.5, 1., 0.5);
    let close = (
        plaza + ahead * 3. + Vec3::Y * 1.3,
        plaza + ahead * 7. - t.side.as_vec3() * 0.5,
    );
    // From above the back of the terrace, over all of it
    let above = (t.eye - ahead * 4. + Vec3::Y * 6., t.look + ahead * 4.);
    // From the end of the pier toward where the sun sets
    let end = pier + ahead * 6. + Vec3::Y * 1.6;
    let west = (end, end + Vec3::new(0., -2.5, 12.));
    let scenes = [
        ("dry", (t.eye, t.look), noon, wet(WeatherKind::Clear, 0.)),
        ("rain", (t.eye, t.look), noon, wet(WeatherKind::Rain, 1.)),
        ("rain_above", above, noon, wet(WeatherKind::Rain, 1.)),
        ("dry_above", above, noon, wet(WeatherKind::Clear, 0.)),
        ("puddle", close, noon, wet(WeatherKind::Rain, 1.)),
        (
            "soaking",
            (t.eye, t.look),
            noon,
            wet(WeatherKind::Rain, 0.6),
        ),
        (
            "half_dried",
            (t.eye, t.look),
            noon,
            wet(WeatherKind::Clear, 0.5),
        ),
        ("sunset_rain", west, sunset, wet(WeatherKind::Rain, 1.)),
        ("sunset_after", west, sunset, wet(WeatherKind::Clear, 0.8)),
        (
            "thunder",
            (t.eye, t.look),
            noon,
            wet(WeatherKind::Thunder, 1.),
        ),
        (
            "midnight",
            (t.eye, t.look),
            hour(0.25),
            wet(WeatherKind::Rain, 1.),
        ),
    ];
    for (name, view, tick, weather) in scenes {
        shoot(&mut h, camera, &format!("wet_{name}"), view, tick, weather);
    }
    // Under a roof over the back of the terrace, looking out
    let roof_eye = t.origin.as_vec3() + Vec3::new(0.5, 2.62, 0.5);
    let mut roof = Vec::new();
    for along in -1..=4 {
        for across in -3..=3 {
            roof.push((
                t.origin + t.ahead * along + t.side * across + IVec3::Y * 4,
                Some(BlockId::OakPlanks),
            ));
        }
    }
    for (along, across) in [(-1, -3), (-1, 3), (4, -3), (4, 3)] {
        for y in 1..=3 {
            roof.push((
                t.origin + t.ahead * along + t.side * across + IVec3::Y * y,
                Some(BlockId::OakLog),
            ));
        }
    }
    place(&mut h, roof);
    let out = roof_eye + t.ahead.as_vec3() * 8. - Vec3::Y * 0.6;
    shoot(
        &mut h,
        camera,
        "wet_roof",
        (roof_eye, out),
        noon,
        wet(WeatherKind::Rain, 1.),
    );
}

#[test]
#[ignore = "pictures for a human: UI_SNAPSHOTS=1 cargo test -p client e2e::weather -- --ignored"]
fn snapshot_the_wet_world_in_ultra() {
    let (mut h, camera, v) = start(ShaderQuality::Ultra);
    let t = build_terrace(&mut h, &v);
    shoot(
        &mut h,
        camera,
        "wet_ultra_rain",
        (t.eye, t.look),
        hour(0.75),
        wet(WeatherKind::Rain, 1.),
    );
}

#[test]
#[ignore = "pictures for a human: UI_SNAPSHOTS=1 cargo test -p client e2e::weather -- --ignored"]
fn snapshot_the_wet_world_in_medium() {
    let (mut h, camera, v) = start(ShaderQuality::Medium);
    let t = build_terrace(&mut h, &v);
    shoot(
        &mut h,
        camera,
        "wet_medium_rain",
        (t.eye, t.look),
        hour(0.75),
        wet(WeatherKind::Rain, 1.),
    );
}

/// Takes the picture `weather_<name>` as `shoot` does, with the strikes `strikes` (the block
/// struck and the seed) at the brightest of their first stroke, or `age` seconds old.
fn shoot_strikes(
    h: &mut Harness,
    camera: Entity,
    name: &str,
    view: (Vec3, Vec3),
    tick: u64,
    strikes: &[(IVec3, u32)],
    age: Option<f32>,
) {
    let world = h.world_mut();
    let reach = shown_reach(
        world.resource::<RenderDistance>(),
        world.get_resource::<ServerRenderDistanceLimit>(),
    );
    let held = strikes
        .iter()
        .map(|&(at, seed)| {
            let mut strike = LiveStrike::new(Strike { at, seed }, view.0, reach);
            strike.age = age.unwrap_or_else(|| first_peak(seed));
            strike
        })
        .collect();
    world.insert_resource(HeldLightning(held));
    let weather = wet(WeatherKind::Thunder, 1.);
    shoot(h, camera, name, view, tick, weather);
    h.world_mut().insert_resource(HeldLightning::default());
}

/// When the first stroke of the strike of seed `seed` is at its brightest.
fn first_peak(seed: u32) -> f32 {
    Strokes::new(seed).peaks().next().unwrap()
}

/// The block struck `distance` blocks from `eye` toward `toward` on the ground plane: the top
/// of its column.
fn struck(h: &Harness, eye: Vec3, toward: Vec3, distance: f32) -> IVec3 {
    let column = eye.xz() + (toward - eye).xz().normalize() * distance;
    let (x, z) = (column.x.floor() as i32, column.y.floor() as i32);
    let map = h.world().resource::<ClientWorldMap>();
    let (y, _) = surface(map, x, z).expect("a struck column");
    IVec3::new(x, y, z)
}

/// The view from `eye` toward a strike at `at`, raised to show the bolt above it.
fn toward_strike(eye: Vec3, at: IVec3) -> (Vec3, Vec3) {
    let foot = at.as_vec3() + Vec3::new(0.5, 1., 0.5);
    let distance = foot.xz().distance(eye.xz());
    (eye, foot + Vec3::Y * distance * 0.42)
}

const SEEDS: [u32; 3] = [20_261_021, 77, 4_096];

#[test]
#[ignore = "pictures for a human: UI_SNAPSHOTS=1 cargo test -p client e2e::weather -- --ignored"]
fn snapshot_the_lightning() {
    let (mut h, camera, v) = start(ShaderQuality::High);
    let noon = hour(0.75);
    let midnight = hour(0.25);
    let near = struck(&h, v.eye, v.across, 50.);
    shoot_strikes(
        &mut h,
        camera,
        "lightning_near",
        toward_strike(v.eye, near),
        noon,
        &[(near, SEEDS[0])],
        None,
    );
    shoot_strikes(
        &mut h,
        camera,
        "lightning_night",
        toward_strike(v.eye, near),
        midnight,
        &[(near, SEEDS[0])],
        None,
    );
    let other = struck(&h, v.eye, v.across + Vec3::new(20., 0., 20.), 60.);
    shoot_strikes(
        &mut h,
        camera,
        "lightning_night_2",
        toward_strike(v.eye, other),
        midnight,
        &[(other, SEEDS[1])],
        None,
    );
    // The second stroke: the main channel alone
    let second = Strokes::new(SEEDS[0]).peaks().nth(1).unwrap();
    shoot_strikes(
        &mut h,
        camera,
        "lightning_restrike",
        toward_strike(v.eye, near),
        midnight,
        &[(near, SEEDS[0])],
        Some(second),
    );
    // Too far for the bolt: the clouds flash over it
    let far = struck(&h, v.eye, v.across, 300.);
    let up = (
        v.eye,
        v.eye + (v.across - v.eye).with_y(0.).normalize() * 100. + Vec3::Y * 40.,
    );
    shoot_strikes(
        &mut h,
        camera,
        "lightning_far",
        up,
        midnight,
        &[(far, SEEDS[2])],
        None,
    );
    shoot_strikes(
        &mut h,
        camera,
        "lightning_far_noon",
        up,
        noon,
        &[(far, SEEDS[2])],
        None,
    );
    // Over the wet terrace, the puddles lit
    let t = build_terrace(&mut h, &v);
    let lake = struck(&h, t.eye, t.look, 45.);
    let view = (
        t.eye + Vec3::Y * 0.5,
        t.look + (lake.as_vec3() - t.look) * 0.3 + Vec3::Y * 6.,
    );
    shoot_strikes(
        &mut h,
        camera,
        "lightning_lake",
        view,
        midnight,
        &[(lake, SEEDS[1])],
        None,
    );
    shoot_strikes(
        &mut h,
        camera,
        "lightning_lake_noon",
        view,
        noon,
        &[(lake, SEEDS[1])],
        None,
    );
}

#[test]
#[ignore = "pictures for a human: UI_SNAPSHOTS=1 cargo test -p client e2e::weather -- --ignored"]
fn snapshot_the_lightning_in_ultra() {
    let (mut h, camera, v) = start(ShaderQuality::Ultra);
    let near = struck(&h, v.eye, v.across, 50.);
    shoot_strikes(
        &mut h,
        camera,
        "lightning_ultra",
        toward_strike(v.eye, near),
        hour(0.75),
        &[(near, SEEDS[0])],
        None,
    );
    shoot_strikes(
        &mut h,
        camera,
        "lightning_ultra_night",
        toward_strike(v.eye, near),
        hour(0.25),
        &[(near, SEEDS[0])],
        None,
    );
}

#[test]
#[ignore = "pictures for a human: UI_SNAPSHOTS=1 cargo test -p client e2e::weather -- --ignored"]
fn snapshot_the_lightning_without_the_shaders() {
    let (mut h, camera, v) = start(ShaderQuality::Off);
    let near = struck(&h, v.eye, v.across, 50.);
    shoot_strikes(
        &mut h,
        camera,
        "lightning_off",
        toward_strike(v.eye, near),
        hour(0.75),
        &[(near, SEEDS[0])],
        None,
    );
    shoot_strikes(
        &mut h,
        camera,
        "lightning_off_night",
        toward_strike(v.eye, near),
        hour(0.25),
        &[(near, SEEDS[0])],
        None,
    );
}
