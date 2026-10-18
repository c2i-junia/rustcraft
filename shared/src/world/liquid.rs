//! The levels of a liquid, stored in `BlockData::level`: a source is at level 0, the liquid
//! that flows away from it on the ground at levels 1 to 7, and the liquid that falls at level 8.

use bevy::math::IVec3;

use crate::world::{
    global_block_to_chunk_pos, BlockData, BlockDirection, BlockId, BlockTransparency, ChunkMap,
};

/// Level of a source.
pub const SOURCE_LEVEL: u8 = 0;
/// Level of the thinnest flowing liquid: it spreads no further.
pub const MAX_FLOW_LEVEL: u8 = 7;
/// Level of a liquid fed from above.
pub const FALLING_LEVEL: u8 = 8;

/// Height of the surface of a liquid at `level` with no liquid above it, in blocks: 8/9 for a
/// source and a falling liquid, one ninth less for each level of flow.
pub fn surface_height(level: u8) -> f32 {
    let ninths = match level {
        1..=MAX_FLOW_LEVEL => FALLING_LEVEL - level,
        _ => FALLING_LEVEL,
    };
    ninths as f32 / 9.0
}

/// Height of the surface of the liquid at `pos` above the bottom of its cell, in blocks:
/// the whole cell under more liquid, else `surface_height` of its level. `None` without a
/// liquid at `pos`.
pub fn liquid_height(world_map: &ChunkMap, pos: IVec3) -> Option<f32> {
    let is_liquid = |block: &BlockData| block.id.get_visibility() == BlockTransparency::Liquid;
    let block = world_map.get_block(pos).filter(|block| is_liquid(block))?;
    if world_map.get_block(pos + IVec3::Y).is_some_and(is_liquid) {
        Some(1.0)
    } else {
        Some(surface_height(block.level))
    }
}

/// What one step of the flow of water does to a cell.
#[derive(Debug, PartialEq)]
pub enum FlowChange {
    Set(BlockData),
    Remove,
}

/// What one step of the flow of water does to the cell at `pos`, from its neighbours as they
/// are, `None` when it stays as it is. A source, a block that water cannot wash away and a
/// cell of a chunk that is not loaded never change.
///
/// A cell gets falling water (`FALLING_LEVEL`) under water, else one level more than the
/// lowest horizontal neighbour that spreads to it, up to `MAX_FLOW_LEVEL`. Water spreads
/// sideways only when it cannot go down, and falling water spreads as a source does. Water
/// that nothing feeds any more is removed, which empties a flow once its source is gone.
pub fn flow_step(world_map: &ChunkMap, pos: IVec3) -> Option<FlowChange> {
    if !world_map.is_chunk_loaded(global_block_to_chunk_pos(&pos)) {
        return None;
    }
    let current = world_map.get_block(pos);
    if !can_be_flooded(current) {
        return None;
    }
    let current_level = current
        .filter(|block| is_water(block))
        .map(|block| block.level);
    match (current_level, fed_level(world_map, pos)) {
        (current, fed) if current == fed => None,
        (_, Some(level)) => Some(FlowChange::Set(BlockData {
            level,
            ..BlockData::new(BlockId::Water, BlockDirection::Front)
        })),
        (Some(_), None) => Some(FlowChange::Remove),
        (None, None) => None,
    }
}

fn is_water(block: &BlockData) -> bool {
    block.id == BlockId::Water
}

/// Whether flowing water can take the place of `block`: air, a decoration that it washes
/// away, or water that is not a source.
fn can_be_flooded(block: Option<&BlockData>) -> bool {
    match block {
        None => true,
        Some(block) if is_water(block) => block.level != SOURCE_LEVEL,
        Some(block) => block.id.get_visibility() == BlockTransparency::Decoration,
    }
}

/// The level of water that the cell at `pos` gets from its neighbours, `None` if none feeds it.
fn fed_level(world_map: &ChunkMap, pos: IVec3) -> Option<u8> {
    if world_map.get_block(pos + IVec3::Y).is_some_and(is_water) {
        return Some(FALLING_LEVEL);
    }
    [IVec3::X, IVec3::NEG_X, IVec3::Z, IVec3::NEG_Z]
        .into_iter()
        .filter_map(|offset| {
            let neighbour = pos + offset;
            let block = world_map
                .get_block(neighbour)
                .filter(|block| is_water(block))?;
            if !rests_on_something(world_map, neighbour) {
                return None;
            }
            let spread_from = match block.level {
                FALLING_LEVEL => SOURCE_LEVEL,
                level => level,
            };
            Some(spread_from + 1)
        })
        .min()
        .filter(|&level| level <= MAX_FLOW_LEVEL)
}

