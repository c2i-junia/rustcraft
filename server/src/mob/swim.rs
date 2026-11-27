//! How the fish live (`MobKind::is_aquatic`): in the water they hang still a while, then swim to
//! a spot nearby, or back to their shoal (`school`), and dart away from a player who comes close
//! or strikes them. They never leave the water by themselves; one out of it flops towards the
//! nearest water.

use std::collections::HashMap;

use bevy::math::{ops::atan2, IVec3, Quat, Vec3};
use rand::Rng;
use shared::{
    messages::PlayerId,
    players::Player,
    world::{ChunkMap, MobAction, MobId, MobTarget, ServerMob},
    TICKS_PER_SECOND,
};

use super::behavior::{is_in_water, jump, may_move_to, WALK_TICKS};

/// How fast a fish swims about, and darts away, in blocks per second.
pub const SWIM_SPEED: f32 = 1.6;
pub const DART_SPEED: f32 = 4.5;
/// A fish darts away from a player this close, and stops once this far from it.
const SCARE_DISTANCE: f32 = 4.0;
const CALM_DISTANCE: f32 = 9.0;
/// How far from where it is a fish swims to, along each axis.
const ROAM: f32 = 4.0;
/// How long a fish hangs still, in ticks: half a second to 3 seconds.
const HOVER_TICKS: std::ops::Range<u32> = TICKS_PER_SECOND as u32 / 2..3 * TICKS_PER_SECOND as u32;
/// A fish within this distance of its spot is there.
const ARRIVED: f32 = 0.4;
/// The fish of a kind within this distance of one are its shoal, which it swims back to.
const SHOAL_DISTANCE: f32 = 10.0;
/// How far around its shoal a fish swims to, along each axis.
const SHOAL_SPREAD: f32 = 1.5;
/// How far a fish out of the water looks for it, in blocks along each axis.
const WATER_SEARCH: i32 = 4;

/// Sends each fish about to choose a spot back towards its shoal, the middle of the fish of its
/// kind around it, if it has strayed from it.
pub fn school(mobs: &mut HashMap<MobId, ServerMob>, rng: &mut impl Rng) {
    let fish: Vec<(MobId, ServerMob)> = mobs
        .iter()
        .filter(|(_, mob)| mob.kind.is_aquatic())
        .map(|(id, mob)| (*id, mob.clone()))
        .collect();
    for (id, mob) in &fish {
        if mob.action != MobAction::Idle || mob.action_ticks > 1 {
            continue;
        }
        let shoal: Vec<Vec3> = fish
            .iter()
            .filter(|(other, fellow)| {
                other != id
                    && fellow.kind == mob.kind
                    && fellow.position.distance(mob.position) < SHOAL_DISTANCE
            })
            .map(|(_, fellow)| fellow.position)
            .collect();
        if shoal.is_empty() {
            continue;
        }
        let middle = shoal.iter().sum::<Vec3>() / shoal.len() as f32;
        if middle.distance(mob.position) < SHOAL_SPREAD * 2.0 {
            continue;
        }
        let spread = Vec3::new(
            rng.gen_range(-SHOAL_SPREAD..SHOAL_SPREAD),
            rng.gen_range(-0.5..0.5),
            rng.gen_range(-SHOAL_SPREAD..SHOAL_SPREAD),
        );
        if let Some(mob) = mobs.get_mut(id) {
            mob.action = MobAction::Walk;
            mob.target = MobTarget::Position(middle + spread);
            mob.action_ticks = WALK_TICKS / 2;
        }
    }
}

