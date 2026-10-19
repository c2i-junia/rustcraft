use bevy::prelude::*;
use noise::{NoiseFn, Perlin};
use shared::{world::*, CHUNK_SIZE};

/// Water fills every column up to this height; flora only grows on land above it.
const SEA_LEVEL: i32 = 62;
const TERRAIN_SCALE: f64 = 0.1;
/// How far, along x and z, the biomes that blend into the height of a column lie.
const BIOME_BLEND: i32 = 4;

/// Largest horizontal distance between the column a structure is rooted in and any of its
/// blocks. It is set by the big tree: a branch starts at most 1 block from the trunk and is
/// at most 2 blocks long, and its tip carries one more leaf (`x + 1 + 2`). Generating a chunk
/// therefore looks at the columns of the chunk plus this margin on each side.
const STRUCTURE_MARGIN: i32 = 3;

/// Lowest and highest surface of any column: a surface height is a weighted mean of the base
/// heights of the biomes around, plus a weighted mean of their variations times a noise in
/// `[-1, 1]`, so it stays within the lowest base less the largest variation and the highest
/// base plus the largest variation. `surface_bounds_hold_for_every_biome` checks them.
const LOWEST_SURFACE: i32 = 43;
const HIGHEST_SURFACE: i32 = 82;

/// Highest block of a structure above the surface of its column: the crown of a big tree,
/// whose 6-block trunk starts on the block above the surface and whose leaves reach 3 blocks
/// above the top of the trunk (`1 + 6 - 2 + 2 + 2`).
const STRUCTURE_HEIGHT: i32 = 9;

// Salts: each random decision of a column hashes `(seed, x, z, salt)` with its own salt.
const SALT_FLOWER_ROLL: u64 = 1;
const SALT_FLOWER_KIND: u64 = 2;
const SALT_TALL_GRASS_ROLL: u64 = 3;
const SALT_TREE_ROLL: u64 = 4;
const SALT_TRUNK_HEIGHT: u64 = 5;
const SALT_CACTUS_ROLL: u64 = 6;
const SALT_CACTUS_HEIGHT: u64 = 7;
/// Each of the two branches of a big tree uses 4 consecutive salts from here.
const SALT_BRANCH: u64 = 0x100;
/// One salt per leaf of the 5x5x3 crown, from here.
const SALT_LEAF: u64 = 0x200;

/// The `splitmix64` finaliser: a bijective mixing of 64 bits, good enough that consecutive
/// inputs give unrelated outputs.
fn splitmix64(value: u64) -> u64 {
    let mut z = value.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// The random decisions of one world column. Every value is a pure function of
/// `(seed, x, z, salt)`, so a column decides the same thing whichever chunk asks.
struct ColumnRandom {
    seed: u32,
    x: i32,
    z: i32,
}

impl ColumnRandom {
    /// Folds the seed, the coordinates and the salt into one 64-bit hash, one `splitmix64`
    /// round after each of them.
    fn bits(&self, salt: u64) -> u64 {
        let mut hash = splitmix64(self.seed as u64);
        hash = splitmix64(hash ^ self.x as u32 as u64);
        hash = splitmix64(hash ^ self.z as u32 as u64);
        splitmix64(hash ^ salt)
    }

    /// A number in `[0, 1)`.
    fn unit(&self, salt: u64) -> f32 {
        (self.bits(salt) >> 40) as f32 / (1u64 << 24) as f32
    }

    /// A number in `0..count`.
    fn below(&self, salt: u64, count: u64) -> i32 {
        ((self.bits(salt) >> 32) % count) as i32
    }

    /// -1 or 1 one time in four each, 0 the rest of the time.
    fn jitter(&self, salt: u64) -> i32 {
        match self.below(salt, 4) {
            0 => -1,
            3 => 1,
            _ => 0,
        }
    }
}

/// What can grow on a column, before it is expanded into blocks.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Decoration {
    Flower(BlockId),
    TallGrass,
    Tree,
    BigTree,
    Cactus,
}

/// Terrain of one world column: the noise is evaluated once per column and reused.
struct Column {
    x: i32,
    z: i32,
    height: i32,
    biome_type: BiomeType,
}

struct Noises {
    terrain: Perlin,
    biome: BiomeNoises,
}

impl Noises {
    fn new(seed: u32) -> Self {
        Self {
            terrain: Perlin::new(seed),
            biome: BiomeNoises::new(seed),
        }
    }

    /// The temperature and the humidity of the column at `(x, z)`, each in `[0, 1]`.
    fn climate(&self, x: i32, z: i32) -> (f64, f64) {
        let point = [x as f64 * BIOME_SCALE, z as f64 * BIOME_SCALE];
        (
            (self.biome.temperature.get(point) + 1.0) / 2.0,
            (self.biome.humidity.get(point) + 1.0) / 2.0,
        )
    }
}

