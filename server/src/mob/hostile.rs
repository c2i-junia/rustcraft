//! How the hostile mobs hunt, once per fixed step after their fall (`behavior::step_mob`): the
//! generic part keeps or forgets the prey and looks for one (`prey`), then one arm per kind
//! (`act`) goes at it and strikes. The Gnawer and the Bramble strike in reach (`melee`); the
//! Roller winds up and rolls (`roll`). The Stumpling strikes in reach too, but flees fire
//! first (`stumpling`), and sinks into the ground at dawn (`burrow`). The Drowner swims at its
//! prey in water and drags it down, and walks back to the water from land (`swim`). The
//! Cinderling hovers at a distance and throws embers (`hover_and_throw`, `cinder::ember`). The
//! Gust Owl circles high over its prey and dives at it (`circle_and_dive`). The Gnawer and the
//! Stumpling find their way to their prey on foot (`chase`, `path::find_path`): round a wall,
//! up a step, down a ledge, and for the Stumpling through a door, which it opens.

use std::collections::HashMap;
use std::ops::RangeInclusive;
use std::sync::LazyLock;

use bevy::math::{bounding::Aabb3d, bounding::IntersectsVolume, ops::atan2, IVec3, Quat, Vec3};
use rand::Rng;
use shared::{
    messages::PlayerId,
    players::{
        effects::EffectKind,
        game_mode::{DamageSource, SAFE_FALL_DISTANCE},
        Player,
    },
    world::{
        depths::strengthened,
        hostiles::{distance_to_box, is_prey, line_of_sight, Difficulty, Hunt, Strike},
        BlockHitbox, BlockId, BlockTransparency, ChunkMap, MobAction, MobKind, MobTarget, Route,
        ServerMob,
    },
};

use super::behavior::{flee, is_in_water, jump, may_move_to, rest, walk, walk_speed, wander};
use super::float::{climb_out, is_afloat, swim_speed};
use super::path::{cell_at, find_path, Cell, Pathing, Walker, MAX_DROP, MAX_NODES};

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
/// How high a Cinderling hovers, from the ground to the bottom of its box, in blocks.
pub const HOVER_HEIGHT: f32 = 2.0;
/// How many cells below itself a Cinderling looks for the ground it hovers over.
const HOVER_SCAN: i32 = 8;
/// How far a Cinderling keeps from its prey, horizontally, in blocks, give or take
/// `KEEP_SLACK`.
pub const KEEP_DISTANCE: f32 = 6.0;
const KEEP_SLACK: f32 = 0.5;
/// The ring a Gust Owl circles on: its radius, and its height above the prey's feet, in blocks.
pub const CIRCLE_RADIUS: f32 = 4.0;
pub const CIRCLE_HEIGHT: f32 = 8.0;
/// How high above its prey's feet a Gust Owl may start a dive, in blocks.
pub const DIVE_BAND: RangeInclusive<f32> = 6.0..=10.0;
/// How fast a Gust Owl dives, in blocks/s, and for how long at most, in ticks.
pub const DIVE_SPEED: f32 = 10.0;
pub const DIVE_TICKS: u32 = 40;
/// How often a walking hostile looks for its way again, in ticks, and how far its prey moves
/// before it looks at once, in blocks along an axis.
pub const REPLAN_TICKS: u32 = 20;
const REPLAN_MOVE: i32 = 2;
/// How near the centre of a cell of its way a hostile goes before it heads for the next one,
/// in blocks, and how far from it it gives the way up and looks for another.
const WAYPOINT_REACHED: f32 = 0.3;
const WAYPOINT_LOST: f32 = 3.0;
/// How near a closed door of its way a hostile that opens doors opens it, in blocks.
const DOOR_REACH: f32 = 1.6;

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

