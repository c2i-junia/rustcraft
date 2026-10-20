use std::collections::HashMap;
use std::sync::{Arc, LazyLock, Mutex};

use bevy::math::{IVec3, Vec3};

use crate::CHUNK_SIZE;

pub fn block_to_chunk_coord(x: i32) -> i32 {
    x.div_euclid(CHUNK_SIZE)
}

pub fn block_vec3_to_chunk_v3_coord(v: Vec3) -> Vec3 {
    Vec3::new(
        block_to_chunk_coord(v.x.floor() as i32) as f32,
        block_to_chunk_coord(v.y.floor() as i32) as f32,
        block_to_chunk_coord(v.z.floor() as i32) as f32,
    )
}

pub fn world_position_to_chunk_position(v: Vec3) -> IVec3 {
    IVec3::new(
        block_to_chunk_coord(v.x.floor() as i32),
        block_to_chunk_coord(v.y.floor() as i32),
        block_to_chunk_coord(v.z.floor() as i32),
    )
}

pub fn to_global_pos(chunk_pos: &IVec3, local_block_pos: &IVec3) -> IVec3 {
    *chunk_pos * CHUNK_SIZE + *local_block_pos
}

pub fn to_local_pos(global_block_pos: &IVec3) -> IVec3 {
    global_block_pos.rem_euclid(IVec3::splat(CHUNK_SIZE))
}

pub fn global_block_to_chunk_pos(global_block_pos: &IVec3) -> IVec3 {
    IVec3::new(
        block_to_chunk_coord(global_block_pos.x),
        block_to_chunk_coord(global_block_pos.y),
        block_to_chunk_coord(global_block_pos.z),
    )
}

/// How many chunks apart two chunks are, along the axis where they are farthest apart: the
/// chunks within `d` of a chunk form a cube of side `2d + 1` around it.
pub fn chunk_distance(a: IVec3, b: IVec3) -> i32 {
    (a - b).abs().max_element()
}

/// How far above and below its player a render distance reaches, at most, in chunks: the ground
/// lies within a few chunks of height, so a larger render distance only reaches farther along x
/// and z. The chunks within 8 reach 128 blocks above and below the player.
pub const MAX_VERTICAL_RENDER_DISTANCE: i32 = 8;

/// How far above and below its player the render distance `distance` reaches, in chunks.
pub fn vertical_render_distance(distance: i32) -> i32 {
    distance.min(MAX_VERTICAL_RENDER_DISTANCE)
}

/// Whether `chunk` is within `horizontal` chunks of `center` along x and z, and within
/// `vertical` chunks along y.
pub fn chunk_in_reach(chunk: IVec3, center: IVec3, horizontal: i32, vertical: i32) -> bool {
    let gap = (chunk - center).abs();
    gap.x.max(gap.z) <= horizontal && gap.y <= vertical
}

/// Whether `chunk` is within the render distance `distance` of the chunk `center`.
pub fn within_render_distance(chunk: IVec3, center: IVec3, distance: i32) -> bool {
    chunk_in_reach(chunk, center, distance, vertical_render_distance(distance))
}

/// The offsets from its centre of the chunks within the render distance `distance`, nearest
/// first. Each distance is computed once: there are 70 000 of them at 32.
pub fn render_distance_offsets(distance: i32) -> Arc<[IVec3]> {
    static OFFSETS: LazyLock<Mutex<HashMap<i32, Arc<[IVec3]>>>> = LazyLock::new(Default::default);
    let mut offsets = OFFSETS
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    offsets
        .entry(distance)
        .or_insert_with(|| {
            let vertical = vertical_render_distance(distance);
            let mut chunks: Vec<IVec3> = (-distance..=distance)
                .flat_map(|x| {
                    (-vertical..=vertical)
                        .flat_map(move |y| (-distance..=distance).map(move |z| IVec3::new(x, y, z)))
                })
                .collect();
            chunks.sort_by_key(|c| c.length_squared());
            chunks.into()
        })
        .clone()
}

pub const SIX_OFFSETS: [IVec3; 6] = [
    IVec3::new(1, 0, 0),
    IVec3::new(-1, 0, 0),
    IVec3::new(0, 1, 0),
    IVec3::new(0, -1, 0),
    IVec3::new(0, 0, 1),
    IVec3::new(0, 0, -1),
];

