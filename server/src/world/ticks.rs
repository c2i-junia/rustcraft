//! Random block ticks, and the fires' own ticks, simulated by the server only: the cells they
//! change reach the clients as block updates, and are saved, like the blocks changed by the
//! players. What a block does on its tick is shared (`shared::world::ticks::random_tick`).

use std::collections::HashMap;
use std::ops::RangeInclusive;

use bevy::prelude::*;
use rand::Rng;
use shared::{
    messages::PlayerId,
    players::Player,
    world::{
        render_distance_offsets, ticks::random_tick, ticks::BlockChange, ticks::TickContext,
        ticks::RANDOM_TICKS_PER_SECTION, world_position_to_chunk_position, BlockId, ChunkMap,
        WeatherKind,
    },
    CHUNK_SIZE,
};

use super::data::ServerWorldMap;
use super::weather::ServerWeather;

/// How far from a player, in chunks, the blocks get random ticks: a cube of 9 chunks a side
/// around its chunk, or its render distance when lower.
pub const SIMULATION_DISTANCE: i32 = 4;
/// Most fires that tick in one tick of the server. The others wait for the next, so that a
/// forest on fire does not stall a tick.
pub const MAX_FIRE_UPDATES: usize = 64;
/// The ticks between two ticks of a fire, as Minecraft's: a random tick, which a cell gets once
/// in 1365 ticks on average, would leave it still for a minute.
const FIRE_TICK_INTERVAL: RangeInclusive<u32> = 30..=40;

/// The fires the server knows, and the ticks each has left before its next tick.
///
/// They are not saved: a fire loaded with its chunk waits for a random tick to land on it, a
/// minute on average, before it is scheduled again.
#[derive(Resource, Default)]
pub struct BlockTicks {
    fires: HashMap<IVec3, u32>,
}

/// Gives the blocks near the players their random ticks, and the fires their own.
pub fn block_ticks_system(
    mut ticks: ResMut<BlockTicks>,
    mut world_map: ResMut<ServerWorldMap>,
    weather: Res<ServerWeather>,
) {
    let context = TickContext {
        raining: weather.kind != WeatherKind::Clear,
    };
    let ServerWorldMap {
        chunks,
        players,
        render_distances,
        ..
    } = &mut *world_map;
    let radius = |id| render_distances.get(id).min(SIMULATION_DISTANCE);
    ticks.tick(chunks, players, radius, &context, &mut rand::thread_rng());
}

/// The cells that get a random tick in this tick: `RANDOM_TICKS_PER_SECTION` drawn in each
/// loaded chunk within `radius` of a player, each chunk once, but the chunks of air only.
fn random_tick_cells(
    chunks: &ChunkMap,
    players: &HashMap<PlayerId, Player>,
    radius: impl Fn(PlayerId) -> i32,
    rng: &mut impl Rng,
) -> Vec<IVec3> {
    let mut around = std::collections::HashSet::new();
    for (&id, player) in players {
        let centre = world_position_to_chunk_position(player.position);
        around.extend(
            render_distance_offsets(radius(id))
                .iter()
                .map(|offset| centre + *offset),
        );
    }
    let mut cells = Vec::new();
    for chunk_pos in around {
        let Some(chunk) = chunks.chunk(chunk_pos) else {
            continue;
        };
        if chunk.uniform() == Some(None) {
            continue;
        }
        for _ in 0..RANDOM_TICKS_PER_SECTION {
            let local = IVec3::new(
                rng.gen_range(0..CHUNK_SIZE),
                rng.gen_range(0..CHUNK_SIZE),
                rng.gen_range(0..CHUNK_SIZE),
            );
            cells.push(chunk_pos * CHUNK_SIZE + local);
        }
    }
    cells
}

fn is_fire(chunks: &ChunkMap, pos: IVec3) -> bool {
    chunks
        .get_block(pos)
        .is_some_and(|block| block.id == BlockId::Fire)
}

