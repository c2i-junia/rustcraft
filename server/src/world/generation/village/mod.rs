//! Villages: a square with a well, streets, and buildings along them.
//!
//! The world is cut into regions of `REGION` blocks; each may hold one village, whose centre is
//! drawn from the seed and the region, deep enough inside it that the village never leaves it
//! (`placement`). A village is kept where its biome has a style of village and the land around
//! is dry and flat enough. Its plan, the blocks of its streets and buildings, is then laid out
//! from the seed and the shape of the terrain alone (`layout`), never from a chunk already made:
//! each chunk the village touches places the blocks of the plan that fall in it, the same way
//! whichever chunk is generated first.

use std::collections::HashSet;

use bevy::prelude::*;
use shared::world::{Chunk, Container, ServerMob, VillageStyle};

use super::structure::StructureBlocks;

mod layout;
mod loot;
mod pieces;
mod placement;

pub use layout::layout;
pub use pieces::PieceKind;
pub use placement::{dry, region_of, site, REACH, REGION};

/// A village as it is built: its blocks by column of chunks, the columns nothing else grows on,
/// and what lives and lies in it.
pub struct VillagePlan {
    pub centre: IVec2,
    pub style: VillageStyle,
    /// The corners of the columns the village changes or keeps clear, inclusive.
    pub min: IVec2,
    pub max: IVec2,
    /// The highest block of the village.
    pub top: i32,
    /// The blocks of the village, and what its chests and barrels hold.
    blocks: StructureBlocks,
    /// The columns where nothing grows: the streets, the buildings, and around them.
    footprint: HashSet<IVec2>,
    /// The animals of its pens: where each stands.
    animals: Vec<ServerMob>,
    /// Its buildings: their kind, their corners and the height of their floor.
    pub pieces: Vec<PlacedPiece>,
}

/// A building of a village.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlacedPiece {
    pub kind: PieceKind,
    /// The corners of its columns, inclusive.
    pub min: IVec2,
    pub max: IVec2,
    pub floor: i32,
}

impl VillagePlan {
    /// Whether nothing grows on the column `(x, z)`.
    pub fn covers(&self, x: i32, z: i32) -> bool {
        self.footprint.contains(&IVec2::new(x, z))
    }

    /// Whether the village changes or keeps clear a column within `margin` of the columns from
    /// `min` to `max`.
    pub fn touches(&self, min: IVec2, max: IVec2, margin: i32) -> bool {
        self.min.x <= max.x + margin
            && self.max.x >= min.x - margin
            && self.min.y <= max.y + margin
            && self.max.y >= min.y - margin
    }

    /// Places the blocks of the village that fall in the chunk at `chunk_pos`: they replace
    /// whatever was generated there.
    pub fn place_blocks(&self, chunk: &mut Chunk, chunk_pos: IVec3) {
        self.blocks.place(chunk, chunk_pos);
    }

    /// The animals of the village born with the chunk at `chunk_pos`: those standing in it.
    pub fn chunk_animals(&self, chunk_pos: IVec3) -> impl Iterator<Item = ServerMob> + '_ {
        self.animals
            .iter()
            .filter(move |animal| {
                let feet = animal.position - Vec3::Y * (animal.height / 2.0);
                shared::world::world_position_to_chunk_position(feet + Vec3::Y * 0.01) == chunk_pos
            })
            .cloned()
    }

    /// What the containers of the village in the chunk at `chunk_pos` hold, by their position
    /// in the chunk.
    pub fn chunk_containers(
        &self,
        chunk_pos: IVec3,
    ) -> impl Iterator<Item = (IVec3, Container)> + '_ {
        self.blocks.chunk_containers(chunk_pos)
    }

    /// The kinds of the animals of the village.
    #[cfg(test)]
    pub fn animal_kinds(&self) -> Vec<shared::world::MobKind> {
        self.animals.iter().map(|animal| animal.kind).collect()
    }

    /// Every block of the village, in the order placed.
    #[cfg(test)]
    pub fn all_blocks(
        &self,
    ) -> impl Iterator<Item = (IVec3, Option<shared::world::BlockData>)> + '_ {
        self.blocks.all()
    }
}

