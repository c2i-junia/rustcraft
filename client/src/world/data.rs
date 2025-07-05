use bevy::math::bounding::Aabb3d;
use bevy::prelude::*;
use shared::world::BlockData;
use shared::world::WorldMap;
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
    Liquids,
    Items,
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

    fn check_collision_box(&self, hitbox: &Aabb3d) -> bool {
        // Check all blocks inside the hitbox
        for x in (hitbox.min.x.round() as i32)..=(hitbox.max.x.round() as i32) {
            for y in (hitbox.min.y.round() as i32)..=(hitbox.max.y.round() as i32) {
                for z in (hitbox.min.z.round() as i32)..=(hitbox.max.z.round() as i32) {
                    if let Some(block) = self.get_block_by_coordinates(&IVec3::new(x, y, z)) {
                        if block.id.has_hitbox() {
                            return true;
                        }
                    }
                }
            }
        }
        false
    }

    fn check_collision_point(&self, point: &Vec3) -> bool {
        if let Some(block) = self.get_block_by_coordinates(&IVec3::new(
            point.x.round() as i32,
            point.y.round() as i32,
            point.z.round() as i32,
        )) {
            block.id.has_hitbox()
        } else {
            false
        }
    }

    fn get_surrounding_chunks(&self, position: Vec3, radius: i32) -> Vec<IVec3> {
        let mut chunks: Vec<IVec3> = Vec::new();
        let x: i32 = position.x.round() as i32;
        let y: i32 = position.y.round() as i32;
        let z: i32 = position.z.round() as i32;
        let cx: i32 = block_to_chunk_coord(x);
        let cy: i32 = block_to_chunk_coord(y);
        let cz: i32 = block_to_chunk_coord(z);
        for dx in -radius..=radius {
            for dy in -radius..=radius {
                for dz in -radius..=radius {
                    chunks.push(IVec3::new(cx + dx, cy + dy, cz + dz));
                }
            }
        }
        chunks
    }

    fn get_heigh_ground(&self, position: Vec3) -> i32 {
        for y in (0..256).rev() {
            if self
                .get_block_by_coordinates(&IVec3::new(position.x as i32, y, position.z as i32))
                .is_some()
            {
                return y;
            }
        }
        0
    }
}

impl ClientWorldMap {
    pub fn try_to_break_block(&mut self, position: &IVec3) -> Option<(BlockData, bool)> {
        let block: &BlockData = self.get_block_by_coordinates(position)?;
        let kind: BlockData = *block;

        let chunk_pos: IVec3 = global_block_to_chunk_pos(position);

        let chunk_map: &mut ClientChunk =
            self.map
                .get_mut(&IVec3::new(chunk_pos.x, chunk_pos.y, chunk_pos.z))?;

        let local_block_pos: IVec3 = to_local_pos(position);

        let mut block = chunk_map.map.get_mut(&local_block_pos);

        let data = block.take()?;

        data.breaking_progress += 1;

        // info!("Block breaking progress: {}", data.breaking_progress);
        if kind.id.get_break_time() != 100 && data.breaking_progress >= 6 * kind.id.get_break_time()
        {
            chunk_map.map.remove(&local_block_pos);
            return Some((kind, true));
        }
        Some((kind, false))
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
