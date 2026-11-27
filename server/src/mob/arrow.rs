//! The arrows of the players, once per fixed step after the embers: they fly in a curve, hit the
//! players and the mobs as a blow does, stick in the blocks and are picked up again. An arrow is
//! a mob of its own kind (`MobKind::Arrow`, a projectile), as an ember: the other mob systems
//! leave it alone, and the save leaves it out. What it is doing is in `ServerMob::arrow`; a stuck
//! one is `MobAction::Idle`, one in flight `MobAction::Walk`.

use bevy::{
    math::{
        bounding::{Aabb3d, IntersectsVolume},
        Quat, Vec3, Vec3Swizzles,
    },
    prelude::{DetectChangesMut, Fixed, Res, ResMut, Time},
};
use rand::Rng;
use shared::{
    messages::PlayerId,
    players::{
        bow::{BowShot, ARROW_SPEED},
        effects::EffectKind,
        game_mode::DamageSource,
        movement::is_terrain_loaded,
    },
    world::{
        alchemy, hostiles::is_prey, ArrowFlight, BlockHitbox, BlockTransparency, ChunkMap, DroppedItem,
        ItemId, ItemStack, MobAction, MobKind, ServerMob,
    },
    TICKS_PER_SECOND,
};

use super::combat::{drop_loot, loot, wound};
use crate::world::{data::ServerWorldMap, dropped_items::within_reach};

/// The pull on an arrow in flight, in blocks/s².
const ARROW_GRAVITY: f32 = -16.0;
/// How long an arrow stays, in flight, then again once stuck, in ticks: 3 minutes.
const ARROW_LIFE_TICKS: u32 = 3 * 60 * TICKS_PER_SECOND as u32;
/// How long an arrow leaves its shooter alone, in ticks: it flies out of its box.
const SHOOTER_SAFE_TICKS: u32 = 5;
/// The longest move of an arrow tested at once, in blocks.
const ARROW_STEP: f32 = 0.1;
/// How far below the eyes an arrow starts, in blocks, so that it does not hide the view.
const BELOW_EYES: f32 = 0.12;
/// How far the middle of an arrow stops before what its point is stuck in, in blocks.
const STUCK_BACK: f32 = 0.22;
/// What an arrow keeps of its speed over a tick in a liquid.
const LIQUID_DRAG: f32 = 0.6;
/// The horizontal speed an arrow throws a player or a mob back at, weakly and fully drawn.
const PUSH_SPEED: (f32, f32) = (2.0, 7.0);

/// An arrow `shooter` lets fly from `from`, its eyes, toward `direction`.
pub fn arrow(shooter: PlayerId, from: Vec3, direction: Vec3, shot: BowShot) -> ServerMob {
    let direction = direction.normalize_or(Vec3::NEG_Z);
    // A bolt flies faster and flatter, a throwing axe slower, a potion slower still
    let speed = match shot.ammo {
        ItemId::Bolt => ARROW_SPEED * 1.3,
        ItemId::ThrowingAxe => ARROW_SPEED * 0.55,
        potion if potion.is_splash_potion() => ARROW_SPEED * 0.45,
        _ => ARROW_SPEED,
    };
    let potion = shot.ammo.is_splash_potion().then_some(shot.ammo);
    let kind = if potion.is_some() {
        MobKind::ThrownPotion
    } else {
        MobKind::Arrow
    };
    ServerMob {
        velocity: direction * (speed * shot.power),
        rotation: pointing(direction),
        action: MobAction::Walk,
        action_ticks: ARROW_LIFE_TICKS,
        arrow: Some(ArrowFlight {
            shooter,
            shot,
            stuck_in: None,
            pierce_left: shot.pierce,
        }),
        potion,
        ..ServerMob::new(kind, from - Vec3::Y * BELOW_EYES)
    }
}

/// The rotation of an arrow, whose model faces +Z, that points along `direction`.
fn pointing(direction: Vec3) -> Quat {
    let yaw = direction.x.atan2(direction.z);
    let pitch = -direction.y.atan2(direction.xz().length());
    Quat::from_rotation_y(yaw) * Quat::from_rotation_x(pitch)
}

