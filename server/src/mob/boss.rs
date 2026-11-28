//! The Ember Mother, the boss: she sleeps on the island of her lair (`shared::world::lair`), and
//! wakes when a player that hostiles may hurt comes in. Over half her health she keeps away,
//! throws embers and calls Cinderlings, then rests a moment within reach; under it she goes at
//! the players, strikes them up close and sets the ground around her on fire. Once nobody is left
//! inside she falls asleep again, healed. Beaten, she never comes back (`Boss::defeated`, saved).
//!
//! She is a hostile that `behavior`, the spawns, the pushes and the save leave alone: this system
//! moves her, and puts her back asleep whenever her lair is loaded. Asleep, as in a peaceful
//! world, where she never wakes, nothing hurts her.

use std::collections::{HashMap, HashSet, VecDeque};
use std::f32::consts::TAU;

use bevy::math::{ops::atan2, IVec3, Quat, Vec3};
use bevy::time::{Fixed, Time};
use bevy_ecs::system::{Res, ResMut};
use bevy_renet::RenetServer;
use rand::Rng;
use shared::messages::{BossBar, PlayerId, ServerToClientMessage};
use shared::players::{game_mode::DamageSource, movement::is_terrain_loaded, Player};
use shared::world::depths::DepthTier;
use shared::world::hostiles::{distance_to_box, is_prey};
use shared::world::lair::{heart, holds, HEALTH_PER_PLAYER, LAKE, MOST_CALLED, REACH};
use shared::world::{
    global_block_to_chunk_pos, BlockData, BlockDirection, BlockHitbox, BlockId, ChunkMap,
    MobAction, MobId, MobKind, MobTarget, ServerMob,
};
use shared::CHUNK_SIZE;

use super::cinder::ember;
use super::combat::{drop_loot, loot};
use super::hostile::ATTACK_SHOW_TICKS;
use crate::network::extensions::SendGameMessageExtension;
use crate::world::data::ServerWorldMap;
use crate::world::simulation::LoadedGenerator;

/// Ticks without anyone inside before she falls asleep: 5 s.
const EMPTY_TICKS: u32 = 100;
/// How far her bar shows around the lair, in blocks.
const BAR_MARGIN: f32 = 6.0;
/// How far from the middle of the lair she goes, and how near in the first phase, whose ground is
/// out of the lake.
const ARENA: f32 = 24.0;
const SHORE: f32 = LAKE as f32 + 4.0;
/// How high she hovers, and how high over the ground she follows a player.
const HOVER: f32 = 0.5;
const CLIMB: f32 = 10.0;
/// The distance she keeps in the first phase, and the length of its cycle in ticks: she throws a
/// volley every `VOLLEY` ticks, then rests from `REST` on.
const KEEP: f32 = 9.0;
const CYCLE: u32 = 300;
const REST: u32 = 180;
const VOLLEY: u32 = 45;
const EMBERS: usize = 3;
/// How far off its mark an ember of a volley falls.
const SPREAD: f32 = 1.5;
/// Every this many ticks she calls a Cinderling, up to two and one more per player but the first
/// (`lair::MOST_CALLED`).
const CALL: u32 = 160;
/// In the second phase: how fast she goes, every how many ticks she lights the ground, how many
/// cells, how far around her, and how many of her fires burn at most.
const FURY_SPEED: f32 = 3.8;
const BLAZE: u32 = 50;
const BLAZE_CELLS: usize = 10;
const BLAZE_RING: std::ops::Range<f32> = 1.5..5.0;
const MOST_FIRES: usize = 40;
/// She throws embers at a player farther than this, in the second phase.
const FAR: f32 = 6.0;

