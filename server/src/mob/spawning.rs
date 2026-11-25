//! Hostile mobs appear in the dark around the players and go once no player is near; a
//! peaceful world has none. Every `SPAWN_INTERVAL_TICKS`, each player gets one attempt at a
//! random spot of the ring around it, which costs a bounded scan of the world
//! (`find_floor`, `ticks::sees_sky`, `hostiles::surroundings`).

use std::collections::HashMap;
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
        BlockId, ChunkMap, MobId, MobKind, ServerMob,
    },
};

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

/// Removes every hostile of a peaceful world, at once; otherwise, every
/// `SPAWN_INTERVAL_TICKS`, despawns the hostiles far from every player and gives each player
/// a spawn attempt.
pub fn hostile_spawning_system(mut world_map: ResMut<ServerWorldMap>, time: Res<ServerTime>) {
    let ServerWorldMap {
        mobs,
        chunks,
        players,
        changed_mobs,
        difficulty,
        ..
    } = world_map.as_mut();
    if !difficulty.has_hostiles() {
        mobs.retain(|_, mob| !mob.kind.is_hostile());
        return;
    }
    if !time.0.is_multiple_of(SPAWN_INTERVAL_TICKS) {
        return;
    }
    despawn_far_hostiles(mobs, players);
    let night = is_night(time.0);
    let mut rng = rand::thread_rng();
    let ids: Vec<PlayerId> = players.keys().copied().collect();
    for id in ids {
        for mob in spawn_attempt(chunks, players, mobs, id, night, &mut rng) {
            super::add_mob(mobs, changed_mobs, mob);
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

/// The hostiles of one attempt for the player `id`: a spot at a random angle and distance in
/// `SPAWN_RING`, at a random height near its feet, scanned down for a floor; if the spot is far
/// enough from every player, dark, and its place has a kind (`spawn_kind`), a pack of that kind
/// standing there, cut to the caps left.
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
    let Some((feet, floor)) = find_floor(chunks, start) else {
        return Vec::new();
    };
    spawn_at(chunks, players, feet, floor, night, room, rng)
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
    let too_close = players
        .values()
        .any(|player| player.position.with_y(0.0).distance(centre.with_y(0.0)) < SPAWN_RING.start);
    if too_close {
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
    use std::collections::HashSet;

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
        // y = 1 < 50, under a block: Gnawers or a Roller, at day as at night
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
                MobKind::Roller => assert_eq!(pack.len(), 1),
                other => panic!("{other:?}"),
            }
            // standing on the floor
            for mob in &pack {
                assert!((mob.position.y - mob.height / 2.0 - 1.0).abs() < 1e-5);
            }
            kinds.insert(kind);
        }
        assert_eq!(kinds.len(), 2);
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
        // Open stone at night has no kind yet
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
        for i in 1..=400 {
            map = run(map, MIDNIGHT_IN_DAY + i * SPAWN_INTERVAL_TICKS);
        }
        let hostiles = map
            .mobs
            .values()
            .filter(|mob| mob.kind.is_hostile())
            .count();
        assert_eq!(hostiles, PLAYER_CAP);
        assert!(count(&map, MobKind::Gnawer) > 0);
        for mob in map.mobs.values() {
            let away = mob.position.with_y(0.0).distance(Vec3::new(0.5, 0.0, 0.5));
            assert!(
                (SPAWN_RING.start - 1.0..=SPAWN_RING.end + 1.0).contains(&away),
                "{away}"
            );
        }
    }

    #[test]
    fn the_world_cap_holds_with_many_players() {
        let mut map = ServerWorldMap {
            chunks: flat(BlockId::Stone, true),
            ..Default::default()
        };
        for (i, hostile_at) in (0..WORLD_CAP).enumerate() {
            map.mobs.insert(
                i as u128,
                hostile(
                    MobKind::Gnawer,
                    Vec3::new(-70.0 + hostile_at as f32, 2.0, 70.0),
                ),
            );
        }
        map.players
            .insert(1, player_at(1, Vec3::new(0.5, 1.9, 0.5)));
        for i in 1..=50 {
            map = run(map, i * SPAWN_INTERVAL_TICKS);
        }
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
}
