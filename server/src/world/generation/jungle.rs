//! The structures of the jungle, of jungle wood: its trees, its bushes and its giant trees.
//! Like the rest of the generation, each is decided from its column alone, except where a
//! giant roots, which also depends on the columns around.

use bevy::prelude::*;
use shared::world::{BiomeType, BlockId, ColumnShape, SEA_LEVEL};

use super::{Blocks, ColumnRandom, PositionRandom, SALT_TREE_ROLL, SALT_TRUNK_HEIGHT};

/// Each of the 4 corners of the 2 wide layers of a jungle tree's crown, from here.
const SALT_CROWN_CORNER: u64 = 0x400;
/// Each of the 8 outermost leaves of a bush, from here.
const SALT_BUSH_EDGE: u64 = 0x410;
const SALT_BUSH_TOP: u64 = 0x418;
const SALT_BRANCH_COUNT: u64 = 0x420;
const SALT_FIRST_BRANCH: u64 = 0x421;
/// Each branch of a giant uses 3 salts from here, 4 apart from one branch to the next.
const SALT_GIANT_BRANCH: u64 = 0x430;
/// One salt per cell of the 12 × 12 × 4 box of a giant's crown, from here.
const SALT_CROWN_RIM: u64 = 0x800;

/// The largest chance of a giant tree on a column, at the heart of the jungle.
pub(super) const GIANT_CHANCE: f32 = 0.004;
/// Two giant trees never root within this many columns of each other, along x or z.
const GIANT_SPACING: i32 = 8;
/// Largest horizontal distance between the root of a giant tree and any of its blocks: the
/// blob at the tip of a branch 3 logs long off the far side of its trunk (`1 + 3 + 2`).
pub(super) const GIANT_MARGIN: i32 = 6;
/// Highest block of a giant tree above the surface of its column: the top of its crown, 2
/// above a trunk of up to 25 logs.
pub(super) const GIANT_HEIGHT: i32 = 27;
/// A giant's trunk starts this far under its root, so that it fills the gaps under the lower
/// of its four columns. The terrain outranks it everywhere else.
pub(super) const GIANT_SINK: i32 = 3;
/// The four columns of a giant's trunk stand within this many blocks of the root's height.
pub(super) const GIANT_STEP: i32 = 2;
/// The radii of the four layers of a giant's crown, from the one under the top of its trunk.
const CROWN_RADII: [f32; 4] = [4.6, 5.4, 4.4, 2.6];
/// The sides of a trunk, as `(x, z)` steps, each a quarter turn from the one before.
const SIDES: [IVec2; 4] = [IVec2::X, IVec2::Y, IVec2::NEG_X, IVec2::NEG_Y];

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

/// The chance of a giant tree on a column `depth` deep in the jungle (see
/// `ColumnShape::jungle_depth`): none below half depth, then smoothly up to `GIANT_CHANCE`.
pub(super) fn giant_chance(depth: f32) -> f32 {
    let t = ((depth - 0.5) / 0.5).clamp(0.0, 1.0);
    GIANT_CHANCE * t * t * (3.0 - 2.0 * t)
}

/// Whether a giant tree roots on the column `(x, z)`, with the shapes of the columns given by
/// `shape`. The column must be a candidate (see [`giant_candidate`]) whose tree roll is the
/// lowest of the candidates within `GIANT_SPACING` columns, the coordinates breaking ties. Only
/// the shapes of the columns whose roll is below `GIANT_CHANCE` are read, and of the three
/// others of their trunk: elsewhere, this costs a hash per column.
pub(super) fn giant_roots(
    seed: u32,
    x: i32,
    z: i32,
    shape: &impl Fn(i32, i32) -> ColumnShape,
) -> bool {
    let roll = tree_roll(seed, x, z);
    if roll >= GIANT_CHANCE || !giant_candidate(x, z, roll, shape) {
        return false;
    }
    for dx in -GIANT_SPACING..=GIANT_SPACING {
        for dz in -GIANT_SPACING..=GIANT_SPACING {
            let (other_x, other_z) = (x + dx, z + dz);
            let other = tree_roll(seed, other_x, other_z);
            let lower = (other, other_x, other_z) < (roll, x, z);
            if lower && giant_candidate(other_x, other_z, other, shape) {
                return false;
            }
        }
    }
    true
}

