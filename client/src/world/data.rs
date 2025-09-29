use bevy::prelude::*;
use shared::world::BlockData;
use shared::world::WorldMap;
use std::collections::HashSet;
use std::hash::Hash;
use std::time::Instant;

use bevy::math::IVec3;
use bevy::prelude::Resource;
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
    Items,
}

#[derive(Clone, Debug)]
pub struct ClientChunk {
    pub map: HashMap<IVec3, BlockData>, // Maps block positions within a chunk to block IDs
    pub entity: Option<Entity>,
    pub last_mesh_ts: Instant, // When was the last time a mesh was created for this chunk ?
}

impl Default for ClientChunk {
    fn default() -> Self {
        Self {
            map: HashMap::new(),
            entity: None,
            last_mesh_ts: Instant::now(),
        }
    }
}

#[derive(Resource, Default, Clone)]
pub struct ClientWorldMap {
    pub name: String,
    pub map: HashMap<IVec3, crate::world::ClientChunk>, // Maps global chunk positions to chunks
    pub total_blocks_count: u64,
    pub total_chunks_count: u64,
}

impl WorldMap for ClientWorldMap {
    fn get_block_by_coordinates(&self, position: &IVec3) -> Option<&BlockData> {
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

    fn get_block_mut_by_coordinates(&mut self, position: &IVec3) -> Option<&mut BlockData> {
        let x: i32 = position.x;
        let y: i32 = position.y;
        let z: i32 = position.z;
        let cx: i32 = block_to_chunk_coord(x);
        let cy: i32 = block_to_chunk_coord(y);
        let cz: i32 = block_to_chunk_coord(z);
        let chunk = self.map.get_mut(&IVec3::new(cx, cy, cz));
        match chunk {
            Some(chunk) => {
                let sub_x: i32 = ((x % CHUNK_SIZE) + CHUNK_SIZE) % CHUNK_SIZE;
                let sub_y: i32 = ((y % CHUNK_SIZE) + CHUNK_SIZE) % CHUNK_SIZE;
                let sub_z: i32 = ((z % CHUNK_SIZE) + CHUNK_SIZE) % CHUNK_SIZE;
                chunk.map.get_mut(&IVec3::new(sub_x, sub_y, sub_z))
            }
            None => None,
        }
    }

    fn remove_block_by_coordinates(&mut self, global_block_pos: &IVec3) -> Option<BlockData> {
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

    fn set_block(&mut self, position: &IVec3, block: BlockData) {
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

    fn mark_block_for_update(&mut self, _block_pos: &IVec3) {
        // Useless in client
    }
}

#[derive(Default, Debug)]
pub struct QueuedEvents {
    pub events: HashSet<WorldRenderRequestUpdateEvent>, // Set of events for rendering updates
}

#[derive(Event, Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub enum WorldRenderRequestUpdateEvent {
    ChunkToReload(IVec3),
}
