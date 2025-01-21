use crate::player::ViewMode;

use super::ClientWorldMap;
use bevy::{math::bounding::Aabb3d, prelude::*};
use shared::world::{BlockData, WorldMap};

#[derive(Debug, Clone, Copy)]
pub enum FaceDirection {
    PlusX,
    MinusX,
    PlusY,
    MinusY,
    PlusZ,
    MinusZ,
}

pub trait FaceDirectionExt {
    fn to_ivec3(&self) -> IVec3;
}

impl FaceDirectionExt for FaceDirection {
    fn to_ivec3(&self) -> IVec3 {
        match self {
            FaceDirection::PlusX => IVec3::new(1, 0, 0),
            FaceDirection::MinusX => IVec3::new(-1, 0, 0),
            FaceDirection::PlusY => IVec3::new(0, 1, 0),
            FaceDirection::MinusY => IVec3::new(0, -1, 0),
            FaceDirection::PlusZ => IVec3::new(0, 0, 1),
            FaceDirection::MinusZ => IVec3::new(0, 0, -1),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct RaycastResponse {
    pub block: BlockData,
    pub position: IVec3,
    pub face: FaceDirection,
    pub bbox: Aabb3d,
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

    let step = 0.05; // Step size for raycasting

    let mut previous_position = current_position;

    for _ in 0..(max_distance / step) as i32 {
        current_position += direction * step;
        let pos_ivec3 = IVec3::new(
            current_position.x.round() as i32,
            current_position.y.round() as i32,
            current_position.z.round() as i32,
        );
        if let Some(block) = world_map.get_block_by_coordinates(&pos_ivec3) {
            let bbox = block.id.get_interaction_box(&pos_ivec3);
            let pos = current_position.into();

            // Check if our hit is outside of the interaction box
            if bbox.closest_point(pos) != pos {
                // If it is, ignore the hit
                continue;
            }

            // Now we need to determine which face of the block we hit
            let face = determine_hit_face(previous_position, current_position);

            return Some(RaycastResponse {
                block: *block,
                position: pos_ivec3,
                face,
                bbox,
            });
        }
        previous_position = current_position;
    }

    None
}

fn determine_hit_face(step_outside: Vec3, step_inside: Vec3) -> FaceDirection {
    let step_outside = IVec3::new(
        step_outside.x as i32,
        step_outside.y as i32,
        step_outside.z as i32,
    );
    let step_inside = IVec3::new(
        step_inside.x as i32,
        step_inside.y as i32,
        step_inside.z as i32,
    );

    let diff = step_inside - step_outside;

    if diff.x.abs() > diff.y.abs() && diff.x.abs() > diff.z.abs() {
        if diff.x > 0 {
            FaceDirection::MinusX
        } else {
            FaceDirection::PlusX
        }
    } else if diff.y.abs() > diff.x.abs() && diff.y.abs() > diff.z.abs() {
        if diff.y > 0 {
            FaceDirection::MinusY
        } else {
            FaceDirection::PlusY
        }
    } else if diff.z > 0 {
        FaceDirection::MinusZ
    } else {
        FaceDirection::PlusZ
    }
}