/// The roll that decides which tree grows on the column `(x, z)`, a giant first.
fn tree_roll(seed: u32, x: i32, z: i32) -> f32 {
    PositionRandom {
        seed,
        coords: [x, z],
    }
    .unit(SALT_TREE_ROLL)
}

/// Whether the column `(x, z)`, whose tree roll is `roll`, may root a giant: it lies in the
/// jungle, deep enough for its roll, and its `+x`, `+z` and `+x+z` neighbours, which hold the
/// rest of the trunk, are within `GIANT_STEP` blocks of its height. All four are dry: above the
/// beach, where the surface of the jungle is grass.
fn giant_candidate(x: i32, z: i32, roll: f32, shape: &impl Fn(i32, i32) -> ColumnShape) -> bool {
    let root = shape(x, z);
    let dry = |height: i32| height > SEA_LEVEL + 1;
    root.biome == BiomeType::Jungle
        && roll < giant_chance(root.jungle_depth)
        && dry(root.height)
        && [(1, 0), (0, 1), (1, 1)].iter().all(|(dx, dz)| {
            let height = shape(x + dx, z + dz).height;
            dry(height) && (height - root.height).abs() <= GIANT_STEP
        })
}

/// A giant tree rooted on `root` and on the three columns at `+x`, `+z` and `+x+z`: a 2 × 2
/// trunk of 16 to 25 jungle logs above the root, sunk `GIANT_SINK` under it, its branches, and
/// a crown of four frayed disks of leaves around the trunk's centre from one block under its
/// top.
pub(super) fn giant_tree_blocks(root: IVec3, random: &ColumnRandom, blocks: &mut Blocks) {
    let height = 16 + random.below(SALT_TRUNK_HEIGHT, 10);
    for (dx, dz) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
        for dy in -GIANT_SINK..height {
            blocks.push((root + IVec3::new(dx, dy, dz), BlockId::JungleLog));
        }
    }
    giant_branch_blocks(root, height, random, blocks);
    giant_crown_blocks(root + IVec3::Y * (height - 1), random, blocks);
}

/// 2 to 4 branches, each off another side of a trunk `height` logs high, from `height / 2` to
/// `4 × height / 5` above `root`: 2 or 3 logs, each one column further out and one block higher
/// than the one before, the last under a blob of leaves, a disk of radius 2.4 and one of 1.5
/// above it.
fn giant_branch_blocks(root: IVec3, height: i32, random: &ColumnRandom, blocks: &mut Blocks) {
    let count = 2 + random.below(SALT_BRANCH_COUNT, 3);
    let first = random.below(SALT_FIRST_BRANCH, 4);
    for branch in 0..count {
        let side = SIDES[((first + branch * 4 / count) % 4) as usize];
        let salt = SALT_GIANT_BRANCH + 4 * branch as u64;
        // one of the two trunk columns along that side
        let along = random.below(salt, 2);
        let start = IVec2::new(
            if side.x == 0 { along } else { (side.x + 1) / 2 },
            if side.y == 0 { along } else { (side.y + 1) / 2 },
        );
        let (low, high) = (height / 2, 4 * height / 5);
        let y = low + random.below(salt + 1, (high - low + 1) as u64);
        let length = 2 + random.below(salt + 2, 2);
        let log = |i: i32| {
            let out = start + side * (i + 1);
            root + IVec3::new(out.x, y + i, out.y)
        };
        for i in 0..length {
            blocks.push((log(i), BlockId::JungleLog));
        }
        let tip = log(length - 1);
        for (dy, radius) in [(0, 2.4f32), (1, 1.5)] {
            for dx in -2i32..=2 {
                for dz in -2i32..=2 {
                    let on_disk = Vec2::new(dx as f32, dz as f32).length() <= radius;
                    if on_disk && (dx, dy, dz) != (0, 0, 0) {
                        blocks.push((tip + IVec3::new(dx, dy, dz), BlockId::JungleLeaves));
                    }
                }
            }
        }
    }
}

