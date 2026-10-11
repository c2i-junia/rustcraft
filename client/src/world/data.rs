use bevy::prelude::*;
use shared::world::ChunkMap;
use std::collections::HashSet;
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
    pub last_mesh_ts: Instant, // When was the last time a mesh was created for this chunk ?
}

impl ChunkRender {
    pub fn new() -> Self {
        Self {
            entity: None,
            last_mesh_ts: Instant::now(),
        }
    }
}

#[derive(Resource, Default)]
pub struct ClientWorldMap {
    pub name: String,
    pub blocks: ChunkMap,
    /// Maps global chunk positions to the render bookkeeping of the chunk
    pub render: HashMap<IVec3, ChunkRender>,
}

#[derive(Default, Debug)]
pub struct QueuedEvents {
    pub events: HashSet<WorldRenderRequestUpdateEvent>, // Set of events for rendering updates
}

#[derive(Message, Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub enum WorldRenderRequestUpdateEvent {
    ChunkToReload(IVec3),
}
