//! The glint of an enchanted item drawn in the world, lying on the ground or held in a hand: a
//! copy of the picture of the item over it, added to its colours in a teal that breathes and
//! drifts toward a pale violet, the colours of the gem of its slot. The glints of every enchanted
//! item share one material per texture, coloured anew each frame (`shimmer`), so that an
//! enchanted item costs one more mesh to draw and nothing more.

use std::collections::HashMap;
use std::f32::consts::TAU;

use bevy::{
    asset::AssetId,
    light::{NotShadowCaster, NotShadowReceiver},
    prelude::*,
};

use super::mesh::ItemLook;

/// The materials of the glints, one per texture of the pictures they are drawn over.
#[derive(Resource, Default)]
pub(crate) struct Glints(HashMap<AssetId<Image>, Handle<StandardMaterial>>);

/// The copy of the mesh of an enchanted item that draws its glint.
#[derive(Component, Debug)]
pub(crate) struct Glint;

/// Seconds of a breath of the glint, and of its drift to violet and back.
const BREATH_SECONDS: f32 = 2.4;
const DRIFT_SECONDS: f32 = 6.5;
/// How much of the colours of the item the glint adds, at the bottom and the top of a breath.
const FAINTEST: f32 = 0.25;
const BRIGHTEST: f32 = 0.8;
const TEAL: Vec3 = Vec3::new(0.05, 0.9, 0.8);
const VIOLET: Vec3 = Vec3::new(0.55, 0.2, 1.0);

impl Glints {
    /// The glint of an enchanted item drawn as `look`, to spawn as a child of its mesh, on its
    /// `layers`. A block drawn as a cube has none: no enchantment fits a block.
    pub(crate) fn of(
        &mut self,
        look: &ItemLook,
        layers: impl Bundle,
        materials: &mut Assets<StandardMaterial>,
    ) -> Option<impl Bundle> {
        let ItemLook::Sprite(mesh, material) = look else {
            return None;
        };
        let source = materials.get(material)?;
        let texture = source.base_color_texture.clone()?;
        let (cull_mode, double_sided) = (source.cull_mode, source.double_sided);
        let glint = self
            .0
            .entry(texture.id())
            .or_insert_with(|| {
                materials.add(StandardMaterial {
                    base_color: glint_color(0.0),
                    base_color_texture: Some(texture),
                    unlit: true,
                    alpha_mode: AlphaMode::Add,
                    // drawn over the item, whose faces it lies on
                    depth_bias: 1.0,
                    cull_mode,
                    double_sided,
                    fog_enabled: false,
                    ..default()
                })
            })
            .clone();
        Some((
            Glint,
            Mesh3d(mesh.clone()),
            MeshMaterial3d(glint),
            Transform::default(),
            Visibility::default(),
            NotShadowCaster,
            NotShadowReceiver,
            layers,
        ))
    }
}

/// The colour of the glints `seconds` into the game: how much of each colour of the item they
/// add, in linear light.
pub(crate) fn glint_color(seconds: f32) -> Color {
    let wave = |period: f32| 0.5 - 0.5 * (seconds * TAU / period).cos();
    let strength = FAINTEST + (BRIGHTEST - FAINTEST) * wave(BREATH_SECONDS);
    let tint = TEAL.lerp(VIOLET, wave(DRIFT_SECONDS)) * strength;
    Color::linear_rgb(tint.x, tint.y, tint.z)
}

/// Colours the glints for this frame.
pub(super) fn shimmer(
    time: Res<Time>,
    glints: Res<Glints>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let color = glint_color(time.elapsed_secs());
    for handle in glints.0.values() {
        if let Some(mut material) = materials.get_mut(handle) {
            material.base_color = color;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_glint_breathes_between_faint_and_bright_and_drifts_to_violet() {
        let strength = |seconds: f32| glint_color(seconds).to_linear().green;
        let faint = strength(0.0);
        let bright = strength(BREATH_SECONDS / 2.0);
        assert!((faint - TEAL.y * FAINTEST).abs() < 1e-4, "{faint}");
        assert!(bright > 2.0 * faint, "{bright} and {faint}");
        let teal = glint_color(0.0).to_linear();
        let violet = glint_color(DRIFT_SECONDS / 2.0).to_linear();
        assert!(teal.green > teal.blue && violet.blue > violet.green);
    }
}