/// One step of every arrow: those that hit, were picked up or are too old are removed, and the
/// clients are told by the next broadcast, which no longer finds them.
pub fn arrow_system(mut world_map: ResMut<ServerWorldMap>, time: Res<Time<Fixed>>) {
    let map = world_map.bypass_change_detection();
    let ids: Vec<_> = map
        .mobs
        .iter()
        .filter(|(_, mob)| matches!(mob.kind, MobKind::Arrow | MobKind::ThrownPotion))
        .map(|(id, _)| *id)
        .collect();
    let mut rng = rand::thread_rng();
    for id in ids {
        let Some(mut arrow) = map.mobs.remove(&id) else {
            continue;
        };
        let was = (arrow.position, arrow.action);
        let stays = step(&mut arrow, map, time.delta_secs(), &mut rng);
        if !stays {
            map.changed_mobs.remove(&id);
            continue;
        }
        if was != (arrow.position, arrow.action) {
            map.changed_mobs.insert(id);
        }
        map.mobs.insert(id, arrow);
    }
}

/// One step of `arrow`. Returns whether it stays in the world.
fn step(arrow: &mut ServerMob, map: &mut ServerWorldMap, delta: f32, rng: &mut impl Rng) -> bool {
    let Some(flight) = arrow.arrow else {
        return false;
    };
    if arrow.action_ticks == 0
        || !arrow.position.is_finite()
        || !arrow.velocity.is_finite()
        || !is_terrain_loaded(arrow.position, &map.chunks)
    {
        return false;
    }
    arrow.action_ticks -= 1;
    let Some(cell) = flight.stuck_in else {
        return fly(arrow, flight, map, delta, rng);
    };
    if map.chunks.get_block(cell).is_none() {
        // Its block is gone: it falls
        arrow.arrow = Some(ArrowFlight {
            stuck_in: None,
            ..flight
        });
        arrow.action = MobAction::Walk;
        arrow.velocity = Vec3::ZERO;
        return true;
    }
    !(flight.shot.recoverable && is_picked_up(arrow.position, flight.shot.ammo, map))
}

/// What flew, as an item of one.
fn ammo_stack(ammo: ItemId) -> ItemStack {
    ItemStack {
        item_id: ammo,
        item_type: ammo.get_default_type(),
        nb: 1,
    }
}

/// Drops what flew where it ended, if it may be picked up again: a throwing axe falls back as
/// itself, an arrow parried by a shield falls off it.
fn fall_as_item(flight: &ArrowFlight, at: Vec3, map: &mut ServerWorldMap) {
    if flight.shot.recoverable {
        map.dropped_items
            .spawn(DroppedItem::new(ammo_stack(flight.shot.ammo), at, Vec3::ZERO));
    }
}

/// Gives the arrow at `position` to a living player within reach with room for it, as an item
/// dropped there, which the player picks up at once.
fn is_picked_up(position: Vec3, ammo: ItemId, map: &mut ServerWorldMap) -> bool {
    let stack = ammo_stack(ammo);
    let taken = map.players.values().any(|player| {
        !player.is_dead()
            && within_reach(player, position)
            && player.inventory.clone().add_item_to_inventory(stack) == 0
    });
    if taken {
        map.dropped_items.spawn(DroppedItem {
            pickup_delay_ticks: 0,
            ..DroppedItem::new(stack, position, Vec3::ZERO)
        });
    }
    taken
}

