use bevy::prelude::*;
use shared::world::liquid::liquid_height;
use shared::world::ChunkMap;
use shared::CHUNK_SIZE;

use crate::world::celestial::{smoothstep, sun_height};
use crate::world::overcast::Overcast;
use crate::world::time::ClientTime;
use crate::world::weather::ClientWeather;
use crate::world::{ClientWorldMap, RenderDistance, ServerRenderDistanceLimit};

use super::CameraController;

/// The colour behind the world by day.
pub const SKY_COLOR: Color = Color::srgb_u8(117, 159, 183);
/// The colour of the sky at midnight.
const NIGHT_SKY_COLOR: Color = Color::srgb_u8(10, 13, 26);
/// The colour the sky warms to while the sun is on the horizon.
const WARM_SKY_COLOR: Color = Color::srgb_u8(235, 140, 80);
const UNDERWATER_COLOR: Color = Color::srgb_u8(30, 62, 130);

/// The colour of the sky at the world clock `tick`, after the height of the sun: dark at
/// night, `SKY_COLOR` by day, warm while the sun is near the horizon.
pub fn sky_color(tick: u64) -> Color {
    let height = sun_height(tick);
    let day = smoothstep(-0.25, 0.25, height);
    // the most warmth at the horizon, none once the sun is a third of the way up or down
    let warmth = 0.75 * (1. - height.abs() / 0.35).max(0.);
    let srgb = |color: Color| Vec3::from_array(color.to_srgba().to_f32_array_no_alpha());
    let sky = srgb(NIGHT_SKY_COLOR)
        .lerp(srgb(SKY_COLOR), day)
        .lerp(srgb(WARM_SKY_COLOR), warmth);
    Color::srgb(sky.x, sky.y, sky.z)
}

/// The colour behind the world at the world clock `tick` under `overcast`: the colour of the
/// sky of the hour, greyed by the clouds of the rain.
pub fn weather_sky_color(tick: u64, overcast: Overcast) -> Color {
    let sky = sky_color(tick);
    if overcast.level == 0. {
        return sky;
    }
    let (grey, _) = overcast.colors(sun_height(tick), false);
    sky.mix(&Color::linear_rgb(grey.x, grey.y, grey.z), overcast.level)
}

/// Whether an eye at `eye` is under the surface of a liquid.
pub fn eye_under_water(map: &ChunkMap, eye: Vec3) -> bool {
    liquid_height(map, eye.floor().as_ivec3()).is_some_and(|height| eye.y - eye.y.floor() < height)
}

