//! The sky of the weather: how overcast it is, and what that does to the sky, the light and the
//! fog. Every curve gives its clear value exactly at an overcast level of 0, so that clear
//! weather draws as before.
//!
//! What the shaders of the sky know of the weather is in one buffer, `WeatherShading`, bound to
//! every material that shows the sky: the chunks (their fog and the reflection of the water),
//! the horizon skirt and the overcast dome. So the grey of the overcast is the same everywhere,
//! and no blue is left. The buffer is written in place as the weather and the hour change:
//! changing a material instead would prepare every chunk drawn again.

use bevy::asset::{load_internal_asset, uuid_handle};
use bevy::camera::visibility::NoFrustumCulling;
use bevy::light::{NotShadowCaster, NotShadowReceiver};
use bevy::mesh::MeshVertexBufferLayoutRef;
use bevy::pbr::{MaterialPipeline, MaterialPipelineKey};
use bevy::prelude::*;
use bevy::render::render_resource::{
    AsBindGroup, RenderPipelineDescriptor, ShaderType, SpecializedMeshPipelineError,
};
use bevy::render::storage::ShaderBuffer;
use bevy::shader::ShaderRef;

use crate::state::GameState;
use crate::world::celestial::{sky_rotation, smoothstep, sun_height};
use crate::world::clouds::SPEED;
use crate::world::lightning::Lightning;
use crate::world::shaders::ShaderFeatures;
use crate::world::time::ClientTime;
use crate::world::weather::ClientWeather;
use crate::world::{GraphicsSettings, MaterialResource};

/// How overcast the sky is, read from the rain and thunder levels: the sky leads the drops.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Overcast {
    /// From 0, a clear sky, to 1, a sky all grey: `smoothstep(0, 0.5, rain)`.
    pub level: f32,
    /// From 0 to 1, how much darker the storm makes it: `smoothstep(0, 0.5, thunder)`.
    pub storm: f32,
}

/// The values of the curves under a clear sky, a full overcast and a full storm.
///
/// The part of the light of the sun and of the moon that gets through: so little that the
/// shadows are gone, as on an overcast day. By day the sun is five times as bright on screen
/// as the light of the sky, so a quarter of it would still cast hard shadows.
const SUN: [f32; 3] = [1., 0.12, 0.05];
/// The part of the saturation kept: 1.12 becomes 0.9 in rain.
const SATURATION: [f32; 3] = [1., 0.9 / 1.12, 0.78];
/// How bright the light of the overcast sky is on screen, as a part of the grey of the sky
/// (a white face under it is as bright as the sky), with the physical sky and without it.
const PHYSICAL_SKY_LIGHT: f32 = 1.;
const PLAIN_SKY_LIGHT: f32 = 1.6;
/// The colour of the light of the overcast sky: a cool grey.
const AMBIENT_COLOR: Color = Color::srgb(0.88, 0.9, 0.95);
/// Where the fog starts and ends, as parts of the distance to the edge of the chunks shown.
const FOG_START: [f32; 3] = [0.6, 0.3, 0.15];
const FOG_END: [f32; 3] = [1., 0.75, 0.6];
/// The closest the fog ends in rain, in blocks.
const MIN_FOG_END: f32 = 48.;

/// The overcast at the horizon by day, at sunset and by night, in linear RGB: with the
/// physical sky as seen on screen before the tonemapping, and without it as the colour behind
/// the world. The zenith is darker, and a storm darker and cooler.
const PHYSICAL_DAY: Vec3 = Vec3::new(0.62, 0.64, 0.68);
const PHYSICAL_GOLDEN: Vec3 = Vec3::new(0.24, 0.2, 0.18);
const PHYSICAL_NIGHT: Vec3 = Vec3::new(0.017, 0.019, 0.024);
const PLAIN_DAY: Vec3 = Vec3::new(0.3, 0.315, 0.34);
const PLAIN_GOLDEN: Vec3 = Vec3::new(0.09, 0.077, 0.07);
const PLAIN_NIGHT: Vec3 = Vec3::new(0.0043, 0.0048, 0.006);
const ZENITH: Vec3 = Vec3::new(0.7, 0.71, 0.73);
/// What a storm makes of the overcast: darker and cooler, a blue slate, and darker still up
/// high.
const STORM_HORIZON: Vec3 = Vec3::new(0.56, 0.6, 0.67);
const STORM_ZENITH: Vec3 = Vec3::new(0.74, 0.77, 0.82);
/// How fast the wind blows along the drift of the clouds, +X, in blocks per second: as the
/// clouds under a clear sky, then enough to slant the rain, and to lash it in a storm.
const WIND: [f32; 3] = [SPEED, 1.6, 6.5];
/// How much of the light of the overcast sky a storm lets through.
const STORM_SKY_LIGHT: f32 = 0.5;
/// The glow of a sun low on the horizon seeping under the clouds, toward the sun, as a part of
/// the brightness of the overcast at the horizon: a dull orange pink; and how much of it a
/// storm lets through.
const GLOW: Vec3 = Vec3::new(0.75, 0.36, 0.13);
const STORM_GLOW: f32 = 0.45;

