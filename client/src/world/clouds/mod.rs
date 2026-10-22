//! Block clouds drifting east high above the terrain, the same for every player: a periodic
//! pattern of full and empty cells, slid along +X by the world clock.

use bevy::{
    light::{NotShadowCaster, NotShadowReceiver},
    prelude::*,
};
use shared::{CHUNK_SIZE, TICKS_PER_SECOND};

use crate::state::{GameSet, GameState};
use crate::world::celestial::{smoothstep, sun_height};
use crate::world::overcast::Overcast;
use crate::world::shaders::ShaderFeatures;
use crate::world::time::ClientTime;
use crate::world::weather::ClientWeather;
use crate::world::{GraphicsSettings, MaterialResource, RenderDistance, ServerRenderDistanceLimit};
use field::{drift, threshold};
use material::{cloud_material_plugin, CloudMaterial, CloudParams};
use mesh::{cloud_mesh, CloudStyle};

pub mod field;
pub mod material;
pub mod mesh;

/// The width of a cell of the pattern along X and Z, in blocks.
pub const CELL: i32 = 12;

/// The height of the clouds, in blocks.
pub const THICKNESS: i32 = 4;

/// The height of the bottom of the clouds: the highest ridges stay under ~180.
pub const ALTITUDE: i32 = 192;

/// The pattern repeats every `PERIOD` cells along X and along Z.
pub const PERIOD: i32 = 256;

/// How fast the clouds drift towards +X, in blocks per second.
pub const SPEED: f32 = 0.5;

/// The share of the cells that are full under a clear sky, in rain and in a thunderstorm: the
/// thresholds of `field` are measured for them.
#[cfg(test)]
pub const COVERAGE: f64 = 0.45;
#[cfg(test)]
pub const RAIN_COVERAGE: f64 = 0.8;
#[cfg(test)]
pub const STORM_COVERAGE: f64 = 0.92;

/// The seed of the pattern: fixed, so that every world has the same sky.
pub const SEED: u64 = 0x00C1_0D5E_ED00_2026;

/// How bright each face of a cloud is, a fraction of the tint.
pub const SHADE_TOP: f32 = 1.0;
pub const SHADE_NORTH_SOUTH: f32 = 0.9;
pub const SHADE_EAST_WEST: f32 = 0.8;
pub const SHADE_BOTTOM: f32 = 0.7;

/// How opaque the clouds are, where they are not faded out, under a clear sky and in rain.
pub const OPACITY: f32 = 0.8;
const RAIN_OPACITY: f32 = 0.95;

/// The farthest the clouds are drawn, in blocks.
const MAX_REACH: f32 = 1024.;

/// Where the clouds start to fade out, as a part of their reach.
const FADE_START: f32 = 0.6;

/// The tint of the clouds by day, at night, and when the sun is near the horizon, in sRGB.
const DAY_TINT: Vec3 = Vec3::ONE;
const NIGHT_TINT: Vec3 = Vec3::new(0.17, 0.2, 0.29);
const WARM_TINT: Vec3 = Vec3::new(1.0, 0.7, 0.5);

/// The colour of the clouds under a full overcast in rain and in a thunderstorm, as a part of
/// the overcast at the zenith: a darker blue grey, as rain clouds hang under a lighter sky.
const RAIN_SHADE: Vec3 = Vec3::new(0.85, 0.89, 0.96);
const STORM_SHADE: Vec3 = Vec3::new(0.7, 0.74, 0.82);

/// The clouds of the sky: one mesh of the cells within reach of the camera, slid along +X by
/// the drift.
#[derive(Component, Default)]
pub struct Clouds {
    /// The style, the centre cell and the radius, in cells, of the window the mesh holds, and
    /// whether it holds the cells of every weather (`cloud_mesh`).
    window: Option<(CloudStyle, IVec2, i32, bool)>,
}

/// The clouds, drawn while in game.
pub fn clouds_plugin(app: &mut App) {
    app.add_plugins(cloud_material_plugin)
        .add_systems(OnEnter(GameState::Game), spawn_clouds)
        .add_systems(
            Update,
            (
                drift_clouds_system,
                rebuild_clouds_system,
                cloud_uniforms_system,
            )
                .chain()
                .after(GameSet::Camera)
                .run_if(in_state(GameState::Game)),
        );
}

