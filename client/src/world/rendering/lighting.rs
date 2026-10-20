//! The light levels of the cells, from 0 to `MAX_LIGHT` as Minecraft's: the light of the sky,
//! which comes straight down from it, and the light of the blocks that give some, such as the
//! torch. Each spreads to the cells around through the blocks that let light through, a level
//! less a cell, so that a cave is dark but for its mouth.
//!
//! The chunks are cubes, loaded around the player in 3D, and whether a cell sees the sky
//! depends on the chunks above it, which may not be loaded. The light is worked out for the
//! chunks of a group as they are meshed, over a box one chunk larger on every side
//! (`LightVolume`): the light of a cell farther out never reaches them. The light of the sky
//! comes into the box through the loaded chunks above it, and where no chunk is loaded, the
//! terrain as generated says whether a cell is ground or air (`TerrainPrior`).

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use bevy::math::{IVec2, IVec3, Vec3Swizzles};
use shared::world::{BlockData, ChunkMap, TerrainNoises, HIGHEST_SURFACE, LOWEST_SURFACE};
use shared::CHUNK_SIZE;

/// The highest light level: that of the open sky.
pub const MAX_LIGHT: u8 = 15;

/// The opacity of a cell no light goes through.
const OPAQUE: u8 = u8::MAX;

/// The opacity of a cell holding `block`: see `BlockId::light_opacity`.
fn opacity(block: Option<&BlockData>) -> u8 {
    block.map_or(0, |block| block.id.light_opacity().unwrap_or(OPAQUE))
}

fn emission(block: Option<&BlockData>) -> u8 {
    block.map_or(0, |block| block.id.light_emission().min(MAX_LIGHT))
}

/// The surface of the terrain as the seed generates it, by chunk column, worked out once for
/// all the meshing tasks of a world: the noises cost about as much as the rest of the light.
pub(crate) struct TerrainSurfaces {
    pub noises: Arc<TerrainNoises>,
    /// The height of the surface of each column of a chunk column, by `x * CHUNK_SIZE + z`.
    columns: Mutex<HashMap<IVec2, Arc<ChunkSurface>>>,
}

type ChunkSurface = [i32; (CHUNK_SIZE * CHUNK_SIZE) as usize];

/// How many chunk columns `TerrainSurfaces` keeps before it starts again, about 8 MB.
const MAX_SURFACES: usize = 8192;

impl TerrainSurfaces {
    pub fn new(noises: Arc<TerrainNoises>) -> Arc<Self> {
        Arc::new(Self {
            noises,
            columns: Mutex::default(),
        })
    }

    /// The surface of the chunk column `chunk_column`.
    fn of(&self, chunk_column: IVec2) -> Arc<ChunkSurface> {
        if let Some(surface) = self.columns.lock().unwrap().get(&chunk_column) {
            return surface.clone();
        }
        // Worked out without the lock: another task may do the same meanwhile
        let first = chunk_column * CHUNK_SIZE;
        let surface: Arc<ChunkSurface> = Arc::new(std::array::from_fn(|at| {
            let (x, z) = (at as i32 / CHUNK_SIZE, at as i32 % CHUNK_SIZE);
            self.noises.column_shape(first.x + x, first.y + z).height
        }));
        let mut columns = self.columns.lock().unwrap();
        if columns.len() >= MAX_SURFACES {
            columns.clear();
        }
        columns.insert(chunk_column, surface.clone());
        surface
    }
}

/// Whether the cells of the chunks that are not loaded are ground or air: the terrain as the
/// seed generates it, without what the players changed. Without the surfaces, every such cell
/// is air.
pub(crate) struct TerrainPrior {
    surfaces: Option<Arc<TerrainSurfaces>>,
    /// The surfaces of the chunk columns asked for, kept to be read without the lock.
    read: HashMap<IVec2, Arc<ChunkSurface>>,
}

impl TerrainPrior {
    pub fn new(surfaces: Option<Arc<TerrainSurfaces>>) -> Self {
        Self {
            surfaces,
            read: HashMap::new(),
        }
    }

    /// Whether the cell at `pos` is under the surface of the generated terrain.
    fn is_ground(&mut self, pos: IVec3) -> bool {
        self.ground_cells(pos.xz(), pos.y, 1) == 1
    }

