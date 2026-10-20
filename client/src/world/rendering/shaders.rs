//! The shaders, a look like the shader packs of Minecraft, made of Bevy's own effects and of the
//! chunk shader, as much of them as the video setting `ShaderQuality` asks for, changed in game
//! as soon as it is set:
//!
//! - a physical sky lit by the sun, in a 16-bit float target tonemapped once at the end, which
//!   also lights the world from every direction (an environment map made from it every frame)
//!   and into which the chunks fade far off; with it, an exposure that follows the height of
//!   the sun, as the eye adapts, and a bloom of what is brighter than white;
//! - SMAA, or temporal anti-aliasing, instead of MSAA;
//! - water that reflects what is on screen and refracts and absorbs what lies under it;
//! - rays of light through a haze over the ground, thicker at dawn and dusk;
//! - ambient occlusion in screen space, over the voxel ambient occlusion;
//! - longer and sharper shadows.
//!
//! `ShaderQuality::Off` draws the world as it was before them.

use bevy::anti_alias::smaa::Smaa;
use bevy::anti_alias::taa::TemporalAntiAliasing;
use bevy::asset::{load_internal_asset, uuid_handle, RenderAssetUsages};
use bevy::camera::{Exposure, Hdr};
use bevy::core_pipeline::prepass::{DepthPrepass, MotionVectorPrepass, NormalPrepass};
use bevy::light::atmosphere::ScatteringMedium;
use bevy::light::{
    Atmosphere, AtmosphereEnvironmentMapLight, CascadeShadowConfig, CascadeShadowConfigBuilder,
    DirectionalLightShadowMap, FogVolume, ShadowFilteringMethod, SunDisk, VolumetricFog,
    VolumetricLight,
};
use bevy::mesh::{Indices, MeshVertexBufferLayoutRef, PrimitiveTopology};
use bevy::pbr::{
    AtmosphereSettings, MaterialPipeline, MaterialPipelineKey, ScreenSpaceAmbientOcclusion,
    ScreenSpaceAmbientOcclusionQualityLevel,
};
use bevy::post_process::bloom::Bloom;
use bevy::prelude::*;
use bevy::render::camera::{MipBias, TemporalJitter};
use bevy::render::render_resource::{
    AsBindGroup, RenderPipelineDescriptor, SpecializedMeshPipelineError,
};
use bevy::render::view::{ColorGrading, ColorGradingGlobal};
use bevy::shader::ShaderRef;

use crate::camera::underwater_fog_system;
use crate::world::celestial::{celestial_light_system, smoothstep, CelestialRoot, SkyLight};
use crate::world::clouds::cloud_uniforms_system;
use crate::world::clouds::material::CloudMaterial;
use crate::world::{
    daylight_ambient_system, ChunkMaterial, GlobalMaterial, GraphicsSettings, MaterialResource,
    RenderDistance, ShaderQuality,
};
use shared::world::terrain::SEA_LEVEL;

/// How the edges of the faces are smoothed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Antialiasing {
    /// Four samples a pixel, as before the shaders.
    Msaa,
    /// Edges found in the picture and smoothed.
    Smaa,
    /// Jittered frames blended over time: smooth edges and stable leaves, at the cost of some
    /// blur in motion.
    Taa,
}

/// What a `ShaderQuality` draws.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShaderFeatures {
    /// The physical sky, the exposure that follows the sun, the 16-bit float target, the
    /// bloom and the grading of the colours.
    pub sky: bool,
    pub antialiasing: Antialiasing,
    /// Whether the water reflects the screen and refracts what lies under it.
    pub water: bool,
    /// How many steps the rays of light are marched in, if they are drawn.
    pub god_rays: Option<u32>,
    pub ambient_occlusion: Option<ScreenSpaceAmbientOcclusionQualityLevel>,
    /// The width of the shadow map, and how far its cascades reach and the first one ends.
    pub shadow_map: usize,
    pub shadow_distance: f32,
    pub first_shadow_cascade: f32,
}

