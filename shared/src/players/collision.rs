use bevy::math::{bounding::Aabb3d, Vec3};

use crate::world::ChunkMap;

use super::Player;

pub fn check_player_collision(
    candidate_position: &Vec3,
    player: &Player,
    world_map: &ChunkMap,
) -> bool {
    world_map.check_collision_box(&Aabb3d::new(
        *candidate_position,
        Vec3::new(player.width, player.height, player.width) / 2.0,
    ))
}

/// Whether the collision box of `player` at `position` touches a liquid.
pub fn is_player_in_liquid(position: &Vec3, player: &Player, world_map: &ChunkMap) -> bool {
    world_map.touches_liquid(&Aabb3d::new(
        *position,
        Vec3::new(player.width, player.height, player.width) / 2.0,
    ))
}