    /// How many of the `count` cells of the column `column` from the height `bottom` up are
    /// under the surface of the generated terrain: they are the lowest ones. The noises are
    /// only read when the cells reach between the lowest and the highest surface.
    fn ground_cells(&mut self, column: IVec2, bottom: i32, count: i32) -> usize {
        let Some(surfaces) = &self.surfaces else {
            return 0;
        };
        let top = if bottom > HIGHEST_SURFACE {
            return 0;
        } else if bottom + count - 1 <= LOWEST_SURFACE {
            LOWEST_SURFACE
        } else {
            let chunk_column = column.div_euclid(IVec2::splat(CHUNK_SIZE));
            let local = column.rem_euclid(IVec2::splat(CHUNK_SIZE));
            let surface = self
                .read
                .entry(chunk_column)
                .or_insert_with(|| surfaces.of(chunk_column));
            surface[(local.x * CHUNK_SIZE + local.y) as usize]
                .clamp(LOWEST_SURFACE, HIGHEST_SURFACE)
        };
        (top - bottom + 1).clamp(0, count) as usize
    }
}

/// The light of a box of cells, and what lets light through: the chunks of a group and one
/// chunk around them (`of_chunks`).
pub(crate) struct LightVolume {
    /// The cell at the lowest corner of the box.
    min: IVec3,
    size: IVec3,
    /// By cell, `(x * size.z + z) * size.y + y` from `min`: the cells of a column follow one
    /// another, from the bottom up.
    opacity: Vec<u8>,
    sky: Vec<u8>,
    block: Vec<u8>,
}

/// The light of a cell, as `LightVolume::light` gives it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Light {
    pub sky: u8,
    pub block: u8,
}

/// The chunks around `chunks` whose blocks their light depends on: the box one chunk larger
/// than them on every side, and the loaded chunks above it, through which the sky shines.
pub(crate) fn chunks_lighting<'a>(
    blocks: &'a ChunkMap,
    chunks: &[IVec3],
) -> impl Iterator<Item = IVec3> + 'a {
    let bounds = chunk_bounds(chunks);
    bounds.into_iter().flat_map(move |(lowest, highest)| {
        (lowest.x..=highest.x).flat_map(move |x| {
            (lowest.z..=highest.z).flat_map(move |z| {
                let above = (highest.y + 1..)
                    .map(move |y| IVec3::new(x, y, z))
                    .take_while(|pos| blocks.is_chunk_loaded(*pos));
                (lowest.y..=highest.y)
                    .map(move |y| IVec3::new(x, y, z))
                    .chain(above)
            })
        })
    })
}

/// The lowest and the highest corners of the box of chunks around `chunks`, one chunk larger
/// on every side. `None` for no chunk.
fn chunk_bounds(chunks: &[IVec3]) -> Option<(IVec3, IVec3)> {
    let lowest = chunks.iter().copied().reduce(IVec3::min)?;
    let highest = chunks.iter().copied().reduce(IVec3::max)?;
    Some((lowest - IVec3::ONE, highest + IVec3::ONE))
}

/// The layers of chunks of the box from `lowest` to `highest` whose light the faces of the
/// chunks inside it read: from one under the lowest chunk that lets light through, to one
/// above the highest chunk inside that holds a block. The air above is lit by the sky as it
/// comes into the box, and the faces never read the light of the ground under it. `None` when
/// the chunks inside hold no block.
fn read_layers(blocks: &ChunkMap, (lowest, highest): (IVec3, IVec3)) -> Option<(i32, i32)> {
    let mut lowest_lit = highest.y;
    let mut highest_filled = None;
    for x in lowest.x..=highest.x {
        for z in lowest.z..=highest.z {
            for y in lowest.y..=highest.y {
                let pos = IVec3::new(x, y, z);
                let Some(chunk) = blocks.chunk(pos) else {
                    continue;
                };
                if y < lowest_lit && chunk.may_hold(|cell| opacity(cell) != OPAQUE) {
                    lowest_lit = y;
                }
                let inside = pos.cmpgt(lowest).all() && pos.cmplt(highest).all();
                if inside && highest_filled < Some(y) && chunk.may_hold(|cell| cell.is_some()) {
                    highest_filled = Some(y);
                }
            }
        }
    }
    let highest_filled = highest_filled?;
    Some((
        (lowest_lit - 1).max(lowest.y),
        (highest_filled + 1).min(highest.y),
    ))
}

