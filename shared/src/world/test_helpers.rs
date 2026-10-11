//! Fixtures for tests that need a world: compiled for the tests of this crate and, through the
//! `test-helpers` feature, for the dev-dependencies of the crates that use it.

use bevy::math::IVec3;

use crate::world::{
    global_block_to_chunk_pos, BlockData, BlockDirection, BlockId, Chunk, ChunkMap,
};

impl ChunkMap {
    /// A map in which every chunk touched by the blocks `min..=max` (global positions) is
    /// loaded and empty.
    pub fn with_loaded_region(min: IVec3, max: IVec3) -> Self {
        let mut map = Self::default();
        let (chunk_min, chunk_max) = (
            global_block_to_chunk_pos(&min),
            global_block_to_chunk_pos(&max),
        );
        for x in chunk_min.x..=chunk_max.x {
            for y in chunk_min.y..=chunk_max.y {
                for z in chunk_min.z..=chunk_max.z {
                    map.insert_chunk(IVec3::new(x, y, z), Chunk::new());
                }
            }
        }
        map
    }

    /// Fills every block of the box `min..=max` (global positions) with `id`. The chunks of the
    /// box must be loaded.
    pub fn fill_box(&mut self, min: IVec3, max: IVec3, id: BlockId) {
        for x in min.x..=max.x {
            for y in min.y..=max.y {
                for z in min.z..=max.z {
                    let pos = IVec3::new(x, y, z);
                    assert!(
                        self.is_chunk_loaded(global_block_to_chunk_pos(&pos)),
                        "fill_box: the chunk of {pos} is not loaded"
                    );
                    self.set_block(pos, BlockData::new(id, BlockDirection::Front));
                }
            }
        }
    }
}
