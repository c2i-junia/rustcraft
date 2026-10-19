//! The loaded world seen from above, as the minimap draws it: the top visible block of each
//! column, kept per column of chunks and scanned again when its chunks or its blocks change.

use std::collections::BTreeSet;

use bevy::platform::collections::{HashMap, HashSet};
use bevy::prelude::*;
use shared::world::{BlockId, BlockTransparency, ChunkMap};
use shared::CHUNK_SIZE;

/// The deepest water the map tells apart, in blocks: deeper water looks the same.
pub const MAX_WATER_DEPTH: u8 = 16;

/// The columns of chunks scanned in a frame at most, so that entering a world, which brings
/// hundreds of chunks at once, never makes a frame stutter.
pub const COLUMNS_PER_FRAME: usize = 8;

const TILE_CELLS: usize = (CHUNK_SIZE * CHUNK_SIZE) as usize;

/// What the map shows of a column of the world.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Surface {
    /// The top block seen from above. Under water, the floor of the water, or water when the
    /// floor is deeper than `MAX_WATER_DEPTH` or not loaded.
    pub block: BlockId,
    /// The height of that block, or of the surface of the water over it.
    pub y: i32,
    /// The blocks of water over the floor, up to `MAX_WATER_DEPTH`; 0 out of the water.
    pub water_depth: u8,
}

/// The surface of a column of chunks.
struct ColumnTile {
    /// The heights of the chunks of the column that are loaded.
    chunks: BTreeSet<i32>,
    /// Indexed by `x * CHUNK_SIZE + z`, in local coordinates. `None` for a column with no
    /// visible block, or not scanned yet.
    cells: [Option<Surface>; TILE_CELLS],
}

/// The surface of every column of chunks the client holds.
#[derive(Resource, Default)]
pub struct MinimapSurface {
    tiles: HashMap<IVec2, ColumnTile>,
    /// The columns of chunks to scan again, as their chunks changed.
    dirty: HashSet<IVec2>,
    /// Changes whenever what the map shows changes, so that it is drawn again.
    pub revision: u64,
}

/// Whether the map sees through a block, to the block under it.
fn seen_through(block: BlockId) -> bool {
    block == BlockId::Glass || block.get_visibility() == BlockTransparency::Decoration
}

/// The column of chunks of a position, and the index of its cell in the tile.
fn split(x: i32, z: i32) -> (IVec2, usize) {
    let column = IVec2::new(x.div_euclid(CHUNK_SIZE), z.div_euclid(CHUNK_SIZE));
    let cell = x.rem_euclid(CHUNK_SIZE) * CHUNK_SIZE + z.rem_euclid(CHUNK_SIZE);
    (column, cell as usize)
}

/// What the map shows at `x`, `z`, looking down through the loaded chunks of the column, at
/// the heights `chunks`.
fn scan_cell(map: &ChunkMap, chunks: &BTreeSet<i32>, x: i32, z: i32) -> Option<Surface> {
    let (column, _) = split(x, z);
    let local = IVec2::new(x.rem_euclid(CHUNK_SIZE), z.rem_euclid(CHUNK_SIZE));
    let mut water_top: Option<i32> = None;
    let water = |floor: BlockId, top: i32, depth: i32| Surface {
        block: floor,
        y: top,
        water_depth: depth.min(i32::from(MAX_WATER_DEPTH)) as u8,
    };
    for &chunk_y in chunks.iter().rev() {
        let Some(chunk) = map.chunk(IVec3::new(column.x, chunk_y, column.y)) else {
            continue;
        };
        for local_y in (0..CHUNK_SIZE).rev() {
            let y = chunk_y * CHUNK_SIZE + local_y;
            if let Some(top) = water_top {
                if top - y >= i32::from(MAX_WATER_DEPTH) {
                    return Some(water(BlockId::Water, top, top - y));
                }
            }
            let Some(block) = chunk.get(IVec3::new(local.x, local_y, local.y)) else {
                continue;
            };
            if seen_through(block.id) {
                continue;
            }
            if block.id == BlockId::Water {
                water_top.get_or_insert(y);
                continue;
            }
            return Some(match water_top {
                Some(top) => water(block.id, top, top - y),
                None => Surface {
                    block: block.id,
                    y,
                    water_depth: 0,
                },
            });
        }
    }
    // Water over chunks that are not loaded: as deep as the map tells
    water_top.map(|top| water(BlockId::Water, top, i32::from(MAX_WATER_DEPTH)))
}

impl MinimapSurface {
    /// A surface for the chunks of `map`, all to scan.
    pub fn of(map: &ChunkMap) -> Self {
        let mut surface = Self::default();
        for chunk in map.chunk_positions() {
            surface.chunk_loaded(chunk);
        }
        surface
    }

    /// What the map shows at `x`, `z`: `None` where nothing is loaded or seen.
    pub fn get(&self, x: i32, z: i32) -> Option<Surface> {
        let (column, cell) = split(x, z);
        self.tiles.get(&column)?.cells[cell]
    }