impl LightVolume {
    /// The light of the cells of `chunks` and of the chunks around them, from the blocks of
    /// `blocks`, which holds them and the chunks above them (`chunks_lighting`). Only the
    /// layers of chunks whose light the faces read are lit (`read_layers`).
    pub fn of_chunks(blocks: &ChunkMap, chunks: &[IVec3], prior: &mut TerrainPrior) -> Self {
        let Some((lowest, highest)) = chunk_bounds(chunks) else {
            return Self::open_sky();
        };
        let Some((bottom, top)) = read_layers(blocks, (lowest, highest)) else {
            return Self::open_sky();
        };
        let (lowest, highest) = (lowest.with_y(bottom), highest.with_y(top.max(bottom)));
        let min = lowest * CHUNK_SIZE;
        let size = (highest - lowest + IVec3::ONE) * CHUNK_SIZE;
        let cells = (size.x * size.y * size.z) as usize;
        let mut volume = Self {
            min,
            size,
            opacity: vec![0; cells],
            sky: vec![0; cells],
            block: vec![0; cells],
        };
        let emitters = volume.fill(blocks, prior, (lowest, highest));
        volume.light_sky(blocks, prior, highest.y + 1);
        volume.light_blocks(emitters);
        volume
    }

    /// A box of no cell: every cell is outside, under the open sky, and lets light through.
    pub fn open_sky() -> Self {
        Self {
            min: IVec3::ZERO,
            size: IVec3::ZERO,
            opacity: Vec::new(),
            sky: Vec::new(),
            block: Vec::new(),
        }
    }

    fn index(&self, local: IVec3) -> usize {
        ((local.x * self.size.z + local.z) * self.size.y + local.y) as usize
    }

    /// The index of the cell at `pos`, in the world, `None` outside the box.
    fn index_of(&self, pos: IVec3) -> Option<usize> {
        let local = pos - self.min;
        (local.cmpge(IVec3::ZERO).all() && local.cmplt(self.size).all()).then(|| self.index(local))
    }

    /// The light of the cell at `pos`, in the world: that of the open sky outside the box, but
    /// none under it, in the ground.
    pub fn light(&self, pos: IVec3) -> Light {
        match self.index_of(pos) {
            Some(index) => Light {
                sky: self.sky[index],
                block: self.block[index],
            },
            None if self.size.y > 0 && pos.y < self.min.y => Light { sky: 0, block: 0 },
            None => Light {
                sky: MAX_LIGHT,
                block: 0,
            },
        }
    }

    /// The light of the cell at `pos` as `light` gives it, and whether it is opaque as
    /// `is_opaque` says: both read at once.
    pub fn cell(&self, pos: IVec3) -> (Light, bool) {
        match self.index_of(pos) {
            Some(index) => (
                Light {
                    sky: self.sky[index],
                    block: self.block[index],
                },
                self.opacity[index] == OPAQUE,
            ),
            None => (self.light(pos), false),
        }
    }

    /// Whether no light goes through the cell at `pos`, in the world: none outside the box.
    pub fn is_opaque(&self, pos: IVec3) -> bool {
        self.index_of(pos)
            .is_some_and(|index| self.opacity[index] == OPAQUE)
    }

