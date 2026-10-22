//! The colour of each block on the minimap: the mean colour of the texture of its top face,
//! tinted as that face is in the world.

use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use shared::world::{BlockId, BlockTransparency};

use crate::world::MaterialResource;

/// The colour of a block whose texture could not be read.
const MISSING: [u8; 3] = [0x9A, 0xA2, 0xAE];

/// How much darker a canopy of leaves is than its texture: the shade under it shows through its
/// holes, and the trees stand out of the grass, tinted alike.
const CANOPY_SHADE: f32 = 0.7;

/// The colour of each block on the map, in sRGB.
#[derive(Resource, Default)]
pub struct MinimapColors(HashMap<BlockId, [u8; 3]>);

impl MinimapColors {
    pub fn of(colors: impl IntoIterator<Item = (BlockId, [u8; 3])>) -> Self {
        Self(colors.into_iter().collect())
    }

    pub fn get(&self, block: BlockId) -> [u8; 3] {
        self.0.get(&block).copied().unwrap_or(MISSING)
    }
}

/// The mean of the pixels of `image` that are not seen through, tinted by `tint` as a vertex
/// colour tints a texture: in linear space. `None` for an image with no such pixel.
pub fn mean_color(image: &Image, tint: [f32; 4]) -> Option<[u8; 3]> {
    let size = image.size();
    let mut sum = Vec3::ZERO;
    let mut count = 0.0;
    for y in 0..size.y {
        for x in 0..size.x {
            let Ok(color) = image.get_color_at(x, y) else {
                continue;
            };
            let linear = color.to_linear();
            if linear.alpha < 0.5 {
                continue;
            }
            sum += Vec3::new(linear.red, linear.green, linear.blue);
            count += 1.0;
        }
    }
    if count == 0.0 {
        return None;
    }
    let mean = sum / count * Vec3::new(tint[0], tint[1], tint[2]);
    let srgb = Color::linear_rgb(mean.x, mean.y, mean.z).to_srgba();
    Some([srgb.red, srgb.green, srgb.blue].map(|channel| (channel * 255.0).round() as u8))
}

/// Reads the colour of every block from the textures of the block atlas, loaded before the game.
pub fn compute_minimap_colors(
    materials: Res<MaterialResource>,
    images: Res<Assets<Image>>,
    mut commands: Commands,
) {
    let Some(blocks) = &materials.blocks else {
        return;
    };
    let colors = BlockId::ALL.into_iter().filter_map(|id| {
        let shape = blocks.looks.shape(id);
        // the top face of a cube, or the only face of a plant
        let top = shape.faces.first()?;
        let image = images.get(blocks.handles.get(&top.texture)?)?;
        let mut color = mean_color(image, top.color.rgba())?;
        if id.get_visibility() == BlockTransparency::Transparent {
            color = color.map(|channel| (f32::from(channel) * CANOPY_SHADE).round() as u8);
        }
        Some((id, color))
    });
    commands.insert_resource(MinimapColors::of(colors));
}

#[cfg(test)]
mod tests {
    use bevy::asset::RenderAssetUsages;
    use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

    use super::*;

    fn image(pixels: &[[u8; 4]]) -> Image {
        Image::new(
            Extent3d {
                width: pixels.len() as u32,
                height: 1,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            pixels.concat(),
            TextureFormat::Rgba8UnormSrgb,
            RenderAssetUsages::default(),
        )
    }

    #[test]
    fn the_mean_colour_skips_what_is_seen_through_and_takes_the_tint() {
        let white = [255, 255, 255, 255];
        let black = [0, 0, 0, 255];
        let hole = [255, 0, 0, 0];
        assert_eq!(mean_color(&image(&[white, hole]), [1.0; 4]), Some([255; 3]));
        // the mean of white and black is taken in linear space, as the GPU blends
        assert_eq!(
            mean_color(&image(&[white, black]), [1.0; 4]),
            Some([188; 3])
        );
        assert_eq!(
            mean_color(&image(&[white]), [0.0, 1.0, 0.0, 1.0]),
            Some([0, 255, 0])
        );
        assert_eq!(mean_color(&image(&[hole]), [1.0; 4]), None);
    }
}
