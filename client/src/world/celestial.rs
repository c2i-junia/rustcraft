use crate::player::CurrentPlayerMarker;
use crate::state::GameState;
use crate::world::time::ClientTime;
use crate::world::{GraphicsSettings, ShaderQuality};
use bevy::{
    asset::{load_internal_asset, uuid_handle},
    camera::visibility::NoFrustumCulling,
    light::{NotShadowCaster, NotShadowReceiver},
    mesh::MeshVertexBufferLayoutRef,
    pbr::{MaterialPipeline, MaterialPipelineKey},
    prelude::*,
    render::render_resource::{
        AsBindGroup, RenderPipelineDescriptor, ShaderType, SpecializedMeshPipelineError,
    },
    shader::ShaderRef,
};
use shared::DAY_DURATION_IN_TICKS;
use std::f32::consts::PI;

/// The anchor of the sun, the moon and the stars: rotating it moves them across the sky.
#[derive(Component)]
pub struct CelestialRoot;

/// The sun, drawn in the sky.
#[derive(Component)]
pub struct SunLight;

/// The moon, drawn in the sky opposite the sun.
#[derive(Component)]
pub struct MoonLight;

/// The stars, drawn all around the sky, which turn with it.
#[derive(Component)]
pub struct Stars;

/// The light of the sky: that of the sun by day and of the moon by night, from where it is.
/// One light, whose shadows are never turned off and on with the time of day: Bevy 0.19 no
/// longer draws the shadows of the meshes it had once drawn when they are turned on again.
#[derive(Component)]
pub struct SkyLight;

const SUN_ILLUMINANCE: f32 = 5000.;
const MOON_ILLUMINANCE: f32 = 500.;
const MOON_LIGHT_COLOR: Color = Color::srgb(0.788, 0.824, 0.871);
/// How high above the horizon the sun or the moon gives its full light, as the sine of its
/// height: lower, it fades out, to nothing on the horizon, where the other one takes over.
const HORIZON_FADE: f32 = 0.1;
/// How far the path of the sun and of the moon leans from the vertical, toward -X, as in the
/// shader packs: never right overhead, they cast shadows at noon too.
const PATH_TILT_DEGREES: f32 = 30.;
/// How far the sun, the moon and the stars are drawn from the player. Drawn on the far plane,
/// they are behind everything whatever this is; far, they are sorted behind the clouds.
const CELESTIAL_DISTANCE: f32 = 500.;
/// The angular radius of the disk of the sun and of the moon, in radians: ten times as large
/// as the real ones, as games draw them.
const SUN_RADIUS: f32 = 0.045;
const MOON_RADIUS: f32 = 0.035;
/// How much brighter the sun, the moon and the stars are drawn with the physical sky, whose
/// tonemapping dims what is as bright as white.
const PHYSICAL_SKY_INTENSITY: f32 = 4.;
/// How many days the moon takes to wax and wane.
const MOON_CYCLE_DAYS: u64 = 8;

const CELESTIAL_SHADER: Handle<Shader> = uuid_handle!("6c1e7f52-3a9d-4b20-8e6f-0d4c5b9a2e17");

/// What `celestial.wgsl` draws: the sun, the moon or the stars, added to the sky behind them.
#[derive(Asset, TypePath, AsBindGroup, Clone, Debug)]
pub struct CelestialMaterial {
    #[uniform(0)]
    pub params: CelestialParams,
}

/// `CelestialParams` in `celestial.wgsl`.
#[derive(ShaderType, Clone, Copy, Debug, Default, PartialEq)]
pub struct CelestialParams {
    /// `CelestialKind` as a number.
    pub kind: u32,
    /// From 0 to 1: the full moon at 0, the new moon at 0.5.
    pub phase: f32,
    /// How much of the glow around the sun or the moon is drawn.
    pub glow: f32,
    /// How visible the body is, from 0 to 1.
    pub visibility: f32,
    /// How bright it is drawn.
    pub intensity: f32,
    /// The colour of the sun or of the moon.
    pub color: Vec4,
}

/// What a `CelestialMaterial` draws.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CelestialKind {
    Sun = 0,
    Moon = 1,
    Stars = 2,
}

impl Material for CelestialMaterial {
    fn vertex_shader() -> ShaderRef {
        CELESTIAL_SHADER.into()
    }

    fn fragment_shader() -> ShaderRef {
        CELESTIAL_SHADER.into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Add
    }

    /// The stars first, then the sun and the moon, then whatever else is seen through: the
    /// transparent meshes are drawn from the farthest.
    fn depth_bias(&self) -> f32 {
        if self.params.kind == CelestialKind::Stars as u32 {
            -2e6
        } else {
            -1e6
        }
    }

    fn enable_prepass() -> bool {
        false
    }

