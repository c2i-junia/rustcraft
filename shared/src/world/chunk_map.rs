use std::collections::{HashMap, HashSet};

use bevy::math::{bounding::Aabb3d, IVec3, Vec3};

use crate::world::{global_block_to_chunk_pos, to_local_pos, BlockData, BlockHitbox, Chunk};

/// The blocks of a world, on the server as on the client: the loaded chunks, and the global
/// positions of the blocks that changed since the last `take_changed_blocks`.
///
/// Positions named `pos` are global block positions, `chunk_pos` are chunk coordinates.
#[derive(Default, Clone, Debug)]
pub struct ChunkMap {
    chunks: HashMap<IVec3, Chunk>,
    changed: HashSet<IVec3>,
}

impl ChunkMap {
    /// Whether the chunk at `chunk_pos` is present in this map.
    pub fn is_chunk_loaded(&self, chunk_pos: IVec3) -> bool {
        self.chunks.contains_key(&chunk_pos)
    }

    /// Adds a chunk, replacing the one already at `chunk_pos`. This is not a block change.
    pub fn insert_chunk(&mut self, chunk_pos: IVec3, chunk: Chunk) {
        self.chunks.insert(chunk_pos, chunk);
    }

    pub fn chunk(&self, chunk_pos: IVec3) -> Option<&Chunk> {
        self.chunks.get(&chunk_pos)
    }