impl Overcast {
    pub fn of(weather: &ClientWeather) -> Self {
        Self {
            level: smoothstep(0., 0.5, weather.rain),
            storm: smoothstep(0., 0.5, weather.thunder),
        }
    }

    /// The overcast of `weather`, clear without one (outside of the game, in the tests).
    pub fn current(weather: Option<&ClientWeather>) -> Self {
        weather.map_or_else(Self::default, Self::of)
    }

    /// The value of a curve keyed at `[clear, rain, storm]`: exactly `clear` at a level of 0.
    fn blend(self, [clear, rain, storm]: [f32; 3]) -> f32 {
        clear.lerp(rain, self.level).lerp(storm, self.storm)
    }

    /// The part of the light of the sun and of the moon that gets through the clouds.
    pub fn sun(self) -> f32 {
        self.blend(SUN)
    }

    /// The saturation of the colour grading, `clear` under a clear sky.
    pub fn saturation(self, clear: f32) -> f32 {
        clear * self.blend(SATURATION)
    }

    /// Where the fog ends, as a part of where it ends under a clear sky.
    pub fn fog_end(self) -> f32 {
        self.blend(FOG_END)
    }

    /// Where the fog starts and ends, in blocks, with the chunks shown up to `distance`.
    pub fn fog_range(self, distance: f32) -> (f32, f32) {
        let end = (distance * self.fog_end()).max(distance.min(MIN_FOG_END));
        let start = (distance * self.blend(FOG_START)).min(end);
        (start, end)
    }

    /// The brightness to add to the ambient light for the light of the overcast sky, which
    /// keeps the world readable as the sun dims: an even light as bright on screen as the grey
    /// of the sky, with the sun at `sun_height` and the camera at the exposure `ev100`.
    pub fn sky_light(self, sun_height: f32, physical_sky: bool, ev100: f32) -> f32 {
        if self.level == 0. {
            return 0.;
        }
        let (horizon, zenith) = self.colors(sun_height, physical_sky);
        let grey = (horizon + zenith).element_sum() / 6.;
        let gain = if physical_sky {
            PHYSICAL_SKY_LIGHT
        } else {
            PLAIN_SKY_LIGHT
        } * self.storm_light();
        // On screen, a light is its brightness times the exposure, 1 / (1.2 * 2^ev100)
        self.level * grey * gain * 1.2 * ev100.exp2()
    }

    /// The wind, along the ground plane (x, z) in blocks per second: the way the clouds drift.
    pub fn wind(self) -> Vec2 {
        Vec2::new(self.blend(WIND), 0.)
    }

    /// The part of the light of the sky that a storm lets through.
    pub fn storm_light(self) -> f32 {
        1f32.lerp(STORM_SKY_LIGHT, self.storm)
    }

    /// The colour of the ambient light, `clear` under a clear sky.
    pub fn ambient_color(self, clear: Color) -> Color {
        if self.level == 0. {
            return clear;
        }
        clear.mix(&AMBIENT_COLOR, self.level)
    }

    /// How bright the light of the physical sky is, which is blue: the light of the overcast
    /// takes over.
    pub fn environment_light(self) -> f32 {
        1. - self.level
    }

    /// How visible the sun, the moon and the stars are: gone by a level of 0.8.
    pub fn celestial_visibility(self) -> f32 {
        1. - smoothstep(0., 0.8, self.level)
    }

    /// The overcast sky at the horizon and at the zenith, in linear RGB, with the sun at
    /// `sun_height` (the sine of its height). With the physical sky, as seen on screen before
    /// the tonemapping; without it, as the colour behind the world. As bright as the clear sky
    /// is at that hour, not as the sky dimmed by the clouds.
    pub fn colors(self, sun_height: f32, physical_sky: bool) -> (Vec3, Vec3) {
        let [day, golden, night] = if physical_sky {
            [PHYSICAL_DAY, PHYSICAL_GOLDEN, PHYSICAL_NIGHT]
        } else {
            [PLAIN_DAY, PLAIN_GOLDEN, PLAIN_NIGHT]
        };
        let horizon = if sun_height >= 0. {
            golden.lerp(day, smoothstep(0., 0.3, sun_height))
        } else {
            golden.lerp(night, smoothstep(0., 0.2, -sun_height))
        };
        let horizon = horizon * Vec3::ONE.lerp(STORM_HORIZON, self.storm);
        let zenith = horizon * ZENITH * Vec3::ONE.lerp(STORM_ZENITH, self.storm);
        (horizon, zenith)
    }

