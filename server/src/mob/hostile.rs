//! How the hostile mobs hunt, once per fixed step after their fall (`behavior::step_mob`): the
//! generic part keeps or forgets the prey and looks for one (`prey`), then one arm per kind
//! (`act`) goes at it and strikes. The Gnawer and the Bramble strike in reach (`melee`); the
//! Roller winds up and rolls (`roll`). The Stumpling strikes in reach too, but flees fire
//! first (`stumpling`), and sinks into the ground at dawn (`burrow`). The Drowner swims at its
//! prey in water and drags it down, and walks back to the water from land (`swim`).

use std::collections::HashMap;
use std::sync::LazyLock;

use bevy::math::{bounding::Aabb3d, bounding::IntersectsVolume, ops::atan2, IVec3, Quat, Vec3};
use rand::Rng;
use shared::{
    messages::PlayerId,
    players::{game_mode::DamageSource, Player},
    world::{
        hostiles::{distance_to_box, is_prey, line_of_sight, Hunt, Strike},
        BlockId, ChunkMap, MobAction, MobKind, MobTarget, ServerMob,
    },
};

use super::behavior::{flee, is_in_water, may_move_to, rest, walk, walk_speed, wander};

/// How long a strike shows (`MobAction::Attack`), in ticks: the client plays the Attack clip
/// once each time the action starts.
pub const ATTACK_SHOW_TICKS: u32 = 10;
/// How long the Roller winds up before it rolls, in ticks.
pub const WIND_UP_TICKS: u32 = 20;
/// The longest roll, in ticks: 3 seconds, 21 blocks.
pub const ROLL_TICKS: u32 = 60;
/// How long a Roller that rolled into a block is stunned, in ticks.
pub const STUN_TICKS: u32 = 40;
/// How long a Roller rests after it rolled into a player, or as far as it rolls, in ticks.
pub const ROLL_REST_TICKS: u32 = 20;
/// How long a Stumpling takes to sink into the ground at dawn, in ticks: 2 seconds.
pub const BURROW_TICKS: u32 = 40;
/// How near a fire or lava frightens a Stumpling, in blocks.
pub const FIRE_FEAR_DISTANCE: i32 = 6;
/// How far, horizontally, a Drowner on land looks for water to go back to, in blocks.
pub const WATER_SEARCH_DISTANCE: i32 = 8;

/// How fast a hostile of `kind` moves by its own velocity at most, in blocks per second: a
/// faster one was thrown (`behavior::is_flung`). The Roller's roll moves it without a
/// velocity, and the Bramble never moves.
pub fn own_speed(kind: MobKind) -> f32 {
    match kind {
        MobKind::Roller | MobKind::Bramble => 0.0,
        _ => kind.hunt().map_or(0.0, |hunt| hunt.speed),
    }
}

/// A hostile struck by `player` takes it as its prey, wherever it is: it does not flee as an
/// animal does. Its knock back is the blow's.
pub fn provoke(mob: &mut ServerMob, player: PlayerId) {
    mob.target = MobTarget::Player(player);
}

/// One step of the hunt of a hostile: the prey, then what its kind does.
///
/// The arm of each kind is where the later hostiles plug in their own way of going at a prey.
pub fn act(
    mob: &mut ServerMob,
    chunks: &ChunkMap,
    players: &mut HashMap<PlayerId, Player>,
    delta: f32,
    rng: &mut impl Rng,
) {
    let (Some(hunt), Some(strike)) = (mob.kind.hunt(), mob.kind.strike()) else {
        return;
    };
    if mob.action == MobAction::Burrow {
        return burrow(mob);
    }
    let prey = prey(mob, chunks, players, &hunt);
    match mob.kind {
        MobKind::Roller => roll(mob, chunks, players, prey, (hunt, strike), delta),
        MobKind::Stumpling => stumpling(mob, chunks, players, prey, (hunt, strike), delta, rng),
        MobKind::Drowner => swim(mob, chunks, players, prey, (hunt, strike), delta, rng),
        _ => melee(mob, chunks, players, prey, (hunt, strike), delta, rng),
    }
}

/// A Stumpling that burrows (`MobAction::Burrow`, which the spawning system starts at dawn)
/// goes down through the ground by its height over `BURROW_TICKS`, its ticks counting down to
/// 0, when the spawning system removes it.
fn burrow(mob: &mut ServerMob) {
    if mob.action_ticks > 0 {
        mob.position.y -= mob.height / BURROW_TICKS as f32;
        mob.action_ticks -= 1;
    }
    mob.velocity = Vec3::ZERO;
}

/// The cells within `FIRE_FEAR_DISTANCE` of a cell, nearest first.
static FEAR_OFFSETS: LazyLock<Vec<IVec3>> = LazyLock::new(|| {
    let r = FIRE_FEAR_DISTANCE;
    let mut offsets: Vec<IVec3> = (-r..=r)
        .flat_map(|x| (-r..=r).flat_map(move |y| (-r..=r).map(move |z| IVec3::new(x, y, z))))
        .filter(|offset| offset.length_squared() <= r * r)
        .collect();
    offsets.sort_by_key(|offset| offset.length_squared());
    offsets
});

