//! How mobs float: every kind but the fliers and the Drowner rises in water until its head is out
//! (`float`), swims slower than it walks, climbs onto a bank a block high (`climb_out`), and
//! without anything else to do swims to the shore (`swim_ashore`).

use std::f32::consts::FRAC_PI_4;
use std::sync::LazyLock;

use bevy::math::{IVec2, Quat, Vec3};
use rand::Rng;
use shared::{
    players::constants::{GRAVITY, WATER_SPEED_MULTIPLIER},
    world::{
        liquid::liquid_height, BlockHitbox, ChunkMap, MobAction, MobKind, MobTarget, ServerMob,
    },
};

use super::behavior::{collides_at, ground_under, is_in_water, passer, WALK_TICKS};

/// How much of its height a floating mob has under water: its head stays out.
const WATERLINE: f32 = 0.6;
/// How fast a mob rises in water at most, in blocks/s. One that goes up faster leaps.
const RISE_SPEED: f32 = 2.5;
/// How fast it goes to its waterline per block away from it, per second.
const RISE_PER_BLOCK: f32 = 2.0;
/// How fast the water brings the velocity of a mob to that speed, per second: at once would stop
/// what falls into the water dead, and much slower it would bob for long.
const WATER_GRIP: f32 = 6.0;
/// Within this distance of its waterline, and this slow, a mob is left where it is, in blocks and
/// blocks/s: it is still rather than moved for ever.
const STILL: f32 = 0.03;
const STILL_SPEED: f32 = 0.5;
/// How far above its waterline a mob is still held by the water, in blocks.
const HOLD: f32 = 0.3;
/// How high above the surface a bank may be for a mob to climb on it, in blocks: the surface of
/// a source is a ninth under the top of its cell.
const BANK_HEIGHT: f32 = 1.15;
/// How much higher than the bank a mob leaps, in blocks, and the least speed of a leap, in
/// blocks/s: it outruns `RISE_SPEED`.
const LEAP_MARGIN: f32 = 0.15;
const LEAP_SPEED: f32 = 5.0;
/// How many cells above its box the surface over a sunk mob is looked for: higher, it only has
/// to rise.
const SURFACE_SCAN: i32 = 4;
/// How far a mob looks for the shore, horizontally, in blocks.
const SHORE_DISTANCE: i32 = 16;

/// The bottom of the box of `mob`.
fn feet(mob: &ServerMob) -> f32 {
    mob.position.y - mob.height / 2.0
}

/// The height of the surface of the water the box of `mob` is in, in the column of its centre,
/// if it is in water.
fn water_surface(mob: &ServerMob, chunks: &ChunkMap) -> Option<f32> {
    let at = |y: i32| mob.position.floor().as_ivec3().with_y(y);
    let is_water = |y: i32| is_in_water(chunks, at(y).as_vec3());
    let top = (mob.position.y + mob.height / 2.0).floor() as i32;
    let lowest = (feet(mob).floor() as i32..=top).find(|&y| is_water(y))?;
    let mut surface = lowest;
    while surface < top + SURFACE_SCAN && is_water(surface + 1) {
        surface += 1;
    }
    let height = surface as f32 + liquid_height(chunks, at(surface))?;
    (height > feet(mob)).then_some(height)
}

/// How far below its waterline `mob` is, in blocks, if it is in water and of a kind that floats
/// there: negative above it.
fn sunk(mob: &ServerMob, chunks: &ChunkMap) -> Option<f32> {
    if matches!(
        mob.kind,
        MobKind::Drowner | MobKind::Cinderling | MobKind::GustOwl
    ) {
        return None;
    }
    Some(water_surface(mob, chunks)? - WATERLINE * mob.height - feet(mob))
}

/// Whether the water carries `mob`, rather than the ground under shallower water.
pub(super) fn is_afloat(mob: &ServerMob, chunks: &ChunkMap) -> bool {
    sunk(mob, chunks).is_some_and(|sunk| sunk > -STILL * 2.0)
}

/// How fast a mob that walks at `speed` goes, `afloat` or not.
pub(super) fn swim_speed(afloat: bool, speed: f32) -> f32 {
    if afloat {
        speed * WATER_SPEED_MULTIPLIER
    } else {
        speed
    }
}

