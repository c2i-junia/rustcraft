use std::collections::HashMap;

use bevy::math::IVec3;

use crate::world::{BlockData, BlockDirection, BlockId, WorldMap};

/// Minimal in-memory `WorldMap` keyed by global block position, for unit tests.
#[derive(Default)]
pub(crate) struct TestWorld {
    pub blocks: HashMap<IVec3, BlockData>,
}

impl TestWorld {
    /// Fills every block of the box `min..=max` (global positions) with `id`.
    pub fn fill_box(&mut self, min: IVec3, max: IVec3, id: BlockId) {
        for x in min.x..=max.x {
            for y in min.y..=max.y {
                for z in min.z..=max.z {
                    self.set_block(
                        &IVec3::new(x, y, z),
                        BlockData::new(id, BlockDirection::Front),
                    );
                }
            }
        }
    }
}

impl WorldMap for TestWorld {
    fn get_block_mut_by_coordinates(&mut self, position: &IVec3) -> Option<&mut BlockData> {
        self.blocks.get_mut(position)
    }

    fn get_block_by_coordinates(&self, position: &IVec3) -> Option<&BlockData> {
        self.blocks.get(position)
    }

    fn remove_block_by_coordinates(&mut self, global_block_pos: &IVec3) -> Option<BlockData> {
        self.blocks.remove(global_block_pos)
    }

    fn set_block(&mut self, position: &IVec3, block: BlockData) {
        self.blocks.insert(*position, block);
    }

    fn mark_block_for_update(&mut self, _position: &IVec3) {}
}
