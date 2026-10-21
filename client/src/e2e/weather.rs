//! Pictures of the weather, for a human to look at: the lake, forest and hills of
//! `e2e::shaders` from the shore at noon under a clear sky, in rain and in a thunderstorm,
//! looking across the water and looking up, then in rain at sunset and at midnight, from above,
//! with the rain halfway in, and without the shaders. Saved with `UI_SNAPSHOTS=1` as
//! `weather_*`.

use std::time::Instant;

use bevy::ecs::system::RunSystemOnce;
use bevy::prelude::*;
use shared::world::{BlockId, WeatherKind};

use super::game::enter_world_with;
use super::harness::{Harness, Options};
use super::shaders::{folder, hour, looking, surface, SITE};
use crate::camera::{underwater_fog_system, CameraController};
use crate::player::PlayerBody;
use crate::world::time::ClientTime;
use crate::world::weather::{ease_weather_system, ClientWeather};
use crate::world::{ClientWorldMap, ShaderQuality};

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
}

/// The weather the pictures are taken in, held whatever the levels would ease to.
#[derive(Resource, Default)]
struct HeldWeather(ClientWeather);

fn hold_weather(held: Res<HeldWeather>, mut weather: ResMut<ClientWeather>) {
    *weather = held.0.clone();
}

/// A client with the shaders `quality` in the world of `e2e::shaders`, its chunks loaded, the
/// UI and the body of the player hidden and the weather held as set; and the camera, placed
/// by hand.
fn start(quality: ShaderQuality) -> (Harness, Entity, Views) {
    let mut h = enter_world_with(
        folder(),
        "Shaders",
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
    h.app
        .init_resource::<HeldWeather>()
        .add_systems(Update, hold_weather.after(ease_weather_system));

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
    let eye = Vec3::new(shore.0 as f32 + 0.5, shore_y + 2.7, shore.1 as f32 + 0.5);
    let ahead = (water_at - eye).with_y(0.).normalize();
    let above = water_at - ahead * 60. + Vec3::Y * 45.;
    let views = Views {
        eye,
        across: water_at + ahead * 40. + Vec3::Y * 4.,
        up: eye + ahead * 30. + Vec3::Y * 45.,
        above,
        west: above + Vec3::new(0., -30., 100.),
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
