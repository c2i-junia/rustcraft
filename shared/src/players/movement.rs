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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        messages::PlayerId,
        world::{test_support::TestWorld, BlockId},
    };

    /// Top surface of the test floor.
    const FLOOR_TOP: f32 = 64.0;

    fn flat_world() -> TestWorld {
        let mut world = TestWorld::default();
        world.fill_box(
            IVec3::new(-40, 60, -40),
            IVec3::new(40, 63, 40),
            BlockId::Stone,
        );
        world
    }

    fn standing_player() -> Player {
        let id: PlayerId = 1;
        let mut player = Player::new(
            id,
            "tester".into(),
            Vec3::new(0.5, FLOOR_TOP + 0.9, 0.5),
            Transform::default(),
        );
        player.on_ground = true;
        player
    }

    fn frame(actions: &[NetworkAction]) -> PlayerFrameInput {
        PlayerFrameInput {
            delta_ms: 16,
            inputs: actions.iter().cloned().collect(),
            ..Default::default()
        }
    }

    fn simulate(player: &mut Player, world: &TestWorld, actions: &[NetworkAction], frames: u32) {
        let input = frame(actions);
        for _ in 0..frames {
            simulate_player_movement(player, world, &input);
        }
    }

    #[test]
    fn player_standing_on_the_floor_stays_on_it() {
        let world = flat_world();
        let mut player = standing_player();
        let start = player.position;
        simulate(&mut player, &world, &[], 200);
        assert!(
            (player.position.y - start.y).abs() < 0.05,
            "y drifted from {} to {}",
            start.y,
            player.position.y
        );
        assert_eq!(player.position.x, start.x);
        assert_eq!(player.position.z, start.z);
    }

    #[test]
    fn walking_forward_moves_along_the_camera_direction() {
        let world = flat_world();
        let mut player = standing_player();
        let start = player.position;
        simulate(&mut player, &world, &[NetworkAction::MoveForward], 50);
        // Default camera looks toward -Z: 50 frames * 5 m/s * 0.016 s = 4 m.
        assert!((player.position.z - (start.z - 4.0)).abs() < 1e-3);
        assert!((player.position.x - start.x).abs() < 1e-4);
        assert!((player.position.y - start.y).abs() < 0.05);
    }

    #[test]
    fn walking_follows_a_rotated_camera() {
        let world = flat_world();
        let mut player = standing_player();
        let start = player.position;
        let mut input = frame(&[NetworkAction::MoveForward]);
        // Quarter turn to the left: forward becomes -X.
        input.camera = Transform::from_rotation(Quat::from_rotation_y(std::f32::consts::FRAC_PI_2));
        for _ in 0..50 {
            simulate_player_movement(&mut player, &world, &input);
        }
        assert!((player.position.x - (start.x - 4.0)).abs() < 1e-3);
        assert!((player.position.z - start.z).abs() < 1e-3);
    }

    #[test]
    fn walking_into_a_wall_stops_the_player() {
        let mut world = flat_world();
        world.fill_box(
            IVec3::new(-5, 64, -3),
            IVec3::new(5, 66, -3),
            BlockId::Stone,
        );
        let mut player = standing_player();
        simulate(&mut player, &world, &[NetworkAction::MoveForward], 100);
        // Wall occupies z in [-3, -2); the player's half width is 0.4.
        assert!(
            player.position.z >= -2.0 + 0.4 - 1e-3,
            "z = {}",
            player.position.z
        );
    }

    #[test]
    fn flying_player_ignores_gravity() {
        let world = TestWorld::default();
        let mut player = standing_player();
        player.position.y = 100.0;
        player.is_flying = true;
        simulate(&mut player, &world, &[], 100);
        assert_eq!(player.position.y, 100.0);
    }

    #[test]
    fn flying_player_moves_up_with_jump_input() {
        let world = TestWorld::default();
        let mut player = standing_player();
        player.position.y = 100.0;
        player.is_flying = true;
        simulate(&mut player, &world, &[NetworkAction::JumpOrFlyUp], 10);
        assert!(player.position.y > 100.0);
    }

    #[test]
    fn jump_leaves_the_ground_and_lands_again() {
        let world = flat_world();
        let mut player = standing_player();
        let start_y = player.position.y;

        simulate_player_movement(&mut player, &world, &frame(&[NetworkAction::JumpOrFlyUp]));
        assert!(!player.on_ground);
        assert!(player.position.y > start_y);

        let mut peak = player.position.y;
        for _ in 0..500 {
            simulate_player_movement(&mut player, &world, &frame(&[]));
            peak = peak.max(player.position.y);
        }
        assert!(peak > start_y + 0.5, "peak {peak} too low");
        assert!((player.position.y - start_y).abs() < 0.05);
    }

    #[test]
    #[ignore = "known bug: jump velocity and gravity are scaled by delta, so jump height depends on frame time"]
    fn jump_height_does_not_depend_on_frame_time() {
        let world = flat_world();
        let peak_with_delta = |delta_ms: u64| {
            let mut player = standing_player();
            let start_y = player.position.y;
            let mut jump = frame(&[NetworkAction::JumpOrFlyUp]);
            jump.delta_ms = delta_ms;
            let mut idle = frame(&[]);
            idle.delta_ms = delta_ms;
            simulate_player_movement(&mut player, &world, &jump);
            let mut peak = player.position.y;
            for _ in 0..2000 {
                simulate_player_movement(&mut player, &world, &idle);
                peak = peak.max(player.position.y);
            }
            peak - start_y
        };
        let slow = peak_with_delta(32);
        let fast = peak_with_delta(8);
        assert!((slow - fast).abs() < 0.05, "32 ms: {slow}, 8 ms: {fast}");
    }
}
