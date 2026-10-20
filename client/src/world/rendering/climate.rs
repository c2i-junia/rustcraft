//! The climate of the columns around the player, which `chunk.wgsl` reads to colour the grass
//! and the leaves from the colour maps of the texture pack, as Minecraft does: the colour
//! changes gradually with the climate, also across the border of two biomes.
//!
//! The climate lies in a texture of one texel per world column, wrapped around: the column
//! `(x, z)` is at `(x, z)` modulo its size. It holds more columns than the render distance
//! reaches across, so that no two chunks drawn at once share a texel. The climate of a column
//! is computed from the seed when its first chunk arrives, on the task pool.

use std::sync::Arc;

use bevy::asset::RenderAssetUsages;
use bevy::platform::collections::{HashMap, HashSet};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::tasks::{block_on, futures_lite::future, AsyncComputeTaskPool, Task};
use shared::world::{ColumnClimate, TerrainNoises, MOUNTAIN_FROM, OCEAN_HUMIDITY};
use shared::CHUNK_SIZE;

use crate::constants::MAX_CHUNK_RENDER_DISTANCE_RADIUS;
use crate::world::{ChunkLoadEvent, ClientWorldMap, MaterialResource};

/// The terrain noises of the world joined, built from the seed the server sent.
#[derive(Resource, Clone)]
pub struct WorldBiomes(pub Arc<TerrainNoises>);

/// Columns of chunks along each side of the climate map: those of the largest render distance,
/// and one more on each side for the chunks that are left behind as the player walks away.
const MAP_CHUNKS: i32 = 2 * MAX_CHUNK_RENDER_DISTANCE_RADIUS as i32 + 3;

/// Texels along each side of the climate map.
pub const CLIMATE_MAP_SIZE: u32 = (MAP_CHUNKS * CHUNK_SIZE) as u32;

/// How much the temperature of the grass drops with each block of a surface above
/// `MOUNTAIN_FROM`: the grass is bluer on the mountains, as on the colour maps of the colder
/// climates.
const MOUNTAIN_COOLING: f64 = 0.006;

/// The texel of a column on the climate map: the temperature in red and the humidity of the
/// land in green, as the colour maps read them, and an opaque alpha that tells a known climate
/// from a column not computed yet. The sea is as wet as the land gets.
pub fn climate_texel(climate: ColumnClimate) -> [u8; 4] {
    let cooling = f64::from((climate.height - MOUNTAIN_FROM).max(0)) * MOUNTAIN_COOLING;
    let temperature = (climate.temperature - cooling).clamp(0.0, 1.0);
    let downfall = (climate.humidity / OCEAN_HUMIDITY).clamp(0.0, 1.0);
    let byte = |value: f64| (value * 255.0).round() as u8;
    [byte(temperature), byte(downfall), 0, 255]
}

/// The texels of the 16 by 16 columns of the chunk column `column` (x, z), by `x * 16 + z`.
fn column_texels(noises: &TerrainNoises, column: IVec2) -> Vec<[u8; 4]> {
    let origin = column * CHUNK_SIZE;
    (0..CHUNK_SIZE)
        .flat_map(|x| (0..CHUNK_SIZE).map(move |z| origin + IVec2::new(x, z)))
        .map(|at| climate_texel(noises.column_climate(at.x, at.y)))
        .collect()
}

