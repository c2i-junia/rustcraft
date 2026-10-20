use bevy::math::{bounding::Aabb3d, Vec3};

use crate::world::{ChunkMap, Portal};

use super::Player;

/// Whether the collision box of `player`, centred on `candidate_position`, would overlap a
/// block. The wall cells of the open portals the player is in front of, within their opening
/// (`Portals::letting_through`), do not count: the player goes into them, and through the
/// portal. Where the player is now decides, not the candidate position, so that the move that
/// takes it behind the plane of the portal is made.
pub fn check_player_collision(
    candidate_position: &Vec3,
    player: &Player,
    world_map: &ChunkMap,
) -> bool {
    let hitbox = Aabb3d::new(
        *candidate_position,
        Vec3::new(player.width, player.height, player.width) / 2.0,
    );
    let mut letting_through = world_map
        .portals()
        .letting_through(player.position, player.width / 2.0)
        .peekable();
    if letting_through.peek().is_none() {
        return world_map.check_collision_box(&hitbox);
    }
    let open: Vec<Portal> = letting_through.copied().collect();
    world_map.check_collision_box_except(&hitbox, |cell| {
        open.iter()
            .any(|portal| portal.cells().any(|wall| wall == cell))
    })
}

/// Whether the collision box of `player` at `position` touches a liquid.
pub fn is_player_in_liquid(position: &Vec3, player: &Player, world_map: &ChunkMap) -> bool {
    world_map.touches_liquid(&Aabb3d::new(
        *position,
        Vec3::new(player.width, player.height, player.width) / 2.0,
    ))
}
