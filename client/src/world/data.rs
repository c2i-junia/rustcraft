use bevy::prelude::*;
use shared::world::{BlockData, ChunkMap};
use std::hash::Hash;
use std::time::Instant;

use bevy::math::IVec3;
use bevy::prelude::Resource;
use shared::CHUNK_SIZE;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Hash, Eq, PartialEq, Clone, Copy)]
pub enum GlobalMaterial {
    Sun,
    Moon,
    Blocks,
    Liquids,
    Items,
}

/// How many columns of chunks wide a group is, along x and z: the chunks of a group are drawn
/// as one entity, or two with their liquids. A frame costs the CPU for each entity drawn, far
/// more than for each face, and a large render distance shows tens of thousands of chunks.
pub const GROUP_WIDTH: i32 = 4;

/// The group of the chunk at `chunk_pos`.
pub fn group_of(chunk_pos: IVec3) -> IVec2 {
    chunk_pos.xz().div_euclid(IVec2::splat(GROUP_WIDTH))
}

/// The block where the group `group` starts, from which its vertices are placed.
pub fn group_origin(group: IVec2) -> IVec3 {
    IVec3::new(group.x, 0, group.y) * GROUP_WIDTH * CHUNK_SIZE
}

/// What the renderer keeps about a group of chunks (`group_of`) of which some are loaded.
#[derive(Clone, Debug)]
pub struct ChunkRender {
    /// The loaded chunks of the group.
    pub chunks: HashSet<IVec3>,
    pub entity: Option<Entity>,
    /// The entity of the liquid faces, apart because they are blended.
    pub liquid_entity: Option<Entity>,
    /// When the mesh on display was requested, or when the chunk arrived if none is yet.
    pub last_mesh_ts: Instant,
    /// The triangles of the solid and the liquid meshes on display, for the benchmarks: the
    /// data of a mesh leaves the main world once on the GPU.
    #[cfg(test)]
    pub triangles: [usize; 2],
}

impl ChunkRender {
    pub fn new() -> Self {
        Self {
            chunks: HashSet::new(),
            entity: None,
            liquid_entity: None,
            last_mesh_ts: Instant::now(),
            #[cfg(test)]
            triangles: [0; 2],
        }
    }

    /// Whether a mesh requested at `request_ts` is more recent than what is displayed, and so
    /// may replace it. A slow task that finishes after a later one must not undo it.
    pub fn is_newer_than_displayed(&self, request_ts: Instant) -> bool {
        request_ts > self.last_mesh_ts
    }
}

#[derive(Resource, Default)]
pub struct ClientWorldMap {
    pub blocks: ChunkMap,
    /// The render bookkeeping of each group of chunks (`group_of`) of which some are loaded
    pub render: HashMap<IVec2, ChunkRender>,
}

impl ClientWorldMap {
    /// Notes that the chunk at `chunk_pos` arrived: meshes of its group requested before are
    /// outdated.
    pub fn chunk_arrived(&mut self, chunk_pos: IVec3) {
        let render = self
            .render
            .entry(group_of(chunk_pos))
            .and_modify(|render| render.last_mesh_ts = Instant::now())
            .or_insert_with(ChunkRender::new);
        render.chunks.insert(chunk_pos);
    }

    /// Forgets the chunk at `chunk_pos`: its blocks, and the meshes of its group once it was the
    /// last of the group, which are despawned. A mesh still being made for the group is dropped
    /// when done then, as the group has no render any more. Otherwise the group is to be meshed
    /// again.
    pub fn unload_chunk(&mut self, chunk_pos: IVec3, commands: &mut Commands) {
        self.blocks.remove_chunk(chunk_pos);
        let group = group_of(chunk_pos);
        let Some(render) = self.render.get_mut(&group) else {
            return;
        };
        render.chunks.remove(&chunk_pos);
        if render.chunks.is_empty() {
            for entity in [render.entity, render.liquid_entity].into_iter().flatten() {
                commands.entity(entity).despawn();
            }
            self.render.remove(&group);
        }
    }
}

#[derive(Message, Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub enum WorldRenderRequestUpdateEvent {
    /// Mesh this chunk again, and only this one. Ignored when the chunk is not loaded.
    ChunkToReload(IVec3),
}

