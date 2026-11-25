//! The hostiles of the dungeons appear at the places their plans hold, while a player is near,
//! within the same caps as the others (`spawning`). They need no darkness, but a torch, a lantern
//! or a lit campfire by a place keeps them from it, and so does anything that fills it.

use std::collections::{BTreeMap, HashMap};

use bevy::math::{IVec2, IVec3, Vec3};
use bevy_ecs::system::{Res, ResMut};
use rand::Rng;
use shared::messages::PlayerId;
use shared::players::Player;
use shared::world::depths::{strength, strengthened, DepthTier};
use shared::world::hostiles::{line_of_sight, may_spawn_at};
use shared::world::{BlockId, ChunkMap, MobKind, ServerMob};

use super::spawning::{player_cap, PLAYER_CAP_DISTANCE, SPAWN_INTERVAL_TICKS, WORLD_CAP};
use crate::init::ServerTime;
use crate::world::data::ServerWorldMap;
use crate::world::generation::dungeon::{region_of, DungeonKind, DungeonPlan};
use crate::world::simulation::LoadedGenerator;

/// A dungeon wakes while a player is this close to it, in blocks.
const WAKE_DISTANCE: f32 = 36.0;
/// No guard appears closer than this to a player, nor in its sight.
const GUARD_DISTANCE: f32 = 8.0;
/// The odds of a guard at each spawn interval, times the strength of its depth tier.
const GUARD_ODDS: (u32, u32) = (1, 4);
/// Most hostiles in a dungeon, its guards or not, times the strength of its depth tier, and how far around it they count.
const MOST_GUARDS: usize = 5;
const GUARD_MARGIN: f32 = 6.0;
/// A light this close to a place keeps the guards from it, in cells along each axis.
const LIGHT_REACH: i32 = 3;

/// Every `SPAWN_INTERVAL_TICKS`, unless the world is peaceful, each dungeon near a player may
/// get a guard at one of its places.
pub fn dungeon_guards_system(
    mut world_map: ResMut<ServerWorldMap>,
    time: Res<ServerTime>,
    generator: Option<Res<LoadedGenerator>>,
) {
    let Some(generator) = generator else {
        return;
    };
    if !time.0.is_multiple_of(SPAWN_INTERVAL_TICKS) || !world_map.difficulty.has_hostiles() {
        return;
    }
    let mut near = BTreeMap::new();
    for player in world_map.players.values() {
        let reach = IVec2::splat(WAKE_DISTANCE as i32);
        let at = player.position.floor().as_ivec3();
        let first = region_of(at.x - reach.x, at.z - reach.y);
        let last = region_of(at.x + reach.x, at.z + reach.y);
        for x in first.x..=last.x {
            for z in first.y..=last.y {
                for (i, kind) in DungeonKind::ALL.into_iter().enumerate() {
                    let Some(plan) = generator.0.dungeon(kind, IVec2::new(x, z)) else {
                        continue;
                    };
                    if distance_to(&plan, player.position) <= WAKE_DISTANCE {
                        near.insert((x, z, i), plan);
                    }
                }
            }
        }
    }
    let mut rng = rand::thread_rng();
    for plan in near.into_values() {
        let place = plan.guards[rng.gen_range(0..plan.guards.len())];
        let tier = DepthTier::of(&world_map.chunks, place.0);
        if !rng.gen_ratio(GUARD_ODDS.0 * strength(tier), GUARD_ODDS.1 * 100) {
            continue;
        }
        let ServerWorldMap {
            mobs,
            chunks,
            players,
            changed_mobs,
            ..
        } = &mut *world_map;
        let hostiles: Vec<Vec3> = mobs
            .values()
            .filter(|mob| mob.kind.is_hostile())
            .map(|mob| mob.position)
            .collect();
        if let Some(mob) = guard(&plan, place, chunks, players, &hostiles) {
            super::add_mob(mobs, changed_mobs, mob);
        }
    }
}

/// How far `position` is from the cells of `plan`.
fn distance_to(plan: &DungeonPlan, position: Vec3) -> f32 {
    let (min, max) = (plan.min.as_vec3(), plan.max.as_vec3() + 1.0);
    position.distance(position.clamp(min, max))
}

