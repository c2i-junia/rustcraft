//! The Cinderlings' fire, once per fixed step after the mobs move: their embers fly, hit the
//! players, set them burning and light what they land on; water and rain put the Cinderlings
//! out. An ember is a mob of its own kind (`MobKind::Ember`, a projectile): the other mob
//! systems leave it to this one, and the save leaves it out.

use std::collections::{HashMap, HashSet};

use bevy::{
    math::{bounding::Aabb3d, bounding::IntersectsVolume, IVec3, Vec3},
    time::{Fixed, Time},
};
use bevy_ecs::system::{Res, ResMut};
use shared::{
    messages::PlayerId,
    players::{game_mode::DamageSource, movement::is_terrain_loaded, Player},
    world::{
        depths::{strengthened, DepthTier},
        fire::flammability,
        hostiles::{is_prey, Difficulty},
        ticks::sees_sky,
        BlockData, BlockDirection, BlockHitbox, BlockId, BlockTransparency, ChunkMap, MobId,
        MobKind, ServerMob, WeatherKind,
    },
};

use crate::world::data::ServerWorldMap;
use crate::world::weather::ServerWeather;

/// The pull on an ember in flight, in blocks/s².
pub const EMBER_GRAVITY: f32 = -16.0;
/// How fast an ember goes across, in blocks/s: its flight lasts the horizontal distance to its
/// target over this, within `EMBER_FLIGHT`.
pub const EMBER_SPEED: f32 = 8.0;
/// The shortest and longest flight of an ember aimed at a target, in seconds.
const EMBER_FLIGHT: (f32, f32) = (0.25, 2.0);
/// How long an ember flies at most, in ticks: 5 s.
pub const EMBER_LIFE_TICKS: u32 = 100;
/// How long an ember sets a player burning at least, in ticks.
pub const EMBER_BURNING_TICKS: u16 = 60;
/// The longest move of an ember tested at once, in blocks.
const EMBER_STEP: f32 = 0.25;

/// An ember thrown from `from` in an arc that falls on `target` (`EMBER_GRAVITY`, flying
/// across at `EMBER_SPEED`).
pub fn ember(from: Vec3, target: Vec3) -> ServerMob {
    let across = (target - from).with_y(0.0);
    let t = (across.length() / EMBER_SPEED).clamp(EMBER_FLIGHT.0, EMBER_FLIGHT.1);
    let rise = (target.y - from.y - EMBER_GRAVITY * t * t / 2.0) / t;
    ServerMob {
        velocity: across / t + Vec3::Y * rise,
        action_ticks: EMBER_LIFE_TICKS,
        ..ServerMob::new(MobKind::Ember, from)
    }
}

/// Flies the embers, then puts out the Cinderlings in water or in the rain.
pub fn cinder_system(
    mut world_map: ResMut<ServerWorldMap>,
    weather: Res<ServerWeather>,
    time: Res<Time<Fixed>>,
) {
    let ServerWorldMap {
        mobs,
        chunks,
        players,
        changed_mobs,
        difficulty,
        ..
    } = world_map.as_mut();
    let delta = time.delta_secs();
    fly_embers(mobs, chunks, players, changed_mobs, *difficulty, delta);
    quench(mobs, chunks, weather.kind != WeatherKind::Clear);
}

/// One step of every ember: those that hit, landed or burnt out are removed.
fn fly_embers(
    mobs: &mut HashMap<MobId, ServerMob>,
    chunks: &mut ChunkMap,
    players: &mut HashMap<PlayerId, Player>,
    changed: &mut HashSet<MobId>,
    difficulty: Difficulty,
    delta: f32,
) {
    let mut gone = Vec::new();
    for (id, ember) in mobs.iter_mut() {
        if ember.kind != MobKind::Ember {
            continue;
        }
        if fly(ember, chunks, players, difficulty, delta) {
            changed.insert(*id);
        } else {
            gone.push(*id);
        }
    }
    for id in gone {
        // The clients are told by the next broadcast, which no longer finds it
        mobs.remove(&id);
        changed.remove(&id);
    }
}

