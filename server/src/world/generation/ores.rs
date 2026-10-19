//! Ores: clusters and veins scattered through the stone. Each chunk holds a few attempts per
//! ore, hashed from its position; an attempt may reach `ORE_REACH` blocks out of its chunk, so
//! a chunk replays the attempts of its 26 neighbours too and keeps the cells that fall in it.

use bevy::prelude::*;
use shared::{world::*, CHUNK_SIZE};

use super::PositionRandom;

/// The decisions of one chunk, by its position.
type ChunkRandom = PositionRandom<3>;

/// How one ore is scattered.
struct Ore {
    block: BlockId,
    /// The heights an attempt may start from, both included.
    lowest: i32,
    highest: i32,
    /// Attempts per chunk.
    attempts: u64,
    /// Sizes of a cluster and lengths of a vein, both included.
    cluster: (u64, u64),
    vein: (u64, u64),
    /// Share of the attempts that are veins, the others are clusters.
    veins: f32,
}

/// The commonest first: a rarer ore is placed after, and wins a cell both claim.
const ORES: [Ore; 3] = [
    Ore {
        block: BlockId::CoalOre,
        lowest: 5,
        highest: 128,
        attempts: 6,
        cluster: (6, 14),
        vein: (8, 14),
        veins: 0.2,
    },
    Ore {
        block: BlockId::IronOre,
        lowest: 5,
        highest: 64,
        attempts: 5,
        cluster: (4, 8),
        vein: (6, 12),
        veins: 0.4,
    },
    Ore {
        block: BlockId::GoldOre,
        lowest: 5,
        highest: 32,
        attempts: 1,
        cluster: (3, 6),
        vein: (6, 10),
        veins: 0.5,
    },
];

/// The farthest a cell of an attempt lies from where it starts, on any axis: half the
/// longest vein. A cluster stays within one block.
const ORE_REACH: i32 = 7;

/// Each attempt of each ore draws its decisions from `SALT_ORE + ore * 0x1000 + attempt *
/// 0x40`, plus up to `0x40` more.
const SALT_ORE: u64 = 0x1000;
/// Where an attempt starts in its chunk: 4 bits for each of x, y and z.
const SALT_START: u64 = 0;
const SALT_KIND: u64 = 3;
const SALT_SIZE: u64 = 4;
const SALT_YAW: u64 = 5;
const SALT_PITCH: u64 = 6;
/// The cells of the 3x3x3 cube around a cluster's start draw one byte each, 8 per salt, from
/// here.
const SALT_CLUSTER_CELLS: u64 = 0x10;

/// Turns into ore the stone of `chunk` that the attempts of it and of its neighbours reach.
pub(super) fn place_ores(seed: u32, chunk_pos: IVec3, chunk: &mut Chunk) {
    let origin = chunk_pos * CHUNK_SIZE;
    let lowest = ORES.iter().map(|ore| ore.lowest).min().unwrap();
    let highest = ORES.iter().map(|ore| ore.highest).max().unwrap();
    if origin.y > highest + ORE_REACH || origin.y + CHUNK_SIZE + ORE_REACH <= lowest {
        return;
    }
    for (rank, ore) in ORES.iter().enumerate() {
        let lower_ranks = &ORES[..rank];
        for neighbour in neighbours(chunk_pos) {
            for cell in attempts_cells(seed, neighbour, rank, Some(chunk_pos)) {
                let local = cell - origin;
                if !Chunk::contains(local) {
                    continue;
                }
                let replaced = chunk.get(local).is_some_and(|block| {
                    block.id == BlockId::Stone || lower_ranks.iter().any(|o| o.block == block.id)
                });
                if replaced {
                    chunk.set(local, BlockData::new(ore.block, BlockDirection::Front));
                }
            }
        }
    }
}

/// The chunk and the 26 around it.
fn neighbours(chunk_pos: IVec3) -> impl Iterator<Item = IVec3> {
    (-1..=1).flat_map(move |x| {
        (-1..=1).flat_map(move |y| (-1..=1).map(move |z| chunk_pos + IVec3::new(x, y, z)))
    })
}

/// The cells of every attempt of the ore `rank` started in the chunk `chunk_pos`; only of
/// those that may reach the chunk `reaching`, if given.
fn attempts_cells(seed: u32, chunk_pos: IVec3, rank: usize, reaching: Option<IVec3>) -> Vec<IVec3> {
    let ore = &ORES[rank];
    let bottom = chunk_pos.y * CHUNK_SIZE;
    if bottom > ore.highest || bottom + CHUNK_SIZE <= ore.lowest {
        return Vec::new();
    }
    let random = ChunkRandom {
        seed,
        coords: chunk_pos.to_array(),
    };
    let mut cells = Vec::new();
    for attempt in 0..ore.attempts {
        let salt = SALT_ORE + rank as u64 * 0x1000 + attempt * 0x40;
        let bits = random.bits(salt + SALT_START);
        let coordinate = |shift: u32| (bits >> shift) as i32 & (CHUNK_SIZE - 1);
        let start =
            chunk_pos * CHUNK_SIZE + IVec3::new(coordinate(0), coordinate(4), coordinate(8));
        if !(ore.lowest..=ore.highest).contains(&start.y) {
            continue;
        }
        let out_of_reach = reaching.is_some_and(|target| {
            let near = target * CHUNK_SIZE - IVec3::splat(ORE_REACH);
            let far = (target + IVec3::ONE) * CHUNK_SIZE + IVec3::splat(ORE_REACH);
            start.cmplt(near).any() || start.cmpge(far).any()
        });
        if out_of_reach {
            continue;
        }
        let size = |(least, most): (u64, u64)| {
            least + random.below(salt + SALT_SIZE, most - least + 1) as u64
        };
        if random.unit(salt + SALT_KIND) < ore.veins {
            vein(&random, salt, start, size(ore.vein), &mut cells);
        } else {
            cluster(&random, salt, start, size(ore.cluster), &mut cells);
        }
    }
    cells
}