/// One step of the hunt of a hostile in a world this hard: the prey, then what its kind does. Returns what it threw,
/// if anything: a Cinderling's ember, which the caller adds to the mobs.
///
/// The arm of each kind is where the later hostiles plug in their own way of going at a prey.
pub fn act(
    mob: &mut ServerMob,
    chunks: &ChunkMap,
    players: &mut HashMap<PlayerId, Player>,
    difficulty: Difficulty,
    delta: f32,
    rng: &mut impl Rng,
    pathing: &mut Pathing,
    cats: &[Vec3],
) -> Option<ServerMob> {
    let (Some(hunt), Some(mut strike)) = (mob.kind.hunt(), mob.kind.strike()) else {
        return None;
    };
    strike.damage = difficulty.strike_damage(strengthened(strike.damage, mob.tier));
    if mob.action == MobAction::Burrow {
        burrow(mob);
        return None;
    }
    if shun_cats(mob, chunks, cats, hunt.speed, delta) {
        return None;
    }
    let prey = prey(mob, chunks, players, &hunt);
    if mob.kind == MobKind::Cinderling {
        return hover_and_throw(mob, chunks, players, prey, (hunt, strike), delta);
    }
    match mob.kind {
        MobKind::Roller => roll(mob, chunks, players, prey, (hunt, strike), delta),
        MobKind::Stumpling => {
            let hunting = (hunt, strike);
            stumpling(mob, chunks, players, prey, hunting, delta, rng, pathing)
        }
        MobKind::Drowner => swim(mob, chunks, players, prey, (hunt, strike), delta, rng),
        MobKind::GustOwl => circle_and_dive(mob, chunks, players, prey, (hunt, strike), delta),
        _ => melee(
            mob,
            chunks,
            players,
            prey,
            (hunt, strike),
            delta,
            rng,
            pathing,
        ),
    }
    None
}

/// A hostile that fears the cats (`companions::fears_cats`) keeps away from a tamed one: the
/// Gnawer runs from it on foot, the Gust Owl flies off from over it, and neither strikes
/// meanwhile. Returns whether it shunned one, which is all it did this step.
fn shun_cats(mob: &mut ServerMob, chunks: &ChunkMap, cats: &[Vec3], speed: f32, delta: f32) -> bool {
    if !super::companions::fears_cats(mob.kind) {
        return false;
    }
    let flies = mob.kind == MobKind::GustOwl;
    let Some(cat) = super::companions::nearest_cat(cats, mob.position, flies) else {
        if mob.action == MobAction::Flee {
            (mob.action, mob.action_ticks) = (MobAction::Idle, 0);
        }
        return false;
    };
    mob.route = None;
    mob.action = MobAction::Flee;
    mob.action_ticks = 0;
    if flies {
        let away = (mob.position - cat).with_y(0.0).normalize_or(Vec3::X);
        let to = mob.position + away * 6.0 + Vec3::Y * 2.0;
        fly_to(mob, chunks, to, speed.max(2.0), delta);
    } else {
        flee(mob, chunks, cat, speed, delta);
    }
    true
}

/// The height of the top of the ground under `mob`: the first cell with a hitbox or a liquid
/// at most `HOVER_SCAN` cells below the bottom of its box.
fn ground_below(chunks: &ChunkMap, mob: &ServerMob) -> Option<f32> {
    let bottom = (mob.position - Vec3::Y * (mob.height / 2.0))
        .floor()
        .as_ivec3();
    (0..=HOVER_SCAN)
        .map(|down| bottom - IVec3::Y * down)
        .find(|cell| {
            chunks.get_block(*cell).is_some_and(|block| {
                !matches!(block.get_hitbox(), BlockHitbox::None)
                    || block.id.get_visibility() == BlockTransparency::Liquid
            })
        })
        .map(|cell| cell.y as f32 + 1.0)
}

/// One step of a floating `mob` toward `target` at `speed`: straight, else only up or down,
/// else only horizontally, each step taken only if `may_move_to` lets it.
fn fly_to(mob: &mut ServerMob, chunks: &ChunkMap, target: Vec3, speed: f32, delta: f32) {
    let step = (target - mob.position).clamp_length_max(speed * delta);
    for step in [step, Vec3::Y * step.y, step.with_y(0.0)] {
        let moved = mob.position + step;
        if step != Vec3::ZERO && may_move_to(mob, chunks, moved) {
            mob.position = moved;
            return;
        }
    }
}

