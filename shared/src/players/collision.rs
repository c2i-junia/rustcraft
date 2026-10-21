use bevy::math::{bounding::Aabb3d, IVec3, Vec3};

use crate::world::ChunkMap;

use super::Player;

/// Room left for the rounding of the volumes `check_player_collision` compares, in cubic blocks.
const VOLUME_ROUNDING: f32 = 1e-6;

/// Whether the collision box of `player`, centred on `candidate_position`, would overlap a
/// block. The wall cells of the open portals the player is in front of, within their opening
/// (`Portals::passable_cells`), do not count: the player goes into them, and through the
/// portal. Where the player is now decides, not the candidate position, so that the move that
/// takes it behind the plane of the portal is made.
///
/// A player already inside blocks, as one left in the wall of a portal that closed or moved, may
/// make a move that reaches no block it is not in yet and goes no deeper into those it is in, so
/// that it can get out. Both sides compute it alike, from the player and the world alone.
pub fn check_player_collision(
    candidate_position: &Vec3,
    player: &Player,
    world_map: &ChunkMap,
) -> bool {
    let half_size = Vec3::new(player.width, player.height, player.width) / 2.0;
    let passable = world_map
        .portals()
        .passable_cells(player.position, player.width / 2.0);
    let let_through = |cell: IVec3| passable.contains(&cell);
    let candidate = Aabb3d::new(*candidate_position, half_size);
    if !world_map.check_collision_box_except(&candidate, let_through) {
        return false;
    }
    let current = Aabb3d::new(player.position, half_size);
    if !world_map.check_collision_box_except(&current, let_through) {
        return true;
    }
    let (low, high) = (
        current.min.floor().as_ivec3(),
        current.max.floor().as_ivec3(),
    );
    let inside = |cell: IVec3| cell.cmpge(low).all() && cell.cmple(high).all();
    world_map.check_collision_box_except(&candidate, |cell| let_through(cell) || inside(cell))
        || world_map.collision_volume_except(&candidate, let_through)
            > world_map.collision_volume_except(&current, let_through) + VOLUME_ROUNDING
}

/// Whether the collision box of `player` at `position` touches a liquid.
pub fn is_player_in_liquid(position: &Vec3, player: &Player, world_map: &ChunkMap) -> bool {
    world_map.touches_liquid(&Aabb3d::new(
        *position,
        Vec3::new(player.width, player.height, player.width) / 2.0,
    ))
}
