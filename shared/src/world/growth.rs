//! What the plants and some blocks do on their random ticks: crops and berries ripen, the rain
//! fills the cisterns and puts the campfires out. Saplings grow too, with `SAPLING_ODDS`, but
//! the tree they become is the server's generator's, so the server grows it.

use bevy::math::IVec3;
use rand::Rng;

use crate::world::ticks::{sees_sky, BlockChange, TickContext};
use crate::world::{BlockData, BlockId, ChunkMap};

/// One random tick in this many moves a crop to its next stage, or one in half as many when
/// it is watered.
pub const CROP_ODDS: u32 = 4;
/// One random tick in this many ripens a bush of berries.
pub const BERRY_ODDS: u32 = 5;
/// One random tick in this many grows a sapling into its tree, if it has room.
pub const SAPLING_ODDS: u32 = 6;
/// How far from the soil of a crop, along x and z, water waters it.
pub const WATER_REACH: i32 = 4;

/// The stage a crop grows into, `None` for a ripe crop or anything else.
pub fn next_stage(id: BlockId) -> Option<BlockId> {
    Some(match id {
        BlockId::RyeSprout => BlockId::YoungRye,
        BlockId::YoungRye => BlockId::RipeRye,
        BlockId::CarrotSprout => BlockId::YoungCarrots,
        BlockId::YoungCarrots => BlockId::TallCarrots,
        BlockId::TallCarrots => BlockId::RipeCarrots,
        BlockId::PotatoSprout => BlockId::YoungPotatoes,
        BlockId::YoungPotatoes => BlockId::TallPotatoes,
        BlockId::TallPotatoes => BlockId::RipePotatoes,
        _ => return None,
    })
}

/// Whether the block is a sapling, which grows into a tree.
pub fn is_sapling(id: BlockId) -> bool {
    matches!(
        id,
        BlockId::OakSapling
            | BlockId::SpruceSapling
            | BlockId::JungleSapling
            | BlockId::WillowSapling
            | BlockId::AcaciaSapling
    )
}

/// Whether the rain falls on the cell at `pos`.
pub fn rained_on(world: &ChunkMap, pos: IVec3, context: &TickContext) -> bool {
    context.raining && sees_sky(world, pos)
}

/// Whether water lies within `WATER_REACH` of the tilled soil at `soil` along x and z, at its
/// height or one above, as by Minecraft's farmland.
pub fn soil_by_water(world: &ChunkMap, soil: IVec3) -> bool {
    if world.get_block(soil).map(|block| block.id) != Some(BlockId::TilledSoil) {
        return false;
    }
    (-WATER_REACH..=WATER_REACH).any(|x| {
        (0..=1).any(|y| {
            (-WATER_REACH..=WATER_REACH).any(|z| {
                world
                    .get_block(soil + IVec3::new(x, y, z))
                    .is_some_and(|block| block.id == BlockId::Water)
            })
        })
    })
}

/// Whether the crop at `pos` is watered, by the rain or by water near its soil: it then grows
/// twice as fast. Both do not water it more.
pub fn is_watered(world: &ChunkMap, pos: IVec3, context: &TickContext) -> bool {
    rained_on(world, pos, context) || soil_by_water(world, pos - IVec3::Y)
}

