//! Lava on its random ticks (see `ticks`): it sets fire to the air by what burns around it.

use bevy::math::IVec3;
use rand::Rng;

use crate::world::fire::flammability;
use crate::world::ticks::BlockChange;
use crate::world::SIX_OFFSETS;
use crate::world::{global_block_to_chunk_pos, BlockData, BlockDirection, BlockId, ChunkMap};

/// A random tick of lava tries to light a fire one time in this many: with
/// `RANDOM_TICKS_PER_SECTION` (4), a lava cell tries once in 4096 ticks.
pub const IGNITE_ODDS: u32 = 4;

/// What the lava at `pos` does on a random tick: one time in `IGNITE_ODDS`, it looks at one
/// cell drawn in the 3x2x3 box around it and above it, and lights a fire there if it is air
/// with a flammable block on one of its sides.
pub fn lava_tick(world: &ChunkMap, pos: IVec3, rng: &mut impl Rng) -> Vec<BlockChange> {
    if rng.gen_range(0..IGNITE_ODDS) != 0 {
        return Vec::new();
    }
    let cell = pos
        + IVec3::new(
            rng.gen_range(-1..=1),
            rng.gen_range(0..=1),
            rng.gen_range(-1..=1),
        );
    let by_fuel = SIX_OFFSETS.iter().any(|offset| {
        world
            .get_block(cell + *offset)
            .is_some_and(|block| flammability(block.id).is_some())
    });
    let open =
        world.get_block(cell).is_none() && world.is_chunk_loaded(global_block_to_chunk_pos(&cell));
    if open && by_fuel {
        vec![BlockChange {
            pos: cell,
            block: Some(BlockData::new(BlockId::Fire, BlockDirection::Front)),
        }]
    } else {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::{rngs::StdRng, SeedableRng};

    /// Lava on a stone floor whose top is at y = 0, and the fires its ticks light.
    fn fires_lit(world: &ChunkMap, lava: IVec3) -> Vec<IVec3> {
        let mut rng = StdRng::seed_from_u64(7);
        (0..300)
            .flat_map(|_| lava_tick(world, lava, &mut rng))
            .map(|change| {
                assert_eq!(change.block.map(|b| b.id), Some(BlockId::Fire));
                change.pos
            })
            .collect()
    }

    fn floor() -> ChunkMap {
        let mut world = ChunkMap::with_loaded_region(IVec3::splat(-16), IVec3::splat(31));
        world.fill_box(IVec3::splat(-16), IVec3::new(31, 0, 31), BlockId::Stone);
        world
    }

    const LAVA: IVec3 = IVec3::new(4, 1, 4);

    #[test]
    fn lava_sets_fire_to_the_air_by_planks_near_it() {
        let mut world = floor();
        world.set_block(LAVA, BlockData::new(BlockId::Lava, BlockDirection::Front));
        let planks = LAVA + IVec3::new(2, 0, 0);
        world.fill_box(planks, planks, BlockId::OakPlanks);
        let lit = fires_lit(&world, LAVA);
        assert!(!lit.is_empty(), "no fire in 300 ticks");
        assert!(lit.len() < 100, "{} fires: too often", lit.len());
        for pos in lit {
            let touches = SIX_OFFSETS.iter().any(|offset| pos + *offset == planks);
            assert!(touches, "a fire at {pos}, away from the planks");
            assert_ne!(pos, LAVA);
        }
    }

    #[test]
    fn lava_among_stone_lights_nothing() {
        let mut world = floor();
        world.set_block(LAVA, BlockData::new(BlockId::Lava, BlockDirection::Front));
        assert!(fires_lit(&world, LAVA).is_empty());
    }
}