/// One step of `arrow` along its curve, by `ARROW_STEP` at most. It hits the first player in
/// survival or mob whose box it touches, but its shooter at first, and is gone, but a bolt that
/// still pierces goes on through the mobs; it sticks in the first block whose hitbox it enters. A
/// shield raised toward it parries it. A throwing axe falls as an item wherever it ends. Returns
/// whether it stays in the world.
fn fly(
    arrow: &mut ServerMob,
    mut flight: ArrowFlight,
    map: &mut ServerWorldMap,
    delta: f32,
    rng: &mut impl Rng,
) -> bool {
    let start = arrow.position;
    let pull = Vec3::Y * ARROW_GRAVITY;
    let moved = arrow.velocity * delta + pull * (delta * delta / 2.0);
    arrow.velocity += pull * delta;
    if is_liquid(&map.chunks, start) {
        arrow.velocity *= LIQUID_DRAG;
    }
    let direction = moved.normalize_or_zero();
    if direction != Vec3::ZERO {
        arrow.rotation = pointing(direction);
    }
    let push = direction.with_y(0.0).normalize_or_zero()
        * (PUSH_SPEED.0 + (PUSH_SPEED.1 - PUSH_SPEED.0) * flight.shot.power);
    let half = Vec3::new(arrow.width, arrow.height, arrow.depth) / 2.0;
    let safe = arrow.action_ticks + SHOOTER_SAFE_TICKS >= ARROW_LIFE_TICKS;

    let steps = (moved.length() / ARROW_STEP).ceil().max(1.0) as u32;
    // The boxes the whole move may touch
    let middle = start + moved / 2.0;
    let swept = Aabb3d::new(middle, moved.abs() / 2.0 + half);
    let near: Vec<_> = map
        .mobs
        .iter()
        .filter(|(_, mob)| !mob.kind.is_projectile())
        // A piercing bolt goes through what it just hurt
        .filter(|(_, mob)| flight.pierce_left == 0 || mob.hurt_ticks == 0)
        .map(|(id, mob)| (*id, mob_box(mob)))
        .filter(|(_, body)| swept.intersects(body))
        .collect();
    // The mobs a piercing bolt went through in this step
    let mut passed = Vec::new();
    for step in 1..=steps {
        let at = start + moved * (step as f32 / steps as f32);
        let body = Aabb3d::new(at, half);
        let struck = map.players.values_mut().find(|player| {
            let half = Vec3::new(player.width, player.height, player.width) / 2.0;
            is_prey(player)
                && !(safe && player.id == flight.shooter)
                && body.intersects(&Aabb3d::new(player.position, half))
        });
        if flight.shot.ammo.is_splash_potion()
            && (struck.is_some() || near.iter().any(|(_, target)| body.intersects(target)))
        {
            burst(&flight, at - direction * 0.3, map, rng);
            return false;
        }
        if let Some(player) = struck {
            let source = DamageSource::Arrow(flight.shooter);
            let parried = player
                .parry(at - direction * 2.0, flight.shot.damage, false)
                .is_some();
            if !parried && player.hurt(flight.shot.damage, source) {
                player.knock_back(push, push.length());
            }
            if parried || flight.shot.ammo == ItemId::ThrowingAxe {
                fall_as_item(&flight, at - direction * 0.5, map);
            }
            return false;
        }
        let hit = near
            .iter()
            .find(|(id, target)| body.intersects(target) && !passed.contains(id));
        if let Some((id, _)) = hit {
            if let Some(mob) = map.mobs.get_mut(id) {
                // One still invulnerable is hit for nothing, as by a blow; a villager always
                if mob.hurt_ticks == 0 && !mob.kind.is_villager() {
                    map.changed_mobs.insert(*id);
                    if wound(mob, flight.shooter, flight.shot.damage, push) {
                        if let Some(shooter) = map.players.get_mut(&flight.shooter) {
                            shooter.beat(mob.kind);
                        }
                        let (drops, position) = (loot(mob, rng), mob.position);
                        drop_loot(&mut map.dropped_items, drops, position);
                        let points = mob.experience();
                        map.dropped_items.drop_orb(points, position, rng);
                        map.mobs.remove(id);
                        map.changed_mobs.remove(id);
                    }
                }
            }
            if flight.pierce_left > 0 {
                flight.pierce_left -= 1;
                arrow.arrow = Some(flight);
                passed.push(*id);
                arrow.position = at;
                continue;
            }
            if flight.shot.ammo == ItemId::ThrowingAxe {
                fall_as_item(&flight, at, map);
            }
            return false;
        }
        if is_in_block(&map.chunks, at) || flight.shot.ammo.is_splash_potion() && is_liquid(&map.chunks, at) {
            if flight.shot.ammo.is_splash_potion() {
                burst(&flight, at - direction * STUCK_BACK, map, rng);
                return false;
            }
            if flight.shot.ammo == ItemId::ThrowingAxe {
                fall_as_item(&flight, at - direction * STUCK_BACK, map);
                return false;
            }
            arrow.position = at - direction * STUCK_BACK;
            arrow.velocity = Vec3::ZERO;
            arrow.action = MobAction::Idle;
            arrow.action_ticks = ARROW_LIFE_TICKS;
            arrow.arrow = Some(ArrowFlight {
                stuck_in: Some(at.floor().as_ivec3()),
                ..flight
            });
            return true;
        }
        arrow.position = at;
    }
    true
}

/// The points of health a splash of poison takes at once from the mobs it reaches, which have no
/// status effects: all of them on the spot, half at the edge (`alchemy::splash_share`).
const POISON_SPLASH_DAMAGE: f32 = 4.0;

