//! What the players do to the mobs they aim at: blows, taking the wool of a sheep, and feeding
//! an animal its food (`breeding::feed`), and opening a trade with a villager.

use bevy::prelude::*;
use rand::Rng;
use shared::{
    messages::{NetworkAction, PlayerFrameInput, PlayerId},
    players::{
        blocks::{aim, held_item},
        hunger::BLOW_EXHAUSTION,
        melee::{self, Recharge, Strike, WeaponClass},
        shield,
        progress::Achievement,
        Player, ViewMode,
    },
    world::{
        aabb_ray_hit, depths::DepthTier, DroppedItem, FaceDirectionExt, ItemId, ItemStack, Leash,
        MobAction, MobId, MobKind, MobTarget, ServerMob,
    },
};
use std::collections::{HashMap, HashSet};

use bevy::math::bounding::Aabb3d;
use shared::world::ChunkMap;

use super::villager::Trades;
use crate::world::dropped_items::DroppedItems;

/// How far from its eyes a player reaches a mob, in blocks, and with a spear.
const REACH: f32 = 3.0;
const SPEAR_REACH: f32 = 5.0;
/// Time between two meals given while the button is held.
const FEED_MS: u64 = 500;
/// Upward speed of a mob that takes a blow.
const KNOCK_SPEED: f32 = 4.0;
/// Horizontal speed of a mob that takes a blow, away from the player, in blocks per second:
/// above how fast it runs, so that it flies until it lands (`behavior::is_flung`).
const KNOCK_BACK_SPEED: f32 = 6.0;
/// Ticks after a blow during which the mob takes no other.
pub const MOB_INVULNERABILITY_TICKS: u32 = 10;
/// Ticks the guard of a Stumpling stays down after an axe knocked it aside: 4 seconds.
pub const GUARD_DOWN_TICKS: u32 = 4 * shared::TICKS_PER_SECOND as u32;
/// Ticks a mob hit flees from the player: 5 seconds.
pub const FLEE_TICKS: u32 = 5 * shared::TICKS_PER_SECOND as u32;

/// Time left before each player holding the right button feeds an animal again.
pub type BlowWaits = HashMap<PlayerId, u64>;

/// The recharge of the blows of each player (`melee::Recharge`): the server counts it from the
/// inputs, and a blow hurts by it.
pub type Recharges = HashMap<PlayerId, Recharge>;

/// The mobs, and what their changes touch.
pub struct MobWorld<'a> {
    pub mobs: &'a mut HashMap<MobId, ServerMob>,
    pub changed: &'a mut HashSet<MobId>,
    pub items: &'a mut DroppedItems,
    pub chunks: &'a mut ChunkMap,
    pub recharges: &'a mut Recharges,
    /// Time left before each player holding the right button feeds an animal again.
    pub feed_waits: &'a mut BlowWaits,
    pub trades: &'a mut Trades,
}

