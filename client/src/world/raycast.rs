use crate::player::ViewMode;

use super::ClientWorldMap;
use bevy::prelude::*;
use shared::world::BlockData;

#[derive(Debug, Clone, Copy)]
pub struct RaycastResponse {
    pub block: BlockData,
    pub position: IVec3,
    pub face: IVec3,
}

pub fn raycast(
    world_map: &ClientWorldMap,
    camera_transform: &Transform,
    player_transform: &Transform,
    view_mode: ViewMode,
) -> Option<RaycastResponse> {
    match view_mode {
        ViewMode::FirstPerson => first_person_raycast(world_map, camera_transform),
        ViewMode::ThirdPerson => {
            third_person_raycast(world_map, camera_transform, player_transform)
        }
    }
}

fn first_person_raycast(
    world_map: &ClientWorldMap,
    camera_transform: &Transform,
) -> Option<RaycastResponse> {
    let camera_position = camera_transform.translation;
    let camera_rotation = camera_transform.rotation;

    let direction = camera_rotation
        .mul_vec3(Vec3::new(0.0, 0.0, -1.0))
        .normalize();

    let current_position = camera_position;

    raycast_from_source_position_and_direction(world_map, current_position, direction)
}

fn third_person_raycast(
    world_map: &ClientWorldMap,
    camera_transform: &Transform,
    player_transform: &Transform,
) -> Option<RaycastResponse> {
    let player_position = player_transform.translation;

    let camera_rotation = camera_transform.rotation;

    let camera_direction = camera_rotation
        .mul_vec3(Vec3::new(0.0, 0.0, -1.0))
        .normalize();

    let direction = camera_direction;

    let current_position = player_position;

    raycast_from_source_position_and_direction(world_map, current_position, direction)
}

fn raycast_from_source_position_and_direction(
    world_map: &ClientWorldMap,
    source_position: Vec3,
    direction: Vec3,
) -> Option<RaycastResponse> {
    let max_distance = 10.0; // Maximum distance for raycasting

    let mut current_position = source_position;

    let step = 0.1; // Step size for raycasting

    let mut previous_position = current_position;

    for _ in 0..(max_distance / step) as i32 {
        current_position += direction * step;
        let pos_ivec3 = IVec3::new(
            current_position.x.floor() as i32,
            current_position.y.floor() as i32,
            current_position.z.floor() as i32,
        );
        if let Some(block) = world_map.get_block_by_coordinates(&pos_ivec3) {
            // Now we need to determine which face of the block we hit
            let face = Vec3::new(
                previous_position.x - pos_ivec3.x as f32,
                previous_position.y - pos_ivec3.y as f32,
                previous_position.z - pos_ivec3.z as f32,
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
        previous_position = current_position;
    }

    None
}