    /// The glow of the sun under the overcast at the horizon toward it, with the sun at
    /// `sun_height`, in linear RGB as `colors`: only while the sun is low, at sunrise and
    /// sunset, and weaker in a storm.
    pub fn glow(self, sun_height: f32, physical_sky: bool) -> Vec3 {
        let low = smoothstep(-0.15, 0., sun_height) * (1. - smoothstep(0.15, 0.45, sun_height));
        let (horizon, _) = self.colors(sun_height, physical_sky);
        let brightness = horizon.element_sum() / 3.;
        GLOW * brightness * low * 1f32.lerp(STORM_GLOW, self.storm)
    }
}

/// The drift of the wind wraps around at this many blocks along x and z: a whole number of the
/// sides of every box the drops of `precipitation.wgsl` wrap around in (18 and 48 for the rain,
/// and 16, 40 and 80 for the snow, which drifts at `SNOW_DRIFT` of the wind), so that the wrap
/// moves no drop, and small enough to keep the fraction of a block in an `f32`.
pub const DRIFT_WRAP: f32 = 3600.;

/// The part of the wind the snow drifts with (`precipitation.wgsl`).
#[cfg(test)]
const SNOW_DRIFT: f32 = 0.8;

/// How far the wind has carried what falls, along the ground plane, in blocks: the wind
/// summed frame after frame, so that as it changes the drops keep the pace of the wind of the
/// moment, which the wind of now times the time gone by would not.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq)]
pub struct WindDrift(pub Vec2);

impl WindDrift {
    /// The drift `secs` seconds later, carried by `wind`, wrapped around at `DRIFT_WRAP`.
    pub fn advance(self, wind: Vec2, secs: f32) -> Self {
        Self((self.0 + wind * secs).rem_euclid(Vec2::splat(DRIFT_WRAP)))
    }
}

/// What the shaders of the sky know of the weather: `Weather` in `sky.wgsl`, bound at 107 in
/// every material that shows the sky.
#[derive(ShaderType, Clone, Copy, Debug, Default, PartialEq)]
pub struct WeatherShading {
    /// The overcast sky at the horizon and at the zenith (`Overcast::colors`), in linear RGB.
    pub horizon: Vec4,
    pub zenith: Vec4,
    /// The glow of a low sun at the horizon toward it (`Overcast::glow`), and in `xy` the
    /// direction of the sun on the ground plane, its x and z.
    pub glow: Vec4,
    pub sun: Vec4,
    /// The wind (`Overcast::wind`) in `xy`, and in `z` how gusty it is, the storm level.
    pub wind: Vec4,
    /// How far the wind has carried what falls (`WindDrift`), in `xy`.
    pub drift: Vec4,
    /// The strike flashing the brightest: the top of the block struck, and in `w` how bright
    /// its channel is now (`Strokes::channel`), however far off it is.
    pub strike: Vec4,
    /// The direction from the eye to the flash in the clouds over that strike.
    pub flash_dir: Vec4,
    /// `Overcast::level`.
    pub overcast: f32,
    /// How much rain or snow falls (`ClientWeather::precipitation`).
    pub rain: f32,
    /// `Overcast::storm`.
    pub storm: f32,
    /// How bright a flash of lightning lights the world, from 0 to 1 (`Lightning::flash`).
    pub flash: f32,
    /// How wet the world open to the rain is (`ClientWeather::wetness`), where the quality
    /// draws it, 0 elsewhere.
    pub wetness: f32,
}

impl WeatherShading {
    /// What the shaders are told under `overcast`, with `rain` falling on a world as wet as
    /// `wetness`, at the world clock `tick`.
    pub fn new(overcast: Overcast, rain: f32, wetness: f32, tick: u64, physical_sky: bool) -> Self {
        let height = sun_height(tick);
        let (horizon, zenith) = overcast.colors(height, physical_sky);
        let sun = sky_rotation(tick) * Vec3::Z;
        let azimuth = Vec2::new(sun.x, sun.z).normalize_or_zero();
        Self {
            horizon: horizon.extend(1.),
            zenith: zenith.extend(1.),
            glow: overcast.glow(height, physical_sky).extend(1.),
            sun: azimuth.extend(0.).extend(0.),
            wind: overcast.wind().extend(overcast.storm).extend(0.),
            drift: Vec4::ZERO,
            strike: Vec4::ZERO,
            flash_dir: Vec4::ZERO,
            overcast: overcast.level,
            rain,
            storm: overcast.storm,
            flash: 0.,
            wetness,
        }
    }
}

impl WeatherShading {
    /// What the shaders are told, with what falls carried by the wind as far as `drift`.
    pub fn with_drift(self, drift: Vec2) -> Self {
        Self {
            drift: drift.extend(0.).extend(0.),
            ..self
        }
    }

