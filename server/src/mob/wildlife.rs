//! The wild animals come back, slowly, to the land the players hunted empty: every
//! `RETURN_INTERVAL_TICKS`, each player gets a chance of a herd of the biome
//! (`WorldGenerator::wild_herds_at`) appearing on the grass of a spot of the ring around it, far
//! from every player and every bedroll, unless the animals there already reach `ZONE_CAP`.

use std::collections::HashMap;
use std::f32::consts::TAU;
use std::ops::Range;

use bevy::prelude::*;
use rand::Rng;
use shared::{
    messages::PlayerId,
    players::Player,
    world::{ticks::sees_sky, BlockId, ChunkMap, Herd, MobId, ServerMob},
};

use crate::init::ServerTime;
use crate::world::{data::ServerWorldMap, simulation::LoadedGenerator};

/// Ticks between two returns of each player: a minute.
pub const RETURN_INTERVAL_TICKS: u64 = 60 * shared::TICKS_PER_SECOND;
/// The chance a player gets a herd back at each return.
const RETURN_CHANCE: f64 = 0.5;
/// How far from the attempting player, horizontally, a herd may come back, in blocks.
pub const RETURN_RING: Range<f32> = 48.0..80.0;
/// No herd comes back closer than this to a player, horizontally, in blocks.
pub const AWAY_FROM_PLAYERS: f32 = 40.0;
/// Nor this close to a bedroll: the land around a base stays as its players keep it.
pub const AWAY_FROM_BEDROLLS: f32 = 64.0;
/// The animals within `ZONE_RADIUS` of a spot past which none comes back there.
pub const ZONE_CAP: usize = 6;
pub const ZONE_RADIUS: f32 = 48.0;
/// How far above and below the player the ground of a spot is looked for, in cells.
const GROUND_SCAN: i32 = 24;

/// Whether a herd may come back at `spot`: far from every player and every bedroll, and the
/// animals around fewer than `ZONE_CAP`.
pub fn may_return_at(
    spot: Vec3,
    players: impl IntoIterator<Item = Vec3>,
    bedrolls: impl IntoIterator<Item = Vec3>,
    animals: impl IntoIterator<Item = Vec3>,
) -> bool {
    let flat = |at: Vec3| at.with_y(0.0);
    players
        .into_iter()
        .all(|player| flat(player).distance(flat(spot)) >= AWAY_FROM_PLAYERS)
        && bedrolls
            .into_iter()
            .all(|bed| flat(bed).distance(flat(spot)) >= AWAY_FROM_BEDROLLS)
        && animals
            .into_iter()
            .filter(|animal| animal.distance(spot) < ZONE_RADIUS)
            .count()
            < ZONE_CAP
}

/// The cell of the grass, or snow, under the open sky that an animal would stand on at
/// `(x, z)`, scanning from `top` down: none where the first block met is anything else.
fn ground(chunks: &ChunkMap, x: i32, top: i32, z: i32) -> Option<IVec3> {
    for y in (top - 2 * GROUND_SCAN..=top).rev() {
        let cell = IVec3::new(x, y, z);
        let Some(block) = chunks.get_block(cell) else {
            continue;
        };
        let free = |up: i32| chunks.get_block(cell + IVec3::Y * up).is_none();
        return (matches!(block.id, BlockId::Grass | BlockId::Snow)
            && free(1)
            && free(2)
            && sees_sky(chunks, cell + IVec3::Y))
        .then_some(cell);
    }
    None
}

/// The herd of `herds` a roll picks, each after the previous one on the same roll, as at the
/// generation.
fn pick_herd(herds: &[Herd], rng: &mut impl Rng) -> Option<Herd> {
    let roll: f32 = rng.gen();
    let mut chance = 0.0;
    herds.iter().copied().find(|herd| {
        chance += herd.chance;
        roll < chance
    })
}

pub fn wildlife_system(
    mut world_map: ResMut<ServerWorldMap>,
    time: Res<ServerTime>,
    generator: Option<Res<LoadedGenerator>>,
) {
    if !time.0.is_multiple_of(RETURN_INTERVAL_TICKS) {
        return;
    }
    let Some(generator) = generator else {
        return;
    };
    let map = world_map.as_mut();
    let mut rng = rand::thread_rng();
    let mut ids: Vec<PlayerId> = map.players.keys().copied().collect();
    ids.sort_unstable();
    for id in ids {
        if !rng.gen_bool(RETURN_CHANCE) {
            continue;
        }
        let Some(player) = map.players.get(&id) else {
            continue;
        };
        let herd = herd_back(
            &map.chunks,
            &map.players,
            player,
            map,
            &generator.0,
            &mut rng,
        );
        for animal in herd {
            super::add_mob(&mut map.mobs, &mut map.changed_mobs, animal);
        }
    }
}

