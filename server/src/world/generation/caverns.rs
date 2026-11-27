//! The three biomes of the deep caves, between height 0 and the lakes of lava of a deep world:
//! damp caves grown with moss and hanging vines, with pools and clay; crystal caverns lined with
//! a pale shell and glowing crystal; and galleries of basalt, floored with magma and lava pools.
//! Two slow 3D noises pick the biome of a cell, so a biome is a pure function of the seed and
//! the cell, and so is what it changes: the rock around the caves (the cells next to one the
//! caves carve, `Caves::carve`) and the air of the caves, whatever chunk asks. Each biome has
//! its creature: the Mossback grazes the damp caves and the Shardling guards the crystal caverns,
//! born with their chunk; the Slagcrawler appears in the dark of the galleries
//! (`hostiles::spawn_kind`).

use std::collections::HashMap;
use std::f32::consts::TAU;

use bevy::prelude::*;
use shared::world::depths::Underground;
use shared::world::{BlockData, BlockDirection, BlockId, Chunk, MobKind, ServerMob};
use shared::CHUNK_SIZE;

use super::super::caves::{CaveMask, Caves, Noise3};
use super::PositionRandom;

/// The biomes of the deep caves.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CaveBiome {
    /// Damp caves, grown with moss and vines.
    Lush,
    /// Crystal caverns, which glow.
    Crystal,
    /// Galleries of basalt near the lava.
    Basalt,
}

impl CaveBiome {
    pub const ALL: [CaveBiome; 3] = [CaveBiome::Lush, CaveBiome::Crystal, CaveBiome::Basalt];

    /// The name `/locate` takes and answers with.
    pub fn name(self) -> &'static str {
        match self {
            CaveBiome::Lush => "lush_caves",
            CaveBiome::Crystal => "crystal_caves",
            CaveBiome::Basalt => "basalt_galleries",
        }
    }

    pub fn from_name(name: &str) -> Option<CaveBiome> {
        Self::ALL.into_iter().find(|biome| biome.name() == name)
    }
}

/// The highest and the lowest cells the biomes change: under height 0, above the lakes.
pub const BIOMES_TOP: i32 = -1;
pub const BIOMES_BOTTOM: i32 = -56;
/// The galleries of basalt lie below this height only, nearer the lava.
const BASALT_BELOW: i32 = -18;

const WET_WAVELENGTH: f64 = 160.0;
const HEAT_WAVELENGTH: f64 = 192.0;
const WAVELENGTH_Y: f64 = 72.0;
/// The noises pick a biome beyond these values.
const LUSH_FROM: f64 = 0.28;
const CRYSTAL_BELOW: f64 = -0.32;
const BASALT_FROM: f64 = 0.3;

const SALT_WET: u64 = 0x4100;
const SALT_HEAT: u64 = 0x4101;
const SALT_CELL: u64 = 0x4110;
const SALT_VINE: u64 = 0x4111;
const SALT_CREATURE: u64 = 0x4120;

/// How long a vine hangs at most, in blocks.
const MAX_VINE: i32 = 4;

/// The noises of the biomes of the deep caves of a world.
pub struct Caverns {
    wet: Noise3,
    heat: Noise3,
}

/// What the caves carve around a chunk: its mask, and those of its neighbours as they are asked.
struct Carved<'a> {
    caves: &'a Caves,
    underground: Underground,
    masks: HashMap<IVec3, CaveMask>,
}

impl Carved<'_> {
    /// Whether the cell `pos` is the air of a cave: carved, and above the lava.
    fn air(&mut self, pos: IVec3) -> bool {
        if pos.y <= self.underground.lava_level() {
            return false;
        }
        let chunk = pos.div_euclid(IVec3::splat(CHUNK_SIZE));
        let caves = self.caves;
        let mask = self
            .masks
            .entry(chunk)
            .or_insert_with(|| caves.carve(chunk * CHUNK_SIZE));
        mask.contains(pos - chunk * CHUNK_SIZE)
    }
}