/// The `splitmix64` finaliser: a bijective mixing of 64 bits, good enough that consecutive
/// inputs give unrelated outputs.
pub fn splitmix64(value: u64) -> u64 {
    let mut z = value.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splitmix64_matches_the_reference_vector() {
        // first output of the reference splitmix64 generator seeded with 0
        assert_eq!(splitmix64(0), 0xE220_A839_7B1D_CDAF);
    }

    #[test]
    fn block_to_chunk_coord_handles_boundaries() {
        let cases = [
            (-33, -3),
            (-17, -2),
            (-16, -1),
            (-1, -1),
            (0, 0),
            (1, 0),
            (15, 0),
            (16, 1),
            (31, 1),
            (32, 2),
        ];
        for (block, chunk) in cases {
            assert_eq!(block_to_chunk_coord(block), chunk, "block {block}");
        }
    }

    #[test]
    fn to_local_pos_wraps_into_chunk_range() {
        let cases = [(-17, 15), (-16, 0), (-1, 15), (0, 0), (15, 15), (16, 0)];
        for (global, local) in cases {
            assert_eq!(
                to_local_pos(&IVec3::new(global, global, global)),
                IVec3::splat(local),
                "global {global}"
            );
        }
    }

    #[test]
    fn to_global_pos_combines_chunk_and_local() {
        assert_eq!(
            to_global_pos(&IVec3::new(0, 0, 0), &IVec3::new(3, 4, 5)),
            IVec3::new(3, 4, 5)
        );
        assert_eq!(
            to_global_pos(&IVec3::new(1, -1, 2), &IVec3::new(0, 15, 7)),
            IVec3::new(16, -1, 39)
        );
        assert_eq!(
            to_global_pos(&IVec3::new(-2, -1, -1), &IVec3::new(15, 0, 15)),
            IVec3::new(-17, -16, -1)
        );
    }

    #[test]
    fn global_block_to_chunk_pos_per_axis() {
        assert_eq!(
            global_block_to_chunk_pos(&IVec3::new(-17, 0, 16)),
            IVec3::new(-2, 0, 1)
        );
        assert_eq!(
            global_block_to_chunk_pos(&IVec3::new(15, -1, -16)),
            IVec3::new(0, -1, -1)
        );
    }

    #[test]
    fn a_render_distance_reaches_as_far_along_x_and_z_but_at_most_8_along_y() {
        for distance in [1, 3, 8, 12] {
            let offsets = render_distance_offsets(distance);
            let vertical = distance.min(8);
            let side = 2 * distance + 1;
            assert_eq!(offsets.len() as i32, side * side * (2 * vertical + 1));
            assert!(offsets
                .iter()
                .all(|&c| within_render_distance(c, IVec3::ZERO, distance)));
            assert!(offsets.contains(&IVec3::new(distance, vertical, -distance)));
            assert!(!within_render_distance(
                IVec3::new(0, vertical + 1, 0),
                IVec3::ZERO,
                distance
            ));
            assert!(!within_render_distance(
                IVec3::new(distance + 1, 0, 0),
                IVec3::ZERO,
                distance
            ));
            assert!(offsets
                .windows(2)
                .all(|pair| pair[0].length_squared() <= pair[1].length_squared()));
        }
        assert_eq!(render_distance_offsets(2)[0], IVec3::ZERO);
    }

    #[test]
    fn chunk_distance_is_the_largest_gap_along_an_axis() {
        assert_eq!(chunk_distance(IVec3::ZERO, IVec3::ZERO), 0);
        assert_eq!(chunk_distance(IVec3::ZERO, IVec3::new(-3, 0, 0)), 3);
        assert_eq!(chunk_distance(IVec3::ZERO, IVec3::new(0, 2, 0)), 2);
        assert_eq!(chunk_distance(IVec3::new(1, 1, 1), IVec3::new(-1, 3, 0)), 2);
        assert_eq!(
            chunk_distance(IVec3::new(5, -2, 4), IVec3::new(4, -1, 3)),
            1
        );
    }

    #[test]
    fn global_local_round_trip() {
        for x in (-40..=40).step_by(3) {
            for y in (-40..=40).step_by(7) {
                for z in (-40..=40).step_by(5) {
                    let p = IVec3::new(x, y, z);
                    let chunk = global_block_to_chunk_pos(&p);
                    let local = to_local_pos(&p);
                    assert!(
                        local.cmpge(IVec3::ZERO).all()
                            && local.cmplt(IVec3::splat(CHUNK_SIZE)).all()
                    );
                    assert_eq!(to_global_pos(&chunk, &local), p);
                }
            }
        }
    }

    #[test]
    fn world_position_to_chunk_position_positive_and_integer() {
        assert_eq!(
            world_position_to_chunk_position(Vec3::new(0.0, 0.0, 0.0)),
            IVec3::ZERO
        );
        assert_eq!(
            world_position_to_chunk_position(Vec3::new(15.9, 16.0, 31.5)),
            IVec3::new(0, 1, 1)
        );
        assert_eq!(
            world_position_to_chunk_position(Vec3::new(-16.0, -17.0, -1.0)),
            IVec3::new(-1, -2, -1)
        );
    }

    #[test]
    fn world_position_to_chunk_position_negative_fractions() {
        assert_eq!(
            world_position_to_chunk_position(Vec3::new(-0.5, -0.5, -0.5)),
            IVec3::splat(-1)
        );
        assert_eq!(
            world_position_to_chunk_position(Vec3::new(-16.5, -15.5, -0.1)),
            IVec3::new(-2, -1, -1)
        );
    }
}
