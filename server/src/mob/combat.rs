//! What the players do to the mobs they aim at: blows, and taking the wool of a sheep.

use bevy::prelude::*;
use rand::Rng;
use shared::{
    messages::{NetworkAction, PlayerFrameInput},
    players::{
        blocks::{aim, held_item},
        Player, ViewMode,
    },
    world::{
        aabb_ray_hit, DroppedItem, FaceDirectionExt, ItemId, ItemStack, MobAction, MobId, MobKind,
        MobTarget, ServerMob,
    },
};
use std::collections::{HashMap, HashSet};

use bevy::math::bounding::Aabb3d;
use shared::world::ChunkMap;

use crate::world::dropped_items::DroppedItems;

/// How far from its eyes a player reaches a mob, in blocks, and with a spear.
const REACH: f32 = 3.0;
const SPEAR_REACH: f32 = 5.0;
/// Time between two blows while the button is held.
const BLOW_MS: u64 = 500;
/// Upward speed of a mob that takes a blow.
const KNOCK_SPEED: f32 = 4.0;

/// Time left before the next blow of each player, while it holds the button.
pub type BlowWaits = HashMap<shared::messages::PlayerId, u64>;

/// The mobs, and what their changes touch.
pub struct MobWorld<'a> {
    pub mobs: &'a mut HashMap<MobId, ServerMob>,
    pub changed: &'a mut HashSet<MobId>,
    pub items: &'a mut DroppedItems,
    pub chunks: &'a mut ChunkMap,
    pub waits: &'a mut BlowWaits,
}

/// Runs the clicks of `input` on the mob the player aims at, if any, and takes them out of the
/// input: they are not for the block behind it, whose state is told again to the client.
pub fn interact(
    player: &mut Player,
    input: &PlayerFrameInput,
    world: MobWorld,
    rng: &mut impl Rng,
) -> Option<PlayerFrameInput> {
    let left = input.is_pressed(NetworkAction::LeftClick);
    let right = input.is_pressed(NetworkAction::RightClick);
    let wait = world.waits.entry(player.id).or_default();
    *wait = if left {
        wait.saturating_sub(input.delta_ms.min(100))
    } else {
        0
    };
    if player.is_dead() || !(left || right) {
        return None;
    }
    let held = held_item(player, input);
    let id = aimed_mob(player, input, held, world.mobs, world.chunks)?;
    let mob = world.mobs.get_mut(&id)?;
    let shears = right
        && mob.kind == MobKind::Sheep
        && !held.is_some_and(|i| i.spawned_mob().is_some() || i.food_health().is_some());
    if !left && !shears {
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
    let position = mob.position;
    if left {
        rest.inputs.remove(&NetworkAction::LeftClick);
        if *wait == 0 {
            *wait = BLOW_MS;
            mob.damage += held.map_or(1, |item| item.attack_damage());
            mob.velocity.y = KNOCK_SPEED;
            mob.on_ground = false;
            mob.action = MobAction::Flee;
            mob.target = MobTarget::Player(player.id);
            world.changed.insert(id);
            if held.is_some_and(|item| item.tool().is_some()) {
                player.inventory.wear_tool(input.hotbar_slot, 1);
            }
            if mob.damage >= mob.kind.health() {
                if mob.kind == MobKind::Sheep {
                    drops.push((ItemId::RawMutton, rng.gen_range(1..=2)));
                    if !mob.sheared {
                        drops.push((ItemId::Wool, 1));
                    }
                }
                // The clients are told by the next broadcast, which no longer finds it
                world.mobs.remove(&id);
                world.changed.remove(&id);
            }
        }
    }
    for (item_id, nb) in drops {
        let stack = ItemStack {
            item_id,
            item_type: item_id.get_default_type(),
            nb,
        };
        world
            .items
            .spawn(DroppedItem::new(stack, position, Vec3::Y * KNOCK_SPEED));
    }
    Some(rest)
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

    /// What `clicks` clicks of `action`, each released, do to a sheep two blocks ahead.
    fn click(action: NetworkAction, clicks: u32) -> (HashMap<MobId, ServerMob>, Vec<ItemId>) {
        let camera = Transform::from_translation(Vec3::splat(0.5));
        let mut player = Player::new(1, "tester".into(), Vec3::splat(0.5), camera);
        let sheep = ServerMob::new(MobKind::Sheep, Vec3::new(0.5, 0.5, -2.0));
        let mut mobs = HashMap::from([(7, sheep)]);
        let mut items = DroppedItems::default();
        let mut chunks = ChunkMap::with_loaded_region(IVec3::splat(-16), IVec3::splat(15));
        let (mut changed, mut waits) = (HashSet::new(), BlowWaits::new());
        let mut rng = StdRng::seed_from_u64(1);
        for pressed in (0..clicks).flat_map(|_| [true, false]) {
            let input = PlayerFrameInput {
                delta_ms: 50,
                inputs: pressed.then(|| action.clone()).into_iter().collect(),
                camera,
                ..Default::default()
            };
            let world = MobWorld {
                mobs: &mut mobs,
                changed: &mut changed,
                items: &mut items,
                chunks: &mut chunks,
                waits: &mut waits,
            };
            interact(&mut player, &input, world, &mut rng);
        }
        let dropped = items.to_saved();
        (mobs, dropped.iter().map(|i| i.stack.item_id).collect())
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
}
