use crate::{
    messages::{NetworkAction, PlayerFrameInput},
    players::{
        collision::{check_player_collision, is_player_in_liquid},
        constants::{
            FLY_SPEED_MULTIPLIER, FLY_SPRINT_SPEED_MULTIPLIER, GRAVITY, JUMP_VELOCITY,
            MAX_PITCH_DEGREES, MAX_UPWARD_VELOCITY, SNEAK_GROUND_PROBE, SNEAK_SPEED_MULTIPLIER,
            SPEED, SPRINT_SPEED_MULTIPLIER, SWIM_DOWN_VELOCITY, SWIM_SPEED, SWIM_UP_VELOCITY,
            WATER_DRAG, WATER_EXIT_VELOCITY, WATER_GRAVITY, WATER_SPEED_MULTIPLIER,
        },
    },
    world::{world_position_to_chunk_position, BlockId, ChunkMap, PORTAL_HEIGHT, PORTAL_WIDTH},
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

/// Speed of a player up or down a ladder, in blocks/s.
const LADDER_SPEED: f32 = 3.0;

/// How high a player on the ground steps up without jumping, onto a slab or a stair.
const STEP_HEIGHT: f32 = 0.5 + 2.0 * CONTACT_GAP;

/// Longest vertical move tested at once, in blocks, so that a fast fall cannot cross a block.
const MAX_VERTICAL_STEP: f32 = 0.5;

/// What one input did to the player, beyond its state.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct MovementOutcome {
    /// How many blocks the player fell, when it landed on the ground in this input: for the
    /// server to hurt it.
    pub fallen: Option<f32>,
    /// Where the camera looks after going through a portal in this input, set upright: for the
    /// client to turn its camera to, which the next inputs bring.
    pub turned: Option<Look>,
}

/// Where a camera looks, as the client's camera controller keeps it: the yaw, in radians about
/// the vertical (0 looking north, -Z, growing to the left), and the pitch (positive looking
/// down), within `MAX_PITCH_DEGREES`. An upright camera, without roll.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct Look {
    pub yaw: f32,
    pub pitch: f32,
}

impl Look {
    /// The upright look closest to the camera rotated by `rotation`: the yaw of where it looks,
    /// or of its up when it looks straight up or down, and its pitch, clamped.
    pub fn upright(rotation: Quat) -> Self {
        let (forward, up) = (rotation * Vec3::NEG_Z, rotation * Vec3::Y);
        // Looking straight down, the top of the view is ahead; straight up, it is behind
        let heading = if forward.xz().length() > 1e-4 {
            forward.xz()
        } else {
            up.xz() * -forward.y.signum()
        };
        let max_pitch = MAX_PITCH_DEGREES.to_radians();
        Self {
            yaw: (-heading.x).atan2(-heading.y),
            pitch: (-forward.y)
                .clamp(-1.0, 1.0)
                .asin()
                .clamp(-max_pitch, max_pitch),
        }
    }

    /// The rotation of a camera looking so, as the camera controller turns it.
    pub fn rotation(&self) -> Quat {
        Quat::from_rotation_y(self.yaw) * Quat::from_rotation_x(-self.pitch)
    }
}

/// Advances `player` by one input. The client (prediction) and the server (authority) both
/// run it, and they must agree on its result: keep it free of randomness and of wall-clock time.
pub fn simulate_player_movement(
    player: &mut Player,
    world_map: &ChunkMap,
    action: &PlayerFrameInput,
) -> MovementOutcome {
    // A dead player waits for its respawn
    if player.is_dead() {
        return MovementOutcome::default();
    }
    // Hold the player still until its terrain has arrived
    if !is_terrain_loaded(player.position, world_map) {
        log::debug!(
            "The chunk at {} or the one below is not loaded, skipping movement simulation",
            player.position
        );
        return MovementOutcome::default();
    }
    let start = player.position;

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
        climb(player, world_map, action, direction);
    }
    let landed = move_vertically(player, world_map, delta);
    if !in_liquid {
        finish_gravity(player, delta);
    }
    // Only a portal gives a horizontal velocity, which the player keeps in the air
    if landed.is_some() || in_liquid || player.is_flying {
        player.velocity.x = 0.0;
        player.velocity.z = 0.0;
    }
    apply_wished_direction(player, world_map, direction, in_liquid, delta);
    MovementOutcome {
        fallen: landed,
        turned: pass_through_portal(player, world_map, start),
    }
}