/// How far the clouds are drawn, in blocks: twice the render distance the server grants,
/// within `MAX_REACH`.
pub fn reach(render_distance: u32, server_limit: Option<u32>) -> f32 {
    let chunks = render_distance.min(server_limit.unwrap_or(u32::MAX));
    (2. * chunks as f32 * CHUNK_SIZE as f32).min(MAX_REACH)
}

fn current_reach(
    render_distance: &RenderDistance,
    server_limit: Option<Res<ServerRenderDistanceLimit>>,
) -> f32 {
    reach(render_distance.distance, server_limit.map(|limit| limit.0))
}

/// The cell of the pattern under `camera` when the clouds have drifted by `drift`.
pub fn camera_cell(camera: Vec3, drift: f32) -> IVec2 {
    let cell = CELL as f32;
    IVec2::new(
        ((camera.x - drift) / cell).floor() as i32,
        (camera.z / cell).floor() as i32,
    )
}

/// How far the clouds have drifted at the world clock `tick`, a part `fraction` of the way to
/// the next tick.
pub fn smooth_drift(tick: u64, fraction: f32) -> f32 {
    drift(tick) + fraction * SPEED / TICKS_PER_SECOND as f32
}

/// The colour of the clouds at the world clock `tick`, after the height of the sun.
pub fn cloud_tint(tick: u64) -> Color {
    let height = sun_height(tick);
    let day = smoothstep(-0.2, 0.3, height);
    // the most warmth at the horizon, none once the sun is a third of the way up or down
    let warmth = 0.7 * (1. - height.abs() / 0.35).max(0.);
    let tint = NIGHT_TINT.lerp(DAY_TINT, day).lerp(WARM_TINT, warmth);
    Color::srgb(tint.x, tint.y, tint.z)
}

/// The colour of the clouds under the full overcast of `overcast`, with the sun at
/// `sun_height`, in linear RGB as on screen: as bright as the overcast at that hour, which the
/// tint of a clear sky is not.
pub fn overcast_cloud_color(overcast: Overcast, sun_height: f32, physical_sky: bool) -> Vec3 {
    let (_, zenith) = overcast.colors(sun_height, physical_sky);
    zenith * RAIN_SHADE.lerp(STORM_SHADE, overcast.storm)
}

/// Spawns the clouds, drawn in colour, with a child that draws their depth first (see
/// `CloudMaterial`).
fn spawn_clouds(
    mut commands: Commands,
    resources: Option<Res<MaterialResource>>,
    mut materials: ResMut<Assets<CloudMaterial>>,
) {
    let weather = resources.map(|resources| resources.weather.clone());
    let weather = weather.unwrap_or_default();
    commands
        .spawn((
            Name::new("Clouds"),
            Clouds::default(),
            MeshMaterial3d(materials.add(CloudMaterial {
                weather: weather.clone(),
                ..default()
            })),
            Transform::default(),
            Visibility::default(),
            NotShadowCaster,
            NotShadowReceiver,
            DespawnOnExit(GameState::Game),
        ))
        .with_child((
            Name::new("Cloud depth"),
            MeshMaterial3d(materials.add(CloudMaterial {
                depth_only: true,
                weather,
                ..default()
            })),
            NotShadowCaster,
            NotShadowReceiver,
        ));
}

/// Slides the clouds to the drift of the clock, moving on between two ticks.
fn drift_clouds_system(
    time: Res<ClientTime>,
    fixed: Option<Res<Time<Fixed>>>,
    mut clouds: Query<&mut Transform, With<Clouds>>,
) {
    let x = smooth_drift(time.0, fixed.map_or(0., |fixed| fixed.overstep_fraction()));
    for mut transform in &mut clouds {
        transform.translation.x = x;
    }
}

