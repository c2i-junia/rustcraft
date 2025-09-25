use crate::player::ViewMode;

use super::ClientWorldMap;
use bevy::{math::bounding::Aabb3d, prelude::*};
use shared::{
    world::{BlockData, WorldMap},
    HALF_BLOCK,
};

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

// Amanatides-Woo fast traversal algorithm
// Tweaked to include block-specific interaction boxes
fn raycast_from_source_position_and_direction(
    world_map: &ClientWorldMap,
    origin: Vec3,
    direction: Vec3,
) -> Option<RaycastResponse> {
    let dir_inv = 1. / direction;

    let step = dir_inv.signum().as_ivec3();
    let delta = dir_inv.abs();

    let mut axis = 0;

    // Origin, constrained to its voxel (integer) position
    // Explicit flooring needed for negative coordinates
    let mut voxel = origin.floor().as_ivec3();

    // Calculate tMax (distance to first voxel boundary)
    // Needed so that the player's position inside a voxel doesn't mess up the ray projection
    let mut t = Vec3::ZERO;
    for i in 0..3 {
        t[i] = if step[i] != 0 {
            let next_boundary = voxel[i] + if step[i] > 0 { 1 } else { 0 };
            (next_boundary as f32 - origin[i]) / direction[i]
        } else {
            f32::INFINITY
        }
    }

    // Total Euclidean distance
    let mut distance = 0.0;

    // Actual raycast loop
    while distance < 20.0 {
        if let Some(block) = world_map.get_block_by_coordinates(&voxel) {
            return Some(RaycastResponse {
                block: *block,
                position: voxel,
                face: match (axis, step[axis]) {
                    (0, -1) => FaceDirection::PlusX,
                    (0, 1) => FaceDirection::MinusX,
                    (1, -1) => FaceDirection::PlusY,
                    (1, 1) => FaceDirection::MinusY,
                    (2, -1) => FaceDirection::PlusZ,
                    (2, 1) => FaceDirection::MinusZ,
                    _ => unreachable!(),
                },
                bbox: Aabb3d::new(voxel.as_vec3() + HALF_BLOCK, HALF_BLOCK),
            });
        }

        // Choose new step direction
        if t.x < t.y {
            if t.x < t.z {
                axis = 0;
            } else {
                axis = 2;
            }
        } else if t.y < t.z {
            axis = 1;
        } else {
            axis = 2;
        }

        // Update the ray's position
        distance += t[axis];
        t[axis] += delta[axis];
        voxel[axis] += step[axis];
    }
    None
}

// Computes the collision between an AABB and a raycasting ray
#[allow(dead_code)]
fn aabb_ray_hit(aabb: &Aabb3d, origin: &Vec3, inv_dir: &Vec3) -> Option<(f32, f32)> {
    let mut tmin: f32 = 0.;
    let mut tmax: f32 = f32::INFINITY;

    // Looping over all axes
    for axis in 0..3 {
        let t1 = (aabb.min[axis] - origin[axis]) * inv_dir[axis];
        let t2 = (aabb.max[axis] - origin[axis]) * inv_dir[axis];

        let dmin = t1.min(t2);
        let dmax = t1.max(t2);

        tmin = dmin.max(tmin);
        tmax = dmax.min(tmax);
    }

    if tmax >= tmin {
        Some((tmin, tmax))
    } else {
        None
    } /* miss */
}
