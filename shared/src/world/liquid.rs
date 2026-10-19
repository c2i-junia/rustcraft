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
    surface_ninths(level) as f32 / 9.0
}

/// `surface_height` in ninths of a block.
fn surface_ninths(level: u8) -> u32 {
    match level {
        1..=MAX_FLOW_LEVEL => (FALLING_LEVEL - level) as u32,
        _ => FALLING_LEVEL as u32,
    }
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

/// Heights of the four top corners of the liquid at `pos` above the bottom of its cell, in
/// blocks: the corner at `x` and `z` (each 0 or 1) at index `x + 2 * z`. `None` without a liquid
/// at `pos`.
///
/// As in Minecraft, a corner averages the cells that share it. A liquid at the height of a
/// source or higher weighs 10, a lower one 1, and air and decorations weigh 1 at height 0,
/// which brings the surface down towards them; blocks do not count. A corner is at 1 when one
/// of its cells is under more liquid. Two cells that share a corner give it the same height, so
/// the surface has no steps.
pub fn corner_heights(world_map: &ChunkMap, pos: IVec3) -> Option<[f32; 4]> {
    // The cell and the eight around it, by x then z from -1 to 1
    let cells: [[CornerCell; 3]; 3] = std::array::from_fn(|x| {
        std::array::from_fn(|z| {
            corner_cell(world_map, pos + IVec3::new(x as i32 - 1, 0, z as i32 - 1))
        })
    });
    if !matches!(cells[1][1], CornerCell::Liquid { .. }) {
        return None;
    }
    Some(std::array::from_fn(|index| {
        let (x, z) = (index % 2 * 2, index / 2 * 2);
        corner_height(cells[1][1], cells[x][1], cells[1][z], cells[x][z])
    }))
}

/// What a cell adds to the height of a corner of a liquid.
#[derive(Clone, Copy)]
enum CornerCell {
    /// A liquid whose surface is `ninths` of a block high, or a whole block under more liquid.
    Liquid { ninths: u32 },
    /// Air or a decoration: the liquid flows into it.
    Open,
    /// A block, or a cell of a chunk that is not loaded.
    Wall,
}

fn corner_cell(world_map: &ChunkMap, pos: IVec3) -> CornerCell {
    if !world_map.is_chunk_loaded(global_block_to_chunk_pos(&pos)) {
        return CornerCell::Wall;
    }
    let Some(block) = world_map.get_block(pos) else {
        return CornerCell::Open;
    };
    match block.id.get_visibility() {
        BlockTransparency::Liquid => {
            let under_liquid = world_map
                .get_block(pos + IVec3::Y)
                .is_some_and(|above| above.id.get_visibility() == BlockTransparency::Liquid);
            CornerCell::Liquid {
                ninths: if under_liquid {
                    9
                } else {
                    surface_ninths(block.level)
                },
            }
        }
        BlockTransparency::Decoration => CornerCell::Open,
        _ => CornerCell::Wall,
    }
}

/// Height of a top corner of the liquid in `cell`, from the cells beside it along x and z that
/// share the corner, and the one across the corner. That one does not count when blocks wall it
/// off on both sides.
fn corner_height(
    cell: CornerCell,
    side_x: CornerCell,
    side_z: CornerCell,
    across: CornerCell,
) -> f32 {
    let across = match (side_x, side_z) {
        (CornerCell::Wall, CornerCell::Wall) => CornerCell::Wall,
        _ => across,
    };
    // In ninths of a block, so that every cell around the corner sums to the same height
    let (mut sum, mut weight) = (0, 0);
    for cell in [cell, side_x, side_z, across] {
        match cell {
            CornerCell::Liquid { ninths: 9 } => return 1.0,
            CornerCell::Liquid { ninths } => {
                let cell_weight = if ninths >= FALLING_LEVEL as u32 {
                    10
                } else {
                    1
                };
                sum += ninths * cell_weight;
                weight += cell_weight;
            }
            CornerCell::Open => weight += 1,
            CornerCell::Wall => {}
        }
    }
    sum as f32 / (weight * 9) as f32
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
    use std::collections::HashMap;

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

    /// The corners of the water at `pos`, in ninths of a block.
    fn corners_in_ninths(world: &ChunkMap, pos: IVec3) -> [f32; 4] {
        corner_heights(world, pos)
            .expect("water at pos")
            .map(|height| height * 9.0)
    }

    fn assert_near(actual: [f32; 4], expected: [f32; 4]) {
        for (a, e) in actual.iter().zip(expected) {
            assert!((a - e).abs() < 1e-4, "{actual:?} != {expected:?}");
        }
    }

    #[test]
    fn a_cell_without_liquid_has_no_corners() {
        let mut world = floor();
        assert_eq!(corner_heights(&world, ON_FLOOR), None);
        world.fill_box(ON_FLOOR, ON_FLOOR, BlockId::Stone);
        assert_eq!(corner_heights(&world, ON_FLOOR), None);
    }

    #[test]
    fn the_surface_of_a_pool_is_level_with_its_sources() {
        let mut world = floor();
        // A pool of 3 by 3 sources walled with stone
        world.fill_box(
            ON_FLOOR - IVec3::new(2, 0, 2),
            ON_FLOOR + IVec3::new(2, 0, 2),
            BlockId::Stone,
        );
        world.fill_box(
            ON_FLOOR - IVec3::new(1, 0, 1),
            ON_FLOOR + IVec3::new(1, 0, 1),
            BlockId::Water,
        );
        for x in -1..=1 {
            for z in -1..=1 {
                let pos = ON_FLOOR + IVec3::new(x, 0, z);
                assert_eq!(corner_heights(&world, pos), Some([8.0 / 9.0; 4]), "{pos}");
            }
        }
    }

    #[test]
    fn the_surface_goes_down_towards_the_air() {
        let mut world = floor();
        world.set_block(ON_FLOOR, water(SOURCE_LEVEL));
        // Alone on the floor, each corner has the source and three cells of air
        assert_near(corners_in_ninths(&world, ON_FLOOR), [80.0 / 13.0; 4]);

        // A wall on one side: the corners against it lose two cells of air
        world.fill_box(
            ON_FLOOR + IVec3::new(-1, 0, -1),
            ON_FLOOR + IVec3::new(1, 0, -1),
            BlockId::Stone,
        );
        assert_near(
            corners_in_ninths(&world, ON_FLOOR),
            [80.0 / 11.0, 80.0 / 11.0, 80.0 / 13.0, 80.0 / 13.0],
        );
    }

    #[test]
    fn a_flow_slopes_down_from_its_source_without_steps() {
        let mut world = floor();
        // A channel along x, walled on both sides, that ends in the air
        world.fill_box(
            ON_FLOOR + IVec3::new(-1, 0, -1),
            ON_FLOOR + IVec3::new(4, 0, 1),
            BlockId::Stone,
        );
        for (x, level) in [(0, SOURCE_LEVEL), (1, 1), (2, 2), (3, 3)] {
            world.set_block(ON_FLOOR + IVec3::X * x, water(level));
        }
        world.remove_block(ON_FLOOR + IVec3::X * 4);

        let corners: Vec<[f32; 4]> = (0..4)
            .map(|x| corners_in_ninths(&world, ON_FLOOR + IVec3::X * x))
            .collect();
        // The corners at x = 0, then between each two cells, then against the air
        let expected = [8.0, 87.0 / 11.0, 13.0 / 2.0, 11.0 / 2.0, 5.0 / 2.0];
        for (x, cell) in corners.iter().enumerate() {
            assert_near(
                *cell,
                [expected[x], expected[x + 1], expected[x], expected[x + 1]],
            );
        }
    }

    #[test]
    fn the_cells_that_share_a_corner_give_it_the_same_height() {
        let mut world = floor();
        // An uneven puddle: a source, flows at several levels, and holes of air
        for (offset, level) in [
            (IVec3::new(0, 0, 0), SOURCE_LEVEL),
            (IVec3::new(1, 0, 0), 1),
            (IVec3::new(0, 0, 1), 2),
            (IVec3::new(1, 0, 1), 3),
            (IVec3::new(-1, 0, 1), 5),
        ] {
            world.set_block(ON_FLOOR + offset, water(level));
        }
        world.fill_box(ON_FLOOR - IVec3::X, ON_FLOOR - IVec3::X, BlockId::Stone);
        world.set_block(ON_FLOOR + IVec3::new(1, 1, 1), water(FALLING_LEVEL));

        let mut heights: HashMap<IVec3, f32> = HashMap::new();
        for x in -1..=1 {
            for z in 0..=1 {
                let pos = ON_FLOOR + IVec3::new(x, 0, z);
                let Some(corners) = corner_heights(&world, pos) else {
                    continue;
                };
                for (index, height) in corners.into_iter().enumerate() {
                    let corner = pos + IVec3::new(index as i32 % 2, 0, index as i32 / 2);
                    let shared = *heights.entry(corner).or_insert(height);
                    assert_eq!(shared, height, "corner {corner} of {pos}");
                }
            }
        }
    }

    #[test]
    fn a_corner_next_to_a_liquid_under_more_liquid_is_at_the_top_of_the_cell() {
        let mut world = floor();
        world.set_block(ON_FLOOR, water(SOURCE_LEVEL));
        world.set_block(ON_FLOOR + IVec3::X, water(FALLING_LEVEL));
        world.set_block(ON_FLOOR + IVec3::new(1, 1, 0), water(SOURCE_LEVEL));
        let [low_x_low_z, high_x_low_z, low_x_high_z, high_x_high_z] =
            corner_heights(&world, ON_FLOOR).unwrap();
        assert_eq!([high_x_low_z, high_x_high_z], [1.0, 1.0]);
        assert!(low_x_low_z < 1.0 && low_x_high_z < 1.0);
        assert_eq!(corner_heights(&world, ON_FLOOR + IVec3::X), Some([1.0; 4]));
    }

    #[test]
    fn water_across_a_corner_walled_off_on_both_sides_does_not_count() {
        let mut world = floor();
        world.set_block(ON_FLOOR, water(MAX_FLOW_LEVEL));
        world.fill_box(ON_FLOOR + IVec3::X, ON_FLOOR + IVec3::X, BlockId::Stone);
        world.fill_box(ON_FLOOR + IVec3::Z, ON_FLOOR + IVec3::Z, BlockId::Stone);
        let alone = corner_heights(&world, ON_FLOOR).unwrap()[3];
        assert_eq!(alone, 1.0 / 9.0);
        world.set_block(ON_FLOOR + IVec3::new(1, 0, 1), water(SOURCE_LEVEL));
        assert_eq!(corner_heights(&world, ON_FLOOR).unwrap()[3], alone);
    }

    #[test]
    fn a_chunk_that_is_not_loaded_does_not_pull_the_surface_down() {
        let mut world = floor();
        let edge = IVec3::new(15, 4, 8);
        world.fill_box(edge - IVec3::new(1, 0, 1), edge + IVec3::Z, BlockId::Stone);
        world.set_block(edge, water(SOURCE_LEVEL));
        world.remove_block(edge + IVec3::Z);
        // Stone on the -x and -z sides, air on +z, and the chunk at +x is not loaded
        let corners = corner_heights(&world, edge).unwrap();
        assert_eq!(corners[0], 8.0 / 9.0);
        assert_eq!(corners[1], 8.0 / 9.0);
        assert!(corners[3] < 8.0 / 9.0);
    }

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