/// The centre of the nearest fire or lava block within `FIRE_FEAR_DISTANCE` of the cell of
/// `position`, if any: at most the 925 cells of `FEAR_OFFSETS`.
fn nearest_fire(chunks: &ChunkMap, position: Vec3) -> Option<Vec3> {
    let cell = position.floor().as_ivec3();
    FEAR_OFFSETS
        .iter()
        .map(|offset| cell + *offset)
        .find(|pos| {
            chunks
                .get_block(*pos)
                .is_some_and(|block| matches!(block.id, BlockId::Fire | BlockId::Lava))
        })
        .map(|pos| pos.as_vec3() + Vec3::splat(0.5))
}

/// The Stumpling: it flees the nearest fire or lava within `FIRE_FEAR_DISTANCE`
/// (`MobAction::Flee`, at its hunting speed), keeping its prey but striking nothing; otherwise
/// it hunts in reach like the Gnawer (`melee`).
fn stumpling(
    mob: &mut ServerMob,
    chunks: &ChunkMap,
    players: &mut HashMap<PlayerId, Player>,
    prey: Option<PlayerId>,
    (hunt, strike): (Hunt, Strike),
    delta: f32,
    rng: &mut impl Rng,
) {
    if let Some(fire) = nearest_fire(chunks, mob.position) {
        mob.action = MobAction::Flee;
        mob.action_ticks = 0;
        return flee(mob, chunks, fire, hunt.speed, delta);
    }
    if mob.action == MobAction::Flee {
        (mob.action, mob.action_ticks) = (MobAction::Idle, 0);
    }
    melee(mob, chunks, players, prey, (hunt, strike), delta, rng)
}

/// Half the size of the box of `player`.
fn half_box(player: &Player) -> Vec3 {
    Vec3::new(player.width, player.height, player.width) / 2.0
}

/// How far the centre of `mob` is from the box of `player`.
fn distance(mob: &ServerMob, player: &Player) -> f32 {
    distance_to_box(mob.position, player.position, half_box(player))
}

/// Where `mob` looks from: near the top of its box.
fn eyes(mob: &ServerMob) -> Vec3 {
    mob.position + Vec3::Y * (mob.height / 2.0 - 0.1)
}

/// The prey of `mob`: the one it hunts if it still may (`is_prey`, within its forget
/// distance), else the nearest prey within its sight that it sees (`line_of_sight`), which it
/// then hunts. Only the players within sight are looked at, each along a segment of at most the
/// sight.
fn prey(
    mob: &mut ServerMob,
    chunks: &ChunkMap,
    players: &HashMap<PlayerId, Player>,
    hunt: &Hunt,
) -> Option<PlayerId> {
    if let MobTarget::Player(id) = mob.target {
        let kept = players
            .get(&id)
            .is_some_and(|player| is_prey(player) && distance(mob, player) <= hunt.forget);
        if kept {
            return Some(id);
        }
        mob.target = MobTarget::None;
    }
    let found = players
        .values()
        .filter(|player| is_prey(player))
        .map(|player| (player, distance(mob, player)))
        .filter(|&(_, d)| d <= hunt.sight)
        .filter(|(player, _)| line_of_sight(chunks, eyes(mob), player.eye_position()))
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(player, _)| player.id);
    if let Some(id) = found {
        mob.target = MobTarget::Player(id);
    }
    found
}

/// Turns `mob` toward `target`, on the horizontal plane.
fn face(mob: &mut ServerMob, target: Vec3) {
    let towards = (target - mob.position).with_y(0.0);
    if towards.length_squared() > 1e-6 {
        mob.rotation = Quat::from_rotation_y(atan2(towards.x, towards.z));
    }
}

/// `mob` strikes `player` (`Player::hurt`, which its invulnerability may ignore), and knocks it
/// away from itself; a Drowner's grab drags it down (`Player::drag`).
fn strike_player(mob: &ServerMob, player: &mut Player, strike: &Strike, away: Vec3) {
    player.hurt(strike.damage, DamageSource::Mob(mob.kind));
    if strike.knock_back > 0.0 {
        player.knock_back(away, strike.knock_back);
    }
    if mob.kind == MobKind::Drowner {
        player.drag();
    }
}

/// Strikes `player` if it is in reach and the cooldown of `mob` is over, and shows it for
/// `ATTACK_SHOW_TICKS`. Returns whether `mob` is busy striking: it then stands.
fn strike_in_reach(mob: &mut ServerMob, player: &mut Player, strike: &Strike) -> bool {
    if distance(mob, player) <= strike.reach && mob.attack_wait == 0 {
        let away = player.position - mob.position;
        strike_player(mob, player, strike, away);
        mob.attack_wait = strike.cooldown;
        mob.action = MobAction::Attack;
        mob.action_ticks = ATTACK_SHOW_TICKS;
        return true;
    }
    if mob.action == MobAction::Attack && mob.action_ticks > 1 {
        mob.action_ticks -= 1;
        return true;
    }
    false
}

