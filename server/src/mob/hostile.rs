//! How the hostile mobs hunt, once per fixed step after their fall (`behavior::step_mob`): the
//! generic part keeps or forgets the prey and looks for one (`prey`), then one arm per kind
//! (`act`) goes at it and strikes. The Gnawer and the Bramble strike in reach (`melee`); the
//! Roller winds up and rolls (`roll`).

use std::collections::HashMap;

use bevy::math::{bounding::Aabb3d, bounding::IntersectsVolume, ops::atan2, Quat, Vec3};
use rand::Rng;
use shared::{
    messages::PlayerId,
    players::{game_mode::DamageSource, Player},
    world::{
        hostiles::{distance_to_box, is_prey, line_of_sight, Hunt, Strike},
        ChunkMap, MobAction, MobKind, MobTarget, ServerMob,
    },
};

use super::behavior::{may_move_to, rest, walk, wander};

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
    let prey = prey(mob, chunks, players, &hunt);
    match mob.kind {
        MobKind::Roller => roll(mob, chunks, players, prey, (hunt, strike), delta),
        _ => melee(mob, chunks, players, prey, (hunt, strike), delta, rng),
    }
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
/// away from itself.
fn strike_player(mob: &ServerMob, player: &mut Player, strike: &Strike, away: Vec3) {
    player.hurt(strike.damage, DamageSource::Mob(mob.kind));
    if strike.knock_back > 0.0 {
        player.knock_back(away, strike.knock_back);
    }
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
    if in_reach && mob.attack_wait == 0 {
        let away = player.position - mob.position;
        strike_player(mob, player, &strike, away);
        mob.attack_wait = strike.cooldown;
        mob.action = MobAction::Attack;
        mob.action_ticks = ATTACK_SHOW_TICKS;
        return stand(mob);
    }
    if mob.action == MobAction::Attack && mob.action_ticks > 1 {
        mob.action_ticks -= 1;
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
}
