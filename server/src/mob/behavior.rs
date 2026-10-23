//! How mobs live, once per fixed step: gravity, then what their action asks. An animal rests
//! (some graze while they do), walks to a spot nearby, and rests again; the shy ones run away
//! from a player who comes too close. Then the animals are pushed out of the players and of one
//! another.

use std::collections::{HashMap, HashSet};
use std::f32::consts::PI;
use std::ops::Range;

use bevy::{
    math::{bounding::Aabb3d, ops::atan2, IVec2, IVec3, Quat, Vec3},
    time::{Fixed, Time},
};
use bevy_ecs::system::{Res, ResMut};
use rand::Rng;
use shared::{
    messages::PlayerId,
    players::{
        constants::{GRAVITY, JUMP_VELOCITY, MAX_UPWARD_VELOCITY, TERMINAL_VELOCITY},
        movement::{is_terrain_loaded, Look},
        Player,
    },
    world::{
        BlockHitbox, BlockId, ChunkMap, MobAction, MobId, MobKind, MobTarget, Passage, Passer,
        ServerMob,
    },
    TICKS_PER_SECOND,
};

use crate::world::data::ServerWorldMap;

/// A walking mob this close to its target, on the horizontal plane, has arrived.
const ARRIVAL_DISTANCE: f32 = 0.5;
/// A fleeing mob stops once it is this far from its target.
const SAFE_DISTANCE: f32 = 15.0;
/// A shy animal runs away from a player who comes this close.
const FEAR_DISTANCE: f32 = 6.0;
/// How long an animal rests before it walks somewhere else, in ticks: 3 to 10 seconds.
const REST_TICKS: Range<u32> = 3 * TICKS_PER_SECOND as u32..10 * TICKS_PER_SECOND as u32;
/// How often an animal that grazes does so while it rests.
const GRAZE_CHANCE: f64 = 0.35;
/// How far from where it stands an animal walks to, in blocks.
const WANDER_DISTANCE: Range<f32> = 2.0..7.0;
/// Spots an animal tries before it rests again, when none is somewhere it can stand.
const WANDER_TRIES: usize = 4;
/// A walk that has not arrived after this many ticks (10 seconds) is given up.
const WALK_TICKS: u32 = 10 * TICKS_PER_SECOND as u32;
/// How far below its feet an animal looks for the ground of a spot, in blocks: it does not
/// walk off a higher drop.
const WANDER_DROP: i32 = 3;

/// How fast a mob is pushed at most, in blocks per second: faster than a sprint, so that a
/// player does not run through it.
const MAX_PUSH_SPEED: f32 = 8.0;
/// An overlap this small is left, so that mobs side by side are not moved for ever.
const PUSH_SLACK: f32 = 0.01;
/// The side of the cells the mobs are sorted into to find those that touch, in blocks: more than
/// the width of any of them.
const PUSH_CELL: f32 = 2.0;

/// Speed of a walking mob, in blocks per second.
fn walk_speed(kind: MobKind) -> f32 {
    match kind {
        MobKind::Fox | MobKind::Wolf | MobKind::Cat | MobKind::Rabbit => 1.6,
        MobKind::Horse => 1.5,
        MobKind::Goat => 1.2,
        MobKind::Sheep | MobKind::Cow | MobKind::Pig | MobKind::Chicken => 1.0,
    }
}

/// Speed of a fleeing mob, in blocks per second.
fn flee_speed(kind: MobKind) -> f32 {
    match kind {
        MobKind::Horse => 6.0,
        MobKind::Fox | MobKind::Wolf | MobKind::Cat | MobKind::Rabbit => 4.0,
        MobKind::Goat => 3.0,
        MobKind::Sheep | MobKind::Cow | MobKind::Pig | MobKind::Chicken => 2.0,
    }
}

/// Whether a mob runs away from the players.
fn is_shy(kind: MobKind) -> bool {
    matches!(kind, MobKind::Fox | MobKind::Rabbit)
}

/// Whether a mob eats the grass.
fn grazes(kind: MobKind) -> bool {
    !matches!(kind, MobKind::Fox | MobKind::Wolf | MobKind::Cat)
}

pub fn mob_behavior_system(mut world_map: ResMut<ServerWorldMap>, time: Res<Time<Fixed>>) {
    let delta = time.delta_secs();
    let ServerWorldMap {
        mobs,
        chunks,
        players,
        changed_mobs,
        ..
    } = world_map.as_mut();
    step_mobs(
        mobs,
        chunks,
        players,
        changed_mobs,
        delta,
        &mut rand::thread_rng(),
    );
    push_mobs(mobs, chunks, players, changed_mobs, delta);
}

/// One step of every mob. Those that moved or changed what they do are added to `changed`.
fn step_mobs(
    mobs: &mut HashMap<MobId, ServerMob>,
    chunks: &ChunkMap,
    players: &HashMap<PlayerId, Player>,
    changed: &mut HashSet<MobId>,
    delta: f32,
    rng: &mut impl Rng,
) {
    for (id, mob) in mobs.iter_mut() {
        // A mob whose position is not a number cannot be simulated: it stays as it is
        if !mob.position.is_finite() || !mob.velocity.is_finite() {
            continue;
        }
        // A mob far from every player stays where it is: the chunks around it may be unloaded,
        // and it would fall through them
        if !is_terrain_loaded(mob.position, chunks) {
            continue;
        }
        let before = (mob.position, mob.rotation, mob.action);
        step_mob(mob, chunks, players, delta, rng);
        if before != (mob.position, mob.rotation, mob.action) {
            changed.insert(*id);
        }
    }
}

/// Gravity, then what the mob does. A mob whose centre went into an open portal comes out of
/// its twin (`pass_through_portal`): after its fall, which then ends its step, and after what
/// it does. A mob flung (`is_flung`) flies instead of doing anything, until it lands, which
/// stops it.
fn step_mob(
    mob: &mut ServerMob,
    chunks: &ChunkMap,
    players: &HashMap<PlayerId, Player>,
    delta: f32,
    rng: &mut impl Rng,
) {
    let start = mob.position;
    mob.portal_rest = chunks.portals().still_resting(mob.position, passer(mob));
    apply_gravity(mob, chunks, delta);
    if pass_through_portal(mob, chunks, start) {
        return;
    }
    let start = mob.position;
    if is_flung(mob) {
        if mob.on_ground {
            mob.velocity.x = 0.0;
            mob.velocity.z = 0.0;
        } else {
            fly(mob, chunks, delta);
            pass_through_portal(mob, chunks, start);
            return;
        }
    }
    act(mob, chunks, players, delta, rng);
    pass_through_portal(mob, chunks, start);
}

/// Whether `mob` goes faster than it ever goes by itself (`flee_speed`): only a portal flings
/// it so.
fn is_flung(mob: &ServerMob) -> bool {
    mob.velocity.with_y(0.0).length() > flee_speed(mob.kind)
}

