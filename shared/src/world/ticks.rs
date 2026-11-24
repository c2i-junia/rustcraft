//! Random block ticks: the rules of what a block does on its own, now and then, like a fire
//! that spreads. The server draws `RANDOM_TICKS_PER_SECTION` cells of each chunk near the players
//! every tick and applies what `random_tick` returns for them; the rules here only read the
//! world.

use bevy::math::IVec3;
use rand::Rng;

use crate::world::ChunkMap;
use crate::world::{fire, global_block_to_chunk_pos, BlockData, BlockId, BlockTransparency};
use crate::CHUNK_SIZE;

/// The cells of a 16³ chunk that get a random tick in each tick, as Minecraft's default.
pub const RANDOM_TICKS_PER_SECTION: usize = 3;

/// What the rules of a tick read beyond the blocks.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TickContext {
    /// Whether it rains (or snows, or storms) on the world.
    pub raining: bool,
}

/// A change a tick makes to the world: the block put at `pos`, `None` for air.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlockChange {
    pub pos: IVec3,
    pub block: Option<BlockData>,
}

/// What the block at `pos` does on a random tick, from the world as it is: the changes to
/// apply, in order, none for a block that has no rule. A fire gets its tick more often than by
/// chance, on its own schedule (see the server), through this same function.
pub fn random_tick(
    world: &ChunkMap,
    pos: IVec3,
    context: &TickContext,
    rng: &mut impl Rng,
) -> Vec<BlockChange> {
    match world.get_block(pos) {
        Some(block) if block.id == BlockId::Fire => {
            fire::fire_tick(world, pos, block, context, rng)
        }
        _ => Vec::new(),
    }
}

/// Whether the cell at `pos` sees the sky: no block stands above it, but decorations, which
/// let the rain through, in the chunks loaded. The chunks above the highest loaded one are
/// taken for open air.
pub fn sees_sky(world: &ChunkMap, pos: IVec3) -> bool {
    let mut cell = pos + IVec3::Y;
    loop {
        let chunk_pos = global_block_to_chunk_pos(&cell);
        let Some(chunk) = world.chunk(chunk_pos) else {
            return true;
        };
        if chunk.uniform() == Some(None) {
            cell.y = (chunk_pos.y + 1) * CHUNK_SIZE;
            continue;
        }
        if world
            .get_block(cell)
            .is_some_and(|block| block.id.get_visibility() != BlockTransparency::Decoration)
        {
            return false;
        }
        cell.y += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::BlockDirection;
    use rand::{rngs::StdRng, SeedableRng};

    fn world() -> ChunkMap {
        let mut world = ChunkMap::with_loaded_region(IVec3::splat(-16), IVec3::splat(47));
        world.fill_box(
            IVec3::new(-16, -16, -16),
            IVec3::new(47, 0, 47),
            BlockId::Stone,
        );
        world
    }

    #[test]
    fn a_cell_sees_the_sky_through_air_and_plants_but_not_through_a_block() {
        let mut world = world();
        let pos = IVec3::new(3, 1, 3);
        assert!(sees_sky(&world, pos));
        world.fill_box(pos + IVec3::Y * 5, pos + IVec3::Y * 5, BlockId::TallGrass);
        assert!(sees_sky(&world, pos));
        world.fill_box(pos + IVec3::Y * 40, pos + IVec3::Y * 40, BlockId::OakLeaves);
        assert!(!sees_sky(&world, pos));
        assert!(sees_sky(&world, pos + IVec3::X));
    }

    #[test]
    fn a_random_tick_runs_the_rule_of_the_block_and_changes_nothing_itself() {
        let mut world = world();
        let pos = IVec3::new(3, 1, 3);
        let context = TickContext::default();
        let mut rng = StdRng::seed_from_u64(7);
        assert!(random_tick(&world, IVec3::new(3, 0, 3), &context, &mut rng).is_empty());
        assert!(random_tick(&world, pos, &context, &mut rng).is_empty());

        let fire = BlockData::new(BlockId::Fire, BlockDirection::Front);
        world.set_block(pos, fire);
        let ticked = random_tick(&world, pos, &context, &mut StdRng::seed_from_u64(7));
        let ruled = fire::fire_tick(&world, pos, &fire, &context, &mut StdRng::seed_from_u64(7));
        assert_eq!(ticked, ruled);
        assert!(!ticked.is_empty());
        assert_eq!(world.get_block(pos), Some(&fire));
    }
}
