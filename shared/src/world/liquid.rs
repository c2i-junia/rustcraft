//! The liquids, water and lava, and their levels, stored in `BlockData::level`: a source is at
//! level 0, the liquid that flows away from it on the ground at levels 1 to 7, and the liquid
//! that falls at level 8. Both flow by the same rules, with their own `Liquid` parameters.

use bevy::math::{IVec3, Vec3};

use crate::world::{
    global_block_to_chunk_pos, BlockData, BlockDirection, BlockId, BlockTransparency, ChunkMap,
    SIX_OFFSETS,
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

/// Whether an eye at `eye` is under the surface of a liquid, any liquid.
pub fn eye_in_liquid(world_map: &ChunkMap, eye: Vec3) -> bool {
    eye_liquid(world_map, eye).is_some()
}

/// The liquid an eye at `eye` is under the surface of, if any.
pub fn eye_liquid(world_map: &ChunkMap, eye: Vec3) -> Option<BlockId> {
    let cell = eye.floor().as_ivec3();
    liquid_height(world_map, cell)
        .filter(|height| eye.y - eye.y.floor() < *height)
        .and_then(|_| world_map.get_block(cell))
        .map(|block| block.id)
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
    let liquid = world_map.get_block(pos)?.id;
    let cells: [[CornerCell; 3]; 3] = std::array::from_fn(|x| {
        std::array::from_fn(|z| {
            corner_cell(
                world_map,
                pos + IVec3::new(x as i32 - 1, 0, z as i32 - 1),
                liquid,
            )
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
    /// A block, another liquid, or a cell of a chunk that is not loaded.
    Wall,
}

/// What the cell at `pos` adds to a corner of a surface of `liquid`.
fn corner_cell(world_map: &ChunkMap, pos: IVec3, liquid: BlockId) -> CornerCell {
    if !world_map.is_chunk_loaded(global_block_to_chunk_pos(&pos)) {
        return CornerCell::Wall;
    }
    let Some(block) = world_map.get_block(pos) else {
        return CornerCell::Open;
    };
    match block.id.get_visibility() {
        BlockTransparency::Liquid if block.id == liquid => {
            let under_liquid = world_map
                .get_block(pos + IVec3::Y)
                .is_some_and(|above| above.id == liquid);
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

/// A liquid, and how it flows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Liquid {
    pub block: BlockId,
    /// The levels the liquid loses from one cell to the next on the ground: a source reaches
    /// `MAX_FLOW_LEVEL / level_drop` cells on flat ground.
    pub level_drop: u8,
    /// The server ticks between two steps of its flow.
    pub ticks_per_step: u32,
}

/// Water reaches 7 cells, a block every 0.25 s.
pub const WATER: Liquid = Liquid {
    block: BlockId::Water,
    level_drop: 1,
    ticks_per_step: 5,
};

/// Lava reaches 3 cells, at the levels 2, 4 and 6 as in Minecraft, a block every 1.5 s.
pub const LAVA: Liquid = Liquid {
    block: BlockId::Lava,
    level_drop: 2,
    ticks_per_step: 30,
};

/// Every liquid.
pub const LIQUIDS: [Liquid; 2] = [WATER, LAVA];

impl Liquid {
    /// The liquid `block` is, if it is one.
    pub fn of(block: BlockId) -> Option<Liquid> {
        LIQUIDS.into_iter().find(|liquid| liquid.block == block)
    }

    /// A cell of this liquid at `level`.
    pub fn at(self, level: u8) -> BlockData {
        BlockData {
            level,
            ..BlockData::new(self.block, BlockDirection::Front)
        }
    }

    fn is(self, block: &BlockData) -> bool {
        block.id == self.block
    }

    /// Whether this liquid flowing can take the place of `block`: air, a decoration that it
    /// washes away, or this liquid where it is not a source. Another liquid is a wall to it.
    fn can_flood(self, block: Option<&BlockData>) -> bool {
        match block {
            None => true,
            Some(block) if self.is(block) => block.level != SOURCE_LEVEL,
            Some(block) => block.id.get_visibility() == BlockTransparency::Decoration,
        }
    }

    /// The level of this liquid that the cell at `pos` gets from its neighbours, `None` if
    /// none feeds it.
    fn fed_level(self, world_map: &ChunkMap, pos: IVec3) -> Option<u8> {
        if world_map
            .get_block(pos + IVec3::Y)
            .is_some_and(|above| self.is(above))
        {
            return Some(FALLING_LEVEL);
        }
        [IVec3::X, IVec3::NEG_X, IVec3::Z, IVec3::NEG_Z]
            .into_iter()
            .filter_map(|offset| {
                let neighbour = pos + offset;
                let block = world_map
                    .get_block(neighbour)
                    .filter(|block| self.is(block))?;
                if !self.rests_on_something(world_map, neighbour) {
                    return None;
                }
                let spread_from = match block.level {
                    FALLING_LEVEL => SOURCE_LEVEL,
                    level => level,
                };
                Some(spread_from + self.level_drop)
            })
            .min()
            .filter(|&level| level <= MAX_FLOW_LEVEL)
    }

    /// Whether this liquid at `pos` cannot go down: it lies on a block, on a source, or on a
    /// chunk that is not loaded.
    fn rests_on_something(self, world_map: &ChunkMap, pos: IVec3) -> bool {
        let below = pos + IVec3::NEG_Y;
        !world_map.is_chunk_loaded(global_block_to_chunk_pos(&below))
            || !self.can_flood(world_map.get_block(below))
    }
}

/// What one step of the flow of a liquid does to a cell.
#[derive(Debug, PartialEq)]
pub enum FlowChange {
    Set(BlockData),
    Remove,
}

/// What one step of the flow of `liquid` does to the cell at `pos`, from its neighbours as
/// they are, `None` when it stays as it is. Lava that water touches hardens first, whichever
/// liquid steps (`harden`). Else a source, a block that the liquid cannot wash away, another
/// liquid and a cell of a chunk that is not loaded never change.
///
/// A cell gets a falling liquid (`FALLING_LEVEL`) under the liquid, else `level_drop` levels
/// more than the lowest horizontal neighbour that spreads to it, up to `MAX_FLOW_LEVEL`. A
/// liquid spreads sideways only when it cannot go down, and a falling liquid spreads as a
/// source does. A liquid that nothing feeds any more is removed, which empties a flow once its
/// source is gone.
pub fn flow_step(world_map: &ChunkMap, pos: IVec3, liquid: Liquid) -> Option<FlowChange> {
    if !world_map.is_chunk_loaded(global_block_to_chunk_pos(&pos)) {
        return None;
    }
    if let Some(hardened) = harden(world_map, pos) {
        return Some(FlowChange::Set(hardened));
    }
    let current = world_map.get_block(pos);
    if !liquid.can_flood(current) {
        return None;
    }
    let current_level = current
        .filter(|block| liquid.is(block))
        .map(|block| block.level);
    match (current_level, liquid.fed_level(world_map, pos)) {
        (current, fed) if current == fed => None,
        (_, Some(level)) => Some(FlowChange::Set(liquid.at(level))),
        (Some(_), None) => Some(FlowChange::Remove),
        (None, None) => None,
    }
}

/// What the lava or the concrete powder at `pos` hardens into, if water touches it on one of
/// its six sides: a source into obsidian, flowing lava into cobblestone, a powder into the
/// concrete of its colour.
pub fn harden(world_map: &ChunkMap, pos: IVec3) -> Option<BlockData> {
    let block = world_map.get_block(pos)?;
    let concrete = block.id.hardened();
    if concrete.is_none() && !LAVA.is(block) {
        return None;
    }
    let touches_water = SIX_OFFSETS.iter().any(|offset| {
        world_map
            .get_block(pos + *offset)
            .is_some_and(|b| WATER.is(b))
    });
    if !touches_water {
        return None;
    }
    if let Some(concrete) = concrete {
        return Some(BlockData::new(concrete, block.direction));
    }
    let lava = block;
    let id = if lava.level == SOURCE_LEVEL {
        BlockId::Obsidian
    } else {
        BlockId::Cobblestone
    };
    Some(BlockData::new(id, BlockDirection::Front))
}

/// Whether `block` is the source of a liquid, which a bucket takes and the flow never makes.
pub fn is_source(block: &BlockData) -> bool {
    Liquid::of(block.id).is_some() && block.level == SOURCE_LEVEL
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
    fn an_eye_is_in_a_liquid_only_below_its_surface() {
        let mut world = ChunkMap::with_loaded_region(IVec3::ZERO, IVec3::splat(15));
        let cell = IVec3::new(1, 1, 1);
        let eye = |y: f32| Vec3::new(1.5, y, 1.5);
        assert!(!eye_in_liquid(&world, eye(1.5)));

        world.fill_box(cell, cell, BlockId::Water);
        // A source fills 8/9 of its cell
        assert!(eye_in_liquid(&world, eye(1.5)));
        assert!(eye_in_liquid(&world, eye(1.85)));
        assert!(!eye_in_liquid(&world, eye(1.95)));
        // Feet in the water are not the eyes
        assert!(!eye_in_liquid(&world, eye(2.5)));
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

    fn flow_step_water(world: &ChunkMap, pos: IVec3) -> Option<FlowChange> {
        flow_step(world, pos, WATER)
    }

    fn lava(level: u8) -> BlockData {
        LAVA.at(level)
    }

    fn block(id: BlockId) -> BlockData {
        BlockData::new(id, BlockDirection::Front)
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
            flow_step_water(&world, ON_FLOOR + IVec3::X),
            Some(FlowChange::Set(water(1)))
        );
        world.set_block(ON_FLOOR, water(3));
        assert_eq!(
            flow_step_water(&world, ON_FLOOR + IVec3::X),
            Some(FlowChange::Set(water(4)))
        );
        // Not above, not below, not diagonally
        for pos in [ON_FLOOR + IVec3::Y, ON_FLOOR + IVec3::new(1, 0, 1)] {
            assert_eq!(flow_step_water(&world, pos), None, "{pos}");
        }
    }

    #[test]
    fn the_thinnest_water_spreads_no_further() {
        let mut world = floor();
        world.set_block(ON_FLOOR, water(MAX_FLOW_LEVEL));
        assert_eq!(flow_step_water(&world, ON_FLOOR + IVec3::X), None);
    }

    #[test]
    fn the_lowest_neighbour_sets_the_level() {
        let mut world = floor();
        world.set_block(ON_FLOOR - IVec3::X, water(5));
        world.set_block(ON_FLOOR + IVec3::X, water(2));
        world.set_block(ON_FLOOR, water(6));
        assert_eq!(
            flow_step_water(&world, ON_FLOOR),
            Some(FlowChange::Set(water(3)))
        );
        world.set_block(ON_FLOOR, water(3));
        assert_eq!(flow_step_water(&world, ON_FLOOR), None);
    }

    #[test]
    fn water_falls_instead_of_spreading_and_spreads_where_it_lands() {
        let mut world = floor();
        let high = ON_FLOOR + IVec3::new(0, 3, 0);
        world.set_block(high, water(SOURCE_LEVEL));
        assert_eq!(flow_step_water(&world, high + IVec3::X), None);
        assert_eq!(
            flow_step_water(&world, high - IVec3::Y),
            Some(FlowChange::Set(water(FALLING_LEVEL)))
        );
        world.set_block(ON_FLOOR, water(FALLING_LEVEL));
        assert_eq!(
            flow_step_water(&world, ON_FLOOR + IVec3::X),
            Some(FlowChange::Set(water(1)))
        );
    }

    #[test]
    fn water_that_nothing_feeds_is_removed_but_a_source_stays() {
        let mut world = floor();
        world.set_block(ON_FLOOR, water(2));
        assert_eq!(flow_step_water(&world, ON_FLOOR), Some(FlowChange::Remove));
        world.set_block(ON_FLOOR, water(SOURCE_LEVEL));
        assert_eq!(flow_step_water(&world, ON_FLOOR), None);
    }

    #[test]
    fn water_washes_decorations_away_but_not_blocks() {
        let mut world = floor();
        world.set_block(ON_FLOOR, water(SOURCE_LEVEL));
        world.fill_box(ON_FLOOR + IVec3::X, ON_FLOOR + IVec3::X, BlockId::Poppy);
        world.fill_box(ON_FLOOR - IVec3::X, ON_FLOOR - IVec3::X, BlockId::Glass);
        assert_eq!(
            flow_step_water(&world, ON_FLOOR + IVec3::X),
            Some(FlowChange::Set(water(1)))
        );
        assert_eq!(flow_step_water(&world, ON_FLOOR - IVec3::X), None);
    }

    #[test]
    fn water_does_not_flow_into_a_chunk_that_is_not_loaded() {
        let mut world = floor();
        let edge = IVec3::new(15, 4, 8);
        world.set_block(edge, water(SOURCE_LEVEL));
        assert_eq!(flow_step_water(&world, edge + IVec3::X), None);
    }

    #[test]
    fn lava_spreads_two_levels_a_cell_and_three_cells_far() {
        let mut world = floor();
        world.set_block(ON_FLOOR, lava(SOURCE_LEVEL));
        assert_eq!(
            flow_step(&world, ON_FLOOR + IVec3::X, LAVA),
            Some(FlowChange::Set(lava(2)))
        );
        for (from, to) in [(2, Some(4)), (4, Some(6)), (6, None)] {
            world.set_block(ON_FLOOR, lava(from));
            assert_eq!(
                flow_step(&world, ON_FLOOR + IVec3::X, LAVA),
                to.map(|level| FlowChange::Set(lava(level))),
                "from {from}"
            );
        }
        // Falling lava spreads as a source
        world.set_block(ON_FLOOR, lava(FALLING_LEVEL));
        assert_eq!(
            flow_step(&world, ON_FLOOR + IVec3::X, LAVA),
            Some(FlowChange::Set(lava(2)))
        );
        // Water is no lava: it feeds no lava, and lava feeds no water
        assert_eq!(flow_step_water(&world, ON_FLOOR + IVec3::X), None);
    }

    #[test]
    fn lava_and_water_do_not_flood_each_other() {
        let mut world = floor();
        world.set_block(ON_FLOOR, water(SOURCE_LEVEL));
        world.set_block(ON_FLOOR + IVec3::X * 2, lava(SOURCE_LEVEL));
        // between them, a cell each would flow into: water's step brings water
        let between = ON_FLOOR + IVec3::X;
        assert_eq!(
            flow_step_water(&world, between),
            Some(FlowChange::Set(water(1)))
        );
        // flowing water is a wall to lava, and flowing lava to water
        world.set_block(between, water(1));
        let beyond = ON_FLOOR + IVec3::X * 3;
        world.set_block(beyond, water(3));
        assert_eq!(flow_step(&world, beyond, LAVA), None);
        world.remove_block(between);
        world.set_block(beyond, lava(2));
        assert_eq!(flow_step_water(&world, beyond), None);
    }

    #[test]
    fn water_hardens_a_lava_source_into_obsidian_and_flowing_lava_into_cobblestone() {
        for (level, hardened) in [
            (SOURCE_LEVEL, BlockId::Obsidian),
            (2, BlockId::Cobblestone),
            (FALLING_LEVEL, BlockId::Cobblestone),
        ] {
            for side in SIX_OFFSETS {
                let mut world = floor();
                let pos = ON_FLOOR + IVec3::Y;
                world.set_block(pos, lava(level));
                world.set_block(pos + side, water(3));
                // at the step of either liquid
                for liquid in LIQUIDS {
                    assert_eq!(
                        flow_step(&world, pos, liquid),
                        Some(FlowChange::Set(block(hardened))),
                        "lava {level}, water at {side}, {liquid:?}"
                    );
                }
            }
        }
        let mut world = floor();
        world.set_block(ON_FLOOR, lava(SOURCE_LEVEL));
        world.set_block(ON_FLOOR + IVec3::new(1, 0, 1), water(SOURCE_LEVEL));
        // not diagonally
        assert_eq!(harden(&world, ON_FLOOR), None);
        assert_eq!(harden(&world, ON_FLOOR + IVec3::new(1, 0, 1)), None);
    }

    #[test]
    fn a_source_of_either_liquid_is_a_source() {
        assert!(is_source(&water(SOURCE_LEVEL)));
        assert!(is_source(&lava(SOURCE_LEVEL)));
        assert!(!is_source(&lava(2)));
        assert!(!is_source(&block(BlockId::Stone)));
        assert_eq!(Liquid::of(BlockId::Lava), Some(LAVA));
        assert_eq!(Liquid::of(BlockId::Stone), None);
    }

    #[test]
    fn the_eye_names_the_liquid_it_is_in() {
        let mut world = floor();
        world.set_block(ON_FLOOR, lava(SOURCE_LEVEL));
        let eye = ON_FLOOR.as_vec3() + Vec3::new(0.5, 0.5, 0.5);
        assert_eq!(eye_liquid(&world, eye), Some(BlockId::Lava));
        assert!(eye_in_liquid(&world, eye));
        assert_eq!(eye_liquid(&world, eye + Vec3::Y), None);
    }

    #[test]
    fn a_surface_of_water_is_not_pulled_up_or_down_by_lava_beside_it() {
        let mut world = floor();
        world.set_block(ON_FLOOR, water(SOURCE_LEVEL));
        let alone = corner_heights(&world, ON_FLOOR).unwrap();
        world.set_block(ON_FLOOR + IVec3::X, lava(6));
        let walled = corner_heights(&world, ON_FLOOR).unwrap();
        assert_eq!(walled[0], alone[0]);
        // lava beside it counts as a wall, not as air or water
        world.fill_box(ON_FLOOR + IVec3::X, ON_FLOOR + IVec3::X, BlockId::Stone);
        assert_eq!(corner_heights(&world, ON_FLOOR).unwrap(), walled);
    }
}