/// Longest move of a mob tested at once, in blocks, as for a player: of its fall, and of its
/// flight when flung.
const MAX_FLIGHT_STEP: f32 = 0.5;

/// Moves a flung mob by its horizontal velocity, along x then z, by steps of `MAX_FLIGHT_STEP`
/// at most: an axis a block stops loses its velocity, and a step into a portal that lets the
/// mob through is made, and ends the move (`may_move_to`).
fn fly(mob: &mut ServerMob, chunks: &ChunkMap, delta: f32) {
    for axis in [Vec3::X, Vec3::Z] {
        let along = mob.velocity.dot(axis) * delta;
        if along == 0.0 {
            continue;
        }
        let steps = (along.abs() / MAX_FLIGHT_STEP).ceil();
        for _ in 0..steps as u32 {
            let moved = mob.position + axis * (along / steps);
            if chunks.portals().goes_into(mob.position, moved, passer(mob)) {
                mob.position = moved;
                return;
            }
            if collides_at(mob, chunks, moved) {
                mob.velocity *= Vec3::ONE - axis;
                break;
            }
            mob.position = moved;
        }
    }
}

/// Takes `mob` through the open portal its centre went into on its way from `from`, if any
/// (`Portals::pass`): its position and velocity come out of the twin turned with it, its body
/// faces where it faced turned so, upright, and the spot it walks to goes through too, so that
/// it walks on rather than back; through a pair that tips the ground over, a floor and a wall,
/// it stops walking instead. Returns whether it went through.
fn pass_through_portal(mob: &mut ServerMob, chunks: &ChunkMap, from: Vec3) -> bool {
    let Some(Passage {
        through,
        position,
        velocity,
        resting,
    }) = chunks
        .portals()
        .pass(from, mob.position, passer(mob), mob.velocity)
    else {
        return false;
    };
    mob.position = position;
    mob.velocity = velocity;
    mob.portal_rest = resting;
    mob.on_ground = false;
    // A mob faces +Z turned by its rotation, a camera -Z: the body's yaw is that of a camera
    // turned half round
    let half_turn = Quat::from_rotation_y(PI);
    let look = Look::through(&through, mob.rotation * half_turn);
    mob.rotation = Quat::from_rotation_y(look.yaw) * half_turn;
    // The spot it walked to goes through too, if the ground is still down beyond the pair
    if let MobTarget::Position(target) = &mut mob.target {
        if through.transform_vector3(Vec3::Y).y > 1.0 - 1e-3 {
            *target = through.transform_point3(*target);
        } else {
            (mob.action, mob.target, mob.action_ticks) = (MobAction::Idle, MobTarget::None, 1);
        }
    }
    mob.passes = mob.passes.wrapping_add(1);
    true
}

/// What the mob's action asks of it this step.
fn act(
    mob: &mut ServerMob,
    chunks: &ChunkMap,
    players: &HashMap<PlayerId, Player>,
    delta: f32,
    rng: &mut impl Rng,
) {
    if is_shy(mob.kind) && mob.action != MobAction::Flee {
        if let Some(id) = nearest_player(players, mob.position, FEAR_DISTANCE) {
            mob.action = MobAction::Flee;
            mob.target = MobTarget::Player(id);
        }
    }
    match mob.action {
        MobAction::Idle | MobAction::Graze => {
            mob.action_ticks = mob.action_ticks.saturating_sub(1);
            if mob.action_ticks == 0 {
                // What it grazed grows its wool back
                if mob.action == MobAction::Graze {
                    mob.sheared = false;
                }
                start_walk(mob, chunks, rng);
            }
        }
        MobAction::Walk => {
            mob.action_ticks = mob.action_ticks.saturating_sub(1);
            let Some(target) = target_position(mob.target, players) else {
                return rest(mob, rng);
            };
            let towards = (target - mob.position).with_y(0.0);
            if towards.length() < ARRIVAL_DISTANCE || mob.action_ticks == 0 {
                return rest(mob, rng);
            }
            walk(
                mob,
                chunks,
                towards.normalize(),
                walk_speed(mob.kind),
                delta,
            );
        }
        MobAction::Flee => {
            let Some(threat) = target_position(mob.target, players) else {
                return rest(mob, rng);
            };
            if mob.position.distance(threat) > SAFE_DISTANCE {
                return rest(mob, rng);
            }
            flee(mob, chunks, threat, flee_speed(mob.kind), delta);
        }
    }
}

/// Where `target` is, if it still is somewhere: a player who left is nowhere.
fn target_position(target: MobTarget, players: &HashMap<PlayerId, Player>) -> Option<Vec3> {
    match target {
        MobTarget::None => None,
        MobTarget::Position(position) => Some(position),
        MobTarget::Player(id) => players.get(&id).map(|player| player.position),
    }
}

/// The living player nearest to `position`, if one is within `distance`.
fn nearest_player(
    players: &HashMap<PlayerId, Player>,
    position: Vec3,
    distance: f32,
) -> Option<PlayerId> {
    players
        .values()
        .filter(|player| !player.is_dead())
        .map(|player| (player.id, player.position.distance(position)))
        .filter(|&(_, d)| d < distance)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(id, _)| id)
}

/// Stops the mob for a while: some kinds graze some of the time.
fn rest(mob: &mut ServerMob, rng: &mut impl Rng) {
    mob.action = if grazes(mob.kind) && rng.gen_bool(GRAZE_CHANCE) {
        MobAction::Graze
    } else {
        MobAction::Idle
    };
    mob.target = MobTarget::None;
    mob.action_ticks = rng.gen_range(REST_TICKS);
    mob.velocity.x = 0.0;
    mob.velocity.z = 0.0;
}

/// Sends the mob to a spot nearby it can stand on, or lets it rest again when it finds none.
fn start_walk(mob: &mut ServerMob, chunks: &ChunkMap, rng: &mut impl Rng) {
    let feet = (mob.position.y - mob.height / 2.0).floor() as i32;
    for _ in 0..WANDER_TRIES {
        let offset = Vec3::new(rng.gen_range(-1.0..1.0), 0.0, rng.gen_range(-1.0..1.0))
            .normalize_or_zero()
            * rng.gen_range(WANDER_DISTANCE);
        let spot = mob.position + offset;
        if let Some(ground) = ground_under(chunks, spot.x, spot.z, feet) {
            mob.action = MobAction::Walk;
            mob.target = MobTarget::Position(spot.with_y(ground + mob.height / 2.0));
            mob.action_ticks = WALK_TICKS;
            return;
        }
    }
    rest(mob, rng);
}

