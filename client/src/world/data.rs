use bevy::prelude::*;
use shared::world::BlockData;
use std::collections::HashSet;
use std::hash::Hash;

use bevy::math::IVec3;
use bevy::prelude::Resource;
use serde::{Deserialize, Serialize};
use shared::world::block_to_chunk_coord;
use shared::world::global_block_to_chunk_pos;
use shared::world::to_local_pos;
use shared::CHUNK_SIZE;
use std::collections::HashMap;

#[derive(Debug, Hash, Eq, PartialEq, Clone, Copy)]
pub enum GlobalMaterial {
    Sun,
    Moon,
    Blocks,
}

#[derive(Clone, Default, Serialize, Deserialize, Debug)]
pub struct ClientChunk {
    pub map: HashMap<IVec3, BlockData>, // Maps block positions within a chunk to block IDs
    #[serde(skip)]
    pub entity: Option<Entity>,
}

#[derive(Resource, Default, Clone, Serialize, Deserialize)]
pub struct ClientWorldMap {
    pub name: String,
    pub map: HashMap<IVec3, crate::world::ClientChunk>, // Maps global chunk positions to chunks
    pub total_blocks_count: u64,
    pub total_chunks_count: u64,
}

impl ClientWorldMap {
    pub fn get_block_by_coordinates(&self, position: &IVec3) -> Option<&BlockData> {
        let x: i32 = position.x;
        let y: i32 = position.y;
        let z: i32 = position.z;
        let cx: i32 = block_to_chunk_coord(x);
        let cy: i32 = block_to_chunk_coord(y);
        let cz: i32 = block_to_chunk_coord(z);
        let chunk: Option<&ClientChunk> = self.map.get(&IVec3::new(cx, cy, cz));
        match chunk {
            Some(chunk) => {
                let sub_x: i32 = ((x % CHUNK_SIZE) + CHUNK_SIZE) % CHUNK_SIZE;
                let sub_y: i32 = ((y % CHUNK_SIZE) + CHUNK_SIZE) % CHUNK_SIZE;
                let sub_z: i32 = ((z % CHUNK_SIZE) + CHUNK_SIZE) % CHUNK_SIZE;
                chunk.map.get(&IVec3::new(sub_x, sub_y, sub_z))
            }
            None => None,
        }
    }

    pub fn remove_block_by_coordinates(&mut self, global_block_pos: &IVec3) -> Option<BlockData> {
        let block: &BlockData = self.get_block_by_coordinates(global_block_pos)?;
        let kind: BlockData = *block;

        let chunk_pos: IVec3 = global_block_to_chunk_pos(global_block_pos);

        let chunk_map: &mut ClientChunk =
            self.map
                .get_mut(&IVec3::new(chunk_pos.x, chunk_pos.y, chunk_pos.z))?;

        let local_block_pos: IVec3 = to_local_pos(global_block_pos);

        chunk_map.map.remove(&local_block_pos);

        Some(kind)
    }

    pub fn set_block(&mut self, position: &IVec3, block: BlockData) {
        let x: i32 = position.x;
        let y: i32 = position.y;
        let z: i32 = position.z;
        let cx: i32 = block_to_chunk_coord(x);
        let cy: i32 = block_to_chunk_coord(y);
        let cz: i32 = block_to_chunk_coord(z);
        let chunk: &mut ClientChunk = self.map.entry(IVec3::new(cx, cy, cz)).or_default();
        let sub_x: i32 = ((x % CHUNK_SIZE) + CHUNK_SIZE) % CHUNK_SIZE;
        let sub_y: i32 = ((y % CHUNK_SIZE) + CHUNK_SIZE) % CHUNK_SIZE;
        let sub_z: i32 = ((z % CHUNK_SIZE) + CHUNK_SIZE) % CHUNK_SIZE;

        chunk.map.insert(IVec3::new(sub_x, sub_y, sub_z), block);
    }
}

#[derive(Default, Debug)]
pub struct QueuedEvents {
    pub events: HashSet<WorldRenderRequestUpdateEvent>, // Set of events for rendering updates
}

#[derive(Event, Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub enum WorldRenderRequestUpdateEvent {
    ChunkToReload(IVec3),
    BlockToReload(IVec3),
}
