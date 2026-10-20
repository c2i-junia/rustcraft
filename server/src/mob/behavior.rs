//! How mobs live, once per fixed step: gravity, then what their action asks. An animal rests
//! (a sheep grazes while it does), walks to a spot nearby, and rests again; a fox runs away
//! from a player who comes too close.

use std::collections::{HashMap, HashSet};
use std::ops::Range;

use bevy::{
    math::{bounding::Aabb3d, ops::atan2, IVec3, Quat, Vec3},
    time::{Fixed, Time},
};
use bevy_ecs::system::{Res, ResMut};
use rand::Rng;
use shared::{
    messages::PlayerId,
    players::{
        constants::{GRAVITY, JUMP_VELOCITY, MAX_UPWARD_VELOCITY},
        movement::is_terrain_loaded,
        Player,
    },
    world::{BlockHitbox, BlockId, ChunkMap, MobAction, MobId, MobKind, MobTarget, ServerMob},
    TICKS_PER_SECOND,
};

use crate::world::data::ServerWorldMap;

/// A walking mob this close to its target, on the horizontal plane, has arrived.
const ARRIVAL_DISTANCE: f32 = 0.5;
/// A fleeing mob stops once it is this far from its target.
const SAFE_DISTANCE: f32 = 15.0;
/// A fox runs away from a player who comes this close.
const FEAR_DISTANCE: f32 = 6.0;
/// How long an animal rests before it walks somewhere else, in ticks: 3 to 10 seconds.
const REST_TICKS: Range<u32> = 3 * TICKS_PER_SECOND as u32..10 * TICKS_PER_SECOND as u32;
/// How often a sheep that rests grazes.
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

/// Speed of a walking mob, in blocks per second.
fn walk_speed(kind: MobKind) -> f32 {
    match kind {
        MobKind::Fox => 1.6,
        MobKind::Sheep => 1.0,
    }
}

/// Speed of a fleeing mob, in blocks per second.
fn flee_speed(kind: MobKind) -> f32 {
    match kind {
        MobKind::Fox => 4.0,
        MobKind::Sheep => 2.0,
    }
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

fn step_mob(
    mob: &mut ServerMob,
    chunks: &ChunkMap,
    players: &HashMap<PlayerId, Player>,
    delta: f32,
    rng: &mut impl Rng,
) {
    apply_gravity(mob, chunks, delta);
    if mob.kind == MobKind::Fox && mob.action != MobAction::Flee {
        if let Some(id) = nearest_player(players, mob.position, FEAR_DISTANCE) {
            mob.action = MobAction::Flee;
            mob.target = MobTarget::Player(id);
        }
    }
    match mob.action {
        MobAction::Idle | MobAction::Graze => {
            mob.action_ticks = mob.action_ticks.saturating_sub(1);
            if mob.action_ticks == 0 {
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

/// Stops the mob for a while: a sheep grazes some of the time.
fn rest(mob: &mut ServerMob, rng: &mut impl Rng) {
    mob.action = if mob.kind == MobKind::Sheep && rng.gen_bool(GRAZE_CHANCE) {
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

/// Whether `mob` would be inside a block at `position`.
fn collides_at(mob: &ServerMob, chunks: &ChunkMap, position: Vec3) -> bool {
    chunks.check_collision_box(&Aabb3d::new(
        position,
        Vec3::new(mob.width, mob.height, mob.depth) / 2.0,
    ))
}

/// Same vertical motion as a player: fall while in the air, land on the first block met. Half
/// the gravity is added before the move and half after it, so that a jump rises as high as a
/// player's at the 20 steps per second of the mobs.
fn apply_gravity(mob: &mut ServerMob, chunks: &ChunkMap, delta: f32) {
    if !mob.on_ground {
        mob.velocity.y += GRAVITY * delta / 2.0;
    }
    let new_y = mob.position.y + mob.velocity.y * delta;
    mob.velocity.y = mob.velocity.y.min(MAX_UPWARD_VELOCITY);

    if collides_at(mob, chunks, mob.position.with_y(new_y)) {
        mob.on_ground = true;
        mob.velocity.y = 0.0;
    } else {
        mob.position.y = new_y;
        mob.on_ground = false;
        mob.velocity.y += GRAVITY * delta / 2.0;
    }
}

fn jump(mob: &mut ServerMob) {
    mob.velocity.y += JUMP_VELOCITY;
    mob.on_ground = false;
    mob.velocity.x = 0.0;
    mob.velocity.z = 0.0;
}

/// One step along `direction` at `speed`. A mob blocked while it was moving jumps; one blocked
/// from a standstill slides along the obstacle on one axis, and jumps if both are blocked.
fn walk(mob: &mut ServerMob, chunks: &ChunkMap, direction: Vec3, speed: f32, delta: f32) {
    let step = Vec3::new(direction.x, 0.0, direction.z) * (speed * delta);
    let along_x = Vec3::new(step.x, 0.0, 0.0);
    let along_z = Vec3::new(0.0, 0.0, step.z);

    if !collides_at(mob, chunks, mob.position + step) {
        mob.position += step;
        mob.velocity.x = step.x;
        mob.velocity.z = step.z;
    } else if mob.on_ground && mob.velocity.x != 0.0 && mob.velocity.z != 0.0 {
        jump(mob);
    } else if mob.on_ground {
        if !collides_at(mob, chunks, mob.position + along_x) {
            mob.position += along_x;
            mob.velocity.x = step.x;
        } else if !collides_at(mob, chunks, mob.position + along_z) {
            mob.position += along_z;
            mob.velocity.z = step.z;
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
    if !collides_at(mob, chunks, fled) {
        mob.position = fled;
    }
    if away != Vec3::ZERO {
        mob.rotation = Quat::from_rotation_y(atan2(away.x, away.z));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;
    use bevy::math::{IVec3, Vec2, Vec3, Vec3Swizzles};
    use bevy::prelude::{Transform, World};
    use rand::{rngs::StdRng, SeedableRng};
    use shared::world::{BlockId, ChunkMap, MobKind, ServerMob};
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

    fn player_at(id: PlayerId, position: Vec3) -> Player {
        Player::new(
            id,
            "tester".to_string(),
            position,
            Transform::from_translation(position),
        )
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