/// The height of the ground an animal whose feet are in the cell at height `feet` would stand
/// on at `(x, z)`: the highest block with a hitbox from one block above its feet down to
/// `WANDER_DROP` below them, with room above it. None over water, a wall, or a deeper drop.
fn ground_under(chunks: &ChunkMap, x: f32, z: f32, feet: i32) -> Option<f32> {
    let (x, z) = (x.floor() as i32, z.floor() as i32);
    let solid = |y: i32| {
        chunks
            .get_block(IVec3::new(x, y, z))
            .is_some_and(|block| !matches!(block.id.get_hitbox(), BlockHitbox::None))
    };
    for y in (feet - WANDER_DROP..=feet + 1).rev() {
        let id = chunks.get_block(IVec3::new(x, y, z)).map(|block| block.id);
        if id == Some(BlockId::Water) {
            return None;
        }
        if solid(y) {
            return (!solid(y + 1) && !solid(y + 2)).then_some(y as f32 + 1.0);
        }
    }
    None
}

/// What the portals know of `mob` going through them: its box, and the floor portal it rests on.
fn passer(mob: &ServerMob) -> Passer {
    Passer::new(
        Vec3::new(mob.width, mob.height, mob.depth) / 2.0,
        mob.portal_rest,
    )
}

/// Whether `mob` would be inside a block at `position`. The wall cells of the open portals that
/// let it in from where it is do not count (`Portals::passable_cells`), as for a player.
fn collides_at(mob: &ServerMob, chunks: &ChunkMap, position: Vec3) -> bool {
    let passable = chunks.portals().passable_cells(mob.position, passer(mob));
    chunks.check_collision_box_except(
        &Aabb3d::new(position, Vec3::new(mob.width, mob.height, mob.depth) / 2.0),
        |cell| passable.contains(&cell),
    )
}

/// Whether `mob` may move to `position`: nothing is in the way, or the move takes its centre
/// into a portal that lets it through, whatever lies behind the wall (`Portals::goes_into`).
fn may_move_to(mob: &ServerMob, chunks: &ChunkMap, position: Vec3) -> bool {
    chunks
        .portals()
        .goes_into(mob.position, position, passer(mob))
        || !collides_at(mob, chunks, position)
}

/// Same vertical motion as a player: fall while in the air, land on the first block met. Half
/// the gravity is added before the move and half after it, so that a jump rises as high as a
/// player's at the 20 steps per second of the mobs. A mob falls no faster than a player
/// (`TERMINAL_VELOCITY`), and its move is tested by steps of `MAX_FLIGHT_STEP` at most, as a
/// player's, so that a fast fall crosses no block: it stops after the last step a block lets it
/// make, and a step into a portal that lets it through is made, and ends the move
/// (`Portals::goes_into`).
fn apply_gravity(mob: &mut ServerMob, chunks: &ChunkMap, delta: f32) {
    if !mob.on_ground {
        mob.velocity.y = (mob.velocity.y + GRAVITY * delta / 2.0).max(-TERMINAL_VELOCITY);
    }
    let distance = mob.velocity.y * delta;
    mob.velocity.y = mob.velocity.y.min(MAX_UPWARD_VELOCITY);

    let steps = (distance.abs() / MAX_FLIGHT_STEP).ceil().max(1.0);
    for _ in 0..steps as u32 {
        let moved = mob.position + Vec3::Y * (distance / steps);
        if chunks.portals().goes_into(mob.position, moved, passer(mob)) {
            mob.position = moved;
            break;
        }
        if collides_at(mob, chunks, moved) {
            // Bumping into a ceiling is not landing
            mob.on_ground = mob.velocity.y <= 0.0;
            mob.velocity.y = 0.0;
            return;
        }
        mob.position = moved;
    }
    mob.on_ground = false;
    mob.velocity.y = (mob.velocity.y + GRAVITY * delta / 2.0).max(-TERMINAL_VELOCITY);
}

fn jump(mob: &mut ServerMob) {
    mob.velocity.y += JUMP_VELOCITY;
    mob.on_ground = false;
    mob.velocity.x = 0.0;
    mob.velocity.z = 0.0;
}

/// One step along `direction` at `speed`, which becomes the mob's horizontal velocity. A mob
/// blocked while it was moving jumps; one blocked from a standstill slides along the obstacle on
/// one axis, and jumps if both are blocked.
fn walk(mob: &mut ServerMob, chunks: &ChunkMap, direction: Vec3, speed: f32, delta: f32) {
    let step = Vec3::new(direction.x, 0.0, direction.z) * (speed * delta);
    let along_x = Vec3::new(step.x, 0.0, 0.0);
    let along_z = Vec3::new(0.0, 0.0, step.z);

    if may_move_to(mob, chunks, mob.position + step) {
        mob.position += step;
        mob.velocity.x = direction.x * speed;
        mob.velocity.z = direction.z * speed;
    } else if mob.on_ground && mob.velocity.x != 0.0 && mob.velocity.z != 0.0 {
        jump(mob);
    } else if mob.on_ground {
        if may_move_to(mob, chunks, mob.position + along_x) {
            mob.position += along_x;
            mob.velocity.x = direction.x * speed;
        } else if may_move_to(mob, chunks, mob.position + along_z) {
            mob.position += along_z;
            mob.velocity.z = direction.z * speed;
        } else {
            jump(mob);
        }
    }

    mob.rotation = Quat::from_rotation_y(atan2(direction.x, direction.z));
}

/// One step away from `threat` at `speed`, on the horizontal plane only: gravity handles the
/// vertical axis. The mob stays in place when a block is in the way.
fn flee(mob: &mut ServerMob, chunks: &ChunkMap, threat: Vec3, speed: f32, delta: f32) {
    let away = (mob.position - threat).with_y(0.0).normalize_or_zero();
    let fled = mob.position + away * (speed * delta);
    if may_move_to(mob, chunks, fled) {
        mob.position = fled;
    }
    if away != Vec3::ZERO {
        mob.rotation = Quat::from_rotation_y(atan2(away.x, away.z));
    }
}

/// What pushes a mob: its position, half its width and half its height.
#[derive(Clone, Copy)]
struct Pusher {
    position: Vec3,
    half: f32,
    half_height: f32,
}

/// How far, on the horizontal plane, a mob must go to leave `pusher`: zero when they do not
/// touch. `tie` is the way it goes when they stand on the same spot.
fn push_out(mob: &Pusher, pusher: &Pusher, tie: Vec3) -> Vec3 {
    let apart = mob.position - pusher.position;
    // One standing on the other is not pushed
    if apart.y.abs() >= mob.half_height + pusher.half_height {
        return Vec3::ZERO;
    }
    let apart = apart.with_y(0.0);
    let overlap = mob.half + pusher.half - apart.length();
    if overlap <= PUSH_SLACK {
        return Vec3::ZERO;
    }
    apart.try_normalize().unwrap_or(tie) * overlap
}

/// The cell of the grid of `push_mobs` a position is in.
fn push_cell(position: Vec3) -> IVec2 {
    IVec2::new(
        (position.x / PUSH_CELL).floor() as i32,
        (position.z / PUSH_CELL).floor() as i32,
    )
}