impl BlockTicks {
    /// One tick: schedules the fires lit since the last one, gives the random ticks, then ticks
    /// up to `MAX_FIRE_UPDATES` of the fires whose time has come.
    fn tick(
        &mut self,
        chunks: &mut ChunkMap,
        players: &HashMap<PlayerId, Player>,
        radius: impl Fn(PlayerId) -> i32,
        context: &TickContext,
        rng: &mut impl Rng,
    ) {
        // Lit by a player during this tick
        let lit: Vec<IVec3> = chunks
            .changed_blocks()
            .filter(|&pos| is_fire(chunks, pos))
            .collect();
        for pos in lit {
            self.schedule(pos, rng);
        }

        for pos in random_tick_cells(chunks, players, radius, rng) {
            if is_fire(chunks, pos) {
                // a fire found here is ticked on its schedule, never twice
                self.schedule(pos, rng);
                continue;
            }
            for change in random_tick(chunks, pos, context, rng) {
                self.apply(chunks, change, rng);
            }
        }

        let mut due = Vec::new();
        for (&pos, ticks_left) in &mut self.fires {
            *ticks_left = ticks_left.saturating_sub(1);
            if *ticks_left == 0 && due.len() < MAX_FIRE_UPDATES {
                due.push(pos);
            }
        }
        for pos in due {
            self.fires.remove(&pos);
            if !is_fire(chunks, pos) {
                continue;
            }
            for change in random_tick(chunks, pos, context, rng) {
                self.apply(chunks, change, rng);
            }
        }
    }

    /// Makes the fire at `pos` tick in `FIRE_TICK_INTERVAL` ticks, unless it is scheduled.
    fn schedule(&mut self, pos: IVec3, rng: &mut impl Rng) {
        self.fires
            .entry(pos)
            .or_insert_with(|| rng.gen_range(FIRE_TICK_INTERVAL));
    }