/// Builds the mesh of the cells within reach of the camera again when the camera has moved to
/// another cell of the pattern, or the reach or the style has changed, or the sky has started
/// or stopped to be overcast: under a clear sky the mesh holds only the clouds of a clear sky,
/// and as soon as it clouds over those of every weather. Only the Fancy clouds draw their depth
/// first: the quads of the Fast ones never overlap.
#[allow(clippy::too_many_arguments)]
fn rebuild_clouds_system(
    cameras: Query<&Transform, (With<Camera3d>, Without<Clouds>)>,
    style: Res<CloudStyle>,
    weather: Option<Res<ClientWeather>>,
    render_distance: Res<RenderDistance>,
    server_limit: Option<Res<ServerRenderDistanceLimit>>,
    mut clouds: Query<(Entity, &mut Clouds, &Transform, &Children)>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut commands: Commands,
) {
    let Ok(camera) = cameras.single() else {
        return;
    };
    let reach = current_reach(&render_distance, server_limit);
    // so that the window covers the reach from anywhere in the cell of the camera
    let radius = (reach / CELL as f32).ceil() as i32 + 1;
    let banded = threshold(Overcast::current(weather.as_deref())) < field::CLEAR_THRESHOLD;
    for (entity, mut clouds, transform, children) in &mut clouds {
        let window = (
            *style,
            camera_cell(camera.translation, transform.translation.x),
            radius,
            banded,
        );
        if clouds.window == Some(window) {
            continue;
        }
        clouds.window = Some(window);
        let mesh = cloud_mesh(window.0, window.1, window.2, window.3).map(|m| meshes.add(m));
        // the depth is drawn with the same mesh
        let depth = children.iter().filter(|_| *style == CloudStyle::Fancy);
        for drawing in children.iter().filter(|_| *style != CloudStyle::Fancy) {
            commands.entity(drawing).remove::<Mesh3d>();
        }
        for drawing in depth.chain([entity]) {
            match &mesh {
                Some(mesh) => commands.entity(drawing).insert(Mesh3d(mesh.clone())),
                None => commands.entity(drawing).remove::<Mesh3d>(),
            };
        }
    }
}