/// Pushes the mobs out of the players, who are not slowed by them, and out of one another, each
/// of two by half of the overlap. A mob against a block slides along it.
fn push_mobs(
    mobs: &mut HashMap<MobId, ServerMob>,
    chunks: &ChunkMap,
    players: &HashMap<PlayerId, Player>,
    changed: &mut HashSet<MobId>,
    delta: f32,
) {
    let mut grid: HashMap<IVec2, Vec<(Option<MobId>, Pusher)>> = HashMap::new();
    for (&id, mob) in mobs.iter() {
        if mob.position.is_finite() && is_terrain_loaded(mob.position, chunks) {
            let pusher = mob_pusher(mob);
            let cell = grid.entry(push_cell(mob.position)).or_default();
            cell.push((Some(id), pusher));
        }
    }
    for player in players.values() {
        // A flying player goes through everything
        if player.is_dead() || player.is_flying || !player.position.is_finite() {
            continue;
        }
        let pusher = Pusher {
            position: player.position,
            half: player.width / 2.0,
            half_height: player.height / 2.0,
        };
        let cell = grid.entry(push_cell(player.position)).or_default();
        cell.push((None, pusher));
    }

    for &(id, ref me) in grid.values().flatten() {
        let Some(id) = id else { continue };
        let centre = push_cell(me.position);
        let mut push = Vec3::ZERO;
        for dx in -1..=1 {
            for dz in -1..=1 {
                for (other, pusher) in grid
                    .get(&(centre + IVec2::new(dx, dz)))
                    .into_iter()
                    .flatten()
                {
                    match other {
                        Some(other) if *other == id => {}
                        Some(other) => {
                            let tie = if id < *other { Vec3::X } else { Vec3::NEG_X };
                            push += push_out(me, pusher, tie) / 2.0;
                        }
                        None => push += push_out(me, pusher, Vec3::X),
                    }
                }
            }
        }
        let push = push.clamp_length_max(MAX_PUSH_SPEED * delta);
        let Some(mob) = mobs.get_mut(&id) else {
            continue;
        };
        for axis in [Vec3::X, Vec3::Z] {
            let step = push * axis;
            if step != Vec3::ZERO && !collides_at(mob, chunks, mob.position + step) {
                mob.position += step;
                changed.insert(id);
            }
        }
    }
}

