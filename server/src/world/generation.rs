use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use bevy::prelude::*;
use shared::{players::PLAYER_HEIGHT, world::*, CHUNK_SIZE};

use super::caves::{CaveMask, Caves, Noise3, CAVE_FLOOR};

mod ores;

/// Largest horizontal distance between the column a structure is rooted in and any of its
/// blocks. It is set by the big tree: a branch starts at most 1 block from the trunk and is
/// at most 2 blocks long, and its tip carries one more leaf (`x + 1 + 2`). Generating a chunk
/// therefore looks at the columns of the chunk plus this margin on each side.
const STRUCTURE_MARGIN: i32 = 3;

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
const SALT_ROCK_LINE: u64 = 8;
/// Each of the two branches of a big tree uses 4 consecutive salts from here.
const SALT_BRANCH: u64 = 0x100;
/// One salt per leaf of the 5x5x3 crown, from here.
const SALT_LEAF: u64 = 0x200;

/// The random decisions tied to a place of the world: a column `[x, z]`, or a chunk or a cell
/// `[x, y, z]`. Every value is a pure function of `(seed, coords, salt)`, so a place decides
/// the same thing whichever chunk asks.
struct PositionRandom<const N: usize> {
    seed: u32,
    coords: [i32; N],
}

/// The random decisions of one world column.
type ColumnRandom = PositionRandom<2>;

