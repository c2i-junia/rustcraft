use bevy_platform::collections::{HashMap, HashSet};

use bevy::math::{bounding::Aabb3d, IVec3, Vec3};

use crate::world::{
    global_block_to_chunk_pos, to_local_pos, BlockData, BlockHitbox, BlockId, BlockTransparency,
    Chunk, Container,
};

/// The blocks of a world, on the server as on the client: the loaded chunks, and the global
/// positions of the blocks that changed since the last `take_changed_blocks`.
///
/// It also holds the contents of the chests, by position. A container only exists where a chest
/// stands: replacing or removing the chest, or removing its chunk, drops it. The server holds
/// the contents of every chest, a client only those of the chest its player has open.
///
/// Positions named `pos` are global block positions, `chunk_pos` are chunk coordinates.
#[derive(Default, Clone, Debug)]
pub struct ChunkMap {
    chunks: HashMap<IVec3, Chunk>,
    changed: HashSet<IVec3>,
    containers: Held<Container>,
}

/// What blocks of one kind hold, by position, like the contents of the chests, and the
/// positions whose value changed, was added or was dropped since the last `take_changed`.
#[derive(Clone, Debug)]
struct Held<T> {
    values: HashMap<IVec3, T>,
    changed: HashSet<IVec3>,
}

impl<T> Default for Held<T> {
    fn default() -> Self {
        Self {
            values: HashMap::default(),
            changed: HashSet::default(),
        }
    }
}

impl<T> Held<T> {
    /// The value at `pos`, to change: the position is recorded as changed.
    fn get_mut(&mut self, pos: IVec3) -> Option<&mut T> {
        let value = self.values.get_mut(&pos)?;
        self.changed.insert(pos);
        Some(value)
    }

    /// Removes the value at `pos` and returns it; the position is recorded if there was one.
    fn take(&mut self, pos: IVec3) -> Option<T> {
        let value = self.values.remove(&pos)?;
        self.changed.insert(pos);
        Some(value)
    }