/// The Gust Owl: it floats (`behavior::floats`). With a prey it circles (`Chase`) round the
/// ring `CIRCLE_RADIUS` wide, `CIRCLE_HEIGHT` above the prey's feet, aiming at the point of the
/// ring its speed takes it to; once its cooldown is over, within `DIVE_BAND` above the prey's
/// feet and in sight, it dives (`Attack`) straight at the prey's centre at `DIVE_SPEED` for
/// `DIVE_TICKS` at most. A dive that reaches the prey strikes it, knocking it on along the dive;
/// a dive done, stopped by a block or run out goes back to circling, its cooldown restarted.
/// Without a prey it hovers where it is.
fn circle_and_dive(
    mob: &mut ServerMob,
    chunks: &ChunkMap,
    players: &mut HashMap<PlayerId, Player>,
    prey: Option<PlayerId>,
    (hunt, strike): (Hunt, Strike),
    delta: f32,
) {
    let Some(player) = prey.and_then(|id| players.get_mut(&id)) else {
        if mob.action != MobAction::Idle {
            (mob.action, mob.action_ticks) = (MobAction::Idle, 0);
        }
        return;
    };
    face(mob, player.position);
    let feet = player.position.y - player.height / 2.0;
    if mob.action == MobAction::Attack {
        let along = (player.position - mob.position).normalize_or_zero();
        let moved = mob.position + along * (DIVE_SPEED * delta);
        mob.action_ticks = mob.action_ticks.saturating_sub(1);
        let blocked = !may_move_to(mob, chunks, moved);
        if !blocked {
            mob.position = moved;
        }
        let reached = distance(mob, player) <= strike.reach;
        if reached {
            strike_player(mob, player, &strike, along);
        }
        if reached || blocked || mob.action_ticks == 0 {
            mob.attack_wait = strike.cooldown;
            (mob.action, mob.action_ticks) = (MobAction::Chase, 0);
        }
        return;
    }
    let dives = mob.attack_wait == 0
        && DIVE_BAND.contains(&(mob.position.y - feet))
        && line_of_sight(chunks, eyes(mob), player.eye_position());
    if dives {
        (mob.action, mob.action_ticks) = (MobAction::Attack, DIVE_TICKS);
        return;
    }
    mob.action = MobAction::Chase;
    let apart = (mob.position - player.position).with_y(0.0);
    let angle = atan2(apart.z, apart.x) + hunt.speed * delta / CIRCLE_RADIUS;
    let ring = player.position.with_y(feet + CIRCLE_HEIGHT)
        + Vec3::new(angle.cos(), 0.0, angle.sin()) * CIRCLE_RADIUS;
    fly_to(mob, chunks, ring, hunt.speed, delta);
}