    fn enable_shadows() -> bool {
        false
    }

    fn specialize(
        _pipeline: &MaterialPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        layout: &MeshVertexBufferLayoutRef,
        _key: MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        descriptor.vertex.buffers = vec![layout.0.get_layout(&[
            Mesh::ATTRIBUTE_POSITION.at_shader_location(0),
            Mesh::ATTRIBUTE_UV_0.at_shader_location(2),
        ])?];
        // The sphere of the stars is seen from inside
        descriptor.primitive.cull_mode = None;
        Ok(())
    }
}

/// Makes `CelestialMaterial` drawable.
pub fn celestial_material_plugin(app: &mut App) {
    load_internal_asset!(app, CELESTIAL_SHADER, "celestial.wgsl", Shader::from_wgsl);
    app.add_plugins(MaterialPlugin::<CelestialMaterial>::default());
}

/// Spawns the sun and the moon, opposite each other around an anchor attached to the player,
/// the stars around them and the light of the sky: rotating the anchor moves them all across
/// the sky.
pub fn setup_main_lighting(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<CelestialMaterial>>,
    player: Query<Entity, With<CurrentPlayerMarker>>,
    graphics: Option<Res<GraphicsSettings>>,
) {
    let mut material = |kind: CelestialKind| {
        MeshMaterial3d(materials.add(CelestialMaterial {
            params: CelestialParams {
                kind: kind as u32,
                visibility: 1.,
                intensity: 1.,
                color: Vec4::ONE,
                ..default()
            },
        }))
    };
    // A quad facing the player, `CELESTIAL_DISTANCE` away along +Z of its body
    let mut body = |kind: CelestialKind, side: f32, transform: Transform| {
        let quad = meshes.add(Rectangle::new(side, side));
        let child = (
            Mesh3d(quad),
            material(kind),
            Transform::from_translation(Vec3::new(0., 0., CELESTIAL_DISTANCE))
                .looking_at(Vec3::ZERO, Vec3::Y),
            NotShadowCaster,
            NotShadowReceiver,
        );
        commands
            .spawn((transform, Visibility::default()))
            .with_child(child)
            .id()
    };
    // The quad of the sun spans its glow, 8 radii across; that of the moon 4
    let sun_light = body(
        CelestialKind::Sun,
        8. * SUN_RADIUS * CELESTIAL_DISTANCE,
        Transform::IDENTITY,
    );
    let moon_light = body(
        CelestialKind::Moon,
        4. * MOON_RADIUS * CELESTIAL_DISTANCE,
        Transform::from_rotation(Quat::from_rotation_y(PI)),
    );
    commands.entity(sun_light).insert(SunLight);
    commands.entity(moon_light).insert(MoonLight);
    let sphere = meshes.add(Sphere::new(CELESTIAL_DISTANCE).mesh().ico(4).unwrap());
    let stars = commands
        .spawn((
            Stars,
            Mesh3d(sphere),
            material(CelestialKind::Stars),
            Transform::IDENTITY,
            NotShadowCaster,
            NotShadowReceiver,
            NoFrustumCulling,
        ))
        .id();
    let sky_light = commands
        .spawn((
            SkyLight,
            DirectionalLight {
                illuminance: SUN_ILLUMINANCE,
                shadow_maps_enabled: graphics.is_none_or(|graphics| graphics.shadows),
                ..default()
            },
            Transform::IDENTITY,
        ))
        .id();

    let celestial_root = commands
        .spawn((
            CelestialRoot,
            DespawnOnExit(GameState::Game),
            Transform::default(),
            Visibility::default(),
        ))
        .add_children(&[sun_light, moon_light, stars, sky_light])
        .id();

    // The sky follows the player, so that the sun and the moon stay at the same distance
    match player.single() {
        Ok(player) => {
            commands.entity(player).add_child(celestial_root);
        }
        Err(_) => warn!("No local player to attach the sky to: it stays at the origin"),
    }
}

/// How the sky is turned at the world clock `tick`: the sun is at `sky_rotation(tick) *
/// Vec3::Z` from the player, the moon opposite. It turns around X, on a path leaning toward -X.
pub fn sky_rotation(tick: u64) -> Quat {
    let normalized_time = (tick % DAY_DURATION_IN_TICKS) as f32 / DAY_DURATION_IN_TICKS as f32;
    Quat::from_rotation_z(PATH_TILT_DEGREES.to_radians())
        * Quat::from_rotation_x(normalized_time * 2.0 * PI)
}

/// How high the sun is at the world clock `tick`: highest at noon, 0 on the horizon, lowest at
/// midnight, as the sine of its height.
pub fn sun_height(tick: u64) -> f32 {
    (sky_rotation(tick) * Vec3::Z).y
}

