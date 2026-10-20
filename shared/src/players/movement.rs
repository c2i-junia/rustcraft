use crate::{
    messages::{NetworkAction, PlayerFrameInput},
    players::{
        collision::{check_player_collision, is_player_in_liquid},
        constants::{
            FLY_SPEED_MULTIPLIER, FLY_SPRINT_SPEED_MULTIPLIER, GRAVITY, JUMP_VELOCITY,
            MAX_UPWARD_VELOCITY, SPEED, SPRINT_SPEED_MULTIPLIER, SWIM_UP_VELOCITY, WATER_DRAG,
            WATER_EXIT_VELOCITY, WATER_GRAVITY, WATER_SPEED_MULTIPLIER,
        },
    },
    world::{world_position_to_chunk_position, ChunkMap},
};
use bevy::prelude::*;

use super::Player;

/// Longest frame the simulation integrates, so a client cannot cover any distance in one input.
pub const MAX_FRAME_DELTA_MS: u64 = 100;

/// Whether the terrain `position` is in and the terrain below it are in `world_map`. A player
/// is not simulated without them: it would fall through chunks that are only missing, not empty.
pub fn is_terrain_loaded(position: Vec3, world_map: &ChunkMap) -> bool {
    let chunk = world_position_to_chunk_position(position);
    world_map.is_chunk_loaded(chunk) && world_map.is_chunk_loaded(chunk - IVec3::Y)
}

/// Below this height a player has fallen out of the world and is put back at `RESPAWN_POSITION`.
const FALL_LIMIT: f32 = -50.0;
const RESPAWN_POSITION: Vec3 = Vec3::new(0.0, 100.0, 0.0);

/// Advances `player` by one input. The client (prediction) and the server (authority) both
/// run it, and they must agree on its result: keep it free of randomness and of wall-clock time.
pub fn simulate_player_movement(
    player: &mut Player,
    world_map: &ChunkMap,
    action: &PlayerFrameInput,
) {
    // Hold the player still until its terrain has arrived
    if !is_terrain_loaded(player.position, world_map) {
        log::debug!(
            "The chunk at {} or the one below is not loaded, skipping movement simulation",
            player.position
        );
        return;
    }

    let delta = action.delta_ms.min(MAX_FRAME_DELTA_MS) as f32 / 1000.0;

    if action.is_pressed(NetworkAction::ToggleFlyMode) {
        player.is_flying = !player.is_flying;
    }
    player.camera_transform = action.camera;

    let direction = wished_direction(action);
    let in_liquid = !player.is_flying && is_player_in_liquid(&player.position, player, world_map);
    if in_liquid {
        swim(player, world_map, action, direction, delta);
    } else {
        apply_vertical_velocity(player, action, delta);
    }
    move_vertically(player, world_map, delta);
    if !in_liquid {
        finish_gravity(player, delta);
    }
    let sprinting =
        action.is_pressed(NetworkAction::Sprint) && action.is_pressed(NetworkAction::MoveForward);
    apply_wished_direction(player, world_map, direction, in_liquid, sprinting, delta);

    if player.position.y < FALL_LIMIT {
        player.position = RESPAWN_POSITION;
        player.velocity.y = 0.0;
    }
}

/// Where the pressed keys ask to go: horizontally relative to the camera, and straight up or
/// down for the fly keys. The horizontal part has length 1 or 0, so diagonals are not faster.
fn wished_direction(action: &PlayerFrameInput) -> Vec3 {
    // Looking exactly up or down leaves no horizontal component: no horizontal movement then
    let horizontal = |direction: Dir3| direction.xyz().with_y(0.0).normalize_or_zero();
    let forward = horizontal(action.camera.forward());
    let right = horizontal(action.camera.right());

    let mut direction = Vec3::ZERO;
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
    if direction != Vec3::ZERO {
        direction = direction.normalize();
    }

    if action.is_pressed(NetworkAction::JumpOrFlyUp) {
        direction += Vec3::Y;
    }
    if action.is_pressed(NetworkAction::SneakOrFlyDown) {
        direction -= Vec3::Y;
    }
    direction
}