#[cfg(test)]
mod tests {
    use super::super::WorldGenerator;
    use super::placement::REACH;
    use super::*;
    use shared::world::BlockData;
    use shared::world::{TerrainNoises, WorldGenSettings, SEA_LEVEL};
    use shared::CHUNK_SIZE;
    use std::collections::HashMap;
    use std::sync::Arc;

    /// The villages of the seed `seed` in the regions within `radius` of the origin.
    fn villages(seed: u32, radius: i32) -> Vec<(IVec2, Arc<VillagePlan>)> {
        let generator = WorldGenerator::new(seed);
        let mut found = Vec::new();
        for x in -radius..=radius {
            for z in -radius..=radius {
                let region = IVec2::new(x, z);
                if let Some(plan) = generator.village(region) {
                    found.push((region, plan));
                }
            }
        }
        found
    }

    #[test]
    fn villages_are_laid_out_with_buildings_that_never_overlap() {
        let mut kinds = HashSet::new();
        let mut total = 0;
        for seed in [1, 42] {
            let found = villages(seed, 3);
            assert!(found.len() >= 3, "seed {seed}: {} villages", found.len());
            for (region, plan) in &found {
                let centre = plan.centre;
                assert_eq!(region_of(centre.x, centre.y), *region);
                // inside its reach, so inside its region
                assert!((plan.min - centre).min_element() >= -REACH, "{centre}");
                assert!((plan.max - centre).max_element() <= REACH, "{centre}");
                assert_eq!(plan.pieces[0].kind, PieceKind::Well);
                for (i, a) in plan.pieces.iter().enumerate() {
                    kinds.insert(a.kind);
                    for b in &plan.pieces[i + 1..] {
                        let apart = a.max.x < b.min.x
                            || b.max.x < a.min.x
                            || a.max.y < b.min.y
                            || b.max.y < a.min.y;
                        assert!(apart, "{centre}: {a:?} and {b:?}");
                    }
                    assert!(a.floor > SEA_LEVEL, "{centre}: {a:?}");
                }
                total += plan.pieces.len();
                for (pos, _) in plan.all_blocks() {
                    assert!(plan.covers(pos.x, pos.z), "{centre}: {pos}");
                }
            }
        }
        assert!(total > 30, "{total} pieces");
        for kind in [
            PieceKind::SmallHouse,
            PieceKind::BigHouse,
            PieceKind::Field,
            PieceKind::Pen,
            PieceKind::Lamp,
        ] {
            assert!(kinds.contains(&kind), "no {kind:?}");
        }
    }

    #[test]
    fn a_village_is_built_into_its_chunks_whatever_their_order() {
        let seed = 42;
        let (_, plan) = villages(seed, 2).into_iter().next().unwrap();
        let piece = plan.pieces[1];
        let chunk_column = piece.min.div_euclid(IVec2::splat(CHUNK_SIZE));
        let chunk_y = piece.floor.div_euclid(CHUNK_SIZE);
        let alone = WorldGenerator::new(seed);
        let amid = WorldGenerator::new(seed);
        for dx in -1..=1 {
            for dz in -1..=1 {
                for dy in -1..=1 {
                    amid.generate_chunk(IVec3::new(
                        chunk_column.x + dx,
                        chunk_y + dy,
                        chunk_column.y + dz,
                    ));
                }
            }
        }
        for dy in -1..=1 {
            let chunk_pos = IVec3::new(chunk_column.x, chunk_y + dy, chunk_column.y);
            assert!(
                alone.generate_chunk(chunk_pos) == amid.generate_chunk(chunk_pos),
                "{chunk_pos}"
            );
            assert!(
                alone.generate_chunk(chunk_pos) == alone.generate_chunk_from_noise(chunk_pos),
                "{chunk_pos}"
            );
        }
    }

