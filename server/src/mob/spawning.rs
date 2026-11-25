//! Hostile mobs appear in the dark around the players and go once no player is near; a
//! peaceful world has none. Every `SPAWN_INTERVAL_TICKS`, each player gets one attempt at a
//! random spot of the ring around it, which costs a bounded scan of the world
//! (`find_floor` or `find_water`, `ticks::sees_sky`, `hostiles::surroundings`).

use std::collections::{HashMap, HashSet};
use std::f32::consts::TAU;
use std::ops::Range;

use bevy::math::{IVec3, Vec3};
use bevy_ecs::system::{Res, ResMut};
use rand::Rng;
use shared::{
    messages::PlayerId,
    players::Player,
    world::{
        hostiles::{
            is_dark, is_night, may_spawn_at, pack_size, spawn_kind, surroundings, SpawnPlace,
        },
        ticks::sees_sky,
        BlockId, ChunkMap, MobAction, MobId, MobKind, MobTarget, ServerMob,
    },
};

use super::hostile::BURROW_TICKS;

use crate::init::ServerTime;
use crate::world::data::ServerWorldMap;

/// Ticks between two spawn attempts of each player, and two looks for hostiles to despawn.
pub const SPAWN_INTERVAL_TICKS: u64 = 40;
/// How far from the attempting player, horizontally, a hostile may appear, in blocks. It never
/// appears closer than the start of the ring to any player.
pub const SPAWN_RING: Range<f32> = 24.0..64.0;
/// How far above or below the feet of the player an attempt starts, in blocks.
const SPAWN_HEIGHT: i32 = 24;
/// How many cells an attempt scans down from where it starts for a floor.
const FLOOR_SCAN: i32 = 24;
/// Most hostiles within `PLAYER_CAP_DISTANCE` of a player for it to get one more.
pub const PLAYER_CAP: usize = 10;
pub const PLAYER_CAP_DISTANCE: f32 = 64.0;
/// Most hostiles in the world.
pub const WORLD_CAP: usize = 40;
/// A hostile this far from every player goes, in blocks.
pub const DESPAWN_DISTANCE: f32 = 96.0;

/// Removes every hostile of a peaceful world, and the embers they threw, at once; otherwise sends the Stumplings into the
/// ground by day (`burrow_at_dawn`) and, every `SPAWN_INTERVAL_TICKS`, despawns the hostiles
/// far from every player and gives each player a spawn attempt.
pub fn hostile_spawning_system(mut world_map: ResMut<ServerWorldMap>, time: Res<ServerTime>) {
    spawning_tick(&mut world_map, time.0, &mut rand::thread_rng());
}

/// What `hostile_spawning_system` does at `tick`, drawing from `rng`.
fn spawning_tick(world_map: &mut ServerWorldMap, tick: u64, rng: &mut impl Rng) {
    let ServerWorldMap {
        mobs,
        chunks,
        players,
        changed_mobs,
        difficulty,
        ..
    } = world_map;
    if !difficulty.has_hostiles() {
        mobs.retain(|_, mob| !mob.kind.is_hostile() && mob.kind != MobKind::Ember);
        return;
    }
    burrow_at_dawn(mobs, changed_mobs, is_night(tick));
    if !tick.is_multiple_of(SPAWN_INTERVAL_TICKS) {
        return;
    }
    despawn_far_hostiles(mobs, players);
    let night = is_night(tick);
    let mut ids: Vec<PlayerId> = players.keys().copied().collect();
    // In a fixed order, so that a seeded `rng` gives the same world
    ids.sort_unstable();
    for id in ids {
        for mob in spawn_attempt(chunks, players, mobs, id, night, rng) {
            super::add_mob(mobs, changed_mobs, mob);
        }
    }
}

/// Removes the Stumplings that burrowed all the way (`MobAction::Burrow` with no ticks left),
/// and, by day, starts the others burrowing (`hostile::BURROW_TICKS`).
fn burrow_at_dawn(mobs: &mut HashMap<MobId, ServerMob>, changed: &mut HashSet<MobId>, night: bool) {
    mobs.retain(|_, mob| !(mob.action == MobAction::Burrow && mob.action_ticks == 0));
    if night {
        return;
    }
    for (id, mob) in mobs.iter_mut() {
        if mob.kind == MobKind::Stumpling && mob.action != MobAction::Burrow {
            mob.action = MobAction::Burrow;
            mob.action_ticks = BURROW_TICKS;
            mob.target = MobTarget::None;
            changed.insert(*id);
        }
    }
}