/// Runs the clicks of `input` on the mob the player aims at, if any, and takes them out of the
/// input: they are not for the block behind it, whose state is told again to the client.
pub fn interact(
    player: &mut Player,
    input: &PlayerFrameInput,
    mut world: MobWorld,
    rng: &mut impl Rng,
) -> Option<PlayerFrameInput> {
    let left = input.is_pressed(NetworkAction::LeftClick);
    let right = input.is_pressed(NetworkAction::RightClick);
    // Every input counts toward the recharge, whatever the player aims at
    let swing = world.recharges.entry(player.id).or_default().step(
        held_item(player, input),
        left && !player.is_dead(),
        input.delta_ms,
    );
    let feed_wait = world.feed_waits.entry(player.id).or_default();
    *feed_wait = if right {
        feed_wait.saturating_sub(input.delta_ms.min(100))
    } else {
        0
    };
    // A trade opens on a press, not while the button stays held after another click
    let pressed = world.trades.press(player.id, right);
    if player.is_dead() || !(left || right) {
        return None;
    }
    let held = held_item(player, input);
    let id = aimed_mob(player, input, held, world.mobs, world.chunks)?;
    let mob = world.mobs.get_mut(&id)?;
    // An animal held its food is fed, rather than the player eating it
    let feeds = right && held.is_some_and(|item| mob.kind.eats(item));
    let shears = right
        && !feeds
        && mob.kind == MobKind::Sheep
        && !mob.is_young()
        // It is drawn, or cast
        && held != Some(ItemId::Bow)
        && held != Some(ItemId::FishingRod)
        && !held.is_some_and(|i| i.spawned_mob().is_some() || i.food_value().is_some());
    // Nothing hurts a villager
    let villager = mob.kind.is_villager();
    // A bow being drawn is not let go by what it sweeps over
    let trades = right && villager && player.bow_draw_ms == 0;
    // A cow gives its milk to a bucket
    let milks = right
        && held == Some(ItemId::Bucket)
        && mob.kind == MobKind::Cow
        && !mob.is_young();
    let mine = mob.owner.as_deref() == Some(player.name.as_str());
    let saddles = right
        && held == Some(ItemId::Saddle)
        && mob.kind.is_steed()
        && !mob.saddled
        && !mob.is_young()
        && mine;
    // A lead ties an animal to the hand, or takes it from its post
    let led = Leash::Player(player.name.clone());
    let leashes = right
        && held == Some(ItemId::Lead)
        && mob.kind.is_leashable()
        && mob.leash.as_ref() != Some(&led);
    let unleashes = right && !leashes && !feeds && mob.leash.as_ref() == Some(&led);
    let armours = right
        && held.is_some_and(|item| item.horse_armour().is_some())
        && mob.kind == MobKind::Horse
        && !mob.is_young()
        && mine;
    let chests = right
        && held == Some(ItemId::Chest)
        && mob.kind.carries_chest()
        && mob.pack.is_none()
        && !mob.is_young()
        && mine;
    let sits = right
        && mob.kind.sits()
        && mine
        && !feeds
        && !leashes
        && !unleashes
        && !player.is_sneaking
        && player.bow_draw_ms == 0
        && !held.is_some_and(|i| i.spawned_mob().is_some() || i == ItemId::FishingRod);
    let mounts = right
        && !feeds
        && !saddles
        && !leashes
        && !unleashes
        && !armours
        && !chests
        && !milks
        && !shears
        && player.bow_draw_ms == 0
        && player.riding.is_none()
        && !player.is_sneaking
        && !held.is_some_and(|i| i.spawned_mob().is_some())
        && super::riding::free_seat(mob, &player.name).is_some();
    let tends = leashes || unleashes || armours || chests || sits;
    if !left && !shears && !feeds && !trades && !saddles && !mounts && !milks && !tends {
        return None;
    }
    let target = aim(
        world.chunks,
        &input.camera,
        &player.position,
        input.view_mode,
        held,
    );
    if let Some(target) = target {
        world.chunks.touch_block(target.position);
        world
            .chunks
            .touch_block(target.position + target.face.to_ivec3());
    }
    let mut rest = input.clone();
    let mut drops = Vec::new();
    if shears {
        rest.inputs.remove(&NetworkAction::RightClick);
        if !mob.sheared {
            mob.sheared = true;
            world.changed.insert(id);
            drops.push((ItemId::Wool, rng.gen_range(1..=2)));
        }
    }
    if feeds {
        rest.inputs.remove(&NetworkAction::RightClick);
        if let (0, Some(item)) = (*feed_wait, held) {
            *feed_wait = FEED_MS;
            let wild = mob.owner.is_none();
            if super::breeding::feed(mob, &player.name, item, rng) {
                if wild && mob.owner.is_some() {
                    player.earn(Achievement::Animal);
                }
                player
                    .inventory
                    .remove_item_from_stack(input.hotbar_slot, 1);
                world.changed.insert(id);
            }
        }
    }
    if trades {
        rest.inputs.remove(&NetworkAction::RightClick);
        if pressed {
            world.trades.open.insert(player.id, id);
        }
    }
    if saddles {
        rest.inputs.remove(&NetworkAction::RightClick);
        if pressed {
            mob.saddled = true;
            player
                .inventory
                .remove_item_from_stack(input.hotbar_slot, 1);
            world.changed.insert(id);
        }
    }
    if tends {
        rest.inputs.remove(&NetworkAction::RightClick);
        if pressed {
            let slot = input.hotbar_slot;
            if leashes {
                // One from its post is taken back in hand, the lead it wears with it
                if mob.leash.is_none() {
                    player.inventory.remove_item_from_stack(slot, 1);
                }
                mob.leash = Some(led.clone());
            } else if unleashes {
                mob.leash = None;
                give(player, ItemId::Lead, &mut drops);
            } else if armours {
                if let Some(worn) = mob.horse_armour.take() {
                    give(player, worn, &mut drops);
                }
                mob.horse_armour = held;
                player.inventory.remove_item_from_stack(slot, 1);
            } else if chests {
                mob.pack = Some(Box::default());
                player.inventory.remove_item_from_stack(slot, 1);
            } else if sits {
                super::companions::toggle_sit(mob);
            }
            world.changed.insert(id);
        }
    }
    if milks {
        rest.inputs.remove(&NetworkAction::RightClick);
        if pressed {
            milk(player, input.hotbar_slot, &mut drops);
        }
    }
    if mounts {
        rest.inputs.remove(&NetworkAction::RightClick);
        if pressed {
            super::riding::mount(player, id, mob, rng);
            world.changed.insert(id);
        }
    }
    let position = mob.position;
    let mut sweep = None;
    let mut rallies = false;
    if left {
        rest.inputs.remove(&NetworkAction::LeftClick);
        // A blow on a mob still invulnerable swings for nothing
        if let (Some(charge), false) = (swing, villager) {
            let class = WeaponClass::of(held);
            let guarded = mob.kind.guards()
                && mob.guard_down == 0
                && shield::in_front(mob.rotation * Vec3::Z, mob.position, player.position);
            if guarded && mob.hurt_ticks == 0 && !class.breaks_shields() {
                // Its bark parries the blow
                mob.hurt_ticks = MOB_INVULNERABILITY_TICKS;
                mob.parries = mob.parries.wrapping_add(1);
                world.changed.insert(id);
                if let Some(item) = held {
                    player
                        .inventory
                        .wear_tool(input.hotbar_slot, item.blow_wear());
                }
            } else if mob.hurt_ticks == 0 {
                if guarded {
                    // An axe knocks the guard aside, and the blow lands
                    mob.guard_down = GUARD_DOWN_TICKS;
                    mob.parries = mob.parries.wrapping_add(1);
                }
                player.exhaust(BLOW_EXHAUSTION);
                let away = (mob.position - player.position)
                    .with_y(0.0)
                    .try_normalize()
                    .unwrap_or_else(|| input.camera.forward().with_y(0.0).normalize_or_zero());
                let weapon = player.inventory.inner.get(&input.hotbar_slot);
                let sharpness = weapon.filter(|_| held.is_some());
                let base = mob.kind.blow_damage(held)
                    + sharpness.map_or(0, |weapon| weapon.bonus_blow_damage());
                let base = player.effects.weakened(base);
                let critical = melee::is_critical(
                    charge,
                    player.on_ground,
                    -player.velocity.y,
                    player.is_flying || player.is_swimming,
                );
                let sweeps = melee::sweeps(class, charge, player.on_ground);
                let damage = melee::blow_damage(base, charge, critical);
                // A hurried blow throws the mob back less far
                let push = away * KNOCK_BACK_SPEED * (0.5 + 0.5 * charge) * class.knock_back();
                let dies = wound(mob, player.id, damage, push);
                mob.struck = if critical {
                    Some(Strike::Critical)
                } else if sweeps {
                    Some(Strike::Sweep)
                } else {
                    None
                };
                if sweeps {
                    sweep = Some(melee::sweep_damage(damage));
                }
                world.changed.insert(id);
                if let Some(item) = held {
                    player
                        .inventory
                        .wear_tool(input.hotbar_slot, item.blow_wear());
                }
                if dies {
                    defeat(player, id, &mut world, rng, &mut drops);
                } else {
                    rallies = true;
                }
            }
        }
    }
    drop_loot(world.items, drops, position);
    // The wolves of the player go at what it struck
    if rallies {
        super::companions::rally(world.mobs, &player.name, id);
    }
    if let Some(damage) = sweep {
        let forward = input.camera.forward().as_vec3();
        sweep_around(player, id, position, forward, damage, &mut world, rng);
    }
    Some(rest)
}