/// The herd that comes back around `player`, if one does.
fn herd_back(
    chunks: &ChunkMap,
    players: &HashMap<PlayerId, Player>,
    player: &Player,
    map: &ServerWorldMap,
    generator: &crate::world::generation::WorldGenerator,
    rng: &mut impl Rng,
) -> Vec<ServerMob> {
    let angle = rng.gen_range(0.0..TAU);
    let distance = rng.gen_range(RETURN_RING);
    let x = (player.position.x + angle.cos() * distance).floor() as i32;
    let z = (player.position.z + angle.sin() * distance).floor() as i32;
    let top = player.position.y as i32 + GROUND_SCAN;
    let Some(floor) = ground(chunks, x, top, z) else {
        return Vec::new();
    };
    let spot = floor.as_vec3() + Vec3::new(0.5, 1.0, 0.5);
    let animals = |mobs: &HashMap<MobId, ServerMob>| -> Vec<Vec3> {
        mobs.values()
            .filter(|mob| mob.kind.is_animal())
            .map(|mob| mob.position)
            .collect()
    };
    let bedrolls: Vec<Vec3> = map
        .places
        .bedrolls
        .values()
        .map(|bed| bed.as_vec3())
        .collect();
    let around = animals(&map.mobs);
    if !may_return_at(
        spot,
        players.values().map(|p| p.position),
        bedrolls,
        around.iter().copied(),
    ) {
        return Vec::new();
    }
    let Some(herd) = pick_herd(&generator.wild_herds_at(x, z), rng) else {
        return Vec::new();
    };
    let near = around
        .iter()
        .filter(|a| a.distance(spot) < ZONE_RADIUS)
        .count();
    let size = rng
        .gen_range(herd.fewest..=herd.most.max(herd.fewest))
        .min((ZONE_CAP - near) as u64);
    let mut herd_mobs = Vec::new();
    for _ in 0..size {
        let (dx, dz) = (rng.gen_range(-2..=2), rng.gen_range(-2..=2));
        let Some(cell) = ground(chunks, x + dx, floor.y + 3, z + dz) else {
            continue;
        };
        let position = ServerMob::standing_at(
            herd.kind,
            cell.x as f32 + 0.5,
            cell.y as f32 + 1.0,
            cell.z as f32 + 0.5,
        );
        herd_mobs.push(ServerMob {
            rotation: Quat::from_rotation_y(rng.gen_range(0.0..TAU)),
            action_ticks: rng.gen_range(1..200),
            ..ServerMob::new(herd.kind, position)
        });
    }
    herd_mobs
}

#[cfg(test)]
mod tests {
    use super::*;

    const SPOT: Vec3 = Vec3::new(0.0, 70.0, 0.0);

    #[test]
    fn a_herd_comes_back_far_from_the_players_and_the_bedrolls() {
        let far = [Vec3::new(60.0, 70.0, 0.0)];
        assert!(may_return_at(SPOT, far, [], []));
        assert!(!may_return_at(SPOT, [Vec3::new(20.0, 70.0, 0.0)], [], []));
        // A player high above still counts: the distance is horizontal
        assert!(!may_return_at(SPOT, [Vec3::new(0.0, 200.0, 10.0)], [], []));
        assert!(!may_return_at(SPOT, far, [Vec3::new(50.0, 70.0, 0.0)], []));
        assert!(may_return_at(SPOT, far, [Vec3::new(90.0, 70.0, 0.0)], []));
    }

    #[test]
    fn no_herd_comes_back_where_the_animals_reach_the_cap() {
        let far = [Vec3::new(60.0, 70.0, 0.0)];
        let few = vec![Vec3::new(5.0, 70.0, 5.0); ZONE_CAP - 1];
        assert!(may_return_at(SPOT, far, [], few.clone()));
        let many = vec![Vec3::new(5.0, 70.0, 5.0); ZONE_CAP];
        assert!(!may_return_at(SPOT, far, [], many));
        // Those far away do not count
        let elsewhere = vec![Vec3::new(500.0, 70.0, 0.0); 50];
        assert!(may_return_at(SPOT, far, [], elsewhere));
    }
}