/// Removes the hostiles farther than `DESPAWN_DISTANCE` from every player. The clients are
/// told by the next broadcast, which no longer finds them.
fn despawn_far_hostiles(mobs: &mut HashMap<MobId, ServerMob>, players: &HashMap<PlayerId, Player>) {
    mobs.retain(|_, mob| {
        !mob.kind.is_hostile()
            || players
                .values()
                .any(|player| player.position.distance(mob.position) <= DESPAWN_DISTANCE)
    });
}

/// The cell of the feet of a mob standing on the first floor (`may_spawn_at`) found scanning
/// down `FLOOR_SCAN` cells from `start`, if any.
fn find_floor(chunks: &ChunkMap, start: IVec3) -> Option<(IVec3, BlockId)> {
    (0..FLOOR_SCAN)
        .map(|down| start - IVec3::Y * down)
        .find_map(|feet| may_spawn_at(chunks, feet).map(|floor| (feet, floor)))
}

/// The deepest water an attempt measures, in cells.
const WATER_SCAN: u32 = 16;

/// The top cell of the water met first scanning down `FLOOR_SCAN` cells from `start`, and how
/// deep it is (each at most `WATER_SCAN` cells away): none when the scan meets a block first, or
/// nothing.
fn find_water(chunks: &ChunkMap, start: IVec3) -> Option<(IVec3, u32)> {
    let met = (0..FLOOR_SCAN)
        .map(|down| start - IVec3::Y * down)
        .find_map(|cell| chunks.get_block(cell).map(|block| (cell, block.id)))
        .and_then(|(cell, id)| (id == BlockId::Water).then_some(cell))?;
    let is_water = |cell: IVec3| {
        chunks
            .get_block(cell)
            .is_some_and(|block| block.id == BlockId::Water)
    };
    // A scan that starts in the water climbs to its top
    let above = (1..WATER_SCAN as i32)
        .take_while(|&up| is_water(met + IVec3::Y * up))
        .count() as i32;
    let top = met + IVec3::Y * above;
    let depth = (0..WATER_SCAN as i32)
        .take_while(|&down| is_water(top - IVec3::Y * down))
        .count() as u32;
    Some((top, depth))
}

/// Whether a hostile at `centre` would be too close to a player: within the start of
/// `SPAWN_RING`, horizontally.
fn near_a_player(players: &HashMap<PlayerId, Player>, centre: Vec3) -> bool {
    players
        .values()
        .any(|player| player.position.with_y(0.0).distance(centre.with_y(0.0)) < SPAWN_RING.start)
}

/// The hostile that appears in the water whose top cell is `top`, `depth` deep, if its place
/// has a kind (a Drowner, `spawn_kind`) and it is far enough from every player: in the middle
/// of the column. Water needs no darkness.
fn spawn_in_water(
    players: &HashMap<PlayerId, Player>,
    (top, depth): (IVec3, u32),
    night: bool,
    rng: &mut impl Rng,
) -> Vec<ServerMob> {
    let centre = Vec3::new(
        top.x as f32 + 0.5,
        top.y as f32 + 1.0 - depth as f32 / 2.0,
        top.z as f32 + 0.5,
    );
    if near_a_player(players, centre) {
        return Vec::new();
    }
    let place = SpawnPlace {
        y: top.y,
        night,
        covered: false,
        floor: BlockId::Water,
        wooded: false,
        water_depth: depth,
    };
    spawn_kind(&place, rng)
        .map(|kind| ServerMob::new(kind, centre))
        .into_iter()
        .collect()
}