/// Puts one `item` into the inventory of `player`, or into `drops` without room.
fn give(player: &mut Player, item: ItemId, drops: &mut Vec<(ItemId, u32)>) {
    let stack = ItemStack {
        item_id: item,
        item_type: item.get_default_type(),
        nb: 1,
    };
    if player.inventory.add_item_to_inventory(stack) > 0 {
        drops.push((item, 1));
    }
}

/// Fills a bucket of the hotbar slot `slot` of `player` with milk: a single bucket becomes the
/// bucket of milk, one of a stack goes into the inventory, or into `drops` without room.
fn milk(player: &mut Player, slot: u32, drops: &mut Vec<(ItemId, u32)>) {
    let Some(stack) = player.inventory.inner.get(&slot).copied() else {
        return;
    };
    let milk = ItemStack {
        item_id: ItemId::MilkBucket,
        item_type: ItemId::MilkBucket.get_default_type(),
        nb: 1,
    };
    if stack.nb == 1 {
        player.inventory.inner.insert(slot, milk);
        return;
    }
    player.inventory.remove_item_from_stack(slot, 1);
    if player.inventory.add_item_to_inventory(milk) > 0 {
        drops.push((ItemId::MilkBucket, 1));
    }
}

/// The mob `id`, dead of a blow of `player`: the player beat it, its loot goes into `drops` and its
/// experience is dropped where it was, and it is taken out of the world.
fn defeat(
    player: &mut Player,
    id: MobId,
    world: &mut MobWorld,
    rng: &mut impl Rng,
    drops: &mut Vec<(ItemId, u32)>,
) {
    let Some(mob) = world.mobs.remove(&id) else {
        return;
    };
    player.beat(mob.kind);
    drops.extend(loot(&mob, rng));
    super::pack::drop_pack(world.items, &mob);
    world.items.drop_orb(mob.experience(), mob.position, rng);
    // The clients are told by the next broadcast, which no longer finds it
    world.changed.remove(&id);
}

/// The sweep of a full blow of a sword of `player` looking along `forward` on the mob `target`,
/// which was at `at`: the other mobs around it and in front of the player take `damage`. The tamed
/// animals, the villagers and the projectiles are spared.
fn sweep_around(
    player: &mut Player,
    target: MobId,
    at: Vec3,
    forward: Vec3,
    damage: u32,
    world: &mut MobWorld,
    rng: &mut impl Rng,
) {
    let swept: Vec<MobId> = world
        .mobs
        .iter()
        .filter(|(&id, mob)| {
            id != target
                && mob.hurt_ticks == 0
                && mob.owner.is_none()
                && !mob.kind.is_projectile()
                && !mob.kind.is_villager()
                && melee::in_sweep(player.position, forward, at, mob.position)
        })
        .map(|(&id, _)| id)
        .collect();
    for id in swept {
        let Some(mob) = world.mobs.get_mut(&id) else {
            continue;
        };
        let away = (mob.position - player.position)
            .with_y(0.0)
            .normalize_or_zero();
        let dies = wound(mob, player.id, damage, away * KNOCK_BACK_SPEED * 0.5);
        mob.struck = Some(Strike::Sweep);
        world.changed.insert(id);
        if dies {
            let position = mob.position;
            let mut drops = Vec::new();
            defeat(player, id, world, rng, &mut drops);
            drop_loot(world.items, drops, position);
        }
    }
}

/// The player `striker` takes `damage` points of health from `mob`, which may be hurt
/// (`ServerMob::hurt_ticks`), and throws it back at the horizontal velocity `push`: a hostile
/// turns on the striker, an animal flees from it. Returns whether the mob dies: a
/// boss is ended by its own system (`boss::step`), which drops what it leaves.
pub fn wound(mob: &mut ServerMob, striker: PlayerId, damage: u32, push: Vec3) -> bool {
    mob.damage += damage;
    mob.hurt_ticks = MOB_INVULNERABILITY_TICKS;
    // A plain blow, unless the striker says otherwise
    mob.struck = None;
    // A boss is not moved by a blow
    if !mob.kind.is_boss() {
        mob.velocity = push + Vec3::Y * KNOCK_SPEED;
        mob.on_ground = false;
    }
    if mob.kind.is_hostile() {
        super::hostile::provoke(mob, striker);
    } else {
        mob.action = MobAction::Flee;
        mob.action_ticks = FLEE_TICKS;
        mob.target = MobTarget::Player(striker);
    }
    mob.damage >= mob.max_health() && !mob.kind.is_boss()
}

