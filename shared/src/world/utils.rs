use bevy::math::{IVec3, Vec3};

use crate::CHUNK_SIZE;

pub fn block_to_chunk_coord(x: i32) -> i32 {
    if x >= 0 {
        x / CHUNK_SIZE
    } else {
        (x - (CHUNK_SIZE - 1)) / CHUNK_SIZE
    }
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
    IVec3 {
        x: ((global_block_pos.x % CHUNK_SIZE) + CHUNK_SIZE) % CHUNK_SIZE,
        y: ((global_block_pos.y % CHUNK_SIZE) + CHUNK_SIZE) % CHUNK_SIZE,
        z: ((global_block_pos.z % CHUNK_SIZE) + CHUNK_SIZE) % CHUNK_SIZE,
    }
}

pub fn global_block_to_chunk_pos(global_block_pos: &IVec3) -> IVec3 {
    IVec3::new(
        block_to_chunk_coord(global_block_pos.x),
        block_to_chunk_coord(global_block_pos.y),
        block_to_chunk_coord(global_block_pos.z),
    )
}

pub const SIX_OFFSETS: [IVec3; 6] = [
    IVec3::new(1, 0, 0),
    IVec3::new(-1, 0, 0),
    IVec3::new(0, 1, 0),
    IVec3::new(0, -1, 0),
    IVec3::new(0, 0, 1),
    IVec3::new(0, 0, -1),
];

#[cfg(test)]
mod tests {
    use super::*;

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
