//! Sitting on a chair (`decor`): the seat of the furniture, as the passenger's seat of a mount
//! (`riding`). A right click on a chair seats the player on it, its hips on the seat and its legs
//! bent forward as a rider's, facing the front of the chair; the sneak key stands it up beside
//! the chair, as a rider gets off its mount. A chair broken under its sitter stands it up too.
//! The client predicts it all, the server repeats it.

use bevy::math::{bounding::Aabb3d, IVec3, Vec3};
use serde::{Deserialize, Serialize};

use crate::{
    messages::{NetworkAction, PlayerFrameInput},
    players::{movement::MovementOutcome, Player},
    world::{
        decor::{chair_heading, SEAT_HEIGHT},
        BlockDirection, ChunkMap, DecorKind,
    },
};

/// Room left between a player standing up and the blocks around it.
const CONTACT_GAP: f32 = 1e-3;
/// How far above its feet a seated player's hips are, as a rider's (`riding::seat_offset`).
const HIPS: f32 = 0.75;

/// The chair a player sits on. Part of the `MovementState`, which the server sends: the others
/// see the player seated. Not saved: a player who joins stands.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct Seat {
    /// The cell of the chair.
    pub at: IVec3,
    /// Where the sitter faces, as `riding::Riding::heading`.
    pub heading: f32,
}

/// The centre of a player `height` high seated on the chair at `at`.
pub fn seated_centre(at: IVec3, height: f32) -> Vec3 {
    at.as_vec3() + Vec3::new(0.5, SEAT_HEIGHT - HIPS + height / 2.0, 0.5)
}

/// Seats `player` on the chair at `at`, turned to `direction`. A rider stays on its mount.
pub fn sit(player: &mut Player, at: IVec3, direction: BlockDirection) -> bool {
    if player.riding.is_some() || player.is_dead() {
        return false;
    }
    player.sitting = Some(Seat {
        at,
        heading: chair_heading(direction),
    });
    player.position = seated_centre(at, player.height);
    player.velocity = Vec3::ZERO;
    player.fall_distance = 0.0;
    player.is_flying = false;
    player.is_sprinting = false;
    player.is_sneaking = false;
    player.is_swimming = false;
    true
}

/// Whether the block at `at` is a chair.
fn is_chair(world: &ChunkMap, at: IVec3) -> bool {
    world
        .get_block(at)
        .is_some_and(|block| block.id.decor_kind() == Some(DecorKind::Chair))
}

/// Stands `player` up from its chair: in front of it where there is room, else beside it, behind
/// it, or on the seat.
pub fn stand_up(player: &mut Player, world: &ChunkMap) {
    let Some(seat) = player.sitting.take() else {
        return;
    };
    let half = Vec3::new(player.width, player.height, player.width) / 2.0;
    let floor = seat.at.y as f32 + half.y + CONTACT_GAP;
    let centre = seat.at.as_vec3() + Vec3::new(0.5, 0.0, 0.5);
    let ahead = Vec3::new(seat.heading.sin(), 0.0, seat.heading.cos());
    let side = Vec3::new(ahead.z, 0.0, -ahead.x);
    let spot = [ahead, side, -side, -ahead]
        .into_iter()
        .map(|direction| (centre + direction).with_y(floor))
        .find(|&spot| !world.check_collision_box(&Aabb3d::new(spot, half)))
        .unwrap_or(centre.with_y(seat.at.y as f32 + SEAT_HEIGHT + half.y + CONTACT_GAP));
    player.position = spot;
    player.velocity = Vec3::ZERO;
    player.fall_distance = 0.0;
    player.on_ground = false;
}

/// One input of a seated player: it stands up with the sneak key, or once its chair is gone;
/// else it stays on the seat, looking where it likes.
pub fn simulate_sitting(
    player: &mut Player,
    world: &ChunkMap,
    action: &PlayerFrameInput,
) -> MovementOutcome {
    player.camera_transform = action.camera;
    player.is_flying = false;
    player.is_sprinting = false;
    player.is_sneaking = false;
    player.is_swimming = false;
    let Some(seat) = player.sitting else {
        return MovementOutcome::default();
    };
    if action.is_pressed(NetworkAction::SneakOrFlyDown) || !is_chair(world, seat.at) {
        stand_up(player, world);
        return MovementOutcome::default();
    }
    player.position = seated_centre(seat.at, player.height);
    player.velocity = Vec3::ZERO;
    player.fall_distance = 0.0;
    player.on_ground = true;
    MovementOutcome::default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{BlockData, BlockId};
    use bevy::prelude::Transform;

    fn world_with_chair(at: IVec3) -> ChunkMap {
        let mut world = ChunkMap::with_loaded_region(IVec3::splat(-32), IVec3::splat(31));
        world.fill_box(IVec3::new(-32, -8, -32), IVec3::new(31, -1, 31), BlockId::Stone);
        world.set_block(at, BlockData::new(BlockId::OakChair, BlockDirection::Back));
        world
    }

    fn input(actions: &[NetworkAction]) -> PlayerFrameInput {
        PlayerFrameInput {
            inputs: actions.iter().cloned().collect(),
            delta_ms: 16,
            ..Default::default()
        }
    }

    #[test]
    fn a_seated_player_stays_on_its_chair_and_stands_up_in_front_of_it_on_sneaking() {
        let at = IVec3::ZERO;
        let world = world_with_chair(at);
        let mut player = Player::new(1, "sitter".into(), Vec3::new(0.5, 3.0, 2.5), Transform::default());
        assert!(sit(&mut player, at, BlockDirection::Back));
        let seat = seated_centre(at, player.height);
        simulate_sitting(&mut player, &world, &input(&[NetworkAction::MoveForward]));
        assert_eq!(player.position, seat);
        assert!(player.sitting.is_some());
        simulate_sitting(&mut player, &world, &input(&[NetworkAction::SneakOrFlyDown]));
        assert_eq!(player.sitting, None);
        // the chair faces +Z: its sitter stands up in the cell in front of it
        assert_eq!(player.position.floor().as_ivec3().with_y(0), IVec3::new(0, 0, 1));
    }

    #[test]
    fn a_player_stands_up_when_its_chair_is_broken() {
        let at = IVec3::ZERO;
        let mut world = world_with_chair(at);
        let mut player = Player::new(1, "sitter".into(), Vec3::new(0.5, 3.0, 2.5), Transform::default());
        sit(&mut player, at, BlockDirection::Front);
        world.remove_block(at);
        simulate_sitting(&mut player, &world, &input(&[]));
        assert_eq!(player.sitting, None);
    }
}