/// Drops `drops` where a mob was.
pub fn drop_loot(items: &mut DroppedItems, drops: Vec<(ItemId, u32)>, position: Vec3) {
    for (item_id, nb) in drops {
        let stack = ItemStack {
            item_id,
            item_type: item_id.get_default_type(),
            nb,
        };
        items.spawn(DroppedItem::new(stack, position, Vec3::Y * KNOCK_SPEED));
    }
}

/// What `mob` drops when it dies: the loot of its kind (`kind_loot`), as many times as the strength
/// of its depth tier, the rest of which is a chance of one time more; and in the deep tiers
/// sometimes a treasure.
pub fn loot(mob: &ServerMob, rng: &mut impl Rng) -> Vec<(ItemId, u32)> {
    let mut drops = kind_loot(mob, rng);
    // What it wore: its lead, its armour, its chest
    if mob.leash.is_some() {
        drops.push((ItemId::Lead, 1));
    }
    if let Some(armour) = mob.horse_armour {
        drops.push((armour, 1));
    }
    if mob.pack.is_some() {
        drops.push((ItemId::Chest, 1));
    }
    let Some(tier) = mob.tier else {
        return drops;
    };
    for _ in 1..tier_times(tier, rng) {
        drops.extend(kind_loot(mob, rng));
    }
    match tier {
        DepthTier::Caves => {}
        DepthTier::Depths => {
            if rng.gen_ratio(1, 8) {
                drops.push((ItemId::IronIngot, 1));
            }
        }
        DepthTier::Furnace => {
            if rng.gen_ratio(1, 6) {
                drops.push((ItemId::GoldIngot, 1));
            }
            if rng.gen_ratio(1, 20) {
                drops.push((ItemId::Diamond, 1));
            }
        }
    }
    drops
}

/// How many times what happens once in the caves happens in `tier`: once per 100 % of its
/// strength, the rest being the chance of one time more.
pub fn tier_times(tier: DepthTier, rng: &mut impl Rng) -> u32 {
    let strength = tier.strength();
    strength / 100 + u32::from(rng.gen_range(0..100) < strength % 100)
}

/// What a mob of the kind of `mob` drops: its meat, and its wool, leather or feathers. Counts of zero
/// are left out.
fn kind_loot(mob: &ServerMob, rng: &mut impl Rng) -> Vec<(ItemId, u32)> {
    // A young animal leaves nothing
    if mob.is_young() {
        return vec![];
    }
    let mut drops = match mob.kind {
        MobKind::Sheep => vec![
            (ItemId::RawMutton, rng.gen_range(1..=2)),
            (ItemId::Wool, u32::from(!mob.sheared)),
        ],
        MobKind::Pig => vec![(ItemId::RawPork, rng.gen_range(1..=3))],
        MobKind::Cow => vec![
            (ItemId::RawBeef, rng.gen_range(1..=3)),
            (ItemId::Leather, rng.gen_range(0..=2)),
        ],
        MobKind::Chicken => vec![
            (ItemId::RawChicken, 1),
            (ItemId::Feather, rng.gen_range(0..=2)),
        ],
        MobKind::Rabbit => vec![(ItemId::RawRabbit, 1)],
        MobKind::Bramble => vec![(ItemId::Stick, rng.gen_range(1..=3))],
        MobKind::Stumpling => vec![
            (ItemId::Stick, rng.gen_range(2..=4)),
            (ItemId::Charcoal, u32::from(rng.gen_bool(0.3))),
        ],
        MobKind::Roller => vec![
            (ItemId::Cobblestone, rng.gen_range(1..=2)),
            (ItemId::Flint, u32::from(rng.gen_ratio(1, 5))),
        ],
        MobKind::GustOwl => vec![(ItemId::Feather, rng.gen_range(1..=3))],
        MobKind::Cinderling => vec![
            (ItemId::Coal, rng.gen_range(1..=2)),
            (ItemId::Flint, u32::from(rng.gen_ratio(1, 10))),
        ],
        MobKind::EmberMother => vec![(ItemId::EmberHeart, 1)],
        // The fish of the seas give those of the rods (`world::fishing`)
        MobKind::Glintfin if rng.gen_ratio(1, 4) => vec![(ItemId::RawGlowfin, 1)],
        MobKind::Glintfin => vec![(ItemId::RawRoach, 1)],
        MobKind::Shoalback => vec![(ItemId::RawPerch, 1)],
        MobKind::Mossback => vec![
            (ItemId::Moss, rng.gen_range(1..=2)),
            (ItemId::Leather, u32::from(rng.gen_ratio(1, 3))),
        ],
        MobKind::Shardling => vec![(ItemId::CrystalShard, rng.gen_range(1..=3))],
        MobKind::Slagcrawler => vec![
            (ItemId::Coal, rng.gen_range(0..=2)),
            (ItemId::Magma, u32::from(rng.gen_ratio(1, 4))),
        ],
        // It breaks into the item it was put down from
        MobKind::Boat => vec![(ItemId::Boat, 1)],
        MobKind::Horse | MobKind::Donkey | MobKind::Mule => vec![
            (ItemId::Leather, rng.gen_range(0..=2)),
            (ItemId::Saddle, u32::from(mob.saddled)),
        ],
        // A part of the creature, which distils into strength (`alchemy`)
        MobKind::Gnawer => vec![(ItemId::GnawerFang, u32::from(rng.gen_ratio(1, 2)))],
        MobKind::Fox
        | MobKind::Wolf
        | MobKind::Goat
        | MobKind::Cat
        | MobKind::Drowner
        | MobKind::ThrownPotion
        | MobKind::Ember
        | MobKind::Arrow
        | MobKind::Bobber
        | MobKind::Farmer
        | MobKind::Smith
        | MobKind::Cartographer => vec![],
    };
    drops.retain(|&(_, nb)| nb > 0);
    drops
}

