//! The floats of the fishing rods (`shared::world::fishing`): a press of the right button with
//! a rod in hand casts one, another draws it in. A float flies like a thrown stone, floats on
//! the water it falls into, and after a random wait a fish bites: it plunges for a moment
//! (`MobAction::Attack`, which the clients see, hear and splash). Drawn in then, the line brings
//! back a catch, which drops at the feet of the angler. A float is a mob of its own kind, a
//! projectile: the other mob systems leave it alone, and the save leaves it out.

use std::collections::{HashMap, HashSet};

use bevy::prelude::*;
use rand::Rng;
use shared::{
    messages::{NetworkAction, PlayerFrameInput, PlayerId},
    players::{constants::EYE_HEIGHT, movement::is_terrain_loaded, Player},
    world::{
        fishing::{
            catch, Bobber, BITE_DIP, BITE_TICKS, BITE_WAIT_TICKS, CAST_SPEED, CATCH_WEAR,
            LINE_LENGTH, SNAG_WEAR,
        },
        hostiles::is_night,
        BlockHitbox, BlockId, ChunkMap, DroppedItem, ItemId, ItemStack, MobAction, MobId, MobKind,
        ServerMob,
    },
};

use crate::init::ServerTime;
use crate::world::data::ServerWorldMap;
use crate::world::dropped_items::DroppedItems;

/// The pull on a float in flight, in blocks/s².
const GRAVITY: f32 = -20.0;
/// What a float keeps of its speed over a second of flight.
const AIR_DRAG: f32 = 0.6;
/// How deep a float sinks into the top cell of the water, in blocks.
const FLOAT_DEPTH: f32 = 0.15;

/// The float of each angler, and whether its right button was down at its last input.
#[derive(Default, Debug)]
pub struct Lines {
    floats: HashMap<PlayerId, MobId>,
    pressed: HashSet<PlayerId>,
}

impl Lines {
    /// Whether the right button of `input` was pressed down by `player` since its last input.
    fn press(&mut self, player: PlayerId, input: &PlayerFrameInput) -> bool {
        if input.is_pressed(NetworkAction::RightClick) {
            self.pressed.insert(player)
        } else {
            self.pressed.remove(&player);
            false
        }
    }
}

/// Whether `player` holds a fishing rod in its hand.
fn holds_rod(player: &Player) -> bool {
    player
        .inventory
        .inner
        .get(&player.hotbar_slot)
        .is_some_and(|stack| stack.item_id == ItemId::FishingRod)
}

/// Casts a float, or draws it in, at a press of the right button with a rod in hand.
pub fn use_rod(
    lines: &mut Lines,
    player: &mut Player,
    input: &PlayerFrameInput,
    mobs: &mut HashMap<MobId, ServerMob>,
    changed: &mut HashSet<MobId>,
    items: &mut DroppedItems,
    rng: &mut impl Rng,
) {
    if !lines.press(player.id, input) || player.is_dead() {
        return;
    }
    let slot = input.hotbar_slot;
    let holds = player
        .inventory
        .inner
        .get(&slot)
        .is_some_and(|stack| stack.item_id == ItemId::FishingRod);
    if !holds {
        return;
    }
    if let Some(id) = lines.floats.remove(&player.id) {
        if let Some(float) = mobs.remove(&id) {
            changed.remove(&id);
            let wear = draw_in(&float, player, items, rng);
            player.inventory.wear_tool(slot, wear);
            return;
        }
    }
    let forward = input.camera.forward().as_vec3();
    let eyes = player.position + Vec3::Y * EYE_HEIGHT;
    let float = ServerMob {
        velocity: forward * CAST_SPEED + Vec3::Y * 2.0,
        // The clients draw its line to the angler
        owner: Some(player.name.clone()),
        action: MobAction::Walk,
        bobber: Some(Bobber {
            angler: player.id,
            in_water: false,
            night: false,
        }),
        ..ServerMob::new(MobKind::Bobber, eyes + forward * 0.6)
    };
    let id = super::add_mob(mobs, changed, float);
    lines.floats.insert(player.id, id);
}