/// The state of the Ember Mother in a world. Only `defeated` is saved: after a load she sleeps.
#[derive(Debug, Default)]
pub struct Boss {
    pub defeated: bool,
    /// The middle block of the ground of her lair, once known.
    lair: Option<IVec3>,
    id: Option<MobId>,
    awake: bool,
    /// Most players inside at once since she woke.
    players: u32,
    /// Ticks since someone was inside.
    empty: u32,
    /// Ticks since she woke.
    clock: u32,
    fires: VecDeque<IVec3>,
    /// The chunks of the lake whose lava turned to obsidian since she was beaten, or the load.
    cooled: HashSet<IVec3>,
    /// The bar each player was last told of.
    told: HashMap<PlayerId, BossBar>,
}

impl Boss {
    /// The boss of a world loaded, beaten or asleep.
    pub fn saved(defeated: bool) -> Self {
        Self {
            defeated,
            ..Self::default()
        }
    }

    /// Whether `position` is in the lair of the world.
    pub fn holds(&self, position: Vec3) -> bool {
        self.lair.is_some_and(|centre| holds(centre, position))
    }
}

/// One tick of the boss. Runs after the blows and the arrows, so that a save of the tick that
/// beat her keeps it.
pub fn boss_system(
    mut world_map: ResMut<ServerWorldMap>,
    generator: Option<Res<LoadedGenerator>>,
    mut server: ResMut<RenetServer>,
    time: Res<Time<Fixed>>,
) {
    let Some(lair) = generator.as_ref().and_then(|made| made.0.lair()) else {
        return;
    };
    let map = &mut *world_map;
    for (id, message) in step(map, lair.centre, time.delta_secs(), &mut rand::thread_rng()) {
        server.send_game_message(id, message);
    }
}

/// One tick of the boss of the lair around `centre`. Returns what to tell the players.
fn step(
    map: &mut ServerWorldMap,
    centre: IVec3,
    delta: f32,
    rng: &mut impl Rng,
) -> Vec<(PlayerId, ServerToClientMessage)> {
    map.boss.lair = Some(centre);
    if map.boss.defeated {
        cool_lake(map, centre);
        return Vec::new();
    }
    let mut messages = Vec::new();
    match map.boss.id {
        Some(id) if !map.mobs.contains_key(&id) => {
            // Gone any other way than beaten: she sleeps anew
            fall_asleep(map, centre, id);
            map.boss.id = None;
        }
        Some(id) if map.boss.awake && is_slain(&map.mobs[&id]) => {
            beaten(map, centre, id, rng);
            for player in map.players.values_mut() {
                if holds(centre, player.position) {
                    player.beat(MobKind::EmberMother);
                    messages.push((player.id, ServerToClientMessage::Victory));
                }
            }
        }
        Some(id) => fight(map, centre, id, delta, rng),
        None if is_terrain_loaded(heart(centre), &map.chunks) => {
            let mother = ServerMob::new(MobKind::EmberMother, bed(centre));
            let id = super::add_mob(&mut map.mobs, &mut map.changed_mobs, mother);
            map.boss.id = Some(id);
            fall_asleep(map, centre, id);
        }
        None => {}
    }
    tell(map, centre, &mut messages);
    messages
}

/// Where she sleeps.
fn bed(centre: IVec3) -> Vec3 {
    heart(centre) + Vec3::Y * (MobKind::EmberMother.size().y / 2.0)
}

/// She is healed, on her island; her Cinderlings and their embers, and her fires, go.
fn fall_asleep(map: &mut ServerWorldMap, centre: IVec3, id: MobId) {
    clear(map, centre);
    let boss = &mut map.boss;
    (boss.awake, boss.players, boss.empty, boss.clock) = (false, 0, 0, 0);
    if let Some(mother) = map.mobs.get_mut(&id) {
        // At her bed at once, as through a portal
        *mother = ServerMob {
            passes: mother.passes.wrapping_add(1),
            // The blows of the next tick land before its `fight`
            hurt_ticks: super::combat::MOB_INVULNERABILITY_TICKS,
            ..ServerMob::new(MobKind::EmberMother, bed(centre))
        };
        map.changed_mobs.insert(id);
    }
}

