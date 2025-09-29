use bevy::{
    math::{bounding::Aabb3d, IVec3, Vec3},
    transform::components::Transform,
};

use crate::{
    players::ViewMode,
    world::{BlockData, BlockHitbox, WorldMap},
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
    world_map: &impl WorldMap,
    camera_transform: &Transform,
    player_position: &Vec3,
    view_mode: ViewMode,
) -> Option<RaycastResponse> {
    match view_mode {
        ViewMode::FirstPerson => first_person_raycast(world_map, camera_transform),
        ViewMode::ThirdPerson => third_person_raycast(world_map, camera_transform, player_position),
    }
}

fn first_person_raycast(
    world_map: &impl WorldMap,
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
    world_map: &impl WorldMap,
    camera_transform: &Transform,
    player_position: &Vec3,
) -> Option<RaycastResponse> {
    let camera_rotation = camera_transform.rotation;

    let camera_direction = camera_rotation
        .mul_vec3(Vec3::new(0.0, 0.0, -1.0))
        .normalize();

    let direction = camera_direction;

    let current_position = *player_position;

    raycast_from_source_position_and_direction(world_map, current_position, direction)
}

// Amanatides-Woo fast traversal algorithm
// Tweaked to include block-specific interaction boxes
pub fn raycast_from_source_position_and_direction(
    world_map: &impl WorldMap,
    origin: Vec3,
    direction: Vec3,
) -> Option<RaycastResponse> {
    let inv_dir = 1. / direction;

    let step = inv_dir.signum().as_ivec3();
    let delta = inv_dir.abs();

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
            match block.id.get_ray_hitbox() {
                BlockHitbox::FullBlock => {
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
                    })
                }
                BlockHitbox::Aabb(hitbox) => {
                    let hitbox = Aabb3d {
                        min: hitbox.min + voxel.as_vec3a(),
                        max: hitbox.max + voxel.as_vec3a(),
                    };
                    if let Some((pos, face)) = aabb_ray_hit(&hitbox, &origin, &direction, &inv_dir)
                    {
                        return Some(RaycastResponse {
                            block: *block,
                            position: pos.floor().as_ivec3(),
                            face,
                            bbox: hitbox,
                        });
                    }
                }
                BlockHitbox::None => {}
            }
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
pub fn aabb_ray_hit(
    aabb: &Aabb3d,
    origin: &Vec3,
    dir: &Vec3,
    inv_dir: &Vec3,
) -> Option<(Vec3, FaceDirection)> {
    let t1 = (Vec3::from(aabb.min) - origin) * inv_dir;
    let t2 = (Vec3::from(aabb.max) - origin) * inv_dir;

    let tmin = t1.min(t2);
    let tmax = t1.max(t2);

    let t_enter = tmin.max_element();
    let t_exit = tmax.min_element();

    if t_enter > t_exit || t_exit < 0.0 {
        return None;
    }

    let t = if t_enter >= 0.0 { t_enter } else { t_exit };
    let hit_pos = origin + dir * t;

    // Determine which face was hit by checking which axis produced t_enter
    let epsilon = 1e-5;

    let mut face = FaceDirection::PlusY;

    if (t_enter - tmin.x).abs() < epsilon {
        if dir.x > 0.0 {
            face = FaceDirection::MinusX;
        } else {
            face = FaceDirection::PlusX;
        }
    } else if (t_enter - tmin.y).abs() < epsilon {
        if dir.y > 0.0 {
            face = FaceDirection::MinusY;
        } else {
            face = FaceDirection::PlusY;
        }
    } else if (t_enter - tmin.z).abs() < epsilon {
        if dir.z > 0.0 {
            face = FaceDirection::MinusZ;
        } else {
            face = FaceDirection::PlusZ;
        }
    }

    Some((hit_pos, face))
}
