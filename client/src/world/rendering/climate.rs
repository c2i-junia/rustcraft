//! The climate of the columns around the player, which `chunk.wgsl` reads to colour the grass
//! and the leaves from the colour maps of the texture pack, as Minecraft does: each biome has
//! its place on the colour maps, and a column takes the mean of the places of the columns
//! around it (`BiomeBlend`), so that the colour fades across the border of two biomes.
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
use shared::world::{BiomeType, TerrainNoises};
use shared::CHUNK_SIZE;

use crate::constants::MAX_CHUNK_RENDER_DISTANCE_RADIUS;
use crate::world::{ChunkLoadEvent, ClientWorldMap, MaterialResource};

/// The terrain noises of the world joined, built from the seed the server sent.
#[derive(Resource, Clone)]
pub struct WorldBiomes(pub Arc<TerrainNoises>);

/// The largest `BiomeBlend::radius` the player may choose: a square of 15 by 15 columns, as
/// Minecraft's largest biome blend.
pub const MAX_BIOME_BLEND: u32 = 7;

/// How far the colour of the grass and of the leaves blends across the border of two biomes, in
/// columns: each column takes the mean of the square of `2 * radius + 1` columns on a side
/// around it, as Minecraft's biome blend. 0 keeps the colour of each biome to its own columns.
/// Saved in `video.ron`.
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct BiomeBlend {
    pub radius: u32,
}

impl Default for BiomeBlend {
    /// A square of 5 by 5 columns, as Minecraft's default
    fn default() -> Self {
        Self { radius: 2 }
    }
}

impl BiomeBlend {
    /// Sets the radius, kept within the bounds the player may choose.
    pub fn set(&mut self, radius: u32) {
        self.radius = radius.min(MAX_BIOME_BLEND);
    }

    /// Columns along a side of the square a column takes the mean of.
    pub fn side(self) -> u32 {
        2 * self.radius + 1
    }
}

/// Columns of chunks along each side of the climate map: those of the largest render distance,
/// and one more on each side for the chunks that are left behind as the player walks away.
const MAP_CHUNKS: i32 = 2 * MAX_CHUNK_RENDER_DISTANCE_RADIUS as i32 + 3;

/// Texels along each side of the climate map.
pub const CLIMATE_MAP_SIZE: u32 = (MAP_CHUNKS * CHUNK_SIZE) as u32;

/// Where the grass and the leaves of `biome` are read on the colour maps: the temperature and
/// the downfall Minecraft gives a biome like it, each in `[0, 1]` as Minecraft clamps them.
fn biome_climate(biome: BiomeType) -> [f64; 2] {
    match biome {
        BiomeType::Desert | BiomeType::Savanna => [1.0, 0.0],
        BiomeType::Plains => [0.8, 0.4],
        BiomeType::Forest | BiomeType::FlowerPlains => [0.7, 0.8],
        // As the windswept hills and the snowy slopes
        BiomeType::MediumMountain => [0.2, 0.3],
        BiomeType::HighMountainGrass => [0.0, 0.3],
        BiomeType::IcePlain => [0.0, 0.5],
        BiomeType::ShallowOcean | BiomeType::Ocean | BiomeType::DeepOcean => [0.5, 0.5],
    }
}

/// The place of `biome` on the colour maps, from their bottom right corner: the temperature
/// along x, and the downfall times the temperature along y, as Minecraft reads them. The means
/// of the columns are taken of these, the places themselves.
fn colormap_place(biome: BiomeType) -> [f64; 2] {
    let [temperature, downfall] = biome_climate(biome);
    [temperature, temperature * downfall]
}

/// The texel of a column whose place on the colour maps is `place`, `colormap_place` or a mean
/// of them: in red and green, and an opaque alpha that tells a known column from one not
/// computed yet.
fn place_texel([x, y]: [f64; 2]) -> [u8; 4] {
    let byte = |value: f64| (value.clamp(0.0, 1.0) * 255.0).round() as u8;
    [byte(x), byte(y), 0, 255]
}

