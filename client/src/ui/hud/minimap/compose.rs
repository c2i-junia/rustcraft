//! The pixels of the minimap: the surface around the player, in the colours of the blocks, with
//! the relief shaded from the north and the water darkened by its depth.

use bevy::prelude::*;
use shared::world::BlockId;

use super::colors::MinimapColors;
use super::surface::{MinimapSurface, Surface, MAX_WATER_DEPTH};

/// The width and the height of the map, in pixels, which are logical px on screen.
pub const MAP_SIZE: u32 = 128;

/// The colour of what is not loaded: `SLOT` of the theme.
pub const UNLOADED: [u8; 3] = [0x0C, 0x0E, 0x12];

/// A block higher than the one north of it is lighter by this factor, a lower one darker.
const LIGHTER: f32 = 1.12;
const DARKER: f32 = 0.82;
/// The share of the colour of the water over its floor in the shallowest water: half, as the
/// game draws water. It grows to all of it in the deepest water.
const SHALLOW_WATER: f32 = 0.5;
/// The share of its colour the deepest water loses.
const DEEP_WATER_DARKENING: f32 = 0.5;

/// The colour of `surface`, of the colour `color`, whose northern neighbour on the map is
/// `north`. Under water, `color` is the floor's, seen through water of the colour `water`.
pub fn shade(color: [u8; 3], water: [u8; 3], surface: Surface, north: Option<Surface>) -> [u8; 3] {
    if surface.water_depth > 0 {
        let deep = f32::from(surface.water_depth) / f32::from(MAX_WATER_DEPTH);
        let share = SHALLOW_WATER + (1.0 - SHALLOW_WATER) * deep;
        let darkening = 1.0 - DEEP_WATER_DARKENING * deep;
        return std::array::from_fn(|i| {
            let mixed = f32::from(color[i]) * (1.0 - share) + f32::from(water[i]) * share;
            (mixed * darkening).round() as u8
        });
    }
    let factor = match north {
        Some(north) if surface.y > north.y => LIGHTER,
        Some(north) if surface.y < north.y => DARKER,
        _ => 1.0,
    };
    color.map(|channel| (f32::from(channel) * factor).round().min(255.0) as u8)
}

/// Writes into `pixels`, RGBA rows from the north, the map centred on the column `center`
/// (x, z), each pixel showing a column every `zoom` blocks.
pub fn compose(
    surface: &MinimapSurface,
    colors: &MinimapColors,
    center: IVec2,
    zoom: i32,
    pixels: &mut [u8],
) {
    let half = MAP_SIZE as i32 / 2;
    let water = colors.get(BlockId::Water);
    for (index, pixel) in pixels.as_chunks_mut::<4>().0.iter_mut().enumerate() {
        let column = IVec2::new(
            index as i32 % MAP_SIZE as i32 - half,
            index as i32 / MAP_SIZE as i32 - half,
        );
        let at = center + column * zoom;
        let color = match surface.get(at.x, at.y) {
            Some(here) => shade(
                colors.get(here.block),
                water,
                here,
                surface.get(at.x, at.y - zoom),
            ),
            None => UNLOADED,
        };
        *pixel = [color[0], color[1], color[2], 255];
    }
}

#[cfg(test)]
mod tests {
    use shared::world::{BlockData, BlockDirection, Chunk, ChunkMap};

    use super::*;

    fn at(y: i32) -> Surface {
        Surface {
            block: BlockId::Grass,
            y,
            water_depth: 0,
        }
    }

    #[test]
    fn the_relief_is_shaded_from_the_north_and_water_by_its_depth() {
        let color = [100, 200, 250];
        let blue = [0, 0, 200];
        assert_eq!(shade(color, blue, at(10), Some(at(10))), color);
        assert_eq!(shade(color, blue, at(10), None), color);
        assert_eq!(shade(color, blue, at(11), Some(at(10))), [112, 224, 255]);
        assert_eq!(shade(color, blue, at(9), Some(at(10))), [82, 164, 205]);
        let water = |water_depth| Surface {
            block: BlockId::Sand,
            y: 62,
            water_depth,
        };
        // water is flat: its floor shows through it, less and darker as it deepens
        assert_eq!(shade(color, blue, water(1), Some(at(0))), [45, 91, 216]);
        assert_eq!(shade(color, blue, water(8), None), [19, 38, 159]);
        assert_eq!(
            shade(color, blue, water(MAX_WATER_DEPTH), None),
            [0, 0, 100]
        );
    }

    #[test]
    fn the_map_is_centred_on_the_player_with_the_north_up_and_zooms_out() {
        let mut map = ChunkMap::default();
        map.insert_chunk(IVec3::ZERO, Chunk::new());
        // north (-z) and east (+x) of the centre, in the chunk next to the origin
        let block = |id| BlockData::new(id, BlockDirection::Front);
        map.set_block(IVec3::new(4, 0, 2), block(BlockId::Stone));
        map.set_block(IVec3::new(5, 0, 1), block(BlockId::Sand));
        let mut surface = MinimapSurface::of(&map);
        surface.scan_dirty(&map, IVec2::ZERO, usize::MAX);
        let colors = MinimapColors::of([(BlockId::Stone, [1, 2, 3]), (BlockId::Sand, [9, 8, 7])]);

        let mut pixels = vec![0; (MAP_SIZE * MAP_SIZE * 4) as usize];
        let pixel = |pixels: &[u8], x: i32, y: i32| {
            let index = ((y + 64) * MAP_SIZE as i32 + x + 64) as usize * 4;
            <[u8; 4]>::try_from(&pixels[index..index + 4]).unwrap()
        };
        compose(&surface, &colors, IVec2::new(4, 2), 1, &mut pixels);
        assert_eq!(pixel(&pixels, 0, 0), [1, 2, 3, 255]);
        assert_eq!(pixel(&pixels, 1, -1), [9, 8, 7, 255]);
        // the rest of the loaded chunk is air, and beyond it nothing is loaded
        assert_eq!(
            pixel(&pixels, -1, 0),
            [UNLOADED[0], UNLOADED[1], UNLOADED[2], 255]
        );

        // two blocks a pixel: the sand, one block east and one north, is not drawn
        compose(&surface, &colors, IVec2::new(4, 2), 2, &mut pixels);
        assert_eq!(pixel(&pixels, 0, 0), [1, 2, 3, 255]);
        assert_eq!(pixel(&pixels, 1, -1)[..3], UNLOADED);
    }
}