/// The guard that appears at `place` of `plan`, if it may: the caps leave it room, the place is
/// still free and unlit, and no player is close to it or sees it.
fn guard(
    plan: &DungeonPlan,
    (feet, kind): (IVec3, MobKind),
    chunks: &ChunkMap,
    players: &HashMap<PlayerId, Player>,
    hostiles: &[Vec3],
) -> Option<ServerMob> {
    let in_dungeon = hostiles
        .iter()
        .filter(|at| distance_to(plan, **at) <= GUARD_MARGIN)
        .count();
    let tier = DepthTier::of(chunks, feet);
    let most = strengthened(MOST_GUARDS.min(plan.guards.len()) as u32, tier) as usize;
    if in_dungeon >= most || hostiles.len() >= WORLD_CAP {
        return None;
    }
    may_spawn_at(chunks, feet)?;
    if chunks.get_block(feet).is_some() || lit(chunks, feet) {
        return None;
    }
    let centre = feet.as_vec3() + Vec3::new(0.5, 1.0, 0.5);
    for player in players.values() {
        let distance = player.position.distance(centre);
        let around = hostiles
            .iter()
            .filter(|at| at.distance(player.position) <= PLAYER_CAP_DISTANCE)
            .count();
        if distance < GUARD_DISTANCE
            || (distance <= PLAYER_CAP_DISTANCE
                && (around >= player_cap(tier) || line_of_sight(chunks, player.position, centre)))
        {
            return None;
        }
    }
    let at = ServerMob::standing_at(kind, centre.x, feet.y as f32, centre.z);
    Some(ServerMob::new(kind, at).in_tier(tier))
}

fn lit(chunks: &ChunkMap, feet: IVec3) -> bool {
    let reach = -LIGHT_REACH..=LIGHT_REACH;
    reach.clone().any(|x| {
        reach.clone().any(|y| {
            reach.clone().any(|z| {
                chunks
                    .get_block(feet + IVec3::new(x, y, z))
                    .is_some_and(|block| {
                        matches!(
                            block.id,
                            BlockId::Torch
                                | BlockId::Lantern
                                | BlockId::HangingLantern
                                | BlockId::Campfire
                        )
                    })
            })
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::generation::WorldGenerator;
    use bevy::prelude::Transform;
    use shared::world::{BlockData, BlockDirection};
    use shared::CHUNK_SIZE;

    #[test]
    fn a_crypt_gets_guards_out_of_sight_of_a_player_and_away_from_the_lights() {
        let generator = WorldGenerator::new(42);
        let plan = (-6..6)
            .flat_map(|x| (-6..6).map(move |z| IVec2::new(x, z)))
            .find_map(|region| generator.dungeon(DungeonKind::Crypt, region))
            .unwrap();
        let mut chunks = ChunkMap::default();
        let side = IVec3::splat(CHUNK_SIZE);
        let (first, last) = (plan.min.div_euclid(side), plan.max.div_euclid(side));
        for x in first.x..=last.x {
            for y in first.y..=last.y {
                for z in first.z..=last.z {
                    let pos = IVec3::new(x, y, z);
                    chunks.insert_chunk(pos, generator.generate_chunk(pos));
                }
            }
        }
        let player_at = |at: Vec3| {
            HashMap::from([(1, Player::new(1, "p".to_string(), at, Transform::default()))])
        };
        let place = plan.guards[0];
        // in the rock, above
        let above = player_at(place.0.as_vec3() + Vec3::Y * 20.0);
        let mob = guard(&plan, place, &chunks, &above, &[]).unwrap();
        assert_eq!(mob.kind, MobKind::Gnawer);
        // too close, and the caps
        let close = player_at(place.0.as_vec3() + Vec3::Y * 2.0);
        assert!(guard(&plan, place, &chunks, &close, &[]).is_none());
        let full = [mob.position; MOST_GUARDS];
        assert!(guard(&plan, place, &chunks, &above, &full).is_none());
        chunks.set_block(
            place.0 + IVec3::Y,
            BlockData::new(BlockId::Torch, BlockDirection::Front),
        );
        assert!(guard(&plan, place, &chunks, &above, &[]).is_none());
    }
}
