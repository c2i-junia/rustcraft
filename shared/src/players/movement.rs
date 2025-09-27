use crate::{
    messages::{NetworkAction, PlayerFrameInput},
    players::{
        collision::check_player_collision,
        constants::{FLY_SPEED_MULTIPLIER, GRAVITY, JUMP_VELOCITY, SPEED},
    },
    world::WorldMap,
};
use bevy::prelude::*;

use super::Player;

pub fn simulate_player_movement(
    player: &mut Player,
    world_map: &impl WorldMap,
    action: &PlayerFrameInput,
) {
    // let's check if the 9 chunks around the player are loaded
    let chunks = world_map.get_surrounding_chunks(player.position, 1);
    if chunks.len() < 9 {
        log::debug!("Not enough chunks loaded, skipping movement simulation");
        return;
    }

    let delta = action.delta_ms as f32 / 1000.0;

    let mut direction = Vec3::ZERO;

    if action.is_pressed(NetworkAction::ToggleFlyMode) {
        player.is_flying = !player.is_flying;
    }

    player.camera_transform = action.camera;

    let is_jumping = action.is_pressed(NetworkAction::JumpOrFlyUp);

    // Calculate movement directions relative to the camera
    let forward = player
        .camera_transform
        .forward()
        .xyz()
        .with_y(0.0)
        .normalize();

    let right = player
        .camera_transform
        .right()
        .xyz()
        .with_y(0.0)
        .normalize();

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

    // Normalize direction to prevent faster movement with diagonals
    if direction != Vec3::ZERO {
        direction = direction.normalize();
    }

    if action.is_pressed(NetworkAction::JumpOrFlyUp) {
        direction += Vec3::Y;
    }
    if action.is_pressed(NetworkAction::SneakOrFlyDown) {
        direction -= Vec3::Y;
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

    let max_velocity = 0.9;

    if player.velocity.y > max_velocity {
        player.velocity.y = max_velocity;
    }

    let new_y = player.position.y + player.velocity.y;
    let new_vec = &Vec3::new(player.position.x, new_y, player.position.z);

    if !player.is_flying {
        if check_player_collision(new_vec, player, world_map) {
            player.on_ground = true;
            player.velocity.y = 0.0;
        } else {
            player.position.y = new_y;
            player.on_ground = false;
        }
    } else {
        player.velocity.y = 0.0;
        player.position.y = new_y;
        player.on_ground = false;
    }

    let speed = if player.is_flying {
        SPEED * FLY_SPEED_MULTIPLIER
    } else {
        SPEED
    };
    let speed = speed * delta;

    // Attempt to move the player by the calculated direction
    let new_x = player.position.x + direction.x * speed;
    let new_z = player.position.z + direction.z * speed;

    let new_vec_x = &player.position.with_x(new_x);
    let new_vec_z = &player.position.with_z(new_z);

    // If a block is detected in the new position, don't move the player on this axis
    if check_player_collision(new_vec_x, player, world_map) && !player.is_flying {
    } else {
        player.position.x = new_x;
    }

    if check_player_collision(new_vec_z, player, world_map) && !player.is_flying {
    } else {
        player.position.z = new_z;
    }

    if player.is_flying {
        player.position.y += direction.y * speed;
    }

    // If the player is below the world, reset their position
    const FALL_LIMIT: f32 = -50.0;
    if player.position.y < FALL_LIMIT {
        player.position = Vec3::new(0.0, 100.0, 0.0);
        player.velocity.y = 0.0;
    }
}

trait IsPressed {
    fn is_pressed(&self, action: NetworkAction) -> bool;
}

impl IsPressed for PlayerFrameInput {
    fn is_pressed(&self, action: NetworkAction) -> bool {
        self.inputs.contains(&action)
    }
}
