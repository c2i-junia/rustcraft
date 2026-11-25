//! Where villages are: at most one per region, where the land suits it.

use bevy::prelude::*;
use shared::world::{TerrainNoises, VillageStyle, MOUNTAIN_FROM, SEA_LEVEL};

use super::super::PositionRandom;

/// Side of a region, in blocks: each may hold one village.
pub const REGION: i32 = 512;
/// The centre of a village lies at least this far inside its region.
const INSET: i32 = 72;
/// No block of a village, nor any column it keeps clear, lies farther than this from its
/// centre along x or z: the village stays inside its region (`INSET` is larger).
pub const REACH: i32 = 64;

/// The chance of a region to hold a village, before its land is looked at.
const CHANCE: f32 = 0.8;
/// How many centres are drawn in a region, until one suits a village.
const ATTEMPTS: u64 = 3;
const SALT_ROLL: u64 = 0x1000;
/// Each attempt takes two salts from here, one per axis.
const SALT_CENTRE: u64 = 0x1010;

/// The land around a centre is read on a grid of this step, this far from it.
const SAMPLE_STEP: i32 = 8;
const SAMPLE_RADIUS: i32 = 40;
/// How deep inside its biome the centre lies, at least (see `ColumnShape::depth_in_biome`).
const DEPTH: f32 = 0.2;
/// Most blocks between the lowest and the highest dry columns read.
const MOST_SPREAD: i32 = 14;
/// The share of the columns read that are dry, at least.
const DRY_SHARE: f32 = 0.85;

/// Where a village stands, before it is laid out.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Site {
    pub centre: IVec2,
    /// The height of the surface at the centre.
    pub height: i32,
    pub style: VillageStyle,
}

/// The region of the column `(x, z)`.
pub fn region_of(x: i32, z: i32) -> IVec2 {
    IVec2::new(x.div_euclid(REGION), z.div_euclid(REGION))
}

/// Whether a column of this height is dry land, above the beach.
pub fn dry(height: i32) -> bool {
    height > SEA_LEVEL + 1
}

/// The village of `region` in the world of `seed` shaped by `terrain`, if it holds one: the
/// first of the centres drawn in the region that suits it (`suits`).
pub fn site(seed: u32, terrain: &TerrainNoises, region: IVec2) -> Option<Site> {
    let random = PositionRandom {
        seed,
        coords: [region.x, region.y],
    };
    if random.unit(SALT_ROLL) >= CHANCE {
        return None;
    }
    let span = (REGION - 2 * INSET) as u64;
    (0..ATTEMPTS).find_map(|attempt| {
        let salt = SALT_CENTRE + 2 * attempt;
        let centre = region * REGION
            + IVec2::new(
                INSET + random.below(salt, span),
                INSET + random.below(salt + 1, span),
            );
        suits(terrain, centre)
    })
}

/// The village centred on `centre`, if the place suits one: it lies in a biome with a style of
/// village, deep inside it, on dry land below the mountains, and the land around is mostly dry,
/// rather flat, and grows no giant tree.
fn suits(terrain: &TerrainNoises, centre: IVec2) -> Option<Site> {
    let shape = terrain.column_shape(centre.x, centre.y);
    let biomes = terrain.biomes();
    let style = biomes.get(shape.biome).village?;
    if shape.depth_in_biome < DEPTH || !dry(shape.height) || shape.height >= MOUNTAIN_FROM - 8 {
        return None;
    }
    let (mut low, mut high) = (shape.height, shape.height);
    let (mut dry_count, mut total) = (0, 0);
    for dx in (-SAMPLE_RADIUS..=SAMPLE_RADIUS).step_by(SAMPLE_STEP as usize) {
        for dz in (-SAMPLE_RADIUS..=SAMPLE_RADIUS).step_by(SAMPLE_STEP as usize) {
            let sample = terrain.column_shape(centre.x + dx, centre.y + dz);
            if biomes.get(sample.biome).flora.giant_trees {
                return None;
            }
            total += 1;
            if dry(sample.height) {
                dry_count += 1;
                low = low.min(sample.height);
                high = high.max(sample.height);
            }
        }
    }
    if (dry_count as f32) < DRY_SHARE * total as f32 || high - low > MOST_SPREAD {
        return None;
    }
    Some(Site {
        centre,
        height: shape.height,
        style,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_village_lies_inside_its_region() {
        const { assert!(INSET > REACH) };
        let terrain = TerrainNoises::new(42);
        let mut found = 0;
        for x in -6..6 {
            for z in -6..6 {
                let region = IVec2::new(x, z);
                if let Some(site) = site(42, &terrain, region) {
                    found += 1;
                    assert_eq!(
                        region_of(site.centre.x - REACH, site.centre.y - REACH),
                        region
                    );
                    assert_eq!(
                        region_of(site.centre.x + REACH, site.centre.y + REACH),
                        region
                    );
                    assert_eq!(site, super::site(42, &terrain, region).unwrap());
                }
            }
        }
        // 144 regions, a third of them sea or mountains: villages, but not everywhere
        assert!((30..110).contains(&found), "{found}");
    }
}