/// Jump and the first half of the gravity of the frame. A flying player has no vertical
/// velocity: it only moves up or down by its wished direction.
///
/// The gravity is added half before the move and half after it, so that the move goes at the
/// mean velocity of the frame: the height of a jump then does not depend on the frame time.
fn apply_vertical_velocity(player: &mut Player, action: &PlayerFrameInput, delta: f32) {
    if player.is_flying {
        return;
    }
    if player.on_ground && action.is_pressed(NetworkAction::JumpOrFlyUp) {
        player.velocity.y = JUMP_VELOCITY;
        player.on_ground = false;
    }
    if !player.on_ground {
        player.velocity.y += GRAVITY * delta / 2.0;
    }
}

/// The second half of the gravity of the frame, for a player still in the air after its move.
fn finish_gravity(player: &mut Player, delta: f32) {
    if !player.is_flying && !player.on_ground {
        player.velocity.y += GRAVITY * delta / 2.0;
    }
}

/// The vertical velocity of a player in a liquid: a weak gravity braked by a drag, and jump
/// held to swim up. Pushing against a block while swimming up lifts the player out of the
/// water, so that it can climb onto the shore.
fn swim(
    player: &mut Player,
    world_map: &ChunkMap,
    action: &PlayerFrameInput,
    direction: Vec3,
    delta: f32,
) {
    // A linear drag, without `exp`: the client and the server must compute the same value
    player.velocity.y += (WATER_GRAVITY - WATER_DRAG * player.velocity.y) * delta;
    if action.is_pressed(NetworkAction::JumpOrFlyUp) {
        let ahead = player.position + direction.with_y(0.0) * 0.1;
        let against_a_block = check_player_collision(&ahead, player, world_map);
        let up = if against_a_block {
            WATER_EXIT_VELOCITY
        } else {
            SWIM_UP_VELOCITY
        };
        player.velocity.y = player.velocity.y.max(up);
    }
}

/// Moves the player by its vertical velocity, and lands it on the block in the way.
fn move_vertically(player: &mut Player, world_map: &ChunkMap, delta: f32) {
    player.velocity.y = player.velocity.y.min(MAX_UPWARD_VELOCITY);

    let new_y = player.position.y + player.velocity.y * delta;
    if player.is_flying {
        player.velocity.y = 0.0;
        player.position.y = new_y;
        player.on_ground = false;
    } else if check_player_collision(&player.position.with_y(new_y), player, world_map) {
        player.on_ground = true;
        player.velocity.y = 0.0;
    } else {
        player.position.y = new_y;
        player.on_ground = false;
    }
}