fn interpolated_height(
    x: i32,
    z: i32,
    climate: &impl Fn(i32, i32) -> (f64, f64),
    perlin: &Perlin,
    scale: f64,
) -> i32 {
    // get the properties of the main biome at (x, z)
    let (temperature, humidity) = climate(x, z);
    let biome_type = determine_biome(temperature, humidity);
    let biome = get_biome_data(biome_type);

    // initialize weighted values
    let mut weighted_base_height = biome.base_height as f64;
    let mut weighted_variation = biome.height_variation as f64;
    let mut total_weight = 1.0;

    // loop through neighboring blocks to get influences
    for &offset_x in &[-BIOME_BLEND, 0, BIOME_BLEND] {
        for &offset_z in &[-BIOME_BLEND, 0, BIOME_BLEND] {
            if offset_x == 0 && offset_z == 0 {
                continue; // ignore the central position
            }

            // determine the biome of the neighboring block
            let (neighbor_temp, neighbor_humidity) = climate(x + offset_x, z + offset_z);
            let neighbor_biome_type = determine_biome(neighbor_temp, neighbor_humidity);
            let neighbor_biome = get_biome_data(neighbor_biome_type);

            // weight by distance (the farther a neighbor is, the less influence it has)
            let distance = ((offset_x.pow(2) + offset_z.pow(2)) as f64).sqrt();
            let weight = 1.0 / (distance + 1.0); // distance +1 to avoid division by zero

            // update weighted values
            weighted_base_height += neighbor_biome.base_height as f64 * weight;
            weighted_variation += neighbor_biome.height_variation as f64 * weight;
            total_weight += weight;
        }
    }

    // normalize weighted values
    weighted_base_height /= total_weight;
    weighted_variation /= total_weight;

    // final calculation of height with perlin noise
    let terrain_noise = perlin.get([x as f64 * scale, z as f64 * scale]);
    let interpolated_height = weighted_base_height + (weighted_variation * terrain_noise);

    interpolated_height.round() as i32
}

#[cfg(test)]
fn terrain_column(noises: &Noises, x: i32, z: i32) -> Column {
    terrain_column_in(noises, x, z, &|x, z| noises.climate(x, z))
}

/// `terrain_column`, reading the climate of the columns from `climate`.
fn terrain_column_in(
    noises: &Noises,
    x: i32,
    z: i32,
    climate: &impl Fn(i32, i32) -> (f64, f64),
) -> Column {
    let (temperature, humidity) = climate(x, z);
    let height = interpolated_height(x, z, climate, &noises.terrain, TERRAIN_SCALE);
    Column {
        x,
        z,
        height,
        biome_type: determine_biome(temperature, humidity),
    }
}

/// The block at height `y` of a terrain column, or `None` above the surface and the sea.
fn terrain_block(y: i32, height: i32, biome: &Biome) -> Option<BlockId> {
    if y > height && y > SEA_LEVEL {
        None
    } else if y == 0 {
        Some(BlockId::Bedrock)
    } else if y < height - 4 {
        Some(BlockId::Stone)
    } else if y < height {
        Some(biome.sub_surface_block)
    } else if y == height {
        Some(biome.surface_block)
    } else {
        Some(BlockId::Water)
    }
}

/// Which of two blocks claiming the same cell stays: the higher rank wins. Terrain (and any
/// block not listed) beats every decoration, a trunk or a cactus beats leaves, leaves beat
/// tall grass and tall grass beats flowers. Equal ranks keep the block placed first; columns
/// are visited in world coordinate order, so that choice is the same in every chunk.
fn precedence(block: BlockId) -> u8 {
    match block {
        BlockId::Dandelion | BlockId::Poppy => 1,
        BlockId::TallGrass => 2,
        BlockId::OakLeaves => 3,
        BlockId::OakLog | BlockId::Cactus => 4,
        _ => 5,
    }
}

/// Writes `block` at a world position if it falls inside the chunk and does not displace a
/// block of higher or equal rank.
fn place(chunk: &mut Chunk, chunk_origin: IVec3, world_pos: IVec3, block: BlockId) {
    let local = world_pos - chunk_origin;
    if !Chunk::contains(local) {
        return;
    }
    let outranked = chunk
        .get(local)
        .is_some_and(|existing| precedence(existing.id) >= precedence(block));
    if !outranked {
        chunk.set(local, BlockData::new(block, BlockDirection::Front));
    }
}

