//! A proof of concept of a shader-pack-grade look, made of Bevy's own effects and of the chunk
//! shader: turned on by the environment variable `RUSTCRAFT_SHADERS`, a list of features
//! separated by commas, or `all`. Nothing of it is saved or shown in the settings.
//!
//! - `hdr`: a 16-bit float target, tonemapped once at the end (`RUSTCRAFT_TONEMAP`).
//! - `atmosphere`: a physical sky lit by the sun, which also lights the world from every
//!   direction (an environment map made from it every frame) and colours the fog of the chunks.
//! - `bloom`: what is brighter than white bleeds around it.
//! - `taa`: temporal anti-aliasing, instead of MSAA.
//! - `ssao`: ambient occlusion in screen space, over the voxel ambient occlusion.
//! - `godrays`: a thin volumetric fog lit by the sun through the shadow maps.
//! - `water`: the water reflects what is on screen and refracts and absorbs what lies under it.
//! - `tilt`: the path of the sun leans south, as shader packs do: it is never right overhead.
//! - `shadows`: sharper and longer shadows, filtered over frames.

use bevy::anti_alias::smaa::Smaa;
use bevy::anti_alias::taa::TemporalAntiAliasing;
use bevy::asset::{load_internal_asset, uuid_handle, RenderAssetUsages};
use bevy::camera::{Exposure, Hdr};
use bevy::core_pipeline::prepass::DepthPrepass;
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::light::atmosphere::ScatteringMedium;
use bevy::light::{
    Atmosphere, AtmosphereEnvironmentMapLight, CascadeShadowConfigBuilder,
    DirectionalLightShadowMap, FogVolume, ShadowFilteringMethod, SunDisk, VolumetricFog,
    VolumetricLight,
};
use bevy::mesh::{Indices, MeshVertexBufferLayoutRef, PrimitiveTopology};
use bevy::pbr::{AtmosphereSettings, ScreenSpaceAmbientOcclusion};
use bevy::pbr::{MaterialPipeline, MaterialPipelineKey};
use bevy::post_process::bloom::Bloom;
use bevy::prelude::*;
use bevy::render::render_resource::{
    AsBindGroup, RenderPipelineDescriptor, SpecializedMeshPipelineError,
};
use bevy::render::view::{ColorGrading, ColorGradingGlobal};
use bevy::shader::ShaderRef;

use crate::camera::underwater_fog_system;
use crate::world::celestial::{
    celestial_light_system, update_celestial_bodies, CelestialRoot, SkyLight, SunLight,
};
use crate::world::clouds::cloud_uniforms_system;
use crate::world::clouds::material::CloudMaterial;
use crate::world::{
    daylight_ambient_system, ChunkMaterial, GlobalMaterial, MaterialResource, RenderDistance,
    BLOCK_LIGHT,
};
use shared::world::terrain::SEA_LEVEL;

/// The features of the proof of concept that are turned on.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq)]
pub struct ShaderPoc {
    pub hdr: bool,
    pub atmosphere: bool,
    pub bloom: bool,
    pub taa: bool,
    pub ssao: bool,
    pub godrays: bool,
    pub water: bool,
    pub tilt: bool,
    pub shadows: bool,
}

/// The exposure of the camera without the proof of concept, `Exposure::BLENDER`.
const BASE_EV100: f32 = 9.7;
/// The exposure with the physical sky by day, in the golden hour and by night, as the eye
/// adapts: keyframed on the height of the sun as Vibrant Visuals does.
const DAY_EV100: f32 = 13.;
const GOLDEN_EV100: f32 = 11.8;
const NIGHT_EV100: f32 = 9.4;
/// The light of the sun before the atmosphere filters it, as the physical sky expects.
const RAW_SUNLIGHT: f32 = 130_000.;
/// How much brighter the clouds are drawn than their tint, which was made for a sky of at
/// most white: the physical sky is brighter.
const CLOUD_BRIGHTNESS: f32 = 2.2;
/// How far the path of the sun leans from the vertical, toward the south.
const SUN_TILT_DEGREES: f32 = 30.;
/// The bit of `ChunkShading::effects` by which the water reflects the screen and refracts what
/// lies under it (`chunk.wgsl`).
pub const EFFECT_SSR: u32 = 4;