impl ShaderFeatures {
    pub fn of(quality: ShaderQuality) -> Self {
        let off = Self {
            sky: false,
            antialiasing: Antialiasing::Msaa,
            water: false,
            god_rays: None,
            ambient_occlusion: None,
            shadow_map: 2048,
            // Bevy's own cascades
            shadow_distance: 150.,
            first_shadow_cascade: 10.,
        };
        match quality {
            ShaderQuality::Off => off,
            ShaderQuality::Low => Self {
                sky: true,
                antialiasing: Antialiasing::Smaa,
                ..off
            },
            ShaderQuality::Medium => Self {
                sky: true,
                antialiasing: Antialiasing::Taa,
                water: true,
                shadow_distance: 160.,
                first_shadow_cascade: 12.,
                ..off
            },
            ShaderQuality::High => Self {
                sky: true,
                antialiasing: Antialiasing::Taa,
                water: true,
                god_rays: Some(24),
                ambient_occlusion: Some(ScreenSpaceAmbientOcclusionQualityLevel::Medium),
                shadow_map: 2048,
                shadow_distance: 192.,
                first_shadow_cascade: 12.,
            },
            ShaderQuality::Ultra => Self {
                sky: true,
                antialiasing: Antialiasing::Taa,
                water: true,
                god_rays: Some(48),
                ambient_occlusion: Some(ScreenSpaceAmbientOcclusionQualityLevel::High),
                shadow_map: 4096,
                shadow_distance: 192.,
                first_shadow_cascade: 12.,
            },
        }
    }

    /// Whether the depth of the opaque faces is drawn before them: the water, TAA and the
    /// ambient occlusion read it.
    fn depth_prepass(&self) -> bool {
        self.water || self.antialiasing == Antialiasing::Taa || self.ambient_occlusion.is_some()
    }
}

/// The exposure of the camera without the shaders, `Exposure::BLENDER`, for which the lights of
/// the game were made.
const BASE_EV100: f32 = 9.7;
/// The exposure with the physical sky by day, in the golden hour and by night, as the eye
/// adapts: keyframed on the height of the sun as Vibrant Visuals does.
const DAY_EV100: f32 = 13.;
const GOLDEN_EV100: f32 = 11.8;
const NIGHT_EV100: f32 = 9.4;
/// The light of the sun before the atmosphere filters it, as the physical sky expects.
const RAW_SUNLIGHT: f32 = 130_000.;
/// How far under the horizon the sun still lights the sky, as the sine of its height (6°, the
/// civil twilight), and how high above it its light then comes from.
const TWILIGHT: f32 = 0.1;
const TWILIGHT_LIGHT_HEIGHT: f32 = 0.035;
/// How much brighter the clouds are drawn than their tint, which was made for a sky of at
/// most white: the physical sky is brighter.
const CLOUD_BRIGHTNESS: f32 = 2.2;
/// The ambient light with the physical sky, which lights the world itself by day, and the cool
/// light of the night, a bit brighter: the eye sees in the dark.
const SKY_AMBIENT: f32 = 60.;
const NIGHT_AMBIENT_BOOST: f32 = 1.6;
const NIGHT_AMBIENT_COLOR: Color = Color::srgb(0.62, 0.72, 1.);
/// A touch more vivid, as the packs grade their pictures.
const SATURATION: f32 = 1.12;

/// The bit of `ChunkShading::effects` by which the water reflects the screen and refracts what
/// lies under it (`chunk.wgsl`).
pub const EFFECT_SSR: u32 = 4;

/// The exposure of the camera with the physical sky and the sun at `sun_height`, as the sine
/// of its height.
fn sky_ev100(sun_height: f32) -> f32 {
    if sun_height >= 0. {
        GOLDEN_EV100.lerp(DAY_EV100, (sun_height / 0.3).clamp(0., 1.))
    } else {
        // The sky darkens fast once the sun is down
        GOLDEN_EV100.lerp(NIGHT_EV100, (-sun_height / 0.1).clamp(0., 1.))
    }
}

/// How much brighter the lights must be for the scene to look as bright at `ev100`.
fn light_scale(ev100: f32) -> f32 {
    2f32.powf(ev100 - BASE_EV100)
}

const HORIZON_SHADER: Handle<Shader> = uuid_handle!("b6a2d0f4-91c3-4e57-8d1a-5c7e2f90a3b4");

