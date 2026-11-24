//! The flow of the liquids, simulated by the server only: the cells it changes reach the
//! clients as block updates, and are saved, like the blocks changed by the players.

use std::collections::HashSet;

use bevy::prelude::*;
use shared::world::{
    liquid::{flow_step, FlowChange, Liquid, LAVA, WATER},
    ChunkMap, SIX_OFFSETS,
};

use super::data::ServerWorldMap;

/// Most cells one step of a liquid updates. The others wait for the next step, so that a breach
/// in an ocean does not stall a tick.
const MAX_CELLS_PER_STEP: usize = 4096;

/// The cells whose liquid may change at the next step of each liquid, water and lava, which
/// step every `Liquid::ticks_per_step` ticks.
///
/// They are not saved: a liquid at rest is stable, and a flow that a stop of the server cut
/// short resumes when a block next to it changes. A generated chunk changes no cell, so the
/// lava lakes of the caves are never scheduled until something touches them.
#[derive(Resource)]
pub struct LiquidFlow {
    water: Schedule,
    lava: Schedule,
}

impl Default for LiquidFlow {
    fn default() -> Self {
        Self {
            water: Schedule::new(WATER),
            lava: Schedule::new(LAVA),
        }
    }
}

/// The cells one liquid may change at its next step.
struct Schedule {
    liquid: Liquid,
    scheduled: HashSet<IVec3>,
    ticks_to_next_step: u32,
}

/// Runs a step of each liquid when its time comes, then schedules every cell changed during
/// this tick, by a player or by a flow, and its six neighbours, for both liquids: water may
/// harden lava, and either may flow into the cell.
pub fn liquid_flow_system(mut flow: ResMut<LiquidFlow>, mut world_map: ResMut<ServerWorldMap>) {
    flow.tick(&mut world_map.chunks);
}

impl LiquidFlow {
    fn tick(&mut self, chunks: &mut ChunkMap) {
        self.water.tick(chunks);
        self.lava.tick(chunks);

        for pos in chunks.changed_blocks() {
            for schedule in [&mut self.water, &mut self.lava] {
                schedule.scheduled.insert(pos);
                schedule
                    .scheduled
                    .extend(SIX_OFFSETS.iter().map(|offset| pos + *offset));
            }
        }
    }
}

impl Schedule {
    fn new(liquid: Liquid) -> Self {
        Self {
            liquid,
            scheduled: HashSet::new(),
            ticks_to_next_step: 0,
        }
    }

    fn tick(&mut self, chunks: &mut ChunkMap) {
        if self.ticks_to_next_step == 0 {
            self.ticks_to_next_step = self.liquid.ticks_per_step;
            self.step(chunks);
        }
        self.ticks_to_next_step -= 1;
    }