    fn of_chunk(&self, chunk_pos: IVec3) -> impl Iterator<Item = (IVec3, &T)> + '_ {
        self.values
            .iter()
            .filter(move |(pos, _)| global_block_to_chunk_pos(pos) == chunk_pos)
            .map(|(pos, value)| (*pos, value))
    }

    fn take_changed(&mut self) -> Vec<IVec3> {
        self.changed.drain().collect()
    }

    /// Forgets the values of the chunk at `chunk_pos`, and their changes.
    fn remove_chunk(&mut self, chunk_pos: IVec3) {
        let outside = |pos: &IVec3| global_block_to_chunk_pos(pos) != chunk_pos;
        self.values.retain(|pos, _| outside(pos));
        self.changed.retain(outside);
    }
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

    /// Removes the chunk at `chunk_pos`, and the changes logged for its blocks: nobody is to
    /// hear about the blocks of a chunk that is gone. This is not a block change.
    pub fn remove_chunk(&mut self, chunk_pos: IVec3) -> Option<Chunk> {
        let chunk = self.chunks.remove(&chunk_pos)?;
        let outside = |pos: &IVec3| global_block_to_chunk_pos(pos) != chunk_pos;
        self.changed.retain(outside);
        self.containers.remove_chunk(chunk_pos);
        Some(chunk)
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
        if block.id != BlockId::Chest {
            self.containers.take(pos);
        }
    }

    /// Empties `pos` and returns the block that was there, if any. The contents of a chest
    /// there are dropped: take them first with `take_container`.
    pub fn remove_block(&mut self, pos: IVec3) -> Option<BlockData> {
        let chunk = self.chunks.get_mut(&global_block_to_chunk_pos(&pos))?;
        let removed = chunk.remove(to_local_pos(&pos))?;
        self.changed.insert(pos);
        self.containers.take(pos);
        Some(removed)
    }

    /// The contents of the chest at `pos`, if this map holds them.
    pub fn container(&self, pos: IVec3) -> Option<&Container> {
        self.containers.values.get(&pos)
    }

    /// The contents of the chest at `pos`, to change: the chest is recorded as changed.
    pub fn container_mut(&mut self, pos: IVec3) -> Option<&mut Container> {
        self.containers.get_mut(pos)
    }

    /// Stores the contents of the chest at `pos`, replacing what this map held for it. Does
    /// nothing unless a chest stands at `pos`. This is not a change: it is how contents are
    /// loaded, and how a client takes those the server sends.
    pub fn insert_container(&mut self, pos: IVec3, container: Container) {
        if self
            .get_block(pos)
            .is_some_and(|block| block.id == BlockId::Chest)
        {
            self.containers.values.insert(pos, container);
        }
    }

    /// Removes the contents of the chest at `pos` from this map and returns them; the chest is
    /// recorded as changed if this map held them.
    pub fn take_container(&mut self, pos: IVec3) -> Option<Container> {
        self.containers.take(pos)
    }

    /// The contents of the chests of the chunk at `chunk_pos`, with their global position.
    pub fn containers_of_chunk(
        &self,
        chunk_pos: IVec3,
    ) -> impl Iterator<Item = (IVec3, &Container)> + '_ {
        self.containers.of_chunk(chunk_pos)
    }

    /// The positions of the chests whose contents changed, were added or were dropped since the
    /// previous call, each once, in no order.
    pub fn take_changed_containers(&mut self) -> Vec<IVec3> {
        self.containers.take_changed()
    }

    /// Records `pos` as changed without changing it, so that whoever drains the log reports its
    /// current value. The server uses it when it refuses an interaction that a client may have
    /// predicted. Does nothing when the chunk of `pos` is not loaded.
    pub fn touch_block(&mut self, pos: IVec3) {
        if self.is_chunk_loaded(global_block_to_chunk_pos(&pos)) {
            self.changed.insert(pos);
        }
    }

    /// The positions of the blocks changed since the last `take_changed_blocks`, in no order,
    /// left in the log.
    pub fn changed_blocks(&self) -> impl Iterator<Item = IVec3> + '_ {
        self.changed.iter().copied()
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
        cells_overlapped(hitbox).any(|pos| {
            let Some(block) = self.get_block(pos) else {
                return false;
            };
            match block.get_hitbox() {
                BlockHitbox::FullBlock => true,
                BlockHitbox::None => false,
                BlockHitbox::Aabb(block_hitbox) => {
                    // The block's box is relative to its cell
                    let cell = pos.as_vec3a();
                    let min = hitbox.min.max(block_hitbox.min + cell);
                    let max = hitbox.max.min(block_hitbox.max + cell);

                    // The two boxes overlap when their intersection is not empty
                    min.cmple(max).all()
                }
            }
        })
    }

    /// Whether `hitbox` overlaps a cell that holds a liquid.
    pub fn touches_liquid(&self, hitbox: &Aabb3d) -> bool {
        cells_overlapped(hitbox).any(|pos| {
            self.get_block(pos)
                .is_some_and(|block| block.id.get_visibility() == BlockTransparency::Liquid)
        })
    }
}

