use bevy::{
    math::{bounding::Aabb3d, IVec3, Vec3},
    transform::components::Transform,
};

use crate::{
    players::ViewMode,
    world::{liquid::is_source, BlockData, BlockHitbox, ChunkMap},
    HALF_BLOCK,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

/// The first block hit by the ray the player aims with. The ray follows the camera's
/// direction; it starts at the camera in first person and at the player in third person.
pub fn raycast(
    world_map: &ChunkMap,
    camera_transform: &Transform,
    player_position: &Vec3,
    view_mode: ViewMode,
) -> Option<RaycastResponse> {
    cast(
        world_map,
        camera_transform,
        player_position,
        view_mode,
        false,
    )
}

/// `raycast`, but the ray also stops at a water source, which it hits as a whole block: what an
/// empty bucket aims at. Flowing water stays transparent to it.
pub fn raycast_to_water_source(
    world_map: &ChunkMap,
    camera_transform: &Transform,
    player_position: &Vec3,
    view_mode: ViewMode,
) -> Option<RaycastResponse> {
    cast(
        world_map,
        camera_transform,
        player_position,
        view_mode,
        true,
    )
}

fn cast(
    world_map: &ChunkMap,
    camera_transform: &Transform,
    player_position: &Vec3,
    view_mode: ViewMode,
    sources: bool,
) -> Option<RaycastResponse> {
    let origin = match view_mode {
        ViewMode::FirstPerson => camera_transform.translation,
        ViewMode::ThirdPerson => *player_position,
    };
    let direction = camera_transform.rotation.mul_vec3(Vec3::NEG_Z).normalize();
    traverse(world_map, origin, direction, sources)
}

/// Maximum distance, in ray parameter units (blocks for a normalized direction), a ray travels.
const MAX_RAY_DISTANCE: f32 = 20.0;

/// Largest absolute coordinate accepted for a ray origin. It keeps every integer of the
/// traversal (`voxel`, `voxel + 1`, and the few thousand steps of a ray) far from `i32`
/// overflow, which a camera position sent by a client could otherwise trigger.
const MAX_ORIGIN_COORDINATE: f32 = 1.0e9;

/// Hard cap on the traversal steps, whatever the length of the direction vector.
const MAX_RAY_STEPS_CAP: usize = 4096;

/// Number of voxel boundaries a ray of `MAX_RAY_DISTANCE` can cross: at most `|d.i| * L + 1`
/// per axis (about `3 L + 3` for a normalized direction), so the loop always ends.
fn max_ray_steps(direction: Vec3) -> usize {
    let steps = (direction.abs().element_sum() * MAX_RAY_DISTANCE).ceil() as usize;
    steps.saturating_add(3).min(MAX_RAY_STEPS_CAP)
}

pub fn raycast_from_source_position_and_direction(
    world_map: &ChunkMap,
    origin: Vec3,
    direction: Vec3,
) -> Option<RaycastResponse> {
    traverse(world_map, origin, direction, false)
}

// Amanatides-Woo fast traversal algorithm
// Tweaked to include block-specific interaction boxes, and the water sources when `sources`
fn traverse(
    world_map: &ChunkMap,
    origin: Vec3,
    direction: Vec3,
    sources: bool,
) -> Option<RaycastResponse> {
    // The camera comes from the network: reject anything that cannot define a ray
    if !origin.is_finite()
        || origin.abs().max_element() > MAX_ORIGIN_COORDINATE
        || !direction.is_finite()
        || direction == Vec3::ZERO
    {
        return None;
    }

    let inv_dir = 1. / direction;
    let step = inv_dir.signum().as_ivec3();
    let delta = inv_dir.abs();

    // Origin, constrained to its voxel (integer) position
    // Explicit flooring needed for negative coordinates
    let mut voxel = origin.floor().as_ivec3();
    let mut t = first_boundaries(origin, direction, voxel, step);
    let mut axis = 0;
    // Ray parameter of the last voxel boundary crossed
    let mut distance = 0.0;

    for _ in 0..max_ray_steps(direction) {
        if distance >= MAX_RAY_DISTANCE {
            break;
        }
        if let Some(block) = world_map.get_block(voxel) {
            let entered_by = entry_face(axis, step[axis])?;
            let hit = hit_block(
                *block, voxel, entered_by, sources, origin, direction, inv_dir,
            );
            if hit.is_some() {
                return hit;
            }
        }

        axis = nearest_boundary_axis(t);
        distance = t[axis];
        t[axis] += delta[axis];
        voxel[axis] += step[axis];
    }
    None
}

/// Ray parameter of the first voxel boundary on each axis (`tMax` in the algorithm), so that
/// the position of the origin inside its voxel does not skew the traversal.
fn first_boundaries(origin: Vec3, direction: Vec3, voxel: IVec3, step: IVec3) -> Vec3 {
    let mut t = Vec3::ZERO;
    for i in 0..3 {
        // `-0.0` has a negative sign but no direction: dividing by it would give NaN
        t[i] = if direction[i] != 0.0 {
            let next_boundary = voxel[i] + if step[i] > 0 { 1 } else { 0 };
            (next_boundary as f32 - origin[i]) / direction[i]
        } else {
            f32::INFINITY
        }
    }
    t
}

/// The axis whose next voxel boundary the ray reaches first.
fn nearest_boundary_axis(t: Vec3) -> usize {
    if t.x < t.y {
        if t.x < t.z {
            0
        } else {
            2
        }
    } else if t.y < t.z {
        1
    } else {
        2
    }
}

/// The face of a voxel a ray enters through when it steps by `step` along `axis`. `None` for
/// a step of 0, which a finite, non-zero direction never gives.
fn entry_face(axis: usize, step: i32) -> Option<FaceDirection> {
    Some(match (axis, step) {
        (0, -1) => FaceDirection::PlusX,
        (0, 1) => FaceDirection::MinusX,
        (1, -1) => FaceDirection::PlusY,
        (1, 1) => FaceDirection::MinusY,
        (2, -1) => FaceDirection::PlusZ,
        (2, 1) => FaceDirection::MinusZ,
        _ => return None,
    })
}

/// Whether the ray, which entered `voxel` through `entered_by`, hits the interaction box of
/// the `block` it holds, a whole block for a water source when it stops at `sources`.
fn hit_block(
    block: BlockData,
    voxel: IVec3,
    entered_by: FaceDirection,
    sources: bool,
    origin: Vec3,
    direction: Vec3,
    inv_dir: Vec3,
) -> Option<RaycastResponse> {
    let hitbox = if sources && is_source(&block) {
        BlockHitbox::FullBlock
    } else {
        block.get_ray_hitbox()
    };
    match hitbox {
        BlockHitbox::FullBlock => Some(RaycastResponse {
            block,
            position: voxel,
            face: entered_by,
            bbox: Aabb3d::new(voxel.as_vec3() + HALF_BLOCK, HALF_BLOCK),
        }),
        BlockHitbox::Aabb(hitbox) => {
            let hitbox = Aabb3d {
                min: hitbox.min + voxel.as_vec3a(),
                max: hitbox.max + voxel.as_vec3a(),
            };
            let (_, face) = aabb_ray_hit(&hitbox, &origin, &direction, &inv_dir)?;
            Some(RaycastResponse {
                block,
                position: voxel,
                face,
                bbox: hitbox,
            })
        }
        BlockHitbox::None => None,
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::BlockId;
    use bevy::math::Quat;

    const ORIGIN: Vec3 = Vec3::new(0.5, 0.5, 0.5);

    /// Every chunk the tests can reach is loaded, and empty but for the given block.
    fn empty_world() -> ChunkMap {
        ChunkMap::with_loaded_region(IVec3::splat(-48), IVec3::splat(48))
    }

    fn world_with_block(position: IVec3, id: BlockId) -> ChunkMap {
        let mut world = empty_world();
        world.fill_box(position, position, id);
        world
    }

    #[test]
    fn axis_aligned_rays_hit_the_block_in_front_with_the_facing_face() {
        let cases = [
            (Vec3::X, IVec3::new(3, 0, 0), FaceDirection::MinusX),
            (Vec3::NEG_X, IVec3::new(-3, 0, 0), FaceDirection::PlusX),
            (Vec3::Y, IVec3::new(0, 3, 0), FaceDirection::MinusY),
            (Vec3::NEG_Y, IVec3::new(0, -3, 0), FaceDirection::PlusY),
            (Vec3::Z, IVec3::new(0, 0, 3), FaceDirection::MinusZ),
            (Vec3::NEG_Z, IVec3::new(0, 0, -3), FaceDirection::PlusZ),
        ];
        for (direction, block_pos, face) in cases {
            let world = world_with_block(block_pos, BlockId::Stone);
            let hit = raycast_from_source_position_and_direction(&world, ORIGIN, direction)
                .unwrap_or_else(|| panic!("no hit along {direction:?}"));
            assert_eq!(hit.position, block_pos, "direction {direction:?}");
            assert_eq!(hit.face, face, "direction {direction:?}");
            assert_eq!(hit.block.id, BlockId::Stone);
        }
    }

    #[test]
    fn ray_returns_the_nearest_of_two_blocks() {
        let mut world = world_with_block(IVec3::new(2, 0, 0), BlockId::Dirt);
        world.fill_box(IVec3::new(4, 0, 0), IVec3::new(4, 0, 0), BlockId::Stone);
        let hit = raycast_from_source_position_and_direction(&world, ORIGIN, Vec3::X).unwrap();
        assert_eq!(hit.position, IVec3::new(2, 0, 0));
        assert_eq!(hit.block.id, BlockId::Dirt);
    }

    #[test]
    fn ray_misses_when_nothing_is_in_the_way() {
        let world = world_with_block(IVec3::new(0, 3, 0), BlockId::Stone);
        assert!(raycast_from_source_position_and_direction(&world, ORIGIN, Vec3::X).is_none());
        assert!(
            raycast_from_source_position_and_direction(&empty_world(), ORIGIN, Vec3::X).is_none()
        );
    }

    #[test]
    fn ray_does_not_hit_a_block_behind_the_origin() {
        let world = world_with_block(IVec3::new(-3, 0, 0), BlockId::Stone);
        assert!(raycast_from_source_position_and_direction(&world, ORIGIN, Vec3::X).is_none());
    }

    #[test]
    fn water_is_transparent_to_rays() {
        let mut world = world_with_block(IVec3::new(2, 0, 0), BlockId::Water);
        world.fill_box(IVec3::new(4, 0, 0), IVec3::new(4, 0, 0), BlockId::Stone);
        let hit = raycast_from_source_position_and_direction(&world, ORIGIN, Vec3::X).unwrap();
        assert_eq!(hit.position, IVec3::new(4, 0, 0));
    }

    #[test]
    fn a_ray_to_water_sources_stops_at_a_source_and_goes_through_flowing_water() {
        use crate::world::BlockDirection;
        let water = |level| BlockData {
            level,
            ..BlockData::new(BlockId::Water, BlockDirection::Front)
        };
        let mut world = world_with_block(IVec3::new(6, 0, 0), BlockId::Stone);
        world.set_block(IVec3::new(2, 0, 0), water(3));
        world.set_block(IVec3::new(4, 0, 0), water(0));

        let hit = traverse(&world, ORIGIN, Vec3::X, true).unwrap();
        assert_eq!(hit.position, IVec3::new(4, 0, 0));
        assert_eq!(hit.face, FaceDirection::MinusX);
        assert_eq!(hit.block, water(0));
        // A ray that ignores water reaches the stone
        let hit = traverse(&world, ORIGIN, Vec3::X, false).unwrap();
        assert_eq!(hit.position, IVec3::new(6, 0, 0));
        // So does the camera's ray, and the bucket's stops at the source
        let camera = Transform::from_translation(ORIGIN).looking_to(Vec3::X, Vec3::Y);
        let view = ViewMode::FirstPerson;
        let at = |hit: Option<RaycastResponse>| hit.map(|hit| hit.position);
        assert_eq!(
            at(raycast(&world, &camera, &ORIGIN, view)),
            Some(IVec3::new(6, 0, 0))
        );
        assert_eq!(
            at(raycast_to_water_source(&world, &camera, &ORIGIN, view)),
            Some(IVec3::new(4, 0, 0))
        );
    }

    #[test]
    fn diagonal_ray_hits_the_block_it_points_at() {
        // From the centre of (0,0,0) along (2,1,0): reaches the centre of (4,2,0)
        // without passing through any voxel corner.
        let target = IVec3::new(4, 2, 0);
        let world = world_with_block(target, BlockId::Stone);
        let direction = Vec3::new(2.0, 1.0, 0.0).normalize();
        let hit = raycast_from_source_position_and_direction(&world, ORIGIN, direction).unwrap();
        assert_eq!(hit.position, target);
        assert_eq!(hit.face, FaceDirection::MinusX);
    }

    #[test]
    fn ray_from_negative_coordinates_hits_the_block_in_front() {
        let origin = Vec3::new(-10.5, -10.5, -10.5);
        let target = IVec3::new(-8, -11, -11);
        let world = world_with_block(target, BlockId::Stone);
        let hit = raycast_from_source_position_and_direction(&world, origin, Vec3::X).unwrap();
        assert_eq!(hit.position, IVec3::new(-8, -11, -11));
    }

    #[test]
    fn ray_hits_a_block_one_to_six_blocks_away() {
        for n in 1..=6 {
            let target = IVec3::new(n, 0, 0);
            let world = world_with_block(target, BlockId::Stone);
            let hit = raycast_from_source_position_and_direction(&world, ORIGIN, Vec3::X);
            assert_eq!(hit.map(|h| h.position), Some(target), "n = {n}");
        }
    }

    #[test]
    fn ray_hits_a_block_up_to_nineteen_blocks_away() {
        for n in 1..=19 {
            let target = IVec3::new(n, 0, 0);
            let world = world_with_block(target, BlockId::Stone);
            let hit = raycast_from_source_position_and_direction(&world, ORIGIN, Vec3::X);
            assert_eq!(hit.map(|h| h.position), Some(target), "n = {n}");
        }
    }

    #[test]
    fn ray_does_not_reach_beyond_the_maximum_distance() {
        let target = IVec3::new(MAX_RAY_DISTANCE as i32 + 2, 0, 0);
        let world = world_with_block(target, BlockId::Stone);
        assert!(raycast_from_source_position_and_direction(&world, ORIGIN, Vec3::X).is_none());
    }

    #[test]
    fn rays_with_a_non_finite_origin_or_degenerate_direction_hit_nothing() {
        let world = world_with_block(IVec3::new(2, 0, 0), BlockId::Stone);
        let cases = [
            (ORIGIN, Vec3::ZERO),
            (ORIGIN, Vec3::NAN),
            (ORIGIN, Vec3::new(f32::INFINITY, 0.0, 0.0)),
            (Vec3::NAN, Vec3::X),
            (Vec3::new(0.5, f32::INFINITY, 0.5), Vec3::X),
        ];
        for (origin, direction) in cases {
            assert!(
                raycast_from_source_position_and_direction(&world, origin, direction).is_none(),
                "origin {origin:?}, direction {direction:?}"
            );
        }
    }

    #[test]
    fn rays_with_a_huge_finite_origin_hit_nothing_and_return() {
        let world = world_with_block(IVec3::new(2, 0, 0), BlockId::Stone);
        let started = std::time::Instant::now();
        for axis in 0..3 {
            for value in [3.0e9, -3.0e9, 1.0e30, -1.0e30, f32::MAX, f32::MIN] {
                let mut origin = ORIGIN;
                origin[axis] = value;
                for direction in [Vec3::X, Vec3::NEG_Y, Vec3::ONE.normalize()] {
                    assert!(
                        raycast_from_source_position_and_direction(&world, origin, direction)
                            .is_none(),
                        "origin {origin:?}, direction {direction:?}"
                    );
                }
            }
        }
        assert!(started.elapsed().as_secs() < 2);
    }

    #[test]
    fn a_ray_that_hits_nothing_stops_within_the_step_bound() {
        let world = empty_world();
        let direction = Vec3::ONE.normalize();
        assert!(max_ray_steps(direction) <= 3 * MAX_RAY_DISTANCE as usize + 3);
        let started = std::time::Instant::now();
        // An origin right at the accepted bound must not overflow either
        let origin = Vec3::splat(MAX_ORIGIN_COORDINATE);
        assert!(raycast_from_source_position_and_direction(&world, origin, direction).is_none());
        assert!(started.elapsed().as_secs() < 2);
    }

    #[test]
    fn camera_with_a_degenerate_rotation_hits_nothing() {
        let world = world_with_block(IVec3::new(2, 0, 0), BlockId::Stone);
        for rotation in [Quat::from_xyzw(0.0, 0.0, 0.0, 0.0), Quat::NAN] {
            let camera = Transform::from_translation(ORIGIN).with_rotation(rotation);
            for view_mode in [ViewMode::FirstPerson, ViewMode::ThirdPerson] {
                assert!(
                    raycast(&world, &camera, &ORIGIN, view_mode).is_none(),
                    "{rotation:?} in {view_mode:?}"
                );
            }
        }
    }

    #[test]
    fn flower_hitbox_is_hit_through_its_aabb() {
        let world = world_with_block(IVec3::new(2, 0, 0), BlockId::Poppy);
        let origin = Vec3::new(0.5, 0.3, 0.5);
        let hit = raycast_from_source_position_and_direction(&world, origin, Vec3::X).unwrap();
        assert_eq!(hit.position, IVec3::new(2, 0, 0));
        assert_eq!(hit.face, FaceDirection::MinusX);
    }

    #[test]
    fn flower_hitbox_is_missed_above_its_aabb() {
        let world = world_with_block(IVec3::new(2, 0, 0), BlockId::Poppy);
        let origin = Vec3::new(0.5, 0.9, 0.5);
        assert!(raycast_from_source_position_and_direction(&world, origin, Vec3::X).is_none());
    }

    #[test]
    fn a_vine_is_hit_through_its_plate_in_its_own_cell() {
        use crate::world::{BlockDirection, VINE_GAP};
        let vine = IVec3::new(2, 0, 2);
        let centre = vine.as_vec3() + HALF_BLOCK;
        for direction in [
            BlockDirection::Front,
            BlockDirection::Right,
            BlockDirection::Back,
            BlockDirection::Left,
        ] {
            let mut world = empty_world();
            world.set_block(vine, BlockData::new(BlockId::Vine, direction));
            let support = direction.offset().as_vec3();
            let along = Vec3::new(support.z, 0.0, -support.x);
            let cast = |origin: Vec3, toward: Vec3| {
                raycast_from_source_position_and_direction(&world, origin, toward)
            };

            // from the open side, straight at the plate
            let hit = cast(centre - support * 2.0, support)
                .unwrap_or_else(|| panic!("{direction:?}: the plate is missed"));
            assert_eq!(hit.position, vine, "{direction:?}");
            assert_eq!(hit.face.to_ivec3().as_vec3(), -support, "{direction:?}");
            // through the middle of the cell, beside the plate
            for side in [along, -along, Vec3::Y, Vec3::NEG_Y] {
                assert!(
                    cast(centre + side * 2.0, -side).is_none(),
                    "{direction:?}: hit from {side}"
                );
            }
            // along the support, through the plate, from each side and from above
            let on_plate = centre + support * (0.5 - VINE_GAP);
            for side in [along, -along, Vec3::Y] {
                let hit = cast(on_plate + side * 2.0, -side)
                    .unwrap_or_else(|| panic!("{direction:?}: missed from {side}"));
                assert_eq!(hit.position, vine, "{direction:?}: from {side}");
            }
        }
    }

    #[test]
    fn rays_with_zero_components_from_an_integer_boundary_hit_the_block_in_front() {
        let block_pos = IVec3::new(3, 0, 0);
        let world = world_with_block(block_pos, BlockId::Stone);
        for direction in [Vec3::X, Vec3::new(1.0, -0.0, -0.0)] {
            let hit = raycast_from_source_position_and_direction(&world, Vec3::ZERO, direction)
                .unwrap_or_else(|| panic!("no hit along {direction:?}"));
            assert_eq!(hit.position, block_pos, "direction {direction:?}");
            assert_eq!(hit.face, FaceDirection::MinusX, "direction {direction:?}");
        }
    }
}