pub fn shaders_plugin(app: &mut App) {
    load_internal_asset!(app, HORIZON_SHADER, "horizon.wgsl", Shader::from_wgsl);
    app.add_plugins(MaterialPlugin::<HorizonMaterial>::default())
        .init_resource::<DirectionalLightShadowMap>();
    // The time each pass takes on the GPU, for `bench_shaders`
    if std::env::var_os("RUSTCRAFT_GPU_TIMINGS").is_some() {
        app.add_plugins(bevy::render::diagnostic::RenderDiagnosticsPlugin);
    }
    app.add_systems(
        Update,
        (
            apply_camera_features,
            apply_light_features,
            apply_atmosphere,
            follow_camera_with_fog_volume,
            follow_camera_with_horizon,
            apply_water_features,
            sky_fog.after(underwater_fog_system),
            brighten_clouds.after(cloud_uniforms_system),
            scale_sky_light
                .after(celestial_light_system)
                .after(daylight_ambient_system),
        ),
    );
}

fn features(graphics: &GraphicsSettings) -> ShaderFeatures {
    ShaderFeatures::of(graphics.shaders)
}

/// Gives the cameras of the world the effects of the quality chosen, when it changes and when
/// they are made.
fn apply_camera_features(
    graphics: Res<GraphicsSettings>,
    all: Query<Entity, With<Camera3d>>,
    added: Query<Entity, Added<Camera3d>>,
    mut applied: Local<Option<ShaderQuality>>,
    mut commands: Commands,
) {
    let cameras: Vec<Entity> = if *applied != Some(graphics.shaders) {
        all.iter().collect()
    } else {
        added.iter().collect()
    };
    *applied = Some(graphics.shaders);
    let features = features(&graphics);
    for camera in cameras {
        let mut entity = commands.entity(camera);
        if features.sky {
            entity.insert((
                Hdr,
                Bloom::NATURAL,
                ColorGrading {
                    global: ColorGradingGlobal {
                        post_saturation: SATURATION,
                        ..default()
                    },
                    ..default()
                },
                AtmosphereSettings::default(),
                AtmosphereEnvironmentMapLight::default(),
            ));
        } else {
            entity
                .remove::<(
                    AtmosphereSettings,
                    AtmosphereEnvironmentMapLight,
                    Bloom,
                    Hdr,
                )>()
                .insert((ColorGrading::default(), Exposure::BLENDER));
        }
        // The prepasses go before the effects that need them, and with them
        entity.remove::<(
            TemporalAntiAliasing,
            TemporalJitter,
            MipBias,
            Smaa,
            ScreenSpaceAmbientOcclusion,
            VolumetricFog,
            DepthPrepass,
            NormalPrepass,
            MotionVectorPrepass,
        )>();
        match features.antialiasing {
            Antialiasing::Msaa => {
                entity.insert((Msaa::Sample4, ShadowFilteringMethod::Gaussian));
            }
            Antialiasing::Smaa => {
                entity.insert((Msaa::Off, Smaa::default(), ShadowFilteringMethod::Gaussian));
            }
            Antialiasing::Taa => {
                entity.insert((
                    Msaa::Off,
                    TemporalAntiAliasing::default(),
                    ShadowFilteringMethod::Temporal,
                ));
            }
        }
        if features.depth_prepass() {
            entity.insert(DepthPrepass);
        }
        if let Some(quality_level) = features.ambient_occlusion {
            entity.insert(ScreenSpaceAmbientOcclusion {
                quality_level,
                ..default()
            });
        }
        if let Some(step_count) = features.god_rays {
            entity.insert(VolumetricFog {
                ambient_intensity: 0.,
                step_count,
                jitter: if features.antialiasing == Antialiasing::Taa {
                    0.5
                } else {
                    0.
                },
                ..default()
            });
        }
    }
}

/// Gives the light of the sky the shadows of the quality chosen, and the rays of light; the
/// physical sky would draw a disk of the sun of its own, but `celestial.rs` draws it.
fn apply_light_features(
    graphics: Res<GraphicsSettings>,
    all: Query<Entity, With<SkyLight>>,
    added: Query<Entity, Added<SkyLight>>,
    mut shadow_map: ResMut<DirectionalLightShadowMap>,
    mut applied: Local<Option<ShaderQuality>>,
    mut commands: Commands,
) {
    let lights: Vec<Entity> = if *applied != Some(graphics.shaders) {
        all.iter().collect()
    } else {
        added.iter().collect()
    };
    *applied = Some(graphics.shaders);
    let features = features(&graphics);
    if shadow_map.size != features.shadow_map {
        shadow_map.size = features.shadow_map;
    }
    for light in lights {
        let mut entity = commands.entity(light);
        let cascades: CascadeShadowConfig = CascadeShadowConfigBuilder {
            maximum_distance: features.shadow_distance,
            first_cascade_far_bound: features.first_shadow_cascade,
            ..default()
        }
        .build();
        entity.insert((cascades, SunDisk::OFF));
        if features.god_rays.is_some() {
            entity.insert(VolumetricLight);
        } else {
            entity.remove::<VolumetricLight>();
        }
    }
}