/// One step of the vertical motion of a mob the water holds, which also brakes one flung: its
/// velocity goes smoothly to the one that brings it to its waterline. Returns whether the water
/// holds it: else it falls, out of the water, over its waterline or leaping.
pub(super) fn float(mob: &mut ServerMob, chunks: &ChunkMap, delta: f32) -> bool {
    let Some(sunk) = sunk(mob, chunks) else {
        return false;
    };
    if sunk < -HOLD || mob.velocity.y > RISE_SPEED {
        return false;
    }
    let grip = 1.0 - (-WATER_GRIP * delta).exp();
    mob.velocity.x -= mob.velocity.x * grip;
    mob.velocity.z -= mob.velocity.z * grip;
    mob.on_ground = false;
    if sunk.abs() < STILL && mob.velocity.y.abs() < STILL_SPEED {
        mob.velocity.y = 0.0;
        return true;
    }
    let wished = (sunk * RISE_PER_BLOCK).min(RISE_SPEED);
    mob.velocity.y += (wished - mob.velocity.y) * grip;
    // A fast fall crosses no block, as out of the water
    let distance = mob.velocity.y * delta;
    let steps = (distance.abs() / super::behavior::MAX_FLIGHT_STEP)
        .ceil()
        .max(1.0);
    for _ in 0..steps as u32 {
        let moved = mob.position + Vec3::Y * (distance / steps);
        if chunks.portals().goes_into(mob.position, moved, passer(mob)) {
            mob.position = moved;
            break;
        }
        if collides_at(mob, chunks, moved) {
            mob.on_ground = mob.velocity.y <= 0.0;
            mob.velocity.y = 0.0;
            break;
        }
        mob.position = moved;
    }
    true
}

/// Makes a mob afloat that a block stops on its way `step` leap onto it, if it is a bank at most
/// `BANK_HEIGHT` above the surface with room on it. Returns whether it leaps.
pub(super) fn climb_out(mob: &mut ServerMob, chunks: &ChunkMap, step: Vec3) -> bool {
    let Some(surface) = water_surface(mob, chunks) else {
        return false;
    };
    let feet = feet(mob);
    let highest = (surface + BANK_HEIGHT).floor() as i32;
    for bank in feet.floor() as i32 + 1..=highest {
        let rise = bank as f32 - feet + LEAP_MARGIN;
        let risen = mob.position + Vec3::Y * rise;
        if collides_at(mob, chunks, risen) {
            return false;
        }
        if !collides_at(mob, chunks, risen + step) {
            mob.velocity.y = (-2.0 * GRAVITY * rise).sqrt().max(LEAP_SPEED);
            mob.on_ground = false;
            return true;
        }
    }
    false
}

/// The columns within `SHORE_DISTANCE`, nearest first.
static SHORE_OFFSETS: LazyLock<Vec<IVec2>> = LazyLock::new(|| {
    let r = SHORE_DISTANCE;
    let mut offsets: Vec<IVec2> = (-r..=r)
        .flat_map(|x| (-r..=r).map(move |z| IVec2::new(x, z)))
        .filter(|offset| offset.length_squared() <= r * r)
        .collect();
    offsets.sort_by_key(|offset| offset.length_squared());
    offsets
});

/// The nearest spot a mob afloat at `position` can climb on, the water at the cell height
/// `water`, with no wall on the straight way to it.
fn nearest_shore(chunks: &ChunkMap, position: Vec3, water: i32) -> Option<Vec3> {
    let cell = position.floor().as_ivec3();
    let is_wall = |x: i32, z: i32| {
        (water + 2..=water + 3).any(|y| {
            chunks
                .get_block(cell.with_x(x).with_z(z).with_y(y))
                .is_some_and(|block| !matches!(block.id.get_hitbox(), BlockHitbox::None))
        })
    };
    SHORE_OFFSETS.iter().find_map(|offset| {
        let (x, z) = (
            (cell.x + offset.x) as f32 + 0.5,
            (cell.z + offset.y) as f32 + 0.5,
        );
        let ground = ground_under(chunks, x, z, water)?;
        let spot = Vec3::new(x, ground, z);
        let way = spot - position;
        let steps = way.with_y(0.0).length().ceil().max(1.0) as u32;
        let blocked = (1..=steps).any(|step| {
            let along = position + way * (step as f32 / steps as f32);
            is_wall(along.x.floor() as i32, along.z.floor() as i32)
        });
        (!blocked).then_some(spot)
    })
}