fn cell_random(seed: u32, pos: IVec3) -> PositionRandom<3> {
    PositionRandom {
        seed,
        coords: pos.to_array(),
    }
}

impl Caverns {
    pub fn new(seed: u32) -> Self {
        Self {
            wet: Noise3::new(seed, SALT_WET, WET_WAVELENGTH, WAVELENGTH_Y),
            heat: Noise3::new(seed, SALT_HEAT, HEAT_WAVELENGTH, WAVELENGTH_Y),
        }
    }

    /// The biome of the cell `pos` of a deep world, if it lies in one.
    pub fn biome_at(&self, pos: IVec3) -> Option<CaveBiome> {
        if !(BIOMES_BOTTOM..=BIOMES_TOP).contains(&pos.y) {
            return None;
        }
        if pos.y < BASALT_BELOW && self.heat.get(pos) > BASALT_FROM {
            return Some(CaveBiome::Basalt);
        }
        let wet = self.wet.get(pos);
        if wet > LUSH_FROM {
            Some(CaveBiome::Lush)
        } else if wet < CRYSTAL_BELOW {
            Some(CaveBiome::Crystal)
        } else {
            None
        }
    }

    /// Whether the chunk at `chunk_pos` may hold cells of the biomes.
    fn reaches(chunk_pos: IVec3, underground: Underground) -> bool {
        let bottom = chunk_pos.y * CHUNK_SIZE;
        underground == Underground::Deep
            && bottom <= BIOMES_TOP
            && bottom + CHUNK_SIZE > BIOMES_BOTTOM
    }

    /// Changes the rock around the caves of the chunk at `chunk_pos`, carved as `mask`, and the
    /// air in them, by the biome of each cell.
    pub fn decorate(
        &self,
        seed: u32,
        caves: &Caves,
        underground: Underground,
        chunk_pos: IVec3,
        mask: CaveMask,
        chunk: &mut Chunk,
    ) {
        if !Self::reaches(chunk_pos, underground) {
            return;
        }
        let origin = chunk_pos * CHUNK_SIZE;
        let mut carved = Carved {
            caves,
            underground,
            masks: HashMap::from([(chunk_pos, mask)]),
        };
        let block = |id| BlockData::new(id, BlockDirection::Front);
        for y in 0..CHUNK_SIZE {
            let world_y = origin.y + y;
            if !(BIOMES_BOTTOM..=BIOMES_TOP).contains(&world_y) {
                continue;
            }
            for z in 0..CHUNK_SIZE {
                for x in 0..CHUNK_SIZE {
                    let local = IVec3::new(x, y, z);
                    let pos = origin + local;
                    let Some(biome) = self.biome_at(pos) else {
                        continue;
                    };
                    let random = cell_random(seed, pos);
                    let roll = random.unit(SALT_CELL);
                    if carved.air(pos) {
                        if let Some(id) = air_block(&mut carved, seed, biome, pos, roll) {
                            chunk.set(local, block(id));
                        }
                        continue;
                    }
                    let is_rock = chunk
                        .get(local)
                        .is_some_and(|b| matches!(b.id, BlockId::Gloomrock | BlockId::Stone));
                    if !is_rock {
                        continue;
                    }
                    let above = carved.air(pos + IVec3::Y);
                    let below = carved.air(pos - IVec3::Y);
                    let beside = [IVec3::X, IVec3::NEG_X, IVec3::Z, IVec3::NEG_Z]
                        .into_iter()
                        .filter(|step| carved.air(pos + *step))
                        .count();
                    if !above && !below && beside == 0 {
                        continue;
                    }
                    // A pit in the floor, walled all round, holds a pool
                    let pit = above
                        && beside == 0
                        && [IVec3::X, IVec3::NEG_X, IVec3::Z, IVec3::NEG_Z]
                            .into_iter()
                            .all(|step| !carved.air(pos + step - IVec3::Y));
                    let id = match biome {
                        CaveBiome::Lush if pit && roll < 0.12 => BlockId::Water,
                        CaveBiome::Lush if above || below => BlockId::Moss,
                        CaveBiome::Lush if roll < 0.2 => BlockId::Clay,
                        CaveBiome::Lush if roll < 0.6 => BlockId::Moss,
                        CaveBiome::Lush => continue,
                        CaveBiome::Crystal if roll < 0.07 => BlockId::Glowcrystal,
                        CaveBiome::Crystal => BlockId::Geodestone,
                        CaveBiome::Basalt if pit && roll < 0.08 => BlockId::Lava,
                        CaveBiome::Basalt if above && roll < 0.18 => BlockId::Magma,
                        CaveBiome::Basalt => BlockId::Basalt,
                    };
                    chunk.set(local, block(id));
                }
            }
        }
    }