    /// Applies `change`, and schedules the fire it lights or keeps.
    fn apply(&mut self, chunks: &mut ChunkMap, change: BlockChange, rng: &mut impl Rng) {
        match change.block {
            Some(block) => {
                chunks.set_block(change.pos, block);
                if block.id == BlockId::Fire {
                    self.schedule(change.pos, rng);
                }
            }
            None => {
                chunks.remove_block(change.pos);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::{rngs::StdRng, SeedableRng};
    use shared::world::{chunk_distance, global_block_to_chunk_pos, BlockData, BlockDirection};

    /// A stone floor whose top is at y = 0, over chunks x and z from -2 to 5.
    fn floor() -> ChunkMap {
        let mut world = ChunkMap::with_loaded_region(IVec3::new(-32, -16, -32), IVec3::splat(95));
        world.fill_box(
            IVec3::new(-32, -16, -32),
            IVec3::new(95, 0, 95),
            BlockId::Stone,
        );
        world.take_changed_blocks();
        world
    }

    fn player_at(position: Vec3) -> HashMap<PlayerId, Player> {
        HashMap::from([(
            1,
            Player {
                id: 1,
                position,
                ..Default::default()
            },
        )])
    }

    fn fire(age: u8) -> BlockData {
        BlockData {
            level: age,
            ..BlockData::new(BlockId::Fire, BlockDirection::Front)
        }
    }

    fn age(world: &ChunkMap, pos: IVec3) -> Option<u8> {
        world
            .get_block(pos)
            .filter(|block| block.id == BlockId::Fire)
            .map(|block| block.level)
    }

    #[test]
    fn random_ticks_fall_in_the_chunks_near_a_player_three_per_chunk_but_air() {
        let world = floor();
        let players = player_at(Vec3::new(8.0, 4.0, 8.0));
        let cells = random_tick_cells(
            &world,
            &players,
            |_| SIMULATION_DISTANCE,
            &mut StdRng::seed_from_u64(1),
        );
        let mut per_chunk: HashMap<IVec3, usize> = HashMap::new();
        for cell in &cells {
            *per_chunk
                .entry(global_block_to_chunk_pos(cell))
                .or_default() += 1;
        }
        // Only the floor's chunks, y = -1 and 0, from x and z = -2 (the floor's edge) to 4
        assert_eq!(per_chunk.len(), 7 * 7 * 2);
        for (chunk, count) in per_chunk {
            assert!(
                chunk_distance(chunk, IVec3::ZERO) <= SIMULATION_DISTANCE,
                "{chunk}"
            );
            assert!(chunk.y == 0 || chunk.y == -1, "{chunk}: air");
            assert_eq!(count, RANDOM_TICKS_PER_SECTION);
        }
        // A lower render distance narrows them
        let near = random_tick_cells(&world, &players, |_| 1, &mut StdRng::seed_from_u64(1));
        assert_eq!(near.len(), 3 * 3 * 2 * RANDOM_TICKS_PER_SECTION);
    }

    /// Runs `ticks` ticks with a player in the middle of the floor, each followed by the
    /// broadcast that empties the log.
    fn run(ticks: &mut BlockTicks, world: &mut ChunkMap, count: u32, rng: &mut StdRng) {
        let players = player_at(Vec3::new(40.0, 4.0, 40.0));
        for _ in 0..count {
            ticks.tick(
                world,
                &players,
                |_| SIMULATION_DISTANCE,
                &TickContext::default(),
                rng,
            );
            world.take_changed_blocks();
        }
    }

    #[test]
    fn a_fire_lit_by_a_player_ticks_within_two_seconds() {
        let mut world = floor();
        let mut ticks = BlockTicks::default();
        let mut rng = StdRng::seed_from_u64(3);
        let pos = IVec3::new(40, 1, 40);
        world.set_block(pos, fire(0));
        run(&mut ticks, &mut world, 1, &mut rng);
        assert_eq!(age(&world, pos), Some(0));
        run(&mut ticks, &mut world, *FIRE_TICK_INTERVAL.end(), &mut rng);
        assert_eq!(age(&world, pos), Some(1));
        // and again, until it goes out on the stone
        run(
            &mut ticks,
            &mut world,
            20 * FIRE_TICK_INTERVAL.end(),
            &mut rng,
        );
        assert_eq!(age(&world, pos), None);
        assert!(ticks.fires.is_empty());
    }

    #[test]
    fn at_most_64_fires_tick_in_a_tick_and_the_others_in_the_next() {
        let mut world = floor();
        let mut ticks = BlockTicks::default();
        let cells: Vec<IVec3> = (0..100)
            .map(|i| IVec3::new(i % 10 * 2, 1, i / 10 * 2))
            .collect();
        for &pos in &cells {
            world.set_block(pos, fire(0));
            ticks.fires.insert(pos, 1);
        }
        world.take_changed_blocks();
        let mut rng = StdRng::seed_from_u64(5);

        run(&mut ticks, &mut world, 1, &mut rng);
        let aged = || {
            cells
                .iter()
                .filter(|&&pos| age(&world, pos) == Some(1))
                .count()
        };
        assert_eq!(aged(), MAX_FIRE_UPDATES);
        run(&mut ticks, &mut world, 1, &mut rng);
        assert_eq!(
            cells
                .iter()
                .filter(|&&pos| age(&world, pos) == Some(1))
                .count(),
            100
        );
    }

    #[test]
    fn a_fire_found_by_a_random_tick_is_scheduled() {
        // A layer of fires on stone, near the player, unknown to the schedule: random ticks
        // land on some of them
        let mut world = floor();
        let layer = || (32..48).flat_map(|x| (32..48).map(move |z| IVec3::new(x, 1, z)));
        for pos in layer() {
            world.set_block(pos, fire(0));
        }
        world.take_changed_blocks();
        let mut ticks = BlockTicks::default();
        run(&mut ticks, &mut world, 300, &mut StdRng::seed_from_u64(9));
        let aged = layer().filter(|&pos| age(&world, pos) != Some(0)).count();
        assert!(aged > 0);
    }
}