/// One step of `ember` along its arc, by `EMBER_STEP` at most. It hits the first prey whose box
/// it touches (`hit`), and lands in the first cell with a hitbox or a liquid: on a flammable
/// block it lights the cell it came from, if that is air; a liquid puts it out. Returns whether
/// it flies on: not after a hit, a landing, its life, or over unloaded terrain.
fn fly(
    ember: &mut ServerMob,
    chunks: &mut ChunkMap,
    players: &mut HashMap<PlayerId, Player>,
    difficulty: Difficulty,
    delta: f32,
) -> bool {
    if ember.action_ticks == 0
        || !ember.position.is_finite()
        || !ember.velocity.is_finite()
        || !is_terrain_loaded(ember.position, chunks)
    {
        return false;
    }
    ember.action_ticks -= 1;
    let pull = Vec3::Y * EMBER_GRAVITY;
    let start = ember.position;
    let moved = ember.velocity * delta + pull * (delta * delta / 2.0);
    ember.velocity += pull * delta;
    let steps = (moved.length() / EMBER_STEP).ceil().max(1.0) as u32;
    let half = Vec3::new(ember.width, ember.height, ember.depth) / 2.0;
    for step in 1..=steps {
        let at = start + moved * (step as f32 / steps as f32);
        let body = Aabb3d::new(at, half);
        let struck = players.values_mut().find(|player| {
            let half = Vec3::new(player.width, player.height, player.width) / 2.0;
            is_prey(player) && body.intersects(&Aabb3d::new(player.position, half))
        });
        if let Some(player) = struck {
            hit(player, ember.tier, difficulty);
            return false;
        }
        if let Some(block) = chunks.get_block(at.floor().as_ivec3()).copied() {
            if block.id.get_visibility() == BlockTransparency::Liquid {
                return false;
            }
            if !matches!(block.get_hitbox(), BlockHitbox::None) {
                let before = ember.position.floor().as_ivec3();
                if flammability(block.id).is_some() && chunks.get_block(before).is_none() {
                    chunks.set_block(before, BlockData::new(BlockId::Fire, BlockDirection::Front));
                }
                return false;
            }
        }
        ember.position = at;
    }
    true
}

/// An ember hits `player`: the damage of a Cinderling of `tier`, and burning for `EMBER_BURNING_TICKS` at
/// least.
fn hit(player: &mut Player, tier: Option<DepthTier>, difficulty: Difficulty) {
    let damage = MobKind::Cinderling
        .strike()
        .map_or(1, |strike| strike.damage);
    let damage = difficulty.strike_damage(strengthened(damage, tier));
    player.hurt(damage, DamageSource::Mob(MobKind::Cinderling));
    player.burning_ticks = player.burning_ticks.max(EMBER_BURNING_TICKS);
}

/// Removes, without loot, the Cinderlings whose box touches water, and while it rains those
/// that see the sky (`ticks::sees_sky`), where the terrain is loaded.
fn quench(mobs: &mut HashMap<MobId, ServerMob>, chunks: &ChunkMap, raining: bool) {
    mobs.retain(|_, mob| {
        if mob.kind != MobKind::Cinderling || !is_terrain_loaded(mob.position, chunks) {
            return true;
        }
        let rained_on = raining && sees_sky(chunks, mob.position.floor().as_ivec3());
        !(rained_on || touches_water(chunks, mob))
    });
}

