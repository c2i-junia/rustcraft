use crate::{
    messages::{NetworkAction, PlayerFrameInput},
    players::{
        collision::check_player_collision,
        constants::{GRAVITY, JUMP_VELOCITY, SPEED},
    },
    world::WorldMap,
};
use bevy::prelude::*;

use super::Player;

#[allow(clippy::let_and_return)]
pub fn simulate_player_movement(
    player: &mut Player,
    world_map: &(impl WorldMap + Clone),
    action: &PlayerFrameInput,
    delta_ms: u64,
) -> bool {
    // TODO: Ridiculous performance issue, clone should be avoided
    let world_clone = world_map.clone();

    let delta = delta_ms as f32 / 1000.0;

    let initial_pos = player.position;
    let initial_rot = player.camera_transform.rotation;

    let mut is_jumping = false;

    let mut direction = Vec3::ZERO;

    if action.is_pressed(NetworkAction::ToggleFlyMode) {
        player.is_flying = !player.is_flying;
    }

    player.camera_transform.rotation = action.camera;

    if action.is_pressed(NetworkAction::JumpOrFlyUp) {
        is_jumping = true;
    }

    // Calculate movement directions relative to the camera
    let mut forward = player.camera_transform.forward().xyz();
    forward.y = 0.0;

    let mut right = player.camera_transform.right().xyz();
    right.y = 0.0;

    // Adjust direction based on key presses
    if action.is_pressed(NetworkAction::MoveBackward) {
        direction -= forward;
    }
    if action.is_pressed(NetworkAction::MoveForward) {
        direction += forward;
    }
    if action.is_pressed(NetworkAction::MoveLeft) {
        direction -= right;
    }
    if action.is_pressed(NetworkAction::MoveRight) {
        direction += right;
    }
    if action.is_pressed(NetworkAction::JumpOrFlyUp) {
        direction += Vec3::new(0.0, 1.0, 0.0);
    }
    if action.is_pressed(NetworkAction::SneakOrFlyDown) {
        direction -= Vec3::new(0.0, 1.0, 0.0);
    }

    // Handle jumping (if on the ground) and gravity, only if not flying
    if !player.is_flying {
        if player.on_ground && is_jumping {
            // Player can jump only when grounded
            player.velocity.y = JUMP_VELOCITY * delta;
            player.on_ground = false;
        } else if !player.on_ground {
            // Apply gravity when the player is in the air
            player.velocity.y += GRAVITY * delta;
        }
    }

    let new_y = player.position.y + player.velocity.y;
    let new_vec = &Vec3::new(player.position.x, new_y, player.position.z);

    let max_velocity = 0.9;

    if player.velocity.y > max_velocity {
        player.velocity.y = max_velocity;
    }

    if !player.is_flying {
        if check_player_collision(new_vec, player, &world_clone) {
            player.on_ground = true;
            player.velocity.y = 0.0; // Réinitialiser la vélocité verticale si le joueur est au sol
        } else {
            player.position.y = new_y;
            player.on_ground = false;
        }
    }

    let speed = if player.is_flying { SPEED * 5. } else { SPEED };
    let speed = speed * delta;

    // Attempt to move the player by the calculated direction
    let new_x = player.position.x + direction.x * speed;
    let new_y = player.position.y + direction.y * speed;
    let new_z = player.position.z + direction.z * speed;

    let new_vec = &Vec3::new(new_x, new_y, new_z);
    if check_player_collision(new_vec, player, &world_clone) && !player.is_flying {
        // If a block is detected in the new position, don't move the player
    } else {
        player.position.x = new_x;
        player.position.y = new_y;
        player.position.z = new_z;
    }

    // If the player is below the world, reset their position
    const FALL_LIMIT: f32 = -50.0;
    if player.position.y < FALL_LIMIT {
        player.position = Vec3::new(0.0, 100.0, 0.0);
        player.velocity.y = 0.0;
    }

    let has_moved = player.position != initial_pos;
    let has_rotated = player.camera_transform.rotation != initial_rot;

    let requires_network_broadcast = has_moved || has_rotated;

    requires_network_broadcast
}

trait IsPressed {
    fn is_pressed(&self, action: NetworkAction) -> bool;
}

impl IsPressed for PlayerFrameInput {
    fn is_pressed(&self, action: NetworkAction) -> bool {
        self.inputs.contains(&action)
    }
}