/// The hostiles of one attempt for the player `id`: a spot at a random angle and distance in
/// `SPAWN_RING`, at a random height near its feet, scanned down for water (`spawn_in_water`)
/// or a floor; if the spot is far
/// enough from every player, dark, and its place has a kind (`spawn_kind`), a pack of that kind
/// standing there, cut to the caps left, and to the members within `PLAYER_CAP_DISTANCE`.
fn spawn_attempt(
    chunks: &ChunkMap,
    players: &HashMap<PlayerId, Player>,
    mobs: &HashMap<MobId, ServerMob>,
    id: PlayerId,
    night: bool,
    rng: &mut impl Rng,
) -> Vec<ServerMob> {
    let Some(player) = players.get(&id) else {
        return Vec::new();
    };
    let hostiles = || mobs.values().filter(|mob| mob.kind.is_hostile());
    let in_world = hostiles().count();
    let near = hostiles()
        .filter(|mob| mob.position.distance(player.position) <= PLAYER_CAP_DISTANCE)
        .count();
    let room = WORLD_CAP
        .saturating_sub(in_world)
        .min(PLAYER_CAP.saturating_sub(near));
    if room == 0 {
        return Vec::new();
    }
    let angle = rng.gen_range(0.0..TAU);
    let distance = rng.gen_range(SPAWN_RING);
    let feet_y = (player.position.y - player.height / 2.0).floor() as i32;
    let start = IVec3::new(
        (player.position.x + angle.cos() * distance).floor() as i32,
        feet_y + rng.gen_range(-SPAWN_HEIGHT..=SPAWN_HEIGHT),
        (player.position.z + angle.sin() * distance).floor() as i32,
    );
    let mut pack = if let Some(water) = find_water(chunks, start) {
        spawn_in_water(players, water, night, rng)
    } else if let Some((feet, floor)) = find_floor(chunks, start) {
        spawn_at(chunks, players, feet, floor, night, room, rng)
    } else {
        Vec::new()
    };
    // A spot at the edge of the ring, or above or below the feet, may be past the cap's
    // distance: a hostile there would not count against the cap it spawned for
    pack.retain(|mob| mob.position.distance(player.position) <= PLAYER_CAP_DISTANCE);
    pack
}

/// The pack that appears with its feet at `feet`, on `floor`, if the spot is dark and far
/// enough from every player and its place has a kind; at most `room` mobs.
fn spawn_at(
    chunks: &ChunkMap,
    players: &HashMap<PlayerId, Player>,
    feet: IVec3,
    floor: BlockId,
    night: bool,
    room: usize,
    rng: &mut impl Rng,
) -> Vec<ServerMob> {
    let centre = feet.as_vec3() + Vec3::new(0.5, 0.0, 0.5);
    if near_a_player(players, centre) {
        return Vec::new();
    }
    let open_sky = sees_sky(chunks, feet);
    if !night && open_sky {
        return Vec::new();
    }
    let around = surroundings(chunks, feet);
    if !is_dark(night, open_sky, around.light) {
        return Vec::new();
    }
    let place = SpawnPlace {
        y: feet.y,
        night,
        covered: !open_sky,
        floor,
        wooded: around.leaves,
        water_depth: 0,
    };
    let Some(kind) = spawn_kind(&place, rng) else {
        return Vec::new();
    };
    let count = (pack_size(kind, rng) as usize).min(room);
    (0..count).map(|i| pack_member(kind, centre, i)).collect()
}

