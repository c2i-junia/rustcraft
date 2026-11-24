//! Fire: the blocks that burn, and what a fire does on each of its ticks (see `ticks`). Its
//! age, from 0 to `MAX_FIRE_AGE`, is its `BlockData::level`.

use bevy::math::IVec3;
use rand::Rng;

use crate::world::ticks::{sees_sky, BlockChange, TickContext};
use crate::world::{
    global_block_to_chunk_pos, BlockData, BlockDirection, BlockHitbox, BlockId, ChunkMap,
    SIX_OFFSETS,
};

/// The age past which a fire goes out.
pub const MAX_FIRE_AGE: u8 = 15;
/// A flammable neighbour burns at a tick of a fire with chance `burn / BURN_ODDS`.
const BURN_ODDS: u32 = 300;
/// An air cell by a flammable block catches at a tick of a fire next to it with chance
/// `catch / CATCH_ODDS`.
const CATCH_ODDS: u32 = 100;

/// How readily a block burns, as Minecraft's: `catch` for the fire to spread next to it, `burn`
/// for it to burn away.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Flammability {
    pub catch: u8,
    pub burn: u8,
}

/// How readily `block` burns, `None` for a block that does not.
pub fn flammability(block: BlockId) -> Option<Flammability> {
    let (catch, burn) = match block {
        BlockId::OakPlanks
        | BlockId::SprucePlanks
        | BlockId::JunglePlanks
        | BlockId::CraftingTable => (5, 20),
        BlockId::OakLog | BlockId::SpruceLog | BlockId::JungleLog => (5, 5),
        BlockId::OakLeaves | BlockId::SpruceLeaves | BlockId::JungleLeaves | BlockId::Cocoa => {
            (30, 60)
        }
        BlockId::TallGrass | BlockId::Fern | BlockId::Dandelion | BlockId::Poppy => (60, 100),
        BlockId::Vine => (15, 100),
        _ => return None,
    };
    Some(Flammability { catch, burn })
}

fn is_flammable(world: &ChunkMap, pos: IVec3) -> bool {
    world
        .get_block(pos)
        .is_some_and(|block| flammability(block.id).is_some())
}

/// Whether a fire stays at `pos`: on a whole block, or by a block that burns.
pub fn fire_can_stay(world: &ChunkMap, pos: IVec3) -> bool {
    let on_block = world
        .get_block(pos + IVec3::NEG_Y)
        .is_some_and(|below| matches!(below.get_hitbox(), BlockHitbox::FullBlock));
    on_block
        || SIX_OFFSETS
            .iter()
            .any(|offset| is_flammable(world, pos + *offset))
}

/// A new fire, lit by one of `age`: as old, or one tick older one time in 5.
fn new_fire(age: u8, rng: &mut impl Rng) -> BlockData {
    let older = u8::from(rng.gen_range(0..5) == 0);
    BlockData {
        level: (age + older).min(MAX_FIRE_AGE),
        ..BlockData::new(BlockId::Fire, BlockDirection::Front)
    }
}

