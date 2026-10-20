//! Animals: a herd may live on each column of chunks, decided from its position like the rest
//! of the generation, by the biome of the column it is centred on. The herd belongs to the
//! chunk that holds the surface of that column: that chunk is saved once generated, so it is
//! never generated again, and neither is the herd.

use std::f32::consts::TAU;

use bevy::prelude::*;
use shared::{world::*, CHUNK_SIZE};

use super::{column_decorations, random_of, ChunkColumns, Column, Decoration, PositionRandom};

/// The decisions of one column of chunks, by its position.
type ChunkColumnRandom = PositionRandom<2>;

const SALT_HERD_ROLL: u64 = 0x300;
const SALT_HERD_X: u64 = 0x301;
const SALT_HERD_Z: u64 = 0x302;
const SALT_HERD_SIZE: u64 = 0x303;
/// Each animal of a herd uses 4 consecutive salts from here.
const SALT_ANIMAL: u64 = 0x310;

/// How far from the centre of its herd an animal stands, along x and z.
const HERD_SPREAD: i32 = 3;

/// The herds that may live on a column of chunks of `biome`: the chance of each, and the
/// sizes it comes in, both included. The chances of a biome add up to less than 1.
fn herds(biome: BiomeType) -> &'static [(MobKind, f32, (u64, u64))] {
    match biome {
        BiomeType::Plains | BiomeType::FlowerPlains => &[(MobKind::Sheep, 0.12, (2, 4))],
        BiomeType::Forest => &[(MobKind::Fox, 0.10, (1, 2)), (MobKind::Sheep, 0.03, (2, 3))],
        BiomeType::MediumMountain => &[(MobKind::Sheep, 0.06, (2, 4))],
        BiomeType::HighMountainGrass => &[(MobKind::Sheep, 0.04, (2, 3))],
        BiomeType::IcePlain => &[(MobKind::Fox, 0.06, (1, 2))],
        _ => &[],
    }
}

/// Whether an animal may appear standing on `column`: dry land of grass, or snow on the ice
/// plains, where no tree, bush or cactus grows, nor the trunk of a giant tree.
fn may_stand_on(column: &Column, seed: u32) -> bool {
    let ground = match column.biome_type {
        BiomeType::IcePlain => matches!(column.surface, BlockId::Grass | BlockId::Snow),
        _ => column.surface == BlockId::Grass,
    };
    ground
        && column.height > SEA_LEVEL + 1
        && column.giant.is_none()
        && !column_decorations(column, &random_of(seed, column))
            .iter()
            .any(|d| {
                matches!(
                    d,
                    Decoration::Tree
                        | Decoration::BigTree
                        | Decoration::JungleTree
                        | Decoration::Bush
                        | Decoration::Cactus
                )
            })
}

/// The animals born with the chunk at `chunk_pos`, whose columns are `columns`.
pub(super) fn chunk_animals(seed: u32, chunk_pos: IVec3, columns: &ChunkColumns) -> Vec<ServerMob> {
    let random = ChunkColumnRandom {
        seed,
        coords: [chunk_pos.x, chunk_pos.z],
    };
    let centre = (
        random.below(SALT_HERD_X, CHUNK_SIZE as u64),
        random.below(SALT_HERD_Z, CHUNK_SIZE as u64),
    );
    let centre_column = columns.at(centre.0, centre.1);
    if centre_column.height.div_euclid(CHUNK_SIZE) != chunk_pos.y
        || !may_stand_on(centre_column, seed)
    {
        return Vec::new();
    }
    let roll = random.unit(SALT_HERD_ROLL);
    let mut chance = 0.0;
    let Some(&(kind, _, (fewest, most))) = herds(centre_column.biome_type).iter().find(|herd| {
        chance += herd.1;
        roll < chance
    }) else {
        return Vec::new();
    };

    let size = fewest + random.below(SALT_HERD_SIZE, most - fewest + 1) as u64;
    let origin = chunk_pos * CHUNK_SIZE;
    let mut taken = Vec::new();
    let mut animals = Vec::new();
    for i in 0..size {
        let salt = SALT_ANIMAL + 4 * i;
        let spread = (2 * HERD_SPREAD + 1) as u64;
        let dx = (centre.0 + random.below(salt, spread) - HERD_SPREAD).clamp(0, CHUNK_SIZE - 1);
        let dz = (centre.1 + random.below(salt + 1, spread) - HERD_SPREAD).clamp(0, CHUNK_SIZE - 1);
        let column = columns.at(dx, dz);
        if taken.contains(&(dx, dz)) || !may_stand_on(column, seed) {
            continue;
        }
        taken.push((dx, dz));
        let position = ServerMob::standing_at(
            kind,
            (origin.x + dx) as f32 + 0.5,
            column.height as f32 + 1.0,
            (origin.z + dz) as f32 + 0.5,
        );
        animals.push(ServerMob {
            rotation: Quat::from_rotation_y(random.unit(salt + 2) * TAU),
            // so that a herd does not set off all at once
            action_ticks: 1 + random.below(salt + 3, 200) as u32,
            ..ServerMob::new(kind, position)
        });
    }
    animals
}

