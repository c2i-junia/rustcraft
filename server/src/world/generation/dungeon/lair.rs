//! The lair of the Ember Mother: a huge cavern at the bottom of the world, around a lake of lava
//! with an island in its middle, where she sleeps (`mob::boss`). Like a dungeon it is a function
//! of the seed alone; each chunk it touches carves and fills the cells of it that it holds, after
//! the caves, the ores and the dungeons. The caves it meets open on it above its ground.

use bevy::prelude::*;
use shared::world::lair::{self, FLOOR, HEIGHT, ISLAND, LAKE, RADIUS, REACH, TOP, WAVER};
use shared::world::{BlockData, BlockDirection, BlockId, BlockTransparency, Chunk, TerrainNoises};
use shared::CHUNK_SIZE;

use super::super::PositionRandom;
use crate::world::caves::Noise3;

const SALT_WALLS: u64 = 0x3510;
const SALT_GROUND: u64 = 0x3511;
/// The bed of the lake, and the lowest cells the lair fills.
const BED: i32 = FLOOR - 5;
/// Braziers stand in a ring this far from the centre, this many of them.
const BRAZIERS: i32 = 8;
const BRAZIER_RING: f32 = 25.0;

pub struct Lair {
    seed: u32,
    /// The middle block of its ground.
    pub centre: IVec3,
    walls: Noise3,
}

impl Lair {
    pub fn new(seed: u32, terrain: &TerrainNoises) -> Self {
        Self {
            seed,
            centre: lair::centre(seed, terrain),
            walls: Noise3::new(seed, SALT_WALLS, 14.0, 10.0),
        }
    }

    /// Writes the cells of the lair that fall in the chunk at `chunk_pos`.
    pub fn place_blocks(&self, chunk: &mut Chunk, chunk_pos: IVec3) {
        let origin = chunk_pos * CHUNK_SIZE;
        let last = origin + CHUNK_SIZE - 1;
        let (low, high) = (self.centre - REACH, self.centre + REACH);
        let apart = origin.x > high.x || origin.z > high.z || last.x < low.x || last.z < low.z;
        if apart || origin.y > TOP || last.y < BED {
            return;
        }
        for x in 0..CHUNK_SIZE {
            for z in 0..CHUNK_SIZE {
                for y in (BED - origin.y).max(0)..=(TOP - origin.y).min(CHUNK_SIZE - 1) {
                    let local = IVec3::new(x, y, z);
                    self.place_cell(chunk, local, origin + local);
                }
            }
        }
    }

    fn place_cell(&self, chunk: &mut Chunk, local: IVec3, pos: IVec3) {
        let from = pos - self.centre;
        let across = (from.x as f32).hypot(from.z as f32);
        if across > REACH as f32 {
            return;
        }
        let rock = |id| BlockData::new(id, BlockDirection::Front);
        if pos.y <= FLOOR {
            chunk.set(local, rock(self.ground(pos, across)));
            return;
        }
        let wide = f64::from(across) / f64::from(RADIUS);
        let high = f64::from(from.y) / f64::from(HEIGHT);
        let field = wide * wide + high * high;
        if field < 1.0 + WAVER * self.walls.get(pos) {
            match self.brazier(from) {
                Some(id) => chunk.set(local, rock(id)),
                None => {
                    chunk.remove(local);
                }
            }
        } else {
            // no lava nor water of a cave flows in
            let liquid = chunk
                .get(local)
                .is_some_and(|block| block.id.get_visibility() == BlockTransparency::Liquid);
            if liquid {
                chunk.set(local, rock(BlockId::Stone));
            }
        }
    }

