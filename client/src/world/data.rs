use bevy::prelude::*;
use shared::world::ChunkMap;
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
    Items,
}

/// What the renderer keeps about a loaded chunk, next to its blocks.
#[derive(Clone, Debug)]
pub struct ChunkRender {
    pub entity: Option<Entity>,
    /// When the mesh on display was requested, or when the chunk arrived if none is yet.
    pub last_mesh_ts: Instant,
}

impl ChunkRender {
    pub fn new() -> Self {
        Self {
            entity: None,
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

#[derive(Message, Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub enum WorldRenderRequestUpdateEvent {
    /// Mesh this chunk again, and only this one. Ignored when the chunk is not loaded.
    ChunkToReload(IVec3),
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

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