/// Stops `mob` where it stands, which a strike or a wind-up does.
fn stand(mob: &mut ServerMob) {
    mob.velocity.x = 0.0;
    mob.velocity.z = 0.0;
}

/// A hostile that strikes in reach: it walks at its prey at its hunting speed, if it walks,
/// strikes it when its centre is within reach of the prey's box and its cooldown is over, shows
/// the strike for `ATTACK_SHOW_TICKS`, and waits facing the prey while it is in reach. Without
/// a prey it wanders as an animal, or stands.
fn melee(
    mob: &mut ServerMob,
    chunks: &ChunkMap,
    players: &mut HashMap<PlayerId, Player>,
    prey: Option<PlayerId>,
    (hunt, strike): (Hunt, Strike),
    delta: f32,
    rng: &mut impl Rng,
) {
    let Some(player) = prey.and_then(|id| players.get_mut(&id)) else {
        if matches!(mob.action, MobAction::Chase | MobAction::Attack) {
            rest(mob, rng);
        }
        return wander(mob, chunks, players, delta, rng);
    };
    let in_reach = distance(mob, player) <= strike.reach;
    face(mob, player.position);
    if strike_in_reach(mob, player, &strike) {
        return stand(mob);
    }
    if in_reach || hunt.speed == 0.0 {
        mob.action = MobAction::Idle;
        mob.action_ticks = 0;
        return stand(mob);
    }
    mob.action = MobAction::Chase;
    let towards = (player.position - mob.position)
        .with_y(0.0)
        .normalize_or_zero();
    walk(mob, chunks, towards, hunt.speed, delta);
}

/// The Drowner. In water (`behavior::is_in_water`, where it floats) it swims straight at its
/// prey's centre at its hunting speed, a step taken only if its box is free there and its centre
/// stays in water, and rests without a prey. Anywhere, it grabs its prey in reach
/// (`strike_in_reach`, which drags it down). On land it does not chase: it walks back to the
/// nearest water (`nearest_water`) at its walking speed, or wanders when there is none near.
fn swim(
    mob: &mut ServerMob,
    chunks: &ChunkMap,
    players: &mut HashMap<PlayerId, Player>,
    prey: Option<PlayerId>,
    (hunt, strike): (Hunt, Strike),
    delta: f32,
    rng: &mut impl Rng,
) {
    let in_water = is_in_water(chunks, mob.position);
    if let Some(player) = prey.and_then(|id| players.get_mut(&id)) {
        face(mob, player.position);
        if strike_in_reach(mob, player, &strike) {
            return stand(mob);
        }
        if in_water {
            if distance(mob, player) <= strike.reach {
                (mob.action, mob.action_ticks) = (MobAction::Idle, 0);
                return;
            }
            mob.action = MobAction::Chase;
            let towards = (player.position - mob.position).normalize_or_zero();
            let step = towards * (hunt.speed * delta);
            // Straight on, else along the ground or straight up or down
            for step in [step, step.with_y(0.0), Vec3::Y * step.y] {
                let moved = mob.position + step;
                if step != Vec3::ZERO
                    && is_in_water(chunks, moved)
                    && may_move_to(mob, chunks, moved)
                {
                    mob.position = moved;
                    break;
                }
            }
            return;
        }
    } else if in_water {
        if mob.action != MobAction::Idle {
            (mob.action, mob.action_ticks) = (MobAction::Idle, 0);
        }
        return;
    }
    // On land
    match nearest_water(chunks, mob) {
        Some(water) => {
            mob.action = MobAction::Walk;
            mob.target = prey.map_or(MobTarget::None, MobTarget::Player);
            let towards = (water - mob.position).with_y(0.0).normalize_or_zero();
            walk(mob, chunks, towards, walk_speed(mob.kind), delta);
        }
        None => {
            if mob.action == MobAction::Walk && !matches!(mob.target, MobTarget::Position(_)) {
                rest(mob, rng);
            }
            wander(mob, chunks, players, delta, rng);
        }
    }
}

/// The cells within `WATER_SEARCH_DISTANCE` horizontally, nearest first, at the height of the
/// feet and one below.
static WATER_OFFSETS: LazyLock<Vec<IVec3>> = LazyLock::new(|| {
    let r = WATER_SEARCH_DISTANCE;
    let mut offsets: Vec<IVec3> = (-r..=r)
        .flat_map(|x| (-1..=0).flat_map(move |y| (-r..=r).map(move |z| IVec3::new(x, y, z))))
        .filter(|offset| offset.x * offset.x + offset.z * offset.z <= r * r)
        .collect();
    offsets.sort_by_key(|offset| (offset.x * offset.x + offset.z * offset.z, -offset.y));
    offsets
});

