//! The structures of the jungle, of jungle wood: its trees and its bushes. Like the rest of the
//! generation, each is decided from its column alone.

use bevy::prelude::*;
use shared::world::BlockId;

use super::{Blocks, ColumnRandom, SALT_TRUNK_HEIGHT};

/// Each of the 4 corners of the 2 wide layers of a jungle tree's crown, from here.
const SALT_CROWN_CORNER: u64 = 0x400;
/// Each of the 8 outermost leaves of a bush, from here.
const SALT_BUSH_EDGE: u64 = 0x410;
const SALT_BUSH_TOP: u64 = 0x418;

/// Whether a leaf that grows one time in two grows.
fn grows(random: &ColumnRandom, salt: u64) -> bool {
    random.below(salt, 2) == 0
}

/// A trunk of 5 to 9 jungle logs from `root`. The two layers below its top log hold a 5 × 5
/// square of leaves whose corners grow one time in two, the layer of the top log a 3 × 3
/// square, and a plus of leaves lies above it.
pub(super) fn jungle_tree_blocks(root: IVec3, random: &ColumnRandom, blocks: &mut Blocks) {
    let trunk_height = 5 + random.below(SALT_TRUNK_HEIGHT, 5);
    for dy in 0..trunk_height {
        blocks.push((root + IVec3::Y * dy, BlockId::JungleLog));
    }
    let top = root + IVec3::Y * (trunk_height - 1);
    for layer in 0..2u64 {
        let mut corner = 0;
        for dx in -2i32..=2 {
            for dz in -2i32..=2 {
                if dx == 0 && dz == 0 {
                    continue;
                }
                if dx.abs() == 2 && dz.abs() == 2 {
                    corner += 1;
                    if !grows(random, SALT_CROWN_CORNER + 4 * layer + corner - 1) {
                        continue;
                    }
                }
                let pos = top + IVec3::new(dx, layer as i32 - 2, dz);
                blocks.push((pos, BlockId::JungleLeaves));
            }
        }
    }
    for dx in -1i32..=1 {
        for dz in -1i32..=1 {
            if dx != 0 || dz != 0 {
                blocks.push((top + IVec3::new(dx, 0, dz), BlockId::JungleLeaves));
            }
            if dx.abs() + dz.abs() <= 1 {
                blocks.push((top + IVec3::new(dx, 1, dz), BlockId::JungleLeaves));
            }
        }
    }
}

/// A jungle log on `root`, in the middle of a diamond of leaves of radius 2 whose 8 outermost
/// leaves grow one time in two, under a plus of leaves and, one time in two, one more leaf.
pub(super) fn bush_blocks(root: IVec3, random: &ColumnRandom, blocks: &mut Blocks) {
    blocks.push((root, BlockId::JungleLog));
    let mut edge = 0;
    for dx in -2i32..=2 {
        for dz in -2i32..=2 {
            let distance = dx.abs() + dz.abs();
            if distance == 2 {
                edge += 1;
                if !grows(random, SALT_BUSH_EDGE + edge - 1) {
                    continue;
                }
            }
            if distance > 0 && distance <= 2 {
                blocks.push((root + IVec3::new(dx, 0, dz), BlockId::JungleLeaves));
            }
            if distance <= 1 {
                blocks.push((root + IVec3::new(dx, 1, dz), BlockId::JungleLeaves));
            }
        }
    }
    if grows(random, SALT_BUSH_TOP) {
        blocks.push((root + IVec3::Y * 2, BlockId::JungleLeaves));
    }
}

#[cfg(test)]
mod tests {
    use super::super::PositionRandom;
    use super::*;
    use std::collections::{HashMap, HashSet};

    const ROOT: IVec3 = IVec3::new(0, 70, 0);