impl<const N: usize> PositionRandom<N> {
    /// Folds the seed, the coordinates and the salt into one 64-bit hash, one `splitmix64`
    /// round after each of them.
    fn bits(&self, salt: u64) -> u64 {
        let mut hash = splitmix64(self.seed as u64);
        for coord in self.coords {
            hash = splitmix64(hash ^ coord as u32 as u64);
        }
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

/// From this height the ground is bare stone, and from `SNOW_LINE` snow over stone: both lines
/// move up or down by up to `ROCK_LINE_JITTER` from one column to the next.
const STONE_LINE: i32 = 104;
const SNOW_LINE: i32 = 122;
const ROCK_LINE_JITTER: u64 = 3;
/// A mountain column higher than a neighbour by more than this is a rock face: bare stone,
/// even under the snow line.
const ROCK_FACE: i32 = 3;

/// What covers the ground under the water is chosen by a noise of patches this wide, which
/// leans from sand toward gravel then stone by `BED_DEPTH_BIAS` per block of water above it.
/// A bed sloping down by more than `BED_STEEP` blocks is bare stone.
const BED_PATCH_WAVELENGTH: f64 = 24.0;
const BED_DEPTH_BIAS: f64 = 0.03;
const BED_GRAVEL_BELOW: f64 = -0.1;
const BED_STONE_BELOW: f64 = -0.4;
const BED_STEEP: i32 = 2;
/// The salt of the bed patch noise, apart from those of the caves.
const SALT_BED_PATCHES: u64 = 0x200;

/// Terrain of one world column: the noise is evaluated once per column and reused.
struct Column {
    x: i32,
    z: i32,
    height: i32,
    biome_type: BiomeType,
    surface: BlockId,
    sub_surface: BlockId,
    /// The highest cell of the column caves may carve.
    cave_ceiling: i32,
}

impl Column {
    /// `drop` is how far the column stands above its lowest neighbour, and `bed_patch` the
    /// value there of the noise that picks what covers the ground under the water.
    fn new(seed: u32, x: i32, z: i32, shape: ColumnShape, drop: i32, bed_patch: f64) -> Self {
        let random = PositionRandom {
            seed,
            coords: [x, z],
        };
        let ground = Ground {
            height: shape.height,
            biome_type: shape.biome,
            drop,
            bed_patch,
        };
        let (surface, sub_surface) = ground_blocks(&ground, &random);
        let mut column = Column {
            x,
            z,
            height: shape.height,
            biome_type: shape.biome,
            surface,
            sub_surface,
            cave_ceiling: shape.height,
        };
        column.cave_ceiling = cave_ceiling(&column, drop, &random);
        column
    }
}

/// Thickness of the rock kept between a cave and the water above it or beside it.
const CAVE_ROOF: i32 = 4;

/// The highest cell of `column` caves may carve: they stay `CAVE_ROOF` under the ground of the
/// column and of its neighbours where either holds water, so that no water touches a cave,
/// and they leave the ground under anything growing. Elsewhere they may open at the surface.
fn cave_ceiling(column: &Column, drop: i32, random: &ColumnRandom) -> i32 {
    let lowest = column.height - drop.max(0);
    if lowest < SEA_LEVEL {
        lowest - CAVE_ROOF
    } else if !column_decorations(column, random).is_empty() {
        column.height - 1
    } else {
        column.height
    }
}

/// What the ground blocks of a column are chosen from.
struct Ground {
    height: i32,
    biome_type: BiomeType,
    /// How far the column stands above its lowest neighbour.
    drop: i32,
    bed_patch: f64,
}

/// The surface and sub-surface blocks of a column: those of its biome, except under the water,
/// where they are sand, gravel or stone (see [`bed_blocks`]), on the beach just above it, which
/// is sand (a frozen shore stays snowy), and high in the mountains, which are bare stone, then
/// snow over stone.
fn ground_blocks(ground: &Ground, random: &ColumnRandom) -> (BlockId, BlockId) {
    let Ground {
        height,
        biome_type,
        drop,
        ..
    } = *ground;
    let beach = height <= SEA_LEVEL + 1 && biome_type != BiomeType::IcePlain;
    let jitter = random.below(SALT_ROCK_LINE, 2 * ROCK_LINE_JITTER + 1) - ROCK_LINE_JITTER as i32;
    let rock_face = height >= MOUNTAIN_FROM && drop > ROCK_FACE;
    if height < SEA_LEVEL {
        bed_blocks(ground)
    } else if beach {
        (BlockId::Sand, BlockId::Sand)
    } else if height >= SNOW_LINE + jitter && !rock_face {
        (BlockId::Snow, BlockId::Stone)
    } else if height >= STONE_LINE + jitter || rock_face {
        (BlockId::Stone, BlockId::Stone)
    } else {
        let biome = get_biome_data(biome_type);
        (biome.surface_block, biome.sub_surface_block)
    }
}

/// The ground under the water: sand in the shallows, giving way to gravel and stone as the
/// water deepens, in patches, and bare stone where the bed is steep.
fn bed_blocks(ground: &Ground) -> (BlockId, BlockId) {
    let depth = SEA_LEVEL - ground.height;
    let score = ground.bed_patch - BED_DEPTH_BIAS * depth as f64;
    if ground.drop > BED_STEEP || score < BED_STONE_BELOW {
        (BlockId::Stone, BlockId::Stone)
    } else if score < BED_GRAVEL_BELOW {
        (BlockId::Gravel, BlockId::Gravel)
    } else {
        (BlockId::Sand, BlockId::Sand)
    }
}

/// The block at height `y` of a terrain column, or `None` above the surface and the sea.
fn terrain_block(y: i32, column: &Column) -> Option<BlockId> {
    let height = column.height;
    if y > height && y > SEA_LEVEL {
        None
    } else if y == 0 {
        Some(BlockId::Bedrock)
    } else if y < height - 4 {
        Some(BlockId::Stone)
    } else if y < height {
        Some(column.sub_surface)
    } else if y == height {
        Some(column.surface)
    } else if y == SEA_LEVEL && column.biome_type == BiomeType::IcePlain {
        Some(BlockId::Ice)
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
    PositionRandom {
        seed,
        coords: [column.x, column.z],
    }
}

/// Decides, from `(seed, x, z)` only, what is rooted on the column.
fn column_decorations(column: &Column, random: &ColumnRandom) -> Vec<Decoration> {
    let mut decorations = Vec::new();
    if column.height <= SEA_LEVEL {
        return decorations;
    }
    let biome_type = column.biome_type;
    if column.surface == BlockId::Sand {
        if biome_type == BiomeType::Desert && random.unit(SALT_CACTUS_ROLL) < 0.01 {
            decorations.push(Decoration::Cactus);
        }
        return decorations;
    }
    // nothing grows on stone, snow or ice
    if column.surface != BlockId::Grass {
        return decorations;
    }

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

    if biome_type != BiomeType::HighMountainGrass && random.unit(SALT_TALL_GRASS_ROLL) < 0.10 {
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

/// The columns a chunk generated from the noise reads: those of the chunk and of the margin
/// around it, the same for every chunk of a column of chunks.
struct ChunkColumns {
    /// Indexed by `(dz + STRUCTURE_MARGIN) * MARGIN_SIDE + dx + STRUCTURE_MARGIN`.
    columns: Vec<Column>,
    /// The highest surface of the columns of the chunk.
    top: i32,
    /// The highest surface of all of them.
    highest: i32,
}

/// Columns along a side of `ChunkColumns`.
const MARGIN_SIDE: i32 = CHUNK_SIZE + 2 * STRUCTURE_MARGIN;

impl ChunkColumns {
    fn at(&self, dx: i32, dz: i32) -> &Column {
        &self.columns[((dz + STRUCTURE_MARGIN) * MARGIN_SIDE + dx + STRUCTURE_MARGIN) as usize]
    }
}

/// Columns of chunks whose `ChunkColumns` a generator keeps, twice over: a dozen chunks of
/// each column of chunks are generated from the noise, from tasks that each take a few, and
/// the columns of a large render distance are too many to keep them all.
const KEPT_CHUNK_COLUMNS: usize = 1024;

/// The `ChunkColumns` used last: those of `recent`, and of `older` before it filled up.
#[derive(Default)]
struct ColumnsCache {
    recent: HashMap<IVec2, Arc<ChunkColumns>>,
    older: HashMap<IVec2, Arc<ChunkColumns>>,
}

/// Generates the chunks of one world. Its noises are built once, from the seed.
pub struct WorldGenerator {
    seed: u32,
    terrain: TerrainNoises,
    caves: Caves,
    bed_patches: Noise3,
    columns: Mutex<ColumnsCache>,
}

impl WorldGenerator {
    pub fn new(seed: u32) -> Self {
        Self {
            seed,
            terrain: TerrainNoises::new(seed),
            caves: Caves::new(seed),
            bed_patches: Noise3::new(seed, SALT_BED_PATCHES, BED_PATCH_WAVELENGTH, 1.0),
            columns: Mutex::default(),
        }
    }

    pub fn seed(&self) -> u32 {
        self.seed
    }

    /// Generates a chunk. It is a pure function of the seed and the position: terrain comes
    /// from the noise, and decorations are decided per world column, so a structure rooted in
    /// a neighbouring column is cut at the chunk border the same way whichever chunk is
    /// generated first.
    pub fn generate_chunk(&self, chunk_pos: IVec3) -> Chunk {
        let chunk_origin = chunk_pos * CHUNK_SIZE;
        // Most chunks lie above everything or deep underground: they need no column noise
        if chunk_origin.y > (HIGHEST_SURFACE + STRUCTURE_HEIGHT).max(SEA_LEVEL) {
            return Chunk::new();
        }
        // Below the sub-surface blocks of the lowest surface, which is also as low as the
        // caves are kept from the water, the ground is stone that only caves and ores change
        if chunk_origin.y + CHUNK_SIZE - 1 < LOWEST_SURFACE - CAVE_ROOF.max(4) {
            return self.underground_chunk(chunk_pos);
        }
        let columns = self.chunk_columns(chunk_pos.xz());
        // Above the ground, the sea and anything growing on them around
        if chunk_origin.y > (columns.highest + STRUCTURE_HEIGHT).max(SEA_LEVEL) {
            return Chunk::new();
        }
        self.generate_chunk_from_columns(chunk_pos, &columns)
    }

    /// The `ChunkColumns` of the column of chunks `chunk_column`, from the cache or computed.
    fn chunk_columns(&self, chunk_column: IVec2) -> Arc<ChunkColumns> {
        {
            let mut cache = self.columns.lock().unwrap();
            if let Some(columns) = cache.recent.get(&chunk_column) {
                return columns.clone();
            }
            if let Some(columns) = cache.older.remove(&chunk_column) {
                cache.recent.insert(chunk_column, columns.clone());
                return columns;
            }
        }
        // computed out of the lock: two tasks may compute the same, which is rare and harmless
        let columns = Arc::new(self.compute_chunk_columns(chunk_column));
        let mut cache = self.columns.lock().unwrap();
        if cache.recent.len() >= KEPT_CHUNK_COLUMNS {
            cache.older = std::mem::take(&mut cache.recent);
        }
        cache.recent.insert(chunk_column, columns.clone());
        columns
    }

    fn compute_chunk_columns(&self, chunk_column: IVec2) -> ChunkColumns {
        let origin = chunk_column * CHUNK_SIZE;
        // The shapes of the columns, and of one more ring that tells the steepness of the
        // outermost ones
        let ring = STRUCTURE_MARGIN + 1;
        let side = CHUNK_SIZE + 2 * ring;
        let shapes: Vec<ColumnShape> = (0..side * side)
            .map(|i| {
                let (dx, dz) = (i % side - ring, i / side - ring);
                self.terrain.column_shape(origin.x + dx, origin.y + dz)
            })
            .collect();
        let shape_at = |dx: i32, dz: i32| shapes[((dz + ring) * side + dx + ring) as usize];

        let mut columns = Vec::with_capacity((MARGIN_SIDE * MARGIN_SIDE) as usize);
        for dz in -STRUCTURE_MARGIN..CHUNK_SIZE + STRUCTURE_MARGIN {
            for dx in -STRUCTURE_MARGIN..CHUNK_SIZE + STRUCTURE_MARGIN {
                let shape = shape_at(dx, dz);
                let lowest = [(1, 0), (-1, 0), (0, 1), (0, -1)]
                    .iter()
                    .map(|(ox, oz)| shape_at(dx + ox, dz + oz).height)
                    .min()
                    .unwrap();
                let (x, z) = (origin.x + dx, origin.y + dz);
                columns.push(Column::new(
                    self.seed,
                    x,
                    z,
                    shape,
                    shape.height - lowest,
                    self.bed_patch(x, z),
                ));
            }
        }
        let top = (0..CHUNK_SIZE)
            .flat_map(|dx| (0..CHUNK_SIZE).map(move |dz| (dx, dz)))
            .map(|(dx, dz)| shape_at(dx, dz).height)
            .max()
            .unwrap();
        let highest = columns.iter().map(|column| column.height).max().unwrap();
        ChunkColumns {
            columns,
            top,
            highest,
        }
    }

    /// A chunk below the lowest surface less the depth of the sub-surface blocks: stone, the
    /// bedrock at height 0, the caves and the ores.
    fn underground_chunk(&self, chunk_pos: IVec3) -> Chunk {
        let chunk_origin = chunk_pos * CHUNK_SIZE;
        let caves = self.caves.carve(chunk_origin);
        // held as a single cell while nothing else is in it
        let mut chunk = Chunk::filled(BlockData::new(BlockId::Stone, BlockDirection::Front));
        for x in 0..CHUNK_SIZE {
            for y in 0..CHUNK_SIZE {
                for z in 0..CHUNK_SIZE {
                    let local = IVec3::new(x, y, z);
                    if caves.contains(local) {
                        chunk.remove(local);
                    } else if chunk_origin.y + y == 0 {
                        chunk.set(
                            local,
                            BlockData::new(BlockId::Bedrock, BlockDirection::Front),
                        );
                    }
                }
            }
        }
        ores::place_ores(self.seed, chunk_pos, &mut chunk);
        chunk
    }

    /// `generate_chunk` for any chunk, from the noise of its columns.
    #[cfg(test)]
    fn generate_chunk_from_noise(&self, chunk_pos: IVec3) -> Chunk {
        self.generate_chunk_from_columns(chunk_pos, &self.compute_chunk_columns(chunk_pos.xz()))
    }

    /// `generate_chunk_from_noise`, from the columns the noise gives.
    fn generate_chunk_from_columns(&self, chunk_pos: IVec3, columns: &ChunkColumns) -> Chunk {
        let seed = self.seed;
        let chunk_origin = chunk_pos * CHUNK_SIZE;

        let mut chunk = Chunk::new();

        // only a chunk that holds ground above the floor of the caves may be carved
        let caves: Option<CaveMask> = (chunk_origin.y <= columns.top
            && chunk_origin.y + CHUNK_SIZE > CAVE_FLOOR)
            .then(|| self.caves.carve(chunk_origin));

        let chunk_range = 0..CHUNK_SIZE;
        for dx in -STRUCTURE_MARGIN..CHUNK_SIZE + STRUCTURE_MARGIN {
            for dz in -STRUCTURE_MARGIN..CHUNK_SIZE + STRUCTURE_MARGIN {
                let column = columns.at(dx, dz);

                if chunk_range.contains(&dx) && chunk_range.contains(&dz) {
                    for y in chunk_origin.y..chunk_origin.y + CHUNK_SIZE {
                        let local = IVec3::new(dx, y - chunk_origin.y, dz);
                        let carved = y <= column.cave_ceiling
                            && caves.as_ref().is_some_and(|caves| caves.contains(local));
                        if carved {
                            continue;
                        }
                        if let Some(block) = terrain_block(y, column) {
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
                    for (pos, block) in column_blocks(seed, column) {
                        place(&mut chunk, chunk_origin, pos, block);
                    }
                }
            }
        }
        ores::place_ores(seed, chunk_pos, &mut chunk);
        chunk
    }
}

/// How far from the origin, in blocks, a new player's spawn is looked for.
const SPAWN_SEARCH_RADIUS: i32 = 512;

/// Room left under the feet of a player who spawns, so that its box is clear of the block.
const SPAWN_GAP: f32 = 0.01;

/// Where the players of a world of `seed` spawn and respawn (see
/// [`WorldGenerator::spawn_position`]). It depends on the seed alone, so that it stays where it
/// was however the world is built over.
pub fn spawn_position(seed: u32) -> Vec3 {
    WorldGenerator::new(seed).spawn_position()
}

/// The position of a player standing on the surface of the column `(x, z)` at `height`: the
/// middle of its body.
fn standing_on(x: i32, height: i32, z: i32) -> Vec3 {
    Vec3::new(
        x as f32 + 0.5,
        height as f32 + 1.0 + PLAYER_HEIGHT / 2.0 + SPAWN_GAP,
        z as f32 + 0.5,
    )
}

impl WorldGenerator {
    /// Where a new player appears: above the column nearest to the origin that is dry, below
    /// the mountains, and free of anything growing on it or on the columns around it.
    pub fn spawn_position(&self) -> Vec3 {
        let open = |x: i32, z: i32| {
            let shape = self.terrain.column_shape(x, z);
            if shape.height <= SEA_LEVEL + 1 || shape.height >= MOUNTAIN_FROM {
                return None;
            }
            for (dx, dz) in (-1..=1).flat_map(|dx| (-1..=1).map(move |dz| (dx, dz))) {
                let column = self.column(x + dx, z + dz);
                let decorations = column_decorations(&column, &random_of(self.seed, &column));
                if !decorations.is_empty() || column.height != shape.height {
                    return None;
                }
            }
            Some(shape.height)
        };
        // square rings around the origin, the nearest first
        for radius in 0..=SPAWN_SEARCH_RADIUS {
            for i in -radius..=radius {
                let ring = [(i, -radius), (i, radius), (-radius, i), (radius, i)];
                for (x, z) in ring {
                    if let Some(height) = open(x, z) {
                        return standing_on(x, height, z);
                    }
                }
            }
        }
        let height = self.terrain.column_shape(0, 0).height.max(SEA_LEVEL);
        standing_on(0, height, 0)
    }

    /// The column `(x, z)`, its steepness read from the shapes of its four neighbours.
    fn column(&self, x: i32, z: i32) -> Column {
        let shape = self.terrain.column_shape(x, z);
        let lowest = [(1, 0), (-1, 0), (0, 1), (0, -1)]
            .iter()
            .map(|(dx, dz)| self.terrain.column_shape(x + dx, z + dz).height)
            .min()
            .unwrap();
        Column::new(
            self.seed,
            x,
            z,
            shape,
            shape.height - lowest,
            self.bed_patch(x, z),
        )
    }

    fn bed_patch(&self, x: i32, z: i32) -> f64 {
        self.bed_patches.get(IVec3::new(x, 0, z))
    }
}

/// Generates one chunk of the world of `seed`, building its noises for it alone: a caller that
/// generates many chunks keeps a [`WorldGenerator`] instead.
pub fn generate_chunk(chunk_pos: IVec3, seed: u32) -> Chunk {
    WorldGenerator::new(seed).generate_chunk(chunk_pos)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::ops::Range;

    /// Whether `block` is part of the ground, not water nor something growing on it.
    fn is_ground(block: BlockId) -> bool {
        !matches!(block, BlockId::Water) && precedence(block) == precedence(BlockId::Stone)
    }

    /// Height, biome and surface block of a world column, read back from the generated chunks.
    fn probe(seed: u32, x: i32, z: i32) -> (i32, BiomeType, BlockId) {
        let biome = TerrainNoises::new(seed).column_shape(x, z).biome;
        let local = IVec3::new(x.rem_euclid(CHUNK_SIZE), 0, z.rem_euclid(CHUNK_SIZE));
        let mut surface = (-1, BlockId::Debug);
        for cy in 0..12 {
            let chunk_pos = IVec3::new(x.div_euclid(CHUNK_SIZE), cy, z.div_euclid(CHUNK_SIZE));
            let chunk = generate_chunk(chunk_pos, seed);
            for dy in 0..CHUNK_SIZE {
                match chunk.get(local.with_y(dy)).map(|b| b.id) {
                    Some(block) if is_ground(block) => surface = (cy * CHUNK_SIZE + dy, block),
                    _ => {}
                }
            }
        }
        (surface.0, biome, surface.1)
    }

    #[test]
    fn terrain_is_pinned_for_a_fixed_seed() {
        // One column of each biome, recorded when the mountains were added; the sea columns
        // again when the beds got deeper, rougher, and covered with gravel and stone.
        let expected = [
            ((408, 316), (67, BiomeType::Plains, BlockId::Grass)),
            ((478, 622), (66, BiomeType::Forest, BlockId::Grass)),
            ((185, -880), (95, BiomeType::MediumMountain, BlockId::Grass)),
            (
                (-570, 710),
                (122, BiomeType::HighMountainGrass, BlockId::Stone),
            ),
            (
                (466, -408),
                (164, BiomeType::HighMountainGrass, BlockId::Snow),
            ),
            ((-878, -454), (70, BiomeType::Desert, BlockId::Sand)),
            ((828, 137), (79, BiomeType::IcePlain, BlockId::Snow)),
            ((-126, 239), (67, BiomeType::FlowerPlains, BlockId::Grass)),
            ((-38, -183), (55, BiomeType::ShallowOcean, BlockId::Stone)),
            ((890, 975), (52, BiomeType::Ocean, BlockId::Gravel)),
            ((-938, -559), (38, BiomeType::DeepOcean, BlockId::Stone)),
        ];
        for ((x, z), want) in expected {
            assert_eq!(probe(42, x, z), want, "column ({x}, {z})");
        }
    }

    fn ground_of(
        height: i32,
        biome_type: BiomeType,
        drop: i32,
        bed_patch: f64,
    ) -> (BlockId, BlockId) {
        let ground = Ground {
            height,
            biome_type,
            drop,
            bed_patch,
        };
        ground_blocks(&ground, &random(1, [0, 0]))
    }

    #[test]
    fn the_shores_are_sand_except_frozen_ones() {
        let ground = |height, biome| ground_of(height, biome, 0, 0.0);
        let sand = (BlockId::Sand, BlockId::Sand);
        let plains = BiomeType::Plains;
        let ice = BiomeType::IcePlain;
        assert_eq!(
            ground(SEA_LEVEL + 2, plains),
            (BlockId::Grass, BlockId::Dirt)
        );
        assert_eq!(ground(SEA_LEVEL + 1, plains), sand);
        assert_eq!(ground(SEA_LEVEL, plains), sand);
        assert_eq!(ground(SEA_LEVEL + 1, ice), (BlockId::Snow, BlockId::Ice));
        assert_eq!(ground(SEA_LEVEL - 1, ice), sand);
    }

    #[test]
    fn the_bed_turns_from_sand_to_gravel_and_stone_as_the_water_deepens_or_steepens() {
        let bed =
            |depth, drop, patch| ground_of(SEA_LEVEL - depth, BiomeType::Ocean, drop, patch).0;
        assert_eq!(bed(1, 0, 0.0), BlockId::Sand);
        assert_eq!(bed(1, 0, -0.3), BlockId::Gravel);
        assert_eq!(bed(1, 0, -0.6), BlockId::Stone);
        // the same patch, under more and more water
        assert_eq!(bed(2, 0, 0.1), BlockId::Sand);
        assert_eq!(bed(10, 0, 0.1), BlockId::Gravel);
        assert_eq!(bed(20, 0, 0.1), BlockId::Stone);
        // a steep bed is bare whatever the patch
        assert_eq!(bed(2, BED_STEEP + 1, 0.5), BlockId::Stone);
        assert_eq!(bed(2, BED_STEEP, 0.5), BlockId::Sand);
    }

    #[test]
    fn high_ground_is_bare_stone_then_snow_and_rock_faces_stay_bare() {
        let stone = (BlockId::Stone, BlockId::Stone);
        let snow = (BlockId::Snow, BlockId::Stone);
        let high = BiomeType::HighMountainGrass;
        for x in 0..50 {
            let random = random(1, [x, 0]);
            let ground = |height, drop| {
                let ground = Ground {
                    height,
                    biome_type: high,
                    drop,
                    bed_patch: 0.0,
                };
                ground_blocks(&ground, &random)
            };
            let jitter = ROCK_LINE_JITTER as i32;
            assert_eq!(ground(STONE_LINE - jitter - 1, 0).0, BlockId::Grass);
            assert_eq!(ground(STONE_LINE + jitter, 0), stone);
            assert_eq!(ground(SNOW_LINE + jitter, 0), snow);
            assert_eq!(ground(SNOW_LINE + jitter, ROCK_FACE + 1), stone);
            assert_eq!(ground(MOUNTAIN_FROM, ROCK_FACE + 1), stone);
            assert_eq!(
                ground(MOUNTAIN_FROM, ROCK_FACE),
                (BlockId::Grass, BlockId::Dirt)
            );
        }
    }

    #[test]
    fn no_structure_grows_higher_than_its_bound() {
        let seed = 42;
        let generator = WorldGenerator::new(seed);
        let mut highest = 0;
        for x in -300..300 {
            for z in -300..300 {
                let column = generator.column(x, z);
                for (pos, _) in column_blocks(seed, &column) {
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
            "the forests have a big tree as tall as can be"
        );
    }

    #[test]
    fn chunks_without_noise_are_the_ones_the_noise_makes() {
        let generator = WorldGenerator::new(42);
        // plains, a deep sea, a mountain range and its peaks, a lake
        for (cx, cz) in [(0, 0), (-59, -35), (-40, 40), (29, -26), (-16, 7)] {
            for cy in -2..=13 {
                let chunk_pos = IVec3::new(cx, cy, cz);
                assert!(
                    generator.generate_chunk(chunk_pos)
                        == generator.generate_chunk_from_noise(chunk_pos),
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
        // a single chunk can be solid stone under a mountain for both seeds
        let column = |seed| -> Vec<_> {
            (2..10)
                .flat_map(|cy| blocks(&generate_chunk(IVec3::new(0, cy, 0), seed)))
                .collect()
        };
        assert_ne!(column(42), column(43));
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
        let generator = WorldGenerator::new(seed);
        let xs_window = xs.clone();
        for x in xs {
            for z in zs.clone() {
                let column = generator.column(x, z);
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
    fn a_new_player_spawns_on_open_dry_ground_near_the_origin() {
        let generator = WorldGenerator::new(42);
        let spawn = generator.spawn_position();
        let (x, z) = (spawn.x.floor() as i32, spawn.z.floor() as i32);
        let column = generator.column(x, z);
        assert!(column.height > SEA_LEVEL && column.height < MOUNTAIN_FROM);
        assert_eq!(spawn, standing_on(x, column.height, z));
        assert!(x.abs().max(z.abs()) <= SPAWN_SEARCH_RADIUS);
        // nothing stands where the player appears, nor right above
        let feet = IVec3::new(x, column.height + 1, z);
        for pos in [feet, feet + IVec3::Y] {
            let chunk = generate_chunk(pos.div_euclid(IVec3::splat(CHUNK_SIZE)), 42);
            assert_eq!(
                chunk.get(pos.rem_euclid(IVec3::splat(CHUNK_SIZE))),
                None,
                "{pos}"
            );
        }
        assert_eq!(generator.spawn_position(), spawn);
    }

    #[test]
    fn rivers_and_lakes_hold_water_inland() {
        let generator = WorldGenerator::new(42);
        let (mut inland, mut wet) = (0, 0);
        for x in (-1024..1024).step_by(4) {
            for z in (-1024..1024).step_by(4) {
                let shape = generator.terrain.column_shape(x, z);
                let ocean = matches!(
                    shape.biome,
                    BiomeType::ShallowOcean | BiomeType::Ocean | BiomeType::DeepOcean
                );
                if !ocean {
                    inland += 1;
                    if shape.height < SEA_LEVEL {
                        wet += 1;
                    }
                }
            }
        }
        // about one land column in ten
        let share = wet as f64 / inland as f64;
        assert!((0.04..0.2).contains(&share), "{wet} of {inland}");
    }

    #[test]
    fn water_stays_at_the_sea_level_and_freezes_in_the_ice_plains() {
        // around the origin of the seed 42: an ice plain, its frozen lakes, and liquid ones
        let generator = WorldGenerator::new(42);
        let (mut water, mut ice) = (0, 0);
        for cx in -4..8 {
            for cz in -4..8 {
                for cy in 3..5 {
                    let chunk_pos = IVec3::new(cx, cy, cz);
                    let chunk = generator.generate_chunk(chunk_pos);
                    for (local, block) in chunk.iter() {
                        let pos = chunk_pos * CHUNK_SIZE + local;
                        match block.id {
                            BlockId::Water => {
                                water += 1;
                                assert!(pos.y <= SEA_LEVEL, "water at {pos}");
                                // nothing for it to flow into: no cave beside it or under it
                                for side in
                                    [IVec3::X, IVec3::NEG_X, IVec3::Z, IVec3::NEG_Z, IVec3::NEG_Y]
                                {
                                    let next = local + side;
                                    if Chunk::contains(next) {
                                        assert!(chunk.get(next).is_some(), "air at {}", pos + side);
                                    }
                                }
                            }
                            // the ground of the ice plains is snow over ice
                            BlockId::Ice if pos.y > generator.column(pos.x, pos.z).height => {
                                ice += 1;
                                assert_eq!(pos.y, SEA_LEVEL, "ice at {pos}");
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
        assert!(water > 0 && ice > 0, "{water} water, {ice} ice");
    }

    #[test]
    fn caves_open_underground_and_at_the_surface_but_leave_the_bottom_whole() {
        let generator = WorldGenerator::new(42);
        let (mut underground, mut openings) = (0, 0);
        for x in (-256..256).step_by(32) {
            for z in (-256..256).step_by(32) {
                let column = generator.column(x, z);
                for cy in 0..=column.height / CHUNK_SIZE {
                    let chunk_pos =
                        IVec3::new(x, cy * CHUNK_SIZE, z).div_euclid(IVec3::splat(CHUNK_SIZE));
                    let chunk = generator.generate_chunk(chunk_pos);
                    let (lx, lz) = (x.rem_euclid(CHUNK_SIZE), z.rem_euclid(CHUNK_SIZE));
                    for ly in 0..CHUNK_SIZE {
                        let y = cy * CHUNK_SIZE + ly;
                        if y > column.height || chunk.get(IVec3::new(lx, ly, lz)).is_some() {
                            continue;
                        }
                        assert!(y >= CAVE_FLOOR, "a cave at ({x}, {y}, {z})");
                        underground += 1;
                        if y == column.height {
                            openings += 1;
                        }
                    }
                }
            }
        }
        assert!(
            underground > 200 && openings > 0,
            "{underground} cells, {openings} openings"
        );
    }

    #[test]
    fn plants_and_trunks_stand_on_something() {
        let generator = WorldGenerator::new(42);
        for chunk_pos in sample_chunks() {
            let chunk = generator.generate_chunk(chunk_pos);
            for (local, block) in chunk.iter() {
                let rooted = matches!(
                    block.id,
                    BlockId::Dandelion
                        | BlockId::Poppy
                        | BlockId::TallGrass
                        | BlockId::OakLog
                        | BlockId::Cactus
                );
                let below = local - IVec3::Y;
                if rooted && Chunk::contains(below) {
                    assert!(
                        chunk.get(below).is_some(),
                        "{:?} over air at {local} in {chunk_pos}",
                        block.id
                    );
                }
            }
        }
    }

    #[test]
    fn nothing_grows_above_the_stone_line() {
        // a window over a mountain chain of the seed 42
        let seed = 42;
        let generator = WorldGenerator::new(seed);
        let mut mountain_columns = 0;
        for x in (-960..-640).step_by(2) {
            for z in (512..732).step_by(2) {
                let column = generator.column(x, z);
                let decorations = column_decorations(&column, &random_of(seed, &column));
                if column.height >= MOUNTAIN_FROM {
                    mountain_columns += 1;
                }
                if column.height >= STONE_LINE + ROCK_LINE_JITTER as i32 {
                    assert_eq!(decorations, [], "({x}, {z}) at {}", column.height);
                }
            }
        }
        assert!(
            mountain_columns > 1000,
            "{mountain_columns} mountain columns"
        );
    }

    #[test]
    fn a_tree_rooted_on_the_top_layer_of_a_chunk_belongs_to_the_chunk_above() {
        let seed = 42;
        let generator = WorldGenerator::new(seed);
        let mut found = None;
        'scan: for x in (-1000..1000).step_by(7) {
            for z in (-1000..1000).step_by(7) {
                let column = generator.column(x, z);
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
        // the trunk's column, in the chunk below, is ground up to the top
        for y in 0..CHUNK_SIZE {
            let block = below.get(local.with_y(y)).map(|b| b.id);
            assert_ne!(block, Some(BlockId::OakLog), "at y {y}");
        }
        assert_eq!(
            below.get(local.with_y(CHUNK_SIZE - 1)).map(|b| b.id),
            Some(column.surface)
        );
    }

    #[test]
    fn no_structure_reaches_further_than_the_margin() {
        let seed = 7;
        let mut widest = 0;
        for x in 0..150 {
            for z in 0..150 {
                let shape = ColumnShape {
                    height: 70,
                    biome: BiomeType::Forest,
                };
                let column = Column::new(seed, x, z, shape, 0, 0.0);
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
        let generator = WorldGenerator::new(42);
        for chunk_pos in sample_chunks() {
            let chunk = generator.generate_chunk(chunk_pos);
            let caves = generator.caves.carve(chunk_pos * CHUNK_SIZE);
            for dx in 0..CHUNK_SIZE {
                for dz in 0..CHUNK_SIZE {
                    let column = generator
                        .column(chunk_pos.x * CHUNK_SIZE + dx, chunk_pos.z * CHUNK_SIZE + dz);
                    for dy in 0..CHUNK_SIZE {
                        let y = chunk_pos.y * CHUNK_SIZE + dy;
                        let local = IVec3::new(dx, dy, dz);
                        if y <= column.height || y <= SEA_LEVEL {
                            let carved = y <= column.cave_ceiling && caves.contains(local);
                            let expected = if carved {
                                None
                            } else {
                                terrain_block(y, &column)
                            };
                            // ores are scattered through the stone afterwards
                            let actual = chunk.get(local).map(|b| match b.id {
                                BlockId::CoalOre | BlockId::IronOre | BlockId::GoldOre => {
                                    BlockId::Stone
                                }
                                id => id,
                            });
                            assert_eq!(
                                actual, expected,
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

    /// Times the chunks within 6 of the spawn, the largest render distance a server granted by
    /// default when this was written. Ignored: run it in release, with
    /// `cargo test -p server --release generation_time -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn generation_time_of_a_render_distance() {
        let radius = 6;
        let spawn_chunk = IVec3::new(0, SEA_LEVEL / CHUNK_SIZE, 0);
        let start = std::time::Instant::now();
        let generator = WorldGenerator::new(42);
        let mut count = 0;
        for x in -radius..=radius {
            for y in -radius..=radius {
                for z in -radius..=radius {
                    let chunk_pos = spawn_chunk + IVec3::new(x, y, z);
                    std::hint::black_box(generator.generate_chunk(chunk_pos));
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

    fn random<const N: usize>(seed: u32, coords: [i32; N]) -> PositionRandom<N> {
        PositionRandom { seed, coords }
    }

    #[test]
    fn column_random_is_pinned() {
        // The flora of every generated world depends on these: they must not move.
        let pinned = [
            (42, [0, 0], SALT_TREE_ROLL, 0xE83E_6035_A495_EFC6),
            (42, [-1500, 1056], SALT_LEAF + 3, 0x72C8_8AC5_F55C_BD3C),
            (u32::MAX, [7, -7], SALT_FLOWER_ROLL, 0x255C_0451_9D29_2354),
        ];
        for (seed, coords, salt, want) in pinned {
            assert_eq!(random(seed, coords).bits(salt), want, "{coords:?}");
        }
    }

    /// Checks that the hash of `coords` changes with the seed, the salt and each coordinate,
    /// and that reordering the coordinates or stepping diagonally does not collide.
    fn assert_depends_on_every_input<const N: usize>(coords: [i32; N]) {
        let base = random(1, coords).bits(3);
        assert_eq!(base, random(1, coords).bits(3));
        assert_ne!(base, random(1, coords).bits(4));
        assert_ne!(base, random(2, coords).bits(3));
        for axis in 0..N {
            let mut stepped = coords;
            stepped[axis] += 1;
            assert_ne!(base, random(1, stepped).bits(3), "axis {axis}");
            let mut negated = coords;
            negated[axis] = -negated[axis];
            assert_ne!(base, random(1, negated).bits(3), "axis {axis}");
        }
        let mut reversed = coords;
        reversed.reverse();
        assert_ne!(base, random(1, reversed).bits(3));
        let diagonal = coords.map(|c| c + 1);
        assert_ne!(base, random(1, diagonal).bits(3));
    }

    #[test]
    fn position_random_depends_on_every_input_and_nothing_else() {
        assert_depends_on_every_input([5, -7]);
        assert_depends_on_every_input([5, 64, -7]);
    }

    #[test]
    fn position_random_is_roughly_uniform() {
        let mut below_tenth = 0;
        for x in -50..50 {
            for z in -50..50 {
                let column = random(9, [x, z]);
                let cell = random(9, [x, z, x - z]);
                for value in [column.unit(SALT_TREE_ROLL), cell.unit(SALT_TREE_ROLL)] {
                    assert!((0.0..1.0).contains(&value));
                    below_tenth += (value < 0.1) as i32;
                }
                assert!((0..3).contains(&column.below(SALT_TRUNK_HEIGHT, 3)));
            }
        }
        assert!(
            (1800..2200).contains(&below_tenth),
            "{below_tenth} of 20000"
        );
    }

    #[test]
    fn the_spawn_stands_on_the_ground_and_not_on_a_tree() {
        use shared::players::{collision::check_player_collision, Player};

        for seed in [0, 1, 69, 1234, 987_654, 31_337] {
            let spawn = spawn_position(seed);
            let column = spawn.floor().as_ivec3().with_y(0);
            assert!(
                column.x.abs().max(column.z.abs()) <= SPAWN_SEARCH_RADIUS,
                "seed {seed}: {spawn}"
            );
            let mut world = ChunkMap::default();
            let spawn_chunk = column.div_euclid(IVec3::splat(CHUNK_SIZE));
            for y in 0..12 {
                for x in -1..=1 {
                    for z in -1..=1 {
                        let pos = spawn_chunk + IVec3::new(x, y, z);
                        world.insert_chunk(pos, generate_chunk(pos, seed));
                    }
                }
            }
            let player = Player::new(0, "p".into(), spawn, Transform::default());
            assert!(
                !check_player_collision(&spawn, &player, &world),
                "seed {seed}: the spawn {spawn} is in a block"
            );
            let below = (spawn - Vec3::Y * (PLAYER_HEIGHT / 2.0 + 0.5))
                .floor()
                .as_ivec3();
            let ground = world.get_block(below).map(|block| block.id);
            assert!(
                ground
                    .is_some_and(|id| id == BlockId::Water
                        || matches!(id.get_hitbox(), BlockHitbox::FullBlock)),
                "seed {seed}: nothing to stand on under {spawn}: {ground:?}"
            );
            assert!(
                !matches!(
                    ground,
                    Some(BlockId::OakLeaves | BlockId::OakLog | BlockId::Cactus)
                ),
                "seed {seed}: spawned on {ground:?}"
            );
            // Nothing above it to stand on: it is the top of the column
            for y in below.y + 1..=(HIGHEST_SURFACE + STRUCTURE_HEIGHT).max(SEA_LEVEL) {
                let block = world.get_block(below.with_y(y)).map(|block| block.id);
                assert!(
                    block
                        .is_none_or(|id| matches!(id.get_hitbox(), BlockHitbox::None)
                            && id != BlockId::Water),
                    "seed {seed}: {block:?} at {y}, above the spawn"
                );
            }
            assert_eq!(
                spawn_position(seed),
                spawn,
                "the spawn depends on the seed only"
            );
        }
    }
}
