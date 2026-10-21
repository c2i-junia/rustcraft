//! The colour of the fog that shows as the sky. Without HDR, each material tonemaps what it
//! draws, its fog included, while the clear colour, the sky, is not tonemapped: a fog the colour
//! of the sky showed a shade darker than it, and a block wholly in the fog a flat shape on it.
//! The fog takes instead the colour that the tonemapping turns into the sky's, found on the CPU
//! from the LUT of TonyMcMapface, the tonemapping of the cameras, which the shaders sample.

use bevy::core_pipeline::tonemapping::TonemappingLuts;
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;

/// How many texels the LUT of TonyMcMapface has along each side.
const LUT_SIZE: usize = 48;

/// TonyMcMapface on the CPU: its LUT, decoded, sampled as the shaders sample it.
#[derive(Resource)]
pub struct ToneCurve {
    /// Linear colours, red along x, green along y, blue along z.
    texels: Vec<Vec3>,
}

impl ToneCurve {
    /// The curve of the LUT `image`, if it is TonyMcMapface's: 48³ texels in RGB9E5.
    pub fn from_lut(image: &Image) -> Option<Self> {
        let size = image.texture_descriptor.size;
        if image.texture_descriptor.format != TextureFormat::Rgb9e5Ufloat
            || (size.width, size.height, size.depth_or_array_layers)
                != (LUT_SIZE as u32, LUT_SIZE as u32, LUT_SIZE as u32)
        {
            return None;
        }
        let data = image.data.as_ref()?;
        let texels: Vec<Vec3> = data
            .as_chunks::<4>()
            .0
            .iter()
            .map(|texel| rgb9e5(u32::from_le_bytes(*texel)))
            .collect();
        (texels.len() == LUT_SIZE.pow(3)).then_some(Self { texels })
    }

    /// The linear colour on screen of the linear colour `stimulus`, as `tone_mapping` in Bevy's
    /// `tonemapping_shared.wgsl` computes it: the LUT sampled at `stimulus / (stimulus + 1)`,
    /// between the centres of its texels.
    #[cfg(test)]
    pub fn apply(&self, stimulus: Vec3) -> Vec3 {
        let encoded = stimulus.max(Vec3::ZERO) / (stimulus.max(Vec3::ZERO) + 1.0);
        self.sample(encoded)
    }

    /// The linear colour `stimulus` that shows as the linear colour `shown`: Newton's method on
    /// the encoded stimulus, the LUT being linear between its texels.
    pub fn stimulus_for(&self, shown: Vec3) -> Vec3 {
        // Below 1, where `encoded / (1 - encoded)` stays finite
        let highest = 1.0 - 1.0 / (LUT_SIZE as f32 * 4.0);
        let mut encoded = (shown / (shown + 1.0)).clamp(Vec3::ZERO, Vec3::splat(highest));
        for _ in 0..12 {
            let now = self.sample(encoded);
            let error = shown - now;
            if error.abs().max_element() < 1e-6 {
                break;
            }
            let step = 1e-3;
            let column = |axis: Vec3| (self.sample(encoded + axis * step) - now) / step;
            let jacobian = Mat3::from_cols(column(Vec3::X), column(Vec3::Y), column(Vec3::Z));
            if jacobian.determinant().abs() < 1e-9 {
                break;
            }
            encoded =
                (encoded + jacobian.inverse() * error).clamp(Vec3::ZERO, Vec3::splat(highest));
        }
        encoded / (1.0 - encoded)
    }

    /// The LUT at `encoded`, between 0 and 1 on each axis, trilinearly between its texels.
    fn sample(&self, encoded: Vec3) -> Vec3 {
        let last = (LUT_SIZE - 1) as f32;
        let at = encoded.clamp(Vec3::ZERO, Vec3::ONE) * last;
        let low = at.floor().min(Vec3::splat(last - 1.0));
        let t = at - low;
        let (x, y, z) = (low.x as usize, low.y as usize, low.z as usize);
        let texel = |dx: usize, dy: usize, dz: usize| {
            self.texels[(z + dz) * LUT_SIZE * LUT_SIZE + (y + dy) * LUT_SIZE + (x + dx)]
        };
        let along_x = |dy, dz| texel(0, dy, dz).lerp(texel(1, dy, dz), t.x);
        let along_y = |dz| along_x(0, dz).lerp(along_x(1, dz), t.y);
        along_y(0).lerp(along_y(1), t.z)
    }
}

