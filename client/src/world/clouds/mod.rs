//! Block clouds drifting east high above the terrain, the same for every player: a periodic
//! pattern of full and empty cells, slid along +X by the world clock.

use bevy::{
    light::{NotShadowCaster, NotShadowReceiver},
    prelude::*,
};
use shared::{CHUNK_SIZE, TICKS_PER_SECOND};

use crate::state::{GameSet, GameState};
use crate::world::celestial::sun_angle;
use crate::world::time::ClientTime;
use crate::world::{RenderDistance, ServerRenderDistanceLimit};
use field::drift;
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

/// The share of the cells that are full: `field::THRESHOLD` is measured for it.
#[cfg(test)]
pub const COVERAGE: f64 = 0.45;

/// The seed of the pattern: fixed, so that every world has the same sky.
pub const SEED: u64 = 0x00C1_0D5E_ED00_2026;

/// How bright each face of a cloud is, a fraction of the tint.
pub const SHADE_TOP: f32 = 1.0;
pub const SHADE_NORTH_SOUTH: f32 = 0.9;
pub const SHADE_EAST_WEST: f32 = 0.8;
pub const SHADE_BOTTOM: f32 = 0.7;

/// How opaque the clouds are, where they are not faded out.
pub const OPACITY: f32 = 0.8;

/// The farthest the clouds are drawn, in blocks.
const MAX_REACH: f32 = 1024.;

/// Where the clouds start to fade out, as a part of their reach.
const FADE_START: f32 = 0.6;

/// The tint of the clouds by day, at night, and when the sun is near the horizon, in sRGB.
const DAY_TINT: Vec3 = Vec3::ONE;
const NIGHT_TINT: Vec3 = Vec3::new(0.17, 0.2, 0.29);
const WARM_TINT: Vec3 = Vec3::new(1.0, 0.7, 0.5);

/// The clouds of the sky: one mesh of the cells within reach of the camera, slid along +X by
/// the drift.
#[derive(Component, Default)]
pub struct Clouds {
    /// The centre cell and the radius, in cells, of the window the mesh holds.
    window: Option<(IVec2, i32)>,
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
    let height = (Quat::from_rotation_x(sun_angle(tick)) * Vec3::Z).y;
    let day = smoothstep(-0.2, 0.3, height);
    // the most warmth at the horizon, none once the sun is a third of the way up or down
    let warmth = 0.7 * (1. - height.abs() / 0.35).max(0.);
    let tint = NIGHT_TINT.lerp(DAY_TINT, day).lerp(WARM_TINT, warmth);
    Color::srgb(tint.x, tint.y, tint.z)
}

fn smoothstep(from: f32, to: f32, x: f32) -> f32 {
    let t = ((x - from) / (to - from)).clamp(0., 1.);
    t * t * (3. - 2. * t)
}

/// Spawns the clouds, drawn in colour, with a child that draws their depth first (see
/// `CloudMaterial`).
fn spawn_clouds(mut commands: Commands, mut materials: ResMut<Assets<CloudMaterial>>) {
    commands
        .spawn((
            Name::new("Clouds"),
            Clouds::default(),
            MeshMaterial3d(materials.add(CloudMaterial::default())),
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
/// another cell of the pattern, or the reach has changed.
fn rebuild_clouds_system(
    cameras: Query<&Transform, (With<Camera3d>, Without<Clouds>)>,
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
    for (entity, mut clouds, transform, children) in &mut clouds {
        let window = (
            camera_cell(camera.translation, transform.translation.x),
            radius,
        );
        if clouds.window == Some(window) {
            continue;
        }
        clouds.window = Some(window);
        let mesh = cloud_mesh(CloudStyle::Fancy, window.0, window.1).map(|m| meshes.add(m));
        // the depth is drawn with the same mesh
        for drawing in children.iter().chain([entity]) {
            match &mesh {
                Some(mesh) => commands.entity(drawing).insert(Mesh3d(mesh.clone())),
                None => commands.entity(drawing).remove::<Mesh3d>(),
            };
        }
    }
}

/// Gives the materials of the clouds the tint of the hour and the fade of the reach.
fn cloud_uniforms_system(
    time: Res<ClientTime>,
    render_distance: Res<RenderDistance>,
    server_limit: Option<Res<ServerRenderDistanceLimit>>,
    clouds: Query<&MeshMaterial3d<CloudMaterial>>,
    mut materials: ResMut<Assets<CloudMaterial>>,
) {
    let reach = current_reach(&render_distance, server_limit);
    let params = CloudParams {
        tint: cloud_tint(time.0).to_linear().to_vec4(),
        opacity: OPACITY,
        fade_start: FADE_START * reach,
        fade_end: reach,
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
    use bevy::state::app::StatesPlugin;
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

    #[test]
    fn the_uniforms_follow_the_reach_and_the_hour() {
        let mut app = app();
        app.insert_resource(ClientTime(NOON_IN_DAY));
        app.update();
        let clouds = clouds(&mut app).unwrap();
        let handle = app
            .world()
            .get::<MeshMaterial3d<CloudMaterial>>(clouds)
            .unwrap()
            .0
            .clone();
        let params = app
            .world()
            .resource::<Assets<CloudMaterial>>()
            .get(&handle)
            .unwrap()
            .params;
        let reach = reach(4, None);
        assert_eq!(params.fade_end, reach);
        assert_eq!(params.fade_start, FADE_START * reach);
        assert_eq!(params.opacity, OPACITY);
        assert_eq!(params.tint, cloud_tint(NOON_IN_DAY).to_linear().to_vec4());
    }
}
