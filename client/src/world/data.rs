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

use crate::player::ViewMode;

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

#[derive(Debug)]
pub struct RaycastResponse {
    pub block: BlockData,
    pub position: IVec3,
    pub face: IVec3,
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

    pub fn raycast(
        &self,
        camera_transform: &Transform,
        player_transform: &Transform,
        view_mode: ViewMode,
    ) -> Option<RaycastResponse> {
        match view_mode {
            ViewMode::FirstPerson => self.first_person_raycast(camera_transform),
            ViewMode::ThirdPerson => self.third_person_raycast(camera_transform, player_transform),
        }
    }

    fn first_person_raycast(&self, camera_transform: &Transform) -> Option<RaycastResponse> {
        let max_distance = 10.0; // Maximum distance for raycasting

        let camera_position = camera_transform.translation;
        let camera_rotation = camera_transform.rotation;

        let direction = camera_rotation
            .mul_vec3(Vec3::new(0.0, 0.0, -1.0))
            .normalize();

        let mut current_position = camera_position;

        let step = 0.1; // Step size for raycasting

        for _ in 0..(max_distance / step) as i32 {
            current_position += direction * step;
            let pos_ivec3 = IVec3::new(
                current_position.x.floor() as i32,
                current_position.y.floor() as i32,
                current_position.z.floor() as i32,
            );
            if let Some(block) = self.get_block_by_coordinates(&pos_ivec3) {
                // Now we need to determine which face of the block we hit
                let face = Vec3::new(
                    current_position.x - pos_ivec3.x as f32,
                    current_position.y - pos_ivec3.y as f32,
                    current_position.z - pos_ivec3.z as f32,
                );

                let mut block_face = IVec3::ZERO;

                if face.x.abs() > face.y.abs() && face.x.abs() > face.z.abs() {
                    block_face.x = if face.x > 0.0 { 1 } else { -1 };
                } else if face.y.abs() > face.x.abs() && face.y.abs() > face.z.abs() {
                    block_face.y = if face.y > 0.0 { 1 } else { -1 };
                } else {
                    block_face.z = if face.z > 0.0 { 1 } else { -1 };
                }

                return Some(RaycastResponse {
                    block: *block,
                    position: pos_ivec3,
                    face: block_face,
                });
            }
        }

        None
    }

    fn third_person_raycast(
        &self,
        camera_transform: &Transform,
        player_transform: &Transform,
    ) -> Option<RaycastResponse> {
        // the raycast is done from the camera position to the player position

        let max_distance = 10.0; // Maximum distance for raycasting

        let player_position = player_transform.translation;

        let camera_rotation = camera_transform.rotation;

        let camera_direction = camera_rotation
            .mul_vec3(Vec3::new(0.0, 0.0, -1.0))
            .normalize();

        let direction = camera_direction;

        let mut current_position = player_position;

        let step = 0.1; // Step size for raycasting

        for _ in 0..(max_distance / step) as i32 {
            current_position += direction * step;
            let pos_ivec3 = IVec3::new(
                current_position.x.floor() as i32,
                current_position.y.floor() as i32,
                current_position.z.floor() as i32,
            );
            if let Some(block) = self.get_block_by_coordinates(&pos_ivec3) {
                // Now we need to determine which face of the block we hit
                let face = Vec3::new(
                    current_position.x - pos_ivec3.x as f32,
                    current_position.y - pos_ivec3.y as f32,
                    current_position.z - pos_ivec3.z as f32,
                );

                let mut block_face = IVec3::ZERO;

                if face.x.abs() > face.y.abs() && face.x.abs() > face.z.abs() {
                    block_face.x = if face.x > 0.0 { 1 } else { -1 };
                } else if face.y.abs() > face.x.abs() && face.y.abs() > face.z.abs() {
                    block_face.y = if face.y > 0.0 { 1 } else { -1 };
                } else {
                    block_face.z = if face.z > 0.0 { 1 } else { -1 };
                }

                return Some(RaycastResponse {
                    block: *block,
                    position: pos_ivec3,
                    face: block_face,
                });
            }
        }

        None
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