/// 0 up to `from`, 1 from `to`, and a smooth curve between.
pub fn smoothstep(from: f32, to: f32, x: f32) -> f32 {
    let t = ((x - from) / (to - from)).clamp(0., 1.);
    t * t * (3. - 2. * t)
}

pub fn update_celestial_bodies(
    mut query: Query<&mut Transform, With<CelestialRoot>>,
    time: Res<ClientTime>,
) {
    for mut tr in query.iter_mut() {
        tr.rotation = sky_rotation(time.0);
    }
}

/// The phase of the moon on the day of the world clock `tick`, from 0 to 1: full on the
/// first day, new halfway through its cycle.
pub fn moon_phase(tick: u64) -> f32 {
    let day = tick / DAY_DURATION_IN_TICKS;
    (day % MOON_CYCLE_DAYS) as f32 / MOON_CYCLE_DAYS as f32
}

/// The colour of the sun at the height `height`, as the sine of its height: white high up,
/// orange then red as the air it shines through thickens near the horizon.
fn sun_color(height: f32) -> Vec3 {
    let low = 1. - smoothstep(0.0, 0.3, height);
    Vec3::new(1.0, 0.95, 0.86).lerp(Vec3::new(1.0, 0.45, 0.18), low)
}

/// Colours the sun, phases the moon and shows the stars at night. The physical sky of the
/// shaders draws a glow of its own around the sun and the moon.
pub fn update_celestial_materials(
    time: Res<ClientTime>,
    graphics: Res<GraphicsSettings>,
    bodies: Query<(&MeshMaterial3d<CelestialMaterial>, Has<Stars>)>,
    mut materials: ResMut<Assets<CelestialMaterial>>,
) {
    let height = sun_height(time.0);
    let physical_sky = graphics.shaders != ShaderQuality::Off;
    for (handle, stars) in &bodies {
        let Some(material) = materials.get(handle) else {
            continue;
        };
        let mut params = material.params;
        // The tonemapping of the physical sky dims what is as bright as white
        params.intensity = if physical_sky {
            PHYSICAL_SKY_INTENSITY
        } else {
            1.
        };
        if stars {
            params.visibility = 1. - smoothstep(-0.25, 0.05, height);
        } else if params.kind == CelestialKind::Sun as u32 {
            params.color = sun_color(height).extend(1.);
            params.glow = if physical_sky { 0.3 } else { 1. };
        } else {
            params.color = Vec4::new(0.86, 0.9, 1.0, 1.0);
            params.phase = moon_phase(time.0);
            params.glow = if physical_sky { 0.5 } else { 1. };
        }
        if params != material.params {
            if let Some(mut material) = materials.get_mut(handle) {
                material.params = params;
            }
        }
    }
}