/// Removes what she called, threw and lit.
fn clear(map: &mut ServerWorldMap, centre: IVec3) {
    map.mobs.retain(|_, mob| {
        !(matches!(mob.kind, MobKind::Cinderling | MobKind::Ember) && holds(centre, mob.position))
    });
    for cell in std::mem::take(&mut map.boss.fires) {
        put_out(&mut map.chunks, cell);
    }
}

fn put_out(chunks: &mut ChunkMap, cell: IVec3) {
    if chunks
        .get_block(cell)
        .is_some_and(|b| b.id == BlockId::Fire)
    {
        chunks.remove_block(cell);
    }
}

/// Whether the blows or the arrows took all her health (`combat::wound`, which leaves her there).
fn is_slain(mother: &ServerMob) -> bool {
    mother.damage >= mother.max_health()
}

/// She is beaten for good: she leaves her loot, and the lava of her lair turns to obsidian.
fn beaten(map: &mut ServerWorldMap, centre: IVec3, id: MobId, rng: &mut impl Rng) {
    map.boss.defeated = true;
    // The clients are told by the next broadcast, which no longer finds her
    map.changed_mobs.remove(&id);
    if let Some(mother) = map.mobs.remove(&id) {
        let drops = loot(&mother, rng);
        drop_loot(&mut map.dropped_items, drops, mother.position);
        map.dropped_items
            .drop_orb(mother.experience(), mother.position, rng);
    }
    clear(map, centre);
    for x in -REACH..=REACH {
        for z in -REACH..=REACH {
            for y in -6..=1 {
                let cell = centre + IVec3::new(x, y, z);
                let is_lava = map
                    .chunks
                    .get_block(cell)
                    .is_some_and(|b| b.id == BlockId::Lava);
                if is_lava && holds(centre, cell.as_vec3() + 0.5) {
                    let obsidian = BlockData::new(BlockId::Obsidian, BlockDirection::Front);
                    map.chunks.set_block(cell, obsidian);
                }
            }
        }
    }
}

/// Turns to obsidian the lava of the lake in the chunks that were not loaded as she was beaten,
/// once each is.
fn cool_lake(map: &mut ServerWorldMap, centre: IVec3) {
    // The shore wavers
    let reach = IVec3::new(LAKE + 2, 0, LAKE + 2);
    let (low, high) = (centre - reach - IVec3::Y * 6, centre + reach);
    let (first, last) = (
        global_block_to_chunk_pos(&low),
        global_block_to_chunk_pos(&high),
    );
    for x in first.x..=last.x {
        for y in first.y..=last.y {
            for z in first.z..=last.z {
                let chunk = IVec3::new(x, y, z);
                if map.chunks.is_chunk_loaded(chunk) && map.boss.cooled.insert(chunk) {
                    let origin = chunk * CHUNK_SIZE;
                    turn_to_obsidian(
                        &mut map.chunks,
                        low.max(origin),
                        high.min(origin + CHUNK_SIZE - 1),
                    );
                }
            }
        }
    }
}

/// Turns the lava from `low` to `high`, included, to obsidian.
fn turn_to_obsidian(chunks: &mut ChunkMap, low: IVec3, high: IVec3) {
    for x in low.x..=high.x {
        for y in low.y..=high.y {
            for z in low.z..=high.z {
                let cell = IVec3::new(x, y, z);
                if chunks
                    .get_block(cell)
                    .is_some_and(|b| b.id == BlockId::Lava)
                {
                    let obsidian = BlockData::new(BlockId::Obsidian, BlockDirection::Front);
                    chunks.set_block(cell, obsidian);
                }
            }
        }
    }
}