    /// Moves the liquid of up to `MAX_CELLS_PER_STEP` scheduled cells. Every cell is computed
    /// from the map as it was before the step, so the order of the cells does not matter.
    fn step(&mut self, chunks: &mut ChunkMap) {
        let cells: Vec<IVec3> = self
            .scheduled
            .iter()
            .take(MAX_CELLS_PER_STEP)
            .copied()
            .collect();
        for pos in &cells {
            self.scheduled.remove(pos);
        }

        let changes: Vec<(IVec3, FlowChange)> = cells
            .into_iter()
            .filter_map(|pos| Some((pos, flow_step(chunks, pos, self.liquid)?)))
            .collect();
        for (pos, change) in changes {
            match change {
                FlowChange::Set(block) => chunks.set_block(pos, block),
                FlowChange::Remove => {
                    chunks.remove_block(pos);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::world::{liquid::MAX_FLOW_LEVEL, BlockData, BlockDirection, BlockId};

    const TICKS_PER_FLOW_STEP: u32 = WATER.ticks_per_step;

    /// A stone floor whose top is at y = 4, from x and z = -16 to 31.
    fn floor() -> ChunkMap {
        let mut world = ChunkMap::with_loaded_region(IVec3::new(-16, 0, -16), IVec3::splat(31));
        world.fill_box(
            IVec3::new(-16, 0, -16),
            IVec3::new(31, 3, 31),
            BlockId::Stone,
        );
        world.take_changed_blocks();
        world
    }

    const ON_FLOOR: IVec3 = IVec3::new(8, 4, 8);

    fn source() -> BlockData {
        BlockData::new(BlockId::Water, BlockDirection::Front)
    }

    /// Runs `ticks` ticks of the server, each followed by the broadcast that empties the log.
    fn run(flow: &mut LiquidFlow, world: &mut ChunkMap, ticks: u32) {
        for _ in 0..ticks {
            flow.tick(world);
            world.take_changed_blocks();
        }
    }

    fn water_level(world: &ChunkMap, pos: IVec3) -> Option<u8> {
        world
            .get_block(pos)
            .filter(|block| block.id == BlockId::Water)
            .map(|block| block.level)
    }

    #[test]
    fn a_source_spreads_seven_blocks_around_it_one_block_per_step() {
        let mut world = floor();
        let mut flow = LiquidFlow::default();
        world.set_block(ON_FLOOR, source());

        // The first step comes at once, then one every five ticks
        run(&mut flow, &mut world, 1 + TICKS_PER_FLOW_STEP);
        assert_eq!(water_level(&world, ON_FLOOR + IVec3::X), Some(1));
        assert_eq!(water_level(&world, ON_FLOOR + IVec3::X * 2), None);

        run(&mut flow, &mut world, 20 * TICKS_PER_FLOW_STEP);
        for distance in 1..=7 {
            let level = Some(distance as u8);
            assert_eq!(water_level(&world, ON_FLOOR - IVec3::Z * distance), level);
            assert_eq!(
                water_level(&world, ON_FLOOR + IVec3::new(distance - 1, 0, 1)),
                level
            );
        }
        assert_eq!(water_level(&world, ON_FLOOR + IVec3::X * 8), None);
        assert_eq!(water_level(&world, ON_FLOOR + IVec3::Y), None);
        assert!(flow.water.scheduled.is_empty(), "the flow settled");
    }

    #[test]
    fn a_flow_drains_once_its_source_is_gone() {
        let mut world = floor();
        let mut flow = LiquidFlow::default();
        world.set_block(ON_FLOOR, source());
        run(&mut flow, &mut world, 20 * TICKS_PER_FLOW_STEP);

        world.remove_block(ON_FLOOR);
        run(&mut flow, &mut world, 40 * TICKS_PER_FLOW_STEP);

        let water = (-16..32)
            .flat_map(|x| (-16..32).map(move |z| IVec3::new(x, 4, z)))
            .filter(|&pos| water_level(&world, pos).is_some())
            .count();
        assert_eq!(water, 0);
    }

    #[test]
    fn water_poured_from_above_falls_to_the_floor_and_spreads_there() {
        let mut world = floor();
        let mut flow = LiquidFlow::default();
        let high = ON_FLOOR + IVec3::Y * 6;
        world.set_block(high, source());
        run(&mut flow, &mut world, 30 * TICKS_PER_FLOW_STEP);

        for y in 4..10 {
            assert!(water_level(&world, ON_FLOOR.with_y(y)).is_some(), "y = {y}");
        }
        assert_eq!(
            water_level(&world, ON_FLOOR + IVec3::X * MAX_FLOW_LEVEL as i32),
            Some(MAX_FLOW_LEVEL)
        );
        assert_eq!(water_level(&world, high + IVec3::NEG_Y + IVec3::X), None);
    }

    #[test]
    fn a_trench_dug_from_a_lake_fills_with_water() {
        let mut world = floor();
        let mut flow = LiquidFlow::default();
        // A lake of sources from z = 0 to 4, in a stone bank one block high
        world.fill_box(
            IVec3::new(-16, 4, -16),
            IVec3::new(31, 4, 31),
            BlockId::Stone,
        );
        for x in 0..8 {
            for z in 0..5 {
                world.set_block(IVec3::new(x, 4, z), source());
            }
        }
        world.take_changed_blocks();
        run(&mut flow, &mut world, 10 * TICKS_PER_FLOW_STEP);
        assert!(
            flow.water.scheduled.is_empty(),
            "a lake at rest does nothing"
        );

        // A trench three blocks long from the lake toward +Z
        for z in 5..8 {
            world.remove_block(IVec3::new(3, 4, z));
        }
        run(&mut flow, &mut world, 10 * TICKS_PER_FLOW_STEP);
        for (z, level) in [(5, 1), (6, 2), (7, 3)] {
            assert_eq!(water_level(&world, IVec3::new(3, 4, z)), Some(level));
        }

        // Blocking the trench at its mouth drains what is behind
        world.set_block(
            IVec3::new(3, 4, 5),
            BlockData::new(BlockId::Dirt, BlockDirection::Front),
        );
        run(&mut flow, &mut world, 20 * TICKS_PER_FLOW_STEP);
        assert_eq!(water_level(&world, IVec3::new(3, 4, 6)), None);
        assert_eq!(water_level(&world, IVec3::new(3, 4, 7)), None);
    }

    #[test]
    fn a_step_updates_a_bounded_number_of_cells_and_leaves_the_rest_for_later() {
        let mut world = floor();
        let mut flow = Schedule::new(WATER);
        flow.scheduled = (0..MAX_CELLS_PER_STEP as i32 + 10)
            .map(|i| IVec3::new(i, 100, 0))
            .collect();
        flow.step(&mut world);
        assert_eq!(flow.scheduled.len(), 10);
    }

    #[test]
    fn the_first_step_after_a_change_is_at_most_five_ticks_away() {
        let mut world = floor();
        let mut flow = LiquidFlow::default();
        run(&mut flow, &mut world, 3);
        world.set_block(ON_FLOOR, source());
        run(&mut flow, &mut world, TICKS_PER_FLOW_STEP);
        assert_eq!(water_level(&world, ON_FLOOR + IVec3::X), Some(1));
    }

    fn lava_level(world: &ChunkMap, pos: IVec3) -> Option<u8> {
        world
            .get_block(pos)
            .filter(|block| block.id == BlockId::Lava)
            .map(|block| block.level)
    }

    fn id_at(world: &ChunkMap, pos: IVec3) -> Option<BlockId> {
        world.get_block(pos).map(|block| block.id)
    }

    #[test]
    fn a_lava_source_spreads_three_blocks_one_block_every_thirty_ticks() {
        let mut world = floor();
        let mut flow = LiquidFlow::default();
        world.set_block(ON_FLOOR, LAVA.at(0));

        run(&mut flow, &mut world, 1 + LAVA.ticks_per_step - 1);
        assert_eq!(lava_level(&world, ON_FLOOR + IVec3::X), None, "not yet");
        run(&mut flow, &mut world, 1);
        assert_eq!(lava_level(&world, ON_FLOOR + IVec3::X), Some(2));

        run(&mut flow, &mut world, 10 * LAVA.ticks_per_step);
        for (distance, level) in [(1, 2), (2, 4), (3, 6)] {
            assert_eq!(
                lava_level(&world, ON_FLOOR - IVec3::Z * distance),
                Some(level)
            );
        }
        assert_eq!(lava_level(&world, ON_FLOOR + IVec3::X * 4), None);
        assert!(flow.lava.scheduled.is_empty(), "the flow settled");
    }

    #[test]
    fn water_poured_by_a_lava_lake_hardens_its_shore_within_a_water_step() {
        let mut world = floor();
        let mut flow = LiquidFlow::default();
        // a lake of lava sources at rest
        for x in 0..4 {
            world.set_block(IVec3::new(x, 4, 0), LAVA.at(0));
        }
        world.take_changed_blocks();
        run(&mut flow, &mut world, 2 * LAVA.ticks_per_step);
        assert!(flow.water.scheduled.is_empty() && flow.lava.scheduled.is_empty());

        // water poured two cells away flows to the lake, which hardens where it touches
        world.set_block(IVec3::new(1, 4, 2), WATER.at(0));
        run(&mut flow, &mut world, 2 * TICKS_PER_FLOW_STEP + 1);
        assert_eq!(id_at(&world, IVec3::new(1, 4, 0)), Some(BlockId::Obsidian));
        assert_eq!(
            lava_level(&world, IVec3::new(3, 4, 0)),
            Some(0),
            "out of reach"
        );
    }

    #[test]
    fn flowing_lava_that_meets_water_turns_into_cobblestone() {
        let mut world = floor();
        let mut flow = LiquidFlow::default();
        world.set_block(ON_FLOOR, LAVA.at(0));
        world.set_block(ON_FLOOR + IVec3::X * 3, WATER.at(0));
        // a channel along x, one block wide, closed at both ends
        for z in [-1, 1] {
            world.fill_box(
                ON_FLOOR + IVec3::new(-1, 0, z),
                ON_FLOOR + IVec3::new(4, 0, z),
                BlockId::Stone,
            );
        }
        for x in [-1, 4] {
            world.fill_box(
                ON_FLOOR + IVec3::X * x,
                ON_FLOOR + IVec3::X * x,
                BlockId::Stone,
            );
        }
        // stone between them until the lava has flowed
        let gap = ON_FLOOR + IVec3::X * 2;
        world.fill_box(gap, gap, BlockId::Stone);
        // just past a step of the lava
        run(&mut flow, &mut world, 2 * LAVA.ticks_per_step + 1);
        assert_eq!(lava_level(&world, ON_FLOOR + IVec3::X), Some(2));

        // water reaches the flowing lava, long before the lava would move on
        world.remove_block(gap);
        run(&mut flow, &mut world, 3 * TICKS_PER_FLOW_STEP);
        assert_eq!(
            id_at(&world, ON_FLOOR + IVec3::X),
            Some(BlockId::Cobblestone)
        );
        assert_eq!(lava_level(&world, ON_FLOOR), Some(0));
    }

    #[test]
    fn a_chunk_loaded_full_of_lava_schedules_nothing() {
        let mut world = ChunkMap::default();
        let mut flow = LiquidFlow::default();
        let mut chunk = shared::world::Chunk::filled(LAVA.at(0));
        chunk.remove(IVec3::new(3, 15, 3));
        world.insert_chunk(IVec3::ZERO, chunk);
        run(&mut flow, &mut world, 2);
        assert!(flow.water.scheduled.is_empty() && flow.lava.scheduled.is_empty());
    }
}