/// Under water, a fog the colour of the water hides what is more than a few blocks away. Out
/// of it, a fog the colour of the sky of the hour hides the edge of the chunks shown: it
/// thickens over the last part of the render distance, and the world fades into the sky at its
/// end; in rain, it closes in. The fog is changed in place, as the sky changes every tick.
pub fn underwater_fog_system(
    world_map: Res<ClientWorldMap>,
    time: Res<ClientTime>,
    weather: Option<Res<ClientWeather>>,
    render_distance: Res<RenderDistance>,
    server_limit: Option<Res<ServerRenderDistanceLimit>>,
    mut cameras: Query<
        (Entity, &Transform, &mut Camera, Option<&mut DistanceFog>),
        With<CameraController>,
    >,
    mut commands: Commands,
) {
    let distance = render_distance
        .distance
        .min(server_limit.map_or(u32::MAX, |limit| limit.0));
    for (entity, transform, mut camera, fog) in &mut cameras {
        let (color, start, end) = if eye_under_water(&world_map.blocks, transform.translation) {
            (UNDERWATER_COLOR, 2.0, 16.0)
        } else {
            // Every chunk within the render distance of the chunk of the player is shown: the
            // edge is at least this far from the player along x and z
            let edge = (distance * CHUNK_SIZE as u32) as f32;
            let overcast = Overcast::current(weather.as_deref());
            let (start, end) = overcast.fog_range(edge);
            (weather_sky_color(time.0, overcast), start, end)
        };
        let falloff = FogFalloff::Linear { start, end };
        match fog {
            Some(mut fog) => {
                let same = fog.color == color
                    && matches!(fog.falloff, FogFalloff::Linear { start: s, end: e } if s == start && e == end);
                if !same {
                    fog.color = color;
                    fog.falloff = falloff;
                }
            }
            None => {
                commands.entity(entity).insert(DistanceFog {
                    color,
                    falloff,
                    ..default()
                });
            }
        }
        if !matches!(camera.clear_color, ClearColorConfig::Custom(c) if c == color) {
            camera.clear_color = ClearColorConfig::Custom(color);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::world::{BlockId, ChunkMap, WeatherKind};
    use shared::{DAY_DURATION_IN_TICKS, MIDNIGHT_IN_DAY, NOON_IN_DAY};

    fn srgb(color: Color) -> Vec3 {
        let color = color.to_srgba();
        Vec3::new(color.red, color.green, color.blue)
    }

    #[test]
    fn the_sky_is_blue_at_noon_and_dark_at_midnight() {
        let noon = srgb(sky_color(4 * DAY_DURATION_IN_TICKS + NOON_IN_DAY));
        assert!(noon.abs_diff_eq(srgb(SKY_COLOR), 1e-4), "{noon}");
        let midnight = srgb(sky_color(4 * DAY_DURATION_IN_TICKS + MIDNIGHT_IN_DAY));
        assert!(
            midnight.abs_diff_eq(srgb(NIGHT_SKY_COLOR), 1e-4),
            "{midnight}"
        );
        assert!(midnight.max_element() < 0.15, "{midnight}");
    }

    #[test]
    fn the_sky_is_warm_at_sunrise_and_sunset() {
        // a cycle starts at sunset; the sun rises half a cycle later
        for tick in [0, DAY_DURATION_IN_TICKS / 2] {
            let sky = srgb(sky_color(tick));
            assert!(sky.x > sky.z + 0.15, "{tick}: {sky}");
            assert!(sky.x > srgb(SKY_COLOR).x, "{tick}: {sky}");
        }
    }

    #[test]
    fn the_sky_changes_without_jumps() {
        let step = DAY_DURATION_IN_TICKS / 400;
        for tick in (0..DAY_DURATION_IN_TICKS).step_by(step as usize) {
            let (now, next) = (srgb(sky_color(tick)), srgb(sky_color(tick + step)));
            assert!(now.distance(next) < 0.05, "{tick}: {now} then {next}");
        }
    }

    fn fog_of(app: &App, camera: Entity) -> (Color, f32) {
        let fog = app.world().get::<DistanceFog>(camera).unwrap();
        let FogFalloff::Linear { end, .. } = fog.falloff else {
            panic!("a linear fog");
        };
        (fog.color, end)
    }

    #[test]
    fn under_water_the_fog_is_near_and_out_of_it_at_the_end_of_the_render_distance() {
        let mut blocks = ChunkMap::with_loaded_region(IVec3::ZERO, IVec3::splat(15));
        blocks.fill_box(IVec3::ZERO, IVec3::new(15, 3, 15), BlockId::Water);
        let mut app = App::new();
        app.insert_resource(ClientWorldMap {
            blocks,
            ..default()
        })
        .insert_resource(ClientTime(NOON_IN_DAY))
        .insert_resource(RenderDistance { distance: 24 })
        .add_systems(Update, underwater_fog_system);
        let camera = app
            .world_mut()
            .spawn((
                Camera::default(),
                Transform::from_xyz(5.5, 2.5, 5.5),
                CameraController::default(),
            ))
            .id();

        app.update();
        assert_eq!(fog_of(&app, camera), (UNDERWATER_COLOR, 16.0));

        // In the top cell of the water, above its surface
        app.world_mut()
            .get_mut::<Transform>(camera)
            .unwrap()
            .translation
            .y = 3.95;
        app.update();
        assert_eq!(fog_of(&app, camera), (SKY_COLOR, 24.0 * 16.0));
        assert!(matches!(
            app.world().get::<Camera>(camera).unwrap().clear_color,
            ClearColorConfig::Custom(color) if color == SKY_COLOR
        ));

        // A server that allows less brings the fog nearer
        app.insert_resource(ServerRenderDistanceLimit(8));
        app.update();
        assert_eq!(fog_of(&app, camera), (SKY_COLOR, 8.0 * 16.0));
    }

    fn clear_color_of(app: &App, camera: Entity) -> Color {
        match app.world().get::<Camera>(camera).unwrap().clear_color {
            ClearColorConfig::Custom(color) => color,
            other => panic!("a custom clear colour, not {other:?}"),
        }
    }

    #[test]
    fn the_fog_closes_in_and_greys_under_rain() {
        let mut blocks = ChunkMap::with_loaded_region(IVec3::ZERO, IVec3::splat(15));
        blocks.fill_box(IVec3::ZERO, IVec3::new(15, 3, 15), BlockId::Water);
        let mut app = App::new();
        app.insert_resource(ClientWorldMap {
            blocks,
            ..default()
        })
        .insert_resource(ClientTime(NOON_IN_DAY))
        .insert_resource(ClientWeather::settled(WeatherKind::Rain))
        .insert_resource(RenderDistance { distance: 16 })
        .add_systems(Update, underwater_fog_system);
        let camera = app
            .world_mut()
            .spawn((
                Camera::default(),
                Transform::from_xyz(5.5, 10.5, 5.5),
                CameraController::default(),
            ))
            .id();
        app.update();
        let overcast = Overcast::of(&ClientWeather::settled(WeatherKind::Rain));
        let (horizon, _) = overcast.colors(sun_height(NOON_IN_DAY), false);
        let grey = Color::linear_rgb(horizon.x, horizon.y, horizon.z);
        let fog = app.world().get::<DistanceFog>(camera).unwrap();
        assert!(matches!(
            fog.falloff,
            FogFalloff::Linear { start, end } if (start, end) == overcast.fog_range(256.)
        ));
        assert!(
            srgb(fog.color).abs_diff_eq(srgb(grey), 1e-4),
            "{:?}",
            fog.color
        );
        assert_eq!(clear_color_of(&app, camera), fog.color);

        // under water, as under a clear sky
        app.world_mut()
            .get_mut::<Transform>(camera)
            .unwrap()
            .translation
            .y = 2.5;
        app.update();
        assert_eq!(fog_of(&app, camera), (UNDERWATER_COLOR, 16.0));
    }

    #[test]
    fn the_sky_follows_the_hour_out_of_the_water_and_not_in_it() {
        let mut blocks = ChunkMap::with_loaded_region(IVec3::ZERO, IVec3::splat(15));
        blocks.fill_box(IVec3::ZERO, IVec3::new(15, 3, 15), BlockId::Water);
        let mut app = App::new();
        app.insert_resource(ClientWorldMap {
            blocks,
            ..default()
        })
        .insert_resource(ClientTime(NOON_IN_DAY))
        .insert_resource(RenderDistance { distance: 8 })
        .add_systems(Update, underwater_fog_system);
        let camera = app
            .world_mut()
            .spawn((
                Camera::default(),
                Transform::from_xyz(5.5, 10.5, 5.5),
                CameraController::default(),
            ))
            .id();
        app.update();
        // a part of the fog this system does not set, to see that it is changed in place
        let sun_glow = Color::srgb(1., 0.5, 0.);
        app.world_mut()
            .get_mut::<DistanceFog>(camera)
            .unwrap()
            .directional_light_color = sun_glow;

        for tick in [MIDNIGHT_IN_DAY, 0, NOON_IN_DAY] {
            app.insert_resource(ClientTime(tick));
            app.update();
            assert_eq!(fog_of(&app, camera), (sky_color(tick), 8.0 * 16.0));
            assert_eq!(clear_color_of(&app, camera), sky_color(tick));
        }
        let fog = app.world().get::<DistanceFog>(camera).unwrap();
        assert_eq!(fog.directional_light_color, sun_glow);

        // under water, whatever the hour
        app.world_mut()
            .get_mut::<Transform>(camera)
            .unwrap()
            .translation
            .y = 2.5;
        for tick in [MIDNIGHT_IN_DAY, NOON_IN_DAY] {
            app.insert_resource(ClientTime(tick));
            app.update();
            assert_eq!(fog_of(&app, camera), (UNDERWATER_COLOR, 16.0));
            assert_eq!(clear_color_of(&app, camera), UNDERWATER_COLOR);
        }
    }
}