    /// The creatures born with the chunk at `chunk_pos`: a Mossback or two on the floor of the
    /// damp caves, a Shardling in the crystal caverns, now and then.
    pub fn chunk_creatures(
        &self,
        seed: u32,
        caves: &Caves,
        underground: Underground,
        chunk_pos: IVec3,
    ) -> Vec<ServerMob> {
        if !Self::reaches(chunk_pos, underground) {
            return Vec::new();
        }
        let origin = chunk_pos * CHUNK_SIZE;
        let random = PositionRandom {
            seed,
            coords: chunk_pos.to_array(),
        };
        let mut carved = Carved {
            caves,
            underground,
            masks: HashMap::new(),
        };
        let mut creatures = Vec::new();
        for attempt in 0..3u64 {
            let salt = SALT_CREATURE + 8 * attempt;
            let (x, z) = (
                random.below(salt, CHUNK_SIZE as u64),
                random.below(salt + 1, CHUNK_SIZE as u64),
            );
            // The highest floor of the column in the chunk with room for two cells
            let floor = (1..CHUNK_SIZE - 2)
                .rev()
                .map(|y| origin + IVec3::new(x, y, z))
                .find(|&feet| {
                    carved.air(feet) && carved.air(feet + IVec3::Y) && !carved.air(feet - IVec3::Y)
                });
            let Some(feet) = floor else {
                continue;
            };
            let kind = match self.biome_at(feet) {
                Some(CaveBiome::Lush) if random.unit(salt + 2) < 0.45 => MobKind::Mossback,
                Some(CaveBiome::Crystal) if random.unit(salt + 2) < 0.35 => MobKind::Shardling,
                _ => continue,
            };
            let position = ServerMob::standing_at(
                kind,
                feet.x as f32 + 0.5,
                feet.y as f32,
                feet.z as f32 + 0.5,
            );
            creatures.push(
                ServerMob {
                    rotation: Quat::from_rotation_y(random.unit(salt + 3) * TAU),
                    action_ticks: 1 + random.below(salt + 4, 200) as u32,
                    ..ServerMob::new(kind, position)
                }
                .in_tier(underground.tier_at(feet.y)),
            );
        }
        creatures
    }

    /// The cell of the biome `biome` nearest to `from`, looked for every 16 blocks within
    /// `radius` along each axis, at a few heights.
    pub fn nearest(&self, biome: CaveBiome, from: IVec2, radius: i32) -> Option<IVec3> {
        const STEP: i32 = 16;
        let heights = [-12, -30, -44];
        for ring in 0..=radius / STEP {
            let mut best: Option<(i64, IVec3)> = None;
            for dx in -ring..=ring {
                for dz in -ring..=ring {
                    if dx.abs() != ring && dz.abs() != ring {
                        continue;
                    }
                    let column = from + IVec2::new(dx, dz) * STEP;
                    for y in heights {
                        let pos = IVec3::new(column.x, y, column.y);
                        if self.biome_at(pos) == Some(biome) {
                            let distance = (column - from).as_i64vec2().length_squared();
                            if best.is_none_or(|(d, _)| distance < d) {
                                best = Some((distance, pos));
                            }
                        }
                    }
                }
            }
            if let Some((_, pos)) = best {
                return Some(pos);
            }
        }
        None
    }
}