    #[test]
    fn the_blocks_of_a_village_are_in_its_chunks_with_its_chests_and_animals() {
        let seed = 42;
        let generator = WorldGenerator::new(seed);
        let mut chests = 0;
        let mut animals = 0;
        let mut checked = 0;
        for (_, plan) in villages(seed, 2) {
            let (min, max) = (
                plan.min.div_euclid(IVec2::splat(CHUNK_SIZE)),
                plan.max.div_euclid(IVec2::splat(CHUNK_SIZE)),
            );
            let mut placed: HashMap<IVec3, Option<BlockData>> = HashMap::new();
            for (pos, block) in plan.all_blocks() {
                placed.insert(pos, block);
            }
            for cx in min.x..=max.x {
                for cz in min.y..=max.y {
                    for cy in 3..=6 {
                        let chunk_pos = IVec3::new(cx, cy, cz);
                        let chunk = generator.generate_chunk(chunk_pos);
                        let origin = chunk_pos * CHUNK_SIZE;
                        for (pos, block) in &placed {
                            let local = *pos - origin;
                            if Chunk::contains(local) {
                                assert_eq!(chunk.get(local), block.as_ref(), "{pos}");
                                checked += 1;
                            }
                        }
                        for (local, container) in generator.chunk_containers(chunk_pos) {
                            assert!(chunk.get(local).unwrap().id.is_container());
                            chests += usize::from(!container.is_empty());
                        }
                        for animal in generator.chunk_animals(chunk_pos) {
                            let feet = (animal.position - Vec3::Y * animal.height / 2.0)
                                .floor()
                                .as_ivec3();
                            let below = feet - IVec3::Y - origin;
                            if plan.covers(feet.x, feet.z) && Chunk::contains(below) {
                                animals += 1;
                                assert!(chunk.get(below).is_some(), "{feet}");
                                assert_eq!(chunk.get(feet - origin), None, "{feet}");
                            }
                        }
                    }
                }
            }
        }
        // the blocks of the villages that fall in the chunks around the surface
        assert!(checked > 5000, "{checked} blocks");
        assert!(chests > 0, "no chest holds anything");
        assert!(animals > 0, "no animal in a pen");
    }

    #[test]
    fn nothing_grows_on_a_street_and_no_one_spawns_in_a_village() {
        let seed = 42;
        let generator = WorldGenerator::new(seed);
        let (_, plan) = villages(seed, 2).into_iter().next().unwrap();
        let column = generator.column(plan.centre.x, plan.centre.y);
        assert!(column.village);
        assert!(
            super::super::column_decorations(&column, &super::super::random_of(seed, &column))
                .is_empty()
        );
        assert_eq!(generator.spawn_height(plan.centre.x, plan.centre.y), None);
    }

    #[test]
    fn a_world_without_villages_has_none() {
        let settings = WorldGenSettings::legacy();
        let generator = WorldGenerator::with(42, &settings, shared::world::Biomes::vanilla());
        for x in -2..=2 {
            for z in -2..=2 {
                assert!(generator.village(IVec2::new(x, z)).is_none());
            }
        }
        // the land is the same
        let terrain = TerrainNoises::new(42);
        assert_eq!(
            terrain.column_shape(5, 5),
            generator.terrain().column_shape(5, 5)
        );
    }

    #[test]
    fn each_style_builds_of_its_own_wood() {
        let mut seen = HashSet::new();
        for seed in [1, 2, 3, 42] {
            for (_, plan) in villages(seed, 3) {
                let wall = plan
                    .all_blocks()
                    .filter_map(|(_, block)| block)
                    .any(|block| block.id == pieces::material(plan.style, pieces::Role::Wall));
                assert!(wall || plan.pieces.len() <= 2, "{:?}", plan.style);
                seen.insert(plan.style);
                let animals = plan.animal_kinds();
                assert!(animals
                    .iter()
                    .all(|kind| *kind != shared::world::MobKind::Wolf));
            }
        }
        assert!(seen.len() >= 2, "{seen:?}");
    }
}