/// Tells each player near what its bar shows, when that changed.
fn tell(
    map: &mut ServerWorldMap,
    centre: IVec3,
    messages: &mut Vec<(PlayerId, ServerToClientMessage)>,
) {
    let Boss {
        awake, id, told, ..
    } = &mut map.boss;
    let mother = id.filter(|_| *awake).and_then(|id| map.mobs.get(&id));
    let bar = mother.map(|mother| BossBar {
        kind: mother.kind,
        health: mother.max_health().saturating_sub(mother.damage),
        max_health: mother.max_health(),
    });
    told.retain(|id, _| map.players.contains_key(id));
    for player in map.players.values() {
        let towards = heart(centre) - player.position;
        let nearer = player.position + towards.clamp_length_max(BAR_MARGIN);
        let shown = bar.filter(|_| holds(centre, nearer));
        if told.get(&player.id).copied() != shown {
            messages.push((player.id, ServerToClientMessage::BossBar(shown)));
            match shown {
                Some(bar) => told.insert(player.id, bar),
                None => told.remove(&player.id),
            };
        }
    }
}

/// One tick of her, alive.
fn fight(map: &mut ServerWorldMap, centre: IVec3, id: MobId, delta: f32, rng: &mut impl Rng) {
    let inside = map
        .players
        .values()
        .filter(|player| is_prey(player) && holds(centre, player.position));
    let inside: Vec<PlayerId> = inside.map(|player| player.id).collect();
    let peaceful = !map.difficulty.has_hostiles();
    if !map.boss.awake {
        if inside.is_empty() || peaceful {
            if let Some(mother) = map.mobs.get_mut(&id) {
                // Nothing hurts her asleep
                mother.hurt_ticks = super::combat::MOB_INVULNERABILITY_TICKS;
            }
            return;
        }
        map.boss.awake = true;
    }
    let boss = &mut map.boss;
    boss.clock += 1;
    boss.empty = if inside.is_empty() { boss.empty + 1 } else { 0 };
    boss.players = boss.players.max(inside.len() as u32);
    if peaceful || boss.empty > EMPTY_TICKS {
        return fall_asleep(map, centre, id);
    }
    let Some(mut mother) = map.mobs.get(&id).cloned() else {
        return;
    };
    let before = (mother.position, mother.rotation, mother.action);
    mother.extra_health = HEALTH_PER_PLAYER * map.boss.players.saturating_sub(1);
    mother.hurt_ticks = mother.hurt_ticks.saturating_sub(1);
    mother.attack_wait = mother.attack_wait.saturating_sub(1);
    mother.action_ticks = mother.action_ticks.saturating_sub(1);
    if mother.action_ticks == 0 {
        mother.action = MobAction::Chase;
    }
    let nearest = inside.iter().filter_map(|id| map.players.get(id));
    let nearest = nearest.min_by(|a, b| {
        let apart = |p: &Player| p.position.distance_squared(mother.position);
        apart(a).total_cmp(&apart(b))
    });
    if let Some(prey) = nearest.cloned() {
        mother.target = MobTarget::Player(prey.id);
        let towards = (prey.position - mother.position).with_y(0.0);
        if towards.length_squared() > 1e-4 {
            mother.rotation = Quat::from_rotation_y(atan2(towards.x, towards.z));
        }
        if in_fury(&mother) {
            fury(map, centre, &mut mother, &prey, delta, rng);
        } else {
            siege(map, centre, &mut mother, &prey, delta, rng);
        }
        melt(&mut map.chunks, &mother, centre.y);
    }
    if before != (mother.position, mother.rotation, mother.action) {
        map.changed_mobs.insert(id);
    }
    map.mobs.insert(id, mother);
}

/// Whether she is in her second phase: under half her health.
fn in_fury(mother: &ServerMob) -> bool {
    mother.damage * 2 > mother.max_health()
}