impl ShaderPoc {
    /// The features named in `list`, separated by commas: `all` names them all.
    pub fn parse(list: &str) -> Self {
        let mut poc = Self::default();
        for name in list.split(',').map(str::trim) {
            match name {
                "all" => {
                    poc = Self {
                        hdr: true,
                        atmosphere: true,
                        bloom: true,
                        taa: true,
                        ssao: true,
                        godrays: true,
                        water: true,
                        tilt: true,
                        shadows: true,
                    }
                }
                "hdr" => poc.hdr = true,
                "atmosphere" => poc.atmosphere = true,
                "bloom" => poc.bloom = true,
                "taa" => poc.taa = true,
                "ssao" => poc.ssao = true,
                "godrays" => poc.godrays = true,
                "water" => poc.water = true,
                "tilt" => poc.tilt = true,
                "shadows" => poc.shadows = true,
                "" => {}
                other => warn!("RUSTCRAFT_SHADERS: no feature called {other:?}"),
            }
        }
        poc
    }

    pub fn from_env() -> Self {
        std::env::var("RUSTCRAFT_SHADERS")
            .map(|list| Self::parse(&list))
            .unwrap_or_default()
    }

    /// Whether the scene is drawn in a 16-bit float target: the physical sky and the bloom
    /// need it.
    fn hdr(&self) -> bool {
        self.hdr || self.atmosphere || self.bloom
    }

    /// Whether MSAA is off: TAA and SSAO need it.
    fn msaa_off(&self) -> bool {
        self.taa || self.ssao
    }

    /// The exposure of the camera with the sun at `sun_height`, from -1 to 1.
    fn ev100(&self, sun_height: f32) -> f32 {
        if !self.atmosphere {
            return BASE_EV100;
        }
        if sun_height >= 0. {
            GOLDEN_EV100.lerp(DAY_EV100, (sun_height / 0.3).clamp(0., 1.))
        } else {
            GOLDEN_EV100.lerp(NIGHT_EV100, (-sun_height / 0.2).clamp(0., 1.))
        }
    }
}

/// How much brighter the lights must be for the scene to look as bright at `ev100`.
fn light_scale(ev100: f32) -> f32 {
    2f32.powf(ev100 - BASE_EV100)
}

/// The tonemapping that `RUSTCRAFT_TONEMAP` names, Tony McMapface by default.
fn tonemapping() -> Tonemapping {
    match std::env::var("RUSTCRAFT_TONEMAP").as_deref() {
        Ok("agx") => Tonemapping::AgX,
        Ok("aces") => Tonemapping::AcesFitted,
        Ok("blender") => Tonemapping::BlenderFilmic,
        Ok("khronos") => Tonemapping::KhronosPbrNeutral,
        _ => Tonemapping::TonyMcMapface,
    }
}

const HORIZON_SHADER: Handle<Shader> = uuid_handle!("b6a2d0f4-91c3-4e57-8d1a-5c7e2f90a3b4");

pub fn shader_poc_plugin(app: &mut App) {
    load_internal_asset!(app, HORIZON_SHADER, "horizon.wgsl", Shader::from_wgsl);
    app.add_plugins(MaterialPlugin::<HorizonMaterial>::default());
    // Set beforehand by the benchmark, read from the environment otherwise
    let poc = match app.world().get_resource::<ShaderPoc>() {
        Some(poc) => *poc,
        None => ShaderPoc::from_env(),
    };
    app.insert_resource(poc);
    // The time each pass takes on the GPU, for the benchmark of `e2e::shaders`
    if std::env::var_os("RUSTCRAFT_GPU_TIMINGS").is_some() {
        app.add_plugins(bevy::render::diagnostic::RenderDiagnosticsPlugin);
    }
    if poc == ShaderPoc::default() {
        return;
    }
    if poc.shadows {
        app.insert_resource(DirectionalLightShadowMap { size: 4096 });
    }
    app.add_systems(
        Update,
        (
            set_up_cameras,
            set_up_lights,
            spawn_atmosphere,
            follow_camera_with_fog_volume,
            follow_camera_with_horizon,
            tune_chunk_materials,
            sky_fog.after(underwater_fog_system),
            tilt_sun_path
                .after(update_celestial_bodies)
                .before(celestial_light_system),
            brighten_clouds.after(cloud_uniforms_system),
            scale_sky_light
                .after(celestial_light_system)
                .after(daylight_ambient_system),
        ),
    );
}