/// A texel in RGB9E5: three 9-bit mantissas sharing a 5-bit exponent.
fn rgb9e5(packed: u32) -> Vec3 {
    let scale = 2f32.powi((packed >> 27) as i32 - 15 - 9);
    let mantissa = |shift: u32| ((packed >> shift) & 0x1ff) as f32 * scale;
    Vec3::new(mantissa(0), mantissa(9), mantissa(18))
}

/// The colour of a fog that shows as `sky` once tonemapped by `curve`. Without a curve (a
/// build without the LUTs, or a test), the sky itself.
pub fn sky_fog_color(sky: Color, curve: Option<&ToneCurve>) -> Color {
    let Some(curve) = curve else {
        return sky;
    };
    let fog = curve.stimulus_for(Vec3::from_array(sky.to_linear().to_f32_array_no_alpha()));
    Color::linear_rgb(fog.x, fog.y, fog.z)
}

/// Reads the LUT of TonyMcMapface, which Bevy keeps in the main world.
pub fn setup_tone_curve(
    luts: Option<Res<TonemappingLuts>>,
    images: Option<Res<Assets<Image>>>,
    mut commands: Commands,
) {
    let curve = luts
        .zip(images)
        .and_then(|(luts, images)| ToneCurve::from_lut(images.get(&luts.tony_mc_mapface)?));
    match curve {
        Some(curve) => commands.insert_resource(curve),
        None => warn!("No LUT of TonyMcMapface: the fog of the sky is the colour of the sky"),
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::camera::sky_color;
    use bevy::core_pipeline::tonemapping::TonemappingPlugin;
    use bevy::shader::Shader;
    use shared::{DAY_DURATION_IN_TICKS, MIDNIGHT_IN_DAY, NOON_IN_DAY};

    /// TonyMcMapface's curve, from the LUT Bevy loads.
    pub(crate) fn tony_mc_mapface() -> ToneCurve {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            ImagePlugin::default(),
        ))
        .init_asset::<Shader>()
        .add_plugins(TonemappingPlugin)
        .add_systems(Startup, setup_tone_curve);
        app.update();
        app.world_mut()
            .remove_resource::<ToneCurve>()
            .expect("the LUT of TonyMcMapface")
    }

    /// `color` in sRGB, from 0 to 255.
    fn levels(color: Color) -> Vec3 {
        Vec3::from_array(color.to_srgba().to_f32_array_no_alpha()) * 255.0
    }

    /// The colour on screen of a fog of colour `fog`.
    fn shown(curve: &ToneCurve, fog: Color) -> Color {
        let linear = curve.apply(Vec3::from_array(fog.to_linear().to_f32_array_no_alpha()));
        Color::linear_rgb(linear.x, linear.y, linear.z)
    }

    #[test]
    fn the_curve_turns_the_fog_measured_by_day_into_the_sky() {
        // Measured on screen before the sky followed the hour: this fog showed as SKY_COLOR
        let measured = Color::srgb_u8(125, 183, 216);
        let curve = tony_mc_mapface();
        let off = levels(shown(&curve, measured)) - levels(crate::camera::SKY_COLOR);
        assert!(off.abs().max_element() < 1.5, "{off}");
        let fog = levels(sky_fog_color(crate::camera::SKY_COLOR, Some(&curve)));
        assert!((fog - levels(measured)).abs().max_element() < 1.5, "{fog}");
    }

    #[test]
    fn the_fog_shows_as_the_sky_at_every_hour() {
        let curve = tony_mc_mapface();
        let noon = 3 * DAY_DURATION_IN_TICKS + NOON_IN_DAY;
        let midnight = 3 * DAY_DURATION_IN_TICKS + MIDNIGHT_IN_DAY;
        let step = DAY_DURATION_IN_TICKS / 96;
        let hours = (0..DAY_DURATION_IN_TICKS).step_by(step as usize);
        for tick in hours.chain([noon, midnight, 0, DAY_DURATION_IN_TICKS / 2]) {
            let sky = sky_color(tick);
            let fog = sky_fog_color(sky, Some(&curve));
            let off = levels(shown(&curve, fog)) - levels(sky);
            assert!(off.abs().max_element() < 0.5, "at {tick}: {off}");
        }
        // At night the fog is dark too, not the blue of the day
        let night = levels(sky_fog_color(sky_color(midnight), Some(&curve)));
        assert!(night.max_element() < 40.0, "{night}");
    }

    #[test]
    fn without_a_curve_the_fog_is_the_sky() {
        assert_eq!(
            sky_fog_color(crate::camera::SKY_COLOR, None),
            crate::camera::SKY_COLOR
        );
    }
}