/// Whether the water at `pos` cannot go down: it lies on a block, on a source, or on a chunk
/// that is not loaded.
fn rests_on_something(world_map: &ChunkMap, pos: IVec3) -> bool {
    let below = pos + IVec3::NEG_Y;
    !world_map.is_chunk_loaded(global_block_to_chunk_pos(&below))
        || !can_be_flooded(world_map.get_block(below))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_liquid_under_more_liquid_fills_its_cell() {
        let mut world = ChunkMap::with_loaded_region(IVec3::ZERO, IVec3::splat(15));
        let bottom = IVec3::new(1, 1, 1);
        assert_eq!(liquid_height(&world, bottom), None);

        world.set_block(
            bottom,
            BlockData {
                level: 3,
                ..BlockData::new(BlockId::Water, BlockDirection::Front)
            },
        );
        assert_eq!(liquid_height(&world, bottom), Some(5.0 / 9.0));

        world.fill_box(bottom + IVec3::Y, bottom + IVec3::Y, BlockId::Water);
        assert_eq!(liquid_height(&world, bottom), Some(1.0));
        assert_eq!(liquid_height(&world, bottom + IVec3::Y), Some(8.0 / 9.0));
    }

    #[test]
    fn the_surface_goes_down_as_the_liquid_flows_away() {
        assert_eq!(surface_height(SOURCE_LEVEL), 8.0 / 9.0);
        assert_eq!(surface_height(FALLING_LEVEL), 8.0 / 9.0);
        assert_eq!(surface_height(MAX_FLOW_LEVEL), 1.0 / 9.0);
        for level in SOURCE_LEVEL..MAX_FLOW_LEVEL {
            assert!(surface_height(level + 1) < surface_height(level), "{level}");
        }
    }

    fn water(level: u8) -> BlockData {
        BlockData {
            level,
            ..BlockData::new(BlockId::Water, BlockDirection::Front)
        }
    }

    /// A stone floor whose top is at y = 4, in a loaded chunk.
    fn floor() -> ChunkMap {
        let mut world = ChunkMap::with_loaded_region(IVec3::ZERO, IVec3::splat(15));
        world.fill_box(IVec3::ZERO, IVec3::new(15, 3, 15), BlockId::Stone);
        world
    }

    const ON_FLOOR: IVec3 = IVec3::new(8, 4, 8);

    #[test]
    fn water_on_the_floor_spreads_one_level_further_to_its_side() {
        let mut world = floor();
        world.set_block(ON_FLOOR, water(SOURCE_LEVEL));
        assert_eq!(
            flow_step(&world, ON_FLOOR + IVec3::X),
            Some(FlowChange::Set(water(1)))
        );
        world.set_block(ON_FLOOR, water(3));
        assert_eq!(
            flow_step(&world, ON_FLOOR + IVec3::X),
            Some(FlowChange::Set(water(4)))
        );
        // Not above, not below, not diagonally
        for pos in [ON_FLOOR + IVec3::Y, ON_FLOOR + IVec3::new(1, 0, 1)] {
            assert_eq!(flow_step(&world, pos), None, "{pos}");
        }
    }

    #[test]
    fn the_thinnest_water_spreads_no_further() {
        let mut world = floor();
        world.set_block(ON_FLOOR, water(MAX_FLOW_LEVEL));
        assert_eq!(flow_step(&world, ON_FLOOR + IVec3::X), None);
    }

    #[test]
    fn the_lowest_neighbour_sets_the_level() {
        let mut world = floor();
        world.set_block(ON_FLOOR - IVec3::X, water(5));
        world.set_block(ON_FLOOR + IVec3::X, water(2));
        world.set_block(ON_FLOOR, water(6));
        assert_eq!(flow_step(&world, ON_FLOOR), Some(FlowChange::Set(water(3))));
        world.set_block(ON_FLOOR, water(3));
        assert_eq!(flow_step(&world, ON_FLOOR), None);
    }

    #[test]
    fn water_falls_instead_of_spreading_and_spreads_where_it_lands() {
        let mut world = floor();
        let high = ON_FLOOR + IVec3::new(0, 3, 0);
        world.set_block(high, water(SOURCE_LEVEL));
        assert_eq!(flow_step(&world, high + IVec3::X), None);
        assert_eq!(
            flow_step(&world, high - IVec3::Y),
            Some(FlowChange::Set(water(FALLING_LEVEL)))
        );
        world.set_block(ON_FLOOR, water(FALLING_LEVEL));
        assert_eq!(
            flow_step(&world, ON_FLOOR + IVec3::X),
            Some(FlowChange::Set(water(1)))
        );
    }

    #[test]
    fn water_that_nothing_feeds_is_removed_but_a_source_stays() {
        let mut world = floor();
        world.set_block(ON_FLOOR, water(2));
        assert_eq!(flow_step(&world, ON_FLOOR), Some(FlowChange::Remove));
        world.set_block(ON_FLOOR, water(SOURCE_LEVEL));
        assert_eq!(flow_step(&world, ON_FLOOR), None);
    }

    #[test]
    fn water_washes_decorations_away_but_not_blocks() {
        let mut world = floor();
        world.set_block(ON_FLOOR, water(SOURCE_LEVEL));
        world.fill_box(ON_FLOOR + IVec3::X, ON_FLOOR + IVec3::X, BlockId::Poppy);
        world.fill_box(ON_FLOOR - IVec3::X, ON_FLOOR - IVec3::X, BlockId::Glass);
        assert_eq!(
            flow_step(&world, ON_FLOOR + IVec3::X),
            Some(FlowChange::Set(water(1)))
        );
        assert_eq!(flow_step(&world, ON_FLOOR - IVec3::X), None);
    }

    #[test]
    fn water_does_not_flow_into_a_chunk_that_is_not_loaded() {
        let mut world = floor();
        let edge = IVec3::new(15, 4, 8);
        world.set_block(edge, water(SOURCE_LEVEL));
        assert_eq!(flow_step(&world, edge + IVec3::X), None);
    }
}