/// The first phase: she keeps away on the shore, throws volleys of embers and calls Cinderlings,
/// then rests on the ground.
fn siege(
    map: &mut ServerWorldMap,
    centre: IVec3,
    mother: &mut ServerMob,
    prey: &Player,
    delta: f32,
    rng: &mut impl Rng,
) {
    let clock = map.boss.clock;
    let ground = heart(centre).y + mother.height / 2.0;
    if clock % CYCLE >= REST {
        go(mother, mother.position.with_y(ground), 2.0, delta);
        if mother.action == MobAction::Chase {
            mother.action = MobAction::Idle;
        }
    } else {
        let spot = away(centre, mother.position, prey.position).with_y(ground + HOVER);
        go(mother, spot, own_speed(), delta);
        if clock.is_multiple_of(VOLLEY) {
            throw(map, mother, prey, rng);
        }
    }
    if clock % CALL == CALL / 2 {
        call(map, centre, mother, prey, rng);
    }
}

/// Where she keeps away from a player at `prey`, from `at`: out of the lake, across from it, or
/// along the shore when it stands there.
fn away(centre: IVec3, at: Vec3, prey: Vec3) -> Vec3 {
    let off = (at - prey).with_y(0.0).normalize_or(Vec3::X) * KEEP;
    let across = within(centre, prey + off, SHORE);
    if (across - prey).with_y(0.0).length() > KEEP - 2.0 {
        return across;
    }
    let radius = (SHORE + ARENA) / 2.0;
    let from = (prey - heart(centre)).with_y(0.0).normalize_or(Vec3::X);
    let side = if from.cross(at - heart(centre)).y >= 0.0 {
        1.0
    } else {
        -1.0
    };
    let turn = Quat::from_rotation_y(side * KEEP / radius);
    heart(centre) + turn * from * radius
}

fn own_speed() -> f32 {
    super::hostile::own_speed(MobKind::EmberMother)
}

/// The second phase: she goes at her prey, strikes it in reach, sets the ground around her on
/// fire, and throws embers at who is far.
fn fury(
    map: &mut ServerWorldMap,
    centre: IVec3,
    mother: &mut ServerMob,
    prey: &Player,
    delta: f32,
    rng: &mut impl Rng,
) {
    let clock = map.boss.clock;
    let Some(strike) = mother.kind.strike() else {
        return;
    };
    let half = Vec3::new(prey.width, prey.height, prey.width) / 2.0;
    let apart = distance_to_box(mother.position, prey.position, half);
    // She stands a moment after a blow
    if mother.action != MobAction::Attack && apart > strike.reach * 0.8 {
        let ground = heart(centre).y + mother.height / 2.0 + HOVER;
        let feet = prey.position.y - prey.height / 2.0 + mother.height / 2.0;
        let spot = within(centre, prey.position, 0.0);
        go(
            mother,
            spot.with_y(feet.clamp(ground, ground + CLIMB)),
            FURY_SPEED,
            delta,
        );
    }
    if apart <= strike.reach && mother.attack_wait == 0 {
        mother.attack_wait = strike.cooldown;
        (mother.action, mother.action_ticks) = (MobAction::Attack, ATTACK_SHOW_TICKS);
        let damage = map.difficulty.strike_damage(strike.damage);
        if let Some(player) = map.players.get_mut(&prey.id) {
            // Her blow knocks a raised shield down
            if player.parry(mother.position, damage, true).is_none() {
                player.hurt(damage, DamageSource::Mob(mother.kind));
            }
            player.knock_back(player.position - mother.position, strike.knock_back);
        }
    }
    if clock.is_multiple_of(BLAZE) {
        blaze(map, centre, mother, rng);
    }
    if apart > FAR && clock.is_multiple_of(VOLLEY) {
        throw(map, mother, prey, rng);
    }
}

/// One step of her toward `spot` at `speed`, through what stands there (`melt`).
fn go(mother: &mut ServerMob, spot: Vec3, speed: f32, delta: f32) {
    mother.position += (spot - mother.position).clamp_length_max(speed * delta);
}

/// The nearest point to `spot` of the ring of her arena, at least `inner` from its middle.
fn within(centre: IVec3, spot: Vec3, inner: f32) -> Vec3 {
    let from = (spot - heart(centre)).with_y(0.0);
    let length = from.length().clamp(inner, ARENA);
    heart(centre) + from.normalize_or(Vec3::X) * length
}