fn mob_pusher(mob: &ServerMob) -> Pusher {
    Pusher {
        position: mob.position,
        half: mob.width / 2.0,
        half_height: mob.height / 2.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;
    use bevy::math::{IVec3, Vec2, Vec3, Vec3Swizzles};
    use bevy::prelude::{Transform, World};
    use rand::{rngs::StdRng, SeedableRng};
    use shared::world::{
        BlockId, ChunkMap, FaceDirection, MobKind, Portal, PortalColor, ServerMob,
    };
    use std::time::Duration;

    /// What `Time<Fixed>` is on the server: the duration of a tick.
    fn tick() -> Duration {
        Duration::from_secs_f64(1.0 / TICKS_PER_SECOND as f64)
    }

    fn mob_at(position: Vec3, target: MobTarget) -> ServerMob {
        ServerMob {
            target,
            action: MobAction::Walk,
            action_ticks: WALK_TICKS,
            ..ServerMob::new(MobKind::Fox, position)
        }
    }

    /// Runs a step of `mobs`, in the air of loaded chunks up to y = 63.
    fn run_behavior(mobs: Vec<ServerMob>) -> ServerWorldMap {
        let mut world = World::new();
        let mut map = ServerWorldMap {
            chunks: ChunkMap::with_loaded_region(IVec3::splat(-32), IVec3::splat(63)),
            ..Default::default()
        };
        map.mobs = mobs
            .into_iter()
            .enumerate()
            .map(|(i, m)| (i as u128, m))
            .collect();
        world.insert_resource(map);
        let mut time = Time::<Fixed>::default();
        time.advance_by(tick());
        world.insert_resource(time);
        world.run_system_once(mob_behavior_system).unwrap();
        world.remove_resource::<ServerWorldMap>().unwrap()
    }

    #[test]
    fn a_mob_with_a_nan_position_does_not_stop_the_other_mobs() {
        let broken = mob_at(Vec3::NAN, MobTarget::Position(Vec3::ZERO));
        let healthy_position = Vec3::new(0.0, 50.0, 0.0);
        let healthy = mob_at(healthy_position, MobTarget::Position(Vec3::X * 10.0));
        // The healthy mob comes second: the NaN one used to end the whole loop.
        let map = run_behavior(vec![broken, healthy]);
        assert_ne!(map.mobs[&1].position, healthy_position);
        assert!(map.mobs[&1].position.is_finite());
        assert!(!map.changed_mobs.contains(&0));
    }

    #[test]
    fn a_mob_over_chunks_that_are_not_loaded_stays_still() {
        let position = Vec3::new(0.0, 50.0, 0.0);
        let mob = mob_at(position, MobTarget::Position(Vec3::X * 10.0));
        let mut world = World::new();
        world.insert_resource(ServerWorldMap {
            mobs: [(0, mob)].into(),
            ..Default::default()
        });
        let mut time = Time::<Fixed>::default();
        time.advance_by(tick());
        world.insert_resource(time);
        world.run_system_once(mob_behavior_system).unwrap();
        let map = world.resource::<ServerWorldMap>();
        assert_eq!(map.mobs[&0].position, position);
    }

    #[test]
    fn a_mob_whose_target_left_stops_and_rests() {
        let position = Vec3::new(0.0, 50.0, 0.0);
        let map = run_behavior(vec![mob_at(position, MobTarget::Player(99))]);
        let mob = &map.mobs[&0];
        assert_eq!(mob.position.xz(), position.xz());
        assert_eq!(mob.action, MobAction::Idle);
        assert!(REST_TICKS.contains(&mob.action_ticks));
    }

    #[test]
    fn a_mob_on_its_target_keeps_a_finite_state() {
        let position = Vec3::new(0.0, 50.0, 0.0);
        let map = run_behavior(vec![mob_at(position, MobTarget::Position(position))]);
        assert!(map.mobs[&0].position.is_finite());
        assert!(map.mobs[&0].rotation.is_finite());
    }

    /// Flat stone ground (top face at y = 1) with an optional wall of stone at x = -2.
    fn ground_world(with_wall: bool) -> ServerWorldMap {
        let mut map = ServerWorldMap {
            // The chunks below the ground too: a mob is not moved without them
            chunks: ChunkMap::with_loaded_region(IVec3::new(-10, -16, -10), IVec3::new(9, 3, 9)),
            ..Default::default()
        };
        map.chunks
            .fill_box(IVec3::new(-10, 0, -10), IVec3::new(9, 0, 9), BlockId::Stone);
        if with_wall {
            map.chunks
                .fill_box(IVec3::new(-2, 1, -10), IVec3::new(-2, 3, 9), BlockId::Stone);
        }
        map
    }

    /// A mob of `kind` standing on the ground of `ground_world` at `(x, z)`.
    fn standing(kind: MobKind, x: f32, z: f32) -> ServerMob {
        ServerMob {
            on_ground: true,
            ..ServerMob::new(kind, ServerMob::standing_at(kind, x, 1.0, z))
        }
    }

    fn fleeing_fox_at_origin(target: Vec3) -> ServerMob {
        ServerMob {
            action: MobAction::Flee,
            target: MobTarget::Position(target),
            ..standing(MobKind::Fox, 0.5, 0.5)
        }
    }

    /// Runs `seconds` of `mob` in `map`, and returns the map.
    fn run_seconds(mut map: ServerWorldMap, mob: ServerMob, seconds: u64) -> ServerWorldMap {
        map.mobs.insert(0, mob);
        let mut world = World::new();
        world.insert_resource(map);
        let mut time = Time::<Fixed>::default();
        time.advance_by(tick());
        world.insert_resource(time);
        for _ in 0..seconds * TICKS_PER_SECOND {
            world.run_system_once(mob_behavior_system).unwrap();
        }
        world.remove_resource::<ServerWorldMap>().unwrap()
    }

    fn hitbox(mob: &ServerMob) -> Aabb3d {
        Aabb3d::new(
            mob.position,
            Vec3::new(mob.width, mob.height, mob.depth) / 2.0,
        )
    }

    #[test]
    fn a_fleeing_mob_does_not_sink_when_its_target_is_above() {
        let standing_y = ServerMob::standing_at(MobKind::Fox, 0.0, 1.0, 0.0).y;
        for target in [Vec3::new(0.5, 9.0, 0.5), Vec3::new(3.5, 8.0, 0.5)] {
            let map = run_seconds(ground_world(false), fleeing_fox_at_origin(target), 5);
            let fox = &map.mobs[&0];
            assert!(
                (fox.position.y - standing_y).abs() < 0.05,
                "target {target}: y = {}",
                fox.position.y
            );
        }
    }

    #[test]
    fn a_fleeing_mob_does_not_enter_a_wall() {
        let target = Vec3::new(5.5, 1.5, 0.5);
        let map = run_seconds(ground_world(true), fleeing_fox_at_origin(target), 2);
        let fox = map.mobs[&0].clone();
        assert!(
            !map.chunks.check_collision_box(&hitbox(&fox)),
            "fox at {}",
            fox.position
        );
        let half = fox.width / 2.0;
        assert!(
            (fox.position.x - (-1.0 + half)).abs() < 0.25,
            "the fox should have run up to the wall and stopped: {}",
            fox.position
        );
    }

    #[test]
    fn a_fleeing_mob_runs_away_horizontally_on_open_ground() {
        let target = Vec3::new(3.5, 1.35, 4.5);
        let start = fleeing_fox_at_origin(target).position;
        let map = run_seconds(ground_world(false), fleeing_fox_at_origin(target), 1);
        let moved = map.mobs[&0].position - start;
        let away = Vec3::new(-3.0, 0.0, -4.0).normalize();
        let expected = flee_speed(MobKind::Fox);
        assert!((moved.dot(away) - expected).abs() < 0.2, "moved {moved}");
        assert!(moved.y.abs() < 0.05);
    }

    #[test]
    fn a_resting_animal_walks_to_a_spot_nearby_then_rests_again() {
        let start = standing(MobKind::Sheep, 0.5, 0.5);
        let mut map = ground_world(false);
        map.mobs.insert(
            0,
            ServerMob {
                action_ticks: 1,
                ..start.clone()
            },
        );
        let mut rng = StdRng::seed_from_u64(1);
        let mut changed = HashSet::new();
        let mut step = |map: &mut ServerWorldMap, changed: &mut HashSet<MobId>| {
            let delta = tick().as_secs_f32();
            step_mobs(
                &mut map.mobs,
                &map.chunks,
                &map.players,
                changed,
                delta,
                &mut rng,
            );
        };

        step(&mut map, &mut changed);
        let mob = &map.mobs[&0];
        assert_eq!(mob.action, MobAction::Walk);
        let MobTarget::Position(spot) = mob.target else {
            panic!("no spot: {:?}", mob.target);
        };
        let distance = (spot - start.position).xz().length();
        assert!(WANDER_DISTANCE.contains(&distance), "{distance}");
        assert_eq!(spot.y, start.position.y, "the spot is on the ground");
        assert!(changed.contains(&0));

        for _ in 0..WALK_TICKS {
            step(&mut map, &mut changed);
            if map.mobs[&0].action != MobAction::Walk {
                break;
            }
        }
        let mob = &map.mobs[&0];
        assert!(matches!(mob.action, MobAction::Idle | MobAction::Graze));
        assert!((mob.position - spot).xz().length() < ARRIVAL_DISTANCE + 0.1);
        assert!(!map.chunks.check_collision_box(&hitbox(mob)));
    }

    #[test]
    fn an_animal_resting_on_the_ground_is_not_sent_again() {
        let mut map = ground_world(false);
        map.mobs.insert(
            0,
            ServerMob {
                action_ticks: 100,
                ..standing(MobKind::Sheep, 0.5, 0.5)
            },
        );
        let mut changed = HashSet::new();
        let delta = tick().as_secs_f32();
        let mut rng = StdRng::seed_from_u64(1);
        step_mobs(
            &mut map.mobs,
            &map.chunks,
            &map.players,
            &mut changed,
            delta,
            &mut rng,
        );
        assert!(changed.is_empty());
    }

    #[test]
    fn an_animal_does_not_walk_into_water() {
        let mut map = ground_world(false);
        // water all around a single dry block
        map.chunks
            .fill_box(IVec3::new(-10, 1, -10), IVec3::new(9, 1, 9), BlockId::Water);
        map.chunks
            .fill_box(IVec3::new(0, 1, 0), IVec3::new(0, 1, 0), BlockId::Stone);
        let sheep = ServerMob {
            action_ticks: 1,
            ..ServerMob::new(
                MobKind::Sheep,
                ServerMob::standing_at(MobKind::Sheep, 0.5, 2.0, 0.5),
            )
        };
        let map = run_seconds(map, sheep, 30);
        let sheep = &map.mobs[&0];
        assert_eq!(sheep.position.xz(), Vec2::new(0.5, 0.5));
    }

    /// Runs `ticks` steps of the mob 0 of `map`.
    fn run_ticks(map: &mut ServerWorldMap, ticks: u64) {
        let mut rng = StdRng::seed_from_u64(1);
        for _ in 0..ticks {
            step_mobs(
                &mut map.mobs,
                &map.chunks,
                &map.players,
                &mut HashSet::new(),
                tick().as_secs_f32(),
                &mut rng,
            );
        }
    }

    #[test]
    fn a_walking_sheep_goes_at_its_speed_and_jumps_onto_a_step_in_its_way() {
        // A step one block high from x = 3 on
        let mut map = ground_world(false);
        map.chunks
            .fill_box(IVec3::new(3, 1, -10), IVec3::new(9, 1, 9), BlockId::Stone);
        let start = standing(MobKind::Sheep, 0.5, 0.5);
        let target = ServerMob::standing_at(MobKind::Sheep, 6.5, 2.0, 3.5);
        map.mobs.insert(
            0,
            ServerMob {
                action: MobAction::Walk,
                target: MobTarget::Position(target),
                action_ticks: WALK_TICKS,
                ..start.clone()
            },
        );
        // A second on the flat ground: a block's walk, toward the target
        run_ticks(&mut map, TICKS_PER_SECOND);
        let sheep = &map.mobs[&0];
        let walked = (sheep.position - start.position).xz();
        assert!((walked.length() - 1.0).abs() < 0.06, "{walked}");
        assert!(
            walked
                .normalize()
                .distance((target - start.position).xz().normalize())
                < 1e-3
        );
        assert!((sheep.position.y - start.position.y).abs() < 0.05);

        // Then up the step, and on to the target
        for _ in 0..WALK_TICKS {
            run_ticks(&mut map, 1);
            if map.mobs[&0].action != MobAction::Walk {
                break;
            }
        }
        let sheep = &map.mobs[&0];
        assert_ne!(sheep.action, MobAction::Walk, "{}", sheep.position);
        assert!((sheep.position - target).xz().length() < ARRIVAL_DISTANCE + 0.1);
        // On top of the step, within a step of gravity: a mob stops above the block it meets
        assert!(
            (sheep.position.y - target.y).abs() < 0.1,
            "{}",
            sheep.position
        );
    }

    /// The ground of `ground_world`, with an open pair of portals: the blue one on a wall
    /// facing south at z = -4, x in 0..2, its bottom on the ground, and the orange one on a
    /// wall facing west at x = 8, z in 0..2. North into the blue one is west out of the orange
    /// one. With `floor`, the blue one lies on the ground instead, x in 0..2 and z in -2..1,
    /// its up north: down into it is west out of the orange one.
    fn portal_world(floor: bool) -> ServerWorldMap {
        let mut map = ground_world(false);
        map.chunks
            .fill_box(IVec3::new(8, 1, -6), IVec3::new(8, 5, 6), BlockId::Stone);
        let blue = if floor {
            Portal {
                owner: 7,
                color: PortalColor::Blue,
                anchor: IVec3::new(0, 0, 0),
                face: FaceDirection::PlusY,
                up: FaceDirection::MinusZ,
            }
        } else {
            map.chunks
                .fill_box(IVec3::new(-3, 1, -4), IVec3::new(4, 5, -4), BlockId::Stone);
            Portal {
                owner: 7,
                color: PortalColor::Blue,
                anchor: IVec3::new(0, 1, -4),
                face: FaceDirection::PlusZ,
                up: FaceDirection::PlusY,
            }
        };
        let portals = map.chunks.portals_mut();
        portals.insert(blue);
        portals.insert(Portal {
            owner: 7,
            color: PortalColor::Orange,
            anchor: IVec3::new(8, 1, 0),
            face: FaceDirection::MinusX,
            up: FaceDirection::PlusY,
        });
        map
    }

    /// Steps the mob 0 of `map` until it went through a portal, and returns it as it was
    /// before that step.
    fn step_through(map: &mut ServerWorldMap) -> ServerMob {
        let mut rng = StdRng::seed_from_u64(1);
        let delta = tick().as_secs_f32();
        for _ in 0..10 * TICKS_PER_SECOND {
            let before = map.mobs[&0].clone();
            step_mobs(
                &mut map.mobs,
                &map.chunks,
                &map.players,
                &mut HashSet::new(),
                delta,
                &mut rng,
            );
            if map.mobs[&0].passes != before.passes {
                return before;
            }
        }
        panic!("never went through, at {}", map.mobs[&0].position);
    }

    #[test]
    fn a_sheep_walks_through_a_wall_portal_and_on_out_of_its_twin() {
        let mut map = portal_world(false);
        let start = standing(MobKind::Sheep, 1.0, -1.0);
        let target = start.position.with_z(-9.0);
        map.mobs.insert(
            0,
            ServerMob {
                action: MobAction::Walk,
                target: MobTarget::Position(target),
                action_ticks: WALK_TICKS,
                ..start.clone()
            },
        );
        step_through(&mut map);
        let sheep = map.mobs[&0].clone();
        assert_eq!(sheep.passes, 1);
        // Out of the orange portal, at the height it walked in, facing west, its target 6
        // blocks west of the orange portal as it was 6 blocks north of the blue one
        assert!(
            sheep.position.x < 8.0 && sheep.position.x > 7.0,
            "{}",
            sheep.position
        );
        assert!((sheep.position.z - 1.0).abs() < 0.01, "{}", sheep.position);
        assert!(
            (sheep.position.y - start.position.y).abs() < 0.01,
            "{}",
            sheep.position
        );
        let facing = sheep.rotation * Vec3::Z;
        assert!(facing.distance(Vec3::NEG_X) < 1e-4, "{facing}");
        let MobTarget::Position(target) = sheep.target else {
            panic!("{:?}", sheep.target);
        };
        assert!(
            target.distance(Vec3::new(2.0, start.position.y, 1.0)) < 1e-3,
            "{target}"
        );

        // It walks on to its target, west, and rests there
        for _ in 0..10 * TICKS_PER_SECOND {
            let mut changed = HashSet::new();
            step_mobs(
                &mut map.mobs,
                &map.chunks,
                &map.players,
                &mut changed,
                tick().as_secs_f32(),
                &mut StdRng::seed_from_u64(2),
            );
            if map.mobs[&0].action != MobAction::Walk {
                break;
            }
        }
        let sheep = &map.mobs[&0];
        assert_ne!(sheep.action, MobAction::Walk);
        assert_eq!(sheep.passes, 1);
        assert!(
            (sheep.position - target).xz().length() < ARRIVAL_DISTANCE + 0.1,
            "{}",
            sheep.position
        );
        assert!(!map.chunks.check_collision_box(&hitbox(sheep)));
    }

    #[test]
    fn a_sheep_too_wide_for_where_it_meets_a_portal_is_stopped_by_its_wall() {
        // Nothing beside the opening on the west: only the portal's wall stops a box over its
        // edge. The centre is within the opening, but not within the opening shrunk by half the
        // sheep's width
        let mut map = portal_world(false);
        for x in -3..=-1 {
            for y in 1..=5 {
                map.chunks.remove_block(IVec3::new(x, y, -4));
            }
        }
        let start = standing(MobKind::Sheep, 0.3, -1.0);
        let sheep = ServerMob {
            action: MobAction::Walk,
            target: MobTarget::Position(start.position.with_z(-9.0)),
            action_ticks: WALK_TICKS,
            ..start
        };
        let map = run_seconds(map, sheep, 4);
        let sheep = &map.mobs[&0];
        assert_eq!(sheep.passes, 0);
        assert!(sheep.position.z > -3.0 + 0.4, "{}", sheep.position);
        assert!(!map.chunks.check_collision_box(&hitbox(sheep)));
    }

    #[test]
    fn a_sheep_falling_into_a_floor_portal_flies_out_of_a_wall_and_stops_walking() {
        let mut map = portal_world(true);
        let start = Vec3::new(1.0, 6.0, -1.0);
        let sheep = ServerMob {
            action: MobAction::Walk,
            target: MobTarget::Position(start.with_z(-8.0)),
            action_ticks: WALK_TICKS,
            ..ServerMob::new(MobKind::Sheep, start)
        };
        map.mobs.insert(0, sheep);
        let before = step_through(&mut map);
        let sheep = map.mobs[&0].clone();
        assert!(
            sheep.position.x < 8.0 && sheep.position.x > 7.0,
            "{}",
            sheep.position
        );
        // Its velocity turned west, as fast as it fell, within a step of gravity
        assert!(
            (sheep.velocity.x - before.velocity.y).abs() < -GRAVITY * tick().as_secs_f32() + 1e-3,
            "{} out, {} in",
            sheep.velocity,
            before.velocity
        );
        assert!(!sheep.on_ground);
        // The spot it walked to is not on the ground of the other side: it stops walking
        assert_ne!(sheep.action, MobAction::Walk);
        assert!(
            matches!(sheep.target, MobTarget::None),
            "{:?}",
            sheep.target
        );

        // It flies west, out of the wall, and lands more than a block away from it
        run_ticks(&mut map, 2 * TICKS_PER_SECOND);
        let sheep = &map.mobs[&0];
        assert_eq!(sheep.passes, 1);
        assert!(sheep.position.x < 7.0, "{}", sheep.position);
        assert!(
            (sheep.position.y - ServerMob::standing_at(MobKind::Sheep, 0.0, 1.0, 0.0).y).abs()
                < 0.1,
            "{}",
            sheep.position
        );
        // Landing stopped the flight: it may walk again, no faster than it walks
        assert!(!is_flung(sheep), "{}", sheep.velocity);
        assert!(!map.chunks.check_collision_box(&hitbox(sheep)));
    }

    /// A sheep resting in the air at `position`, which does not start a walk.
    fn dropped_sheep(position: Vec3) -> ServerMob {
        ServerMob {
            action: MobAction::Idle,
            action_ticks: u32::MAX,
            ..ServerMob::new(MobKind::Sheep, position)
        }
    }

    #[test]
    fn a_sheep_dropped_from_high_into_a_floor_portal_comes_out_clear_of_blocks() {
        // A wall two blocks in front of the orange portal: its front cells are clear, the next
        // ones are not
        let mut map = portal_world(true);
        map.chunks
            .fill_box(IVec3::new(6, 1, -2), IVec3::new(6, 4, 3), BlockId::Stone);
        // The chunks above the portal, for the sheep to fall from 20 blocks up
        for (x, z) in [(0, -1), (0, 0)] {
            map.chunks
                .insert_chunk(IVec3::new(x, 1, z), shared::world::Chunk::new());
        }
        map.mobs
            .insert(0, dropped_sheep(Vec3::new(1.0, 21.0, -0.5)));
        let before = step_through(&mut map);
        assert!(before.velocity.y < -30.0, "{}", before.velocity);
        let sheep = map.mobs[&0].clone();
        assert!(
            !collides_at(&sheep, &map.chunks, sheep.position),
            "{}",
            sheep.position
        );
        // It flies west into the wall, and falls to its foot
        for _ in 0..2 * TICKS_PER_SECOND {
            run_ticks(&mut map, 1);
            let sheep = &map.mobs[&0];
            assert!(
                !collides_at(sheep, &map.chunks, sheep.position),
                "{}",
                sheep.position
            );
        }
        let sheep = &map.mobs[&0];
        assert_eq!(sheep.passes, 1);
        let standing = ServerMob::standing_at(MobKind::Sheep, 0.0, 1.0, 0.0).y;
        assert!(
            (sheep.position.y - standing).abs() < 0.1 && sheep.position.x > 7.0,
            "{}",
            sheep.position
        );
    }

    /// A sheep standing still on a floor portal whose twin is a floor too falls in, comes out
    /// of the twin too slowly to leave it, and stands on it: it goes through once, not back and
    /// forth for ever.
    #[test]
    fn a_sheep_resting_on_a_floor_portal_with_a_floor_twin_goes_through_once_and_stands() {
        let mut map = portal_world(true);
        map.chunks.portals_mut().insert(Portal {
            owner: 7,
            color: PortalColor::Orange,
            anchor: IVec3::new(4, 0, 0),
            face: FaceDirection::PlusY,
            up: FaceDirection::MinusZ,
        });
        map.mobs.insert(
            0,
            ServerMob {
                action: MobAction::Idle,
                action_ticks: u32::MAX,
                ..standing(MobKind::Sheep, 1.0, -0.5)
            },
        );
        for tick in 0..5 * TICKS_PER_SECOND {
            run_ticks(&mut map, 1);
            let sheep = &map.mobs[&0];
            assert!(
                sheep.passes <= 1,
                "tick {tick}: through {} times, at {}",
                sheep.passes,
                sheep.position
            );
            assert!(!collides_at(sheep, &map.chunks, sheep.position));
        }
        let sheep = &map.mobs[&0];
        assert_eq!(sheep.passes, 1);
        let standing = ServerMob::standing_at(MobKind::Sheep, 5.0, 1.0, -0.5);
        assert!(
            sheep.position.distance(standing) < 0.1,
            "{}",
            sheep.position
        );
    }

    /// `portal_world` with a floor portal, and a stone ceiling 3 blocks above the ground, the
    /// orange portal under it, above the blue one: what falls into the floor falls out of the
    /// ceiling, faster and faster. A sheep dropped between them.
    fn falling_for_ever() -> ServerWorldMap {
        let mut map = portal_world(true);
        map.chunks
            .fill_box(IVec3::new(-3, 4, -4), IVec3::new(4, 4, 3), BlockId::Stone);
        map.chunks.portals_mut().insert(Portal {
            owner: 7,
            color: PortalColor::Orange,
            anchor: IVec3::new(1, 4, 0),
            face: FaceDirection::MinusY,
            up: FaceDirection::MinusZ,
        });
        map.mobs.insert(0, dropped_sheep(Vec3::new(1.0, 2.0, -0.5)));
        map
    }

    #[test]
    fn a_sheep_falling_for_long_between_a_floor_and_a_ceiling_portal_never_ends_in_a_block() {
        let mut map = falling_for_ever();
        let mut passes = 0;
        for tick in 0..10 * TICKS_PER_SECOND {
            run_ticks(&mut map, 1);
            let sheep = &map.mobs[&0];
            assert!(
                !collides_at(sheep, &map.chunks, sheep.position),
                "tick {tick}: {} at {}",
                sheep.position,
                sheep.velocity
            );
            passes = sheep.passes;
        }
        // Still falling, through the pair again and again
        assert!(passes > 50, "{passes}");
        let sheep = &map.mobs[&0];
        assert!(!sheep.on_ground, "{}", sheep.position);
    }

    #[test]
    fn a_sheep_at_the_terminal_speed_lands_on_a_floor_one_block_thick() {
        // The ground of `ground_world` is one block thick, with air under it; a step of the fall
        // would end below it
        let mut map = ground_world(false);
        map.mobs.insert(
            0,
            ServerMob {
                velocity: Vec3::new(0.0, -TERMINAL_VELOCITY, 0.0),
                ..dropped_sheep(Vec3::new(0.5, 2.5, 0.5))
            },
        );
        run_ticks(&mut map, TICKS_PER_SECOND);
        let sheep = &map.mobs[&0];
        let standing = ServerMob::standing_at(MobKind::Sheep, 0.0, 1.0, 0.0).y;
        assert!(
            (sheep.position.y - standing).abs() < 0.1,
            "{}",
            sheep.position
        );
        assert!(!collides_at(sheep, &map.chunks, sheep.position));
    }

    #[test]
    fn a_sheep_out_of_a_ceiling_portal_at_the_terminal_speed_lands_on_a_thin_floor_under_it() {
        // The blue portal on the ground, the orange one under a stone ceiling at y = 8, two
        // blocks above a stone floor one block thick (y = 5), with air under it
        let mut map = portal_world(true);
        map.chunks
            .fill_box(IVec3::new(4, 8, -3), IVec3::new(7, 8, 1), BlockId::Stone);
        map.chunks
            .fill_box(IVec3::new(3, 5, -4), IVec3::new(7, 5, 2), BlockId::Stone);
        map.chunks.portals_mut().insert(Portal {
            owner: 7,
            color: PortalColor::Orange,
            anchor: IVec3::new(6, 8, 0),
            face: FaceDirection::MinusY,
            up: FaceDirection::MinusZ,
        });
        map.mobs.insert(
            0,
            ServerMob {
                velocity: Vec3::new(0.0, -TERMINAL_VELOCITY, 0.0),
                ..dropped_sheep(Vec3::new(1.0, 2.0, -0.5))
            },
        );
        step_through(&mut map);
        for _ in 0..2 * TICKS_PER_SECOND {
            run_ticks(&mut map, 1);
            let sheep = &map.mobs[&0];
            assert!(sheep.position.y > 6.0, "{}", sheep.position);
            assert!(!collides_at(sheep, &map.chunks, sheep.position));
        }
        let sheep = &map.mobs[&0];
        assert_eq!(sheep.passes, 1);
        let standing = ServerMob::standing_at(MobKind::Sheep, 0.0, 6.0, 0.0).y;
        assert!(
            (sheep.position.y - standing).abs() < 0.1,
            "{}",
            sheep.position
        );
    }

    #[test]
    fn a_sheep_falling_for_long_between_a_floor_and_a_ceiling_portal_keeps_to_the_terminal_speed() {
        let mut map = falling_for_ever();
        // 30 s: without a cap, 900 blocks/s
        for _ in 0..30 * TICKS_PER_SECOND {
            run_ticks(&mut map, 1);
            let velocity = map.mobs[&0].velocity;
            assert!(velocity.length() <= TERMINAL_VELOCITY + 1e-3, "{velocity}");
        }
        let velocity = map.mobs[&0].velocity;
        assert!((velocity.y + TERMINAL_VELOCITY).abs() < 1e-3, "{velocity}");
    }

    fn player_at(id: PlayerId, position: Vec3) -> Player {
        Player::new(
            id,
            "tester".to_string(),
            position,
            Transform::from_translation(position),
        )
    }

    #[test]
    fn a_player_pushes_a_sheep_it_walks_into() {
        let mut map = ground_world(false);
        map.players
            .insert(7, player_at(7, Vec3::new(0.1, 1.9, 0.5)));
        let sheep = ServerMob {
            action_ticks: 1000,
            ..standing(MobKind::Sheep, 0.5, 0.5)
        };
        let map = run_seconds(map, sheep, 1);
        let sheep = &map.mobs[&0];
        let player = &map.players[&7];
        let touching = (sheep.width + player.width) / 2.0;
        assert!((sheep.position.x - player.position.x - touching).abs() < 0.05);
        assert!((sheep.position.z - 0.5).abs() < 0.01);
        assert!(map.changed_mobs.contains(&0));
    }

    #[test]
    fn a_player_who_flies_or_stands_above_pushes_nothing() {
        for (height, flying) in [(1.9, true), (4.0, false)] {
            let mut map = ground_world(false);
            let mut player = player_at(7, Vec3::new(0.1, height, 0.5));
            player.is_flying = flying;
            map.players.insert(7, player);
            let sheep = ServerMob {
                action_ticks: 1000,
                ..standing(MobKind::Sheep, 0.5, 0.5)
            };
            let map = run_seconds(map, sheep, 1);
            assert_eq!(map.mobs[&0].position.xz(), Vec2::new(0.5, 0.5));
        }
    }

    #[test]
    fn a_sheep_is_not_pushed_into_a_wall() {
        let mut map = ground_world(true);
        map.players
            .insert(7, player_at(7, Vec3::new(0.0, 1.9, 0.5)));
        let sheep = ServerMob {
            action_ticks: 1000,
            ..standing(MobKind::Sheep, -0.5, 0.5)
        };
        let map = run_seconds(map, sheep, 1);
        assert!(!map.chunks.check_collision_box(&hitbox(&map.mobs[&0])));
    }

    #[test]
    fn two_animals_on_the_same_spot_come_apart() {
        let mut map = ground_world(false);
        let cow = ServerMob {
            action_ticks: 1000,
            ..standing(MobKind::Cow, 0.5, 0.5)
        };
        map.mobs.insert(1, cow.clone());
        let map = run_seconds(map, cow, 1);
        let apart = map.mobs[&0].position.distance(map.mobs[&1].position);
        assert!((apart - 0.9).abs() < 0.05, "{apart}");
    }

    #[test]
    fn a_fox_runs_from_a_player_who_comes_close_and_a_sheep_does_not() {
        for (kind, flees) in [(MobKind::Fox, true), (MobKind::Sheep, false)] {
            let mut map = ground_world(false);
            let player = player_at(7, Vec3::new(3.5, 1.9, 0.5));
            map.players.insert(7, player);
            let mob = ServerMob {
                action_ticks: 1000,
                ..standing(kind, 0.5, 0.5)
            };
            let map = run_seconds(map, mob, 1);
            let mob = &map.mobs[&0];
            assert_eq!(mob.action == MobAction::Flee, flees, "{kind:?}");
            assert_eq!(mob.position.x < 0.0, flees, "{kind:?} at {}", mob.position);
        }
    }
}