/// Lights the world from the sun while it is above the horizon, and from the moon otherwise,
/// fading out as either nears the horizon: a light from under the ground would light the faces
/// turned down through the ground. The shadows are drawn if the settings say so.
pub fn celestial_light_system(
    roots: Query<(&Transform, &Children), With<CelestialRoot>>,
    mut lights: Query<
        (&mut Transform, &mut DirectionalLight),
        (With<SkyLight>, Without<CelestialRoot>),
    >,
    graphics: Option<Res<GraphicsSettings>>,
) {
    let shadows = graphics.is_none_or(|graphics| graphics.shadows);
    for (root, children) in &roots {
        for child in children {
            let Ok((mut transform, mut light)) = lights.get_mut(*child) else {
                continue;
            };
            // The sun is where the light it gives comes from, `Vec3::Z` of the anchor
            let sun_height = (root.rotation * Vec3::Z).y;
            let (rotation, illuminance, color) = if sun_height >= 0. {
                (Quat::IDENTITY, SUN_ILLUMINANCE, Color::WHITE)
            } else {
                (
                    Quat::from_rotation_y(PI),
                    MOON_ILLUMINANCE,
                    MOON_LIGHT_COLOR,
                )
            };
            let fade = (sun_height.abs() / HORIZON_FADE).clamp(0., 1.);
            transform.rotation = rotation;
            light.illuminance = illuminance * fade * fade * (3. - 2. * fade);
            light.color = color;
            if light.shadow_maps_enabled != shadows {
                light.shadow_maps_enabled = shadows;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;
    use shared::{MIDNIGHT_IN_DAY, NOON_IN_DAY};

    /// How high the sun is at noon, as the sine of its height: the tilt of its path keeps it from
    /// being right overhead.
    fn noon_sun_height() -> f32 {
        PATH_TILT_DEGREES.to_radians().cos()
    }

    /// The light of the sky at `tick`: where it shines toward, how bright it is and whether it
    /// casts shadows, with the shadows on in the settings if `shadows`.
    fn sky_light_at(tick: u64, shadows: bool) -> (Vec3, f32, bool) {
        let mut world = World::new();
        world.insert_resource(ClientTime(tick));
        world.insert_resource(GraphicsSettings {
            shadows,
            ..default()
        });
        let light = world
            .spawn((SkyLight, DirectionalLight::default(), Transform::IDENTITY))
            .id();
        let root = world
            .spawn((CelestialRoot, Transform::default()))
            .add_child(light)
            .id();
        world.run_system_once(update_celestial_bodies).unwrap();
        world.run_system_once(celestial_light_system).unwrap();
        let root = world.get::<Transform>(root).unwrap().rotation;
        let local = world.get::<Transform>(light).unwrap().rotation;
        let light = world.get::<DirectionalLight>(light).unwrap();
        (
            root * local * Vec3::NEG_Z,
            light.illuminance,
            light.shadow_maps_enabled,
        )
    }

    #[test]
    fn the_sky_shines_down_from_the_sun_by_day_and_the_moon_by_night() {
        let (toward, illuminance, shadows) = sky_light_at(NOON_IN_DAY, true);
        assert!((toward.y + noon_sun_height()).abs() < 1e-4, "{toward}");
        assert_eq!((illuminance, shadows), (SUN_ILLUMINANCE, true));
        let (toward, illuminance, shadows) = sky_light_at(MIDNIGHT_IN_DAY, true);
        assert!((toward.y + noon_sun_height()).abs() < 1e-4, "{toward}");
        assert_eq!((illuminance, shadows), (MOON_ILLUMINANCE, true));
        // Every hour, the light comes from above, if any
        for hour in 0..24 {
            let (toward, illuminance, _) = sky_light_at(DAY_DURATION_IN_TICKS * hour / 24, true);
            assert!(toward.y <= 0. || illuminance == 0., "hour {hour}: {toward}");
        }
        assert!(!sky_light_at(NOON_IN_DAY, false).2);
    }

    #[test]
    fn the_light_fades_out_at_the_horizon() {
        // At sunrise, the sun is on the horizon
        let sunrise = (0..DAY_DURATION_IN_TICKS)
            .step_by(100)
            .min_by_key(|tick| {
                let (toward, _, _) = sky_light_at(*tick, true);
                (toward.y.abs() * 1e6) as u64
            })
            .unwrap();
        assert!(sky_light_at(sunrise, true).1 < SUN_ILLUMINANCE * 0.01);
    }

    /// Where the sun is drawn, relative to the player, at `tick`.
    fn sun_position_at(tick: u64) -> Vec3 {
        let mut app = App::new();
        app.insert_resource(ClientTime(tick))
            .add_systems(Update, update_celestial_bodies);
        let root = app
            .world_mut()
            .spawn((CelestialRoot, Transform::default()))
            .id();
        app.update();
        let rotation = app.world().get::<Transform>(root).unwrap().rotation;
        rotation * Vec3::new(0., 0., CELESTIAL_DISTANCE)
    }

    #[test]
    fn the_sun_is_at_its_highest_at_noon_and_lowest_at_midnight() {
        let noon = sun_position_at(5 * DAY_DURATION_IN_TICKS + NOON_IN_DAY);
        let highest = CELESTIAL_DISTANCE * noon_sun_height();
        assert!((noon.y - highest).abs() < 1e-2, "{noon}");
        // Leaning toward -X, not right overhead
        assert!(noon.x < -CELESTIAL_DISTANCE * 0.4, "{noon}");
        let midnight = sun_position_at(5 * DAY_DURATION_IN_TICKS + MIDNIGHT_IN_DAY);
        assert!((midnight.y + highest).abs() < 1e-2, "{midnight}");
        // An hour on either side of noon, it is lower
        for hour in [-1, 1] {
            let tick = (5 * DAY_DURATION_IN_TICKS + NOON_IN_DAY) as i64
                + hour * DAY_DURATION_IN_TICKS as i64 / 24;
            assert!(sun_position_at(tick as u64).y < noon.y - 1.);
        }
    }

    #[test]
    fn the_moon_waxes_and_wanes_over_its_cycle() {
        assert_eq!(moon_phase(NOON_IN_DAY), 0.);
        assert_eq!(moon_phase(4 * DAY_DURATION_IN_TICKS + 5), 0.5);
        assert_eq!(moon_phase(MOON_CYCLE_DAYS * DAY_DURATION_IN_TICKS), 0.);
    }

    #[test]
    fn the_sun_reddens_near_the_horizon() {
        let high = sun_color(noon_sun_height());
        let low = sun_color(0.02);
        assert!(high.z > 0.8, "{high}");
        assert!(low.z < 0.3 && low.x == 1., "{low}");
    }
}