/// What the fire `fire` at `pos` does on a tick. The rain puts it out where it sees the sky,
/// and so does the lack of support (`fire_can_stay`); else it ages by one, and goes out past
/// `MAX_FIRE_AGE`. Then each flammable neighbour may burn, into fire or air, and each air
/// neighbour by a flammable block may catch.
pub fn fire_tick(
    world: &ChunkMap,
    pos: IVec3,
    fire: &BlockData,
    context: &TickContext,
    rng: &mut impl Rng,
) -> Vec<BlockChange> {
    let put_out = vec![BlockChange { pos, block: None }];
    if context.raining && sees_sky(world, pos) || !fire_can_stay(world, pos) {
        return put_out;
    }
    let age = fire.level + 1;
    if age > MAX_FIRE_AGE {
        return put_out;
    }
    let mut changes = vec![BlockChange {
        pos,
        block: Some(BlockData {
            level: age,
            ..*fire
        }),
    }];
    for offset in SIX_OFFSETS {
        let neighbour = pos + offset;
        match world.get_block(neighbour) {
            Some(block) => {
                let Some(flammability) = flammability(block.id) else {
                    continue;
                };
                if rng.gen_range(0..BURN_ODDS) < u32::from(flammability.burn) {
                    // An old fire leaves ashes rather than flames
                    let flames = rng.gen_range(0..u32::from(age) + 10) < 5;
                    changes.push(BlockChange {
                        pos: neighbour,
                        block: flames.then(|| new_fire(age, rng)),
                    });
                }
            }
            None if world.is_chunk_loaded(global_block_to_chunk_pos(&neighbour)) => {
                let catch = SIX_OFFSETS
                    .iter()
                    .filter_map(|offset| world.get_block(neighbour + *offset))
                    .filter_map(|block| flammability(block.id))
                    .map(|flammability| flammability.catch)
                    .max();
                if catch.is_some_and(|catch| rng.gen_range(0..CATCH_ODDS) < u32::from(catch)) {
                    changes.push(BlockChange {
                        pos: neighbour,
                        block: Some(new_fire(age, rng)),
                    });
                }
            }
            None => {}
        }
    }
    changes
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::{rngs::StdRng, SeedableRng};

    /// A stone floor whose top is at y = 0, from -16 to 47.
    fn floor() -> ChunkMap {
        let mut world = ChunkMap::with_loaded_region(IVec3::splat(-16), IVec3::splat(47));
        world.fill_box(IVec3::splat(-16), IVec3::new(47, 0, 47), BlockId::Stone);
        world
    }

    const ON_FLOOR: IVec3 = IVec3::new(8, 1, 8);

    fn fire(age: u8) -> BlockData {
        BlockData {
            level: age,
            ..BlockData::new(BlockId::Fire, BlockDirection::Front)
        }
    }

    fn put_out(pos: IVec3) -> Vec<BlockChange> {
        vec![BlockChange { pos, block: None }]
    }

    fn tick(world: &ChunkMap, pos: IVec3, age: u8, raining: bool, seed: u64) -> Vec<BlockChange> {
        let context = TickContext { raining };
        fire_tick(
            world,
            pos,
            &fire(age),
            &context,
            &mut StdRng::seed_from_u64(seed),
        )
    }

    #[test]
    fn wood_leaves_and_plants_burn_and_stone_earth_and_stations_of_stone_do_not() {
        for block in [
            BlockId::OakLog,
            BlockId::SprucePlanks,
            BlockId::JungleLeaves,
            BlockId::TallGrass,
            BlockId::Fern,
            BlockId::Vine,
            BlockId::Poppy,
            BlockId::Cocoa,
            BlockId::CraftingTable,
        ] {
            assert!(flammability(block).is_some(), "{block:?}");
        }
        for block in [
            BlockId::Stone,
            BlockId::Dirt,
            BlockId::Grass,
            BlockId::Sand,
            BlockId::Chest,
            BlockId::Furnace,
            BlockId::Water,
            BlockId::Fire,
        ] {
            assert_eq!(flammability(block), None, "{block:?}");
        }
    }

    #[test]
    fn a_fire_with_nothing_under_it_nor_anything_to_burn_goes_out() {
        let world = floor();
        let high = ON_FLOOR + IVec3::Y * 3;
        assert!(!fire_can_stay(&world, high));
        assert_eq!(tick(&world, high, 0, false, 1), put_out(high));
        // a fire by planks stays, even in the air
        let mut world = world;
        world.fill_box(high + IVec3::X, high + IVec3::X, BlockId::OakPlanks);
        assert!(fire_can_stay(&world, high));
        assert!(fire_can_stay(&world, ON_FLOOR));
    }

    #[test]
    fn a_fire_on_stone_ages_by_one_and_goes_out_past_its_last_age() {
        let world = floor();
        assert_eq!(
            tick(&world, ON_FLOOR, 3, false, 1),
            vec![BlockChange {
                pos: ON_FLOOR,
                block: Some(fire(4)),
            }]
        );
        assert_eq!(
            tick(&world, ON_FLOOR, MAX_FIRE_AGE - 1, false, 1)[0].block,
            Some(fire(MAX_FIRE_AGE))
        );
        assert_eq!(
            tick(&world, ON_FLOOR, MAX_FIRE_AGE, false, 1),
            put_out(ON_FLOOR)
        );
    }

    #[test]
    fn rain_puts_out_a_fire_under_the_sky_and_not_one_under_a_roof() {
        let mut world = floor();
        assert_eq!(tick(&world, ON_FLOOR, 0, true, 1), put_out(ON_FLOOR));
        world.fill_box(
            ON_FLOOR + IVec3::Y * 4,
            ON_FLOOR + IVec3::Y * 4,
            BlockId::Cobblestone,
        );
        assert_eq!(tick(&world, ON_FLOOR, 0, true, 1)[0].block, Some(fire(1)));
    }

    #[test]
    fn a_fire_by_planks_spreads_to_the_air_around_them_and_burns_them() {
        let mut world = floor();
        // A pillar of planks two blocks high next to the fire
        let planks = ON_FLOOR + IVec3::X;
        world.fill_box(planks, planks + IVec3::Y, BlockId::OakPlanks);
        let (mut burnt, mut caught) = (0, 0);
        for seed in 0..2000 {
            for change in tick(&world, ON_FLOOR, 0, false, seed).into_iter().skip(1) {
                if change.pos == planks {
                    burnt += 1;
                } else {
                    // only air cells by the planks catch: above the fire, by the pillar
                    assert_eq!(world.get_block(change.pos), None, "{change:?}");
                    assert!(
                        SIX_OFFSETS
                            .iter()
                            .any(|o| is_flammable(&world, change.pos + *o)),
                        "{change:?}"
                    );
                    assert_eq!(change.block.map(|b| b.id), Some(BlockId::Fire));
                    caught += 1;
                }
            }
        }
        // 20 / 300 of the ticks burn the planks, 5 / 100 set the cell above the fire alight
        assert!((90..180).contains(&burnt), "burnt {burnt} times in 2000");
        assert!((60..150).contains(&caught), "caught {caught} times in 2000");
    }

    #[test]
    fn burning_planks_turn_into_fire_or_air() {
        let mut world = floor();
        world.fill_box(ON_FLOOR + IVec3::X, ON_FLOOR + IVec3::X, BlockId::OakPlanks);
        let burnt: Vec<Option<BlockData>> = (0..3000)
            .flat_map(|seed| tick(&world, ON_FLOOR, 0, false, seed))
            .filter(|change| change.pos == ON_FLOOR + IVec3::X)
            .map(|change| change.block)
            .collect();
        // a young fire turns it into fire one time in two
        assert!(burnt.contains(&None));
        assert!(burnt
            .iter()
            .any(|block| block.is_some_and(|b| b.id == BlockId::Fire)));
    }

    #[test]
    fn a_fire_among_stone_never_spreads() {
        let world = floor();
        for seed in 0..500 {
            let changes = tick(&world, ON_FLOOR, 0, false, seed);
            assert_eq!(changes.len(), 1, "{changes:?}");
        }
    }
}