/// Moves the player along `direction`. On foot, an axis blocked by a block is left unchanged,
/// so the player slides along walls; a flying player goes through blocks. A sprint speeds up
/// a player on foot or in flight, not a swimmer.
fn apply_wished_direction(
    player: &mut Player,
    world_map: &ChunkMap,
    direction: Vec3,
    in_liquid: bool,
    sprinting: bool,
    delta: f32,
) {
    let speed = match (player.is_flying, in_liquid, sprinting) {
        (true, _, true) => SPEED * FLY_SPEED_MULTIPLIER * FLY_SPRINT_SPEED_MULTIPLIER,
        (true, _, false) => SPEED * FLY_SPEED_MULTIPLIER,
        (false, true, _) => SPEED * WATER_SPEED_MULTIPLIER,
        (false, false, true) => SPEED * SPRINT_SPEED_MULTIPLIER,
        (false, false, false) => SPEED,
    };
    let step = direction * (speed * delta);

    // Both axes are tested from the position before the move
    let moved_x = player.position.with_x(player.position.x + step.x);
    let moved_z = player.position.with_z(player.position.z + step.z);
    if player.is_flying || !check_player_collision(&moved_x, player, world_map) {
        player.position.x = moved_x.x;
    }
    if player.is_flying || !check_player_collision(&moved_z, player, world_map) {
        player.position.z = moved_z.z;
    }

    if player.is_flying {
        player.position.y += step.y;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        messages::PlayerId,
        players::{
            constants::{WATER_DRAG, WATER_GRAVITY, WATER_SPEED_MULTIPLIER},
            PLAYER_HEIGHT,
        },
        world::{world_position_to_chunk_position, BlockId, Chunk},
    };

    /// Top surface of the test floor.
    const FLOOR_TOP: f32 = 64.0;

    /// Loaded chunks from y = 64 up, with nothing in them.
    fn open_air() -> ChunkMap {
        ChunkMap::with_loaded_region(IVec3::new(-16, 64, -16), IVec3::new(15, 127, 15))
    }

    /// A stone floor from y = 60 to y = 63, in chunks loaded up to y = 95.
    fn flat_world() -> ChunkMap {
        let mut world =
            ChunkMap::with_loaded_region(IVec3::new(-40, 48, -40), IVec3::new(40, 95, 40));
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
            ..default()
        }
    }

    fn simulate(player: &mut Player, world: &ChunkMap, actions: &[NetworkAction], frames: u32) {
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
    fn frame_delta_is_clamped_to_the_maximum() {
        let world = flat_world();
        let walk = |delta_ms: u64| {
            let mut player = standing_player();
            let mut input = frame(&[NetworkAction::MoveForward]);
            input.delta_ms = delta_ms;
            simulate_player_movement(&mut player, &world, &input);
            player.position
        };
        let at_limit = walk(MAX_FRAME_DELTA_MS);
        assert_ne!(at_limit, standing_player().position);
        assert_eq!(walk(MAX_FRAME_DELTA_MS + 1), at_limit);
        assert_eq!(walk(u64::MAX), at_limit);
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
    fn sprinting_forward_is_faster_than_walking() {
        let world = flat_world();
        let mut player = standing_player();
        let start = player.position;
        simulate(
            &mut player,
            &world,
            &[NetworkAction::MoveForward, NetworkAction::Sprint],
            50,
        );
        let expected = 4.0 * SPRINT_SPEED_MULTIPLIER;
        assert!((start.z - player.position.z - expected).abs() < 1e-3);
    }

    #[test]
    fn sprinting_without_moving_forward_does_not_speed_up() {
        let world = flat_world();
        let mut player = standing_player();
        let start = player.position;
        simulate(
            &mut player,
            &world,
            &[NetworkAction::MoveBackward, NetworkAction::Sprint],
            50,
        );
        assert!((player.position.z - start.z - 4.0).abs() < 1e-3);
    }

    #[test]
    fn a_sprinting_flyer_goes_twice_as_fast() {
        let world = open_air();
        let mut player = standing_player();
        player.position.y = 100.0;
        player.is_flying = true;
        let start = player.position;
        simulate(
            &mut player,
            &world,
            &[NetworkAction::MoveForward, NetworkAction::Sprint],
            20,
        );
        // 20 frames of 16 ms at 5 * 4 * 2 blocks/s, staying in the loaded chunks
        let expected = 0.32 * SPEED * FLY_SPEED_MULTIPLIER * FLY_SPRINT_SPEED_MULTIPLIER;
        assert!((start.z - player.position.z - expected).abs() < 1e-2);
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
        let world = open_air();
        let mut player = standing_player();
        player.position.y = 100.0;
        player.is_flying = true;
        simulate(&mut player, &world, &[], 100);
        assert_eq!(player.position.y, 100.0);
    }

    #[test]
    fn flying_player_moves_up_with_jump_input() {
        let world = open_air();
        let mut player = standing_player();
        player.position.y = 100.0;
        player.is_flying = true;
        simulate(&mut player, &world, &[NetworkAction::JumpOrFlyUp], 10);
        assert!(player.position.y > 100.0);
    }

    /// Apex height above the standing position reached by a jump at the given frame time.
    fn jump_apex(world: &ChunkMap, delta_ms: u64) -> f32 {
        let mut player = standing_player();
        let start_y = player.position.y;
        let mut jump = frame(&[NetworkAction::JumpOrFlyUp]);
        jump.delta_ms = delta_ms;
        let mut idle = frame(&[]);
        idle.delta_ms = delta_ms;
        simulate_player_movement(&mut player, world, &jump);
        let mut peak = player.position.y;
        for _ in 0..3000 {
            simulate_player_movement(&mut player, world, &idle);
            peak = peak.max(player.position.y);
        }
        assert!(
            (player.position.y - start_y).abs() < 0.25,
            "did not land again at {delta_ms} ms: y = {}",
            player.position.y
        );
        peak - start_y
    }

    #[test]
    fn jump_leaves_the_ground_and_lands_again() {
        let world = flat_world();
        let mut player = standing_player();
        let start_y = player.position.y;

        simulate_player_movement(&mut player, &world, &frame(&[NetworkAction::JumpOrFlyUp]));
        assert!(!player.on_ground);
        assert!(player.position.y > start_y);

        // As in Minecraft: v² / 2g = 8.66² / 60 = 1.25 blocks, enough to climb onto a block
        let apex = jump_apex(&world, 16);
        assert!((apex - 1.25).abs() < 0.02, "apex {apex}");
    }

    #[test]
    fn a_jump_climbs_onto_a_block_but_not_onto_two() {
        for (height, climbed) in [(1, true), (2, false)] {
            let mut world = flat_world();
            world.fill_box(
                IVec3::new(-5, 64, -10),
                IVec3::new(5, 63 + height, -3),
                BlockId::Stone,
            );
            let mut player = standing_player();
            simulate(
                &mut player,
                &world,
                &[NetworkAction::MoveForward, NetworkAction::JumpOrFlyUp],
                100,
            );
            assert_eq!(
                player.position.z < -3.0,
                climbed,
                "{height} block(s): z = {}",
                player.position.z
            );
        }
    }

    #[test]
    fn jump_height_does_not_depend_on_frame_time() {
        let world = flat_world();
        let reference = jump_apex(&world, 16);
        for delta_ms in [7, 33] {
            let apex = jump_apex(&world, delta_ms);
            assert!(
                (apex - reference).abs() < reference * 0.1,
                "{delta_ms} ms: {apex}, 16 ms: {reference}"
            );
        }
    }

    #[test]
    fn player_does_not_move_while_its_chunk_is_not_loaded() {
        // The floor is loaded up to y = 95: the chunk of a player at y = 100 is missing.
        let mut world = flat_world();
        let mut player = standing_player();
        player.position.y = 100.0;
        let chunk = world_position_to_chunk_position(player.position);
        assert!(!world.is_chunk_loaded(chunk));
        let start = player.position;
        simulate(&mut player, &world, &[NetworkAction::MoveForward], 20);
        assert_eq!(player.position, start);

        world.insert_chunk(chunk, Chunk::new());
        simulate(&mut player, &world, &[NetworkAction::MoveForward], 20);
        assert_ne!(player.position, start);
    }

    #[test]
    fn player_does_not_move_while_the_chunk_below_is_not_loaded() {
        let mut player = standing_player();
        player.position.y = 100.0;
        let chunk = world_position_to_chunk_position(player.position);
        let mut world = ChunkMap::with_loaded_region(IVec3::new(-16, 96, -16), IVec3::splat(111));
        assert!(world.is_chunk_loaded(chunk) && !world.is_chunk_loaded(chunk - IVec3::Y));
        let start = player.position;
        simulate(&mut player, &world, &[], 20);
        assert_eq!(player.position, start);

        world.insert_chunk(chunk - IVec3::Y, Chunk::new());
        simulate(&mut player, &world, &[], 20);
        assert!(player.position.y < start.y);
    }

    #[test]
    fn looking_straight_up_or_down_keeps_the_position_finite() {
        let world = flat_world();
        for direction in [Vec3::Y, Vec3::NEG_Y] {
            for up in [Vec3::Z, Vec3::NEG_Z, Vec3::X] {
                let mut player = standing_player();
                let mut input = frame(&[NetworkAction::MoveForward, NetworkAction::MoveLeft]);
                input.camera = Transform::default().looking_to(direction, up);
                simulate_player_movement(&mut player, &world, &input);
                assert!(
                    player.position.is_finite(),
                    "looking {direction:?} (up {up:?}): {:?}",
                    player.position
                );
            }
        }
    }

    /// Top of the water of `pool`.
    const WATER_TOP: f32 = 68.0;

    /// `flat_world` under four blocks of water, from y = 64 to y = 67.
    fn pool() -> ChunkMap {
        let mut world = flat_world();
        world.fill_box(
            IVec3::new(-40, 64, -40),
            IVec3::new(40, 67, 40),
            BlockId::Water,
        );
        world
    }

    /// A player whose whole body is under water, in the middle of `pool`.
    fn diver() -> Player {
        let mut player = standing_player();
        player.position.y = 66.0;
        player.on_ground = false;
        player
    }

    #[test]
    fn a_player_sinks_slowly_in_water() {
        let world = pool();
        let mut player = diver();
        // Half a second: a fall in the air would cover 3.75 blocks
        simulate(&mut player, &world, &[], 31);
        assert!(
            player.position.y > 65.0,
            "sank to {} in half a second",
            player.position.y
        );
        assert!(player.position.y < 66.0);
        // Never faster than where the drag balances the gravity
        assert!(player.velocity.y > WATER_GRAVITY / WATER_DRAG - 0.01);
    }

    #[test]
    fn holding_jump_in_water_swims_up_to_the_surface() {
        let world = pool();
        let mut player = standing_player();
        // From the bottom of the pool
        simulate(&mut player, &world, &[NetworkAction::JumpOrFlyUp], 120);
        assert!(
            player.position.y - PLAYER_HEIGHT / 2.0 > WATER_TOP - 1.0,
            "feet at {}",
            player.position.y - PLAYER_HEIGHT / 2.0
        );
        // It bobs at the surface, it does not jump out of the water
        let mut highest: f32 = 0.0;
        for _ in 0..300 {
            simulate(&mut player, &world, &[NetworkAction::JumpOrFlyUp], 1);
            highest = highest.max(player.position.y - PLAYER_HEIGHT / 2.0);
        }
        assert!(highest < WATER_TOP + 0.5, "feet up to {highest}");
    }

    #[test]
    fn water_slows_down_a_fall() {
        let world = pool();
        let mut player = diver();
        player.position.y = WATER_TOP + 0.8;
        player.velocity.y = -30.0;
        simulate(&mut player, &world, &[], 15);
        assert!(player.velocity.y > -15.0, "still at {}", player.velocity.y);
    }

    #[test]
    fn a_player_walks_slower_in_water() {
        let world = pool();
        let mut player = standing_player();
        let start = player.position;
        simulate(&mut player, &world, &[NetworkAction::MoveForward], 50);
        let expected = 4.0 * WATER_SPEED_MULTIPLIER;
        assert!((start.z - player.position.z - expected).abs() < 1e-3);
    }

    #[test]
    fn a_swimmer_climbs_onto_a_shore_one_block_above_the_water() {
        let mut world = pool();
        // The shore: from z = -10 toward -Z, its top one block above the water
        world.fill_box(
            IVec3::new(-40, 64, -40),
            IVec3::new(40, 68, -10),
            BlockId::Stone,
        );
        let mut player = diver();
        simulate(
            &mut player,
            &world,
            &[NetworkAction::MoveForward, NetworkAction::JumpOrFlyUp],
            300,
        );
        assert!(
            player.position.z < -10.0,
            "stuck at z = {}",
            player.position.z
        );
        // Jump held on the shore jumps: let the player land
        simulate(&mut player, &world, &[], 120);
        assert!(player.on_ground);
        assert!((player.position.y - PLAYER_HEIGHT / 2.0 - 69.0).abs() < 0.05);
    }

    #[test]
    fn a_flying_player_ignores_the_water() {
        let world = pool();
        let mut player = diver();
        player.is_flying = true;
        let start = player.position;
        simulate(&mut player, &world, &[NetworkAction::MoveForward], 50);
        assert_eq!(player.position.y, start.y);
        assert!((start.z - player.position.z - 4.0 * FLY_SPEED_MULTIPLIER).abs() < 1e-3);
    }
}