/// The cells that `hitbox` overlaps.
fn cells_overlapped(hitbox: &Aabb3d) -> impl Iterator<Item = IVec3> {
    // Manual flooring is needed for negative coordinates
    let min = hitbox.min.floor().as_ivec3();
    let max = hitbox.max.floor().as_ivec3();
    (min.x..=max.x).flat_map(move |x| {
        (min.y..=max.y).flat_map(move |y| (min.z..=max.z).map(move |z| IVec3::new(x, y, z)))
    })
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
    fn remove_chunk_drops_it_and_the_changes_of_its_blocks_only() {
        let mut map = loaded(IVec3::ZERO, IVec3::new(31, 15, 15));
        let kept = IVec3::new(20, 1, 1);
        map.set_block(IVec3::new(3, 4, 5), block(BlockId::Stone));
        map.set_block(kept, block(BlockId::Dirt));

        assert!(map.remove_chunk(IVec3::ZERO).is_some());

        assert!(!map.is_chunk_loaded(IVec3::ZERO));
        assert!(map.is_chunk_loaded(IVec3::X));
        assert_eq!(map.get_block(kept), Some(&block(BlockId::Dirt)));
        assert_eq!(map.take_changed_blocks(), vec![kept]);
        assert!(map.remove_chunk(IVec3::ZERO).is_none());
    }

    fn holding(nb: u32) -> Container {
        let mut container = Container::default();
        container.slots[0] = Some(crate::world::ItemStack {
            item_id: crate::world::ItemId::Dirt,
            item_type: crate::world::ItemType::Block(BlockId::Dirt),
            nb,
        });
        container
    }

    /// A map with a chest at `pos` holding 3 dirt, and no change recorded.
    fn chest_at(pos: IVec3) -> ChunkMap {
        let mut map = loaded(IVec3::ZERO, IVec3::new(31, 15, 15));
        map.set_block(pos, block(BlockId::Chest));
        map.insert_container(pos, holding(3));
        map.take_changed_blocks();
        map
    }

    #[test]
    fn contents_are_only_stored_where_a_chest_stands() {
        let mut map = chest_at(IVec3::new(1, 2, 3));
        map.set_block(IVec3::new(4, 4, 4), block(BlockId::Stone));

        map.insert_container(IVec3::new(4, 4, 4), holding(1));
        map.insert_container(IVec3::new(5, 5, 5), holding(1));

        assert_eq!(map.container(IVec3::new(1, 2, 3)), Some(&holding(3)));
        assert_eq!(map.container(IVec3::new(4, 4, 4)), None);
        assert_eq!(map.container(IVec3::new(5, 5, 5)), None);
        assert!(map.take_changed_containers().is_empty());
    }

    #[test]
    fn replacing_or_removing_a_chest_drops_its_contents() {
        let pos = IVec3::new(1, 2, 3);
        let mut map = chest_at(pos);
        map.set_block(pos, block(BlockId::Stone));
        assert_eq!(map.container(pos), None);
        assert_eq!(map.take_changed_containers(), vec![pos]);

        let mut map = chest_at(pos);
        map.remove_block(pos);
        assert_eq!(map.container(pos), None);
        assert_eq!(map.take_changed_containers(), vec![pos]);
    }

    #[test]
    fn a_breaking_chest_keeps_its_contents() {
        let pos = IVec3::new(1, 2, 3);
        let mut map = chest_at(pos);
        map.set_block(
            pos,
            BlockData {
                breaking_progress: 4,
                ..block(BlockId::Chest)
            },
        );
        assert_eq!(map.container(pos), Some(&holding(3)));
        assert!(map.take_changed_containers().is_empty());
    }

    #[test]
    fn changing_or_taking_contents_records_the_chest() {
        let pos = IVec3::new(1, 2, 3);
        let mut map = chest_at(pos);
        map.container_mut(pos).unwrap().slots[1] = holding(5).slots[0];
        assert_eq!(map.take_changed_containers(), vec![pos]);
        assert!(map.take_changed_containers().is_empty());

        let mut expected = holding(3);
        expected.slots[1] = holding(5).slots[0];
        assert_eq!(map.take_container(pos), Some(expected));
        assert_eq!(map.container(pos), None);
        assert_eq!(map.take_changed_containers(), vec![pos]);
        assert!(map.take_changed_blocks().is_empty());
    }

    #[test]
    fn removing_a_chunk_drops_its_contents_only() {
        let (dropped, kept) = (IVec3::new(1, 2, 3), IVec3::new(20, 2, 3));
        let mut map = chest_at(dropped);
        map.set_block(kept, block(BlockId::Chest));
        map.insert_container(kept, holding(7));
        map.container_mut(dropped);

        map.remove_chunk(IVec3::ZERO);

        assert_eq!(map.container(dropped), None);
        assert_eq!(map.container(kept), Some(&holding(7)));
        assert!(map.take_changed_containers().is_empty());
        let listed: Vec<IVec3> = map.containers_of_chunk(IVec3::X).map(|(p, _)| p).collect();
        assert_eq!(listed, vec![kept]);
        assert_eq!(map.containers_of_chunk(IVec3::ZERO).count(), 0);
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

    #[test]
    fn a_cocoa_pod_collides_only_through_its_box() {
        let pod = IVec3::new(2, 3, 4);
        let centre = pod.as_vec3() + 0.5;
        let small_box = |center: Vec3| Aabb3d::new(center, Vec3::splat(0.05));
        for direction in [
            BlockDirection::Front,
            BlockDirection::Right,
            BlockDirection::Back,
            BlockDirection::Left,
        ] {
            let mut map = loaded(IVec3::ZERO, IVec3::splat(15));
            map.set_block(pod, BlockData::new(BlockId::Cocoa, direction));
            let support = direction.offset().as_vec3();
            // the pod hangs on the side of its support, the other half of the cell is empty
            assert!(
                map.check_collision_box(&small_box(centre + support * 0.25)),
                "{direction:?}"
            );
            assert!(
                !map.check_collision_box(&small_box(centre - support * 0.4)),
                "{direction:?}"
            );
        }
    }
}