/// The `i`-th mob of a pack of `kind` around `centre`: side by side, the pushes part them.
fn pack_member(kind: MobKind, centre: Vec3, i: usize) -> ServerMob {
    let offset = Vec3::new(0.1 * i as f32, 0.0, -0.1 * i as f32);
    let at = centre + offset;
    ServerMob::new(kind, ServerMob::standing_at(kind, at.x, centre.y, at.z))
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;
    use bevy::prelude::{Transform, World};
    use rand::{rngs::StdRng, SeedableRng};
    use shared::world::hostiles::Difficulty;
    use shared::world::{BlockData, BlockDirection};
    use shared::{DAY_DURATION_IN_TICKS, MIDNIGHT_IN_DAY, NOON_IN_DAY};

    fn player_at(id: PlayerId, position: Vec3) -> Player {
        Player::new(id, format!("p{id}"), position, Transform::default())
    }

    /// A flat floor of `floor` with its top at y = 1 over x, z in -80..80, and a stone ceiling
    /// at y = 4 when `covered`.
    fn flat(floor: BlockId, covered: bool) -> ChunkMap {
        let mut chunks =
            ChunkMap::with_loaded_region(IVec3::new(-80, -16, -80), IVec3::new(79, 15, 79));
        chunks.fill_box(IVec3::new(-80, 0, -80), IVec3::new(79, 0, 79), floor);
        if covered {
            chunks.fill_box(
                IVec3::new(-80, 4, -80),
                IVec3::new(79, 4, 79),
                BlockId::Stone,
            );
        }
        chunks
    }

    fn rng() -> StdRng {
        StdRng::seed_from_u64(8)
    }

    #[test]
    fn a_dark_cave_floor_far_from_the_players_gets_a_pack_of_its_kind() {
        let chunks = flat(BlockId::Stone, true);
        let players = HashMap::from([(1, player_at(1, Vec3::new(0.5, 2.0, 0.5)))]);
        // y = 1 < 20, under a block: Gnawers, a Roller or a Cinderling, at day as at night
        let feet = IVec3::new(30, 1, 0);
        let mut kinds = HashSet::new();
        let mut rng = rng();
        for _ in 0..50 {
            let pack = spawn_at(&chunks, &players, feet, BlockId::Stone, false, 10, &mut rng);
            assert!(!pack.is_empty());
            let kind = pack[0].kind;
            assert!(pack.iter().all(|mob| mob.kind == kind));
            match kind {
                MobKind::Gnawer => assert!((2..=4).contains(&pack.len())),
                MobKind::Roller | MobKind::Cinderling => assert_eq!(pack.len(), 1),
                other => panic!("{other:?}"),
            }
            // standing on the floor
            for mob in &pack {
                assert!((mob.position.y - mob.height / 2.0 - 1.0).abs() < 1e-5);
            }
            kinds.insert(kind);
        }
        assert_eq!(kinds.len(), 3);
    }

    #[test]
    fn no_hostile_appears_in_the_light_by_day_or_near_a_player() {
        let players = HashMap::from([(1, player_at(1, Vec3::new(0.5, 2.0, 0.5)))]);
        let feet = IVec3::new(30, 1, 0);
        // A torch 8 blocks away
        let mut lit = flat(BlockId::Stone, true);
        lit.set_block(
            IVec3::new(38, 1, 0),
            BlockData::new(BlockId::Torch, BlockDirection::Front),
        );
        assert!(spawn_at(&lit, &players, feet, BlockId::Stone, true, 10, &mut rng()).is_empty());
        // Under the sky by day, even where the night would spawn one
        let open = flat(BlockId::Stone, false);
        let mut woods = flat(BlockId::Grass, false);
        woods.fill_box(
            IVec3::new(30, 6, 3),
            IVec3::new(30, 6, 3),
            BlockId::OakLeaves,
        );
        assert!(spawn_at(
            &woods,
            &players,
            feet,
            BlockId::Grass,
            false,
            10,
            &mut rng()
        )
        .is_empty());
        assert_eq!(
            spawn_at(&woods, &players, feet, BlockId::Grass, true, 10, &mut rng())[0].kind,
            MobKind::Bramble
        );
        // Open stone at night has no kind
        assert!(spawn_at(&open, &players, feet, BlockId::Stone, true, 10, &mut rng()).is_empty());
        // Within 24 blocks of a player, this one or another
        let cave = flat(BlockId::Stone, true);
        let near = IVec3::new(20, 1, 0);
        assert!(spawn_at(&cave, &players, near, BlockId::Stone, false, 10, &mut rng()).is_empty());
        let mut two = players.clone();
        two.insert(2, player_at(2, Vec3::new(40.5, 2.0, 0.5)));
        assert!(spawn_at(&cave, &two, feet, BlockId::Stone, false, 10, &mut rng()).is_empty());
    }

    #[test]
    fn a_pack_is_cut_to_the_room_left() {
        let chunks = flat(BlockId::Stone, true);
        let players = HashMap::from([(1, player_at(1, Vec3::new(0.5, 2.0, 0.5)))]);
        for _ in 0..20 {
            let pack = spawn_at(
                &chunks,
                &players,
                IVec3::new(30, 1, 0),
                BlockId::Stone,
                false,
                1,
                &mut rng(),
            );
            assert_eq!(pack.len(), 1);
        }
    }

    #[test]
    fn the_floor_is_found_scanning_down_a_bounded_number_of_cells() {
        let chunks = flat(BlockId::Stone, false);
        assert_eq!(
            find_floor(&chunks, IVec3::new(5, 10, 5)),
            Some((IVec3::new(5, 1, 5), BlockId::Stone))
        );
        assert_eq!(find_floor(&chunks, IVec3::new(5, 1 + FLOOR_SCAN, 5)), None);
    }

    fn hostile(kind: MobKind, position: Vec3) -> ServerMob {
        ServerMob::new(kind, position)
    }

    /// The ids of the mobs of `map`, of each kind.
    fn count(map: &ServerWorldMap, kind: MobKind) -> usize {
        map.mobs.values().filter(|mob| mob.kind == kind).count()
    }

    /// Runs the spawning system once at `tick` on `map`.
    fn run(map: ServerWorldMap, tick: u64) -> ServerWorldMap {
        let mut world = World::new();
        world.insert_resource(map);
        world.insert_resource(ServerTime(tick));
        world.run_system_once(hostile_spawning_system).unwrap();
        world.remove_resource::<ServerWorldMap>().unwrap()
    }

    /// Runs the spawning on `map` at each of `ticks`, drawing from one seeded rng, so that
    /// the attempts are the same on every run.
    fn run_seeded(mut map: ServerWorldMap, ticks: impl IntoIterator<Item = u64>) -> ServerWorldMap {
        let mut rng = rng();
        for tick in ticks {
            spawning_tick(&mut map, tick, &mut rng);
        }
        map
    }

    #[test]
    fn hostiles_far_from_every_player_despawn_and_animals_stay() {
        let mut map = ServerWorldMap::default();
        map.players.insert(1, player_at(1, Vec3::ZERO));
        map.mobs
            .insert(1, hostile(MobKind::Gnawer, Vec3::new(96.0, 0.0, 0.0)));
        map.mobs
            .insert(2, hostile(MobKind::Gnawer, Vec3::new(97.0, 0.0, 0.0)));
        map.mobs
            .insert(3, hostile(MobKind::Sheep, Vec3::new(500.0, 0.0, 0.0)));
        // Not on an attempt tick: nothing yet
        let map = run(map, SPAWN_INTERVAL_TICKS + 1);
        assert_eq!(map.mobs.len(), 3);
        let map = run(map, SPAWN_INTERVAL_TICKS * 2);
        let mut ids: Vec<_> = map.mobs.keys().copied().collect();
        ids.sort();
        assert_eq!(ids, [1, 3]);
    }

    #[test]
    fn a_peaceful_world_loses_its_hostiles_at_once_and_gets_none() {
        let mut map = ServerWorldMap {
            chunks: flat(BlockId::Stone, true),
            difficulty: Difficulty::Peaceful,
            ..Default::default()
        };
        map.players
            .insert(1, player_at(1, Vec3::new(0.5, 2.0, 0.5)));
        map.mobs
            .insert(1, hostile(MobKind::Roller, Vec3::new(3.0, 2.0, 0.0)));
        map.mobs
            .insert(2, hostile(MobKind::Pig, Vec3::new(3.0, 2.0, 0.0)));
        map.mobs
            .insert(3, hostile(MobKind::Ember, Vec3::new(3.0, 2.0, 0.0)));
        let mut map = run(map, 7);
        assert_eq!(map.mobs.keys().copied().collect::<Vec<_>>(), [2]);
        for i in 1..=50 {
            map = run(map, i * SPAWN_INTERVAL_TICKS);
        }
        assert_eq!(map.mobs.len(), 1);
    }

    #[test]
    fn players_in_a_dark_cave_get_hostiles_up_to_their_cap() {
        let mut map = ServerWorldMap {
            chunks: flat(BlockId::Stone, true),
            ..Default::default()
        };
        map.players
            .insert(1, player_at(1, Vec3::new(0.5, 1.9, 0.5)));
        // Embers in flight do not count
        for i in 0..PLAYER_CAP {
            map.mobs.insert(
                1000 + i as u128,
                hostile(MobKind::Ember, Vec3::new(30.0, 2.0, 0.0)),
            );
        }
        let map = run_seeded(
            map,
            (1..=400).map(|i| MIDNIGHT_IN_DAY + i * SPAWN_INTERVAL_TICKS),
        );
        let hostiles = map
            .mobs
            .values()
            .filter(|mob| mob.kind.is_hostile())
            .count();
        assert_eq!(hostiles, PLAYER_CAP);
        assert!(count(&map, MobKind::Gnawer) > 0);
        for mob in map.mobs.values().filter(|mob| mob.kind.is_hostile()) {
            let away = mob.position.with_y(0.0).distance(Vec3::new(0.5, 0.0, 0.5));
            assert!(
                (SPAWN_RING.start - 1.0..=SPAWN_RING.end + 1.0).contains(&away),
                "{away}"
            );
            // Each one counts against the cap
            assert!(mob.position.distance(Vec3::new(0.5, 1.9, 0.5)) <= PLAYER_CAP_DISTANCE);
        }
    }

    #[test]
    fn a_pack_past_the_cap_distance_is_not_spawned() {
        // A player 20 blocks above a cave: a spot at the far side of the ring, 64 blocks away
        // horizontally, is farther than 64 from it, and would spawn a pack that its cap does
        // not count. Every spot beyond is dropped.
        let mut map = ServerWorldMap {
            chunks: flat(BlockId::Stone, true),
            ..Default::default()
        };
        let player = Vec3::new(0.5, 21.9, 0.5);
        map.players.insert(1, player_at(1, player));
        let mut rng = rng();
        let mut spawned = 0;
        for _ in 0..1000 {
            let pack = spawn_attempt(&map.chunks, &map.players, &map.mobs, 1, true, &mut rng);
            for mob in pack {
                let away = mob.position.distance(player);
                assert!(away <= PLAYER_CAP_DISTANCE, "a hostile {away} away");
                spawned += 1;
            }
        }
        assert!(spawned > 0);
    }

    #[test]
    fn the_world_cap_holds_with_many_players() {
        let mut map = ServerWorldMap {
            chunks: flat(BlockId::Stone, true),
            ..Default::default()
        };
        // Four players, each with room in its own cap: the world's is what stops them. The
        // hostiles stand 66 to 76 blocks from the first player, out of its cap and within the
        // despawn distance of every player, so that none goes
        for i in 0..WORLD_CAP - 3 {
            map.mobs.insert(
                i as u128,
                hostile(MobKind::Gnawer, Vec3::new(66.0 + i as f32 * 0.25, 2.0, 0.5)),
            );
        }
        for (id, x, z) in [
            (1, 0.5, 0.5),
            (2, -10.5, 0.5),
            (3, 0.5, -10.5),
            (4, -10.5, -10.5),
        ] {
            map.players.insert(id, player_at(id, Vec3::new(x, 1.9, z)));
        }
        let map = run_seeded(
            map,
            (1..=50).map(|i| MIDNIGHT_IN_DAY + i * SPAWN_INTERVAL_TICKS),
        );
        assert_eq!(map.mobs.len(), WORLD_CAP);
    }

    #[test]
    fn an_open_plain_by_day_gets_no_hostile() {
        let mut map = ServerWorldMap {
            chunks: flat(BlockId::Grass, false),
            ..Default::default()
        };
        map.players
            .insert(1, player_at(1, Vec3::new(0.5, 1.9, 0.5)));
        for i in 1..=100 {
            map = run(
                map,
                DAY_DURATION_IN_TICKS + NOON_IN_DAY + i * SPAWN_INTERVAL_TICKS,
            );
        }
        assert!(map.mobs.is_empty());
    }

    #[test]
    fn at_dawn_the_stumplings_burrow_and_go_once_under_the_ground() {
        let mut map = ServerWorldMap {
            chunks: flat(BlockId::Grass, false),
            ..Default::default()
        };
        map.players
            .insert(1, player_at(1, Vec3::new(0.5, 1.9, 0.5)));
        map.mobs
            .insert(1, hostile(MobKind::Stumpling, Vec3::new(3.0, 2.0, 0.0)));
        map.mobs
            .insert(2, hostile(MobKind::Gnawer, Vec3::new(3.0, 2.0, 0.0)));
        // At night it stays as it is
        let map = run(map, MIDNIGHT_IN_DAY + 1);
        assert_eq!(map.mobs[&1].action, MobAction::Idle);
        // On the first tick of the day it starts to burrow, the Gnawer does not
        let mut map = run(map, DAY_DURATION_IN_TICKS / 2 + 1);
        assert_eq!(map.mobs[&1].action, MobAction::Burrow);
        assert_eq!(
            map.mobs[&1].action_ticks,
            super::super::hostile::BURROW_TICKS
        );
        assert!(map.changed_mobs.contains(&1));
        assert_eq!(map.mobs[&2].action, MobAction::Idle);
        // and keeps burrowing
        map.mobs.get_mut(&1).unwrap().action_ticks = 1;
        let mut map = run(map, DAY_DURATION_IN_TICKS / 2 + 2);
        assert_eq!(map.mobs[&1].action_ticks, 1);
        // Down, it goes, leaving nothing
        map.mobs.get_mut(&1).unwrap().action_ticks = 0;
        let map = run(map, DAY_DURATION_IN_TICKS / 2 + 3);
        assert!(!map.mobs.contains_key(&1));
        assert!(map.mobs.contains_key(&2));
    }

    #[test]
    fn an_open_plain_at_night_gets_stumplings() {
        let mut map = ServerWorldMap {
            chunks: flat(BlockId::Grass, false),
            ..Default::default()
        };
        map.players
            .insert(1, player_at(1, Vec3::new(0.5, 1.9, 0.5)));
        let map = run_seeded(map, (1..=200).map(|i| i * SPAWN_INTERVAL_TICKS));
        assert!(count(&map, MobKind::Stumpling) > 0);
        assert_eq!(count(&map, MobKind::Stumpling), map.mobs.len());
    }

    #[test]
    fn deep_water_gets_drowners_in_it_by_day_and_shallow_water_none() {
        let mut chunks = flat(BlockId::Stone, false);
        // 5 deep beyond x = 20, 2 deep below x = -20
        chunks.fill_box(
            IVec3::new(20, 1, -80),
            IVec3::new(79, 5, 79),
            BlockId::Water,
        );
        chunks.fill_box(
            IVec3::new(-80, 1, -80),
            IVec3::new(-20, 2, 79),
            BlockId::Water,
        );
        assert_eq!(
            find_water(&chunks, IVec3::new(30, 12, 0)),
            Some((IVec3::new(30, 5, 0), 5))
        );
        assert_eq!(
            find_water(&chunks, IVec3::new(-30, 12, 0)),
            Some((IVec3::new(-30, 2, 0), 2))
        );
        assert_eq!(find_water(&chunks, IVec3::new(0, 12, 0)), None);
        // From inside the water
        assert_eq!(
            find_water(&chunks, IVec3::new(30, 2, 0)),
            Some((IVec3::new(30, 5, 0), 5))
        );
        let mut map = ServerWorldMap {
            chunks,
            ..Default::default()
        };
        map.players
            .insert(1, player_at(1, Vec3::new(0.5, 1.9, 0.5)));
        let map = run_seeded(
            map,
            (1..=200).map(|i| DAY_DURATION_IN_TICKS + NOON_IN_DAY + i * SPAWN_INTERVAL_TICKS),
        );
        assert!(count(&map, MobKind::Drowner) > 0);
        assert_eq!(count(&map, MobKind::Drowner), map.mobs.len());
        for mob in map.mobs.values() {
            assert!(mob.position.x > 20.0, "{}", mob.position);
            // In the middle of the water, its box under its top
            assert!((mob.position.y - 3.5).abs() < 1e-4, "{}", mob.position);
        }
    }
}