/// One step of a fish.
pub fn act(
    mob: &mut ServerMob,
    chunks: &ChunkMap,
    players: &HashMap<PlayerId, Player>,
    delta: f32,
    rng: &mut impl Rng,
) {
    if !is_in_water(chunks, mob.position) {
        return flop(mob, chunks, rng);
    }
    mob.velocity = Vec3::ZERO;
    // A player close by scares it, one that struck it until it is far
    let nearest = players
        .values()
        .filter(|player| !player.is_dead())
        .map(|player| (player.id, player.position.distance(mob.position)))
        .min_by(|a, b| a.1.total_cmp(&b.1));
    if mob.action != MobAction::Flee {
        if let Some((id, _)) = nearest.filter(|&(_, d)| d < SCARE_DISTANCE) {
            mob.action = MobAction::Flee;
            mob.target = MobTarget::Player(id);
            mob.action_ticks = 0;
        }
    }
    match mob.action {
        MobAction::Flee => {
            let threat = match mob.target {
                MobTarget::Player(id) => players.get(&id).map(|player| player.position),
                _ => None,
            };
            let Some(threat) = threat.filter(|t| t.distance(mob.position) < CALM_DISTANCE) else {
                return hover(mob, rng);
            };
            if mob.action_ticks > 0 {
                mob.action_ticks -= 1;
            }
            let away = (mob.position - threat).normalize_or_zero();
            if !swim_towards(mob, chunks, away, DART_SPEED * delta) {
                // Cornered: anywhere else
                let aside = Vec3::new(rng.gen_range(-1.0..1.0), 0.0, rng.gen_range(-1.0..1.0));
                swim_towards(mob, chunks, aside.normalize_or_zero(), DART_SPEED * delta);
            }
        }
        MobAction::Walk => {
            mob.action_ticks = mob.action_ticks.saturating_sub(1);
            let MobTarget::Position(spot) = mob.target else {
                return hover(mob, rng);
            };
            let towards = spot - mob.position;
            if towards.length() < ARRIVED || mob.action_ticks == 0 {
                return hover(mob, rng);
            }
            if !swim_towards(mob, chunks, towards.normalize(), SWIM_SPEED * delta) {
                hover(mob, rng);
            }
        }
        _ => {
            mob.action_ticks = mob.action_ticks.saturating_sub(1);
            if mob.action_ticks == 0 {
                let offset = Vec3::new(
                    rng.gen_range(-ROAM..ROAM),
                    rng.gen_range(-ROAM / 2.0..ROAM / 2.0),
                    rng.gen_range(-ROAM..ROAM),
                );
                mob.action = MobAction::Walk;
                mob.target = MobTarget::Position(mob.position + offset);
                mob.action_ticks = WALK_TICKS / 2;
            }
        }
    }
}

/// Lets the fish hang still for a while.
fn hover(mob: &mut ServerMob, rng: &mut impl Rng) {
    mob.action = MobAction::Idle;
    mob.target = MobTarget::None;
    mob.action_ticks = rng.gen_range(HOVER_TICKS);
}

/// Whether a fish's box at `position` lies in the water: its middle, its bottom and its top.
fn in_water_at(mob: &ServerMob, chunks: &ChunkMap, position: Vec3) -> bool {
    let half = mob.height / 2.0;
    [-half, 0.0, half]
        .into_iter()
        .all(|dy| is_in_water(chunks, position + Vec3::Y * dy))
}

/// Swims `distance` along `direction`, turned towards it, if it stays in the water and nothing is
/// in its way; else along the level, or up or down only. Returns whether it moved.
fn swim_towards(mob: &mut ServerMob, chunks: &ChunkMap, direction: Vec3, distance: f32) -> bool {
    let step = direction * distance;
    for step in [step, step.with_y(0.0), Vec3::Y * step.y] {
        if step.length_squared() < 1e-8 {
            continue;
        }
        let moved = mob.position + step;
        if in_water_at(mob, chunks, moved) && may_move_to(mob, chunks, moved) {
            mob.position = moved;
            if step.x != 0.0 || step.z != 0.0 {
                mob.rotation = Quat::from_rotation_y(atan2(step.x, step.z));
            }
            return true;
        }
    }
    false
}

/// A fish out of the water hops, towards the nearest water within `WATER_SEARCH` if there is
/// some, anywhere else.
fn flop(mob: &mut ServerMob, chunks: &ChunkMap, rng: &mut impl Rng) {
    mob.action = MobAction::Idle;
    if !mob.on_ground || !rng.gen_ratio(1, 12) {
        return;
    }
    let feet = mob.position.floor().as_ivec3();
    let r = WATER_SEARCH;
    let water = (-r..=r)
        .flat_map(|x| (-2..=1).flat_map(move |y| (-r..=r).map(move |z| IVec3::new(x, y, z))))
        .filter(|offset| {
            chunks
                .get_block(feet + *offset)
                .is_some_and(|block| block.id.is_watery())
        })
        .min_by_key(|offset| offset.length_squared());
    let towards = match water {
        Some(offset) => offset.as_vec3().with_y(0.0).normalize_or_zero(),
        None => {
            Vec3::new(rng.gen_range(-1.0..1.0), 0.0, rng.gen_range(-1.0..1.0)).normalize_or_zero()
        }
    };
    jump(mob);
    mob.velocity.y *= 0.6;
    mob.velocity.x = towards.x * 2.0;
    mob.velocity.z = towards.z * 2.0;
}