/// The nearest mob the player aims at, within its reach and before any block.
fn aimed_mob(
    player: &Player,
    input: &PlayerFrameInput,
    held: Option<ItemId>,
    mobs: &HashMap<MobId, ServerMob>,
    chunks: &ChunkMap,
) -> Option<MobId> {
    let origin = match input.view_mode {
        ViewMode::FirstPerson => input.camera.translation,
        ViewMode::ThirdPerson => player.position,
    };
    // The camera comes from the network
    if !origin.is_finite() || origin.distance(player.position) > 3.0 {
        return None;
    }
    let direction = input.camera.forward().as_vec3();
    let inverse = 1.0 / direction;
    let distance = |hitbox: &Aabb3d| {
        aabb_ray_hit(hitbox, &origin, &direction, &inverse).map(|(hit, _)| hit.distance(origin))
    };
    let reach = if held == Some(ItemId::Spear) {
        SPEAR_REACH
    } else {
        REACH
    };
    let block = aim(
        chunks,
        &input.camera,
        &player.position,
        input.view_mode,
        None,
    )
    .and_then(|target| distance(&target.bbox))
    .unwrap_or(f32::INFINITY);
    mobs.iter()
        .filter(|(_, mob)| !mob.kind.is_projectile())
        // Not the mount it rides
        .filter(|(id, _)| player.riding.is_none_or(|riding| riding.mob != **id))
        .filter_map(|(id, mob)| {
            let half = Vec3::new(mob.width, mob.height, mob.depth) / 2.0;
            Some((*id, distance(&Aabb3d::new(mob.position, half))?))
        })
        .filter(|&(_, d)| d <= reach && d < block)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(id, _)| id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::{rngs::StdRng, SeedableRng};
    use shared::world::ItemType;

    const AHEAD: Vec3 = Vec3::new(0.5, 0.5, -2.0);

    /// A player at the origin looking north, at a mob two blocks ahead, `held` in its first slot.
    fn fighter(id: u64, held: Option<ItemId>) -> Player {
        let camera = Transform::from_translation(Vec3::splat(0.5));
        let mut player = Player::new(id, "tester".into(), Vec3::splat(0.5), camera);
        if let Some(item_id) = held {
            let stack = ItemStack {
                item_id,
                item_type: item_id.get_default_type(),
                nb: 1,
            };
            player.inventory.inner.insert(0, stack);
        }
        player
    }

    /// The mobs, what they dropped, and what changes them.
    struct Arena {
        mobs: HashMap<MobId, ServerMob>,
        items: DroppedItems,
        chunks: ChunkMap,
        changed: HashSet<MobId>,
        recharges: Recharges,
        feed_waits: BlowWaits,
        trades: Trades,
        rng: StdRng,
    }

    impl Arena {
        fn with(mob: ServerMob) -> Self {
            Self {
                mobs: HashMap::from([(7, mob)]),
                items: DroppedItems::default(),
                chunks: ChunkMap::with_loaded_region(IVec3::splat(-16), IVec3::splat(15)),
                changed: HashSet::new(),
                recharges: Recharges::new(),
                feed_waits: BlowWaits::new(),
                trades: Trades::default(),
                rng: StdRng::seed_from_u64(1),
            }
        }

        /// One tick of `player`, `action` pressed or not; the mobs' invulnerability counts down
        /// as `step_mobs` does.
        fn tick(&mut self, player: &mut Player, action: &NetworkAction, pressed: bool) {
            let input = PlayerFrameInput {
                delta_ms: 50,
                inputs: pressed.then(|| action.clone()).into_iter().collect(),
                camera: player.camera_transform,
                ..Default::default()
            };
            let world = MobWorld {
                mobs: &mut self.mobs,
                changed: &mut self.changed,
                items: &mut self.items,
                chunks: &mut self.chunks,
                recharges: &mut self.recharges,
                feed_waits: &mut self.feed_waits,
                trades: &mut self.trades,
            };
            interact(player, &input, world, &mut self.rng);
            for mob in self.mobs.values_mut() {
                mob.hurt_ticks = mob.hurt_ticks.saturating_sub(1);
            }
        }

        /// `clicks` clicks of `action` by `player`, each held a tick then released long enough
        /// for the mob's invulnerability to end.
        fn click(&mut self, player: &mut Player, action: NetworkAction, clicks: u32) {
            for _ in 0..clicks {
                self.tick(player, &action, true);
                for _ in 0..MOB_INVULNERABILITY_TICKS {
                    self.tick(player, &action, false);
                }
            }
        }

        fn dropped(&self) -> Vec<ItemId> {
            self.items
                .to_saved()
                .iter()
                .map(|i| i.stack.item_id)
                .collect()
        }
    }

    #[test]
    fn a_full_blow_of_a_sword_on_the_ground_sweeps_the_mob_beside_its_target_too() {
        let mut arena = Arena::with(ServerMob::new(MobKind::Cow, AHEAD));
        arena
            .mobs
            .insert(8, ServerMob::new(MobKind::Pig, AHEAD + Vec3::X * 1.5));
        let mut player = fighter(1, Some(ItemId::IronSword));
        player.on_ground = true;
        arena.tick(&mut player, &NetworkAction::LeftClick, true);
        let damage = ItemId::IronSword.attack_damage();
        assert_eq!(arena.mobs[&7].damage, damage);
        assert_eq!(arena.mobs[&7].struck, Some(Strike::Sweep));
        assert_eq!(arena.mobs[&8].damage, melee::sweep_damage(damage));
        assert_eq!(arena.mobs[&8].struck, Some(Strike::Sweep));
    }

    #[test]
    fn a_full_blow_while_falling_is_critical_and_a_hurried_one_weak() {
        let mut arena = Arena::with(ServerMob::new(MobKind::Drowner, AHEAD));
        let mut player = fighter(1, Some(ItemId::IronAxe));
        (player.velocity.y, player.on_ground) = (-3.0, false);
        arena.tick(&mut player, &NetworkAction::LeftClick, true);
        let damage = ItemId::IronAxe.attack_damage();
        assert_eq!(arena.mobs[&7].damage, melee::blow_damage(damage, 1.0, true));
        assert_eq!(arena.mobs[&7].struck, Some(Strike::Critical));
        // Pressed again as soon as it may be hurt: a hurried blow
        for _ in 0..MOB_INVULNERABILITY_TICKS {
            arena.tick(&mut player, &NetworkAction::LeftClick, false);
        }
        let before = arena.mobs[&7].damage;
        arena.tick(&mut player, &NetworkAction::LeftClick, true);
        let charge = 550.0 / 1000.0;
        assert_eq!(
            arena.mobs[&7].damage - before,
            melee::blow_damage(damage, charge, false)
        );
        assert_eq!(arena.mobs[&7].struck, None);
    }

    #[test]
    fn a_stumpling_parries_from_its_front_but_an_axe_knocks_its_guard_aside() {
        // Facing the player
        let mut arena = Arena::with(ServerMob::new(MobKind::Stumpling, AHEAD));
        let mut player = fighter(1, Some(ItemId::IronSword));
        arena.click(&mut player, NetworkAction::LeftClick, 1);
        assert_eq!(arena.mobs[&7].damage, 0);
        assert_eq!(arena.mobs[&7].parries, 1);

        let mut player = fighter(1, Some(ItemId::IronAxe));
        arena.click(&mut player, NetworkAction::LeftClick, 1);
        assert!(arena.mobs[&7].damage > 0);
        assert_eq!(arena.mobs[&7].guard_down, GUARD_DOWN_TICKS);

        // Turned away, it is struck by anything
        let mut turned = ServerMob::new(MobKind::Stumpling, AHEAD);
        turned.rotation = Quat::from_rotation_y(std::f32::consts::PI);
        let mut arena = Arena::with(turned);
        arena.click(&mut fighter(1, Some(ItemId::IronSword)), NetworkAction::LeftClick, 1);
        assert!(arena.mobs[&7].damage > 0);
    }

    #[test]
    fn a_weakened_player_strikes_softer_and_a_mace_throws_back_farther() {
        use shared::players::effects::EffectKind;
        let mut arena = Arena::with(ServerMob::new(MobKind::Cow, AHEAD));
        let mut player = fighter(1, Some(ItemId::IronSword));
        player.give_effect(EffectKind::Weakness, 1, 10);
        arena.tick(&mut player, &NetworkAction::LeftClick, true);
        assert_eq!(
            arena.mobs[&7].damage,
            ItemId::IronSword.attack_damage() - 2
        );
        let sword_push = arena.mobs[&7].velocity.with_y(0.0).length();

        let mut arena = Arena::with(ServerMob::new(MobKind::Cow, AHEAD));
        arena.tick(&mut fighter(1, Some(ItemId::Mace)), &NetworkAction::LeftClick, true);
        assert!(arena.mobs[&7].velocity.with_y(0.0).length() > sword_push * 2.0);
    }

    #[test]
    fn a_cow_fills_a_bucket_with_milk() {
        let mut arena = Arena::with(ServerMob::new(MobKind::Cow, AHEAD));
        let mut player = fighter(1, Some(ItemId::Bucket));
        arena.click(&mut player, NetworkAction::RightClick, 1);
        assert_eq!(player.inventory.inner[&0].item_id, ItemId::MilkBucket);
    }

    #[test]
    fn an_ember_in_flight_is_not_struck() {
        let mut arena = Arena::with(ServerMob::new(MobKind::Ember, AHEAD));
        arena.click(&mut fighter(1, None), NetworkAction::LeftClick, 3);
        assert_eq!(arena.mobs[&7].damage, 0);
        assert_eq!(arena.mobs[&7].velocity, Vec3::ZERO);
    }

    /// What `clicks` clicks of `action` do to a sheep two blocks ahead.
    fn click(action: NetworkAction, clicks: u32) -> (HashMap<MobId, ServerMob>, Vec<ItemId>) {
        let mut arena = Arena::with(ServerMob::new(MobKind::Sheep, AHEAD));
        arena.click(&mut fighter(1, None), action, clicks);
        let dropped = arena.dropped();
        (arena.mobs, dropped)
    }

    #[test]
    fn blows_hurt_a_sheep_then_kill_it_for_its_meat_and_its_wool() {
        let (mobs, items) = click(NetworkAction::LeftClick, 7);
        assert_eq!(mobs[&7].damage, 7);
        assert!(items.is_empty());
        let (mobs, items) = click(NetworkAction::LeftClick, 8);
        assert!(mobs.is_empty());
        assert_eq!(items, [ItemId::RawMutton, ItemId::Wool]);
    }

    #[test]
    fn the_wool_of_a_sheep_is_taken_once() {
        let (mobs, items) = click(NetworkAction::RightClick, 3);
        assert!(mobs[&7].sheared);
        assert_eq!(items, [ItemId::Wool]);
    }

    #[test]
    fn a_mob_hit_ignores_blows_for_ten_ticks() {
        let mut arena = Arena::with(ServerMob::new(MobKind::Cow, AHEAD));
        let (mut first, mut second) = (fighter(1, None), fighter(2, None));
        let left = NetworkAction::LeftClick;
        arena.tick(&mut first, &left, true);
        assert_eq!(arena.mobs[&7].damage, 1);
        // Another player's blow, the very next tick and in the ninth, lands nothing
        arena.tick(&mut second, &left, true);
        for _ in 0..MOB_INVULNERABILITY_TICKS - 3 {
            arena.tick(&mut second, &left, false);
        }
        arena.tick(&mut second, &left, true);
        assert_eq!(arena.mobs[&7].damage, 1);
        // Ten ticks after the first, a third player's lands
        arena.tick(&mut fighter(3, None), &left, true);
        assert_eq!(arena.mobs[&7].damage, 2);
    }

    #[test]
    fn a_blow_knocks_the_mob_back_away_from_the_player_and_it_flees_for_five_seconds() {
        // The mob is ahead and to the right of the player, who looks at it
        let at = Vec3::new(2.5, 0.5, -2.0);
        let mut arena = Arena::with(ServerMob::new(MobKind::Pig, at));
        let mut player = fighter(3, None);
        player.camera_transform.look_at(at, Vec3::Y);
        arena.tick(&mut player, &NetworkAction::LeftClick, true);
        let pig = &arena.mobs[&7];
        let away = (at - player.position).with_y(0.0).normalize();
        let push = pig.velocity.with_y(0.0);
        assert!(
            (push - away * KNOCK_BACK_SPEED).length() < 1e-4,
            "{push} not {away} x {KNOCK_BACK_SPEED}"
        );
        assert_eq!(pig.velocity.y, KNOCK_SPEED);
        assert!(!pig.on_ground);
        assert_eq!(pig.action, MobAction::Flee);
        assert!(matches!(pig.target, MobTarget::Player(3)));
        assert_eq!(pig.action_ticks, FLEE_TICKS);
        assert_eq!(FLEE_TICKS, 5 * 20);
    }

    #[test]
    fn a_hostile_hit_takes_the_player_as_its_prey_instead_of_fleeing() {
        let mut arena = Arena::with(ServerMob::new(MobKind::Gnawer, AHEAD));
        let mut player = fighter(3, None);
        arena.tick(&mut player, &NetworkAction::LeftClick, true);
        let gnawer = &arena.mobs[&7];
        assert_eq!(gnawer.damage, 1);
        assert_ne!(gnawer.action, MobAction::Flee);
        assert!(matches!(gnawer.target, MobTarget::Player(3)));
        assert!(!gnawer.on_ground, "knocked back");
    }

    #[test]
    fn a_blow_that_lands_tires_the_player() {
        let mut arena = Arena::with(ServerMob::new(MobKind::Cow, AHEAD));
        let mut player = fighter(1, None);
        let left = NetworkAction::LeftClick;
        arena.tick(&mut player, &left, true);
        assert_eq!(player.exhaustion, BLOW_EXHAUSTION);
        // A blow on the invulnerable cow lands nothing, and does not tire
        arena.tick(&mut player, &left, false);
        arena.tick(&mut player, &left, true);
        assert_eq!(player.exhaustion, BLOW_EXHAUSTION);
    }

    #[test]
    fn a_blow_wears_a_sword_by_one_and_a_pickaxe_by_two() {
        for (item, wear) in [(ItemId::IronSword, 1), (ItemId::IronPickaxe, 2)] {
            let mut arena = Arena::with(ServerMob::new(MobKind::Horse, AHEAD));
            let mut player = fighter(1, Some(item));
            arena.click(&mut player, NetworkAction::LeftClick, 2);
            assert_eq!(
                player.inventory.inner[&0].item_type,
                ItemType::Tool { wear: 2 * wear },
                "{item:?}"
            );
        }
    }

    #[test]
    fn a_cow_held_rye_eats_one_and_wants_a_mate_and_a_beef_is_not_for_it() {
        let mut arena = Arena::with(ServerMob::new(MobKind::Cow, AHEAD));
        let mut player = fighter(1, Some(ItemId::Rye));
        player.inventory.inner.get_mut(&0).unwrap().nb = 3;
        // Held down, it eats once
        for _ in 0..5 {
            arena.tick(&mut player, &NetworkAction::RightClick, true);
        }
        assert_eq!(player.inventory.inner[&0].nb, 2);
        assert!(arena.mobs[&7].in_love > 0);
        assert!(arena.changed.contains(&7));
        // Wanting a mate already, it eats no more
        arena.click(&mut player, NetworkAction::RightClick, 2);
        assert_eq!(player.inventory.inner[&0].nb, 2);

        let mut arena = Arena::with(ServerMob::new(MobKind::Cow, AHEAD));
        let mut player = fighter(1, Some(ItemId::RawBeef));
        arena.click(&mut player, NetworkAction::RightClick, 1);
        assert_eq!(arena.mobs[&7].in_love, 0);
        assert_eq!(player.inventory.inner[&0].nb, 1);
    }

    #[test]
    fn a_bow_being_drawn_is_not_let_go_by_a_villager_it_sweeps_over() {
        for (drawn_ms, opens) in [(0, true), (500, false)] {
            let mut arena = Arena::with(ServerMob::new(MobKind::Farmer, AHEAD));
            let mut player = fighter(1, Some(ItemId::Bow));
            player.bow_draw_ms = drawn_ms;
            arena.tick(&mut player, &NetworkAction::RightClick, true);
            assert_eq!(arena.trades.open.contains_key(&1), opens, "{drawn_ms}");
        }
    }

    #[test]
    fn a_young_sheep_is_not_shorn_and_a_young_animal_leaves_nothing() {
        let young = ServerMob::young(MobKind::Sheep, AHEAD);
        let mut arena = Arena::with(young.clone());
        arena.click(&mut fighter(1, None), NetworkAction::RightClick, 1);
        assert!(!arena.mobs[&7].sheared);
        assert!(loot(&young, &mut StdRng::seed_from_u64(1)).is_empty());
    }

    #[test]
    fn each_animal_drops_its_loot() {
        let mut rng = StdRng::seed_from_u64(5);
        // What a kind dropped over many deaths: each item, the fewest and the most at once
        let mut drops = |kind| {
            let mut seen: HashMap<ItemId, (u32, u32)> = HashMap::new();
            let mut deaths = 0;
            for _ in 0..200 {
                let dropped = loot(&ServerMob::new(kind, AHEAD), &mut rng);
                deaths += 1;
                for (item, nb) in dropped {
                    assert!(nb > 0, "{kind:?} dropped no {item:?}");
                    let (low, high) = seen.entry(item).or_insert((nb, nb));
                    (*low, *high) = ((*low).min(nb), (*high).max(nb));
                }
            }
            assert_eq!(deaths, 200);
            let mut seen: Vec<_> = seen.into_iter().collect();
            seen.sort();
            seen
        };
        use ItemId::*;
        assert_eq!(drops(MobKind::Pig), [(RawPork, (1, 3))]);
        assert_eq!(drops(MobKind::Cow), [(RawBeef, (1, 3)), (Leather, (1, 2))]);
        assert_eq!(
            drops(MobKind::Chicken),
            [(RawChicken, (1, 1)), (Feather, (1, 2))]
        );
        assert_eq!(drops(MobKind::Rabbit), [(RawRabbit, (1, 1))]);
        assert_eq!(drops(MobKind::Sheep), [(Wool, (1, 1)), (RawMutton, (1, 2))]);
        // A horse leaves its hide, and its saddle when it wore one; a boat breaks into its item
        assert_eq!(drops(MobKind::Horse), [(Leather, (1, 2))]);
        assert_eq!(drops(MobKind::Boat), [(Boat, (1, 1))]);
        let saddled = ServerMob {
            saddled: true,
            ..ServerMob::new(MobKind::Horse, AHEAD)
        };
        assert!(loot(&saddled, &mut StdRng::seed_from_u64(9)).contains(&(Saddle, 1)));
        for kind in [
            MobKind::Fox,
            MobKind::Wolf,
            MobKind::Goat,
            MobKind::Cat,
            MobKind::Drowner,
            MobKind::Ember,
        ] {
            assert_eq!(drops(kind), [], "{kind:?}");
        }
        // Its fang, one time in two, for the potions of strength
        assert_eq!(drops(MobKind::Gnawer), [(GnawerFang, (1, 1))]);
        assert_eq!(drops(MobKind::Bramble), [(Stick, (1, 3))]);
        assert_eq!(
            drops(MobKind::Stumpling),
            [(Stick, (2, 4)), (Charcoal, (1, 1))]
        );
        assert_eq!(
            drops(MobKind::Roller),
            [(Cobblestone, (1, 2)), (Flint, (1, 1))]
        );
        assert_eq!(drops(MobKind::GustOwl), [(Feather, (1, 3))]);
        assert_eq!(
            drops(MobKind::Cinderling),
            [(Coal, (1, 2)), (Flint, (1, 1))]
        );
        let flints = (0..1000)
            .filter(|_| {
                loot(&ServerMob::new(MobKind::Roller, AHEAD), &mut rng).contains(&(Flint, 1))
            })
            .count();
        assert!((150..250).contains(&flints), "{flints} flints in 1000");
        let cinder_flints = (0..1000)
            .filter(|_| {
                loot(&ServerMob::new(MobKind::Cinderling, AHEAD), &mut rng).contains(&(Flint, 1))
            })
            .count();
        assert!(
            (60..140).contains(&cinder_flints),
            "{cinder_flints} flints in 1000"
        );
        let charcoals = (0..1000)
            .filter(|_| {
                loot(&ServerMob::new(MobKind::Stumpling, AHEAD), &mut rng).contains(&(Charcoal, 1))
            })
            .count();
        assert!(
            (250..350).contains(&charcoals),
            "{charcoals} charcoals in 1000"
        );
        let sheared = ServerMob {
            sheared: true,
            ..ServerMob::new(MobKind::Sheep, AHEAD)
        };
        assert!(loot(&sheared, &mut rng)
            .iter()
            .all(|&(item, _)| item == RawMutton));
    }

    #[test]
    fn a_hostile_of_the_furnace_is_twice_as_sturdy_and_leaves_twice_as_much() {
        let mob = ServerMob::new(MobKind::Roller, AHEAD).in_tier(Some(DepthTier::Furnace));
        assert_eq!(mob.max_health(), 2 * MobKind::Roller.health());
        assert_eq!(mob.experience(), 2 * MobKind::Roller.experience());
        let mut rng = StdRng::seed_from_u64(5);
        let mut gold = 0;
        for _ in 0..50 {
            let drops = loot(&mob, &mut rng);
            let nb = |id| -> u32 { drops.iter().filter(|d| d.0 == id).map(|d| d.1).sum() };
            assert!((2..=4).contains(&nb(ItemId::Cobblestone)));
            gold += nb(ItemId::GoldIngot);
        }
        assert!(gold > 0);
    }
}