/// The planet under the world, with the physical sky only: its ground is at y = 0.
fn apply_atmosphere(
    graphics: Res<GraphicsSettings>,
    atmospheres: Query<Entity, With<Atmosphere>>,
    mut media: ResMut<Assets<ScatteringMedium>>,
    mut commands: Commands,
) {
    let sky = features(&graphics).sky;
    if sky && atmospheres.is_empty() {
        // The ground of the planet shows under the horizon past the horizon skirt: dark and
        // green, it reads as land far off in the haze
        commands.spawn((
            Atmosphere {
                ground_albedo: Vec3::new(0.05, 0.08, 0.045),
                ..Atmosphere::earth(media.add(ScatteringMedium::earth(256, 256)))
            },
            DespawnOnExit(crate::state::GameState::Game),
        ));
    } else if !sky {
        for atmosphere in &atmospheres {
            commands.entity(atmosphere).despawn();
        }
    }
}

/// The haze lit by the sun, a layer over the ground around the camera.
#[derive(Component)]
struct GodRayVolume;

fn follow_camera_with_fog_volume(
    graphics: Res<GraphicsSettings>,
    cameras: Query<&GlobalTransform, With<Camera3d>>,
    roots: Query<&Transform, (With<CelestialRoot>, Without<GodRayVolume>)>,
    mut volumes: Query<(Entity, &mut Transform, &mut FogVolume), With<GodRayVolume>>,
    mut commands: Commands,
) {
    let camera = cameras.iter().next();
    let (Some(camera), Some(_)) = (camera, features(&graphics).god_rays) else {
        for (volume, ..) in &volumes {
            commands.entity(volume).despawn();
        }
        return;
    };
    // The haze thickens as the sun goes down, as in the shader packs: rays of light at dawn and
    // dusk, a clear sky at noon
    let sun_height = roots
        .iter()
        .next()
        .map_or(0., |root| (root.rotation * Vec3::Z).y);
    let density = 0.008 * 1f32.lerp(0.12, ((sun_height - 0.1) / 0.4).clamp(0., 1.));
    // A layer of haze over the ground, whose top is not seen against the sky
    let at = camera.translation().with_y(SEA_LEVEL as f32 + 30.);
    let size = Vec3::new(640., 140., 640.);
    match volumes.single_mut() {
        Ok((_, mut transform, mut volume)) => {
            transform.translation = at;
            if volume.density_factor != density {
                volume.density_factor = density;
            }
        }
        Err(_) => {
            commands.spawn((
                GodRayVolume,
                FogVolume {
                    density_factor: density,
                    scattering: 0.6,
                    absorption: 0.1,
                    scattering_asymmetry: 0.7,
                    ..default()
                },
                Transform::from_translation(at).with_scale(size),
                DespawnOnExit(crate::state::GameState::Game),
            ));
        }
    }
}

/// The water reflects the screen and refracts what lies under it, or is blended over it as
/// before the shaders.
fn apply_water_features(
    graphics: Res<GraphicsSettings>,
    materials: Res<MaterialResource>,
    mut chunk_materials: ResMut<Assets<ChunkMaterial>>,
) {
    let water = features(&graphics).water;
    let Some(handle) = materials.chunk_materials.get(&GlobalMaterial::Liquids) else {
        return;
    };
    let Some(material) = chunk_materials.get(handle) else {
        return;
    };
    let effects = if water {
        material.extension.shading.effects | EFFECT_SSR
    } else {
        material.extension.shading.effects & !EFFECT_SSR
    };
    if material.extension.shading.effects == effects {
        return;
    }
    let mut material = chunk_materials.get_mut(handle).unwrap();
    material.extension.shading.effects = effects;
    if water {
        // Opaque, the water is drawn after the other opaque faces, with a copy of them
        // (`specular_transmission > 0`), and stays out of the depth prepass
        material.base.alpha_mode = AlphaMode::Opaque;
        material.base.specular_transmission = 1.;
    } else {
        material.base.alpha_mode = AlphaMode::Blend;
        material.base.specular_transmission = 0.;
    }
}