#[cfg(test)]
mod tests {
    use super::super::{GiantTrunk, WorldGenerator};
    use super::*;
    use std::collections::HashMap;

    /// The animals of the columns of chunks around the origin, `radius` of them on each side,
    /// generated from `seed`.
    fn animals_around(seed: u32, radius: i32) -> Vec<ServerMob> {
        let generator = WorldGenerator::new(seed);
        let mut animals = Vec::new();
        for x in -radius..=radius {
            for z in -radius..=radius {
                for y in 0..12 {
                    animals.extend(generator.chunk_animals(IVec3::new(x, y, z)));
                }
            }
        }
        animals
    }

    #[test]
    fn the_animals_of_a_chunk_depend_only_on_the_seed_and_its_position() {
        let generator = WorldGenerator::new(5);
        let other = WorldGenerator::new(5);
        for x in -6..6 {
            for y in 0..12 {
                let chunk = IVec3::new(x, y, 3);
                let positions = |g: &WorldGenerator| -> Vec<Vec3> {
                    g.chunk_animals(chunk).iter().map(|m| m.position).collect()
                };
                assert_eq!(positions(&generator), positions(&other), "{chunk}");
            }
        }
    }

    #[test]
    fn herds_of_both_kinds_appear_on_land_standing_on_an_open_block() {
        let seed = 42;
        let generator = WorldGenerator::new(seed);
        let animals = animals_around(seed, 24);
        let mut by_kind: HashMap<MobKind, usize> = HashMap::new();
        for animal in &animals {
            *by_kind.entry(animal.kind).or_default() += 1;
            let feet = animal.position.y - animal.height / 2.0;
            let (x, z) = (
                animal.position.x.floor() as i32,
                animal.position.z.floor() as i32,
            );
            let column = generator.column(x, z);
            assert_eq!(feet, column.height as f32 + 1.0, "{animal:?}");
            assert!(column.height > SEA_LEVEL, "{animal:?}");
        }
        // 49 x 49 columns of chunks: a few herds of each kind, but no crowd
        for kind in MobKind::ALL {
            let count = by_kind.get(&kind).copied().unwrap_or(0);
            assert!((3..1000).contains(&count), "{kind:?}: {count}");
        }
    }

    #[test]
    fn no_herd_lives_in_the_jungle() {
        assert!(herds(BiomeType::Jungle).is_empty());
    }

    #[test]
    fn no_animal_appears_under_a_jungle_tree_or_on_a_bush() {
        let seed = 5;
        let shape = ColumnShape {
            height: 70,
            biome: BiomeType::Jungle,
            jungle_depth: 1.0,
        };
        let mut found = Vec::new();
        for x in 0..100 {
            for z in 0..100 {
                let column = Column::new(seed, x, z, shape, 0, 0.0, None);
                let decorations = column_decorations(&column, &random_of(seed, &column));
                for wanted in [Decoration::JungleTree, Decoration::Bush] {
                    if decorations.contains(&wanted) {
                        assert!(!may_stand_on(&column, seed), "({x}, {z}): {wanted:?}");
                        found.push(wanted);
                    }
                }
            }
        }
        assert!(found.contains(&Decoration::JungleTree) && found.contains(&Decoration::Bush));
    }

    #[test]
    fn no_animal_appears_on_a_giants_trunk() {
        let seed = 5;
        let shape = ColumnShape {
            height: 70,
            biome: BiomeType::Jungle,
            jungle_depth: 1.0,
        };
        for x in 0..20 {
            for z in 0..20 {
                let open = Column::new(seed, x, z, shape, 0, 0.0, None);
                for part in [GiantTrunk::Root, GiantTrunk::Beside] {
                    let trunk = Column::new(seed, x, z, shape, 0, 0.0, Some(part));
                    if may_stand_on(&open, seed) {
                        assert!(!may_stand_on(&trunk, seed), "({x}, {z}): {part:?}");
                    }
                }
            }
        }
    }

    #[test]
    fn a_herd_belongs_to_one_chunk_of_its_column() {
        let generator = WorldGenerator::new(9);
        for x in -8..8 {
            for z in -8..8 {
                let chunks_with_animals = (0..16)
                    .filter(|&y| !generator.chunk_animals(IVec3::new(x, y, z)).is_empty())
                    .count();
                assert!(
                    chunks_with_animals <= 1,
                    "({x}, {z}): {chunks_with_animals}"
                );
            }
        }
    }
}