/// The Cinderling: it floats (`behavior::floats`), its box `HOVER_HEIGHT` above the ground
/// (`ground_below`; its height kept over a deeper void). With a prey it goes to `KEEP_DISTANCE`
/// from it horizontally, and once its cooldown is over throws an ember at it
/// (`cinder::ember`) if it is within reach and in sight, which shows for `ATTACK_SHOW_TICKS`.
/// Without a prey it hovers where it is.
fn hover_and_throw(
    mob: &mut ServerMob,
    chunks: &ChunkMap,
    players: &HashMap<PlayerId, Player>,
    prey: Option<PlayerId>,
    (hunt, strike): (Hunt, Strike),
    delta: f32,
) -> Option<ServerMob> {
    let height = ground_below(chunks, mob).map_or(mob.position.y, |ground| {
        ground + HOVER_HEIGHT + mob.height / 2.0
    });
    let Some(player) = prey.and_then(|id| players.get(&id)) else {
        if mob.action != MobAction::Idle {
            (mob.action, mob.action_ticks) = (MobAction::Idle, 0);
        }
        fly_to(mob, chunks, mob.position.with_y(height), hunt.speed, delta);
        return None;
    };
    face(mob, player.position);
    let apart = (mob.position - player.position).with_y(0.0);
    let spot = if (apart.length() - KEEP_DISTANCE).abs() > KEEP_SLACK {
        player.position + apart.normalize_or(Vec3::X) * KEEP_DISTANCE
    } else {
        mob.position
    };
    fly_to(mob, chunks, spot.with_y(height), hunt.speed, delta);
    let throws = mob.attack_wait == 0
        && distance(mob, player) <= strike.reach
        && line_of_sight(chunks, eyes(mob), player.eye_position());
    if throws {
        mob.attack_wait = strike.cooldown;
        (mob.action, mob.action_ticks) = (MobAction::Attack, ATTACK_SHOW_TICKS);
        let ember = super::cinder::ember(mob.position, player.position);
        return Some(ember.in_tier(mob.tier));
    }
    if mob.action == MobAction::Attack && mob.action_ticks > 1 {
        mob.action_ticks -= 1;
    } else {
        (mob.action, mob.action_ticks) = (MobAction::Chase, 0);
    }
    None
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
    pathing: &mut Pathing,
) {
    if let Some(fire) = nearest_fire(chunks, mob.position) {
        mob.action = MobAction::Flee;
        mob.action_ticks = 0;
        return flee(mob, chunks, fire, hunt.speed, delta);
    }
    if mob.action == MobAction::Flee {
        (mob.action, mob.action_ticks) = (MobAction::Idle, 0);
    }
    melee(
        mob,
        chunks,
        players,
        prey,
        (hunt, strike),
        delta,
        rng,
        pathing,
    )
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
/// away from itself; a Drowner's grab drags it down (`Player::drag`). A shield raised toward
/// `mob` parries the blow, but a Roller's charge knocks it down. A blow that lands may leave its
/// effect (`inflicted_effect`).
fn strike_player(mob: &ServerMob, player: &mut Player, strike: &Strike, away: Vec3) {
    let knocks_down = mob.kind == MobKind::Roller;
    if player
        .parry(mob.position, strike.damage, knocks_down)
        .is_some()
    {
        // The shield takes the push, a little of it
        if strike.knock_back > 0.0 {
            player.knock_back(away, strike.knock_back * 0.3);
        }
        return;
    }
    // A rider's steed takes half the blow, its armour sparing it a part (`companions`)
    let damage = match player.riding.filter(|riding| riding.kind.is_steed()) {
        Some(_) => {
            let borne = strike.damage / 2 + strike.damage % 2;
            player.mount_blows += borne;
            strike.damage - borne
        }
        None => strike.damage,
    };
    let landed = damage > 0 && player.hurt(damage, DamageSource::Mob(mob.kind));
    if strike.knock_back > 0.0 {
        player.knock_back(away, strike.knock_back);
    }
    if mob.kind == MobKind::Drowner {
        player.drag();
    }
    if let (true, Some((kind, level, seconds))) = (landed, inflicted_effect(mob.kind)) {
        player.give_effect(kind, level, seconds);
    }
}

/// The status effect the blow of a hostile of `kind` leaves, its level and its seconds: the
/// Drowner's cold grip slows, the Bramble's thorns poison, the Gnawer's bite weakens, the
/// Slagcrawler's sets burning.
pub fn inflicted_effect(kind: MobKind) -> Option<(EffectKind, u8, u32)> {
    match kind {
        MobKind::Drowner => Some((EffectKind::Slowness, 2, 6)),
        MobKind::Bramble => Some((EffectKind::Poison, 1, 6)),
        MobKind::Gnawer => Some((EffectKind::Weakness, 1, 10)),
        MobKind::Slagcrawler => Some((EffectKind::Burn, 1, 4)),
        _ => None,
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

/// A hostile that strikes in reach: it walks to its prey at its hunting speed, if it walks,
/// along the way it finds (`chase`), strikes it when its centre is within reach of the prey's box and its cooldown is over, shows
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
    pathing: &mut Pathing,
) {
    let Some(player) = prey.and_then(|id| players.get_mut(&id)) else {
        mob.route = None;
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
    chase(mob, chunks, player, hunt.speed, delta, pathing);
}

/// Whether a hostile of `kind` opens the doors in its way, and walks through the open ones:
/// the Stumpling, which has arms. The Gnawer is too small, the Drowner keeps to the water.
pub fn opens_doors(kind: MobKind) -> bool {
    kind == MobKind::Stumpling
}

/// How `mob` walks (`path::Walker`): as many cells tall as its box, and dropping down no more
/// than a fall that would leave a player of its health alive, from `SAFE_FALL_DISTANCE` when
/// nearly dead to `MAX_DROP`.
fn walker(mob: &ServerMob) -> Walker {
    let health = mob.max_health().saturating_sub(mob.damage).max(1) as i32;
    let safe = SAFE_FALL_DISTANCE as i32;
    Walker {
        clearance: (mob.height - 0.01).ceil().max(1.0) as i32,
        max_drop: (safe + health - 1).clamp(safe, MAX_DROP),
        opens_doors: opens_doors(mob.kind),
    }
}

/// The cell of the feet of a box `height` tall centred on `position`.
fn feet_cell(position: Vec3, height: f32) -> IVec3 {
    (position - Vec3::Y * (height / 2.0 - 0.05))
        .floor()
        .as_ivec3()
}

/// The middle of the floor of `cell`.
fn floor_centre(cell: IVec3) -> Vec3 {
    cell.as_vec3() + Vec3::new(0.5, 0.0, 0.5)
}

/// One step of `mob` on foot toward `player` at `speed`, along its way (`Route`). It looks for
/// its way again (`path::find_path`, within what is left of the tick's budget) while it stands
/// on the ground and it has none, every `REPLAN_TICKS`, once its prey moved more than
/// `REPLAN_MOVE` blocks from the goal of its way, or once it strayed `WAYPOINT_LOST` from it.
/// It heads for the centre of the next cell of its way, jumps when that cell is a step higher
/// and the step is in its way, and opens the closed doors of it within `DOOR_REACH` if it opens
/// doors. Without a way, or at its end, it walks straight at its prey.
fn chase(
    mob: &mut ServerMob,
    chunks: &ChunkMap,
    player: &Player,
    speed: f32,
    delta: f32,
    pathing: &mut Pathing,
) {
    chase_to(mob, chunks, (player.position, player.height), speed, delta, pathing);
}

/// `chase`, toward the box centred on `target.0`, `target.1` tall: a player, or the prey or
/// the owner of a tamed animal (`companions`).
pub(super) fn chase_to(
    mob: &mut ServerMob,
    chunks: &ChunkMap,
    (target, height): (Vec3, f32),
    speed: f32,
    delta: f32,
    pathing: &mut Pathing,
) {
    let goal = feet_cell(target, height);
    let start = feet_cell(mob.position, mob.height);
    let lost = mob.route.as_ref().is_some_and(|route| {
        route.cells.get(route.next).is_some_and(|cell| {
            (floor_centre(*cell) - mob.position).with_y(0.0).length() > WAYPOINT_LOST
        })
    });
    let stale = mob.route.as_ref().is_none_or(|route| {
        route.wait == 0 || lost || (route.goal - goal).abs().max_element() > REPLAN_MOVE
    });
    if let Some(route) = &mut mob.route {
        route.wait = route.wait.saturating_sub(1);
    }
    if stale && mob.on_ground && pathing.nodes_left > 0 {
        let nodes = MAX_NODES.min(pathing.nodes_left);
        let (found, expanded) =
            find_path(start, goal, walker(mob), nodes, |pos| cell_at(chunks, pos));
        pathing.nodes_left -= expanded.min(pathing.nodes_left);
        let (cells, reaches) =
            found.map_or((Vec::new(), false), |found| (found.cells, found.reaches));
        mob.route = Some(Route {
            cells,
            next: 0,
            goal,
            reaches,
            wait: REPLAN_TICKS,
        });
    }
    let mut waypoint = None;
    if let Some(route) = &mut mob.route {
        while let Some(cell) = route.cells.get(route.next) {
            if (floor_centre(*cell) - mob.position).with_y(0.0).length() > WAYPOINT_REACHED {
                waypoint = Some(*cell);
                break;
            }
            route.next += 1;
        }
    }
    let Some(cell) = waypoint else {
        let towards = (target - mob.position)
            .with_y(0.0)
            .normalize_or_zero();
        return walk(mob, chunks, towards, speed, delta);
    };
    let towards = (floor_centre(cell) - mob.position)
        .with_y(0.0)
        .normalize_or_zero();
    if opens_doors(mob.kind) {
        let clearance = walker(mob).clearance;
        for up in 0..clearance {
            let door = cell + IVec3::Y * up;
            let near = (floor_centre(door) - mob.position).with_y(0.0).length() <= DOOR_REACH;
            if near && cell_at(chunks, door) == (Cell::Door { open: false }) {
                pathing.doors.push(door);
            }
        }
    }
    let blocked = !may_move_to(
        mob,
        chunks,
        mob.position + towards * (speed * delta).max(0.1),
    );
    if cell.y > start.y && mob.on_ground && blocked {
        jump(mob);
        mob.rotation = Quat::from_rotation_y(atan2(towards.x, towards.z));
        return;
    }
    walk(mob, chunks, towards, speed, delta);
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
            let afloat = is_afloat(mob, chunks);
            let step = direction * (swim_speed(afloat, hunt.speed) * delta);
            let moved = mob.position + step;
            if !may_move_to(mob, chunks, moved) {
                if afloat && climb_out(mob, chunks, step) {
                    return;
                }
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
    fn a_gnawer_finds_its_way_round_a_wall_to_its_prey() {
        let mut w = with(
            mob_at(MobKind::Gnawer, 0.5, 0.5),
            vec![player_at(1, 6.5, 0.5)],
        );
        w.resource_mut::<ServerWorldMap>().chunks.fill_box(
            IVec3::new(3, 1, -4),
            IVec3::new(3, 2, 4),
            BlockId::Stone,
        );
        w.resource_mut::<ServerWorldMap>()
            .mobs
            .get_mut(&0)
            .unwrap()
            .target = MobTarget::Player(1);
        let full = player(&w, 1).health;
        run(&mut w, 200);
        assert!(player(&w, 1).health < full, "{:?}", mob(&w).position);
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

    /// The embers in `w`.
    fn embers(w: &World) -> Vec<ServerMob> {
        w.resource::<ServerWorldMap>()
            .mobs
            .values()
            .filter(|mob| mob.kind == MobKind::Ember)
            .cloned()
            .collect()
    }

    /// How high the bottom of the box of `mob` is above the ground of `ground` (y = 1).
    fn hover(mob: &ServerMob) -> f32 {
        mob.position.y - mob.height / 2.0 - 1.0
    }

    #[test]
    fn a_cinderling_hovers_two_blocks_above_the_ground_and_keeps_six_blocks_from_its_prey() {
        // Too close: it backs away and rises
        let mut w = with(
            mob_at(MobKind::Cinderling, 0.5, 0.5),
            vec![player_at(1, 3.5, 0.5)],
        );
        run(&mut w, 100);
        assert!(
            (hover(mob(&w)) - HOVER_HEIGHT).abs() < 0.05,
            "{}",
            hover(mob(&w))
        );
        let apart = (mob(&w).position - player(&w, 1).position)
            .with_y(0.0)
            .length();
        assert!((5.4..=6.6).contains(&apart), "{apart}");
        // Too far: it comes closer, at 2 blocks a second
        let mut w = with(
            mob_at(MobKind::Cinderling, 0.5, 0.5),
            vec![player_at(1, 14.5, 0.5)],
        );
        run(&mut w, 20);
        assert!(mob(&w).position.x > 1.5, "{}", mob(&w).position);
        assert!(mob(&w).position.x < 2.6, "{}", mob(&w).position);
        run(&mut w, 100);
        let apart = (mob(&w).position - player(&w, 1).position)
            .with_y(0.0)
            .length();
        assert!((5.4..=6.6).contains(&apart), "{apart}");
        // Without a prey, it hovers where it is
        let mut w = with(mob_at(MobKind::Cinderling, 0.5, 0.5), vec![]);
        run(&mut w, 60);
        assert!((hover(mob(&w)) - HOVER_HEIGHT).abs() < 0.05);
        assert_eq!(mob(&w).position.x, 0.5);
        assert_eq!(mob(&w).action, MobAction::Idle);
    }

    #[test]
    fn a_cinderling_throws_an_ember_at_its_prey_every_sixty_ticks_and_shows_it() {
        let mut w = with(
            mob_at(MobKind::Cinderling, 0.5, 0.5),
            vec![player_at(1, 6.5, 0.5)],
        );
        run(&mut w, 1);
        assert_eq!(embers(&w).len(), 1);
        assert_eq!(mob(&w).action, MobAction::Attack);
        let ember = &embers(&w)[0];
        assert!(
            ember.velocity.x > 0.0,
            "toward its prey: {}",
            ember.velocity
        );
        assert!(ember.velocity.y > 0.0, "in an arc: {}", ember.velocity);
        run(&mut w, 59);
        assert_eq!(embers(&w).len(), 1);
        run(&mut w, 1);
        assert_eq!(embers(&w).len(), 2);
        // Not at a creative player
        let mut creative = player_at(1, 6.5, 0.5);
        creative.game_mode = GameMode::Creative;
        let mut w = with(mob_at(MobKind::Cinderling, 0.5, 0.5), vec![creative]);
        run(&mut w, 30);
        assert!(embers(&w).is_empty());
    }

    #[test]
    fn a_cinderling_throws_nothing_through_a_wall() {
        let mut w = with(
            mob_at(MobKind::Cinderling, 0.5, 0.5),
            vec![player_at(1, 6.5, 0.5)],
        );
        // Its prey seen first, then a wall rises between them
        run(&mut w, 1);
        w.resource_mut::<ServerWorldMap>().chunks.fill_box(
            IVec3::new(3, 1, -5),
            IVec3::new(3, 8, 5),
            BlockId::Stone,
        );
        run(&mut w, 80);
        assert_eq!(embers(&w).len(), 1);
    }

    /// A Gust Owl at `at`, its dive ready or not.
    fn owl_at(at: Vec3, ready: bool) -> ServerMob {
        ServerMob {
            attack_wait: if ready { 0 } else { 10_000 },
            ..ServerMob::new(MobKind::GustOwl, at)
        }
    }

    #[test]
    fn a_gust_owl_climbs_over_its_prey_and_circles_six_to_ten_blocks_above_it() {
        let mut w = with(
            owl_at(Vec3::new(0.5, 1.8, 0.5), false),
            vec![player_at(1, 4.5, 0.5)],
        );
        run(&mut w, 100);
        let feet = player(&w, 1).position.y - player(&w, 1).height / 2.0;
        let above = |w: &World| mob(w).position.y - feet;
        let around = |w: &World| (mob(w).position - player(w, 1).position).with_y(0.0);
        assert!((6.0..=10.0).contains(&above(&w)), "{}", above(&w));
        assert!((3.0..=5.0).contains(&around(&w).length()), "{}", around(&w));
        assert_eq!(mob(&w).action, MobAction::Chase);
        let before = around(&w);
        run(&mut w, 20);
        assert!((3.0..=5.0).contains(&around(&w).length()), "{}", around(&w));
        assert!(before.angle_between(around(&w)) > 0.5, "it circles");
        assert!((6.0..=10.0).contains(&above(&w)));
        // Without a prey it hovers where it is
        let mut w = with(owl_at(Vec3::new(0.5, 9.0, 0.5), false), vec![]);
        run(&mut w, 20);
        assert_eq!(mob(&w).position, Vec3::new(0.5, 9.0, 0.5));
        assert_eq!(mob(&w).action, MobAction::Idle);
    }

    #[test]
    fn a_gust_owl_dives_at_its_prey_for_two_and_knocks_it_back_at_eight() {
        let mut w = with(
            owl_at(Vec3::new(4.5, 10.0, 0.5), true),
            vec![player_at(1, 0.5, 0.5)],
        );
        run(&mut w, 1);
        assert_eq!(mob(&w).action, MobAction::Attack);
        let mut ticks = 1;
        while player(&w, 1).health == MAX_HEALTH && ticks < 40 {
            run(&mut w, 1);
            ticks += 1;
        }
        assert_eq!(player(&w, 1).health, MAX_HEALTH - 2, "after {ticks} ticks");
        assert!(ticks < 25, "{ticks} ticks at 10 blocks a second");
        let push = player(&w, 1).velocity.with_y(0.0);
        assert!((push.length() - 8.0).abs() < 1e-4, "{push}");
        assert!(push.x < 0.0, "away from the owl: {push}");
        // It climbs back to circle, and dives again only after its cooldown
        assert_eq!(mob(&w).action, MobAction::Chase);
        let low = mob(&w).position.y;
        run(&mut w, 20);
        assert!(mob(&w).position.y > low + 2.0);
        assert_eq!(player(&w, 1).health, MAX_HEALTH - 2);
        // A fall soon after is the owl's doing
        w.resource_mut::<ServerWorldMap>()
            .players
            .get_mut(&1)
            .unwrap()
            .hurt(MAX_HEALTH, DamageSource::Fall);
        assert_eq!(
            player(&w, 1).death_cause,
            Some(DeathCause::ThrownOff(MobKind::GustOwl))
        );
        assert_eq!(
            DeathCause::ThrownOff(MobKind::GustOwl).message(),
            "A Gust Owl threw you off a ledge"
        );
    }

    #[test]
    fn a_dive_a_block_stops_climbs_back_without_a_strike() {
        let mut w = with(
            owl_at(Vec3::new(4.5, 10.0, 0.5), true),
            vec![player_at(1, 0.5, 0.5)],
        );
        run(&mut w, 1);
        assert_eq!(mob(&w).action, MobAction::Attack);
        // A roof rises over the player
        w.resource_mut::<ServerWorldMap>().chunks.fill_box(
            IVec3::new(-3, 5, -3),
            IVec3::new(3, 5, 3),
            BlockId::Glass,
        );
        run(&mut w, 59);
        assert_eq!(player(&w, 1).health, MAX_HEALTH);
        assert_ne!(mob(&w).action, MobAction::Attack);
        assert!(mob(&w).position.y > 6.0, "{}", mob(&w).position);
    }
}