/// What drawing in `float` brings `player`: a catch while a fish bites, which drops at its feet;
/// nothing otherwise. Returns the wear it adds to the rod.
fn draw_in(
    float: &ServerMob,
    player: &Player,
    items: &mut DroppedItems,
    rng: &mut impl Rng,
) -> u32 {
    let Some(bobber) = float.bobber else {
        return 0;
    };
    if bobber.in_water && float.action == MobAction::Attack {
        let (item_id, nb) = catch(bobber.night, rng);
        let stack = ItemStack {
            item_id,
            item_type: item_id.get_default_type(),
            nb,
        };
        items.spawn(DroppedItem {
            pickup_delay_ticks: 0,
            ..DroppedItem::new(stack, player.position, Vec3::ZERO)
        });
        items.drop_orb(rng.gen_range(1..=3), player.position, rng);
        return CATCH_WEAR;
    }
    if !bobber.in_water && float.action == MobAction::Idle {
        // Caught on the ground
        return SNAG_WEAR;
    }
    0
}

/// Whether the cell at `pos` holds water.
fn is_water(chunks: &ChunkMap, pos: IVec3) -> bool {
    chunks
        .get_block(pos)
        .is_some_and(|block| block.id == BlockId::Water)
}

/// Whether the cell at `pos` holds a block a float lands on.
fn is_solid(chunks: &ChunkMap, pos: IVec3) -> bool {
    chunks.get_block(pos).is_some_and(|block| {
        block.id != BlockId::Water && !matches!(block.get_hitbox(), BlockHitbox::None)
    })
}

/// Where a float in the water at `cell` floats: on the top cell of its water.
fn surface(chunks: &ChunkMap, cell: IVec3) -> f32 {
    let mut top = cell;
    while is_water(chunks, top + IVec3::Y) && top.y < cell.y + 16 {
        top += IVec3::Y;
    }
    top.y as f32 + 1.0 - FLOAT_DEPTH
}

/// A wait for a bite, drawn from `rng`.
fn wait(rng: &mut impl Rng) -> u32 {
    rng.gen_range(BITE_WAIT_TICKS)
}

/// One step of every float: it flies, floats, waits for a bite and bites; one whose angler is
/// gone, dead, far or no longer holds a rod is drawn in for nothing.
pub fn fishing_system(
    mut world_map: ResMut<ServerWorldMap>,
    time: Res<Time<Fixed>>,
    tick: Res<ServerTime>,
) {
    let map = world_map.bypass_change_detection();
    let night = is_night(tick.0);
    let delta = time.delta_secs();
    let mut rng = rand::thread_rng();
    let ids: Vec<_> = map
        .mobs
        .iter()
        .filter(|(_, mob)| mob.kind == MobKind::Bobber)
        .map(|(id, _)| *id)
        .collect();
    for id in ids {
        let Some(float) = map.mobs.get_mut(&id) else {
            continue;
        };
        let Some(mut bobber) = float.bobber else {
            map.mobs.remove(&id);
            continue;
        };
        let held = map.players.get(&bobber.angler).is_some_and(|angler| {
            !angler.is_dead()
                && holds_rod(angler)
                && angler.position.distance(float.position) <= LINE_LENGTH
        });
        if !held || !is_terrain_loaded(float.position, &map.chunks) {
            map.mobs.remove(&id);
            map.changed_mobs.remove(&id);
            if map.fishing.floats.get(&bobber.angler) == Some(&id) {
                map.fishing.floats.remove(&bobber.angler);
            }
            continue;
        }
        bobber.night = night;
        let before = (float.position, float.action);
        step(float, &mut bobber, &map.chunks, delta, &mut rng);
        float.bobber = Some(bobber);
        if before != (float.position, float.action) {
            map.changed_mobs.insert(id);
        }
    }
    // A float drawn in or lost leaves its angler free to cast another
    let mobs = &map.mobs;
    map.fishing.floats.retain(|_, id| mobs.contains_key(id));
}