    /// What the shaders are told, with the flash of `lightning` seen from `eye`: nothing more
    /// while nothing flashes.
    pub fn with_flash(self, lightning: &Lightning, eye: Vec3) -> Self {
        let Some(strike) = lightning.brightest() else {
            return self;
        };
        let foot = strike.at.as_vec3() + Vec3::new(0.5, 1., 0.5);
        Self {
            strike: foot.extend(strike.strokes.channel(strike.age)),
            flash_dir: (strike.cloud_point() - eye)
                .normalize_or(Vec3::Y)
                .extend(0.),
            flash: lightning.flash(),
            ..self
        }
    }
}

/// The buffer of `WeatherShading`, under a clear sky.
pub fn new_weather_buffer() -> ShaderBuffer {
    ShaderBuffer::from(WeatherShading::default())
}

const OVERCAST_SHADER: Handle<Shader> = uuid_handle!("d3e5a7c1-4f2b-4b9e-8a61-7c0e9f3b5d24");

/// The overcast sky and the dome that draws it.
pub fn overcast_plugin(app: &mut App) {
    load_internal_asset!(app, OVERCAST_SHADER, "overcast.wgsl", Shader::from_wgsl);
    app.add_plugins(MaterialPlugin::<OvercastMaterial>::default())
        .init_resource::<WindDrift>()
        .add_systems(
            Update,
            (weather_shading_system, follow_camera_with_dome).run_if(in_state(GameState::Game)),
        );
}

/// Tells the shaders of the sky the overcast of now, when it changes, and carries what falls
/// on with the wind.
#[allow(clippy::too_many_arguments)]
pub fn weather_shading_system(
    time: Res<ClientTime>,
    frame: Res<Time>,
    mut drift: ResMut<WindDrift>,
    weather: Option<Res<ClientWeather>>,
    lightning: Option<Res<Lightning>>,
    cameras: Query<&GlobalTransform, With<Camera3d>>,
    graphics: Res<GraphicsSettings>,
    materials: Res<MaterialResource>,
    mut buffers: ResMut<Assets<ShaderBuffer>>,
    mut written: Local<Option<WeatherShading>>,
) {
    let features = ShaderFeatures::of(graphics.shaders);
    let wetness = match weather.as_deref() {
        Some(weather) if features.wet => weather.wetness,
        _ => 0.,
    };
    let overcast = Overcast::current(weather.as_deref());
    *drift = drift.advance(overcast.wind(), frame.delta_secs());
    let shading = WeatherShading::new(
        overcast,
        weather.as_deref().map_or(0., ClientWeather::precipitation),
        wetness,
        time.0,
        features.sky,
    )
    .with_drift(drift.0);
    let eye = cameras
        .iter()
        .next()
        .map_or(Vec3::ZERO, GlobalTransform::translation);
    let shading = match lightning {
        Some(lightning) => shading.with_flash(&lightning, eye),
        None => shading,
    };
    if *written == Some(shading) {
        return;
    }
    if let Some(mut buffer) = buffers.get_mut(&materials.weather) {
        // The same size: the buffer on the GPU is written in place, and stays bound
        buffer.set_data(shading);
        *written = Some(shading);
    }
}

/// The grey sky of the weather (`overcast.wgsl`), drawn over the physical sky by the overcast
/// level: a sphere around the camera on the far plane, so that it covers the sky only. Bevy
/// draws the sky after the opaque meshes, so the dome is among the transparent ones, after the
/// sun, the moon and the stars and before the clouds.
#[derive(Asset, TypePath, AsBindGroup, Clone, Debug)]
pub struct OvercastMaterial {
    #[storage(107, read_only)]
    pub weather: Handle<ShaderBuffer>,
}

impl Material for OvercastMaterial {
    fn vertex_shader() -> ShaderRef {
        OVERCAST_SHADER.into()
    }

    fn fragment_shader() -> ShaderRef {
        OVERCAST_SHADER.into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Blend
    }

    /// After the sun and the moon (-1e6), before everything near.
    fn depth_bias(&self) -> f32 {
        -5e5
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
        descriptor.vertex.buffers = vec![layout
            .0
            .get_layout(&[Mesh::ATTRIBUTE_POSITION.at_shader_location(0)])?];
        // Seen from inside
        descriptor.primitive.cull_mode = None;
        Ok(())
    }
}

/// The overcast dome around the camera.
#[derive(Component)]
pub struct OvercastDome;