    /// The positions of the loaded chunks, in no particular order.
    pub fn chunk_positions(&self) -> impl Iterator<Item = IVec3> + '_ {
        self.chunks.keys().copied()
    }

    /// The block at `pos`: `None` for air and when its chunk is not loaded.
    pub fn get_block(&self, pos: IVec3) -> Option<&BlockData> {
        self.chunk(global_block_to_chunk_pos(&pos))?
            .get(to_local_pos(&pos))
    }

    /// Stores `block` at `pos`. Does nothing when the chunk of `pos` is not loaded: a block is
    /// never placed into unknown terrain. Storing the block already there is not a change.
    pub fn set_block(&mut self, pos: IVec3, block: BlockData) {
        let Some(chunk) = self.chunks.get_mut(&global_block_to_chunk_pos(&pos)) else {
            return;
        };
        let local = to_local_pos(&pos);
        if chunk.get(local) == Some(&block) {
            return;
        }
        chunk.set(local, block);
        self.changed.insert(pos);
    }

    /// Empties `pos` and returns the block that was there, if any.
    pub fn remove_block(&mut self, pos: IVec3) -> Option<BlockData> {
        let chunk = self.chunks.get_mut(&global_block_to_chunk_pos(&pos))?;
        let removed = chunk.remove(to_local_pos(&pos))?;
        self.changed.insert(pos);
        Some(removed)
    }

    /// Records `pos` as changed without changing it, so that whoever drains the log reports its
    /// current value. The server uses it when it refuses an interaction that a client may have
    /// predicted. Does nothing when the chunk of `pos` is not loaded.
    pub fn touch_block(&mut self, pos: IVec3) {
        if self.is_chunk_loaded(global_block_to_chunk_pos(&pos)) {
            self.changed.insert(pos);
        }
    }

    /// The positions of the blocks changed since the previous call, each once, sorted by
    /// `x`, then `y`, then `z`.
    pub fn take_changed_blocks(&mut self) -> Vec<IVec3> {
        let mut changed: Vec<IVec3> = self.changed.drain().collect();
        changed.sort_by_key(|pos| (pos.x, pos.y, pos.z));
        changed
    }

    /// Height of the highest block in the column of `position`, 0 for an empty column.
    pub fn get_height_ground(&self, position: Vec3) -> i32 {
        (0..256)
            .rev()
            .find(|&y| {
                self.get_block(IVec3::new(
                    position.x.floor() as i32,
                    y,
                    position.z.floor() as i32,
                ))
                .is_some()
            })
            .unwrap_or(0)
    }

    /// Whether `hitbox` overlaps the hitbox of a block.
    pub fn check_collision_box(&self, hitbox: &Aabb3d) -> bool {
        // Check all blocks inside the hitbox
        // Manual flooring is needed for negative coordinates
        for x in (hitbox.min.x.floor() as i32)..=(hitbox.max.x.floor() as i32) {
            for y in (hitbox.min.y.floor() as i32)..=(hitbox.max.y.floor() as i32) {
                for z in (hitbox.min.z.floor() as i32)..=(hitbox.max.z.floor() as i32) {
                    if let Some(block) = self.get_block(IVec3::new(x, y, z)) {
                        match block.id.get_hitbox() {
                            BlockHitbox::FullBlock => return true,
                            BlockHitbox::None => continue,
                            BlockHitbox::Aabb(block_hitbox) => {
                                let min = hitbox.min.max(block_hitbox.min);
                                let max = hitbox.max.min(block_hitbox.max);

                                if min == max.min(min) {
                                    return true;
                                }
                            }
                        }
                    }
                }
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{BlockDirection, BlockId};

    fn block(id: BlockId) -> BlockData {
        BlockData::new(id, BlockDirection::Front)
    }

    fn loaded(min: IVec3, max: IVec3) -> ChunkMap {
        ChunkMap::with_loaded_region(min, max)
    }

    #[test]
    fn blocks_round_trip_at_negative_coordinates() {
        let mut map = loaded(IVec3::splat(-17), IVec3::splat(-1));
        for pos in [
            IVec3::new(-1, -1, -1),
            IVec3::new(-16, -16, -16),
            IVec3::new(-17, -1, -16),
        ] {
            map.set_block(pos, block(BlockId::Stone));
            assert_eq!(map.get_block(pos), Some(&block(BlockId::Stone)), "{pos}");
        }
        // (-1, -1, -1) is the last cell of the chunk (-1, -1, -1)
        let chunk = map.chunk(IVec3::splat(-1)).unwrap();
        assert_eq!(chunk.get(IVec3::splat(15)), Some(&block(BlockId::Stone)));
        assert_eq!(
            map.remove_block(IVec3::splat(-1)),
            Some(block(BlockId::Stone))
        );
        assert_eq!(map.get_block(IVec3::splat(-1)), None);
    }

    #[test]
    fn neighbouring_blocks_across_a_chunk_border_live_in_different_chunks() {
        let mut map = loaded(IVec3::new(-1, 0, 0), IVec3::new(16, 0, 0));
        map.set_block(IVec3::new(15, 0, 0), block(BlockId::Dirt));
        map.set_block(IVec3::new(16, 0, 0), block(BlockId::Sand));
        map.set_block(IVec3::new(-1, 0, 0), block(BlockId::Glass));

        assert_eq!(
            map.get_block(IVec3::new(15, 0, 0)).unwrap().id,
            BlockId::Dirt
        );
        assert_eq!(
            map.get_block(IVec3::new(16, 0, 0)).unwrap().id,
            BlockId::Sand
        );
        assert_eq!(
            map.get_block(IVec3::new(-1, 0, 0)).unwrap().id,
            BlockId::Glass
        );
        assert_eq!(map.chunk(IVec3::ZERO).unwrap().iter().count(), 1);
        assert_eq!(map.chunk(IVec3::X).unwrap().iter().count(), 1);
        assert_eq!(
            map.chunk(IVec3::X).unwrap().get(IVec3::ZERO).unwrap().id,
            BlockId::Sand
        );
    }

    #[test]
    fn an_unloaded_chunk_reads_as_air_and_ignores_writes() {
        let mut map = loaded(IVec3::ZERO, IVec3::ZERO);
        let outside = IVec3::new(16, 0, 0);
        assert!(!map.is_chunk_loaded(IVec3::X));

        map.set_block(outside, block(BlockId::Stone));

        assert_eq!(map.get_block(outside), None);
        assert_eq!(map.remove_block(outside), None);
        assert!(!map.is_chunk_loaded(IVec3::X));
        assert_eq!(map.chunk_positions().count(), 1);
        assert!(map.take_changed_blocks().is_empty());
    }

    #[test]
    fn insert_chunk_loads_it_without_recording_a_change() {
        let mut map = ChunkMap::default();
        let mut chunk = Chunk::new();
        chunk.set(IVec3::new(1, 2, 3), block(BlockId::Stone));
        map.insert_chunk(IVec3::new(2, -1, 0), chunk);

        assert!(map.is_chunk_loaded(IVec3::new(2, -1, 0)));
        assert_eq!(
            map.get_block(IVec3::new(33, -14, 3)),
            Some(&block(BlockId::Stone))
        );
        assert!(map.take_changed_blocks().is_empty());
    }

    #[test]
    fn chunk_positions_lists_the_loaded_chunks() {
        let map = loaded(IVec3::new(-1, 0, 0), IVec3::new(16, 0, 0));
        let mut positions: Vec<IVec3> = map.chunk_positions().collect();
        positions.sort_by_key(|p| (p.x, p.y, p.z));
        assert_eq!(positions, vec![IVec3::new(-1, 0, 0), IVec3::ZERO, IVec3::X]);
    }

    #[test]
    fn set_and_remove_record_the_changed_position_once() {
        let mut map = loaded(IVec3::ZERO, IVec3::splat(15));
        let pos = IVec3::new(3, 4, 5);

        map.set_block(pos, block(BlockId::Stone));
        map.set_block(pos, block(BlockId::Dirt));
        map.remove_block(pos);

        assert_eq!(map.take_changed_blocks(), vec![pos]);
    }

    #[test]
    fn storing_the_same_value_is_not_a_change() {
        let mut map = loaded(IVec3::ZERO, IVec3::splat(15));
        let pos = IVec3::new(3, 4, 5);
        map.set_block(pos, block(BlockId::Stone));
        map.take_changed_blocks();

        map.set_block(pos, block(BlockId::Stone));
        assert!(map.take_changed_blocks().is_empty());

        // a different direction or breaking progress is a different value
        map.set_block(pos, BlockData::new(BlockId::Stone, BlockDirection::Back));
        assert_eq!(map.take_changed_blocks(), vec![pos]);
        map.set_block(
            pos,
            BlockData {
                breaking_progress: 1,
                ..BlockData::new(BlockId::Stone, BlockDirection::Back)
            },
        );
        assert_eq!(map.take_changed_blocks(), vec![pos]);
    }

    #[test]
    fn touching_a_block_records_it_without_changing_it() {
        let mut map = loaded(IVec3::ZERO, IVec3::splat(15));
        let pos = IVec3::new(3, 4, 5);
        map.set_block(pos, block(BlockId::Stone));
        map.take_changed_blocks();

        map.touch_block(pos);
        map.touch_block(pos);
        map.touch_block(IVec3::new(7, 7, 7));

        assert_eq!(map.take_changed_blocks(), vec![pos, IVec3::new(7, 7, 7)]);
        assert_eq!(map.get_block(pos), Some(&block(BlockId::Stone)));
        assert_eq!(map.get_block(IVec3::new(7, 7, 7)), None);
    }

    #[test]
    fn touching_an_unloaded_chunk_records_nothing() {
        let mut map = loaded(IVec3::ZERO, IVec3::ZERO);
        map.touch_block(IVec3::new(16, 0, 0));
        assert!(map.take_changed_blocks().is_empty());
    }

    #[test]
    fn removing_air_is_not_a_change() {
        let mut map = loaded(IVec3::ZERO, IVec3::splat(15));
        assert_eq!(map.remove_block(IVec3::new(1, 1, 1)), None);
        assert!(map.take_changed_blocks().is_empty());
    }

    #[test]
    fn take_changed_blocks_drains_and_sorts_by_position() {
        let mut map = loaded(IVec3::splat(-16), IVec3::splat(16));
        let positions = [
            IVec3::new(5, 0, 0),
            IVec3::new(-3, 9, 1),
            IVec3::new(5, -2, 7),
            IVec3::new(5, 0, -1),
            IVec3::new(-3, 9, 0),
        ];
        for pos in positions {
            map.set_block(pos, block(BlockId::Stone));
        }

        assert_eq!(
            map.take_changed_blocks(),
            vec![
                IVec3::new(-3, 9, 0),
                IVec3::new(-3, 9, 1),
                IVec3::new(5, -2, 7),
                IVec3::new(5, 0, -1),
                IVec3::new(5, 0, 0),
            ]
        );
        assert!(map.take_changed_blocks().is_empty());
    }

    #[test]
    fn height_ground_is_the_highest_block_of_the_column() {
        let mut map = loaded(IVec3::new(-20, 0, -20), IVec3::new(20, 100, 20));
        map.fill_box(
            IVec3::new(-3, 0, -3),
            IVec3::new(-3, 70, -3),
            BlockId::Stone,
        );
        map.fill_box(IVec3::new(4, 10, 4), IVec3::new(4, 12, 4), BlockId::Stone);

        assert_eq!(map.get_height_ground(Vec3::new(-2.5, 0.0, -2.5)), 70);
        assert_eq!(map.get_height_ground(Vec3::new(4.2, 0.0, 4.9)), 12);
        assert_eq!(map.get_height_ground(Vec3::new(8.0, 0.0, 8.0)), 0);
    }

    #[test]
    fn collision_box_hits_full_blocks_only_where_they_are() {
        let mut map = loaded(IVec3::splat(-16), IVec3::splat(16));
        map.fill_box(IVec3::new(-1, 0, 0), IVec3::new(-1, 0, 0), BlockId::Stone);

        let unit_box = |center: Vec3| Aabb3d::new(center, Vec3::splat(0.4));
        assert!(map.check_collision_box(&unit_box(Vec3::new(-0.5, 0.5, 0.5))));
        assert!(map.check_collision_box(&unit_box(Vec3::new(-0.05, 0.5, 0.5))));
        assert!(!map.check_collision_box(&unit_box(Vec3::new(1.5, 0.5, 0.5))));
        assert!(!map.check_collision_box(&unit_box(Vec3::new(-0.5, 2.5, 0.5))));
    }
}