/// Gives the materials of the clouds the tint of the hour and the weather, the fade of the
/// reach, which closes in with the fog in rain, and the threshold of the weather.
pub(crate) fn cloud_uniforms_system(
    time: Res<ClientTime>,
    weather: Option<Res<ClientWeather>>,
    graphics: Option<Res<GraphicsSettings>>,
    render_distance: Res<RenderDistance>,
    server_limit: Option<Res<ServerRenderDistanceLimit>>,
    clouds: Query<&MeshMaterial3d<CloudMaterial>>,
    mut materials: ResMut<Assets<CloudMaterial>>,
) {
    let overcast = Overcast::current(weather.as_deref());
    let reach = current_reach(&render_distance, server_limit) * overcast.fog_end();
    let physical_sky = graphics.is_some_and(|graphics| ShaderFeatures::of(graphics.shaders).sky);
    let grey = overcast_cloud_color(overcast, sun_height(time.0), physical_sky);
    let params = CloudParams {
        tint: cloud_tint(time.0).to_linear().to_vec4(),
        opacity: OPACITY.lerp(RAIN_OPACITY, overcast.level),
        fade_start: FADE_START * reach,
        fade_end: reach,
        threshold: threshold(overcast),
        overcast: grey.extend(overcast.level),
    };
    for material in &clouds {
        // Only a changed material is sent to the GPU again
        if materials.get(material).is_some_and(|m| m.params != params) {
            if let Some(mut material) = materials.get_mut(material) {
                material.params = params;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::weather::ClientWeather;
    use bevy::state::app::StatesPlugin;
    use field::{CLEAR_THRESHOLD, STORM_THRESHOLD};
    use shared::world::WeatherKind;

    const RAIN: Overcast = Overcast {
        level: 1.,
        storm: 0.,
    };
    const STORM: Overcast = Overcast {
        level: 1.,
        storm: 1.,
    };
    use shared::{DAY_DURATION_IN_TICKS, MIDNIGHT_IN_DAY, NOON_IN_DAY};

    fn srgb(color: Color) -> Vec3 {
        let color = color.to_srgba();
        Vec3::new(color.red, color.green, color.blue)
    }

    #[test]
    fn the_clouds_are_white_at_noon() {
        let noon = srgb(cloud_tint(3 * DAY_DURATION_IN_TICKS + NOON_IN_DAY));
        assert!(noon.abs_diff_eq(DAY_TINT, 1e-4), "{noon}");
    }

    #[test]
    fn the_clouds_are_grey_blue_at_midnight() {
        let midnight = srgb(cloud_tint(3 * DAY_DURATION_IN_TICKS + MIDNIGHT_IN_DAY));
        assert!(midnight.abs_diff_eq(NIGHT_TINT, 1e-4), "{midnight}");
    }

    #[test]
    fn the_clouds_are_warm_at_sunrise_and_sunset() {
        // a cycle starts at sunset; the sun rises half a cycle later
        for tick in [0, DAY_DURATION_IN_TICKS / 2] {
            let tint = srgb(cloud_tint(tick));
            assert!(tint.x > tint.z + 0.2, "{tick}: {tint}");
            assert!(
                tint.x > NIGHT_TINT.x && tint.z < DAY_TINT.z,
                "{tick}: {tint}"
            );
        }
    }

    #[test]
    fn the_clouds_of_the_rain_are_a_blue_grey_darker_than_the_sky_and_in_a_storm_darker_still() {
        for physical in [true, false] {
            let (_, zenith) = RAIN.colors(0.9, physical);
            let rain = overcast_cloud_color(RAIN, 0.9, physical);
            let storm = overcast_cloud_color(STORM, 0.9, physical);
            assert!(rain.z > rain.x && storm.z > storm.x, "{rain} {storm}");
            // seen from under, darker than the sky
            let brightness = |color: Vec3| color.element_sum() / 3.;
            let under = rain * SHADE_BOTTOM;
            assert!(brightness(under) < 0.7 * brightness(zenith), "{rain}");
            assert!(brightness(storm) < 0.6 * brightness(rain), "{storm}");
            // and dark at night
            let night = overcast_cloud_color(RAIN, -0.9, physical);
            assert!(night.max_element() < 0.1 * rain.min_element(), "{night}");
        }
    }

    #[test]
    fn the_clouds_thicken_and_close_in_with_the_weather() {
        let mut app = app();
        app.insert_resource(ClientWeather::settled(WeatherKind::Thunder));
        app.update();
        let params = params(&mut app);
        assert_eq!(params.overcast.w, 1.);
        assert_eq!(params.threshold, STORM_THRESHOLD);
        assert_eq!(params.opacity, RAIN_OPACITY);
        let reach = reach(4, None);
        assert!(
            (params.fade_end - 0.6 * reach).abs() < 1e-3,
            "{}",
            params.fade_end
        );
        assert!((params.fade_start - FADE_START * 0.6 * reach).abs() < 1e-3);
    }

    #[test]
    fn the_reach_is_twice_the_render_distance() {
        assert_eq!(reach(12, None), 2. * 12. * CHUNK_SIZE as f32);
    }

    #[test]
    fn the_reach_follows_the_server_limit() {
        assert_eq!(reach(12, Some(8)), 2. * 8. * CHUNK_SIZE as f32);
        assert_eq!(reach(6, Some(8)), 2. * 6. * CHUNK_SIZE as f32);
    }

    #[test]
    fn the_reach_is_capped() {
        assert_eq!(reach(64, None), MAX_REACH);
    }

    #[test]
    fn the_cell_of_the_camera_is_taken_in_the_drifted_pattern() {
        let cell = CELL as f32;
        assert_eq!(camera_cell(Vec3::new(1., 300., -1.), 0.), IVec2::new(0, -1));
        assert_eq!(camera_cell(Vec3::new(cell, 0., cell), 0.), IVec2::new(1, 1));
        // the pattern has moved east under the camera
        assert_eq!(camera_cell(Vec3::new(1., 0., 0.), 2.), IVec2::new(-1, 0));
        assert_eq!(
            camera_cell(Vec3::new(1., 0., 0.), 2. * cell),
            IVec2::new(-2, 0)
        );
    }

    fn app() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, StatesPlugin, AssetPlugin::default()))
            .init_asset::<Mesh>()
            .init_asset::<CloudMaterial>()
            .insert_state(GameState::Game)
            .insert_resource(ClientTime(0))
            .insert_resource(RenderDistance { distance: 4 })
            .init_resource::<CloudStyle>()
            .add_systems(OnEnter(GameState::Game), spawn_clouds)
            .add_systems(
                Update,
                (
                    drift_clouds_system,
                    rebuild_clouds_system,
                    cloud_uniforms_system,
                )
                    .chain()
                    .run_if(in_state(GameState::Game)),
            );
        app.world_mut()
            .spawn((Camera3d::default(), Transform::from_xyz(1., 70., 1.)));
        app.update();
        app
    }

    fn clouds(app: &mut App) -> Option<Entity> {
        app.world_mut()
            .query_filtered::<Entity, With<Clouds>>()
            .iter(app.world())
            .next()
    }

    fn mesh(app: &mut App) -> Handle<Mesh> {
        let clouds = clouds(app).unwrap();
        app.world().get::<Mesh3d>(clouds).unwrap().0.clone()
    }

    fn move_camera(app: &mut App, to: Vec3) {
        let world = app.world_mut();
        let mut camera = world
            .query_filtered::<&mut Transform, With<Camera3d>>()
            .single_mut(world)
            .unwrap();
        camera.translation = to;
        app.update();
    }

    #[test]
    fn the_clouds_appear_on_entering_the_game_and_leave_with_it() {
        let mut app = app();
        assert!(clouds(&mut app).is_some());
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Menu);
        app.update();
        assert!(clouds(&mut app).is_none());
    }

    #[test]
    fn the_mesh_is_kept_while_the_camera_stays_in_its_cell() {
        let mut app = app();
        let first = mesh(&mut app);
        for x in [2., 5., 11.9] {
            move_camera(&mut app, Vec3::new(x, 90., 11.));
            assert_eq!(mesh(&mut app), first, "{x}");
        }
    }

    #[test]
    fn the_mesh_is_rebuilt_when_the_camera_changes_cell() {
        let mut app = app();
        let first = mesh(&mut app);
        move_camera(&mut app, Vec3::new(CELL as f32 + 1., 70., 1.));
        let second = mesh(&mut app);
        assert_ne!(second, first);
        // and when the reach changes
        app.insert_resource(ServerRenderDistanceLimit(2));
        app.update();
        assert_ne!(mesh(&mut app), second);
    }

    fn quads_of_the_mesh(app: &mut App) -> usize {
        let handle = mesh(app);
        let meshes = app.world().resource::<Assets<Mesh>>();
        meshes.get(&handle).unwrap().count_vertices() / 4
    }

    #[test]
    fn the_mesh_holds_the_cells_of_every_weather_only_while_the_sky_is_overcast() {
        let mut app = app();
        let clear = quads_of_the_mesh(&mut app);
        let mut weather = ClientWeather::settled(WeatherKind::Clear);
        app.insert_resource(weather.clone());
        app.update();
        assert_eq!(quads_of_the_mesh(&mut app), clear);
        // the first wisp of overcast brings the cells of every weather in
        weather.rain = 0.01;
        app.insert_resource(weather.clone());
        app.update();
        let overcast = quads_of_the_mesh(&mut app);
        assert!(overcast > 3 * clear, "{overcast} {clear}");
        app.insert_resource(ClientWeather::settled(WeatherKind::Thunder));
        app.update();
        assert_eq!(quads_of_the_mesh(&mut app), overcast);
        app.insert_resource(ClientWeather::settled(WeatherKind::Clear));
        app.update();
        assert_eq!(quads_of_the_mesh(&mut app), clear);
    }

    #[test]
    fn the_clouds_slide_with_the_clock() {
        let mut app = app();
        let first = mesh(&mut app);
        app.insert_resource(ClientTime(10 * TICKS_PER_SECOND));
        app.update();
        let clouds = clouds(&mut app).unwrap();
        let x = app.world().get::<Transform>(clouds).unwrap().translation.x;
        // and on by the part of a tick elapsed since
        let tick = SPEED / TICKS_PER_SECOND as f32;
        assert!((5.0..=5. + tick).contains(&x), "{x}");
        // the camera is now over the cell west of the one it was over
        assert_ne!(mesh(&mut app), first);
    }

    /// The entities drawing the clouds and their materials, the depth first.
    fn drawings(app: &mut App) -> Vec<(Entity, CloudMaterial)> {
        let world = app.world_mut();
        let handles: Vec<_> = world
            .query::<(Entity, &MeshMaterial3d<CloudMaterial>)>()
            .iter(world)
            .map(|(entity, material)| (entity, material.0.clone()))
            .collect();
        let materials = world.resource::<Assets<CloudMaterial>>();
        let mut drawings: Vec<_> = handles
            .into_iter()
            .map(|(entity, handle)| (entity, materials.get(&handle).unwrap().clone()))
            .collect();
        drawings.sort_by_key(|(_, material)| !material.depth_only);
        drawings
    }

    #[test]
    fn the_depth_of_the_clouds_is_drawn_just_before_their_colour() {
        let mut app = app();
        let [(depth, depth_material), (colour, colour_material)] =
            drawings(&mut app).try_into().unwrap();
        assert!(depth_material.depth_only && !colour_material.depth_only);
        assert!(depth_material.depth_bias() < colour_material.depth_bias());
        // the same mesh at the same place, the drift included
        assert_eq!(app.world().get::<ChildOf>(depth).unwrap().parent(), colour);
        assert_eq!(
            app.world().get::<Transform>(depth),
            Some(&Transform::IDENTITY)
        );
        let mesh_of = |app: &App, entity| app.world().get::<Mesh3d>(entity).cloned();
        assert_eq!(mesh_of(&app, depth), mesh_of(&app, colour));
        move_camera(&mut app, Vec3::new(CELL as f32 + 1., 70., 1.));
        assert_eq!(mesh_of(&app, depth), mesh_of(&app, colour));
        // both fade out alike
        app.update();
        let [(_, depth_material), (_, colour_material)] = drawings(&mut app).try_into().unwrap();
        assert_eq!(depth_material.params, colour_material.params);
        assert!(app.world().get::<NotShadowCaster>(depth).is_some());
    }

    #[test]
    fn the_smooth_drift_moves_on_between_two_ticks() {
        let tick = SPEED / TICKS_PER_SECOND as f32;
        assert_eq!(smooth_drift(20, 0.), drift(20));
        assert_eq!(smooth_drift(20, 0.5), drift(20) + tick / 2.);
    }

    fn params(app: &mut App) -> CloudParams {
        let clouds = clouds(app).unwrap();
        let handle = app
            .world()
            .get::<MeshMaterial3d<CloudMaterial>>(clouds)
            .unwrap()
            .0
            .clone();
        app.world()
            .resource::<Assets<CloudMaterial>>()
            .get(&handle)
            .unwrap()
            .params
    }

    #[test]
    fn the_uniforms_follow_the_reach_and_the_hour() {
        let mut app = app();
        app.insert_resource(ClientTime(NOON_IN_DAY));
        app.update();
        let params = params(&mut app);
        let reach = reach(4, None);
        assert_eq!(params.fade_end, reach);
        assert_eq!(params.fade_start, FADE_START * reach);
        assert_eq!(params.opacity, OPACITY);
        assert_eq!(params.tint, cloud_tint(NOON_IN_DAY).to_linear().to_vec4());
        assert_eq!(params.threshold, CLEAR_THRESHOLD);
        assert_eq!(params.overcast.w, 0.);
    }

    fn meshes_of_the_drawings(app: &mut App) -> (Option<Mesh3d>, Option<Mesh3d>) {
        let [(depth, _), (colour, _)] = drawings(app).try_into().unwrap();
        let mesh_of = |entity| app.world().get::<Mesh3d>(entity).cloned();
        (mesh_of(depth), mesh_of(colour))
    }

    #[test]
    fn the_style_of_the_clouds_applies_at_once() {
        let mut app = app();
        let (depth, fancy) = meshes_of_the_drawings(&mut app);
        assert!(fancy.is_some() && depth == fancy);

        // a flat layer, drawn without its depth
        *app.world_mut().resource_mut::<CloudStyle>() = CloudStyle::Fast;
        app.update();
        let (depth, fast) = meshes_of_the_drawings(&mut app);
        assert_eq!(depth, None);
        let fast = fast.unwrap();
        assert_ne!(Some(&fast), fancy.as_ref());
        let mesh = app.world().resource::<Assets<Mesh>>().get(&fast.0).unwrap();
        let positions = mesh
            .attribute(Mesh::ATTRIBUTE_POSITION)
            .unwrap()
            .as_float3();
        let middle = ALTITUDE as f32 + THICKNESS as f32 / 2.;
        assert!(positions.unwrap().iter().all(|p| p[1] == middle));

        *app.world_mut().resource_mut::<CloudStyle>() = CloudStyle::Off;
        app.update();
        assert_eq!(meshes_of_the_drawings(&mut app), (None, None));

        *app.world_mut().resource_mut::<CloudStyle>() = CloudStyle::Fancy;
        app.update();
        let (depth, fancy) = meshes_of_the_drawings(&mut app);
        assert!(fancy.is_some() && depth == fancy);
    }
}