    /// A chunk came in: its column is scanned again.
    pub fn chunk_loaded(&mut self, chunk: IVec3) {
        let column = chunk.xz();
        self.tiles
            .entry(column)
            .or_insert_with(|| ColumnTile {
                chunks: BTreeSet::new(),
                cells: [None; TILE_CELLS],
            })
            .chunks
            .insert(chunk.y);
        self.dirty.insert(column);
    }

    /// A chunk went: its column is scanned again, or forgotten with its last chunk.
    pub fn chunk_unloaded(&mut self, chunk: IVec3) {
        let column = chunk.xz();
        let Some(tile) = self.tiles.get_mut(&column) else {
            return;
        };
        tile.chunks.remove(&chunk.y);
        if tile.chunks.is_empty() {
            self.tiles.remove(&column);
            self.dirty.remove(&column);
            self.revision += 1;
        } else {
            self.dirty.insert(column);
        }
    }

    /// The block at `position` changed in `map`: its cell is scanned again, unless its whole
    /// column is about to be.
    pub fn block_changed(&mut self, map: &ChunkMap, position: IVec3) {
        let (column, cell) = split(position.x, position.z);
        if self.dirty.contains(&column) {
            return;
        }
        let Some(tile) = self.tiles.get_mut(&column) else {
            return;
        };
        let surface = scan_cell(map, &tile.chunks, position.x, position.z);
        if tile.cells[cell] != surface {
            tile.cells[cell] = surface;
            self.revision += 1;
        }
    }

    /// Scans again at most `budget` dirty columns, the nearest to the column `near` first.
    pub fn scan_dirty(&mut self, map: &ChunkMap, near: IVec2, budget: usize) {
        let mut dirty: Vec<IVec2> = self.dirty.iter().copied().collect();
        dirty.sort_by_key(|column| (*column - near).abs().max_element());
        for column in dirty.into_iter().take(budget) {
            self.dirty.remove(&column);
            let Some(tile) = self.tiles.get_mut(&column) else {
                continue;
            };
            let origin = column * CHUNK_SIZE;
            let mut changed = false;
            for (cell, surface) in tile.cells.iter_mut().enumerate() {
                let x = origin.x + cell as i32 / CHUNK_SIZE;
                let z = origin.y + cell as i32 % CHUNK_SIZE;
                let scanned = scan_cell(map, &tile.chunks, x, z);
                changed |= *surface != scanned;
                *surface = scanned;
            }
            if changed {
                self.revision += 1;
            }
        }
    }