fn random_of(seed: u32, column: &Column) -> ColumnRandom {
    ColumnRandom {
        seed,
        x: column.x,
        z: column.z,
    }
}

/// Decides, from `(seed, x, z)` only, what is rooted on the column.
fn column_decorations(column: &Column, random: &ColumnRandom) -> Vec<Decoration> {
    let mut decorations = Vec::new();
    if column.height <= SEA_LEVEL {
        return decorations;
    }
    let biome_type = column.biome_type;

    let flower_chance = match biome_type {
        BiomeType::FlowerPlains => 0.1,
        BiomeType::Plains | BiomeType::Forest | BiomeType::MediumMountain => 0.02,
        _ => 0.0,
    };
    if random.unit(SALT_FLOWER_ROLL) < flower_chance {
        let flower = if random.unit(SALT_FLOWER_KIND) < 0.5 {
            BlockId::Dandelion
        } else {
            BlockId::Poppy
        };
        decorations.push(Decoration::Flower(flower));
    }

    let grassless = [
        BiomeType::HighMountainGrass,
        BiomeType::Desert,
        BiomeType::IcePlain,
    ];
    if !grassless.contains(&biome_type) && random.unit(SALT_TALL_GRASS_ROLL) < 0.10 {
        decorations.push(Decoration::TallGrass);
    }

    let tree_roll = random.unit(SALT_TREE_ROLL);
    match biome_type {
        BiomeType::Forest if tree_roll < 0.01 => decorations.push(Decoration::BigTree),
        BiomeType::Forest if tree_roll < 0.06 => decorations.push(Decoration::Tree),
        BiomeType::FlowerPlains | BiomeType::MediumMountain if tree_roll < 0.02 => {
            decorations.push(Decoration::Tree)
        }
        _ => {}
    }

    if biome_type == BiomeType::Desert && random.unit(SALT_CACTUS_ROLL) < 0.01 {
        decorations.push(Decoration::Cactus);
    }
    decorations
}

type Blocks = Vec<(IVec3, BlockId)>;

/// A 5x5 crown of leaves, 3 layers high starting at `bottom`: a diamond with a few extra
/// leaves jittered onto the edge of its two lower layers.
fn crown_blocks(bottom: IVec3, random: &ColumnRandom, blocks: &mut Blocks) {
    for layer in 0..3 {
        for offset_x in -2i32..=2 {
            for offset_z in -2i32..=2 {
                let distance = offset_x.abs() + offset_z.abs();
                let leaf_index = (layer * 5 + offset_x + 2) * 5 + offset_z + 2;
                let jittered = distance == 3 - layer
                    && layer < 2
                    && random.unit(SALT_LEAF + leaf_index as u64) < 0.2;
                if distance < 3 - layer || jittered {
                    let pos = bottom + IVec3::new(offset_x, layer, offset_z);
                    blocks.push((pos, BlockId::OakLeaves));
                }
            }
        }
    }
}

fn trunk_blocks(root: IVec3, trunk_height: i32, blocks: &mut Blocks) {
    for dy in 0..trunk_height {
        blocks.push((root + IVec3::Y * dy, BlockId::OakLog));
    }
}

fn tree_blocks(root: IVec3, random: &ColumnRandom, blocks: &mut Blocks) {
    let trunk_height = 3 + random.below(SALT_TRUNK_HEIGHT, 3); // 3 to 5
    trunk_blocks(root, trunk_height, blocks);
    crown_blocks(root + IVec3::Y * (trunk_height - 1), random, blocks);
}

fn big_tree_blocks(root: IVec3, random: &ColumnRandom, blocks: &mut Blocks) {
    let trunk_height = 4 + random.below(SALT_TRUNK_HEIGHT, 3); // 4 to 6
    let leaf_start_y = root.y + trunk_height - 2;

    for branch in 0..2u64 {
        let salt = SALT_BRANCH + branch * 4;
        let branch_x = root.x + random.jitter(salt);
        let branch_z = root.z + random.jitter(salt + 1);
        let branch_y = leaf_start_y - 1 - random.jitter(salt + 2);
        let length = 1 + random.below(salt + 3, 2);
        for x in branch_x..branch_x + length {
            blocks.push((IVec3::new(x, branch_y, branch_z + 1), BlockId::OakLeaves));
            blocks.push((IVec3::new(x, branch_y, branch_z - 1), BlockId::OakLeaves));
            blocks.push((IVec3::new(x, branch_y + 1, branch_z), BlockId::OakLeaves));
            blocks.push((IVec3::new(x, branch_y, branch_z), BlockId::OakLog));
        }
        let tip = IVec3::new(branch_x + length, branch_y, branch_z);
        blocks.push((tip, BlockId::OakLeaves));
    }

    trunk_blocks(root, trunk_height, blocks);

    // two flat rings of leaves around the top of the trunk, without their corners
    for layer in 0..2 {
        for offset_x in -2i32..=2 {
            for offset_z in -2i32..=2 {
                let is_axis = offset_x == 0 && offset_z == 0;
                let is_corner = offset_x.abs() == 2 && offset_z.abs() == 2;
                if !is_axis && !is_corner {
                    let pos =
                        IVec3::new(root.x + offset_x, leaf_start_y + layer, root.z + offset_z);
                    blocks.push((pos, BlockId::OakLeaves));
                }
            }
        }
    }
    crown_blocks(IVec3::new(root.x, leaf_start_y + 2, root.z), random, blocks);
}