/// Whether a cell of water is in the box of `mob`.
fn touches_water(chunks: &ChunkMap, mob: &ServerMob) -> bool {
    let half = Vec3::new(mob.width, mob.height, mob.depth) / 2.0;
    let (low, high) = (
        (mob.position - half).floor().as_ivec3(),
        (mob.position + half).floor().as_ivec3(),
    );
    (low.x..=high.x).any(|x| {
        (low.y..=high.y).any(|y| {
            (low.z..=high.z).any(|z| {
                chunks
                    .get_block(IVec3::new(x, y, z))
                    .is_some_and(|block| block.id == BlockId::Water)
            })
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mob::behavior::mob_behavior_system;
    use bevy::ecs::system::RunSystemOnce;
    use bevy::prelude::{Transform, World};
    use shared::players::game_mode::{DeathCause, GameMode, MAX_HEALTH};
    use shared::TICKS_PER_SECOND;
    use std::time::Duration;

    /// A floor of `floor` with its top at y = 1 over x, z in -40..40.
    fn ground(floor: BlockId) -> ServerWorldMap {
        let mut map = ServerWorldMap {
            chunks: ChunkMap::with_loaded_region(IVec3::new(-40, -16, -40), IVec3::new(39, 31, 39)),
            ..Default::default()
        };
        map.chunks
            .fill_box(IVec3::new(-40, 0, -40), IVec3::new(39, 0, 39), floor);
        map
    }

    fn player_at(id: PlayerId, x: f32, z: f32) -> Player {
        let mut player = Player::new(id, format!("p{id}"), Vec3::ZERO, Transform::default());
        player.position = Vec3::new(x, 1.0 + player.height / 2.0, z);
        player
    }

    fn world(map: ServerWorldMap, weather: WeatherKind) -> World {
        let mut world = World::new();
        world.insert_resource(map);
        let mut time = Time::<Fixed>::default();
        time.advance_by(Duration::from_secs_f64(1.0 / TICKS_PER_SECOND as f64));
        world.insert_resource(time);
        let mut server_weather = ServerWeather::new(&mut rand::thread_rng());
        server_weather.kind = weather;
        world.insert_resource(server_weather);
        world
    }

    fn run(world: &mut World, ticks: u32) {
        for _ in 0..ticks {
            world.run_system_once(cinder_system).unwrap();
        }
    }

    fn state(world: &World) -> &ServerWorldMap {
        world.resource::<ServerWorldMap>()
    }

    #[test]
    fn an_ember_thrown_at_a_player_hits_it_for_one_and_sets_it_burning() {
        let mut map = ground(BlockId::Stone);
        let target = player_at(1, 6.5, 0.5);
        let from = Vec3::new(0.5, 4.0, 0.5);
        map.mobs.insert(9, ember(from, target.position));
        map.players.insert(1, target);
        let mut w = world(map, WeatherKind::Clear);
        // Its flight to the centre lasts 6 / 8 of a second, 15 ticks: the box is met before
        run(&mut w, 10);
        assert!(state(&w).mobs.contains_key(&9));
        run(&mut w, 6);
        assert!(state(&w).mobs.is_empty());
        let player = &state(&w).players[&1];
        assert_eq!(player.health, MAX_HEALTH - 1);
        assert_eq!(player.burning_ticks, EMBER_BURNING_TICKS);
        // Killed by one, a Cinderling got it
        let mut map = ground(BlockId::Stone);
        let mut target = player_at(1, 6.5, 0.5);
        target.health = 1;
        map.mobs.insert(9, ember(from, target.position));
        map.players.insert(1, target);
        let mut w = world(map, WeatherKind::Clear);
        run(&mut w, 20);
        assert_eq!(
            state(&w).players[&1].death_cause,
            Some(DeathCause::Mob(MobKind::Cinderling))
        );
    }

    #[test]
    fn an_ember_passes_a_creative_player_by() {
        let mut map = ground(BlockId::Stone);
        let mut target = player_at(1, 6.5, 0.5);
        target.game_mode = GameMode::Creative;
        map.mobs
            .insert(9, ember(Vec3::new(0.5, 4.0, 0.5), target.position));
        map.players.insert(1, target);
        let mut w = world(map, WeatherKind::Clear);
        run(&mut w, 40);
        assert!(state(&w).mobs.is_empty());
        assert_eq!(state(&w).players[&1].health, MAX_HEALTH);
        assert_eq!(state(&w).players[&1].burning_ticks, 0);
    }

    #[test]
    fn an_ember_landing_on_a_flammable_block_lights_the_cell_above_it_and_on_stone_nothing() {
        for (floor, fire) in [(BlockId::OakPlanks, true), (BlockId::Stone, false)] {
            let mut map = ground(floor);
            map.mobs
                .insert(9, ember(Vec3::new(0.5, 4.0, 0.5), Vec3::new(5.5, 1.0, 0.5)));
            let mut w = world(map, WeatherKind::Clear);
            run(&mut w, 30);
            assert!(state(&w).mobs.is_empty(), "{floor:?}");
            let lit = state(&w)
                .chunks
                .get_block(IVec3::new(5, 1, 0))
                .is_some_and(|block| block.id == BlockId::Fire);
            assert_eq!(lit, fire, "{floor:?}");
            assert_eq!(
                state(&w)
                    .chunks
                    .get_block(IVec3::new(5, 0, 0))
                    .map(|b| b.id),
                Some(floor)
            );
        }
    }

    #[test]
    fn an_ember_goes_out_in_water_and_at_the_end_of_its_life() {
        let mut map = ground(BlockId::Stone);
        map.chunks
            .fill_box(IVec3::new(3, 1, -2), IVec3::new(8, 1, 2), BlockId::Water);
        map.mobs
            .insert(9, ember(Vec3::new(0.5, 4.0, 0.5), Vec3::new(5.5, 1.5, 0.5)));
        let mut w = world(map, WeatherKind::Clear);
        run(&mut w, 30);
        assert!(state(&w).mobs.is_empty());
        assert!(state(&w).chunks.get_block(IVec3::new(5, 2, 0)).is_none());
        // Its life over, in the air
        let mut map = ground(BlockId::Stone);
        let mut old = ember(Vec3::new(0.5, 20.0, 0.5), Vec3::new(0.5, 20.0, 0.5));
        old.action_ticks = 2;
        map.mobs.insert(9, old);
        let mut w = world(map, WeatherKind::Clear);
        run(&mut w, 2);
        assert!(state(&w).mobs.contains_key(&9));
        run(&mut w, 1);
        assert!(state(&w).mobs.is_empty());
    }

    #[test]
    fn the_mobs_systems_leave_an_ember_alone() {
        let mut map = ground(BlockId::Stone);
        let flying = ember(Vec3::new(0.5, 4.0, 0.5), Vec3::new(6.5, 1.0, 0.5));
        map.mobs.insert(9, flying.clone());
        let mut w = world(map, WeatherKind::Clear);
        w.run_system_once(mob_behavior_system).unwrap();
        let after = &state(&w).mobs[&9];
        assert_eq!(after.position, flying.position);
        assert_eq!(after.velocity, flying.velocity);
    }

    #[test]
    fn a_cinderling_goes_out_in_water_and_in_the_rain_under_the_sky_but_not_under_a_roof() {
        let cinderling = |x: f32| ServerMob::new(MobKind::Cinderling, Vec3::new(x, 4.0, 0.5));
        let mut map = ground(BlockId::Stone);
        // One in a pool, one in the open, one under a roof
        map.chunks
            .fill_box(IVec3::new(-10, 1, -2), IVec3::new(-8, 3, 2), BlockId::Water);
        map.chunks
            .fill_box(IVec3::new(8, 8, -2), IVec3::new(12, 8, 2), BlockId::Stone);
        map.mobs.insert(1, cinderling(-8.5));
        map.mobs.insert(2, cinderling(0.5));
        map.mobs.insert(3, cinderling(10.5));
        let mut w = world(map, WeatherKind::Clear);
        run(&mut w, 1);
        let mut left: Vec<_> = state(&w).mobs.keys().copied().collect();
        left.sort();
        assert_eq!(left, [2, 3], "the water puts it out, without loot");
        w.resource_mut::<ServerWeather>().kind = WeatherKind::Rain;
        run(&mut w, 1);
        let left: Vec<_> = state(&w).mobs.keys().copied().collect();
        assert_eq!(left, [3], "the rain too, under the sky");
    }
}