    /// Writes the opacity of each cell of the chunks from `lowest` to `highest`, from their
    /// blocks or, for those that are not loaded, from `prior`. Returns the cells that give
    /// light, with their light.
    fn fill(
        &mut self,
        blocks: &ChunkMap,
        prior: &mut TerrainPrior,
        (lowest, highest): (IVec3, IVec3),
    ) -> Vec<(usize, u8)> {
        let side = CHUNK_SIZE as usize;
        let mut emitters = Vec::new();
        for x in lowest.x..=highest.x {
            for y in lowest.y..=highest.y {
                for z in lowest.z..=highest.z {
                    let chunk_pos = IVec3::new(x, y, z);
                    let corner = chunk_pos * CHUNK_SIZE - self.min;
                    let Some(chunk) = blocks.chunk(chunk_pos) else {
                        // The cells of a column of the chunk follow one another, from the
                        // bottom up, and the ground is at the bottom
                        let first = chunk_pos * CHUNK_SIZE;
                        for x in 0..CHUNK_SIZE {
                            for z in 0..CHUNK_SIZE {
                                let column = first.xz() + IVec2::new(x, z);
                                let ground = prior.ground_cells(column, first.y, CHUNK_SIZE);
                                let start = self.index(corner + IVec3::new(x, 0, z));
                                self.opacity[start..start + ground].fill(OPAQUE);
                            }
                        }
                        continue;
                    };
                    if let Some(cell) = chunk.uniform() {
                        if opacity(cell) != 0 || emission(cell) != 0 {
                            for local in chunk_cells() {
                                let index = self.index(corner + local);
                                self.opacity[index] = opacity(cell);
                                if emission(cell) > 0 {
                                    emitters.push((index, emission(cell)));
                                }
                            }
                        }
                        continue;
                    }
                    // By `(x * side + y) * side + z` in the chunk, by column in the box
                    let opacities = chunk.map_cells(opacity);
                    let emissions = chunk
                        .may_hold(|cell| emission(cell) > 0)
                        .then(|| chunk.map_cells(emission));
                    for x in 0..side {
                        for z in 0..side {
                            let local = IVec3::new(x as i32, 0, z as i32);
                            let start = self.index(corner + local);
                            for y in 0..side {
                                let at = (x * side + y) * side + z;
                                self.opacity[start + y] = opacities[at];
                                if let Some(level) = emissions.as_ref().map(|e| e[at]) {
                                    if level > 0 {
                                        emitters.push((start + y, level));
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        emitters
    }

    /// Lights the cells from the sky: straight down from the light that comes into the box
    /// from above, then spread to the sides.
    fn light_sky(&mut self, blocks: &ChunkMap, prior: &mut TerrainPrior, above: i32) {
        let height = self.size.y as usize;
        let columns = IVec2::new(self.size.x, self.size.z);
        // For each column, the lowest cell from which every cell up is fully lit
        let mut full_from = vec![height; (columns.x * columns.y) as usize];
        let chunk_columns = columns / CHUNK_SIZE;
        for chunk_x in 0..chunk_columns.x {
            for chunk_z in 0..chunk_columns.y {
                let first = self.min.xz() / CHUNK_SIZE + IVec2::new(chunk_x, chunk_z);
                let entering = sky_from_above(blocks, prior, first, above);
                for (at, level) in entering.into_iter().enumerate() {
                    let local = IVec2::new(
                        chunk_x * CHUNK_SIZE + at as i32 / CHUNK_SIZE,
                        chunk_z * CHUNK_SIZE + at as i32 % CHUNK_SIZE,
                    );
                    let column = (local.x * columns.y + local.y) as usize;
                    let start = column * height;
                    let mut level = level;
                    let mut full = level == MAX_LIGHT;
                    for y in (0..height).rev() {
                        level = match self.opacity[start + y] {
                            OPAQUE => 0,
                            cell_opacity => level.saturating_sub(cell_opacity),
                        };
                        self.sky[start + y] = level;
                        full &= level == MAX_LIGHT;
                        if full {
                            full_from[column] = y;
                        }
                    }
                }
            }
        }

        // A cell spreads light if a cell next to it would take more: the cells of a column
        // above the fully lit ones of its own and of its neighbours have nothing to give
        let mut buckets = vec![Vec::new(); MAX_LIGHT as usize + 1];
        for x in 0..columns.x {
            for z in 0..columns.y {
                let column = |x: i32, z: i32| full_from[(x * columns.y + z) as usize];
                let mut highest = column(x, z);
                for (nx, nz) in [(x - 1, z), (x + 1, z), (x, z - 1), (x, z + 1)] {
                    if (0..columns.x).contains(&nx) && (0..columns.y).contains(&nz) {
                        highest = highest.max(column(nx, nz));
                    }
                }
                for y in 0..=highest.min(height - 1) {
                    let index = self.index(IVec3::new(x, y as i32, z));
                    let level = self.sky[index];
                    if level > 1
                        && self
                            .neighbours(index)
                            .any(|next| self.takes_more(&self.sky, next, level))
                    {
                        buckets[level as usize].push(index as u32);
                    }
                }
            }
        }
        let mut sky = std::mem::take(&mut self.sky);
        self.spread(&mut sky, buckets);
        self.sky = sky;
    }

    /// Lights the cells from the blocks that give light, `emitters`.
    fn light_blocks(&mut self, emitters: Vec<(usize, u8)>) {
        if emitters.is_empty() {
            return;
        }
        let mut buckets = vec![Vec::new(); MAX_LIGHT as usize + 1];
        let mut block = std::mem::take(&mut self.block);
        for (index, level) in emitters {
            if level > block[index] {
                block[index] = level;
                buckets[level as usize].push(index as u32);
            }
        }
        self.spread(&mut block, buckets);
        self.block = block;
    }

    /// Whether the cell `next` would take more light in `light` from a cell next to it at
    /// `level`.
    fn takes_more(&self, light: &[u8], next: usize, level: u8) -> bool {
        let next_opacity = self.opacity[next];
        next_opacity != OPAQUE && level.saturating_sub(next_opacity.max(1)) > light[next]
    }

    /// Spreads the light of `light` from the cells of `buckets`, each in the bucket of its
    /// level: the brightest first, so that each cell is lit once.
    fn spread(&self, light: &mut [u8], mut buckets: Vec<Vec<u32>>) {
        for level in (2..=MAX_LIGHT).rev() {
            while let Some(index) = buckets[level as usize].pop() {
                let index = index as usize;
                // Lit brighter since it was put in
                if light[index] != level {
                    continue;
                }
                for next in self.neighbours(index) {
                    if self.takes_more(light, next, level) {
                        let next_level = level - self.opacity[next].max(1);
                        light[next] = next_level;
                        buckets[next_level as usize].push(next as u32);
                    }
                }
            }
        }
    }

    /// The indices of the cells next to the cell at `index`, in the box.
    fn neighbours(&self, index: usize) -> impl Iterator<Item = usize> {
        let height = self.size.y as usize;
        let depth = self.size.z as usize;
        let y = index % height;
        let z = index / height % depth;
        let x = index / (height * depth);
        let (width, column) = (self.size.x as usize, height * depth);
        [
            (y > 0).then(|| index - 1),
            (y + 1 < height).then(|| index + 1),
            (z > 0).then(|| index - height),
            (z + 1 < depth).then(|| index + height),
            (x > 0).then(|| index - column),
            (x + 1 < width).then(|| index + column),
        ]
        .into_iter()
        .flatten()
    }
}

/// The local positions of a chunk.
fn chunk_cells() -> impl Iterator<Item = IVec3> {
    (0..CHUNK_SIZE).flat_map(|x| {
        (0..CHUNK_SIZE).flat_map(move |y| (0..CHUNK_SIZE).map(move |z| IVec3::new(x, y, z)))
    })
}

/// The light of the sky that comes down into the chunk below `(column.x, above, column.y)`,
/// by column `x * CHUNK_SIZE + z`: through the loaded chunks from there up, and from above
/// them the open sky, unless `prior` says the cell above is ground.
fn sky_from_above(
    blocks: &ChunkMap,
    prior: &mut TerrainPrior,
    column: IVec2,
    above: i32,
) -> Vec<u8> {
    let chunk_at = |y: i32| blocks.chunk(IVec3::new(column.x, y, column.y));
    let loaded: Vec<_> = (above..).map_while(chunk_at).collect();
    let top = (above + loaded.len() as i32) * CHUNK_SIZE;
    let side = CHUNK_SIZE as usize;
    let mut levels: Vec<u8> = (0..side * side)
        .map(|at| {
            let cell = IVec3::new(
                column.x * CHUNK_SIZE + (at / side) as i32,
                top,
                column.y * CHUNK_SIZE + (at % side) as i32,
            );
            if prior.is_ground(cell) {
                0
            } else {
                MAX_LIGHT
            }
        })
        .collect();
    for chunk in loaded.iter().rev() {
        if levels.iter().all(|level| *level == 0) {
            break;
        }
        if let Some(cell) = chunk.uniform() {
            match opacity(cell) {
                OPAQUE => levels.fill(0),
                cell_opacity => {
                    for level in levels.iter_mut() {
                        *level =
                            level.saturating_sub(cell_opacity.saturating_mul(CHUNK_SIZE as u8));
                    }
                }
            }
            continue;
        }
        let opacities = chunk.map_cells(opacity);
        for (at, level) in levels.iter_mut().enumerate() {
            let (x, z) = (at / side, at % side);
            for y in (0..side).rev() {
                *level = match opacities[(x * side + y) * side + z] {
                    OPAQUE => 0,
                    cell_opacity => level.saturating_sub(cell_opacity),
                };
            }
        }
    }
    levels
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::world::{BlockDirection, BlockId, Chunk};

    fn block(id: BlockId) -> BlockData {
        BlockData::new(id, BlockDirection::Front)
    }

    #[test]
    fn the_ground_of_the_prior_is_the_bottom_of_a_column_up_to_the_surface() {
        let noises = Arc::new(TerrainNoises::new(42));
        let surface = noises.column_shape(5, 9).height;
        let mut prior = TerrainPrior::new(Some(TerrainSurfaces::new(noises)));
        let column = IVec2::new(5, 9);
        for bottom in [surface - 20, surface - 3, surface, surface + 1] {
            let ground = prior.ground_cells(column, bottom, CHUNK_SIZE);
            let expected = (0..CHUNK_SIZE)
                .take_while(|dy| prior.is_ground(IVec3::new(5, bottom + dy, 9)))
                .count();
            assert_eq!(ground, expected, "from {bottom}");
            assert_eq!(ground, (surface - bottom + 1).clamp(0, CHUNK_SIZE) as usize);
        }
        assert_eq!(prior.ground_cells(column, LOWEST_SURFACE - 40, 16), 16);
        assert_eq!(prior.ground_cells(column, HIGHEST_SURFACE + 1, 16), 0);
        assert_eq!(TerrainPrior::new(None).ground_cells(column, -100, 16), 0);
    }

    /// A world of the chunks from (-1, -1, -1) to (1, 1, 1), all air but for `blocks`.
    fn world(blocks: &[(IVec3, BlockId)]) -> ChunkMap {
        let mut world = ChunkMap::default();
        for x in -1..=1 {
            for y in -1..=1 {
                for z in -1..=1 {
                    world.insert_chunk(IVec3::new(x, y, z), Chunk::new());
                }
            }
        }
        for (pos, id) in blocks {
            world.set_block(*pos, block(*id));
        }
        world
    }

    /// The light of the chunk at the origin and of those around it, in `world`.
    fn light_of(world: &ChunkMap) -> LightVolume {
        LightVolume::of_chunks(world, &[IVec3::ZERO], &mut TerrainPrior::new(None))
    }

    /// A floor of stone at y = 0 across the chunks from -1 to 1.
    fn floor() -> Vec<(IVec3, BlockId)> {
        let mut blocks = Vec::new();
        for x in -16..32 {
            for z in -16..32 {
                blocks.push((IVec3::new(x, 0, z), BlockId::Stone));
            }
        }
        blocks
    }

    /// A box of stone from 2 to 10 on each axis, hollow from 3 to 9.
    fn hollow_box() -> Vec<(IVec3, BlockId)> {
        let mut blocks = Vec::new();
        for x in 2..=10 {
            for y in 2..=10 {
                for z in 2..=10 {
                    let inside = [x, y, z].iter().all(|c| (3..=9).contains(c));
                    if !inside {
                        blocks.push((IVec3::new(x, y, z), BlockId::Stone));
                    }
                }
            }
        }
        blocks
    }

    #[test]
    fn the_sky_lights_the_air_and_not_the_ground() {
        let light = light_of(&world(&floor()));
        assert_eq!(light.light(IVec3::new(5, 1, 5)).sky, MAX_LIGHT);
        assert_eq!(light.light(IVec3::new(5, 0, 5)).sky, 0);
        // Below the floor, the light comes round from the edges of the box, far away
        assert_eq!(light.light(IVec3::new(5, -1, 5)).sky, 0);
        assert!(light.is_opaque(IVec3::new(5, 0, 5)));
        assert!(!light.is_opaque(IVec3::new(5, 1, 5)));
    }

    #[test]
    fn a_closed_cave_is_dark_and_an_opened_one_darkens_away_from_its_mouth() {
        let light = light_of(&world(&hollow_box()));
        for y in 3..=9 {
            assert_eq!(light.light(IVec3::new(6, y, 6)).sky, 0, "{y}");
        }

        // A hole in the roof: straight below it the sky shines, and it fades to the sides
        let mut blocks = hollow_box();
        blocks.retain(|(pos, _)| *pos != IVec3::new(6, 10, 6));
        let light = light_of(&world(&blocks));
        assert_eq!(light.light(IVec3::new(6, 3, 6)).sky, MAX_LIGHT);
        assert_eq!(light.light(IVec3::new(7, 3, 6)).sky, MAX_LIGHT - 1);
        assert_eq!(light.light(IVec3::new(9, 3, 9)).sky, MAX_LIGHT - 6);
    }

    #[test]
    fn the_light_goes_round_walls_a_level_less_a_cell() {
        // A wall along x = 8 up to the top of the box, with a gap at z = 0 above the floor
        let mut blocks = floor();
        for y in -16..32 {
            for z in -16..32 {
                if z != 0 || y > 3 {
                    blocks.push((IVec3::new(8, y, z), BlockId::Stone));
                }
            }
        }
        // A roof over x >= 8, so that the sky reaches it through the gap only
        for x in 9..32 {
            for z in -16..32 {
                blocks.push((IVec3::new(x, 20, z), BlockId::Stone));
            }
        }
        let light = light_of(&world(&blocks));
        assert_eq!(light.light(IVec3::new(8, 1, 0)).sky, MAX_LIGHT - 1);
        assert_eq!(light.light(IVec3::new(9, 1, 0)).sky, MAX_LIGHT - 2);
        assert_eq!(light.light(IVec3::new(9, 1, 3)).sky, MAX_LIGHT - 5);
    }

    #[test]
    fn leaves_and_water_dim_the_sky_and_glass_does_not() {
        let mut blocks = floor();
        for y in 5..8 {
            blocks.push((IVec3::new(2, y, 2), BlockId::Glass));
            blocks.push((IVec3::new(6, y, 6), BlockId::Water));
        }
        let light = light_of(&world(&blocks));
        assert_eq!(light.light(IVec3::new(2, 4, 2)).sky, MAX_LIGHT);
        // Three cells of water take three levels away, but the air around lights them from the
        // sides
        assert_eq!(light.light(IVec3::new(6, 5, 6)).sky, MAX_LIGHT - 1);

        // Under a sea, the deeper the darker
        let mut blocks = floor();
        for x in -16..32 {
            for y in 1..10 {
                for z in -16..32 {
                    blocks.push((IVec3::new(x, y, z), BlockId::Water));
                }
            }
        }
        let light = light_of(&world(&blocks));
        assert_eq!(light.light(IVec3::new(5, 9, 5)).sky, MAX_LIGHT - 1);
        assert_eq!(light.light(IVec3::new(5, 1, 5)).sky, MAX_LIGHT - 9);
    }

    #[test]
    fn only_the_layers_the_faces_read_are_lit_and_under_them_is_dark() {
        // Solid ground up to y = 3, under three layers of air
        let mut world = ChunkMap::default();
        for x in -1..=1 {
            for z in -1..=1 {
                world.insert_chunk(IVec3::new(x, -2, z), Chunk::filled(block(BlockId::Stone)));
                world.insert_chunk(IVec3::new(x, -1, z), Chunk::filled(block(BlockId::Stone)));
                for y in 0..=2 {
                    world.insert_chunk(IVec3::new(x, y, z), Chunk::new());
                }
            }
        }
        for x in -16..32 {
            for z in -16..32 {
                for y in 0..=3 {
                    world.set_block(IVec3::new(x, y, z), block(BlockId::Stone));
                }
            }
        }
        let column = [-1, 0, 1].map(|y| IVec3::new(0, y, 0));
        assert_eq!(
            read_layers(&world, chunk_bounds(&column).unwrap()),
            Some((-1, 1))
        );
        let light = LightVolume::of_chunks(&world, &column, &mut TerrainPrior::new(None));
        // The air over the ground is lit, the ground under the box dark
        assert_eq!(light.light(IVec3::new(8, 4, 8)).sky, MAX_LIGHT);
        assert_eq!(light.light(IVec3::new(8, 30, 8)).sky, MAX_LIGHT);
        assert_eq!(light.light(IVec3::new(8, -20, 8)).sky, 0);
        assert!(light.is_opaque(IVec3::new(8, -10, 8)));
        // Chunks of air only have no faces: nothing to light
        assert_eq!(
            read_layers(&world, chunk_bounds(&[IVec3::new(0, 1, 0)]).unwrap()),
            None
        );
    }

    #[test]
    fn the_sky_comes_down_through_the_loaded_chunks_above_the_box() {
        // A block in a corner, so that the chunk has faces to light
        let mut world = world(&[(IVec3::ZERO, BlockId::Stone)]);
        // A roof two chunks up, above the box of the chunk at the origin
        let mut roof = Chunk::new();
        for x in 0..CHUNK_SIZE {
            for z in 0..CHUNK_SIZE {
                roof.set(IVec3::new(x, 8, z), block(BlockId::Stone));
            }
        }
        world.insert_chunk(IVec3::new(0, 2, 0), roof);
        let light = light_of(&world);
        // Lit from the open chunks around, the nearest 8 cells away
        assert_eq!(light.light(IVec3::new(8, 8, 8)).sky, MAX_LIGHT - 8);
        assert_eq!(light.light(IVec3::new(-8, 8, 8)).sky, MAX_LIGHT);
    }

    #[test]
    fn a_cell_under_the_generated_ground_is_dark_until_its_chunks_come() {
        let noises = Arc::new(TerrainNoises::new(42));
        let surface = noises.column_shape(8, 8).height;
        // Air chunks with nothing loaded above, deep under the surface
        let deep = IVec3::new(0, (surface - 5 * CHUNK_SIZE).div_euclid(CHUNK_SIZE), 0);
        let mut world = ChunkMap::default();
        for x in -1..=1 {
            for y in -1..=1 {
                for z in -1..=1 {
                    world.insert_chunk(deep + IVec3::new(x, y, z), Chunk::new());
                }
            }
        }
        // A block in a corner, so that the chunk has faces to light
        world.set_block(deep * CHUNK_SIZE, block(BlockId::Stone));
        let inside = deep * CHUNK_SIZE + IVec3::splat(8);
        let mut prior = TerrainPrior::new(Some(TerrainSurfaces::new(noises)));
        let light = LightVolume::of_chunks(&world, &[deep], &mut prior);
        assert_eq!(light.light(inside).sky, 0);
        // Without the noises, what is not loaded is air
        let light = LightVolume::of_chunks(&world, &[deep], &mut TerrainPrior::new(None));
        assert_eq!(light.light(inside).sky, MAX_LIGHT);
    }

    #[test]
    fn only_the_box_around_the_chunks_and_the_chunks_above_it_are_read() {
        let mut world = world(&[]);
        world.insert_chunk(IVec3::new(0, 2, 0), Chunk::new());
        world.insert_chunk(IVec3::new(0, 4, 0), Chunk::new());
        let read: Vec<_> = chunks_lighting(&world, &[IVec3::ZERO]).collect();
        assert_eq!(read.len(), 27 + 1);
        assert!(read.contains(&IVec3::new(0, 2, 0)));
        assert!(!read.contains(&IVec3::new(0, 4, 0)));
        assert_eq!(chunks_lighting(&world, &[]).count(), 0);
    }

    #[test]
    fn outside_the_box_is_the_open_sky() {
        let light = light_of(&world(&[]));
        assert_eq!(
            light.light(IVec3::new(100, 0, 0)),
            Light {
                sky: MAX_LIGHT,
                block: 0
            }
        );
        assert!(!light.is_opaque(IVec3::new(100, 0, 0)));
    }
}