/// A new climate map, where no column is known yet.
pub fn new_climate_map() -> Image {
    Image::new_fill(
        Extent3d {
            width: CLIMATE_MAP_SIZE,
            height: CLIMATE_MAP_SIZE,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &[0; 4],
        // Not sRGB: the texels are numbers, not colours
        TextureFormat::Rgba8Unorm,
        RenderAssetUsages::default(),
    )
}

/// Writes the texels of the chunk column `column` into `map`.
fn write_column(map: &mut Image, column: IVec2, texels: &[[u8; 4]]) {
    let Some(data) = map.data.as_mut() else {
        return;
    };
    let size = CLIMATE_MAP_SIZE as i32;
    for (index, texel) in texels.iter().enumerate() {
        let local = IVec2::new(index as i32 / CHUNK_SIZE, index as i32 % CHUNK_SIZE);
        let at = (column * CHUNK_SIZE + local).rem_euclid(IVec2::splat(size));
        let start = ((at.y * size + at.x) * 4) as usize;
        data[start..start + 4].copy_from_slice(texel);
    }
}

/// Writes the climate of every column of `columns` into `map` at once, for the scenes of the
/// benchmarks.
#[cfg(test)]
pub(crate) fn fill_climate_map(map: &mut Image, noises: &TerrainNoises, columns: &[IVec2]) {
    for column in columns {
        write_column(map, *column, &column_texels(noises, *column));
    }
}

/// The chunk columns written on the climate map, and those being computed.
#[derive(Resource, Default)]
pub struct ClimateMap {
    /// The chunk column whose climate each slot of the map holds, by slot.
    slots: HashMap<IVec2, IVec2>,
    tasks: Vec<(IVec2, Task<Vec<[u8; 4]>>)>,
}

impl ClimateMap {
    fn slot(column: IVec2) -> IVec2 {
        column.rem_euclid(IVec2::splat(MAP_CHUNKS))
    }

    /// Starts computing the climate of `column`, unless it is on the map or on its way.
    fn request(&mut self, column: IVec2, noises: &Arc<TerrainNoises>) {
        if self.slots.get(&Self::slot(column)) == Some(&column)
            || self.tasks.iter().any(|(pending, _)| *pending == column)
        {
            return;
        }
        let noises = Arc::clone(noises);
        let task = AsyncComputeTaskPool::get().spawn(async move { column_texels(&noises, column) });
        self.tasks.push((column, task));
    }
}

/// Fills the climate map with the columns of the chunks that arrive. A new world, whose seed
/// gives other climates, empties it first and fills it again with the chunks already held.
pub fn update_climate_map_system(
    mut map: ResMut<ClimateMap>,
    biomes: Option<Res<WorldBiomes>>,
    mut chunk_loads: MessageReader<ChunkLoadEvent>,
    world_map: Res<ClientWorldMap>,
    materials: Res<MaterialResource>,
    mut images: ResMut<Assets<Image>>,
) {
    let Some(biomes) = biomes else {
        chunk_loads.clear();
        return;
    };
    if biomes.is_changed() {
        *map = ClimateMap::default();
        if let Some(mut image) = images.get_mut(&materials.climate_map) {
            *image = new_climate_map();
        }
        let columns: HashSet<IVec2> = world_map
            .blocks
            .chunk_positions()
            .map(|chunk| chunk.xz())
            .collect();
        for column in columns {
            map.request(column, &biomes.0);
        }
    }
    for load in chunk_loads.read() {
        if let ChunkLoadEvent::Loaded(chunk) = load {
            map.request(chunk.xz(), &biomes.0);
        }
    }

    let mut done = Vec::new();
    map.tasks
        .retain_mut(|(column, task)| match block_on(future::poll_once(task)) {
            Some(texels) => {
                done.push((*column, texels));
                false
            }
            None => true,
        });
    if done.is_empty() {
        return;
    }
    let Some(mut image) = images.get_mut(&materials.climate_map) else {
        return;
    };
    for (column, texels) in done {
        write_column(&mut image, column, &texels);
        map.slots.insert(ClimateMap::slot(column), column);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texel_at(map: &Image, x: i32, z: i32) -> [u8; 4] {
        let size = CLIMATE_MAP_SIZE as i32;
        let (x, z) = (x.rem_euclid(size), z.rem_euclid(size));
        let start = ((z * size + x) * 4) as usize;
        map.data.as_ref().unwrap()[start..start + 4]
            .try_into()
            .unwrap()
    }

    #[test]
    fn each_column_is_written_where_the_shader_reads_it_also_west_and_north_of_the_origin() {
        let noises = TerrainNoises::new(42);
        let mut map = new_climate_map();
        let columns = [IVec2::new(-1, -2), IVec2::new(3, 0)];
        fill_climate_map(&mut map, &noises, &columns);
        for (x, z) in [(-16, -32), (-1, -17), (-9, -20), (48, 0), (63, 15)] {
            assert_eq!(
                texel_at(&map, x, z),
                climate_texel(noises.column_climate(x, z)),
                "{x}, {z}"
            );
        }
        // A column of no chunk asked for is unknown
        assert_eq!(texel_at(&map, 20, 20)[3], 0);
    }

    #[test]
    fn the_columns_drawn_at_once_never_share_a_texel() {
        let reach = MAX_CHUNK_RENDER_DISTANCE_RADIUS as i32;
        let slots: HashSet<IVec2> = (-reach..=reach)
            .flat_map(|x| (-reach..=reach).map(move |z| ClimateMap::slot(IVec2::new(x, z) + 1000)))
            .collect();
        assert_eq!(slots.len(), ((2 * reach + 1) * (2 * reach + 1)) as usize);
    }

    #[test]
    fn the_grass_of_the_mountains_is_colder_and_the_sea_is_as_wet_as_the_land_gets() {
        let plain = ColumnClimate {
            temperature: 0.5,
            humidity: 0.2,
            height: 70,
        };
        let peak = ColumnClimate {
            height: MOUNTAIN_FROM + 50,
            ..plain
        };
        assert!(climate_texel(peak)[0] < climate_texel(plain)[0]);
        assert_eq!(climate_texel(peak)[1], climate_texel(plain)[1]);
        let sea = ColumnClimate {
            humidity: 0.9,
            ..plain
        };
        assert_eq!(climate_texel(sea)[1], 255);
    }
}