/// She burns away what stands in her way above the ground of her lair at `floor`, but what
/// holds something: nobody hides behind blocks.
fn melt(chunks: &mut ChunkMap, mother: &ServerMob, floor: i32) {
    let half = Vec3::new(mother.width, mother.height, mother.depth) / 2.0;
    let low = (mother.position - half).floor().as_ivec3();
    let high = (mother.position + half).floor().as_ivec3();
    for x in low.x..=high.x {
        for y in low.y.max(floor + 1)..=high.y {
            for z in low.z..=high.z {
                let cell = IVec3::new(x, y, z);
                let melts = chunks.get_block(cell).is_some_and(|block| {
                    !block.id.is_container()
                        && !matches!(
                            block.id,
                            BlockId::Furnace
                                | BlockId::LitFurnace
                                | BlockId::Alembic
                                | BlockId::Bedrock
                        )
                });
                if melts {
                    chunks.remove_block(cell);
                }
            }
        }
    }
}

/// She throws a volley of embers around her prey.
fn throw(map: &mut ServerWorldMap, mother: &mut ServerMob, prey: &Player, rng: &mut impl Rng) {
    (mother.action, mother.action_ticks) = (MobAction::Attack, ATTACK_SHOW_TICKS);
    let from = mother.position + Vec3::Y * mother.height / 3.0;
    for i in 0..EMBERS {
        // The first at it, the others beside
        let off = if i == 0 { 0.0 } else { SPREAD };
        let aim = prey.position
            + Vec3::new(rng.gen_range(-1.0..=1.0), 0.0, rng.gen_range(-1.0..=1.0)) * off;
        let thrown = ember(from, aim).in_tier(Some(DepthTier::Furnace));
        super::add_mob(&mut map.mobs, &mut map.changed_mobs, thrown);
    }
}

/// She calls a Cinderling out of the lake, unless enough are there.
fn call(
    map: &mut ServerWorldMap,
    centre: IVec3,
    mother: &ServerMob,
    prey: &Player,
    rng: &mut impl Rng,
) {
    let there = map.mobs.values();
    let there = there.filter(|m| m.kind == MobKind::Cinderling && holds(centre, m.position));
    let most = (1 + map.boss.players as usize).min(MOST_CALLED);
    if there.count() >= most {
        return;
    }
    let angle = rng.gen_range(0.0..TAU);
    let off = Vec3::new(angle.cos(), 0.0, angle.sin()) * (LAKE as f32 - 4.0);
    let at = heart(centre) + off + Vec3::Y * 2.0;
    let child = ServerMob {
        target: MobTarget::Player(prey.id),
        rotation: mother.rotation,
        ..ServerMob::new(MobKind::Cinderling, at)
    };
    super::add_mob(&mut map.mobs, &mut map.changed_mobs, child);
}