    /// Whether some columns wait to be scanned.
    pub fn has_dirty(&self) -> bool {
        !self.dirty.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use shared::world::{BlockData, BlockDirection, Chunk};

    use super::*;

    fn block(id: BlockId) -> BlockData {
        BlockData::new(id, BlockDirection::Front)
    }

    /// A map holding `blocks`, with every chunk they fall in.
    fn map_of(blocks: &[(IVec3, BlockId)]) -> ChunkMap {
        let mut map = ChunkMap::default();
        for &(position, id) in blocks {
            let chunk = position.div_euclid(IVec3::splat(CHUNK_SIZE));
            if !map.is_chunk_loaded(chunk) {
                map.insert_chunk(chunk, Chunk::new());
            }
            map.set_block(position, block(id));
        }
        map
    }

    /// The surface of `map`, fully scanned.
    fn scanned(map: &ChunkMap) -> MinimapSurface {
        let mut surface = MinimapSurface::of(map);
        surface.scan_dirty(map, IVec2::ZERO, usize::MAX);
        surface
    }

    fn solid(block: BlockId, y: i32) -> Option<Surface> {
        Some(Surface {
            block,
            y,
            water_depth: 0,
        })
    }

    #[test]
    fn the_top_block_of_a_column_is_its_surface() {
        let map = map_of(&[
            (IVec3::new(3, 60, 5), BlockId::Stone),
            (IVec3::new(3, 61, 5), BlockId::Grass),
        ]);
        let surface = scanned(&map);
        assert_eq!(surface.get(3, 5), solid(BlockId::Grass, 61));
        // a column of air in a loaded chunk shows nothing
        assert_eq!(surface.get(4, 5), None);
        // nor does a column of chunks never loaded
        assert_eq!(surface.get(100, 5), None);
    }

    #[test]
    fn glass_and_plants_are_seen_through_and_leaves_are_not() {
        let map = map_of(&[
            (IVec3::new(0, 10, 0), BlockId::Grass),
            (IVec3::new(0, 11, 0), BlockId::TallGrass),
            (IVec3::new(1, 10, 0), BlockId::Sand),
            (IVec3::new(1, 12, 0), BlockId::Glass),
            (IVec3::new(2, 10, 0), BlockId::Grass),
            (IVec3::new(2, 14, 0), BlockId::OakLeaves),
        ]);
        let surface = scanned(&map);
        assert_eq!(surface.get(0, 0), solid(BlockId::Grass, 10));
        assert_eq!(surface.get(1, 0), solid(BlockId::Sand, 10));
        assert_eq!(surface.get(2, 0), solid(BlockId::OakLeaves, 14));
    }

    #[test]
    fn water_keeps_its_depth_down_to_its_floor_or_to_the_deepest_told() {
        let mut blocks = vec![(IVec3::new(0, 5, 0), BlockId::Sand)];
        blocks.extend((6..=8).map(|y| (IVec3::new(0, y, 0), BlockId::Water)));
        // deeper than the map tells apart, across two chunks
        blocks.push((IVec3::new(1, -10, 0), BlockId::Dirt));
        blocks.extend((-9..=12).map(|y| (IVec3::new(1, y, 0), BlockId::Water)));
        let map = map_of(&blocks);
        let surface = scanned(&map);
        let water = |block, y, water_depth| {
            Some(Surface {
                block,
                y,
                water_depth,
            })
        };
        assert_eq!(surface.get(0, 0), water(BlockId::Sand, 8, 3));
        assert_eq!(
            surface.get(1, 0),
            water(BlockId::Water, 12, MAX_WATER_DEPTH)
        );
    }

    #[test]
    fn a_column_reads_its_chunks_from_the_top_and_negative_positions_their_own_cells() {
        let map = map_of(&[
            (IVec3::new(-1, 3, -17), BlockId::Stone),
            (IVec3::new(-1, 40, -17), BlockId::Snow),
            (IVec3::new(-16, -30, -1), BlockId::Dirt),
        ]);
        let surface = scanned(&map);
        assert_eq!(surface.get(-1, -17), solid(BlockId::Snow, 40));
        assert_eq!(surface.get(-16, -1), solid(BlockId::Dirt, -30));
        assert_eq!(surface.get(-2, -17), None);
    }

    #[test]
    fn a_changed_block_rescans_its_cell_only_and_the_revision_moves() {
        let mut map = map_of(&[
            (IVec3::new(0, 10, 0), BlockId::Grass),
            (IVec3::new(1, 10, 0), BlockId::Grass),
        ]);
        let mut surface = scanned(&map);
        let revision = surface.revision;

        map.set_block(IVec3::new(0, 11, 0), block(BlockId::Cobblestone));
        // changed in the map but not told: the other cell keeps what was scanned
        map.set_block(IVec3::new(1, 11, 0), block(BlockId::Cobblestone));
        surface.block_changed(&map, IVec3::new(0, 11, 0));
        assert_eq!(surface.get(0, 0), solid(BlockId::Cobblestone, 11));
        assert_eq!(surface.get(1, 0), solid(BlockId::Grass, 10));
        assert!(surface.revision > revision);

        // a change under the surface changes nothing
        let revision = surface.revision;
        map.set_block(IVec3::new(0, 2, 0), block(BlockId::Stone));
        surface.block_changed(&map, IVec3::new(0, 2, 0));
        assert_eq!(surface.revision, revision);
    }

    #[test]
    fn an_unloaded_chunk_takes_its_blocks_away_and_the_last_one_its_column() {
        let mut map = map_of(&[
            (IVec3::new(0, 20, 0), BlockId::Snow),
            (IVec3::new(0, 2, 0), BlockId::Stone),
        ]);
        let mut surface = scanned(&map);
        assert_eq!(surface.get(0, 0), solid(BlockId::Snow, 20));

        map.remove_chunk(IVec3::new(0, 1, 0));
        surface.chunk_unloaded(IVec3::new(0, 1, 0));
        // shown until the column is scanned again
        assert!(surface.has_dirty());
        surface.scan_dirty(&map, IVec2::ZERO, 1);
        assert_eq!(surface.get(0, 0), solid(BlockId::Stone, 2));

        let revision = surface.revision;
        map.remove_chunk(IVec3::ZERO);
        surface.chunk_unloaded(IVec3::ZERO);
        assert_eq!(surface.get(0, 0), None);
        assert!(surface.revision > revision);
        assert!(!surface.has_dirty());
    }

    #[test]
    fn the_dirty_columns_nearest_to_the_player_are_scanned_first_within_the_budget() {
        let far = IVec3::new(5 * CHUNK_SIZE, 0, 0);
        let near = IVec3::new(CHUNK_SIZE, 0, 0);
        let map = map_of(&[(far, BlockId::Stone), (near, BlockId::Stone)]);
        let mut surface = MinimapSurface::of(&map);
        surface.scan_dirty(&map, IVec2::ZERO, 1);
        assert_eq!(surface.get(near.x, 0), solid(BlockId::Stone, 0));
        assert_eq!(surface.get(far.x, 0), None);
        surface.scan_dirty(&map, IVec2::ZERO, 1);
        assert_eq!(surface.get(far.x, 0), solid(BlockId::Stone, 0));
        assert!(!surface.has_dirty());
    }
}