/// One step of `float`.
fn step(
    float: &mut ServerMob,
    bobber: &mut Bobber,
    chunks: &ChunkMap,
    delta: f32,
    rng: &mut impl Rng,
) {
    let cell = float.position.floor().as_ivec3();
    match float.action {
        MobAction::Walk => {
            float.velocity += Vec3::Y * GRAVITY * delta;
            float.velocity *= AIR_DRAG.powf(delta);
            let moved = float.velocity * delta;
            let steps = (moved.length() / 0.2).ceil().max(1.0) as u32;
            for _ in 0..steps {
                let next = float.position + moved / steps as f32;
                let next_cell = next.floor().as_ivec3();
                if is_water(chunks, next_cell) {
                    float.position = next.with_y(surface(chunks, next_cell));
                    float.velocity = Vec3::ZERO;
                    float.action = MobAction::Idle;
                    float.action_ticks = wait(rng);
                    bobber.in_water = true;
                    return;
                }
                if is_solid(chunks, next_cell) {
                    // It lies where it struck
                    float.velocity = Vec3::ZERO;
                    float.action = MobAction::Idle;
                    bobber.in_water = false;
                    return;
                }
                float.position = next;
            }
        }
        MobAction::Idle if bobber.in_water => {
            if !is_water(chunks, cell) {
                // Its water is gone: it falls
                float.action = MobAction::Walk;
                bobber.in_water = false;
                return;
            }
            float.action_ticks = float.action_ticks.saturating_sub(1);
            if float.action_ticks == 0 {
                // A fish bites: the float plunges
                float.action = MobAction::Attack;
                float.action_ticks = rng.gen_range(BITE_TICKS);
                float.position.y -= BITE_DIP;
            }
        }
        MobAction::Idle => {
            let below = (float.position - Vec3::Y * (float.height / 2.0 + 0.05))
                .floor()
                .as_ivec3();
            if !is_solid(chunks, below) && !is_solid(chunks, cell) {
                float.action = MobAction::Walk;
            }
        }
        _ => {
            float.action_ticks = float.action_ticks.saturating_sub(1);
            if float.action_ticks == 0 {
                // The fish got away
                float.action = MobAction::Idle;
                float.action_ticks = wait(rng);
                float.position.y += BITE_DIP;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::{rngs::StdRng, SeedableRng};
    use shared::world::BlockData;

    /// A pond of water, its top cells at y = 0, over chunks around the origin.
    fn pond() -> ChunkMap {
        let mut world = ChunkMap::with_loaded_region(IVec3::splat(-16), IVec3::splat(31));
        world.fill_box(
            IVec3::new(-16, -4, -16),
            IVec3::new(31, 0, 31),
            BlockId::Water,
        );
        world
    }

    fn float_at(position: Vec3) -> (ServerMob, Bobber) {
        let bobber = Bobber {
            angler: 1,
            in_water: false,
            night: false,
        };
        let float = ServerMob {
            action: MobAction::Walk,
            velocity: Vec3::new(3.0, 0.0, 0.0),
            ..ServerMob::new(MobKind::Bobber, position)
        };
        (float, bobber)
    }

    #[test]
    fn a_float_cast_over_water_floats_on_it_then_a_fish_bites_and_gets_away() {
        let world = pond();
        let mut rng = StdRng::seed_from_u64(1);
        let (mut float, mut bobber) = float_at(Vec3::new(2.0, 3.0, 2.0));
        for _ in 0..40 {
            step(&mut float, &mut bobber, &world, 0.05, &mut rng);
        }
        assert!(bobber.in_water);
        assert_eq!(float.action, MobAction::Idle);
        assert!((float.position.y - (1.0 - FLOAT_DEPTH)).abs() < 1e-4);
        let mut bit = false;
        for _ in 0..BITE_WAIT_TICKS.end + 1 {
            step(&mut float, &mut bobber, &world, 0.05, &mut rng);
            bit |= float.action == MobAction::Attack;
            if bit && float.action == MobAction::Idle {
                break;
            }
        }
        assert!(bit, "a fish bites within the longest wait");
        assert_eq!(
            float.action,
            MobAction::Idle,
            "and gets away unless drawn in"
        );
        assert!((float.position.y - (1.0 - FLOAT_DEPTH)).abs() < 1e-4);
    }

    #[test]
    fn a_float_cast_on_the_ground_lies_there_and_never_bites() {
        let mut world = ChunkMap::with_loaded_region(IVec3::splat(-16), IVec3::splat(31));
        world.fill_box(
            IVec3::new(-16, -4, -16),
            IVec3::new(31, 0, 31),
            BlockId::Stone,
        );
        world.set_block(
            IVec3::new(8, 1, 8),
            BlockData::new(BlockId::Stone, shared::world::BlockDirection::Front),
        );
        let mut rng = StdRng::seed_from_u64(2);
        let (mut float, mut bobber) = float_at(Vec3::new(2.0, 3.0, 2.0));
        for _ in 0..1000 {
            step(&mut float, &mut bobber, &world, 0.05, &mut rng);
            assert_ne!(float.action, MobAction::Attack);
        }
        assert!(!bobber.in_water);
        assert_eq!(float.action, MobAction::Idle);
    }
}