/// Gives the cameras of the world the effects turned on, once.
fn set_up_cameras(
    poc: Res<ShaderPoc>,
    cameras: Query<Entity, (With<Camera3d>, Without<ShaderPocCamera>)>,
    mut commands: Commands,
) {
    for camera in &cameras {
        let mut entity = commands.entity(camera);
        entity.insert(ShaderPocCamera);
        if poc.hdr() {
            entity.insert((Hdr, tonemapping()));
        }
        if poc.atmosphere {
            // A touch more vivid, as the packs grade their pictures
            entity.insert(ColorGrading {
                global: ColorGradingGlobal {
                    post_saturation: 1.12,
                    ..default()
                },
                ..default()
            });
        }
        if poc.msaa_off() {
            entity.insert(Msaa::Off);
        }
        if poc.taa {
            entity.insert(TemporalAntiAliasing::default());
        } else if poc.msaa_off() {
            entity.insert(Smaa::default());
        }
        if poc.shadows {
            entity.insert(if poc.taa {
                ShadowFilteringMethod::Temporal
            } else {
                ShadowFilteringMethod::Gaussian
            });
        }
        if poc.bloom {
            entity.insert(Bloom::NATURAL);
        }
        if poc.ssao {
            entity.insert(ScreenSpaceAmbientOcclusion::default());
        }
        if poc.atmosphere {
            entity.insert((
                AtmosphereSettings::default(),
                AtmosphereEnvironmentMapLight::default(),
            ));
        }
        if poc.godrays {
            entity.insert(VolumetricFog {
                ambient_intensity: 0.,
                step_count: 32,
                jitter: if poc.taa { 0.5 } else { 0. },
                ..default()
            });
        }
        if poc.water {
            entity.insert(DepthPrepass);
        }
    }
}

/// A camera given the effects of the proof of concept.
#[derive(Component)]
struct ShaderPocCamera;

/// The light of the sky casts god rays and longer shadows; the sun is drawn by the atmosphere.
fn set_up_lights(
    poc: Res<ShaderPoc>,
    lights: Query<Entity, (With<SkyLight>, Without<VolumetricLight>, Without<SunSetUp>)>,
    suns: Query<Entity, With<SunLight>>,
    mut commands: Commands,
) {
    for light in &lights {
        let mut entity = commands.entity(light);
        entity.insert(SunSetUp);
        if poc.godrays {
            entity.insert(VolumetricLight);
        }
        if poc.shadows {
            entity.insert(
                CascadeShadowConfigBuilder {
                    num_cascades: 4,
                    first_cascade_far_bound: 12.,
                    maximum_distance: 192.,
                    ..default()
                }
                .build(),
            );
        }
        if poc.atmosphere {
            for sun in &suns {
                commands.entity(sun).insert(Visibility::Hidden);
            }
        }
    }
}

#[derive(Component)]
struct SunSetUp;

/// The planet under the world, once: its ground is at y = 0.
fn spawn_atmosphere(
    poc: Res<ShaderPoc>,
    atmospheres: Query<(), With<Atmosphere>>,
    mut media: ResMut<Assets<ScatteringMedium>>,
    mut commands: Commands,
) {
    if poc.atmosphere && atmospheres.is_empty() {
        // The ground of the planet shows under the horizon past the chunks: dark and green, it
        // reads as land far off in the haze
        commands.spawn(Atmosphere {
            ground_albedo: Vec3::new(0.05, 0.08, 0.045),
            ..Atmosphere::earth(media.add(ScatteringMedium::earth(256, 256)))
        });
    }
}

/// The fog lit by the sun, a box around the camera.
#[derive(Component)]
struct GodRayVolume;

fn follow_camera_with_fog_volume(
    poc: Res<ShaderPoc>,
    cameras: Query<&GlobalTransform, With<ShaderPocCamera>>,
    roots: Query<&Transform, (With<CelestialRoot>, Without<GodRayVolume>)>,
    mut volumes: Query<(&mut Transform, &mut FogVolume), With<GodRayVolume>>,
    mut commands: Commands,
) {
    if !poc.godrays {
        return;
    }
    let Some(camera) = cameras.iter().next() else {
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
        Ok((mut transform, mut volume)) => {
            transform.translation = at;
            volume.density_factor = density;
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
            ));
        }
    }
}

/// The light of the blocks is as bright at the exposure of the proof of concept, and the
/// water reflects the screen.
fn tune_chunk_materials(
    poc: Res<ShaderPoc>,
    cameras: Query<&Exposure, With<ShaderPocCamera>>,
    materials: Res<MaterialResource>,
    mut chunk_materials: ResMut<Assets<ChunkMaterial>>,
) {
    let ev100 = cameras
        .iter()
        .next()
        .map_or(BASE_EV100, |exposure| exposure.ev100);
    // By steps of a third of a stop: the material is sent again on each change
    let block_light = BLOCK_LIGHT * light_scale((ev100 * 3.).round() / 3.);
    for (kind, handle) in &materials.chunk_materials {
        let Some(material) = chunk_materials.get(handle) else {
            continue;
        };
        let water = poc.water && *kind == GlobalMaterial::Liquids;
        let effects = if water {
            material.extension.shading.effects | EFFECT_SSR
        } else {
            material.extension.shading.effects
        };
        let alpha_mode = if water {
            AlphaMode::Opaque
        } else {
            material.base.alpha_mode
        };
        if material.extension.shading.block_light == block_light
            && material.extension.shading.effects == effects
            && material.base.alpha_mode == alpha_mode
        {
            continue;
        }
        let mut material = chunk_materials.get_mut(handle).unwrap();
        material.extension.shading.block_light = block_light;
        material.extension.shading.effects = effects;
        if water {
            // Opaque, the water is drawn after the other opaque faces, with a copy of them
            // (`specular_transmission > 0`), and stays out of the depth prepass
            material.base.alpha_mode = AlphaMode::Opaque;
            material.base.specular_transmission = 1.;
        }
    }
}