/// She lights cells of the ground around her; the oldest of her fires go out.
fn blaze(map: &mut ServerWorldMap, centre: IVec3, mother: &ServerMob, rng: &mut impl Rng) {
    for _ in 0..BLAZE_CELLS {
        let angle = rng.gen_range(0.0..TAU);
        let off = Vec3::new(angle.cos(), 0.0, angle.sin()) * rng.gen_range(BLAZE_RING);
        let column = (mother.position + off).floor().as_ivec3();
        let cell = column.with_y(centre.y + 1);
        let full = |cell: IVec3| {
            map.chunks
                .get_block(cell)
                .is_some_and(|b| matches!(b.get_hitbox(), BlockHitbox::FullBlock))
        };
        let lights = holds(centre, cell.as_vec3())
            && map.chunks.get_block(cell).is_none()
            && full(cell - IVec3::Y);
        if lights {
            let fire = BlockData::new(BlockId::Fire, BlockDirection::Front);
            map.chunks.set_block(cell, fire);
            map.boss.fires.push_back(cell);
        }
    }
    while map.boss.fires.len() > MOST_FIRES {
        if let Some(cell) = map.boss.fires.pop_front() {
            put_out(&mut map.chunks, cell);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mob::combat::wound;
    use bevy::prelude::Transform;
    use rand::{rngs::StdRng, SeedableRng};
    use shared::players::game_mode::GameMode;
    use shared::world::hostiles::Difficulty;
    use shared::world::ItemId;

    /// The ground of the lair of the tests, as in a shallow world.
    const FLOOR: i32 = shared::world::depths::SHALLOW_LAVA_LEVEL;
    const CENTRE: IVec3 = IVec3::new(0, FLOOR, 0);

    /// A lair of stone with a pool of lava, and a player in it.
    fn lair() -> ServerWorldMap {
        let mut map = ServerWorldMap {
            chunks: ChunkMap::with_loaded_region(IVec3::new(-48, -16, -48), IVec3::new(47, 47, 47)),
            ..Default::default()
        };
        let ground = IVec3::new(-40, FLOOR, -40);
        map.chunks
            .fill_box(ground, IVec3::new(40, FLOOR, 40), BlockId::Stone);
        map.chunks.fill_box(
            IVec3::new(6, FLOOR, -2),
            IVec3::new(10, FLOOR, 2),
            BlockId::Lava,
        );
        map.players.insert(1, player(1, 19.5));
        map
    }

    fn player(id: PlayerId, x: f32) -> Player {
        let mut player = Player::new(id, format!("p{id}"), Vec3::ZERO, Transform::default());
        player.position = Vec3::new(x, heart(CENTRE).y + player.height / 2.0, 0.5);
        player.health = 1000;
        player
    }

    fn run(map: &mut ServerWorldMap, ticks: u32) -> Vec<ServerToClientMessage> {
        let mut rng = StdRng::seed_from_u64(1);
        (0..ticks)
            .flat_map(|_| step(map, CENTRE, 0.05, &mut rng))
            .map(|(_, message)| message)
            .collect()
    }

    fn mother(map: &mut ServerWorldMap) -> &mut ServerMob {
        let id = map.boss.id.unwrap();
        map.mobs.get_mut(&id).unwrap()
    }

    fn count(map: &ServerWorldMap, kind: MobKind) -> usize {
        map.mobs.values().filter(|mob| mob.kind == kind).count()
    }

    #[test]
    fn she_wakes_calls_rages_under_half_her_health_and_sleeps_healed_once_alone() {
        let mut map = lair();
        map.difficulty = Difficulty::Peaceful;
        run(&mut map, 5);
        assert!(!map.boss.awake && mother(&mut map).hurt_ticks > 0);
        map.difficulty = Difficulty::Normal;
        map.players.get_mut(&1).unwrap().game_mode = GameMode::Creative;
        run(&mut map, 5);
        assert!(!map.boss.awake);
        map.players.get_mut(&1).unwrap().game_mode = GameMode::Survival;

        let told = run(&mut map, 600);
        assert!(matches!(told[0], ServerToClientMessage::BossBar(Some(_))));
        assert!(map.boss.awake);
        assert!((1..=2).contains(&count(&map, MobKind::Cinderling)));
        assert!(count(&map, MobKind::Ember) > 0);
        assert!(map.boss.fires.is_empty());
        assert_eq!(mother(&mut map).max_health(), 200);
        map.players.insert(2, player(2, -19.5));
        run(&mut map, 1);
        assert_eq!(mother(&mut map).max_health(), 280);

        // over the half, she comes at them, strikes and lights the ground
        mother(&mut map).damage = 150;
        map.players.get_mut(&1).unwrap().health = 1000;
        run(&mut map, 300);
        assert!(!map.boss.fires.is_empty() && map.boss.fires.len() <= MOST_FIRES);
        let hurt = map.players.values().any(|player| player.health < 1000);
        assert!(hurt, "she strikes");
        let at = mother(&mut map).position;
        assert!(holds(CENTRE, at));

        // both leave
        for player in map.players.values_mut() {
            player.position.x += 100.0;
        }
        run(&mut map, EMPTY_TICKS + 1);
        assert!(!map.boss.awake && map.boss.fires.is_empty());
        assert_eq!(mother(&mut map).damage, 0);
        assert!(mother(&mut map).hurt_ticks > 0, "nothing hurts her asleep");
        assert_eq!(mother(&mut map).position, bed(CENTRE));
        assert_eq!(count(&map, MobKind::Cinderling), 0);
        // asleep, the clients are shown no bar
        assert!(map.boss.told.is_empty());
    }

    #[test]
    fn the_lava_of_a_chunk_not_loaded_at_her_defeat_turns_once_it_is() {
        let mut map = lair();
        let (far, cell) = (IVec3::new(0, 0, -1), IVec3::new(8, FLOOR, -1));
        let chunk = map.chunks.chunk(far).unwrap().clone();
        map.chunks.remove_chunk(far);
        map.boss.defeated = true;
        run(&mut map, 1);
        map.chunks.insert_chunk(far, chunk);
        run(&mut map, 1);
        assert_eq!(map.chunks.get_block(cell).unwrap().id, BlockId::Obsidian);
        // Once: what a player puts there later stays
        let lava = BlockData::new(BlockId::Lava, BlockDirection::Front);
        map.chunks.set_block(cell, lava);
        run(&mut map, 1);
        assert_eq!(map.chunks.get_block(cell).unwrap().id, BlockId::Lava);
    }

    #[test]
    fn beaten_she_leaves_obsidian_and_never_comes_back() {
        let mut map = lair();
        run(&mut map, 30);
        // gone any other way, she is not beaten: she sleeps anew
        let id = map.boss.id.unwrap();
        map.mobs.remove(&id);
        let told = run(&mut map, 1);
        assert!(matches!(told[..], [ServerToClientMessage::BossBar(None)]));
        assert!(!map.boss.defeated);
        run(&mut map, 30);
        assert_ne!(map.boss.id, Some(id));
        assert!(map.boss.awake && map.dropped_items.to_saved().is_empty());

        let mother = mother(&mut map);
        wound(mother, 1, 10, Vec3::X);
        assert_eq!(mother.velocity, Vec3::ZERO, "a blow does not move her");
        assert!(!wound(mother, 1, 190, Vec3::X), "left to her own system");
        let told = run(&mut map, 1);
        assert!(told
            .iter()
            .any(|m| matches!(m, ServerToClientMessage::Victory)));
        assert!(told
            .iter()
            .any(|m| matches!(m, ServerToClientMessage::BossBar(None))));
        assert!(map.boss.defeated && map.mobs.is_empty());
        let dropped = map.dropped_items.to_saved();
        let hearts = dropped
            .iter()
            .filter(|item| item.stack.item_id == ItemId::EmberHeart)
            .count();
        assert_eq!((hearts, dropped.len()), (1, 2), "a heart and an orb, once");
        let pool = map.chunks.get_block(IVec3::new(8, FLOOR, 0)).unwrap().id;
        assert_eq!(pool, BlockId::Obsidian);

        // kept by the save
        let meta = crate::world::format::WorldMeta {
            boss_defeated: map.boss.defeated,
            ..crate::world::format::WorldMeta::new(
                "w".into(),
                shared::world::WorldSeed(1),
                0,
                Default::default(),
            )
        };
        let text = ron::to_string(&meta).unwrap();
        let read: crate::world::format::WorldMeta = ron::from_str(&text).unwrap();
        let data = crate::world::format::WorldData::from(read);
        let mut map = lair();
        map.boss.defeated = data.boss_defeated;
        assert!(map.boss.defeated);
        assert!(run(&mut map, 10).is_empty() && map.mobs.is_empty());
    }
}