/// What the air of a cave of `biome` at `pos` holds, if anything: vines hang from the roof of
/// the damp caves, crystals stand on the floor of the crystal caverns.
fn air_block(
    carved: &mut Carved,
    seed: u32,
    biome: CaveBiome,
    pos: IVec3,
    roll: f32,
) -> Option<BlockId> {
    match biome {
        CaveBiome::Lush => {
            // The roof above within `MAX_VINE`, and how long its vine is
            let roof = (1..=MAX_VINE).find(|&d| !carved.air(pos + IVec3::Y * d))?;
            let random = cell_random(seed, pos + IVec3::Y * roof);
            let length = if random.unit(SALT_VINE) < 0.3 {
                1 + random.below(SALT_VINE + 1, MAX_VINE as u64)
            } else {
                0
            };
            // A vine stops a cell over the floor
            (roof <= length && carved.air(pos - IVec3::Y)).then_some(BlockId::CaveVine)
        }
        CaveBiome::Crystal => {
            (!carved.air(pos - IVec3::Y) && roll < 0.08).then_some(BlockId::CrystalCluster)
        }
        CaveBiome::Basalt => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_biomes_of_the_caves_depend_only_on_the_seed_and_lie_under_height_zero() {
        let caverns = Caverns::new(11);
        let again = Caverns::new(11);
        let other = Caverns::new(12);
        let mut found = HashMap::new();
        let mut differs = false;
        for x in (-2000..2000).step_by(37) {
            for z in (-2000..2000).step_by(41) {
                for y in [10, -1, -20, -40, -60] {
                    let pos = IVec3::new(x, y, z);
                    let biome = caverns.biome_at(pos);
                    assert_eq!(biome, again.biome_at(pos));
                    differs |= biome != other.biome_at(pos);
                    if !(BIOMES_BOTTOM..=BIOMES_TOP).contains(&y) {
                        assert_eq!(biome, None);
                    }
                    if biome == Some(CaveBiome::Basalt) {
                        assert!(y < BASALT_BELOW);
                    }
                    if let Some(biome) = biome {
                        *found.entry(biome).or_insert(0) += 1;
                    }
                }
            }
        }
        assert!(differs);
        for biome in CaveBiome::ALL {
            assert!(
                found.get(&biome).copied().unwrap_or(0) > 20,
                "{biome:?}: {found:?}"
            );
            assert_eq!(CaveBiome::from_name(biome.name()), Some(biome));
        }
    }

    #[test]
    fn a_chunk_of_the_deep_caves_is_the_same_whichever_chunk_came_first() {
        use super::super::WorldGenerator;
        let first = WorldGenerator::new(5);
        let second = WorldGenerator::new(5);
        for x in (-3..3).rev() {
            second.generate_chunk(IVec3::new(x, -2, 1));
        }
        let mut changed = 0;
        for x in -3..3 {
            let pos = IVec3::new(x, -2, 1);
            let (a, b) = (first.generate_chunk(pos), second.generate_chunk(pos));
            for i in 0..CHUNK_SIZE.pow(3) {
                let local = IVec3::new(i % 16, i / 16 % 16, i / 256);
                assert_eq!(a.get(local), b.get(local), "{pos} {local}");
                changed += usize::from(a.get(local).is_some_and(|block| {
                    matches!(
                        block.id,
                        BlockId::Moss | BlockId::Geodestone | BlockId::Basalt | BlockId::CaveVine
                    )
                }));
            }
            let creatures = |g: &WorldGenerator| -> Vec<Vec3> {
                g.chunk_animals(pos)
                    .iter()
                    .map(|mob| mob.position)
                    .collect()
            };
            assert_eq!(creatures(&first), creatures(&second));
        }
        let _ = changed;
    }
}