/// A splash potion bursts at `at`: each living player within `alchemy::SPLASH_RADIUS` gets its
/// effect, for a share of its time by how far it is (`alchemy::splash_share`), and a splash of
/// poison hurts the mobs around, but the villagers.
fn burst(flight: &ArrowFlight, at: Vec3, map: &mut ServerWorldMap, rng: &mut impl Rng) {
    let stack = ItemStack {
        item_id: flight.shot.ammo,
        item_type: flight.shot.ammo_type,
        nb: 1,
    };
    let Some((kind, level, ticks)) = alchemy::potion_effect(&stack) else {
        return;
    };
    for player in map.players.values_mut() {
        let share = alchemy::splash_share(player.position.distance(at));
        if share > 0.0 && !player.is_dead() && player.game_mode.takes_damage() {
            player.take_effect(kind, level, (ticks as f32 * share).round() as u32);
        }
    }
    if kind != EffectKind::Poison {
        return;
    }
    let reached: Vec<_> = map
        .mobs
        .iter()
        .filter(|(_, mob)| !mob.kind.is_projectile() && !mob.kind.is_villager())
        .map(|(id, mob)| (*id, alchemy::splash_share(mob.position.distance(at))))
        .filter(|(_, share)| *share > 0.0)
        .collect();
    for (id, share) in reached {
        let Some(mob) = map.mobs.get_mut(&id) else {
            continue;
        };
        let damage = (POISON_SPLASH_DAMAGE * share * level as f32).round() as u32;
        map.changed_mobs.insert(id);
        if wound(mob, flight.shooter, damage.max(1), Vec3::ZERO) {
            if let Some(shooter) = map.players.get_mut(&flight.shooter) {
                shooter.beat(mob.kind);
            }
            let (drops, position) = (loot(mob, rng), mob.position);
            drop_loot(&mut map.dropped_items, drops, position);
            let points = mob.experience();
            map.dropped_items.drop_orb(points, position, rng);
            map.mobs.remove(&id);
            map.changed_mobs.remove(&id);
        }
    }
}

fn mob_box(mob: &ServerMob) -> Aabb3d {
    let half = Vec3::new(mob.width, mob.height, mob.depth) / 2.0;
    Aabb3d::new(mob.position, half)
}

fn is_liquid(chunks: &ChunkMap, at: Vec3) -> bool {
    chunks
        .get_block(at.floor().as_ivec3())
        .is_some_and(|block| block.id.get_visibility() == BlockTransparency::Liquid)
}

