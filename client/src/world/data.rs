use bevy::prelude::*;
use shared::world::{BlockData, ChunkMap};
use std::hash::Hash;
use std::time::Instant;

use bevy::math::IVec3;
use bevy::prelude::Resource;
use std::collections::HashMap;

#[derive(Debug, Hash, Eq, PartialEq, Clone, Copy)]
pub enum GlobalMaterial {
    Sun,
    Moon,
    Blocks,
    Liquids,
    Items,
}

/// What the renderer keeps about a loaded chunk, next to its blocks.
#[derive(Clone, Debug)]
pub struct ChunkRender {
    pub entity: Option<Entity>,
    /// The entity of the liquid faces, apart because they are blended.
    pub liquid_entity: Option<Entity>,
    /// When the mesh on display was requested, or when the chunk arrived if none is yet.
    pub last_mesh_ts: Instant,
}

impl ChunkRender {
    pub fn new() -> Self {
        Self {
            entity: None,
            liquid_entity: None,
            last_mesh_ts: Instant::now(),
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
    /// Maps global chunk positions to the render bookkeeping of the chunk
    pub render: HashMap<IVec3, ChunkRender>,
}

impl ClientWorldMap {
    /// Forgets the chunk at `chunk_pos`: its blocks and its meshes, which are despawned. A mesh
    /// still being made for it is dropped when done, as the chunk has no render any more.
    pub fn unload_chunk(&mut self, chunk_pos: IVec3, commands: &mut Commands) {
        self.blocks.remove_chunk(chunk_pos);
        if let Some(render) = self.render.remove(&chunk_pos) {
            for entity in [render.entity, render.liquid_entity].into_iter().flatten() {
                commands.entity(entity).despawn();
            }
        }
    }
}

#[derive(Message, Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub enum WorldRenderRequestUpdateEvent {
    /// Mesh this chunk again, and only this one. Ignored when the chunk is not loaded.
    ChunkToReload(IVec3),
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
    fn an_unloaded_chunk_loses_its_blocks_render_and_entities_and_the_others_keep_theirs() {
        let mut world = World::new();
        let mut map = ClientWorldMap::default();
        let spawn_render = |world: &mut World, map: &mut ClientWorldMap, pos: IVec3| {
            map.blocks.insert_chunk(pos, Chunk::new());
            let render = ChunkRender {
                entity: Some(world.spawn_empty().id()),
                liquid_entity: Some(world.spawn_empty().id()),
                ..ChunkRender::new()
            };
            map.render.insert(pos, render.clone());
            render
        };
        let gone = spawn_render(&mut world, &mut map, IVec3::ZERO);
        let kept = spawn_render(&mut world, &mut map, IVec3::X);

        let mut queue = CommandQueue::default();
        let mut commands = Commands::new(&mut queue, &world);
        map.unload_chunk(IVec3::ZERO, &mut commands);
        // a chunk the client does not have is ignored
        map.unload_chunk(IVec3::new(7, 7, 7), &mut commands);
        queue.apply(&mut world);

        assert!(!map.blocks.is_chunk_loaded(IVec3::ZERO));
        assert!(!map.render.contains_key(&IVec3::ZERO));
        for entity in [gone.entity, gone.liquid_entity].into_iter().flatten() {
            assert!(world.get_entity(entity).is_err());
        }
        assert!(map.blocks.is_chunk_loaded(IVec3::X));
        assert!(map.render.contains_key(&IVec3::X));
        for entity in [kept.entity, kept.liquid_entity].into_iter().flatten() {
            assert!(world.get_entity(entity).is_ok());
        }
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