/// Keeps the overcast dome around the camera, with the physical sky only (without it, the
/// colour behind the world turns grey instead), shown while the sky is overcast at all.
fn follow_camera_with_dome(
    graphics: Res<GraphicsSettings>,
    weather: Option<Res<ClientWeather>>,
    resources: Res<MaterialResource>,
    cameras: Query<&GlobalTransform, With<Camera3d>>,
    mut domes: Query<(Entity, &mut Transform, &mut Visibility), With<OvercastDome>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<OvercastMaterial>>,
    mut commands: Commands,
) {
    let camera = cameras.iter().next();
    let (Some(camera), true) = (camera, ShaderFeatures::of(graphics.shaders).sky) else {
        for (dome, ..) in &domes {
            commands.entity(dome).despawn();
        }
        return;
    };
    let visibility = if Overcast::current(weather.as_deref()).level > 0. {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    let transform = Transform::from_translation(camera.translation());
    match domes.single_mut() {
        Ok((_, mut dome, mut shown)) => {
            *dome = transform;
            shown.set_if_neq(visibility);
        }
        Err(_) => {
            commands.spawn((
                OvercastDome,
                Mesh3d(meshes.add(Sphere::new(100.).mesh().ico(3).unwrap())),
                MeshMaterial3d(materials.add(OvercastMaterial {
                    weather: resources.weather.clone(),
                })),
                transform,
                visibility,
                NotShadowCaster,
                NotShadowReceiver,
                NoFrustumCulling,
                DespawnOnExit(GameState::Game),
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::ShaderQuality;
    use shared::world::WeatherKind;
    use shared::NOON_IN_DAY;

    const CLEAR: Overcast = Overcast {
        level: 0.,
        storm: 0.,
    };
    const RAIN: Overcast = Overcast {
        level: 1.,
        storm: 0.,
    };
    const STORM: Overcast = Overcast {
        level: 1.,
        storm: 1.,
    };

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-5
    }

    #[test]
    fn the_sky_closes_over_the_first_half_of_the_rain() {
        let weather = |rain, thunder| {
            let mut weather = ClientWeather::settled(WeatherKind::Thunder);
            (weather.rain, weather.thunder) = (rain, thunder);
            weather
        };
        assert_eq!(Overcast::of(&weather(0., 0.)), CLEAR);
        assert_eq!(Overcast::of(&weather(0.5, 0.)), RAIN);
        assert_eq!(Overcast::of(&weather(1., 1.)), STORM);
        assert_eq!(Overcast::of(&weather(0.25, 0.25)).level, 0.5);
        assert_eq!(Overcast::of(&weather(0.25, 0.25)).storm, 0.5);
        assert_eq!(Overcast::current(None), CLEAR);
        assert_eq!(
            Overcast::current(Some(&ClientWeather::settled(WeatherKind::Rain))),
            RAIN
        );
    }

    #[test]
    fn a_clear_sky_changes_nothing() {
        assert_eq!(CLEAR.sun(), 1.);
        assert_eq!(CLEAR.sky_light(0.9, true, 13.), 0.);
        assert_eq!(CLEAR.environment_light(), 1.);
        assert_eq!(CLEAR.saturation(1.12), 1.12);
        assert_eq!(CLEAR.celestial_visibility(), 1.);
        assert_eq!(CLEAR.fog_range(256.), (0.6 * 256., 256.));
    }

    #[test]
    fn rain_dims_the_sun_and_a_storm_more() {
        assert!(close(RAIN.sun(), 0.12));
        assert!(close(STORM.sun(), 0.05));
        assert!(close(RAIN.saturation(1.12), 0.9));
        assert!(STORM.saturation(1.12) <= RAIN.saturation(1.12));
    }

    #[test]
    fn an_even_grey_light_takes_over_from_the_sun_and_the_blue_sky() {
        assert_eq!(CLEAR.ambient_color(Color::WHITE), Color::WHITE);
        for overcast in [RAIN, STORM] {
            assert_eq!(overcast.environment_light(), 0.);
            let grey = overcast.ambient_color(Color::WHITE).to_linear();
            assert!(
                grey.blue >= grey.red && grey.red > 0.8 * grey.blue,
                "{grey:?}"
            );
        }
        for physical in [true, false] {
            // as bright on screen as the grey of the sky, at any exposure
            let (horizon, zenith) = RAIN.colors(0.9, physical);
            let grey = (horizon + zenith).element_sum() / 6.;
            for ev100 in [9.7, 13.] {
                let on_screen = RAIN.sky_light(0.9, physical, ev100) / (1.2 * ev100.exp2());
                assert!(
                    on_screen >= grey * 0.999 && on_screen < 2. * grey,
                    "{on_screen}"
                );
            }
            // darker in a storm and at night
            let noon = RAIN.sky_light(0.9, physical, 13.);
            assert!(STORM.sky_light(0.9, physical, 13.) < noon);
            assert!(RAIN.sky_light(-0.9, physical, 13.) < 0.1 * noon);
        }
    }

    #[test]
    fn the_sun_the_moon_and_the_stars_are_gone_by_four_fifths_of_the_overcast() {
        let at = |level| Overcast { level, storm: 0. }.celestial_visibility();
        assert_eq!(at(0.8), 0.);
        assert_eq!(at(1.), 0.);
        assert!(at(0.4) > 0. && at(0.4) < 1.);
        assert!(at(0.2) > at(0.4));
    }

    #[test]
    fn the_fog_closes_in_under_rain_and_more_in_a_storm() {
        let (start, end) = RAIN.fog_range(256.);
        assert!(close(start, 0.3 * 256.) && close(end, 0.75 * 256.));
        let (start, end) = STORM.fog_range(256.);
        assert!(close(start, 0.15 * 256.) && close(end, 0.6 * 256.));
        // halfway, in between
        let (start, end) = Overcast {
            level: 0.5,
            storm: 0.,
        }
        .fog_range(256.);
        assert!(start > 0.3 * 256. && start < 0.6 * 256., "{start}");
        assert!(end > 0.75 * 256. && end < 256., "{end}");
    }

    #[test]
    fn the_fog_never_ends_closer_than_48_blocks_nor_past_the_chunks() {
        let (start, end) = STORM.fog_range(64.);
        assert_eq!(end, 48.);
        assert!(start < end);
        // fewer chunks shown than that: the fog ends where they do, as in clear weather
        assert_eq!(STORM.fog_range(32.).1, 32.);
        assert!(STORM.fog_range(32.).0 <= 32.);
    }

    fn brightness(color: Vec3) -> f32 {
        color.element_sum() / 3.
    }

    #[test]
    fn the_overcast_is_grey_lighter_at_the_horizon() {
        for physical in [true, false] {
            for height in [-1., -0.05, 0., 0.1, 0.3, 0.9] {
                for overcast in [RAIN, STORM] {
                    let (horizon, zenith) = overcast.colors(height, physical);
                    // a grey, at most a cool slate in a storm at night
                    for color in [horizon, zenith] {
                        let spread = color.max_element() - color.min_element();
                        assert!(spread <= 0.5 * color.max_element(), "{color}");
                        assert!(color.z >= color.x || height.abs() < 0.2, "{color}");
                    }
                    assert!(brightness(horizon) >= brightness(zenith), "{height}");
                }
            }
        }
    }

    #[test]
    fn a_low_sun_glows_under_the_overcast_less_in_a_storm() {
        for physical in [true, false] {
            assert_eq!(RAIN.glow(0.9, physical), Vec3::ZERO);
            assert_eq!(RAIN.glow(-0.5, physical), Vec3::ZERO);
            let sunset = RAIN.glow(0.03, physical);
            assert!(sunset.x > 1.5 * sunset.y && sunset.y > sunset.z, "{sunset}");
            let (horizon, _) = RAIN.colors(0.03, physical);
            assert!(brightness(sunset) < brightness(horizon), "{sunset}");
            assert!(brightness(STORM.glow(0.03, physical)) < 0.5 * brightness(sunset));
        }
    }

    #[test]
    fn a_storm_is_a_cooler_grey_darker_up_high() {
        for physical in [true, false] {
            let (rain_horizon, rain_zenith) = RAIN.colors(0.9, physical);
            let (horizon, zenith) = STORM.colors(0.9, physical);
            assert!(
                zenith.z - zenith.x > rain_zenith.z - rain_zenith.x,
                "{zenith}"
            );
            assert!(
                brightness(zenith) / brightness(rain_zenith)
                    < brightness(horizon) / brightness(rain_horizon)
            );
            assert!(STORM.sky_light(0.9, physical, 10.) < 0.8 * RAIN.sky_light(0.9, physical, 10.));
        }
    }

    #[test]
    fn the_overcast_follows_the_day_and_darkens_in_a_storm() {
        for physical in [true, false] {
            let noon = brightness(RAIN.colors(0.9, physical).0);
            let dusk = brightness(RAIN.colors(0., physical).0);
            let night = brightness(RAIN.colors(-0.9, physical).0);
            assert!(
                noon > 2. * dusk && dusk > 2. * night,
                "{noon} {dusk} {night}"
            );
            assert!(brightness(STORM.colors(0.9, physical).0) < 0.8 * noon);
            let mut before = RAIN.colors(-1., physical).0;
            for step in -99..=100 {
                let now = RAIN.colors(step as f32 / 100., physical).0;
                assert!(now.distance(before) < 0.05, "{step}: {before} then {now}");
                before = now;
            }
        }
    }

    fn app(shaders: ShaderQuality, kind: WeatherKind) -> (App, Handle<ShaderBuffer>) {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<ShaderBuffer>()
            .init_asset::<Mesh>()
            .init_asset::<OvercastMaterial>()
            .insert_resource(ClientTime(NOON_IN_DAY))
            .insert_resource(ClientWeather::settled(kind))
            .insert_resource(GraphicsSettings {
                shaders,
                ..default()
            })
            .init_resource::<WindDrift>()
            .add_systems(Update, (weather_shading_system, follow_camera_with_dome));
        let weather = app
            .world_mut()
            .resource_mut::<Assets<ShaderBuffer>>()
            .add(new_weather_buffer());
        app.insert_resource(MaterialResource {
            weather: weather.clone(),
            ..default()
        });
        app.world_mut().spawn((
            Camera3d::default(),
            GlobalTransform::from_translation(Vec3::new(3., 70., -8.)),
        ));
        app.update();
        (app, weather)
    }

    /// `shading` with the drift of the wind of `app` now.
    fn drifted(app: &App, shading: WeatherShading) -> WeatherShading {
        shading.with_drift(app.world().resource::<WindDrift>().0)
    }

    fn buffer_data(app: &App, handle: &Handle<ShaderBuffer>) -> Option<Vec<u8>> {
        let buffers = app.world().resource::<Assets<ShaderBuffer>>();
        buffers.get(handle).unwrap().data.clone()
    }

    #[test]
    fn the_shaders_of_the_sky_are_told_the_overcast_of_the_hour() {
        let (mut app, handle) = app(ShaderQuality::High, WeatherKind::Rain);
        let expected = drifted(&app, WeatherShading::new(RAIN, 1., 1., NOON_IN_DAY, true));
        assert_eq!(expected.overcast, 1.);
        assert_eq!(
            buffer_data(&app, &handle),
            ShaderBuffer::from(expected).data
        );
        // without the physical sky, the grey of the colour behind the world
        app.insert_resource(GraphicsSettings {
            shaders: ShaderQuality::Off,
            ..default()
        });
        app.update();
        let expected = drifted(&app, WeatherShading::new(RAIN, 1., 0., NOON_IN_DAY, false));
        assert_eq!(
            buffer_data(&app, &handle),
            ShaderBuffer::from(expected).data
        );
        assert_eq!(app.world().resource::<MaterialResource>().weather, handle);
    }

    #[test]
    fn the_wind_drifts_with_the_clouds_and_blows_harder_in_a_storm() {
        assert_eq!(CLEAR.wind(), Vec2::new(SPEED, 0.));
        let (rain, storm) = (RAIN.wind(), STORM.wind());
        assert_eq!((rain.y, storm.y), (0., 0.));
        assert!(
            rain.x > 2. * SPEED && storm.x > 2.5 * rain.x,
            "{rain} {storm}"
        );
    }

    #[test]
    fn what_falls_keeps_the_pace_of_the_wind_when_the_wind_changes() {
        // half an hour of rain, then a storm rolling in: carried by the wind of the moment,
        // never by the new wind over all the time gone by
        let frame = 1. / 60.;
        let mut drift = WindDrift::default();
        for step in 0..120_000 {
            let overcast = Overcast {
                level: 1.,
                storm: ((step as f32 - 108_000.) / 1_200.).clamp(0., 1.),
            };
            let wind = overcast.wind();
            let next = drift.advance(wind, frame);
            let moved = next.0 - drift.0;
            // across a wrap, the drops move by whole boxes, which shows nothing
            let moved = moved - (moved / DRIFT_WRAP).round() * DRIFT_WRAP;
            assert!(
                moved.distance(wind * frame) < 1e-3,
                "{step}: {moved} for a wind of {wind}"
            );
            drift = next;
        }
        assert!(drift.0.x >= 0. && drift.0.x < DRIFT_WRAP && drift.0.y == 0.);
    }

    /// The half sides of the boxes the bands of `function` in `precipitation.wgsl` wrap around.
    fn band_extents(function: &str) -> Vec<f32> {
        let source = include_str!("../precipitation/precipitation.wgsl");
        let body = source.split(&format!("fn {function}(")).nth(1).unwrap();
        let body = body.split("\n}").next().unwrap();
        body.split("Band(vec3(")
            .skip(1)
            .map(|band| band.split(',').next().unwrap().trim().parse().unwrap())
            .collect()
    }

    #[test]
    fn the_drift_wraps_around_by_whole_boxes_of_every_band_of_rain_and_snow() {
        let (rain, snow) = (band_extents("rain_band"), band_extents("snow_band"));
        assert_eq!((rain.len(), snow.len()), (2, 3));
        for extent in rain {
            assert_eq!(DRIFT_WRAP % (2. * extent), 0., "{extent}");
        }
        // the snow drifts at a part of the wind
        for extent in snow {
            assert_eq!(SNOW_DRIFT * DRIFT_WRAP % (2. * extent), 0., "{extent}");
        }
        let source = include_str!("../precipitation/precipitation.wgsl");
        assert!(source.contains(&format!("const SNOW_DRIFT: f32 = {SNOW_DRIFT:?};")));
    }

    #[test]
    fn the_shaders_are_told_how_far_the_wind_has_carried_what_falls() {
        let (mut app, handle) = app(ShaderQuality::High, WeatherKind::Thunder);
        std::thread::sleep(std::time::Duration::from_millis(20));
        app.update();
        let drift = app.world().resource::<WindDrift>().0;
        assert!(drift.x > 0., "{drift}");
        let expected = WeatherShading::new(STORM, 1., 1., NOON_IN_DAY, true).with_drift(drift);
        assert_eq!(
            buffer_data(&app, &handle),
            ShaderBuffer::from(expected).data
        );
    }

    #[test]
    fn the_shaders_are_told_the_drops_the_storm_and_the_wind_and_no_flash_yet() {
        let shading = WeatherShading::new(STORM, 0.6, 0.3, NOON_IN_DAY, true);
        assert_eq!(shading.rain, 0.6);
        assert_eq!(shading.wetness, 0.3);
        assert_eq!(shading.storm, 1.);
        assert_eq!(shading.flash, 0.);
        assert_eq!(shading.wind, STORM.wind().extend(1.).extend(0.));
        let clear = WeatherShading::new(CLEAR, 0., 0., NOON_IN_DAY, true);
        assert_eq!((clear.rain, clear.storm, clear.wetness), (0., 0., 0.));
    }

    #[test]
    fn the_shaders_are_told_the_flash_of_the_brightest_strike_and_where_it_is() {
        use crate::world::lightning::{bolt::CLOUD_BASE, LiveStrike, Strike};
        let shading = WeatherShading::new(STORM, 1., 1., NOON_IN_DAY, true);
        let mut lightning = Lightning::default();
        assert_eq!(shading.with_flash(&lightning, Vec3::ZERO), shading);
        let eye = Vec3::new(0., CLOUD_BASE, 0.);
        let strike = Strike {
            at: IVec3::new(80, 70, 0),
            seed: 3,
        };
        lightning.strikes.push(LiveStrike::new(strike, eye, 128.));
        let peak = lightning.strikes[0].strokes.peaks().next().unwrap();
        lightning.advance(peak);
        let flashed = shading.with_flash(&lightning, eye);
        assert_eq!(flashed.flash, lightning.flash());
        assert!(
            flashed.flash > 0.5 && flashed.flash < 1.,
            "{}",
            flashed.flash
        );
        // the channel itself at its brightest, however far
        assert_eq!(flashed.strike, Vec4::new(80.5, 71., 0.5, 1.));
        assert!(flashed.flash_dir.truncate().distance(Vec3::X) < 0.01);
        assert_eq!(
            (flashed.overcast, flashed.rain),
            (shading.overcast, shading.rain)
        );
    }

    #[test]
    fn the_shaders_are_told_how_wet_the_world_is_where_the_quality_draws_it() {
        for (quality, told) in [
            (ShaderQuality::Off, 0.),
            (ShaderQuality::Medium, 0.),
            (ShaderQuality::High, 0.4),
            (ShaderQuality::Ultra, 0.4),
        ] {
            let (mut app, handle) = app(quality, WeatherKind::Clear);
            let mut weather = ClientWeather::settled(WeatherKind::Clear);
            weather.wetness = 0.4;
            app.insert_resource(weather);
            app.update();
            let physical = ShaderFeatures::of(quality).sky;
            let expected = drifted(
                &app,
                WeatherShading::new(CLEAR, 0., told, NOON_IN_DAY, physical),
            );
            assert_eq!(
                buffer_data(&app, &handle),
                ShaderBuffer::from(expected).data,
                "{quality:?}"
            );
        }
    }

    fn domes(app: &mut App) -> Vec<(Vec3, Visibility)> {
        let world = app.world_mut();
        world
            .query_filtered::<(&Transform, &Visibility), With<OvercastDome>>()
            .iter(world)
            .map(|(transform, visibility)| (transform.translation, *visibility))
            .collect()
    }

    #[test]
    fn the_dome_covers_the_physical_sky_while_it_is_overcast() {
        let (mut app, _) = app(ShaderQuality::High, WeatherKind::Rain);
        assert_eq!(
            domes(&mut app),
            [(Vec3::new(3., 70., -8.), Visibility::Inherited)]
        );
        app.insert_resource(ClientWeather::settled(WeatherKind::Clear));
        app.update();
        assert_eq!(domes(&mut app)[0].1, Visibility::Hidden);
        // without the physical sky, none
        app.insert_resource(GraphicsSettings {
            shaders: ShaderQuality::Off,
            ..default()
        });
        app.update();
        assert!(domes(&mut app).is_empty());
    }
}