/// `size` cells of the 3x3x3 cube around `start`: its centre, then the cells the nearest to
/// it, shuffled.
fn cluster(random: &ChunkRandom, salt: u64, start: IVec3, size: u64, cells: &mut Vec<IVec3>) {
    let bytes: Vec<u8> = (0..4)
        .flat_map(|i| random.bits(salt + SALT_CLUSTER_CELLS + i).to_le_bytes())
        .collect();
    let mut cube: Vec<(f32, IVec3)> = (0..27)
        .map(|i| {
            let offset = IVec3::new(i % 3, i / 3 % 3, i / 9) - IVec3::ONE;
            let key = bytes[i as usize] as f32 / 256.0 + 0.3 * offset.length_squared() as f32;
            (key, offset)
        })
        .collect();
    cube.sort_by(|a, b| a.0.total_cmp(&b.0));
    cells.extend(
        cube.iter()
            .take(size as usize)
            .map(|(_, offset)| start + *offset),
    );
}

/// A straight line of `length` cells through `start`, at a random heading and slope, mostly
/// level.
fn vein(random: &ChunkRandom, salt: u64, start: IVec3, length: u64, cells: &mut Vec<IVec3>) {
    let yaw = random.unit(salt + SALT_YAW) * std::f32::consts::TAU;
    let pitch = (random.unit(salt + SALT_PITCH) - 0.5) * std::f32::consts::FRAC_PI_2;
    let direction = Vec3::new(
        yaw.cos() * pitch.cos(),
        pitch.sin(),
        yaw.sin() * pitch.cos(),
    );
    let length = length as i32;
    for step in -(length - 1) / 2..=length / 2 {
        cells.push(start + (direction * step as f32).round().as_ivec3());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::generation::WorldGenerator;

    fn ore_rank(block: BlockId) -> Option<usize> {
        ORES.iter().position(|ore| ore.block == block)
    }

    #[test]
    fn no_attempt_reaches_further_than_the_reach() {
        for seed in 0..20 {
            for rank in 0..ORES.len() {
                for chunk_pos in [IVec3::new(0, 1, 0), IVec3::new(-3, 2, 5)] {
                    let box_min = chunk_pos * CHUNK_SIZE - IVec3::splat(ORE_REACH);
                    let box_max = (chunk_pos + IVec3::ONE) * CHUNK_SIZE + IVec3::splat(ORE_REACH);
                    for cell in attempts_cells(seed, chunk_pos, rank, None) {
                        assert!(
                            cell.cmpge(box_min).all() && cell.cmplt(box_max).all(),
                            "{cell}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn an_attempt_crossing_a_border_is_whole_in_every_chunk_it_reaches() {
        let generator = WorldGenerator::new(42);
        let source = IVec3::new(2, 1, -1);
        let mut crossing = 0;
        for (rank, ore) in ORES.iter().enumerate() {
            for cell in attempts_cells(42, source, rank, None) {
                let chunk_pos = cell.div_euclid(IVec3::splat(CHUNK_SIZE));
                if chunk_pos == source {
                    continue;
                }
                crossing += 1;
                let chunk = generator.generate_chunk(chunk_pos);
                let block = chunk
                    .get(cell.rem_euclid(IVec3::splat(CHUNK_SIZE)))
                    .map(|b| b.id);
                // the ore, a rarer one, or what was not stone
                let ok = match block.and_then(ore_rank) {
                    Some(found) => found >= rank,
                    None => block != Some(BlockId::Stone),
                };
                assert!(ok, "{:?} at {cell} instead of {:?}", block, ore.block);
            }
        }
        assert!(crossing > 0);
    }

    #[test]
    fn ores_keep_to_their_heights_and_are_as_common_as_meant() {
        let generator = WorldGenerator::new(42);
        let mut counts = [0usize; 3];
        let mut stone = 0usize;
        for x in -2..2 {
            for z in -2..2 {
                for y in 0..9 {
                    let chunk_pos = IVec3::new(x, y, z);
                    for (local, block) in generator.generate_chunk(chunk_pos).iter() {
                        let world_y = y * CHUNK_SIZE + local.y;
                        if block.id == BlockId::Stone {
                            stone += 1;
                        }
                        if let Some(rank) = ore_rank(block.id) {
                            let ore = &ORES[rank];
                            assert!(
                                (ore.lowest - ORE_REACH..=ore.highest + ORE_REACH)
                                    .contains(&world_y),
                                "{:?} at y {world_y}",
                                ore.block
                            );
                            counts[rank] += 1;
                        }
                    }
                }
            }
        }
        let share = |count: usize| count as f64 / (stone + counts.iter().sum::<usize>()) as f64;
        let [coal, iron, gold] = counts.map(share);
        assert!((0.005..0.03).contains(&coal), "coal {coal}");
        assert!((0.002..0.015).contains(&iron), "iron {iron}");
        assert!((0.0002..0.003).contains(&gold), "gold {gold}");
        assert!(coal > iron && iron > gold);
    }
}
