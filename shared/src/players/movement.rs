use crate::{
    messages::{NetworkAction, PlayerFrameInput},
    players::{
        collision::{check_player_collision, is_player_in_liquid},
        constants::{
            FLY_SPEED_MULTIPLIER, FLY_SPRINT_SPEED_MULTIPLIER, GRAVITY, JUMP_VELOCITY,
            MAX_UPWARD_VELOCITY, SNEAK_GROUND_PROBE, SNEAK_SPEED_MULTIPLIER, SPEED,
            SPRINT_SPEED_MULTIPLIER, SWIM_DOWN_VELOCITY, SWIM_SPEED, SWIM_UP_VELOCITY, WATER_DRAG,
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

/// Below this height a player has fallen out of the world: the server takes it back to the
/// spawn, or kills it in survival.
pub const FALL_LIMIT: f32 = -50.0;

/// Room left between a player and the block it lands on or bumps into: a box that touches a
/// cell to the last bit could be read as inside it.
const CONTACT_GAP: f32 = 1e-3;

/// Longest vertical move tested at once, in blocks, so that a fast fall cannot cross a block.
const MAX_VERTICAL_STEP: f32 = 0.5;

/// Advances `player` by one input. The client (prediction) and the server (authority) both
/// run it, and they must agree on its result: keep it free of randomness and of wall-clock time.
///
/// Returns how many blocks the player fell when it lands on the ground in this input, for the
/// server to hurt it.
pub fn simulate_player_movement(
    player: &mut Player,
    world_map: &ChunkMap,
    action: &PlayerFrameInput,
) -> Option<f32> {
    // A dead player waits for its respawn
    if player.is_dead() {
        return None;
    }
    // Hold the player still until its terrain has arrived
    if !is_terrain_loaded(player.position, world_map) {
        log::debug!(
            "The chunk at {} or the one below is not loaded, skipping movement simulation",
            player.position
        );
        return None;
    }

    let delta = action.delta_ms.min(MAX_FRAME_DELTA_MS) as f32 / 1000.0;

    if action.is_pressed(NetworkAction::ToggleFlyMode) {
        player.is_flying = !player.is_flying;
    }
    player.is_flying &= player.game_mode.can_fly();
    player.camera_transform = action.camera;

    let in_liquid = !player.is_flying && is_player_in_liquid(&player.position, player, world_map);
    if in_liquid || player.is_flying {
        player.fall_distance = 0.0;
    }
    update_stance(player, action, in_liquid);
    let direction = wished_direction(player, action);
    if in_liquid {
        swim(player, world_map, action, direction, delta);
    } else {
        apply_vertical_velocity(player, action, delta);
    }
    let landed = move_vertically(player, world_map, delta);
    if !in_liquid {
        finish_gravity(player, delta);
    }
    apply_wished_direction(player, world_map, direction, in_liquid, delta);
    landed
}

/// Whether the player sneaks, sprints and swims in this input. Sprinting starts with the sprint
/// key while moving forward, and lasts as long as the player moves forward, the key released,
/// until it sneaks. A sprint in a liquid is a swim. Sneak is the fly down key in flight and the
/// swim down key in a liquid: the player only sneaks on foot.
fn update_stance(player: &mut Player, action: &PlayerFrameInput, in_liquid: bool) {
    let forward = action.is_pressed(NetworkAction::MoveForward)
        && !action.is_pressed(NetworkAction::MoveBackward);
    player.is_sneaking =
        action.is_pressed(NetworkAction::SneakOrFlyDown) && !player.is_flying && !in_liquid;
    player.is_sprinting = forward
        && !player.is_sneaking
        && (player.is_sprinting || action.is_pressed(NetworkAction::Sprint));
    player.is_swimming = in_liquid && player.is_sprinting;
}

/// Where the pressed keys ask to go: horizontally relative to the camera, and straight up or
/// down for the fly keys. The horizontal part has length 1 or 0, so diagonals are not faster.
/// A swimmer goes where the camera looks, up and down included, and the keys add no height.
fn wished_direction(player: &Player, action: &PlayerFrameInput) -> Vec3 {
    // Looking exactly up or down leaves no horizontal component: no horizontal movement then
    let horizontal = |direction: Dir3| direction.xyz().with_y(0.0).normalize_or_zero();
    let forward = if player.is_swimming {
        action.camera.forward().xyz()
    } else {
        horizontal(action.camera.forward())
    };
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
    direction = direction.normalize_or_zero();
    if player.is_swimming {
        return direction;
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
/// velocity: it only moves up or down by its wished direction. The gravity pulls a player
/// standing on the ground too, which keeps it in contact with the ground, and tells at once
/// when there is none left under it.
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
    player.velocity.y += GRAVITY * delta / 2.0;
}

/// The second half of the gravity of the frame, for a player still in the air after its move.
fn finish_gravity(player: &mut Player, delta: f32) {
    if !player.is_flying && !player.on_ground {
        player.velocity.y += GRAVITY * delta / 2.0;
    }
}

/// The vertical velocity of a player in a liquid: a weak gravity braked by a drag, jump held to
/// swim up and sneak held to swim down. A swimmer neither sinks nor floats: it rises or dives
/// where it looks. Pushing against a block while swimming up lifts the player out of the
/// water, so that it can climb onto the shore.
fn swim(
    player: &mut Player,
    world_map: &ChunkMap,
    action: &PlayerFrameInput,
    direction: Vec3,
    delta: f32,
) {
    if player.is_swimming {
        player.velocity.y = direction.y * SWIM_SPEED;
    } else {
        // A linear drag, without `exp`: the client and the server must compute the same value
        player.velocity.y += (WATER_GRAVITY - WATER_DRAG * player.velocity.y) * delta;
        if action.is_pressed(NetworkAction::SneakOrFlyDown) {
            player.velocity.y = player.velocity.y.min(SWIM_DOWN_VELOCITY);
        }
    }
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

/// Moves the player by its vertical velocity, and stops it against the block in the way: on
/// top of it when falling, which lands the player, right under it when rising, from where it
/// falls back. Returns the blocks fallen when the player lands.
fn move_vertically(player: &mut Player, world_map: &ChunkMap, delta: f32) -> Option<f32> {
    player.velocity.y = player.velocity.y.min(MAX_UPWARD_VELOCITY);
    let distance = player.velocity.y * delta;

    if player.is_flying {
        player.velocity.y = 0.0;
        player.position.y += distance;
        player.on_ground = false;
        return None;
    }

    let was_on_ground = player.on_ground;
    let steps = (distance.abs() / MAX_VERTICAL_STEP).ceil().max(1.0);
    let step = distance / steps;
    let half_height = player.height / 2.0;
    for _ in 0..steps as u32 {
        let new_y = player.position.y + step;
        if !check_player_collision(&player.position.with_y(new_y), player, world_map) {
            player.position.y = new_y;
            player.fall_distance += (-step).max(0.0);
            continue;
        }

        // The blocks are whole cells: the player is put against the one it ran into
        let falling = step < 0.0;
        let contact = if falling {
            (new_y - half_height).floor() + 1.0 + half_height + CONTACT_GAP
        } else {
            (new_y + half_height).floor() - half_height - CONTACT_GAP
        };
        let (low, high) = if falling {
            (new_y, player.position.y)
        } else {
            (player.position.y, new_y)
        };
        if (low..=high).contains(&contact)
            && !check_player_collision(&player.position.with_y(contact), player, world_map)
        {
            player.fall_distance += (player.position.y - contact).max(0.0);
            player.position.y = contact;
        }
        player.velocity.y = 0.0;
        player.on_ground = falling;
        if !falling {
            return None;
        }
        let fallen = std::mem::take(&mut player.fall_distance);
        return (!was_on_ground).then_some(fallen);
    }
    player.on_ground = false;
    None
}

/// The horizontal speed of the player, in blocks/s.
fn speed(player: &Player, in_liquid: bool) -> f32 {
    if player.is_flying {
        let sprint = if player.is_sprinting {
            FLY_SPRINT_SPEED_MULTIPLIER
        } else {
            1.0
        };
        SPEED * FLY_SPEED_MULTIPLIER * sprint
    } else if player.is_swimming {
        SWIM_SPEED
    } else if in_liquid {
        SPEED * WATER_SPEED_MULTIPLIER
    } else if player.is_sneaking {
        SPEED * SNEAK_SPEED_MULTIPLIER
    } else if player.is_sprinting {
        SPEED * SPRINT_SPEED_MULTIPLIER
    } else {
        SPEED
    }
}

/// Moves the player along `direction`, along x then along z. On foot, an axis blocked by a
/// block is left unchanged, so the player slides along walls, and a player sneaking on the
/// ground does not step off it; a flying player goes through blocks.
fn apply_wished_direction(
    player: &mut Player,
    world_map: &ChunkMap,
    direction: Vec3,
    in_liquid: bool,
    delta: f32,
) {
    let step = direction * (speed(player, in_liquid) * delta);
    let keeps_to_the_ground = player.is_sneaking && player.on_ground;

    for axis in [Vec3::X, Vec3::Z] {
        let moved = player.position + step * axis;
        if !player.is_flying {
            if check_player_collision(&moved, player, world_map) {
                continue;
            }
            let below = moved - Vec3::Y * SNEAK_GROUND_PROBE;
            if keeps_to_the_ground && !check_player_collision(&below, player, world_map) {
                continue;
            }
        }
        player.position = moved;
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
            game_mode::GameMode,
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

    /// A player in creative mode, flying at y = 100.
    fn flyer() -> Player {
        let mut player = standing_player();
        player.game_mode = GameMode::Creative;
        player.is_flying = true;
        player.on_ground = false;
        player.position.y = 100.0;
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
        let mut player = flyer();
        let start = player.position;
        simulate(
            &mut player,
            &world,
            &[NetworkAction::MoveForward, NetworkAction::Sprint],
            20,
        );
        // 20 frames of 16 ms at 5 * 4 * 2 blocks/s, staying in the loaded chunks
        let expected = 0.32 * SPEED * FLY_SPEED_MULTIPLIER * FLY_SPRINT_SPEED_MULTIPLIER;
        let moved = start.z - player.position.z;
        assert!(
            (moved - expected).abs() < 1e-2,
            "{moved} instead of {expected}"
        );
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
        let mut player = flyer();
        simulate(&mut player, &world, &[], 100);
        assert_eq!(player.position.y, 100.0);
    }

    #[test]
    fn flying_player_moves_up_with_jump_input() {
        let world = open_air();
        let mut player = flyer();
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
        let mut player = flyer();
        player.position = diver().position;
        let start = player.position;
        simulate(&mut player, &world, &[NetworkAction::MoveForward], 50);
        assert_eq!(player.position.y, start.y);
        assert!((start.z - player.position.z - 4.0 * FLY_SPEED_MULTIPLIER).abs() < 1e-3);
    }

    /// Feet of a player standing on the floor of `flat_world`.
    fn feet(player: &Player) -> f32 {
        player.position.y - PLAYER_HEIGHT / 2.0
    }

    #[test]
    fn a_player_standing_on_the_floor_is_on_the_ground_at_every_frame() {
        let world = flat_world();
        let mut player = standing_player();
        for _ in 0..100 {
            assert_eq!(
                simulate_player_movement(&mut player, &world, &frame(&[])),
                None
            );
            assert!(player.on_ground);
            assert!((feet(&player) - FLOOR_TOP).abs() < 0.01);
        }
    }

    #[test]
    fn a_jump_reaches_the_top_of_one_block_but_not_of_two() {
        let mut world = flat_world();
        // A step of one block from z = -2 toward -Z, and a wall of two from z = -6
        world.fill_box(
            IVec3::new(-5, 64, -40),
            IVec3::new(5, 64, -2),
            BlockId::Stone,
        );
        world.fill_box(
            IVec3::new(-5, 65, -40),
            IVec3::new(5, 66, -6),
            BlockId::Stone,
        );
        let mut player = standing_player();
        let actions = [NetworkAction::MoveForward, NetworkAction::JumpOrFlyUp];
        simulate(&mut player, &world, &actions, 200);
        simulate(&mut player, &world, &[], 60);
        assert!(
            (feet(&player) - 65.0).abs() < 0.01,
            "feet at {}",
            feet(&player)
        );
        assert!(
            player.position.z > -5.0 + 0.4 - 1e-3,
            "climbed the wall: z = {}",
            player.position.z
        );
    }

    #[test]
    fn holding_jump_under_a_block_does_not_stick_the_player_to_it() {
        let mut world = flat_world();
        // A ceiling 1.2 blocks above the head, lower than a jump
        world.fill_box(IVec3::new(-5, 67, -5), IVec3::new(5, 67, 5), BlockId::Stone);
        let mut player = standing_player();
        let jump = frame(&[NetworkAction::JumpOrFlyUp]);
        let mut frames_on_the_floor = 0;
        let mut highest_head: f32 = 0.0;
        for _ in 0..300 {
            simulate_player_movement(&mut player, &world, &jump);
            highest_head = highest_head.max(player.position.y + PLAYER_HEIGHT / 2.0);
            if (feet(&player) - FLOOR_TOP).abs() < 0.01 {
                frames_on_the_floor += 1;
            }
        }
        assert!(highest_head < 67.0, "head in the ceiling: {highest_head}");
        assert!(highest_head > 66.9, "the jump did not reach the ceiling");
        // Each jump falls back down: before the fix, the player stayed under the ceiling
        assert!(
            frames_on_the_floor >= 5,
            "{frames_on_the_floor} frames on the floor"
        );
    }

    #[test]
    fn bumping_into_a_ceiling_is_not_landing() {
        let mut world = flat_world();
        world.fill_box(IVec3::new(-5, 67, -5), IVec3::new(5, 67, 5), BlockId::Stone);
        let mut player = standing_player();
        simulate(&mut player, &world, &[NetworkAction::JumpOrFlyUp], 1);
        for _ in 0..30 {
            simulate(&mut player, &world, &[], 1);
            if player.velocity.y == 0.0 && feet(&player) > FLOOR_TOP + 0.5 {
                assert!(!player.on_ground, "on the ground under the ceiling");
            }
        }
    }

    #[test]
    fn a_fast_fall_does_not_cross_a_block() {
        let mut world = flat_world();
        world.fill_box(IVec3::new(-5, 80, -5), IVec3::new(5, 80, 5), BlockId::Stone);
        let mut player = standing_player();
        player.position.y = 90.0;
        player.on_ground = false;
        player.velocity.y = -60.0;
        let mut input = frame(&[]);
        input.delta_ms = MAX_FRAME_DELTA_MS;
        for _ in 0..20 {
            simulate_player_movement(&mut player, &world, &input);
        }
        assert!(
            (feet(&player) - 81.0).abs() < 0.01,
            "feet at {}",
            feet(&player)
        );
    }

    fn walked(player: &Player, start: Vec3) -> f32 {
        start.xz().distance(player.position.xz())
    }

    #[test]
    fn sprinting_is_faster_than_walking() {
        let world = flat_world();
        let mut player = standing_player();
        let start = player.position;
        simulate(
            &mut player,
            &world,
            &[NetworkAction::MoveForward, NetworkAction::Sprint],
            50,
        );
        assert!(player.is_sprinting);
        // 50 frames * 5 m/s * 0.016 s = 4 m on foot
        assert!((walked(&player, start) - 4.0 * SPRINT_SPEED_MULTIPLIER).abs() < 1e-3);
    }

    #[test]
    fn a_sprint_lasts_while_moving_forward_and_stops_with_it() {
        let world = flat_world();
        let mut player = standing_player();
        simulate(
            &mut player,
            &world,
            &[NetworkAction::MoveForward, NetworkAction::Sprint],
            1,
        );
        let start = player.position;
        simulate(&mut player, &world, &[NetworkAction::MoveForward], 50);
        assert!(player.is_sprinting, "the sprint stopped with its key");
        assert!((walked(&player, start) - 4.0 * SPRINT_SPEED_MULTIPLIER).abs() < 1e-3);

        simulate(&mut player, &world, &[], 1);
        assert!(!player.is_sprinting);
        let start = player.position;
        simulate(&mut player, &world, &[NetworkAction::MoveForward], 50);
        assert!((walked(&player, start) - 4.0).abs() < 1e-3);
    }

    #[test]
    fn a_player_sprints_only_forward_and_not_while_sneaking() {
        let world = flat_world();
        for actions in [
            [NetworkAction::MoveLeft, NetworkAction::Sprint],
            [NetworkAction::MoveBackward, NetworkAction::Sprint],
        ] {
            let mut player = standing_player();
            simulate(&mut player, &world, &actions, 10);
            assert!(!player.is_sprinting, "{actions:?}");
        }
        let mut player = standing_player();
        let actions = [
            NetworkAction::MoveForward,
            NetworkAction::Sprint,
            NetworkAction::SneakOrFlyDown,
        ];
        simulate(&mut player, &world, &actions, 10);
        assert!(!player.is_sprinting && player.is_sneaking);
    }

    #[test]
    fn sprinting_in_flight_flies_faster() {
        let world = open_air();
        let mut player = flyer();
        let start = player.position;
        simulate(
            &mut player,
            &world,
            &[NetworkAction::MoveForward, NetworkAction::Sprint],
            20,
        );
        // 20 frames * 5 m/s * 0.016 s = 1.6 m on foot; the loaded chunks end at z = -16
        let expected = 1.6 * FLY_SPEED_MULTIPLIER * FLY_SPRINT_SPEED_MULTIPLIER;
        assert!((walked(&player, start) - expected).abs() < 1e-2);
        assert_eq!(player.position.y, start.y);
    }

    #[test]
    fn sneaking_is_slower_than_walking() {
        let world = flat_world();
        let mut player = standing_player();
        let start = player.position;
        simulate(
            &mut player,
            &world,
            &[NetworkAction::MoveForward, NetworkAction::SneakOrFlyDown],
            50,
        );
        assert!(player.is_sneaking);
        assert!((walked(&player, start) - 4.0 * SNEAK_SPEED_MULTIPLIER).abs() < 1e-3);
        assert!((feet(&player) - FLOOR_TOP).abs() < 0.01);
    }

    /// `flat_world` with the floor cut off 2 blocks ahead of the player: from z = -2 toward
    /// -Z, there is nothing.
    fn cliff() -> ChunkMap {
        let mut world =
            ChunkMap::with_loaded_region(IVec3::new(-40, 48, -40), IVec3::new(40, 95, 40));
        world.fill_box(
            IVec3::new(-40, 60, -2),
            IVec3::new(40, 63, 40),
            BlockId::Stone,
        );
        world
    }

    #[test]
    fn a_sneaking_player_does_not_walk_off_an_edge() {
        let world = cliff();
        let mut player = standing_player();
        simulate(
            &mut player,
            &world,
            &[NetworkAction::MoveForward, NetworkAction::SneakOrFlyDown],
            500,
        );
        assert!(player.on_ground);
        assert!((feet(&player) - FLOOR_TOP).abs() < 0.01);
        // The box still overhangs the edge, by up to its half width
        assert!(player.position.z < -2.0 && player.position.z > -2.4 - 0.1);

        // Along the edge, it still walks
        let start = player.position;
        let along = [NetworkAction::MoveLeft, NetworkAction::SneakOrFlyDown];
        simulate(&mut player, &world, &along, 50);
        assert!((walked(&player, start) - 4.0 * SNEAK_SPEED_MULTIPLIER).abs() < 1e-3);

        // Walking, it falls, until the chunks loaded end
        simulate(&mut player, &world, &[NetworkAction::MoveForward], 50);
        assert!(!player.on_ground && feet(&player) < FLOOR_TOP - 0.5);
    }

    #[test]
    fn a_swimmer_dives_and_rises_where_it_looks() {
        let world = pool();
        let swim = [NetworkAction::MoveForward, NetworkAction::Sprint];
        for (pitch, sign) in [(-0.6_f32, -1.0), (0.6, 1.0)] {
            let mut player = diver();
            player.position.y = 66.0;
            let start = player.position;
            let mut input = frame(&swim);
            input.camera = Transform::from_rotation(Quat::from_rotation_x(pitch));
            for _ in 0..20 {
                simulate_player_movement(&mut player, &world, &input);
            }
            assert!(player.is_swimming);
            let moved = player.position - start;
            // 20 frames at the swimming speed
            let expected = SWIM_SPEED * 0.016 * 20.0;
            assert!((moved.length() - expected).abs() < 0.05, "moved {moved}");
            assert!(
                (moved.y - sign * expected * pitch.abs().sin()).abs() < 0.05,
                "pitch {pitch}: moved {moved}"
            );
        }
    }

    #[test]
    fn a_swimmer_looking_ahead_neither_sinks_nor_floats() {
        let world = pool();
        let mut player = diver();
        let start = player.position;
        simulate(
            &mut player,
            &world,
            &[NetworkAction::MoveForward, NetworkAction::Sprint],
            100,
        );
        assert!(player.is_swimming);
        assert!((player.position.y - start.y).abs() < 1e-3);
        assert!((walked(&player, start) - SWIM_SPEED * 1.6).abs() < 1e-3);
    }

    #[test]
    fn holding_sneak_in_water_sinks_faster() {
        let world = pool();
        let mut sinker = diver();
        let mut diver_ = diver();
        simulate(&mut sinker, &world, &[NetworkAction::SneakOrFlyDown], 20);
        simulate(&mut diver_, &world, &[], 20);
        assert!(sinker.position.y < diver_.position.y - 0.5);
        assert!(!sinker.is_sneaking, "sneak is the swim down key in water");
    }

    #[test]
    fn a_survival_player_cannot_fly() {
        let world = flat_world();
        let mut player = standing_player();
        simulate(&mut player, &world, &[NetworkAction::ToggleFlyMode], 1);
        assert!(!player.is_flying);

        // One flying when it is switched to survival falls
        let mut player = flyer();
        player.position.y = 80.0;
        player.game_mode = GameMode::Survival;
        simulate(&mut player, &world, &[], 1);
        assert!(!player.is_flying);
        simulate(&mut player, &world, &[], 200);
        assert!(player.on_ground);
    }

    #[test]
    fn landing_tells_how_far_the_player_fell() {
        let world = flat_world();
        let mut player = standing_player();
        player.position.y += 10.0;
        player.on_ground = false;
        let mut landed = None;
        for _ in 0..200 {
            if let Some(fallen) = simulate_player_movement(&mut player, &world, &frame(&[])) {
                assert_eq!(landed, None, "landed twice");
                landed = Some(fallen);
            }
        }
        let fallen = landed.expect("never landed");
        assert!((fallen - 10.0).abs() < 0.01, "fell {fallen}");
        assert_eq!(player.fall_distance, 0.0);

        // A jump falls back from its height only
        simulate(&mut player, &world, &[NetworkAction::JumpOrFlyUp], 1);
        let mut landed = None;
        for _ in 0..100 {
            landed = landed.or(simulate_player_movement(&mut player, &world, &frame(&[])));
        }
        let fallen = landed.expect("never landed");
        assert!((fallen - 1.25).abs() < 0.05, "fell {fallen}");
    }

    #[test]
    fn a_fall_into_water_ends_in_the_water() {
        let world = pool();
        let mut player = diver();
        player.position.y = WATER_TOP + 20.0;
        let mut landed = None;
        for _ in 0..400 {
            landed = landed.or(simulate_player_movement(&mut player, &world, &frame(&[])));
        }
        // It reaches the bottom of the pool, slowed down by the water
        assert!(player.on_ground);
        assert!(landed.unwrap_or(0.0) < 1.0, "fell {landed:?}");
    }

    #[test]
    fn a_dead_player_does_not_move() {
        let world = flat_world();
        let mut player = standing_player();
        player.position.y += 5.0;
        player.health = 0;
        let start = player.position;
        simulate(&mut player, &world, &[NetworkAction::MoveForward], 50);
        assert_eq!(player.position, start);
    }
}