/// The centre of the nearest water cell around the feet of `mob` (`WATER_OFFSETS`, at most 402
/// lookups), if any.
fn nearest_water(chunks: &ChunkMap, mob: &ServerMob) -> Option<Vec3> {
    let feet = (mob.position - Vec3::Y * (mob.height / 2.0 - 0.1))
        .floor()
        .as_ivec3();
    WATER_OFFSETS
        .iter()
        .map(|offset| feet + *offset)
        .find(|pos| {
            chunks
                .get_block(*pos)
                .is_some_and(|block| block.id == BlockId::Water)
        })
        .map(|pos| pos.as_vec3() + Vec3::splat(0.5))
}

/// The Roller: at rest (`Idle`) until it has a prey, then it winds up facing it
/// (`Attack`, `WIND_UP_TICKS`), then rolls straight on (`Chase`) at its hunting speed for
/// `ROLL_TICKS` at most. The roll stops on the first prey its box touches, which it strikes and
/// knocks back along the roll, or on a block, which stuns it for `STUN_TICKS`. An `Idle` Roller
/// with ticks left rests or is stunned, and looks at no prey.
fn roll(
    mob: &mut ServerMob,
    chunks: &ChunkMap,
    players: &mut HashMap<PlayerId, Player>,
    prey: Option<PlayerId>,
    (hunt, strike): (Hunt, Strike),
    delta: f32,
) {
    let prey_position = prey
        .and_then(|id| players.get(&id))
        .map(|player| player.position);
    match mob.action {
        MobAction::Attack => {
            if let Some(target) = prey_position {
                face(mob, target);
            }
            mob.action_ticks = mob.action_ticks.saturating_sub(1);
            if mob.action_ticks == 0 {
                mob.action = MobAction::Chase;
                mob.action_ticks = ROLL_TICKS;
            }
            stand(mob);
        }
        MobAction::Chase => {
            let direction = mob.rotation * Vec3::Z;
            let moved = mob.position + direction * (hunt.speed * delta);
            if !may_move_to(mob, chunks, moved) {
                (mob.action, mob.action_ticks) = (MobAction::Idle, STUN_TICKS);
                return;
            }
            mob.position = moved;
            let half = Vec3::new(mob.width, mob.height, mob.depth) / 2.0;
            let body = Aabb3d::new(mob.position, half);
            let hit = players.values_mut().find(|player| {
                is_prey(player) && body.intersects(&Aabb3d::new(player.position, half_box(player)))
            });
            if let Some(player) = hit {
                strike_player(mob, player, &strike, direction);
                (mob.action, mob.action_ticks) = (MobAction::Idle, ROLL_REST_TICKS);
                return;
            }
            mob.action_ticks = mob.action_ticks.saturating_sub(1);
            if mob.action_ticks == 0 {
                (mob.action, mob.action_ticks) = (MobAction::Idle, ROLL_REST_TICKS);
            }
        }
        _ => {
            mob.action = MobAction::Idle;
            if mob.action_ticks > 0 {
                mob.action_ticks -= 1;
            } else if let Some(target) = prey_position {
                face(mob, target);
                (mob.action, mob.action_ticks) = (MobAction::Attack, WIND_UP_TICKS);
            }
            stand(mob);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mob::behavior::mob_behavior_system;
    use crate::world::data::ServerWorldMap;
    use bevy::ecs::system::RunSystemOnce;
    use bevy::math::IVec3;
    use bevy::prelude::{Transform, World};
    use bevy::time::{Fixed, Time};
    use shared::players::constants::DRAG_MS;
    use shared::players::game_mode::{DeathCause, GameMode, MAX_HEALTH};
    use shared::world::BlockId;
    use shared::TICKS_PER_SECOND;
    use std::time::Duration;

    /// Flat stone ground with its top at y = 1, over x, z in -40..40.
    fn ground() -> ServerWorldMap {
        let mut map = ServerWorldMap {
            chunks: ChunkMap::with_loaded_region(IVec3::new(-40, -16, -40), IVec3::new(39, 15, 39)),
            ..Default::default()
        };
        map.chunks.fill_box(
            IVec3::new(-40, 0, -40),
            IVec3::new(39, 0, 39),
            BlockId::Stone,
        );
        map
    }

    /// A survival player standing on the ground of `ground` at `(x, z)`.
    fn player_at(id: PlayerId, x: f32, z: f32) -> Player {
        let mut player = Player::new(id, format!("p{id}"), Vec3::ZERO, Transform::default());
        player.position = Vec3::new(x, 1.0 + player.height / 2.0, z);
        player
    }

    fn mob_at(kind: MobKind, x: f32, z: f32) -> ServerMob {
        ServerMob {
            on_ground: true,
            ..ServerMob::new(kind, ServerMob::standing_at(kind, x, 1.0, z))
        }
    }

    /// The world of `map` with its mob 0 and its players, ready to run ticks.
    fn world(map: ServerWorldMap) -> World {
        let mut world = World::new();
        world.insert_resource(map);
        let mut time = Time::<Fixed>::default();
        time.advance_by(Duration::from_secs_f64(1.0 / TICKS_PER_SECOND as f64));
        world.insert_resource(time);
        world
    }

    /// Runs `ticks` of the mobs, the players' damage timers counting down as the server's.
    fn run(world: &mut World, ticks: u32) {
        for _ in 0..ticks {
            world.run_system_once(mob_behavior_system).unwrap();
            for player in world.resource_mut::<ServerWorldMap>().players.values_mut() {
                player.tick_damage_timers();
            }
        }
    }

    fn mob(world: &World) -> &ServerMob {
        &world.resource::<ServerWorldMap>().mobs[&0]
    }

    fn player(world: &World, id: PlayerId) -> &Player {
        &world.resource::<ServerWorldMap>().players[&id]
    }

    fn with(mob: ServerMob, players: Vec<Player>) -> World {
        let mut map = ground();
        map.mobs.insert(0, mob);
        map.players = players.into_iter().map(|p| (p.id, p)).collect();
        world(map)
    }

    #[test]
    fn a_gnawer_sees_a_survival_player_within_sixteen_blocks_and_chases_it() {
        let mut w = with(
            mob_at(MobKind::Gnawer, 0.5, 0.5),
            vec![player_at(1, 10.5, 0.5)],
        );
        run(&mut w, 1);
        assert!(matches!(mob(&w).target, MobTarget::Player(1)));
        assert_eq!(mob(&w).action, MobAction::Chase);
        run(&mut w, 20);
        // 2.8 blocks a second
        let ran = mob(&w).position.x - 0.5;
        assert!((2.6..=3.0).contains(&ran), "{ran}");
    }

    #[test]
    fn a_gnawer_does_not_see_a_player_too_far_behind_a_wall_or_in_creative() {
        let far = with(
            mob_at(MobKind::Gnawer, 0.5, 0.5),
            vec![player_at(1, 18.5, 0.5)],
        );
        let mut walled = with(
            mob_at(MobKind::Gnawer, 0.5, 0.5),
            vec![player_at(1, 10.5, 0.5)],
        );
        walled.resource_mut::<ServerWorldMap>().chunks.fill_box(
            IVec3::new(5, 1, -5),
            IVec3::new(5, 4, 5),
            BlockId::Stone,
        );
        let mut creative = player_at(1, 10.5, 0.5);
        creative.game_mode = GameMode::Creative;
        let creative = with(mob_at(MobKind::Gnawer, 0.5, 0.5), vec![creative]);
        for mut w in [far, walled, creative] {
            run(&mut w, 1);
            assert!(!matches!(mob(&w).target, MobTarget::Player(_)));
            assert_ne!(mob(&w).action, MobAction::Chase);
        }
    }

    #[test]
    fn a_hunter_forgets_its_prey_beyond_twenty_four_blocks_or_once_it_turns_creative() {
        let mut w = with(
            mob_at(MobKind::Gnawer, 0.5, 0.5),
            vec![player_at(1, 10.5, 0.5)],
        );
        run(&mut w, 1);
        // Beyond sight but within the forget distance: still hunted, wall or not
        w.resource_mut::<ServerWorldMap>()
            .players
            .get_mut(&1)
            .unwrap()
            .position
            .x = 22.0;
        run(&mut w, 1);
        assert!(matches!(mob(&w).target, MobTarget::Player(1)));
        w.resource_mut::<ServerWorldMap>()
            .players
            .get_mut(&1)
            .unwrap()
            .position
            .x = 30.0;
        run(&mut w, 1);
        assert!(!matches!(mob(&w).target, MobTarget::Player(_)));
        assert_ne!(mob(&w).action, MobAction::Chase);

        let mut w = with(
            mob_at(MobKind::Gnawer, 0.5, 0.5),
            vec![player_at(1, 5.5, 0.5)],
        );
        run(&mut w, 1);
        w.resource_mut::<ServerWorldMap>()
            .players
            .get_mut(&1)
            .unwrap()
            .game_mode = GameMode::Creative;
        run(&mut w, 1);
        assert!(!matches!(mob(&w).target, MobTarget::Player(_)));
    }

    #[test]
    fn a_gnawer_in_reach_bites_for_two_every_fifteen_ticks_and_shows_it() {
        let mut w = with(
            mob_at(MobKind::Gnawer, 0.5, 0.5),
            vec![player_at(1, 1.6, 0.5)],
        );
        run(&mut w, 1);
        assert_eq!(player(&w, 1).health, MAX_HEALTH - 2);
        assert_eq!(mob(&w).action, MobAction::Attack);
        run(&mut w, ATTACK_SHOW_TICKS - 1);
        assert_eq!(mob(&w).action, MobAction::Attack);
        run(&mut w, 1);
        assert_eq!(mob(&w).action, MobAction::Idle, "waits in reach");
        // The next bite comes 15 ticks after the first
        run(&mut w, 15 - ATTACK_SHOW_TICKS - 1);
        assert_eq!(player(&w, 1).health, MAX_HEALTH - 2);
        run(&mut w, 1);
        assert_eq!(player(&w, 1).health, MAX_HEALTH - 4);
        assert_eq!(mob(&w).action, MobAction::Attack);
        // Until the player dies of it, killed by a Gnawer
        run(&mut w, 15 * 10);
        assert!(player(&w, 1).is_dead());
        assert_eq!(
            player(&w, 1).death_cause,
            Some(DeathCause::Mob(MobKind::Gnawer))
        );
        // and left alone
        run(&mut w, 1);
        assert!(!matches!(mob(&w).target, MobTarget::Player(_)));
    }

    #[test]
    fn a_creative_player_in_reach_is_never_bitten() {
        let mut creative = player_at(1, 1.6, 0.5);
        creative.game_mode = GameMode::Creative;
        let mut w = with(mob_at(MobKind::Gnawer, 0.5, 0.5), vec![creative]);
        // Even once it struck the gnawer
        provoke(
            w.resource_mut::<ServerWorldMap>().mobs.get_mut(&0).unwrap(),
            1,
        );
        run(&mut w, 40);
        assert_eq!(player(&w, 1).health, MAX_HEALTH);
        assert_ne!(mob(&w).action, MobAction::Attack);
    }

    #[test]
    fn a_bramble_never_moves_and_bites_for_three_every_twenty_ticks_within_two_and_a_half() {
        let start = mob_at(MobKind::Bramble, 0.5, 0.5).position;
        let mut w = with(
            mob_at(MobKind::Bramble, 0.5, 0.5),
            vec![player_at(1, 4.5, 0.5)],
        );
        run(&mut w, 100);
        assert_eq!(player(&w, 1).health, MAX_HEALTH, "3.7 blocks from its box");
        assert_eq!(mob(&w).position, start);
        assert_eq!(mob(&w).action, MobAction::Idle);
        // 2.4 blocks from its centre to the player's box
        w.resource_mut::<ServerWorldMap>()
            .players
            .get_mut(&1)
            .unwrap()
            .position
            .x = 0.5 + 2.4 + 0.3;
        run(&mut w, 1);
        assert_eq!(player(&w, 1).health, MAX_HEALTH - 3);
        assert_eq!(mob(&w).action, MobAction::Attack);
        run(&mut w, 19);
        assert_eq!(player(&w, 1).health, MAX_HEALTH - 3);
        run(&mut w, 1);
        assert_eq!(player(&w, 1).health, MAX_HEALTH - 6);
        assert_eq!(mob(&w).position, start);
    }

    /// The Roller at the origin facing a player straight ahead along +x, `distance` away.
    fn roller_facing(distance: f32) -> World {
        with(
            mob_at(MobKind::Roller, 0.5, 0.5),
            vec![player_at(1, 0.5 + distance, 0.5)],
        )
    }

    #[test]
    fn a_roller_winds_up_for_twenty_ticks_then_rolls_at_seven_blocks_a_second() {
        let mut w = roller_facing(11.0);
        run(&mut w, 1);
        assert_eq!(mob(&w).action, MobAction::Attack);
        run(&mut w, WIND_UP_TICKS);
        assert_eq!(mob(&w).position.x, 0.5, "still while it winds up");
        assert_eq!(mob(&w).action, MobAction::Chase);
        // The player steps aside: the roll goes straight on
        w.resource_mut::<ServerWorldMap>()
            .players
            .get_mut(&1)
            .unwrap()
            .position
            .z = 6.0;
        run(&mut w, 20);
        let rolled = mob(&w).position - Vec3::new(0.5, mob(&w).position.y, 0.5);
        assert!((rolled.x - 7.0).abs() < 0.05, "{rolled}");
        assert!(rolled.z.abs() < 1e-4, "{rolled}");
        assert_eq!(mob(&w).action, MobAction::Chase);
        assert_eq!(player(&w, 1).health, MAX_HEALTH);
    }

    #[test]
    fn a_roller_that_hits_a_player_hurts_it_by_four_and_knocks_it_back() {
        let mut w = roller_facing(8.0);
        run(&mut w, WIND_UP_TICKS + 30);
        assert_eq!(player(&w, 1).health, MAX_HEALTH - 4);
        let push = player(&w, 1).velocity.with_y(0.0);
        assert!((push - Vec3::X * 8.0).length() < 1e-4, "{push}");
        assert_eq!(mob(&w).action, MobAction::Idle);
        assert_eq!(
            player(&w, 1).death_cause,
            None,
            "hurt, not killed: {:?}",
            player(&w, 1).health
        );
    }

    #[test]
    fn a_roller_that_rolls_into_a_block_is_stunned_for_forty_ticks() {
        let mut w = roller_facing(8.0);
        w.resource_mut::<ServerWorldMap>().chunks.fill_box(
            IVec3::new(4, 1, -3),
            IVec3::new(4, 3, 3),
            BlockId::Stone,
        );
        // It sees the player over the wall: up in the air
        w.resource_mut::<ServerWorldMap>()
            .players
            .get_mut(&1)
            .unwrap()
            .position
            .y = 8.0;
        run(&mut w, 1);
        assert_eq!(mob(&w).action, MobAction::Attack);
        // 20 ticks of wind-up, 3 blocks of roll at 0.35 a tick
        run(&mut w, WIND_UP_TICKS + 10);
        assert_eq!(mob(&w).action, MobAction::Idle);
        let stunned_at = mob(&w).position;
        assert!(stunned_at.x < 4.0 - 0.375 + 1e-3, "{stunned_at}");
        run(&mut w, STUN_TICKS - 2);
        assert_eq!(mob(&w).action, MobAction::Idle, "stunned");
        run(&mut w, 2);
        assert_eq!(mob(&w).action, MobAction::Attack, "winds up again");
        assert_eq!(player(&w, 1).health, MAX_HEALTH);
    }

    #[test]
    fn a_roller_rolls_through_a_creative_player() {
        let mut creative = player_at(1, 8.5, 0.5);
        creative.game_mode = GameMode::Creative;
        let mut w = with(mob_at(MobKind::Roller, 0.5, 0.5), vec![creative]);
        provoke(
            w.resource_mut::<ServerWorldMap>().mobs.get_mut(&0).unwrap(),
            1,
        );
        run(&mut w, 80);
        assert_eq!(player(&w, 1).health, MAX_HEALTH);
        assert_eq!(player(&w, 1).velocity, Vec3::ZERO);
    }

    #[test]
    fn a_stumpling_plods_at_its_prey_and_hits_for_three_every_twenty_ticks() {
        let mut w = with(
            mob_at(MobKind::Stumpling, 0.5, 0.5),
            vec![player_at(1, 10.5, 0.5)],
        );
        run(&mut w, 20);
        assert_eq!(mob(&w).action, MobAction::Chase);
        // 1.2 blocks a second
        let walked = mob(&w).position.x - 0.5;
        assert!((1.1..=1.3).contains(&walked), "{walked}");
        w.resource_mut::<ServerWorldMap>()
            .players
            .get_mut(&1)
            .unwrap()
            .position
            .x = mob(&w).position.x + 1.2 + 0.4;
        run(&mut w, 1);
        assert_eq!(player(&w, 1).health, MAX_HEALTH - 3);
        run(&mut w, 19);
        assert_eq!(player(&w, 1).health, MAX_HEALTH - 3);
        run(&mut w, 1);
        assert_eq!(player(&w, 1).health, MAX_HEALTH - 6);
    }

    #[test]
    fn a_stumpling_flees_a_fire_or_lava_within_six_blocks_and_strikes_nothing_meanwhile() {
        for (danger, at) in [(BlockId::Fire, 5), (BlockId::Lava, -4), (BlockId::Fire, 7)] {
            let mut w = with(
                mob_at(MobKind::Stumpling, 0.5, 0.5),
                vec![player_at(1, 1.9, 0.5)],
            );
            w.resource_mut::<ServerWorldMap>().chunks.fill_box(
                IVec3::new(0, 1, at),
                IVec3::new(0, 1, at),
                danger,
            );
            run(&mut w, 20);
            let moved = mob(&w).position.z - 0.5;
            if at.abs() > 6 {
                // Too far to be feared: it strikes its prey
                assert_eq!(player(&w, 1).health, MAX_HEALTH - 3, "{danger:?} {at}");
                assert!(moved.abs() < 0.1, "{moved}");
                continue;
            }
            assert_eq!(player(&w, 1).health, MAX_HEALTH, "{danger:?} {at}");
            assert_eq!(mob(&w).action, MobAction::Flee);
            assert!(
                moved * (at as f32) < -0.5,
                "{danger:?} at {at}: moved {moved}"
            );
        }
    }

    #[test]
    fn a_burrowing_stumpling_sinks_by_its_height_in_forty_ticks_and_strikes_nothing() {
        let mut stumpling = mob_at(MobKind::Stumpling, 0.5, 0.5);
        stumpling.action = MobAction::Burrow;
        stumpling.action_ticks = BURROW_TICKS;
        let start = stumpling.position;
        let mut w = with(stumpling, vec![player_at(1, 1.6, 0.5)]);
        run(&mut w, BURROW_TICKS / 2);
        let sunk = start.y - mob(&w).position.y;
        assert!((sunk - 1.95 / 2.0).abs() < 1e-3, "{sunk}");
        run(&mut w, BURROW_TICKS / 2);
        let sunk = start.y - mob(&w).position.y;
        assert!((sunk - 1.95).abs() < 1e-3, "{sunk}");
        assert_eq!(mob(&w).action, MobAction::Burrow);
        assert_eq!(mob(&w).action_ticks, 0);
        // It stays where it went down
        run(&mut w, 5);
        assert!((start.y - mob(&w).position.y - 1.95).abs() < 1e-3);
        assert_eq!(mob(&w).position.x, start.x);
        assert_eq!(player(&w, 1).health, MAX_HEALTH);
    }

    /// `ground` with water from y = 1 to y = 8 over x up to 4: the land begins at x = 5.
    fn lake() -> World {
        let mut map = ground();
        map.chunks.fill_box(
            IVec3::new(-40, 1, -40),
            IVec3::new(4, 8, 39),
            BlockId::Water,
        );
        world(map)
    }

    fn put(w: &mut World, mob: ServerMob, players: Vec<Player>) {
        let mut map = w.resource_mut::<ServerWorldMap>();
        map.mobs.insert(0, mob);
        map.players = players.into_iter().map(|p| (p.id, p)).collect();
    }

    /// A player floating in the lake, its centre at `at`.
    fn swimmer(id: PlayerId, at: Vec3) -> Player {
        let mut player = player_at(id, at.x, at.z);
        player.position = at;
        player.on_ground = false;
        player
    }

    #[test]
    fn a_drowner_swims_at_its_prey_in_three_dimensions_at_three_blocks_a_second() {
        let mut w = lake();
        let start = Vec3::new(-10.5, 2.5, 0.5);
        put(
            &mut w,
            ServerMob::new(MobKind::Drowner, start),
            vec![swimmer(1, Vec3::new(-2.5, 6.5, 0.5))],
        );
        run(&mut w, 1);
        assert_eq!(mob(&w).action, MobAction::Chase);
        run(&mut w, 19);
        let swum = mob(&w).position - start;
        assert!((swum.length() - 3.0).abs() < 0.2, "{swum}");
        assert!(swum.y > 1.0, "it rises toward its prey: {swum}");
        assert!(swum.x > 2.0, "{swum}");
    }

    #[test]
    fn a_drowner_grabs_its_prey_for_two_and_drags_it_down() {
        let mut w = lake();
        put(
            &mut w,
            ServerMob::new(MobKind::Drowner, Vec3::new(-5.5, 4.0, 0.5)),
            vec![swimmer(1, Vec3::new(-4.0, 4.5, 0.5))],
        );
        run(&mut w, 1);
        assert_eq!(player(&w, 1).health, MAX_HEALTH - 2);
        assert_eq!(player(&w, 1).dragged_ms, DRAG_MS);
        assert_eq!(mob(&w).action, MobAction::Attack);
        assert_eq!(
            player(&w, 1).death_cause,
            None,
            "{:?}",
            player(&w, 1).health
        );
        run(&mut w, 20);
        assert_eq!(player(&w, 1).health, MAX_HEALTH - 4);
    }

    #[test]
    fn a_drowner_in_water_floats_and_a_blow_does_not_fling_it() {
        let mut w = lake();
        let start = Vec3::new(-10.5, 4.5, 0.5);
        let mut drowner = ServerMob::new(MobKind::Drowner, start);
        drowner.velocity = Vec3::new(6.0, 4.0, 0.0);
        put(&mut w, drowner, vec![]);
        run(&mut w, 20);
        assert!(
            mob(&w).position.distance(start) < 0.5,
            "{}",
            mob(&w).position
        );
        assert_eq!(mob(&w).velocity, Vec3::ZERO);
    }

    #[test]
    fn a_drowner_does_not_leave_the_water_for_its_prey_and_walks_back_to_it_from_land() {
        // In the water by the shore, its prey on land: it stays in the water
        let mut w = lake();
        put(
            &mut w,
            ServerMob::new(MobKind::Drowner, Vec3::new(3.5, 2.0, 0.5)),
            vec![player_at(1, 10.5, 0.5)],
        );
        run(&mut w, 40);
        let at = mob(&w).position;
        assert_eq!(
            w.resource::<ServerWorldMap>()
                .chunks
                .get_block(at.floor().as_ivec3())
                .map(|block| block.id),
            Some(BlockId::Water),
            "at {at}"
        );
        assert_eq!(player(&w, 1).health, MAX_HEALTH);

        // On land, its prey farther inland: it walks back to the water at 0.8 a second
        let mut w = lake();
        let start = ServerMob::standing_at(MobKind::Drowner, 7.5, 1.0, 0.5);
        let mut drowner = ServerMob::new(MobKind::Drowner, start);
        drowner.on_ground = true;
        put(&mut w, drowner, vec![player_at(1, 12.5, 0.5)]);
        run(&mut w, 20);
        let walked = start.x - mob(&w).position.x;
        assert!((0.7..=0.9).contains(&walked), "{walked}");
        assert_eq!(player(&w, 1).health, MAX_HEALTH);
    }
}