/// Above the water, the chunks fade into the sky itself, which `chunk.wgsl` reads from the
/// environment map of the atmosphere where the fog is transparent; the atmosphere draws the
/// sky over a black background.
fn sky_fog(poc: Res<ShaderPoc>, mut cameras: Query<(&mut Camera, &mut DistanceFog)>) {
    if !poc.atmosphere {
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

/// Leans the path of the sun and the moon toward the south.
fn tilt_sun_path(poc: Res<ShaderPoc>, mut roots: Query<&mut Transform, With<CelestialRoot>>) {
    if !poc.tilt {
        return;
    }
    for mut root in &mut roots {
        root.rotation = Quat::from_rotation_z(SUN_TILT_DEGREES.to_radians()) * root.rotation;
    }
}

/// The sun is as bright as it is above the atmosphere, which filters it, and the moon as bright
/// on screen as without the proof of concept; the exposure follows the height of the sun, and
/// the atmosphere draws the disk of the sun, not of the moon.
fn scale_sky_light(
    poc: Res<ShaderPoc>,
    roots: Query<&Transform, With<CelestialRoot>>,
    mut lights: Query<(Entity, &mut DirectionalLight), With<SkyLight>>,
    mut cameras: Query<&mut Exposure, With<ShaderPocCamera>>,
    mut ambient: ResMut<GlobalAmbientLight>,
    mut commands: Commands,
    mut was_day: Local<Option<bool>>,
) {
    let Some(root) = roots.iter().next() else {
        return;
    };
    let sun_height = (root.rotation * Vec3::Z).y;
    let ev100 = poc.ev100(sun_height);
    for mut exposure in &mut cameras {
        if exposure.ev100 != ev100 {
            exposure.ev100 = ev100;
        }
    }
    if !poc.atmosphere {
        return;
    }
    let day = sun_height >= 0.;
    for (entity, mut light) in &mut lights {
        light.illuminance *= if day {
            RAW_SUNLIGHT / 5000.
        } else {
            light_scale(ev100)
        };
        if *was_day != Some(day) {
            commands
                .entity(entity)
                .insert(if day { SunDisk::EARTH } else { SunDisk::OFF });
        }
    }
    *was_day = Some(day);
    // The ambient light of the day comes from the sky; by night, a cool light a bit brighter
    // than without the proof of concept, which the eye sees in the dark
    ambient.brightness = 60. * light_scale(ev100);
    if !day {
        ambient.brightness *= 1.6;
        ambient.color = Color::srgb(0.62, 0.72, 1.);
    }
}

/// The clouds are as bright as the physical sky around them.
fn brighten_clouds(
    poc: Res<ShaderPoc>,
    clouds: Query<&MeshMaterial3d<CloudMaterial>>,
    materials: Option<ResMut<Assets<CloudMaterial>>>,
) {
    let (true, Some(mut materials)) = (poc.atmosphere, materials) else {
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

/// Keeps the horizon skirt around the camera, past the corners of the chunks shown.
fn follow_camera_with_horizon(
    poc: Res<ShaderPoc>,
    render_distance: Res<RenderDistance>,
    cameras: Query<&GlobalTransform, With<ShaderPocCamera>>,
    mut skirts: Query<&mut Transform, With<HorizonSkirt>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<HorizonMaterial>>,
    mut commands: Commands,
) {
    if !poc.atmosphere {
        return;
    }
    let Some(camera) = cameras.iter().next() else {
        return;
    };
    let radius = render_distance.distance as f32 * 16. * 1.5;
    let transform =
        Transform::from_translation(camera.translation()).with_scale(Vec3::splat(radius));
    match skirts.single_mut() {
        Ok(mut skirt) => *skirt = transform,
        Err(_) => {
            commands.spawn((
                HorizonSkirt,
                Mesh3d(meshes.add(horizon_ring())),
                MeshMaterial3d(materials.add(HorizonMaterial::default())),
                transform,
                bevy::light::NotShadowCaster,
                bevy::light::NotShadowReceiver,
                bevy::camera::visibility::NoFrustumCulling,
            ));
        }
    }
}