/// What `block`, at `pos`, does on a random tick, if it is a plant, a cistern or a campfire.
/// A sapling does nothing here (see `SAPLING_ODDS`).
pub fn growth_tick(
    world: &ChunkMap,
    pos: IVec3,
    block: &BlockData,
    context: &TickContext,
    rng: &mut impl Rng,
) -> Vec<BlockChange> {
    let turned = match block.id {
        id if next_stage(id).is_some() => {
            let odds = if is_watered(world, pos, context) {
                CROP_ODDS / 2
            } else {
                CROP_ODDS
            };
            if !rng.gen_ratio(1, odds) {
                return Vec::new();
            }
            next_stage(id)
        }
        BlockId::BerryBush if rng.gen_ratio(1, BERRY_ODDS) => Some(BlockId::RipeBerryBush),
        BlockId::RainCistern if rained_on(world, pos, context) => Some(BlockId::FullRainCistern),
        BlockId::Campfire if rained_on(world, pos, context) => Some(BlockId::UnlitCampfire),
        _ => None,
    };
    turned
        .map(|id| BlockChange {
            pos,
            block: Some(BlockData::new(id, block.direction)),
        })
        .into_iter()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::BlockDirection;
    use rand::{rngs::StdRng, SeedableRng};

    const AT: IVec3 = IVec3::new(4, 1, 4);
    const RAIN: TickContext = TickContext { raining: true };
    const CLEAR: TickContext = TickContext { raining: false };

    /// Tilled soil on a stone floor whose top is at y = 0, with `id` on it at `AT`.
    fn field(id: BlockId) -> ChunkMap {
        let mut world = ChunkMap::with_loaded_region(IVec3::splat(-16), IVec3::splat(31));
        world.fill_box(IVec3::splat(-16), IVec3::new(31, 0, 31), BlockId::Stone);
        world.fill_box(AT - IVec3::Y, AT - IVec3::Y, BlockId::TilledSoil);
        world.set_block(AT, BlockData::new(id, BlockDirection::Front));
        world
    }

    /// What the block at `AT` becomes after `ticks` random ticks.
    fn after(world: &mut ChunkMap, ticks: u32, context: &TickContext, seed: u64) -> BlockId {
        let mut rng = StdRng::seed_from_u64(seed);
        for _ in 0..ticks {
            let block = *world.get_block(AT).unwrap();
            for change in growth_tick(world, AT, &block, context, &mut rng) {
                world.set_block(change.pos, change.block.unwrap());
            }
        }
        world.get_block(AT).unwrap().id
    }

    /// How many random ticks, on average over many crops, a crop takes to grow one stage.
    fn ticks_per_stage(world: &ChunkMap, context: &TickContext) -> f64 {
        let block = *world.get_block(AT).unwrap();
        let mut rng = StdRng::seed_from_u64(11);
        let tries = 20_000;
        let grown = (0..tries)
            .filter(|_| !growth_tick(world, AT, &block, context, &mut rng).is_empty())
            .count();
        tries as f64 / grown as f64
    }

    #[test]
    fn crops_grow_stage_by_stage_until_ripe() {
        for (stages, ripe) in [
            (
                &[BlockId::RyeSprout, BlockId::YoungRye][..],
                BlockId::RipeRye,
            ),
            (
                &[
                    BlockId::CarrotSprout,
                    BlockId::YoungCarrots,
                    BlockId::TallCarrots,
                ],
                BlockId::RipeCarrots,
            ),
            (
                &[
                    BlockId::PotatoSprout,
                    BlockId::YoungPotatoes,
                    BlockId::TallPotatoes,
                ],
                BlockId::RipePotatoes,
            ),
        ] {
            let mut world = field(stages[0]);
            // stage by stage, never skipping one
            let mut seen = vec![stages[0]];
            let mut rng = StdRng::seed_from_u64(5);
            for _ in 0..400 {
                let block = *world.get_block(AT).unwrap();
                for change in growth_tick(&world, AT, &block, &CLEAR, &mut rng) {
                    let id = change.block.unwrap().id;
                    assert_eq!(next_stage(*seen.last().unwrap()), Some(id));
                    seen.push(id);
                    world.set_block(change.pos, change.block.unwrap());
                }
            }
            assert_eq!(seen.last(), Some(&ripe), "{stages:?}");
            assert_eq!(seen.len(), stages.len() + 1);
            assert_eq!(next_stage(ripe), None);
        }
    }

    #[test]
    fn a_crop_grows_a_stage_one_tick_in_four_or_one_in_two_when_watered() {
        let dry = field(BlockId::CarrotSprout);
        assert!((ticks_per_stage(&dry, &CLEAR) - 4.0).abs() < 0.2);

        // The rain on it, as main's rye
        assert!((ticks_per_stage(&dry, &RAIN) - 2.0).abs() < 0.1);
        let mut roofed = dry.clone();
        roofed.fill_box(AT + IVec3::Y * 3, AT + IVec3::Y * 3, BlockId::Stone);
        assert!((ticks_per_stage(&roofed, &RAIN) - 4.0).abs() < 0.2);

        // Water 4 blocks away from its soil, at the soil's height
        let mut by_water = dry.clone();
        let pond = AT - IVec3::Y + IVec3::new(4, 0, -4);
        by_water.fill_box(pond, pond, BlockId::Water);
        assert!((ticks_per_stage(&by_water, &CLEAR) - 2.0).abs() < 0.1);
        // and not twice again in the rain
        assert!((ticks_per_stage(&by_water, &RAIN) - 2.0).abs() < 0.1);

        // 5 blocks away, or 2 above the soil, it is too far
        for far in [IVec3::new(5, 0, 0), IVec3::new(0, 2, 2)] {
            let mut world = dry.clone();
            let pond = AT - IVec3::Y + far;
            world.fill_box(pond, pond, BlockId::Water);
            assert!(!soil_by_water(&world, AT - IVec3::Y), "{far}");
        }
        // Nor does water by a soil that is not tilled
        let mut untilled = by_water.clone();
        untilled.fill_box(AT - IVec3::Y, AT - IVec3::Y, BlockId::Dirt);
        assert!(!soil_by_water(&untilled, AT - IVec3::Y));
    }

    #[test]
    fn berries_ripen_and_the_rain_fills_cisterns_and_puts_campfires_out() {
        for (id, context, becomes) in [
            (BlockId::BerryBush, CLEAR, BlockId::RipeBerryBush),
            (BlockId::RainCistern, CLEAR, BlockId::RainCistern),
            (BlockId::RainCistern, RAIN, BlockId::FullRainCistern),
            (BlockId::Campfire, CLEAR, BlockId::Campfire),
            (BlockId::Campfire, RAIN, BlockId::UnlitCampfire),
        ] {
            let mut world = field(id);
            assert_eq!(after(&mut world, 200, &context, 3), becomes, "{id:?}");
        }
        // At once in the rain, under the open sky only
        let mut world = field(BlockId::RainCistern);
        assert_eq!(after(&mut world, 1, &RAIN, 3), BlockId::FullRainCistern);
        let mut roofed = field(BlockId::Campfire);
        roofed.fill_box(AT + IVec3::Y * 10, AT + IVec3::Y * 10, BlockId::OakPlanks);
        assert_eq!(after(&mut roofed, 200, &RAIN, 3), BlockId::Campfire);
        // A bush one tick in five
        let bush = field(BlockId::BerryBush);
        assert!((ticks_per_stage(&bush, &CLEAR) - 5.0).abs() < 0.25);
    }

    #[test]
    fn a_sapling_waits_for_the_server_and_other_blocks_do_nothing() {
        for id in [BlockId::OakSapling, BlockId::Stone, BlockId::RipeRye] {
            let world = field(id);
            let block = *world.get_block(AT).unwrap();
            let mut rng = StdRng::seed_from_u64(1);
            for _ in 0..100 {
                assert!(growth_tick(&world, AT, &block, &RAIN, &mut rng).is_empty());
            }
        }
        assert!(is_sapling(BlockId::AcaciaSapling));
        assert!(!is_sapling(BlockId::BerryBush));
    }
}