    /// The block of the ground at `pos`, `across` blocks from the centre.
    fn ground(&self, pos: IVec3, across: f32) -> BlockId {
        let from = pos - self.centre;
        // a causeway leads to the island from two sides
        let dry = across <= ISLAND as f32 + 0.5 || from.z.abs() <= 1;
        let shore = LAKE as f32 + 1.5 * self.walls.get(pos.with_y(FLOOR)) as f32;
        if pos.y > BED && !dry && across <= shore {
            return BlockId::Lava;
        }
        let random = PositionRandom {
            seed: self.seed,
            coords: [pos.x, pos.y, pos.z],
        };
        let roll = random.below(SALT_GROUND, 10);
        if across <= shore + 2.5 {
            match roll {
                0..=5 => BlockId::Obsidian,
                _ => BlockId::Slate,
            }
        } else {
            match roll {
                0..=4 => BlockId::Slate,
                5..=7 => BlockId::Stone,
                8 => BlockId::Cobblestone,
                _ => BlockId::Gravel,
            }
        }
    }

    /// The block of a brazier in the cell `from` the centre, above the ground.
    fn brazier(&self, from: IVec3) -> Option<BlockId> {
        if from.y > 3 {
            return None;
        }
        let turn = std::f32::consts::TAU / BRAZIERS as f32;
        let is_one = (0..BRAZIERS).any(|i| {
            let angle = (i as f32 + 0.5) * turn;
            let at = Vec2::new(angle.cos(), angle.sin()) * BRAZIER_RING;
            at.round().as_ivec2() == from.xz()
        });
        is_one.then_some(if from.y == 3 {
            BlockId::Campfire
        } else {
            BlockId::HewnStone
        })
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::WorldGenerator;
    use super::*;
    use shared::world::WorldGenSettings;
    use std::collections::HashMap;

    #[test]
    fn the_lair_is_built_whole_whatever_the_order_of_its_chunks() {
        for seed in [1, 42, 77] {
            let generator = WorldGenerator::new(seed);
            let centre = generator.lair().unwrap().centre;
            let away = centre.xz().as_vec2().length();
            assert!((250.0..1100.0).contains(&away), "{away}");
            let side = IVec3::splat(CHUNK_SIZE);
            let reach = IVec3::new(REACH + 1, 0, REACH + 1);
            let (low, high) = (
                (centre - reach).with_y(0).div_euclid(side),
                (centre + reach).with_y(TOP + 1).div_euclid(side),
            );
            let mut positions = Vec::new();
            for x in low.x..=high.x {
                for y in low.y..=high.y {
                    for z in low.z..=high.z {
                        positions.push(IVec3::new(x, y, z));
                    }
                }
            }
            let backwards = WorldGenerator::new(seed);
            let made: HashMap<IVec3, Chunk> = positions
                .iter()
                .rev()
                .map(|&pos| (pos, backwards.generate_chunk(pos)))
                .collect();
            for (pos, chunk) in &made {
                assert!(generator.generate_chunk(*pos) == *chunk, "{seed} {pos}");
            }
            let at = |pos: IVec3| {
                made[&pos.div_euclid(side)]
                    .get(pos.rem_euclid(side))
                    .map(|block| block.id)
            };
            // the island, the lake, the shore, and room over them
            let lake = centre + IVec3::new(0, 0, 8);
            assert_eq!(at(lake), Some(BlockId::Lava));
            for (from, over) in [(0, 12), (8, 12), (20, 6)] {
                let ground = centre + IVec3::new(0, 0, from);
                assert!(at(ground).is_some());
                for y in 1..=over {
                    assert_eq!(at(ground + IVec3::Y * y), None, "{seed} {from} {y}");
                }
            }
            // rock all around, and no liquid to flow in
            for x in -REACH..=REACH {
                for z in -REACH..=REACH {
                    for y in FLOOR + 1..=TOP {
                        let id = at(IVec3::new(centre.x + x, y, centre.z + z));
                        let inside = (x * x + z * z) as f32 <= (REACH * REACH) as f32;
                        let flows = matches!(id, Some(BlockId::Lava | BlockId::Water));
                        assert!(!(inside && flows), "{seed} {x} {y} {z}");
                    }
                }
            }
        }
        // none in a world saved before the dungeons
        let settings = WorldGenSettings::legacy();
        let before = WorldGenerator::with(7, &settings, shared::world::Biomes::vanilla());
        assert!(before.lair().is_none());
    }
}