/// Takes the player through the open portal its centre went into on its way from `from`: its
/// position, its velocity and its camera go through the portal (`Portals::through`), and come
/// out of the twin turned with it. The camera keeps where it looks, set upright, and is
/// returned.
///
/// Out of the twin, the centre is kept within its opening shrunk by the half extents of the
/// box, so that the box fits the opening: a player falling into a floor portal near one end
/// comes out of a wall portal with its feet in the opening, not in the wall under it. A box
/// that went into a portal fitted its opening, which the same shape on a wall keeps it doing.
/// The eyes keep their height above the centre.
fn pass_through_portal(player: &mut Player, world_map: &ChunkMap, from: Vec3) -> Option<Look> {
    let portals = world_map.portals();
    let portal = portals.crossed(from, player.position)?;
    let (through, twin) = (portals.through(portal)?, portals.twin(portal)?);
    let eyes = player.camera_transform.translation - player.position;

    let extents = Vec3::new(player.width, player.height, player.width) / 2.0;
    let room = |side: Vec3, length: i32| (length as f32 / 2.0 - side.abs().dot(extents)).max(0.0);
    let (across, high) = (
        room(twin.right(), PORTAL_WIDTH),
        room(twin.up(), PORTAL_HEIGHT),
    );
    let out = twin.local(through.transform_point3(player.position));
    let out = Vec3::new(
        out.x.clamp(-across, across),
        out.y.clamp(-high, high),
        out.z,
    );
    player.position = twin.frame().transform_point3(out);
    player.velocity = through.transform_vector3(player.velocity);

    let camera = &mut player.camera_transform;
    let level = through.transform_vector3(eyes.with_y(0.0)).with_y(0.0);
    camera.translation = player.position + level + Vec3::Y * eyes.y;
    let look = Look::upright(Quat::from_mat3a(&through.matrix3) * camera.rotation);
    camera.rotation = look.rotation();
    Some(look)
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

/// On a ladder, a player falls slowly, climbs by jumping or by walking against a block, and
/// holds on by sneaking.
fn climb(player: &mut Player, world_map: &ChunkMap, action: &PlayerFrameInput, direction: Vec3) {
    let feet = player.position - Vec3::Y * (player.height / 2.0 - 0.1);
    let on_ladder = [feet, player.position].into_iter().any(|point| {
        world_map
            .get_block(point.floor().as_ivec3())
            .is_some_and(|block| block.id == BlockId::Ladder)
    });
    if player.is_flying || !on_ladder {
        return;
    }
    player.fall_distance = 0.0;
    let ahead = player.position + direction.with_y(0.0) * 0.1;
    let climbs = action.is_pressed(NetworkAction::JumpOrFlyUp)
        || direction != Vec3::ZERO && check_player_collision(&ahead, player, world_map);
    if climbs {
        player.velocity.y = LADDER_SPEED;
    } else if player.is_sneaking {
        player.velocity.y = 0.0;
    } else {
        player.velocity.y = player.velocity.y.max(-LADDER_SPEED);
    }
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
        let candidate = player.position.with_y(new_y);
        let into_portal = goes_into_portal(player, world_map, candidate);
        if into_portal || !check_player_collision(&candidate, player, world_map) {
            player.position.y = new_y;
            player.fall_distance += (-step).max(0.0);
            if into_portal {
                // The rest of the move is made out of the twin, in the next input
                player.on_ground = false;
                return None;
            }
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
        } else if falling {
            // A block lower than its cell, such as a slab: its top is a whole sixteenth
            let feet = ((new_y - half_height) * 16.0).floor();
            let top = (1..=8)
                .map(|step| (feet + step as f32) / 16.0 + half_height + CONTACT_GAP)
                .take_while(|y| *y <= player.position.y)
                .find(|y| !check_player_collision(&player.position.with_y(*y), player, world_map));
            if let Some(top) = top {
                player.fall_distance += player.position.y - top;
                player.position.y = top;
            }
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

/// Whether the centre of `player` goes, moving to `candidate`, into an open portal that lets it
/// through (`Portals::letting_through`): the move is then made whatever lies behind the wall
/// cells, as a floor thicker than a block does under a floor portal, and is the crossing.
fn goes_into_portal(player: &Player, world_map: &ChunkMap, candidate: Vec3) -> bool {
    let portals = world_map.portals();
    portals
        .crossed(player.position, candidate)
        .is_some_and(|crossed| {
            portals
                .letting_through(player.position, player.width / 2.0)
                .any(|open| open == crossed)
        })
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

/// Moves the player along `direction`, and by its horizontal velocity, along x then along z.
/// On foot, an axis blocked by a block is left unchanged, and its velocity dropped, so the
/// player slides along walls, and a player sneaking on the ground does not step off it; a
/// flying player goes through blocks.
fn apply_wished_direction(
    player: &mut Player,
    world_map: &ChunkMap,
    direction: Vec3,
    in_liquid: bool,
    delta: f32,
) {
    let step = direction * (speed(player, in_liquid) * delta) + player.velocity.with_y(0.0) * delta;
    let keeps_to_the_ground = player.is_sneaking && player.on_ground;

    for axis in [Vec3::X, Vec3::Z] {
        let moved = player.position + step * axis;
        if !player.is_flying {
            if check_player_collision(&moved, player, world_map) {
                let up = moved + Vec3::Y * STEP_HEIGHT;
                let over = player.position + Vec3::Y * STEP_HEIGHT;
                if player.on_ground
                    && step * axis != Vec3::ZERO
                    && !check_player_collision(&over, player, world_map)
                    && !check_player_collision(&up, player, world_map)
                {
                    player.position = up;
                } else {
                    player.velocity *= Vec3::ONE - axis;
                }
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
            PLAYER_HEIGHT, PLAYER_WIDTH,
        },
        world::{
            world_position_to_chunk_position, BlockId, Chunk, FaceDirection, Portal, PortalColor,
        },
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
                simulate_player_movement(&mut player, &world, &frame(&[])).fallen,
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
            if let Some(fallen) = simulate_player_movement(&mut player, &world, &frame(&[])).fallen
            {
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
            landed = landed.or(simulate_player_movement(&mut player, &world, &frame(&[])).fallen);
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
            landed = landed.or(simulate_player_movement(&mut player, &world, &frame(&[])).fallen);
        }
        // It reaches the bottom of the pool, slowed down by the water
        assert!(player.on_ground);
        assert!(landed.unwrap_or(0.0) < 1.0, "fell {landed:?}");
    }

    /// The flat world with two stone walls, 5 high: one facing +Z at z = -4, x from -3 to 4,
    /// with the blue portal of player 7 at x in 0..2 (plane z = -3); one facing -X at x = 10, z
    /// from -6 to 6, with the orange portal at z in 0..2 (plane x = 10). Through the blue one,
    /// north (-Z) turns into west (-X): a quarter turn to the left.
    fn portal_world() -> ChunkMap {
        let mut world = flat_world();
        world.fill_box(
            IVec3::new(-3, 64, -4),
            IVec3::new(4, 68, -4),
            BlockId::Stone,
        );
        world.fill_box(
            IVec3::new(10, 64, -6),
            IVec3::new(10, 68, 6),
            BlockId::Stone,
        );
        let portals = world.portals_mut();
        portals.insert(Portal {
            owner: 7,
            color: PortalColor::Blue,
            anchor: IVec3::new(0, 64, -4),
            face: FaceDirection::PlusZ,
            up: FaceDirection::PlusY,
        });
        portals.insert(Portal {
            owner: 7,
            color: PortalColor::Orange,
            anchor: IVec3::new(10, 64, 0),
            face: FaceDirection::MinusX,
            up: FaceDirection::PlusY,
        });
        world
    }

    /// Simulates `frames` inputs, of the actions `actions` gives for each, the camera looking
    /// along `looking` from the eyes of the player and turned by what each input reports, as
    /// the client turns it. Returns, for each input, where the player is after it and the turn
    /// it reported.
    fn walk(
        player: &mut Player,
        world: &ChunkMap,
        looking: Vec3,
        frames: u32,
        actions: impl Fn(u32) -> Vec<NetworkAction>,
    ) -> Vec<(Vec3, Option<Look>)> {
        let mut camera = Transform::from_translation(player.position).looking_to(looking, Vec3::Y);
        (0..frames)
            .map(|index| {
                let mut input = frame(&actions(index));
                camera.translation = player.position + Vec3::Y * 0.8;
                input.camera = camera;
                let turned = simulate_player_movement(player, world, &input).turned;
                if let Some(look) = turned {
                    camera.rotation = look.rotation();
                }
                (player.position, turned)
            })
            .collect()
    }

    /// The yaws turned to while walking forward along `looking` for `frames` inputs.
    fn walk_forward(player: &mut Player, world: &ChunkMap, looking: Vec3, frames: u32) -> Vec<f32> {
        walk(player, world, looking, frames, |_| {
            vec![NetworkAction::MoveForward]
        })
        .into_iter()
        .filter_map(|(_, turned)| turned.map(|look| look.yaw))
        .collect()
    }

    /// Checks that the player walked into a wall and stopped there, its centre at `z`: within
    /// a step of it, as a move into a block is not made at all.
    #[track_caller]
    fn assert_stopped_at(player: &Player, z: f32) {
        assert!(
            (player.position.z - z).abs() < 0.1,
            "stopped at {}, not {z}",
            player.position
        );
    }

    #[test]
    fn walking_into_an_open_portal_takes_the_player_out_of_its_twin() {
        let world = portal_world();
        let mut player = standing_player();
        player.position.x = 1.0;
        let turns = walk_forward(&mut player, &world, Vec3::NEG_Z, 120);

        assert_eq!(turns.len(), 1, "{turns:?}");
        assert!(
            (turns[0] - std::f32::consts::FRAC_PI_2).abs() < 1e-5,
            "{turns:?}"
        );
        // Out of the orange portal, in front of it, walking west on the floor
        let end = player.position;
        assert!(end.x < 6.0 && (end.z - 1.0).abs() < 0.05, "{end}");
        assert!((feet(&player) - FLOOR_TOP).abs() < 0.01);
        // The camera the player came in with went through with it
        let forward = player.camera_transform.forward();
        assert!(forward.distance(Vec3::NEG_X) < 1e-4, "{forward:?}");
    }

    #[test]
    fn the_camera_goes_through_the_portal_with_the_player() {
        let world = portal_world();
        let mut player = standing_player();
        player.position = Vec3::new(1.0, FLOOR_TOP + 0.9, -2.95);
        let mut input = frame(&[NetworkAction::MoveForward]);
        // Looking down a little, to the right
        input.camera = Transform::from_translation(player.position + Vec3::Y * 0.8)
            .looking_to(Vec3::new(0.2, -0.3, -1.0), Vec3::Y);
        let through = world
            .portals()
            .through(
                world
                    .portals()
                    .iter()
                    .find(|p| p.color == PortalColor::Blue)
                    .unwrap(),
            )
            .unwrap();

        let outcome = simulate_player_movement(&mut player, &world, &input);

        assert!(outcome.turned.is_some());
        let camera = player.camera_transform;
        let expected = through.transform_point3(input.camera.translation);
        assert!(camera.translation.distance(expected) < 1e-4);
        let forward = through.transform_vector3(input.camera.forward().into());
        assert!(camera.forward().distance(forward) < 1e-4);
        // The pitch is kept
        assert!((camera.forward().y - input.camera.forward().y).abs() < 1e-5);
    }

    #[test]
    fn the_wall_beside_the_opening_still_stops_the_player() {
        let world = portal_world();
        // Left of the opening; and in it, but too near its edge for the box to fit
        for x in [-1.0, 1.7] {
            let mut player = standing_player();
            player.position.x = x;
            let turns = walk_forward(&mut player, &world, Vec3::NEG_Z, 120);
            assert!(turns.is_empty(), "{x}");
            assert_stopped_at(&player, -3.0 + PLAYER_WIDTH / 2.0);
        }
    }

    #[test]
    fn a_flyer_crossing_the_plane_outside_the_frame_stays_on_its_side() {
        let world = portal_world();
        let mut player = flyer();
        // Above the opening, through the wall: a flyer goes through blocks
        player.position = Vec3::new(1.0, 68.0, 0.5);
        let turns = walk_forward(&mut player, &world, Vec3::NEG_Z, 120);
        assert!(turns.is_empty());
        assert!(player.position.z < -6.0, "{}", player.position);
        // In the opening, it goes through the portal
        let mut player = flyer();
        player.position = Vec3::new(1.0, 65.5, 0.5);
        let turns = walk_forward(&mut player, &world, Vec3::NEG_Z, 120);
        assert_eq!(turns.len(), 1);
        assert!(
            player.position.x < 10.0 && player.position.z > -1.0,
            "{}",
            player.position
        );
    }

    #[test]
    fn a_portal_seen_from_behind_is_a_wall() {
        let world = portal_world();
        // Behind the wall, walking south into it
        let mut player = standing_player();
        player.position = Vec3::new(1.0, FLOOR_TOP + 0.9, -7.0);
        let turns = walk_forward(&mut player, &world, Vec3::Z, 120);
        assert!(turns.is_empty());
        assert_stopped_at(&player, -4.0 - PLAYER_WIDTH / 2.0);
        // A flyer goes through the wall from behind, and not through the portal
        let mut player = flyer();
        player.position = Vec3::new(1.0, 65.5, -7.0);
        let turns = walk_forward(&mut player, &world, Vec3::Z, 60);
        assert!(turns.is_empty());
        assert!(player.position.z > -3.0, "{}", player.position);
        assert!(
            (player.position.x - 1.0).abs() < 1e-4,
            "{}",
            player.position
        );
    }

    #[test]
    fn a_closed_portal_is_a_wall() {
        let mut world = portal_world();
        world.portals_mut().remove(7, PortalColor::Orange);
        let mut player = standing_player();
        player.position.x = 1.0;
        let turns = walk_forward(&mut player, &world, Vec3::NEG_Z, 120);
        assert!(turns.is_empty());
        assert_stopped_at(&player, -3.0 + PLAYER_WIDTH / 2.0);
    }

    /// Whether the box of `player` overlaps a block where it is.
    fn inside_blocks(player: &Player, world: &ChunkMap) -> bool {
        let half_size = Vec3::new(player.width, player.height, player.width) / 2.0;
        world.check_collision_box(&bevy::math::bounding::Aabb3d::new(
            player.position,
            half_size,
        ))
    }

    #[test]
    fn a_player_left_in_the_wall_of_a_portal_that_closed_or_moved_walks_back_out() {
        for change in ["closed", "moved"] {
            let mut world = portal_world();
            let portals = world.portals_mut();
            match change {
                "closed" => {
                    portals.remove(7, PortalColor::Orange);
                }
                _ => portals.insert(Portal {
                    owner: 7,
                    color: PortalColor::Blue,
                    anchor: IVec3::new(10, 64, 3),
                    face: FaceDirection::MinusX,
                    up: FaceDirection::PlusY,
                }),
            }
            // Its box 0.2 block into the wall of the blue portal
            let mut player = standing_player();
            player.position = Vec3::new(1.0, FLOOR_TOP + 0.9, -2.8);
            assert!(inside_blocks(&player, &world));

            // Not deeper, nor down
            walk_forward(&mut player, &world, Vec3::NEG_Z, 30);
            assert!(
                (player.position.z - -2.8).abs() < 1e-4,
                "{change}: {}",
                player.position
            );
            assert!((feet(&player) - FLOOR_TOP).abs() < 0.01, "{change}");

            // But back out, and then the wall is a wall again
            walk(&mut player, &world, Vec3::NEG_Z, 30, |_| {
                vec![NetworkAction::MoveBackward]
            });
            assert!(player.position.z > -2.0, "{change}: {}", player.position);
            assert!(!inside_blocks(&player, &world));
            walk_forward(&mut player, &world, Vec3::NEG_Z, 60);
            assert_stopped_at(&player, -3.0 + PLAYER_WIDTH / 2.0);
        }
    }

    #[test]
    fn a_player_inside_blocks_reaches_no_new_one() {
        // Stuck in a pillar 1 block wide, by the side of a wall
        let mut world = flat_world();
        world.fill_box(IVec3::new(0, 64, 0), IVec3::new(0, 65, 0), BlockId::Stone);
        world.fill_box(
            IVec3::new(-3, 64, -2),
            IVec3::new(3, 66, -2),
            BlockId::Stone,
        );
        let mut player = standing_player();
        player.position = Vec3::new(0.5, FLOOR_TOP + 0.9, 0.5);
        // North, toward the wall: out of the pillar, but not into the wall
        walk_forward(&mut player, &world, Vec3::NEG_Z, 60);
        assert!(!inside_blocks(&player, &world));
        assert_stopped_at(&player, -1.0 + PLAYER_WIDTH / 2.0);
    }

    #[test]
    fn the_speed_goes_through_the_portal_turned_with_it() {
        let world = portal_world();
        let mut player = standing_player();
        player.position = Vec3::new(1.0, FLOOR_TOP + 0.9, -2.95);
        // Whatever moves the player horizontally by its velocity, north here, goes west after
        player.velocity = Vec3::new(0.0, 0.0, -3.0);
        let turns = walk_forward(&mut player, &world, Vec3::NEG_Z, 1);
        assert_eq!(turns.len(), 1);
        assert!(
            player.velocity.xz().distance(Vec2::new(-3.0, 0.0)) < 1e-4,
            "{}",
            player.velocity
        );

        // Walking speed: the steps after the crossing are as long as before, westward
        let mut player = standing_player();
        player.position.x = 1.0;
        let start = player.position;
        let trace = walk(&mut player, &world, Vec3::NEG_Z, 120, |_| {
            vec![NetworkAction::MoveForward]
        });
        let crossing = trace
            .iter()
            .position(|(_, turned)| turned.is_some())
            .unwrap();
        let before = trace[1].0 - trace[0].0;
        let after = trace[crossing + 2].0 - trace[crossing + 1].0;
        assert!(trace[0].0 != start);
        assert!(
            (before.length() - after.length()).abs() < 1e-4,
            "{before} {after}"
        );
        assert!(after.normalize().distance(Vec3::NEG_X) < 1e-4, "{after}");
    }

    #[test]
    fn a_jump_through_a_portal_keeps_its_height() {
        // The same jump, half a block before the blue portal, and in the open
        let jump = |world: &ChunkMap| {
            let mut player = standing_player();
            player.position = Vec3::new(1.0, FLOOR_TOP + 0.9, -2.5);
            walk(&mut player, world, Vec3::NEG_Z, 40, |index| {
                let mut actions = vec![NetworkAction::MoveForward];
                if index == 0 {
                    actions.push(NetworkAction::JumpOrFlyUp);
                }
                actions
            })
        };
        let through = jump(&portal_world());
        let open = jump(&flat_world());
        let crossing = through
            .iter()
            .position(|(_, turned)| turned.is_some())
            .unwrap();
        // In the air when it crosses
        assert!(
            through[crossing].0.y > FLOOR_TOP + 1.2,
            "{:?}",
            through[crossing]
        );
        let heights =
            |trace: &[(Vec3, Option<Look>)]| trace.iter().map(|(p, _)| p.y).collect::<Vec<_>>();
        for (through, open) in heights(&through).into_iter().zip(heights(&open)) {
            assert!((through - open).abs() < 1e-4, "{through} {open}");
        }
    }

    /// The flat world with the blue portal of player 7 on the floor, its opening x in 0..2 and
    /// z in -3..0, its up north, and `twin` as the orange one, if any, with a stone wall facing
    /// +Z at z = -20 (x 8..=15, y 64..=69) and a stone ceiling at y = 80 (x 16..=26, z -6..=6)
    /// for it to stand on.
    fn floor_portal_world(twin: Option<Portal>) -> ChunkMap {
        let mut world = flat_world();
        world.fill_box(
            IVec3::new(8, 64, -20),
            IVec3::new(15, 69, -20),
            BlockId::Stone,
        );
        world.fill_box(
            IVec3::new(16, 80, -6),
            IVec3::new(26, 80, 6),
            BlockId::Stone,
        );
        let portals = world.portals_mut();
        portals.insert(Portal {
            owner: 7,
            color: PortalColor::Blue,
            anchor: IVec3::new(0, 63, -1),
            face: FaceDirection::PlusY,
            up: FaceDirection::MinusZ,
        });
        if let Some(twin) = twin {
            portals.insert(twin);
        }
        world
    }

    /// The orange portal on the wall of `floor_portal_world`, its opening x in 11..13, y in
    /// 64..67, facing south.
    fn wall_twin() -> Portal {
        Portal {
            owner: 7,
            color: PortalColor::Orange,
            anchor: IVec3::new(11, 64, -20),
            face: FaceDirection::PlusZ,
            up: FaceDirection::PlusY,
        }
    }

    /// The orange portal on the ceiling of `floor_portal_world`, its up east, facing down.
    fn ceiling_twin() -> Portal {
        Portal {
            owner: 7,
            color: PortalColor::Orange,
            anchor: IVec3::new(20, 80, 0),
            face: FaceDirection::MinusY,
            up: FaceDirection::PlusX,
        }
    }

    /// A player falling from `position`, looking along `looking`.
    fn faller(position: Vec3, looking: Vec3) -> Player {
        let mut player = standing_player();
        player.position = position;
        player.on_ground = false;
        player.camera_transform =
            Transform::from_translation(position + Vec3::Y * 0.8).looking_to(looking, Vec3::Y);
        player
    }

    /// Simulates inputs without a key, the camera turned as the simulation leaves it, at the
    /// eyes of the player, until the player goes through a portal. Returns its velocity before that input, and what that
    /// input reported. No landing is reported on the way.
    fn fall_through(player: &mut Player, world: &ChunkMap) -> (Vec3, MovementOutcome) {
        for _ in 0..200 {
            let before = player.velocity;
            let mut input = frame(&[]);
            input.camera = player.camera_transform;
            input.camera.translation = player.position + Vec3::Y * 0.8;
            let outcome = simulate_player_movement(player, world, &input);
            assert_eq!(outcome.fallen, None, "landed at {}", player.position);
            if outcome.turned.is_some() {
                return (before, outcome);
            }
        }
        panic!("never went through, at {}", player.position);
    }

    /// The vertical velocity a faller at `before` has after an input of 16 ms.
    fn after_a_frame(before: Vec3) -> f32 {
        before.y + GRAVITY * 0.016
    }

    #[test]
    fn falling_into_a_floor_portal_comes_out_of_a_wall_at_the_same_speed() {
        let world = floor_portal_world(Some(wall_twin()));
        let mut player = faller(Vec3::new(1.0, 70.0, -1.5), Vec3::NEG_Z);
        let (before, _) = fall_through(&mut player, &world);

        // Out of the wall, southward, as fast as it fell; the floor is four blocks thick, and
        // the fall went on through all of them
        let speed = -after_a_frame(before);
        assert!(speed > 10.0, "{speed}");
        let velocity = player.velocity;
        assert!(
            velocity.distance(Vec3::new(0.0, 0.0, speed)) < 1e-3,
            "{velocity}, falling at {speed}"
        );
        let out = player.position;
        assert!(out.z > -19.0 && (out.x - 12.0).abs() < 1e-3, "{out}");
        // And it flies on at that speed, falling again
        simulate(&mut player, &world, &[], 1);
        let step = player.position - out;
        assert!((step.z - speed * 0.016).abs() < 1e-3, "{step}");
        assert!(step.y < 0.0, "{step}");
        // Until it lands, where it stops
        simulate(&mut player, &world, &[], 120);
        assert!(player.on_ground);
        assert!((feet(&player) - FLOOR_TOP).abs() < 0.01);
        assert_eq!(player.velocity, Vec3::ZERO);
        let landed = player.position;
        simulate(&mut player, &world, &[], 10);
        assert_eq!(player.position, landed);
    }

    #[test]
    fn a_player_falling_near_the_end_of_a_floor_portal_fits_the_wall_portal_it_comes_out_of() {
        let world = floor_portal_world(Some(wall_twin()));
        // 1.05 blocks south of the centre, along the floor portal's up: 1.05 below the centre of
        // the wall portal, its box would reach below the opening
        let mut player = faller(Vec3::new(1.0, 70.0, -0.45), Vec3::NEG_Z);
        fall_through(&mut player, &world);
        let out = player.position;
        assert!(out.y - PLAYER_HEIGHT / 2.0 >= 64.0 - 1e-3, "{out}");
        assert!(out.y + PLAYER_HEIGHT / 2.0 <= 67.0 + 1e-3, "{out}");
    }

    #[test]
    fn falling_into_a_floor_portal_comes_out_of_a_ceiling_still_falling() {
        let world = floor_portal_world(Some(ceiling_twin()));
        let mut player = faller(Vec3::new(1.0, 70.0, -1.5), Vec3::NEG_Z);
        let (before, _) = fall_through(&mut player, &world);
        let velocity = player.velocity;
        assert!(
            velocity.distance(Vec3::new(0.0, after_a_frame(before), 0.0)) < 1e-3,
            "{velocity}"
        );
        assert!(player.position.y < 80.0, "{}", player.position);
        let ceiling = world.portals().get(7, PortalColor::Orange).unwrap();
        assert!(ceiling.signed_distance(player.position) > 0.0);
    }

    #[test]
    fn a_player_standing_on_an_open_floor_portal_falls_into_it() {
        let world = floor_portal_world(Some(wall_twin()));
        let mut player = standing_player();
        player.position = Vec3::new(1.0, FLOOR_TOP + 0.9, -1.5);
        let (_, outcome) = fall_through(&mut player, &world);
        assert!(outcome.turned.is_some());
        // Its feet over the edge of the opening, its centre beyond the opening shrunk by half
        // its width: it stands
        for x in [-0.1, 1.7] {
            let mut player = standing_player();
            player.position = Vec3::new(x, FLOOR_TOP + 0.9, -1.5);
            simulate(&mut player, &world, &[], 60);
            assert!((feet(&player) - FLOOR_TOP).abs() < 0.01, "{x}");
        }
    }

    #[test]
    fn a_player_left_in_a_floor_portal_that_closed_jumps_back_out() {
        let world = floor_portal_world(None);
        let mut player = standing_player();
        // Its box 0.6 block into the floor of the blue portal
        player.position = Vec3::new(1.0, FLOOR_TOP + 0.3, -1.5);
        assert!(inside_blocks(&player, &world));
        simulate(&mut player, &world, &[], 30);
        assert!(
            (player.position.y - (FLOOR_TOP + 0.3)).abs() < 1e-4,
            "sank to {}",
            player.position
        );
        simulate(&mut player, &world, &[NetworkAction::JumpOrFlyUp], 1);
        simulate(&mut player, &world, &[], 60);
        assert!(!inside_blocks(&player, &world));
        assert!(
            (feet(&player) - FLOOR_TOP).abs() < 0.01,
            "{}",
            player.position
        );
    }

    #[test]
    fn the_camera_comes_out_upright_looking_where_it_looked_through_the_portal() {
        let world = floor_portal_world(Some(wall_twin()));
        // Looking north and 80 degrees down, into the floor portal
        let down = 80f32.to_radians();
        let looking = Vec3::new(0.0, -down.sin(), -down.cos());
        let mut player = faller(Vec3::new(1.0, 70.0, -1.5), looking);
        let (_, outcome) = fall_through(&mut player, &world);

        // Out of the wall: south, 10 degrees up
        let look = outcome.turned.unwrap();
        assert!(
            (look.pitch - -(10f32.to_radians())).abs() < 1e-4,
            "{look:?}"
        );
        assert!(look.yaw.cos() < -0.9999, "{look:?}");
        let camera = player.camera_transform;
        assert!(camera.rotation.angle_between(look.rotation()) < 1e-4);
        assert!(camera.right().y.abs() < 1e-5, "{:?}", camera.right());
        // The eyes above the centre, where the input's camera had them before its fall
        let eyes = camera.translation - player.position;
        assert!(eyes.xz().length() < 1e-4, "{eyes}");
        let fall = player.velocity.length() * 0.016;
        assert!(eyes.y > 0.8 && eyes.y < 0.8 + fall + 1e-3, "{eyes}");
    }

    #[test]
    fn an_upright_look_keeps_the_heading_of_a_rolled_camera_or_of_one_looking_straight_down() {
        let west = std::f32::consts::FRAC_PI_2;
        // Looking west, rolled a quarter turn: its up to the north
        let rolled = Transform::default().looking_to(Vec3::NEG_X, Vec3::NEG_Z);
        let look = Look::upright(rolled.rotation);
        assert!(
            (look.yaw - west).abs() < 1e-5 && look.pitch.abs() < 1e-5,
            "{look:?}"
        );
        // Looking a little down to the west, rolled likewise
        let down = Vec3::new(-1.0, -0.2, 0.0);
        let rolled = Transform::default().looking_to(down, Vec3::NEG_Z);
        let look = Look::upright(rolled.rotation);
        assert!((look.yaw - west).abs() < 1e-5, "{look:?}");
        assert!(
            (look.pitch - down.normalize().y.abs().asin()).abs() < 1e-5,
            "{look:?}"
        );
        // Straight down, the top of the view to the west: the heading is the top's
        let looking_down = Transform::default().looking_to(Vec3::NEG_Y, Vec3::NEG_X);
        let look = Look::upright(looking_down.rotation);
        assert!((look.yaw - west).abs() < 1e-5, "{look:?}");
        assert!(
            (look.pitch - MAX_PITCH_DEGREES.to_radians()).abs() < 1e-5,
            "{look:?}"
        );
    }

    #[test]
    fn walking_into_a_wall_portal_comes_up_out_of_a_floor_looking_up_as_far_as_allowed() {
        let world = floor_portal_world(Some(wall_twin()));
        let mut player = standing_player();
        player.position = Vec3::new(12.0, FLOOR_TOP + 0.9, -17.0);
        let trace = walk(&mut player, &world, Vec3::NEG_Z, 120, |_| {
            vec![NetworkAction::MoveForward]
        });
        let (out, look) = trace
            .iter()
            .find_map(|(position, look)| look.map(|look| (*position, look)))
            .unwrap();
        // Up out of the floor portal, looking straight up but for the clamp, the top of the
        // view to the north, as the wall's up goes through to the floor's
        let blue = world.portals().get(7, PortalColor::Blue).unwrap();
        assert!(
            blue.signed_distance(out) > 0.0 && blue.local(out).x.abs() < 1.0,
            "{out}"
        );
        let max = MAX_PITCH_DEGREES.to_radians();
        assert!((look.pitch + max).abs() < 1e-5, "{look:?}");
        assert!((look.rotation() * Vec3::Y).z < -0.99, "{look:?}");
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

    #[test]
    fn a_player_steps_up_stairs_and_is_stopped_by_a_fence() {
        use crate::world::{BlockData, BlockDirection};
        for (id, height) in [(BlockId::WoodenStairs, 1.0), (BlockId::WoodenSlab, 0.5)] {
            let mut world = flat_world();
            let mut player = standing_player();
            let front = player.position.floor().as_ivec3().with_y(FLOOR_TOP as i32);
            // The back of the stair, its high side, is away from the player, who walks to -Z
            for dz in 1..4 {
                let pos = front - IVec3::Z * dz;
                world.set_block(pos, BlockData::new(id, BlockDirection::Back));
            }
            simulate(&mut player, &world, &[NetworkAction::MoveForward], 20);
            let rise = player.position.y - player.height / 2.0 - FLOOR_TOP;
            assert!((rise - height).abs() < 0.05, "{id:?}: {rise}");
        }
        let mut world = flat_world();
        let mut player = standing_player();
        let front = player.position.floor().as_ivec3().with_y(FLOOR_TOP as i32);
        for dx in -15..15 {
            let pos = front + IVec3::new(dx, 0, -2);
            world.set_block(pos, BlockData::new(BlockId::Fence, BlockDirection::Front));
        }
        let keys = [NetworkAction::MoveForward, NetworkAction::JumpOrFlyUp];
        simulate(&mut player, &world, &keys, 200);
        assert!(
            player.position.z > (front.z - 1) as f32,
            "{}",
            player.position
        );
    }
}