fn cactus_blocks(root: IVec3, random: &ColumnRandom, blocks: &mut Blocks) {
    let cactus_height = 2 + random.below(SALT_CACTUS_HEIGHT, 2);
    for dy in 0..cactus_height {
        blocks.push((root + IVec3::Y * dy, BlockId::Cactus));
    }
}

/// Every block, in world coordinates, of everything rooted on the column. Chunk bounds and
/// ranks are applied later by [`place`].
fn column_blocks(seed: u32, column: &Column) -> Blocks {
    let random = random_of(seed, column);
    let root = IVec3::new(column.x, column.height + 1, column.z);
    let mut blocks = Vec::new();
    for decoration in column_decorations(column, &random) {
        match decoration {
            Decoration::Flower(flower) => blocks.push((root, flower)),
            Decoration::TallGrass => blocks.push((root, BlockId::TallGrass)),
            Decoration::Tree => tree_blocks(root, &random, &mut blocks),
            Decoration::BigTree => big_tree_blocks(root, &random, &mut blocks),
            Decoration::Cactus => cactus_blocks(root, &random, &mut blocks),
        }
    }
    blocks
}

/// Generates a chunk. It is a pure function of its arguments: terrain comes from the noise,
/// and decorations are decided per world column, so a structure rooted in a neighbouring
/// column is cut at the chunk border the same way whichever chunk is generated first.
pub fn generate_chunk(chunk_pos: IVec3, seed: u32) -> Chunk {
    let chunk_origin = chunk_pos * CHUNK_SIZE;
    // Most chunks lie above everything or deep underground: they need no noise
    if chunk_origin.y > (HIGHEST_SURFACE + STRUCTURE_HEIGHT).max(SEA_LEVEL) {
        return Chunk::new();
    }
    if chunk_origin.y + CHUNK_SIZE - 1 < LOWEST_SURFACE - 4 {
        return underground_chunk(chunk_origin);
    }
    generate_chunk_from_noise(chunk_pos, seed)
}

/// A chunk below the lowest surface less the depth of the sub-surface blocks: stone, and the
/// bedrock at height 0.
fn underground_chunk(chunk_origin: IVec3) -> Chunk {
    let mut chunk = Chunk::new();
    for x in 0..CHUNK_SIZE {
        for y in 0..CHUNK_SIZE {
            let block = if chunk_origin.y + y == 0 {
                BlockId::Bedrock
            } else {
                BlockId::Stone
            };
            for z in 0..CHUNK_SIZE {
                chunk.set(
                    IVec3::new(x, y, z),
                    BlockData::new(block, BlockDirection::Front),
                );
            }
        }
    }
    chunk
}