/// With the physical sky, above the water, the chunks fade into the sky itself, which
/// `chunk.wgsl` reads from the environment map of the atmosphere where the fog is transparent;
/// the atmosphere draws the sky over a black background. `underwater_fog_system` gives them
/// back their colours without it.
fn sky_fog(graphics: Res<GraphicsSettings>, mut cameras: Query<(&mut Camera, &mut DistanceFog)>) {
    if !features(&graphics).sky {
        return;
    }
    for (mut camera, mut fog) in &mut cameras {
        let under_water = matches!(fog.falloff, FogFalloff::Linear { end, .. } if end <= 16.);
        if under_water {
            continue;
        }
        if fog.color.alpha() != 0. {
            fog.color.set_alpha(0.);
        }
        if !matches!(camera.clear_color, ClearColorConfig::Custom(c) if c == Color::BLACK) {
            camera.clear_color = ClearColorConfig::Custom(Color::BLACK);
        }
    }
}

/// With the physical sky, the sun is as bright as it is above the atmosphere, which filters
/// it, and the moon and the ambient light as bright on screen as without it; the exposure
/// follows the height of the sun. The sun still lights the sky a while after it has set, from
/// just above the horizon, so that the sky glows at dusk and dawn; the moon takes over after.
fn scale_sky_light(
    graphics: Res<GraphicsSettings>,
    roots: Query<&Transform, With<CelestialRoot>>,
    mut lights: Query<
        (&mut Transform, &mut DirectionalLight),
        (With<SkyLight>, Without<CelestialRoot>),
    >,
    mut cameras: Query<&mut Exposure, With<Camera3d>>,
    mut ambient: ResMut<GlobalAmbientLight>,
) {
    if !features(&graphics).sky {
        if ambient.color != Color::WHITE {
            ambient.color = Color::WHITE;
        }
        return;
    }
    let Some(root) = roots.iter().next() else {
        return;
    };
    let sun = root.rotation * Vec3::Z;
    let ev100 = sky_ev100(sun.y);
    for mut exposure in &mut cameras {
        if exposure.ev100 != ev100 {
            exposure.ev100 = ev100;
        }
    }
    for (mut transform, mut light) in &mut lights {
        if sun.y > -TWILIGHT {
            // From where the sun is, but never from under the ground
            let from = sun.with_y(sun.y.max(TWILIGHT_LIGHT_HEIGHT)).normalize();
            transform.rotation = Quat::from_rotation_arc(Vec3::Z, root.rotation.inverse() * from);
            light.illuminance = RAW_SUNLIGHT * smoothstep(-TWILIGHT, 0.1, sun.y);
            light.color = Color::WHITE;
        } else {
            light.illuminance *= light_scale(ev100) * smoothstep(TWILIGHT, 2. * TWILIGHT, -sun.y);
        }
    }
    ambient.brightness = SKY_AMBIENT * light_scale(ev100);
    if sun.y >= 0. {
        ambient.color = Color::WHITE;
    } else {
        ambient.brightness *= NIGHT_AMBIENT_BOOST;
        ambient.color = NIGHT_AMBIENT_COLOR;
    }
}

/// With the physical sky, the clouds are as bright as the sky around them.
fn brighten_clouds(
    graphics: Res<GraphicsSettings>,
    clouds: Query<&MeshMaterial3d<CloudMaterial>>,
    materials: Option<ResMut<Assets<CloudMaterial>>>,
) {
    let (true, Some(mut materials)) = (features(&graphics).sky, materials) else {
        return;
    };
    for material in &clouds {
        if let Some(mut material) = materials.get_mut(material) {
            let tint = material.params.tint;
            material.params.tint = (tint.truncate() * CLOUD_BRIGHTNESS).extend(tint.w);
        }
    }
}