/// The four layers of a giant's crown, from one under `top`, the top log of the root column of
/// its trunk: disks of `CROWN_RADII` around the trunk's centre, the corner its four columns
/// share, whose rims (the outermost unit of each radius) are frayed, each leaf growing one time
/// in two.
fn giant_crown_blocks(top: IVec3, random: &ColumnRandom, blocks: &mut Blocks) {
    for (layer, radius) in CROWN_RADII.into_iter().enumerate() {
        let dy = layer as i32 - 1;
        for dx in -5i32..=6 {
            for dz in -5i32..=6 {
                let distance = (Vec2::new(dx as f32, dz as f32) - 0.5).length();
                let trunk = dy <= 0 && (0..=1).contains(&dx) && (0..=1).contains(&dz);
                if distance > radius || trunk {
                    continue;
                }
                let cell = (layer as i32 * 12 + dx + 5) * 12 + dz + 5;
                if distance > radius - 1.0 && !grows(random, SALT_CROWN_RIM + cell as u64) {
                    continue;
                }
                blocks.push((top + IVec3::new(dx, dy, dz), BlockId::JungleLeaves));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::precedence;
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

    /// The four columns of a giant's trunk, by offset from its root.
    const TRUNK: [IVec2; 4] = [
        IVec2::new(0, 0),
        IVec2::new(1, 0),
        IVec2::new(0, 1),
        IVec2::new(1, 1),
    ];

    /// The layers of a giant's crown, by height above the top of its trunk, and their radii.
    const CROWN: [(i32, f32); 4] = [(-1, 4.6), (0, 5.4), (1, 4.4), (2, 2.6)];

    /// How far the centre of the cell at `offset` from a giant's root lies from the centre of
    /// its trunk, the corner its four columns share.
    fn from_trunk_centre(offset: IVec2) -> f32 {
        (offset.as_vec2() + 0.5 - 1.0).length()
    }

    /// Giants rooted on many columns of a few seeds: their blocks by offset from the root, each
    /// cell holding the block of highest rank, and the height of their trunk above the root.
    fn giants() -> Vec<(HashMap<IVec3, BlockId>, i32)> {
        let mut giants = Vec::new();
        for seed in [3, 11, 42] {
            for i in 0..300 {
                let random = PositionRandom {
                    seed,
                    coords: [i % 20, i / 20],
                };
                let mut blocks = Vec::new();
                giant_tree_blocks(ROOT, &random, &mut blocks);
                let mut by_offset: HashMap<IVec3, BlockId> = HashMap::new();
                for (pos, block) in blocks {
                    let cell = by_offset.entry(pos - ROOT).or_insert(block);
                    if precedence(block) > precedence(*cell) {
                        *cell = block;
                    }
                }
                let height = (0..)
                    .take_while(|y| {
                        by_offset.get(&IVec3::new(0, *y, 0)) == Some(&BlockId::JungleLog)
                    })
                    .count() as i32;
                giants.push((by_offset, height));
            }
        }
        giants
    }

    #[test]
    fn the_giant_is_a_two_by_two_trunk_under_four_frayed_disks() {
        let mut heights = HashSet::new();
        let (mut rim_grown, mut rim_bare) = (0, 0);
        for (giant, height) in giants() {
            assert!((16..=25).contains(&height), "{height}");
            heights.insert(height);
            for column in TRUNK {
                let mut logs: Vec<i32> = giant
                    .iter()
                    .filter(|(pos, block)| pos.xz() == column && **block == BlockId::JungleLog)
                    .map(|(pos, _)| pos.y)
                    .collect();
                logs.sort();
                assert_eq!(logs, (-3..height).collect::<Vec<_>>(), "{column}");
            }
            let top = height - 1;
            for (dy, radius) in CROWN {
                let layer = leaves(&giant, top + dy);
                for x in -GIANT_MARGIN..=GIANT_MARGIN {
                    for z in -GIANT_MARGIN..=GIANT_MARGIN {
                        let offset = IVec2::new(x, z);
                        let distance = from_trunk_centre(offset);
                        if dy <= 0 && TRUNK.contains(&offset) {
                            continue;
                        }
                        // a leaf, or a log of a branch passing through
                        let filled = giant.contains_key(&IVec3::new(x, top + dy, z));
                        if distance <= radius - 1.0 {
                            assert!(filled, "{offset} at {dy} over the top");
                        } else if distance <= radius && layer.contains(&offset) {
                            rim_grown += 1;
                        } else if distance <= radius {
                            rim_bare += 1;
                        }
                    }
                }
            }
        }
        assert_eq!(heights.len(), 10, "{heights:?}");
        // about one leaf of the rim in two, a few more where a blob of a branch fills it
        let grown = rim_grown as f32 / (rim_grown + rim_bare) as f32;
        assert!((0.45..0.6).contains(&grown), "{grown}");
    }

    #[test]
    fn the_giant_has_two_to_four_branches_rising_outward_to_blobs() {
        let (mut counts, mut lengths) = (HashSet::new(), HashSet::new());
        for (giant, height) in giants() {
            let mut branches: HashMap<IVec2, Vec<IVec3>> = HashMap::new();
            for (pos, block) in &giant {
                if *block == BlockId::JungleLog && !TRUNK.contains(&pos.xz()) {
                    // the side of the trunk the log lies off
                    let side = if pos.x < 0 {
                        IVec2::NEG_X
                    } else if pos.x > 1 {
                        IVec2::X
                    } else if pos.z < 0 {
                        IVec2::NEG_Y
                    } else {
                        IVec2::Y
                    };
                    branches.entry(side).or_default().push(*pos);
                }
            }
            assert!((2..=4).contains(&branches.len()), "{branches:?}");
            counts.insert(branches.len());
            let mut tips = Vec::new();
            for (side, mut logs) in branches {
                logs.sort_by_key(|pos| pos.y);
                assert!((2..=3).contains(&logs.len()), "{side}: {logs:?}");
                lengths.insert(logs.len());
                let first = logs[0];
                assert!(
                    TRUNK.contains(&(first.xz() - side)),
                    "{first} off the trunk"
                );
                assert!(
                    (height / 2..=4 * height / 5).contains(&first.y),
                    "{first} on a trunk of {height}"
                );
                for (i, log) in logs.iter().enumerate() {
                    let step = IVec3::new(side.x, 1, side.y) * i as i32;
                    assert_eq!(*log, first + step, "one out and one up from {first}");
                }
                let tip = *logs.last().unwrap();
                for (dy, radius) in [(0, 2.4f32), (1, 1.5)] {
                    for x in -2..=2 {
                        for z in -2..=2 {
                            let offset = IVec3::new(x, dy, z);
                            if offset.xz().as_vec2().length() <= radius {
                                assert!(giant.contains_key(&(tip + offset)), "{tip} + {offset}");
                            }
                        }
                    }
                }
                tips.push(tip);
            }
            // every leaf belongs to the crown or to a blob
            let top = height - 1;
            for (pos, block) in &giant {
                if *block != BlockId::JungleLeaves {
                    continue;
                }
                let in_crown = CROWN.iter().any(|(dy, radius)| {
                    pos.y == top + dy && from_trunk_centre(pos.xz()) <= *radius
                });
                let in_blob = tips.iter().any(|tip| {
                    let distance = (pos.xz() - tip.xz()).as_vec2().length();
                    (pos.y == tip.y && distance <= 2.4) || (pos.y == tip.y + 1 && distance <= 1.5)
                });
                assert!(in_crown || in_blob, "a stray leaf at {pos}");
            }
        }
        assert_eq!(counts.len(), 3, "{counts:?}");
        assert_eq!(lengths.len(), 2, "{lengths:?}");
    }

    #[test]
    fn giants_stay_within_their_margin_and_under_their_height() {
        let (mut widest, mut highest, mut lowest) = (0, i32::MIN, i32::MAX);
        for seed in [0, 7, 42, 1234, u32::MAX] {
            for x in 0..40 {
                for z in 0..40 {
                    let random = PositionRandom {
                        seed,
                        coords: [x, z],
                    };
                    let mut blocks = Vec::new();
                    giant_tree_blocks(ROOT, &random, &mut blocks);
                    for (pos, _) in blocks {
                        let offset = pos - ROOT;
                        let wide = offset.x.abs().max(offset.z.abs());
                        // above the surface, which lies under the root
                        let high = offset.y + 1;
                        assert!(wide <= GIANT_MARGIN && high <= GIANT_HEIGHT, "{offset}");
                        assert!(offset.y >= -3, "{offset}");
                        widest = widest.max(wide);
                        highest = highest.max(high);
                        lowest = lowest.min(offset.y);
                    }
                }
            }
        }
        assert_eq!((widest, highest, lowest), (GIANT_MARGIN, GIANT_HEIGHT, -3));
    }

    /// A flat jungle `depth` deep, well above the sea.
    fn flat(depth: f32) -> impl Fn(i32, i32) -> ColumnShape {
        flat_at(70, depth)
    }

    fn flat_at(height: i32, depth: f32) -> impl Fn(i32, i32) -> ColumnShape {
        move |_, _| ColumnShape {
            height,
            biome: BiomeType::Jungle,
            jungle_depth: depth,
        }
    }

    fn roll(seed: u32, x: i32, z: i32) -> f32 {
        PositionRandom {
            seed,
            coords: [x, z],
        }
        .unit(SALT_TREE_ROLL)
    }

    /// The columns of the square `[0, side)²` a giant roots on, among `shapes`.
    fn roots(seed: u32, side: i32, shapes: &impl Fn(i32, i32) -> ColumnShape) -> Vec<IVec2> {
        (0..side)
            .flat_map(|x| (0..side).map(move |z| IVec2::new(x, z)))
            .filter(|c| giant_roots(seed, c.x, c.y, shapes))
            .collect()
    }

    #[test]
    fn a_giant_roots_only_in_the_deep_jungle_on_dry_level_ground() {
        let seed = 21;
        assert!(
            roots(seed, 150, &flat(0.5)).is_empty(),
            "none below half depth"
        );
        let found = roots(seed, 150, &flat(1.0));
        assert!(found.len() > 10, "{}", found.len());
        for root in found {
            // `changed` replaces the shape of one column of a flat deep jungle at `height`
            let roots_with = |height: i32, at: IVec2, changed: ColumnShape| {
                let shapes = |x, z| {
                    if IVec2::new(x, z) == at {
                        changed
                    } else {
                        flat_at(height, 1.0)(x, z)
                    }
                };
                giant_roots(seed, root.x, root.y, &shapes)
            };
            let jungle = |height| ColumnShape {
                height,
                biome: BiomeType::Jungle,
                jungle_depth: 1.0,
            };
            let forest = ColumnShape {
                biome: BiomeType::Forest,
                jungle_depth: 0.0,
                ..jungle(70)
            };
            let low = SEA_LEVEL + 2;
            for side in [IVec2::X, IVec2::Y, IVec2::ONE] {
                let beside = root + side;
                assert!(roots_with(70, beside, jungle(72)), "{root}: a step of 2");
                assert!(
                    roots_with(70, beside, jungle(68)),
                    "{root}: a step of 2 down"
                );
                assert!(!roots_with(70, beside, jungle(73)), "{root}: a step of 3");
                assert!(
                    !roots_with(70, beside, jungle(67)),
                    "{root}: a step of 3 down"
                );
                assert!(roots_with(low, beside, jungle(low)), "{root}: by the sea");
                assert!(!roots_with(low, beside, jungle(low - 1)), "{root}: a beach");
                assert!(roots_with(70, beside, forest), "{root}: beside the forest");
            }
            assert!(
                roots_with(70, root - IVec2::X, jungle(80)),
                "{root}: a cliff behind"
            );
            let shallow = ColumnShape {
                jungle_depth: 0.5,
                ..jungle(70)
            };
            assert!(!roots_with(70, root, shallow), "{root}: at half depth");
            assert!(!roots_with(70, root, forest), "{root}: in the forest");
            assert!(
                !roots_with(low, root, jungle(low - 1)),
                "{root}: on the beach"
            );
        }
    }

    #[test]
    fn two_giants_never_root_within_eight_columns() {
        let found = roots(4, 200, &flat(1.0));
        assert!(found.len() > 50, "{}", found.len());
        for (i, a) in found.iter().enumerate() {
            for b in &found[i + 1..] {
                let distance = (*a - *b).abs().max_element();
                assert!(distance > GIANT_SPACING, "{a} and {b}");
            }
        }
    }

    #[test]
    fn a_candidate_roots_unless_one_within_eight_columns_rolls_lower() {
        let seed = 8;
        let shapes = flat(1.0);
        let candidate = |c: IVec2| roll(seed, c.x, c.y) < GIANT_CHANCE;
        let (mut grown, mut beaten) = (0, 0);
        for x in 0..150 {
            for z in 0..150 {
                let column = IVec2::new(x, z);
                if !candidate(column) {
                    assert!(!giant_roots(seed, x, z, &shapes));
                    continue;
                }
                let lower = (-GIANT_SPACING..=GIANT_SPACING)
                    .flat_map(|dx| (-GIANT_SPACING..=GIANT_SPACING).map(move |dz| (dx, dz)))
                    .map(|(dx, dz)| column + IVec2::new(dx, dz))
                    .any(|other| {
                        other != column
                            && candidate(other)
                            && roll(seed, other.x, other.y) < roll(seed, x, z)
                    });
                assert_eq!(giant_roots(seed, x, z, &shapes), !lower, "{column}");
                if lower {
                    beaten += 1;
                } else {
                    grown += 1;
                }
            }
        }
        assert!(grown > 10 && beaten > 10, "{grown} grown, {beaten} beaten");
    }

    #[test]
    fn a_column_shape_is_read_only_under_the_largest_giant_chance() {
        let seed = 13;
        let read = std::cell::RefCell::new(HashSet::new());
        let shapes = |x, z| {
            read.borrow_mut().insert(IVec2::new(x, z));
            flat(1.0)(x, z)
        };
        let found = roots(seed, 100, &shapes);
        assert!(!found.is_empty());
        let read = read.into_inner();
        // a candidate and the three other columns of its trunk
        for column in &read {
            let candidate = [IVec2::ZERO, IVec2::X, IVec2::Y, IVec2::ONE]
                .iter()
                .any(|side| roll(seed, column.x - side.x, column.y - side.y) < GIANT_CHANCE);
            assert!(candidate, "{column} was read");
        }
        assert!(read.len() < 300, "{} columns read", read.len());
    }

    #[test]
    fn giants_are_about_as_dense_as_their_spacing_allows() {
        // A candidate is one column in 250; it grows when its roll is the lowest of the
        // candidates of the 17 × 17 columns around, which happens about 59 % of the time.
        let side = 400;
        let found = roots(17, side, &flat(1.0));
        let share = found.len() as f32 / (side * side) as f32;
        let expected = GIANT_CHANCE * 0.593;
        assert!(
            (share - expected).abs() < 0.15 * expected,
            "{share} instead of {expected}"
        );
        assert_eq!(giant_chance(1.0), GIANT_CHANCE);
        assert_eq!(giant_chance(0.5), 0.0);
        assert_eq!(giant_chance(0.75), GIANT_CHANCE / 2.0);
    }
}