/// The texels of the 16 by 16 columns of the chunk column `column` (x, z), by `x * 16 + z`:
/// the mean of the places of the columns within `blend` of each.
fn column_texels(noises: &TerrainNoises, column: IVec2, blend: BiomeBlend) -> Vec<[u8; 4]> {
    let radius = blend.radius as i32;
    // The columns read, `radius` past the chunk column on each side, and the sums of their
    // places from the corner of the window: `sums[i][j]` holds those of the `i` by `j` first
    let window = CHUNK_SIZE + 2 * radius;
    let corner = column * CHUNK_SIZE - radius;
    let width = window as usize + 1;
    let mut sums = vec![[0.0; 2]; width * width];
    for i in 0..window as usize {
        for j in 0..window as usize {
            let at = corner + IVec2::new(i as i32, j as i32);
            let place = colormap_place(noises.column_shape(at.x, at.y).biome);
            for axis in 0..2 {
                sums[(i + 1) * width + j + 1][axis] =
                    place[axis] + sums[i * width + j + 1][axis] + sums[(i + 1) * width + j][axis]
                        - sums[i * width + j][axis];
            }
        }
    }
    let side = blend.side() as usize;
    let count = (side * side) as f64;
    (0..CHUNK_SIZE as usize)
        .flat_map(|x| (0..CHUNK_SIZE as usize).map(move |z| (x, z)))
        .map(|(x, z)| {
            let sum = |axis: usize| {
                sums[(x + side) * width + z + side][axis]
                    - sums[x * width + z + side][axis]
                    - sums[(x + side) * width + z][axis]
                    + sums[x * width + z][axis]
            };
            place_texel([sum(0) / count, sum(1) / count])
        })
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
pub(crate) fn fill_climate_map(
    map: &mut Image,
    noises: &TerrainNoises,
    columns: &[IVec2],
    blend: BiomeBlend,
) {
    for column in columns {
        write_column(map, *column, &column_texels(noises, *column, blend));
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
    fn request(&mut self, column: IVec2, noises: &Arc<TerrainNoises>, blend: BiomeBlend) {
        if self.slots.get(&Self::slot(column)) == Some(&column)
            || self.tasks.iter().any(|(pending, _)| *pending == column)
        {
            return;
        }
        let noises = Arc::clone(noises);
        let task =
            AsyncComputeTaskPool::get().spawn(async move { column_texels(&noises, column, blend) });
        self.tasks.push((column, task));
    }
}

/// Fills the climate map with the columns of the chunks that arrive. A new world, whose seed
/// gives other biomes, empties it first and fills it again with the chunks already held. A new
/// biome blend computes them all again, and keeps the old texels until the new ones replace
/// them; the tasks of the old blend are dropped, which cancels them.
pub fn update_climate_map_system(
    mut map: ResMut<ClimateMap>,
    biomes: Option<Res<WorldBiomes>>,
    blend: Res<BiomeBlend>,
    mut chunk_loads: MessageReader<ChunkLoadEvent>,
    world_map: Res<ClientWorldMap>,
    materials: Res<MaterialResource>,
    mut images: ResMut<Assets<Image>>,
) {
    let Some(biomes) = biomes else {
        chunk_loads.clear();
        return;
    };
    if biomes.is_changed() || blend.is_changed() {
        *map = ClimateMap::default();
        if biomes.is_changed() {
            if let Some(mut image) = images.get_mut(&materials.climate_map) {
                *image = new_climate_map();
            }
        }
        let columns: HashSet<IVec2> = world_map
            .blocks
            .chunk_positions()
            .map(|chunk| chunk.xz())
            .collect();
        for column in columns {
            map.request(column, &biomes.0, *blend);
        }
    }
    for load in chunk_loads.read() {
        if let ChunkLoadEvent::Loaded(chunk) = load {
            map.request(chunk.xz(), &biomes.0, *blend);
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

    /// Whether the texels `a` and `b` are the same, but for the rounding of their means: the
    /// sums of floats differ with their order.
    fn same_texel(a: [u8; 4], b: [u8; 4]) -> bool {
        a.iter().zip(b).all(|(a, b)| a.abs_diff(b) <= 1)
    }

    /// The texel of the column `(x, z)` with `blend`, computed column by column.
    fn blended_texel(noises: &TerrainNoises, x: i32, z: i32, blend: BiomeBlend) -> [u8; 4] {
        let radius = blend.radius as i32;
        let mut sum = [0.0; 2];
        for dx in -radius..=radius {
            for dz in -radius..=radius {
                let place = colormap_place(noises.column_shape(x + dx, z + dz).biome);
                sum[0] += place[0];
                sum[1] += place[1];
            }
        }
        let count = f64::from(blend.side() * blend.side());
        place_texel([sum[0] / count, sum[1] / count])
    }

    #[test]
    fn each_column_is_written_where_the_shader_reads_it_also_west_and_north_of_the_origin() {
        let noises = TerrainNoises::new(42);
        let mut map = new_climate_map();
        let columns = [IVec2::new(-1, -2), IVec2::new(3, 0)];
        let unblended = BiomeBlend { radius: 0 };
        fill_climate_map(&mut map, &noises, &columns, unblended);
        for (x, z) in [(-16, -32), (-1, -17), (-9, -20), (48, 0), (63, 15)] {
            let biome = noises.column_shape(x, z).biome;
            let (texel, expected) = (texel_at(&map, x, z), place_texel(colormap_place(biome)));
            assert!(
                same_texel(texel, expected),
                "{x}, {z}: {texel:?}, {expected:?}"
            );
        }
        // A column of no chunk asked for is unknown
        assert_eq!(texel_at(&map, 20, 20)[3], 0);
    }

    #[test]
    fn a_column_takes_the_mean_of_the_square_around_it_across_chunk_borders() {
        let noises = TerrainNoises::new(42);
        // Across the border of a desert and a savanna, then on the corners of a chunk
        let column = IVec2::new(-878, -454).div_euclid(IVec2::splat(CHUNK_SIZE));
        for radius in [1, MAX_BIOME_BLEND] {
            let blend = BiomeBlend { radius };
            let mut map = new_climate_map();
            fill_climate_map(&mut map, &noises, &[column], blend);
            let origin = column * CHUNK_SIZE;
            for (dx, dz) in [(0, 0), (15, 0), (0, 15), (15, 15), (7, 9)] {
                let (x, z) = (origin.x + dx, origin.y + dz);
                let (texel, expected) = (texel_at(&map, x, z), blended_texel(&noises, x, z, blend));
                assert!(
                    same_texel(texel, expected),
                    "radius {radius}, {x}, {z}: {texel:?}, {expected:?}"
                );
            }
        }
    }

    #[test]
    fn the_blend_fades_the_colour_across_the_border_of_two_biomes() {
        let noises = TerrainNoises::new(42);
        let blend = BiomeBlend { radius: 3 };
        // Along x from a column, the first one of another biome
        let (x, z) = (-878, -454);
        let start = noises.column_shape(x, z).biome;
        let border = (x..x + 200)
            .find(|&x| noises.column_shape(x, z).biome != start)
            .expect("no border");
        let unblended = |x| place_texel(colormap_place(noises.column_shape(x, z).biome));
        let blended = |x| blended_texel(&noises, x, z, blend);
        // Far from the border each biome keeps its colour; next to it, one between both
        assert_eq!(blended(border - 10), unblended(border - 10));
        assert_ne!(blended(border), unblended(border));
        assert_ne!(blended(border - 1), unblended(border - 1));
    }

    /// The climate map of `app`, once every column asked for is written.
    fn finished_map(app: &mut App) -> Image {
        for _ in 0..1000 {
            app.update();
            if app.world().resource::<ClimateMap>().tasks.is_empty() {
                let handle = app
                    .world()
                    .resource::<MaterialResource>()
                    .climate_map
                    .clone();
                return app
                    .world()
                    .resource::<Assets<Image>>()
                    .get(&handle)
                    .unwrap()
                    .clone();
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        panic!("the climate map is never done");
    }

    #[test]
    fn a_new_biome_blend_colours_the_columns_held_again() {
        let noises = TerrainNoises::new(42);
        // A column on the border of two biomes, and the chunk it is in
        let z = -454;
        let start = noises.column_shape(-878, z).biome;
        let x = (-878..-678)
            .find(|&x| noises.column_shape(x, z).biome != start)
            .unwrap();
        let chunk = IVec3::new(x.div_euclid(CHUNK_SIZE), 4, z.div_euclid(CHUNK_SIZE));

        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_message::<ChunkLoadEvent>()
            .insert_resource(WorldBiomes(Arc::new(TerrainNoises::new(42))))
            .insert_resource(BiomeBlend { radius: 3 })
            .init_resource::<ClimateMap>()
            .init_resource::<ClientWorldMap>()
            .add_systems(Update, update_climate_map_system);
        let mut images = Assets::<Image>::default();
        let climate_map = images.add(new_climate_map());
        app.insert_resource(images)
            .insert_resource(MaterialResource {
                climate_map,
                ..default()
            });
        app.world_mut()
            .resource_mut::<ClientWorldMap>()
            .blocks
            .insert_chunk(chunk, shared::world::Chunk::new());

        let blended = blended_texel(&noises, x, z, BiomeBlend { radius: 3 });
        let texel = texel_at(&finished_map(&mut app), x, z);
        assert!(same_texel(texel, blended), "{texel:?}, {blended:?}");

        app.world_mut().resource_mut::<BiomeBlend>().radius = 0;
        let unblended = place_texel(colormap_place(noises.column_shape(x, z).biome));
        assert!(!same_texel(unblended, blended));
        let texel = texel_at(&finished_map(&mut app), x, z);
        assert!(same_texel(texel, unblended), "{texel:?}, {unblended:?}");
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
    fn the_biome_blend_stays_within_its_bounds() {
        let mut blend = BiomeBlend::default();
        assert_eq!(blend.side(), 5);
        blend.set(50);
        assert_eq!(blend.radius, MAX_BIOME_BLEND);
        assert_eq!(blend.side(), 15);
        blend.set(0);
        assert_eq!(blend.side(), 1);
    }
}