/// Whether the point `at` is in the hitbox of a block.
fn is_in_block(chunks: &ChunkMap, at: Vec3) -> bool {
    let cell = at.floor();
    let Some(block) = chunks.get_block(cell.as_ivec3()) else {
        return false;
    };
    match block.get_hitbox() {
        _ if is_liquid(chunks, at) => false,
        BlockHitbox::None => false,
        BlockHitbox::FullBlock => true,
        BlockHitbox::Aabb(hitbox) => {
            let inside = bevy::math::Vec3A::from(at - cell);
            inside.cmpge(hitbox.min).all() && inside.cmple(hitbox.max).all()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;
    use bevy::math::IVec3;
    use bevy::prelude::{Transform, World};
    use shared::players::game_mode::{DeathCause, MAX_HEALTH};
    use shared::players::Player;
    use shared::world::BlockId;
    use std::time::Duration;

    const EYES: Vec3 = Vec3::new(0.5, 2.6, 0.5);

    /// A floor with its top at y = 1, and a wall across x = 12.
    fn world(target: Option<Player>) -> World {
        let mut map = ServerWorldMap {
            chunks: ChunkMap::with_loaded_region(IVec3::new(-40, -16, -40), IVec3::new(39, 31, 39)),
            ..Default::default()
        };
        let stone = BlockId::Stone;
        map.chunks
            .fill_box(IVec3::new(-40, 0, -40), IVec3::new(39, 0, 39), stone);
        map.chunks
            .fill_box(IVec3::new(12, 1, -5), IVec3::new(12, 6, 5), stone);
        for player in [Some(player_at(1, 0.5)), target].into_iter().flatten() {
            map.players.insert(player.id, player);
        }
        let mut world = World::new();
        world.insert_resource(map);
        let mut time = Time::<Fixed>::default();
        time.advance_by(Duration::from_secs_f64(1.0 / TICKS_PER_SECOND as f64));
        world.insert_resource(time);
        world
    }

    fn player_at(id: PlayerId, x: f32) -> Player {
        let mut player = Player::new(id, format!("p{id}"), Vec3::ZERO, Transform::default());
        player.position = Vec3::new(x, 1.0 + player.height / 2.0, 0.5);
        player
    }

    fn shoot(world: &mut World, ticks: u32) {
        shoot_with(world, ticks, ItemId::Arrow, 0);
    }

    fn shoot_with(world: &mut World, ticks: u32, ammo: ItemId, pierce: u8) {
        let shot = BowShot {
            power: 1.0,
            damage: 9,
            recoverable: true,
            ammo,
            pierce,
            ammo_type: shared::world::ItemType::Generic,
        };
        let mut map = world.resource_mut::<ServerWorldMap>();
        map.mobs.insert(9, arrow(1, EYES, Vec3::X, shot));
        run(world, ticks);
    }

    fn run(world: &mut World, ticks: u32) {
        for _ in 0..ticks {
            world.run_system_once(arrow_system).unwrap();
        }
    }

    fn map(world: &World) -> &ServerWorldMap {
        world.resource::<ServerWorldMap>()
    }

    #[test]
    fn an_arrow_falls_as_it_flies_sticks_in_a_wall_and_is_picked_up_there() {
        let mut w = world(None);
        shoot(&mut w, 20);
        let stuck = &map(&w).mobs[&9];
        assert_eq!(stuck.action, MobAction::Idle);
        assert_eq!(stuck.arrow.unwrap().stuck_in, Some(IVec3::new(12, 1, 0)));
        assert!(stuck.position.y < EYES.y - 0.4, "{}", stuck.position);
        assert!((11.5..12.0).contains(&stuck.position.x));
        // It falls when its block is gone, and goes to who walks over it
        let mut state = w.resource_mut::<ServerWorldMap>();
        state.chunks.remove_block(IVec3::new(12, 1, 0));
        run(&mut w, 20);
        let at = map(&w).mobs[&9].position;
        assert!(at.y < 1.5, "{at}");
        let mut state = w.resource_mut::<ServerWorldMap>();
        state.players.get_mut(&1).unwrap().position = at + Vec3::Y * 0.5;
        run(&mut w, 1);
        assert!(map(&w).mobs.is_empty());
        let mut state = w.resource_mut::<ServerWorldMap>();
        let ServerWorldMap {
            dropped_items,
            players,
            ..
        } = &mut *state;
        dropped_items.pick_up(players);
        let held = players[&1].inventory.inner.values().next().unwrap();
        assert_eq!((held.item_id, held.nb), (ItemId::Arrow, 1));
    }

    #[test]
    fn an_arrow_hurts_and_throws_back_a_player_or_a_mob_and_credits_the_shooter() {
        let mut w = world(Some(player_at(2, 6.5)));
        shoot(&mut w, 10);
        assert!(map(&w).mobs.is_empty());
        let target = &map(&w).players[&2];
        assert_eq!(target.health, MAX_HEALTH - 9);
        assert!(target.velocity.x > 3.0);
        // Again, to the death
        for _ in 0..2 {
            let mut state = w.resource_mut::<ServerWorldMap>();
            state.players.get_mut(&2).unwrap().invulnerable_ticks = 0;
            shoot(&mut w, 10);
        }
        assert_eq!(map(&w).players[&2].death_cause, Some(DeathCause::Shot(1)));

        let mut w = world(None);
        let at = ServerMob::standing_at(MobKind::Stumpling, 6.5, 1.0, 0.5);
        let mob = ServerMob::new(MobKind::Stumpling, at);
        w.resource_mut::<ServerWorldMap>().mobs.insert(3, mob);
        shoot(&mut w, 10);
        let mob = &map(&w).mobs[&3];
        assert_eq!(mob.damage, 9);
        assert!(mob.velocity.x > 3.0);
        assert!(matches!(mob.target, shared::world::MobTarget::Player(1)));
        assert!(!map(&w).mobs.contains_key(&9));
    }

    #[test]
    fn a_splash_potion_bursts_on_a_player_and_reaches_those_around() {
        use shared::players::effects::EffectKind;
        let mut w = world(Some(player_at(2, 6.5)));
        w.resource_mut::<ServerWorldMap>()
            .players
            .insert(3, player_at(3, 8.5));
        let shot = BowShot::splash(
            &ItemStack {
                item_id: ItemId::SplashSlownessPotion,
                item_type: shared::world::ItemType::Generic,
                nb: 1,
            },
            600,
        );
        let mut map_ = w.resource_mut::<ServerWorldMap>();
        let potion = arrow(1, EYES, Vec3::X, shot);
        assert_eq!(potion.kind, MobKind::ThrownPotion);
        assert_eq!(potion.potion, Some(ItemId::SplashSlownessPotion));
        map_.mobs.insert(9, potion);
        run(&mut w, 20);
        assert!(map(&w).mobs.is_empty());
        let hit = map(&w).players[&2].effects.get(EffectKind::Slowness).unwrap();
        let near = map(&w).players[&3].effects.get(EffectKind::Slowness).unwrap();
        assert_eq!(map(&w).players[&2].health, MAX_HEALTH, "it hurts nothing");
        assert!(near.ticks_left < hit.ticks_left);
        // The thrower, far away, is spared
        assert!(map(&w).players[&1].effects.is_empty());
    }

    #[test]
    fn a_splash_of_poison_hurts_the_mobs_around() {
        let mut w = world(None);
        let at = ServerMob::standing_at(MobKind::Mossback, 6.5, 1.0, 0.5);
        w.resource_mut::<ServerWorldMap>()
            .mobs
            .insert(3, ServerMob::new(MobKind::Mossback, at));
        let shot = BowShot::splash(
            &ItemStack {
                item_id: ItemId::SplashPoisonPotion,
                item_type: shared::world::ItemType::Generic,
                nb: 1,
            },
            600,
        );
        let mut map_ = w.resource_mut::<ServerWorldMap>();
        map_.mobs.insert(9, arrow(1, EYES, Vec3::X, shot));
        run(&mut w, 30);
        assert!(!map(&w).mobs.contains_key(&9));
        assert!(map(&w).mobs[&3].damage > 0);
    }

    fn stumplings(w: &mut World) {
        for (id, x) in [(3, 4.5), (4, 7.5)] {
            let at = ServerMob::standing_at(MobKind::Stumpling, x, 1.0, 0.5);
            let mob = ServerMob::new(MobKind::Stumpling, at);
            w.resource_mut::<ServerWorldMap>().mobs.insert(id, mob);
        }
    }

    #[test]
    fn a_bolt_pierces_the_first_creature_and_hurts_the_next_but_an_arrow_stops() {
        let mut w = world(None);
        stumplings(&mut w);
        shoot_with(&mut w, 10, ItemId::Bolt, 1);
        assert_eq!(map(&w).mobs[&3].damage, 9);
        assert_eq!(map(&w).mobs[&4].damage, 9);
        assert!(!map(&w).mobs.contains_key(&9));

        let mut w = world(None);
        stumplings(&mut w);
        shoot(&mut w, 10);
        assert_eq!(map(&w).mobs[&3].damage, 9);
        assert_eq!(map(&w).mobs[&4].damage, 0);
    }

    #[test]
    fn a_shield_raised_toward_an_arrow_parries_it_and_it_falls_off() {
        use shared::players::shield::RAISE_MS;
        use shared::world::ItemStack;
        let mut target = player_at(2, 6.5);
        target.inventory.inner.insert(
            0,
            ItemStack {
                item_id: ItemId::Shield,
                item_type: ItemId::Shield.get_default_type(),
                nb: 1,
            },
        );
        target.shield_ms = RAISE_MS;
        target.camera_transform = Transform::default().looking_to(Vec3::NEG_X, Vec3::Y);
        let mut w = world(Some(target));
        shoot(&mut w, 10);
        let target = &map(&w).players[&2];
        assert_eq!(target.health, MAX_HEALTH);
        assert_eq!(target.parries, 1);
        assert!(map(&w).mobs.is_empty());
        assert_eq!(map(&w).dropped_items.to_saved().len(), 1);
    }

    #[test]
    fn a_throwing_axe_falls_back_as_itself_where_it_ends() {
        let mut w = world(None);
        shoot_with(&mut w, 20, ItemId::ThrowingAxe, 0);
        assert!(map(&w).mobs.is_empty());
        let items = map(&w).dropped_items.to_saved();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].stack.item_id, ItemId::ThrowingAxe);
    }
}