/// Sends a mob afloat that goes nowhere to the nearest shore; with none in sight, straight on,
/// more or less, to look for one farther.
pub(super) fn swim_ashore(mob: &mut ServerMob, chunks: &ChunkMap, rng: &mut impl Rng) {
    let goes_on = mob.action == MobAction::Walk
        && mob.action_ticks > 1
        && matches!(mob.target, MobTarget::Position(spot)
            if (spot - mob.position).with_y(0.0).length() > 0.5);
    if goes_on {
        return;
    }
    let Some(surface) = water_surface(mob, chunks) else {
        return;
    };
    let spot =
        nearest_shore(chunks, mob.position, surface.ceil() as i32 - 1).unwrap_or_else(|| {
            let turn = Quat::from_rotation_y(rng.gen_range(-FRAC_PI_4..FRAC_PI_4));
            mob.position + turn * mob.rotation * Vec3::Z * SHORE_DISTANCE as f32
        });
    mob.action = MobAction::Walk;
    mob.target = MobTarget::Position(spot.with_y(spot.y + mob.height / 2.0));
    mob.action_ticks = WALK_TICKS;
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
    use shared::players::Player;
    use shared::world::BlockId;
    use shared::TICKS_PER_SECOND;
    use std::time::Duration;

    /// A pool of 12 x 12 and 6 deep, its surface in the cells at y = 0, in a stone ground whose
    /// top is at `bank`.
    fn pool(bank: i32) -> ServerWorldMap {
        let mut map = ServerWorldMap {
            chunks: ChunkMap::with_loaded_region(IVec3::new(-48, -32, -48), IVec3::new(47, 31, 47)),
            ..Default::default()
        };
        let top = bank - 1;
        for (low, high, block) in [
            (
                IVec3::new(-32, -8, -32),
                IVec3::new(31, 0, 31),
                BlockId::Stone,
            ),
            (
                IVec3::new(-32, 0, -32),
                IVec3::new(-7, top, 31),
                BlockId::Stone,
            ),
            (
                IVec3::new(6, 0, -32),
                IVec3::new(31, top, 31),
                BlockId::Stone,
            ),
            (
                IVec3::new(-6, 0, -32),
                IVec3::new(5, top, -7),
                BlockId::Stone,
            ),
            (IVec3::new(-6, 0, 6), IVec3::new(5, top, 31), BlockId::Stone),
            (IVec3::new(-6, -5, -6), IVec3::new(5, 0, 5), BlockId::Water),
        ] {
            map.chunks.fill_box(low, high, block);
        }
        map
    }

    /// Runs `ticks` of the mobs of `map`, calling `seen` after each with the mob 0.
    fn run(map: ServerWorldMap, ticks: u64, mut seen: impl FnMut(u64, &ServerMob)) {
        let mut world = World::new();
        world.insert_resource(map);
        let mut time = Time::<Fixed>::default();
        time.advance_by(Duration::from_secs_f64(1.0 / TICKS_PER_SECOND as f64));
        world.insert_resource(time);
        for tick in 0..ticks {
            world.run_system_once(mob_behavior_system).unwrap();
            seen(tick, &world.resource::<ServerWorldMap>().mobs[&0]);
        }
    }

    #[test]
    fn mobs_rise_float_still_and_climb_out_of_the_water() {
        // A Bramble goes nowhere: dropped into the water or sunk, it comes to rest, its head out
        for start in [8.0, -3.5] {
            let mut map = pool(1);
            let bramble = ServerMob::new(MobKind::Bramble, Vec3::new(0.5, start, 0.5));
            map.mobs.insert(0, bramble);
            let mut heights = Vec::new();
            run(map, 200, |_, mob| {
                heights.push(mob.position.y + mob.height / 2.0)
            });

            let end = heights[199];
            assert!(end > 1.0, "its top ends at {end}");
            assert!(
                heights[150..].iter().all(|y| *y == end),
                "{:?}",
                &heights[150..]
            );
        }
        // Animals find the shore, a block above the water or level with it, and stay on it; none
        // keeps its head under meanwhile
        for bank in [1, 2] {
            for kind in [MobKind::Cow, MobKind::Chicken, MobKind::Fox, MobKind::Horse] {
                let mut map = pool(bank);
                map.mobs
                    .insert(0, ServerMob::new(kind, Vec3::new(0.5, -3.0, 0.5)));
                let mut ashore = None;
                run(map, 600, |tick, mob| {
                    if tick > 100 {
                        assert!(mob.position.y + mob.height / 2.0 > 1.0, "{kind:?}");
                    }
                    if feet(mob) > bank as f32 - 0.1 && mob.on_ground {
                        ashore.get_or_insert(tick);
                    } else if ashore.is_some() {
                        assert!(feet(mob) > bank as f32 - 0.1, "{kind:?} went back");
                    }
                });
                assert!(
                    ashore.is_some(),
                    "{kind:?} is still in the water, bank {bank}"
                );
            }
        }
        // A hostile swims at its prey on the bank, and climbs after it
        for kind in [MobKind::Gnawer, MobKind::Stumpling, MobKind::Roller] {
            let mut map = pool(2);
            let mut player = Player::new(1, "Ann".into(), Vec3::ZERO, Transform::default());
            player.position = Vec3::new(8.5, 2.0 + player.height / 2.0, 0.5);
            map.players.insert(1, player);
            map.mobs
                .insert(0, ServerMob::new(kind, Vec3::new(0.5, 0.5, 0.5)));
            let mut climbed = false;
            run(map, 400, |_, mob| {
                climbed |= feet(mob) > 1.9 && mob.position.x > 5.5
            });
            assert!(climbed, "{kind:?}");
        }
    }
}