    /// The structure `shape` makes on many columns, its blocks by offset from its root.
    fn structures(shape: fn(IVec3, &ColumnRandom, &mut Blocks)) -> Vec<HashMap<IVec3, BlockId>> {
        (0..400)
            .map(|i| {
                let random = PositionRandom {
                    seed: 11,
                    coords: [i % 20, i / 20],
                };
                let mut blocks = Vec::new();
                shape(ROOT, &random, &mut blocks);
                let by_offset: HashMap<IVec3, BlockId> = blocks
                    .iter()
                    .map(|(pos, block)| (*pos - ROOT, *block))
                    .collect();
                assert_eq!(by_offset.len(), blocks.len(), "a cell is claimed twice");
                by_offset
            })
            .collect()
    }

    /// The offsets of the leaves of `structure` on the layer `dy` above the root.
    fn leaves(structure: &HashMap<IVec3, BlockId>, dy: i32) -> HashSet<IVec2> {
        structure
            .iter()
            .filter(|(pos, block)| pos.y == dy && **block == BlockId::JungleLeaves)
            .map(|(pos, _)| pos.xz())
            .collect()
    }

    fn plus() -> HashSet<IVec2> {
        [(0, 0), (1, 0), (-1, 0), (0, 1), (0, -1)]
            .map(|(x, z)| IVec2::new(x, z))
            .into()
    }

    #[test]
    fn the_jungle_tree_is_a_trunk_under_a_crown_of_three_layers_and_a_plus() {
        let mut heights = HashSet::new();
        let mut corners = HashSet::new();
        for tree in structures(jungle_tree_blocks) {
            let trunk: Vec<i32> = tree
                .iter()
                .filter(|(_, block)| **block == BlockId::JungleLog)
                .map(|(pos, _)| {
                    assert_eq!(pos.xz(), IVec2::ZERO);
                    pos.y
                })
                .collect();
            let height = trunk.len() as i32;
            assert!((5..=9).contains(&height), "{height}");
            assert_eq!(trunk.iter().max(), Some(&(height - 1)));
            heights.insert(height);

            let top = height - 1;
            let ring = |radius: i32| -> HashSet<IVec2> {
                (-radius..=radius)
                    .flat_map(|x| (-radius..=radius).map(move |z| IVec2::new(x, z)))
                    .filter(|offset| *offset != IVec2::ZERO)
                    .collect()
            };
            assert_eq!(leaves(&tree, top + 1), plus());
            assert_eq!(leaves(&tree, top), ring(1));
            for dy in [top - 2, top - 1] {
                let layer = leaves(&tree, dy);
                assert!(layer.is_subset(&ring(2)));
                let missing: Vec<IVec2> = ring(2).difference(&layer).copied().collect();
                assert!(missing.iter().all(|m| m.x.abs() == 2 && m.y.abs() == 2));
                corners.insert(missing.len());
            }
            let crown: usize = (top - 2..=top + 1).map(|dy| leaves(&tree, dy).len()).sum();
            assert_eq!(tree.len() - trunk.len(), crown, "a leaf outside the crown");
        }
        assert_eq!(heights.len(), 5, "{heights:?}");
        assert!(corners.contains(&0) && corners.contains(&4), "{corners:?}");
    }

    #[test]
    fn the_bush_is_a_log_under_a_mound_of_leaves() {
        let mut tops = HashSet::new();
        for bush in structures(bush_blocks) {
            assert_eq!(bush[&IVec3::ZERO], BlockId::JungleLog);
            let logs = bush.values().filter(|b| **b == BlockId::JungleLog).count();
            assert_eq!(logs, 1);

            let ground = leaves(&bush, 0);
            let inner: HashSet<IVec2> = &plus() - &HashSet::from([IVec2::ZERO]);
            assert!(inner.is_subset(&ground));
            assert!(ground
                .iter()
                .all(|offset| (1..=2).contains(&(offset.x.abs() + offset.y.abs()))));
            assert_eq!(leaves(&bush, 1), plus());
            let top = leaves(&bush, 2);
            assert!(top.is_empty() || top == HashSet::from([IVec2::ZERO]));
            tops.insert(top.len());
            let mound = ground.len() + 5 + top.len();
            assert_eq!(bush.len(), 1 + mound, "a leaf outside the mound");
        }
        assert_eq!(tops.len(), 2, "the top leaf grows on some bushes only");
    }
}