/// `generate_chunk` for any chunk, from the noise of its columns.
fn generate_chunk_from_noise(chunk_pos: IVec3, seed: u32) -> Chunk {
    let noises = Noises::new(seed);
    let chunk_origin = chunk_pos * CHUNK_SIZE;

    // The climate of every column the heights of the chunk read, computed once: each height
    // blends the biomes of nine columns
    let reach = STRUCTURE_MARGIN + BIOME_BLEND;
    let side = CHUNK_SIZE + 2 * reach;
    let corner = chunk_origin.xz() - IVec2::splat(reach);
    let climates: Vec<(f64, f64)> = (0..side * side)
        .map(|i| noises.climate(corner.x + i / side, corner.y + i % side))
        .collect();
    let climate = |x: i32, z: i32| climates[((x - corner.x) * side + z - corner.y) as usize];

    let mut chunk = Chunk::new();

    let chunk_range = 0..CHUNK_SIZE;
    for dx in -STRUCTURE_MARGIN..CHUNK_SIZE + STRUCTURE_MARGIN {
        for dz in -STRUCTURE_MARGIN..CHUNK_SIZE + STRUCTURE_MARGIN {
            let column =
                terrain_column_in(&noises, chunk_origin.x + dx, chunk_origin.z + dz, &climate);

            if chunk_range.contains(&dx) && chunk_range.contains(&dz) {
                let biome = get_biome_data(column.biome_type);
                for y in chunk_origin.y..chunk_origin.y + CHUNK_SIZE {
                    if let Some(block) = terrain_block(y, column.height, &biome) {
                        place(
                            &mut chunk,
                            chunk_origin,
                            IVec3::new(column.x, y, column.z),
                            block,
                        );
                    }
                }
            }

            // decorations only grow upwards from the surface
            if column.height + 1 < chunk_origin.y + CHUNK_SIZE {
                for (pos, block) in column_blocks(seed, &column) {
                    place(&mut chunk, chunk_origin, pos, block);
                }
            }
        }
    }
    chunk
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::ops::Range;

    /// Height, biome and surface block of a world column, read back from the generated chunks.
    fn probe(seed: u32, x: i32, z: i32) -> (i32, BiomeType, BlockId) {
        let biome = BiomeNoises::new(seed).biome_at(x, z);
        let surface = get_biome_data(biome).surface_block;
        let local = IVec3::new(x.rem_euclid(CHUNK_SIZE), 0, z.rem_euclid(CHUNK_SIZE));
        let mut height = -1;
        for cy in 0..8 {
            let chunk_pos = IVec3::new(x.div_euclid(CHUNK_SIZE), cy, z.div_euclid(CHUNK_SIZE));
            let chunk = generate_chunk(chunk_pos, seed);
            for dy in 0..CHUNK_SIZE {
                if chunk.get(local.with_y(dy)).map(|b| b.id) == Some(surface) {
                    height = cy * CHUNK_SIZE + dy;
                }
            }
        }
        (height, biome, surface)
    }

    #[test]
    fn terrain_is_pinned_for_a_fixed_seed() {
        // Values recorded from the generator before decorations became per-column.
        let expected = [
            ((0, 0), (64, BiomeType::FlowerPlains, BlockId::Grass)),
            ((37, -91), (56, BiomeType::Ocean, BlockId::Sand)),
            ((-120, 55), (64, BiomeType::Forest, BlockId::Grass)),
            ((-5, -300), (63, BiomeType::FlowerPlains, BlockId::Grass)),
            ((15, 16), (65, BiomeType::Plains, BlockId::Grass)),
            ((-1500, -1463), (64, BiomeType::IcePlain, BlockId::Snow)),
            ((-1500, -1278), (64, BiomeType::Forest, BlockId::Grass)),
            (
                (-1500, -1056),
                (74, BiomeType::HighMountainGrass, BlockId::Grass),
            ),
            (
                (-1500, -760),
                (70, BiomeType::MediumMountain, BlockId::Grass),
            ),
            ((-1500, -723), (63, BiomeType::Desert, BlockId::Sand)),
            ((-1463, -1352), (51, BiomeType::DeepOcean, BlockId::Sand)),
        ];
        for ((x, z), want) in expected {
            assert_eq!(probe(42, x, z), want, "column ({x}, {z})");
        }
    }

    #[test]
    fn surface_bounds_hold_for_every_biome() {
        // Listing a biome here is a compile error until it is added to `all`
        let all = |biome: BiomeType| match biome {
            BiomeType::Plains
            | BiomeType::Forest
            | BiomeType::MediumMountain
            | BiomeType::HighMountainGrass
            | BiomeType::Desert
            | BiomeType::IcePlain
            | BiomeType::FlowerPlains
            | BiomeType::ShallowOcean
            | BiomeType::Ocean
            | BiomeType::DeepOcean => [
                BiomeType::Plains,
                BiomeType::Forest,
                BiomeType::MediumMountain,
                BiomeType::HighMountainGrass,
                BiomeType::Desert,
                BiomeType::IcePlain,
                BiomeType::FlowerPlains,
                BiomeType::ShallowOcean,
                BiomeType::Ocean,
                BiomeType::DeepOcean,
            ],
        };
        let biomes = all(BiomeType::Plains).map(get_biome_data);
        let largest_variation = biomes.iter().map(|b| b.height_variation).max().unwrap() as i32;
        let lowest_base = biomes.iter().map(|b| b.base_height).min().unwrap() as i32;
        let highest_base = biomes.iter().map(|b| b.base_height).max().unwrap() as i32;
        assert!(LOWEST_SURFACE <= lowest_base - largest_variation);
        assert!(HIGHEST_SURFACE >= highest_base + largest_variation);
    }

    #[test]
    fn no_structure_grows_higher_than_its_bound() {
        let noises = Noises::new(42);
        let mut highest = 0;
        for x in -1600..-1400 {
            for z in -1400..-1200 {
                let column = terrain_column(&noises, x, z);
                for (pos, _) in column_blocks(42, &column) {
                    assert!(
                        pos.y - column.height <= STRUCTURE_HEIGHT,
                        "{pos} over {}",
                        column.height
                    );
                    highest = highest.max(pos.y - column.height);
                }
            }
        }
        assert_eq!(
            highest, STRUCTURE_HEIGHT,
            "the forest has a big tree as tall as can be"
        );
    }

    #[test]
    fn chunks_without_noise_are_the_ones_the_noise_makes() {
        for (cx, cz) in [(0, 0), (-1, -1), (-94, -80), (-94, -66), (-92, -85)] {
            for cy in -2..=8 {
                let chunk_pos = IVec3::new(cx, cy, cz);
                assert!(
                    generate_chunk(chunk_pos, 42) == generate_chunk_from_noise(chunk_pos, 42),
                    "chunk {chunk_pos}"
                );
            }
        }
    }

    /// Chunks around the surface (heights 50 to 80), with positive and negative coordinates.
    fn sample_chunks() -> Vec<IVec3> {
        let mut chunks = Vec::new();
        for (cx, cz) in [(0, 0), (-1, -1), (2, -3), (-5, 4), (7, 7)] {
            for cy in 2..=7 {
                chunks.push(IVec3::new(cx, cy, cz));
            }
        }
        chunks
    }

    #[test]
    fn nothing_is_written_outside_the_chunk() {
        // `Chunk` trips a debug assertion on an out-of-range write: generating must not.
        for chunk_pos in sample_chunks() {
            generate_chunk(chunk_pos, 42);
        }
        let origin = IVec3::new(32, 64, -16);
        let mut chunk = Chunk::new();
        for outside in [
            origin - IVec3::X,
            origin - IVec3::Y,
            origin - IVec3::Z,
            origin + IVec3::splat(CHUNK_SIZE),
            origin + IVec3::new(CHUNK_SIZE, 0, 0),
            origin + IVec3::new(0, 0, CHUNK_SIZE),
        ] {
            place(&mut chunk, origin, outside, BlockId::Stone);
        }
        assert!(chunk.is_empty());
    }

    fn blocks(chunk: &Chunk) -> Vec<(IVec3, BlockId)> {
        chunk.iter().map(|(p, b)| (p, b.id)).collect()
    }

    #[test]
    fn same_inputs_give_the_same_chunk() {
        for chunk_pos in sample_chunks() {
            let first = generate_chunk(chunk_pos, 42);
            let second = generate_chunk(chunk_pos, 42);
            assert_eq!(blocks(&first), blocks(&second), "chunk {chunk_pos}");
        }
    }

    #[test]
    fn another_seed_gives_another_chunk() {
        let chunk_pos = IVec3::new(0, 4, 0);
        let first = generate_chunk(chunk_pos, 42);
        let second = generate_chunk(chunk_pos, 43);
        assert_ne!(blocks(&first), blocks(&second));
    }

    #[test]
    fn the_largest_seed_does_not_panic() {
        for chunk_pos in sample_chunks() {
            generate_chunk(chunk_pos, u32::MAX);
        }
    }

    /// Finds, in a window of columns, the first one carrying a tree whose blocks satisfy `wanted`.
    fn find_tree(
        seed: u32,
        xs: Range<i32>,
        zs: Range<i32>,
        wanted: impl Fn(&Blocks) -> bool,
    ) -> Column {
        let noises = Noises::new(seed);
        let xs_window = xs.clone();
        for x in xs {
            for z in zs.clone() {
                let column = terrain_column(&noises, x, z);
                let has_tree = column_decorations(&column, &random_of(seed, &column))
                    .iter()
                    .any(|d| matches!(d, Decoration::Tree | Decoration::BigTree));
                if has_tree && wanted(&column_blocks(seed, &column)) {
                    return column;
                }
            }
        }
        panic!("no matching tree in x {xs_window:?}, z {zs:?}: widen the window");
    }

    fn touched_chunks(blocks: &Blocks) -> Vec<IVec3> {
        let mut chunks: Vec<IVec3> = blocks
            .iter()
            .map(|(pos, _)| pos.div_euclid(IVec3::splat(CHUNK_SIZE)))
            .collect();
        chunks.sort_by_key(|c| (c.x, c.y, c.z));
        chunks.dedup();
        chunks
    }

    /// Generates every chunk the tree rooted on `column` touches, and
    /// checks that the union holds all of its blocks.
    fn assert_tree_is_complete(seed: u32, column: &Column) {
        let blocks = column_blocks(seed, column);
        let touched = touched_chunks(&blocks);
        assert!(touched.len() >= 2, "the tree must straddle a border");
        let chunks: HashMap<IVec3, Chunk> = touched
            .iter()
            .map(|&c| (c, generate_chunk(c, seed)))
            .collect();

        let mut exact_per_chunk: HashMap<IVec3, usize> = HashMap::new();
        for (pos, block) in &blocks {
            let chunk_pos = pos.div_euclid(IVec3::splat(CHUNK_SIZE));
            let local = pos.rem_euclid(IVec3::splat(CHUNK_SIZE));
            let actual = chunks[&chunk_pos].get(local).map(|b| b.id);
            // a block of another structure or of the terrain may outrank a leaf, never the reverse
            let actual = actual.unwrap_or_else(|| panic!("{pos} ({block:?}) is missing"));
            assert!(
                precedence(actual) >= precedence(*block),
                "{pos}: {actual:?} instead of {block:?}"
            );
            if actual == *block {
                *exact_per_chunk.entry(chunk_pos).or_default() += 1;
            }
            if *block == BlockId::OakLog && pos.x == column.x && pos.z == column.z {
                assert_eq!(actual, BlockId::OakLog, "trunk block at {pos}");
            }
        }
        for chunk_pos in touched {
            assert!(
                exact_per_chunk.get(&chunk_pos).copied().unwrap_or(0) > 0,
                "nothing of the tree in {chunk_pos}"
            );
        }
    }

    fn spans_two_chunks(blocks: &Blocks, axis: usize) -> bool {
        let coords: Vec<i32> = blocks
            .iter()
            .map(|(p, _)| p[axis].div_euclid(CHUNK_SIZE))
            .collect();
        coords.iter().min() != coords.iter().max()
    }

    #[test]
    fn a_tree_across_a_horizontal_border_is_complete() {
        for axis in [0, 2] {
            let column = find_tree(42, -100..100, -100..100, |b| spans_two_chunks(b, axis));
            assert_tree_is_complete(42, &column);
        }
    }

    #[test]
    fn a_tree_across_a_vertical_border_is_complete() {
        // Such trees are rare (only high ground puts a root near a chunk plane): search wide.
        let column = find_tree(42, -300..300, -300..300, |b| spans_two_chunks(b, 1));
        assert_tree_is_complete(42, &column);
    }

    #[test]
    fn a_tree_rooted_on_the_top_layer_of_a_chunk_belongs_to_the_chunk_above() {
        let seed = 42;
        let noises = Noises::new(seed);
        let mut found = None;
        'scan: for x in -100..100 {
            for z in -100..100 {
                let column = terrain_column(&noises, x, z);
                let has_tree = column_decorations(&column, &random_of(seed, &column))
                    .iter()
                    .any(|d| matches!(d, Decoration::Tree | Decoration::BigTree));
                if has_tree && column.height.rem_euclid(CHUNK_SIZE) == CHUNK_SIZE - 1 {
                    found = Some(column);
                    break 'scan;
                }
            }
        }
        let column = found.expect("a tree on a surface at the top of a chunk");
        let root = IVec3::new(column.x, column.height + 1, column.z);
        let chunk_of_root = root.div_euclid(IVec3::splat(CHUNK_SIZE));
        let local = root.rem_euclid(IVec3::splat(CHUNK_SIZE));
        assert_eq!(local.y, 0);

        let above = generate_chunk(chunk_of_root, seed);
        assert_eq!(above.get(local).map(|b| b.id), Some(BlockId::OakLog));
        let below = generate_chunk(chunk_of_root - IVec3::Y, seed);
        assert!(below.iter().all(|(_, b)| b.id != BlockId::OakLog));
    }

    #[test]
    fn no_structure_reaches_further_than_the_margin() {
        let seed = 7;
        let mut widest = 0;
        for x in 0..150 {
            for z in 0..150 {
                let column = Column {
                    x,
                    z,
                    height: 70,
                    biome_type: BiomeType::Forest,
                };
                for (pos, _) in column_blocks(seed, &column) {
                    // the vertical culling in generate_chunk relies on this
                    assert!(pos.y > column.height, "block below its root at {pos}");
                    widest = widest.max((pos.x - x).abs()).max((pos.z - z).abs());
                }
            }
        }
        assert_eq!(widest, STRUCTURE_MARGIN);
    }

    #[test]
    fn decorations_never_replace_terrain() {
        for chunk_pos in sample_chunks() {
            let chunk = generate_chunk(chunk_pos, 42);
            let noises = Noises::new(42);
            for dx in 0..CHUNK_SIZE {
                for dz in 0..CHUNK_SIZE {
                    let column = terrain_column(
                        &noises,
                        chunk_pos.x * CHUNK_SIZE + dx,
                        chunk_pos.z * CHUNK_SIZE + dz,
                    );
                    let biome = get_biome_data(column.biome_type);
                    for dy in 0..CHUNK_SIZE {
                        let y = chunk_pos.y * CHUNK_SIZE + dy;
                        if y <= column.height || y <= SEA_LEVEL {
                            let actual = chunk.get(IVec3::new(dx, dy, dz)).map(|b| b.id);
                            assert_eq!(
                                actual,
                                terrain_block(y, column.height, &biome),
                                "chunk {chunk_pos} local ({dx}, {dy}, {dz})"
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn a_higher_rank_wins_whatever_the_order() {
        let ranked = [
            BlockId::Dandelion,
            BlockId::TallGrass,
            BlockId::OakLeaves,
            BlockId::OakLog,
            BlockId::Stone,
        ];
        let cell = IVec3::new(3, 4, 5);
        for (i, low) in ranked.iter().enumerate() {
            for high in &ranked[i + 1..] {
                for order in [[low, high], [high, low]] {
                    let mut chunk = Chunk::new();
                    for block in order {
                        place(&mut chunk, IVec3::ZERO, cell, *block);
                    }
                    assert_eq!(
                        chunk.get(cell).unwrap().id,
                        *high,
                        "{low:?} against {high:?}"
                    );
                }
            }
        }
    }

    /// Times the chunks a player asks for at spawn with the largest render distance a server
    /// grants by default. Ignored: run it in release, with
    /// `cargo test -p server --release generation_time -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn generation_time_of_a_render_distance() {
        let radius = shared::DEFAULT_MAX_RENDER_DISTANCE as i32;
        let spawn_chunk = IVec3::new(0, SEA_LEVEL / CHUNK_SIZE, 0);
        let start = std::time::Instant::now();
        let mut count = 0;
        for x in -radius..=radius {
            for y in -radius..=radius {
                for z in -radius..=radius {
                    let chunk_pos = spawn_chunk + IVec3::new(x, y, z);
                    std::hint::black_box(generate_chunk(chunk_pos, 42));
                    count += 1;
                }
            }
        }
        let elapsed = start.elapsed();
        println!(
            "{count} chunks in {elapsed:?}, {:?} per chunk",
            elapsed / count
        );
    }

    #[test]
    fn splitmix64_matches_the_reference_vector() {
        // first output of the reference splitmix64 generator seeded with 0
        assert_eq!(splitmix64(0), 0xE220_A839_7B1D_CDAF);
    }

    #[test]
    fn column_random_depends_on_every_input_and_nothing_else() {
        let base = ColumnRandom {
            seed: 1,
            x: 5,
            z: -7,
        };
        let same = ColumnRandom {
            seed: 1,
            x: 5,
            z: -7,
        };
        assert_eq!(base.bits(3), same.bits(3));
        assert_ne!(base.bits(3), base.bits(4));
        assert_ne!(
            base.bits(3),
            ColumnRandom {
                seed: 2,
                x: 5,
                z: -7
            }
            .bits(3)
        );
        assert_ne!(
            base.bits(3),
            ColumnRandom {
                seed: 1,
                x: 6,
                z: -7
            }
            .bits(3)
        );
        assert_ne!(
            base.bits(3),
            ColumnRandom {
                seed: 1,
                x: 5,
                z: -6
            }
            .bits(3)
        );
        assert_ne!(
            ColumnRandom {
                seed: 1,
                x: -1,
                z: 0
            }
            .bits(3),
            ColumnRandom {
                seed: 1,
                x: 1,
                z: 0
            }
            .bits(3)
        );
        // swapped coordinates and a diagonal step must not collide
        let swapped = ColumnRandom {
            seed: 1,
            x: -7,
            z: 5,
        };
        let diagonal = ColumnRandom {
            seed: 1,
            x: 6,
            z: -8,
        };
        assert_ne!(base.bits(3), swapped.bits(3));
        assert_ne!(base.bits(3), diagonal.bits(3));
    }

    #[test]
    fn column_random_is_roughly_uniform() {
        let mut below_tenth = 0;
        for x in -50..50 {
            for z in -50..50 {
                let random = ColumnRandom { seed: 9, x, z };
                let value = random.unit(SALT_TREE_ROLL);
                assert!((0.0..1.0).contains(&value));
                below_tenth += (value < 0.1) as i32;
                assert!((0..3).contains(&random.below(SALT_TRUNK_HEIGHT, 3)));
            }
        }
        assert!((900..1100).contains(&below_tenth), "{below_tenth} of 10000");
    }
}