/// The ring around the camera, under the horizon and past the chunks, of the colour of the sky
/// at the horizon (`horizon.wgsl`): without it, the ground of the planet of the atmosphere
/// would show there.
#[derive(Asset, TypePath, AsBindGroup, Clone, Debug, Default)]
pub struct HorizonMaterial {}

impl Material for HorizonMaterial {
    fn fragment_shader() -> ShaderRef {
        HORIZON_SHADER.into()
    }

    // Its top fades into the sky drawn by the atmosphere, which is a bit different
    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Blend
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
        _layout: &MeshVertexBufferLayoutRef,
        _key: MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        descriptor.primitive.cull_mode = None;
        Ok(())
    }
}

#[derive(Component)]
struct HorizonSkirt;

/// A ring of radius 1 from y = -3 up to y = 0.15, seen from inside.
fn horizon_ring() -> Mesh {
    const SIDES: u32 = 96;
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut indices = Vec::new();
    for i in 0..=SIDES {
        let angle = i as f32 / SIDES as f32 * std::f32::consts::TAU;
        let (sin, cos) = angle.sin_cos();
        positions.push([cos, 0.15, sin]);
        positions.push([cos, -3., sin]);
        normals.extend([[-cos, 0., -sin]; 2]);
    }
    for i in 0..SIDES {
        let a = i * 2;
        indices.extend([a, a + 1, a + 2, a + 1, a + 3, a + 2]);
    }
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_indices(Indices::U32(indices))
}

/// Keeps the horizon skirt around the camera, past the corners of the chunks shown, with the
/// physical sky only.
fn follow_camera_with_horizon(
    graphics: Res<GraphicsSettings>,
    render_distance: Res<RenderDistance>,
    cameras: Query<&GlobalTransform, With<Camera3d>>,
    mut skirts: Query<(Entity, &mut Transform), With<HorizonSkirt>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<HorizonMaterial>>,
    mut commands: Commands,
) {
    let camera = cameras.iter().next();
    let (Some(camera), true) = (camera, features(&graphics).sky) else {
        for (skirt, _) in &skirts {
            commands.entity(skirt).despawn();
        }
        return;
    };
    let radius = render_distance.distance as f32 * 16. * 1.5;
    let transform =
        Transform::from_translation(camera.translation()).with_scale(Vec3::splat(radius));
    match skirts.single_mut() {
        Ok((_, mut skirt)) => *skirt = transform,
        Err(_) => {
            commands.spawn((
                HorizonSkirt,
                Mesh3d(meshes.add(horizon_ring())),
                MeshMaterial3d(materials.add(HorizonMaterial::default())),
                transform,
                bevy::light::NotShadowCaster,
                bevy::light::NotShadowReceiver,
                bevy::camera::visibility::NoFrustumCulling,
                DespawnOnExit(crate::state::GameState::Game),
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_quality_draws_more_than_the_one_below() {
        let features = ShaderQuality::ALL.map(ShaderFeatures::of);
        let off = features[0];
        assert!(!off.sky && !off.water && off.god_rays.is_none());
        assert_eq!(off.antialiasing, Antialiasing::Msaa);
        assert!(!off.depth_prepass());
        for pair in features.windows(2) {
            let (lower, higher) = (pair[0], pair[1]);
            assert!(higher.sky >= lower.sky);
            assert!(higher.water >= lower.water);
            assert!(higher.god_rays.unwrap_or(0) >= lower.god_rays.unwrap_or(0));
            assert!(higher.shadow_map >= lower.shadow_map);
            assert!(higher.shadow_distance >= lower.shadow_distance);
        }
        let ultra = features[4];
        assert!(ultra.sky && ultra.water && ultra.ambient_occlusion.is_some());
        assert_eq!(ultra.antialiasing, Antialiasing::Taa);
    }

    #[test]
    fn the_exposure_follows_the_sun_without_jumps() {
        assert_eq!(sky_ev100(1.), DAY_EV100);
        assert_eq!(sky_ev100(0.), GOLDEN_EV100);
        assert_eq!(sky_ev100(-1.), NIGHT_EV100);
        let mut before = sky_ev100(-1.);
        for step in -99..=100 {
            let ev100 = sky_ev100(step as f32 / 100.);
            assert!(
                (ev100 - before).abs() < 0.25,
                "{step}: {before} then {ev100}"
            );
            before = ev100;
        }
    }
}