/// A chunk came into `ClientWorldMap` from the server, or was unloaded from it.
#[derive(Message, Debug, Copy, Clone, PartialEq, Eq)]
pub enum ChunkLoadEvent {
    Loaded(IVec3),
    Unloaded(IVec3),
}

/// A block of `ClientWorldMap` changed: the local player predicted it, or the server sent it.
#[derive(Message, Debug, Copy, Clone, PartialEq)]
pub struct BlockChanged {
    pub position: IVec3,
    pub before: Option<BlockData>,
    pub after: Option<BlockData>,
    pub by_local_player: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::world::CommandQueue;
    use shared::world::Chunk;
    use std::time::Duration;

    #[test]
    fn an_unloaded_chunk_leaves_its_group_and_the_last_one_takes_the_entities_with_it() {
        let mut world = World::new();
        let mut map = ClientWorldMap::default();
        let first = IVec3::ZERO;
        let second = IVec3::new(GROUP_WIDTH - 1, 3, GROUP_WIDTH - 1);
        let other = IVec3::X * GROUP_WIDTH;
        for pos in [first, second, other] {
            map.blocks.insert_chunk(pos, Chunk::new());
            map.chunk_arrived(pos);
        }
        assert_eq!(map.render.len(), 2);
        let mut entities = |map: &mut ClientWorldMap, pos| {
            let render = map.render.get_mut(&group_of(pos)).unwrap();
            render.entity = Some(world.spawn_empty().id());
            render.liquid_entity = Some(world.spawn_empty().id());
            [render.entity, render.liquid_entity]
        };
        let group = entities(&mut map, first);
        let kept = entities(&mut map, other);

        let mut queue = CommandQueue::default();
        let mut commands = Commands::new(&mut queue, &world);
        map.unload_chunk(first, &mut commands);
        // a chunk the client does not have is ignored
        map.unload_chunk(IVec3::new(7, 7, 7), &mut commands);
        queue.apply(&mut world);
        assert!(!map.blocks.is_chunk_loaded(first));
        assert_eq!(map.render[&group_of(first)].chunks, HashSet::from([second]));
        for entity in group.into_iter().flatten() {
            assert!(world.get_entity(entity).is_ok());
        }

        let mut commands = Commands::new(&mut queue, &world);
        map.unload_chunk(second, &mut commands);
        queue.apply(&mut world);
        assert!(!map.render.contains_key(&group_of(first)));
        for entity in group.into_iter().flatten() {
            assert!(world.get_entity(entity).is_err());
        }
        assert!(map.blocks.is_chunk_loaded(other));
        for entity in kept.into_iter().flatten() {
            assert!(world.get_entity(entity).is_ok());
        }
    }

    #[test]
    fn the_chunks_of_a_group_lie_side_by_side_at_every_height() {
        assert_eq!(group_of(IVec3::new(0, -5, GROUP_WIDTH - 1)), IVec2::ZERO);
        assert_eq!(group_of(IVec3::new(-1, 40, 0)), IVec2::new(-1, 0));
        assert_eq!(
            group_of(IVec3::new(GROUP_WIDTH, 0, -GROUP_WIDTH)),
            IVec2::new(1, -1)
        );
        assert_eq!(
            group_origin(IVec2::new(1, -1)),
            IVec3::new(1, 0, -1) * GROUP_WIDTH * CHUNK_SIZE
        );
    }

    #[test]
    fn only_a_mesh_requested_after_the_displayed_one_may_replace_it() {
        let mut render = ChunkRender::new();
        let first = render.last_mesh_ts + Duration::from_millis(10);
        let second = first + Duration::from_millis(10);

        assert!(render.is_newer_than_displayed(first));
        assert!(render.is_newer_than_displayed(second));

        // the later request finishes first and is displayed
        render.last_mesh_ts = second;
        assert!(!render.is_newer_than_displayed(first));
        assert!(!render.is_newer_than_displayed(second));
        assert!(render.is_newer_than_displayed(second + Duration::from_millis(1)));
    }

    #[test]
    fn a_mesh_requested_before_the_chunk_arrived_is_outdated() {
        let before = Instant::now();
        let render = ChunkRender::new();
        assert!(!render.is_newer_than_displayed(before));
    }
}
