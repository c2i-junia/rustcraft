//! The way a walking hostile finds to its prey: an A* search over the cells its feet may stand
//! in (`find_path`), bounded in nodes and in distance, which walks round a wall, jumps up a step
//! of one block, drops down a ledge no deeper than its walker may fall, and goes through a door
//! if its walker opens doors. The search only reads cells (`Cell`), so it runs on any grid; the
//! world's comes from `cell_at`. The searches of one tick share a budget of nodes (`Pathing`),
//! so that a crowd of hostiles never slows the server down: those over it walk on as they did.

use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap};

use bevy::math::IVec3;
use shared::world::{global_block_to_chunk_pos, BlockHitbox, BlockId, BlockTransparency, ChunkMap};

/// The most nodes one search expands.
pub const MAX_NODES: usize = 400;
/// The most nodes all the searches of one tick expand together.
pub const TICK_NODES: usize = 1600;
/// How far from its start a search goes, along each axis, in blocks.
pub const MAX_RANGE: i32 = 24;
/// The deepest drop a walker in full health takes, in blocks.
pub const MAX_DROP: i32 = 6;

/// The costs of the moves, in tenths of a block.
const FLAT: u32 = 10;
const DIAGONAL: u32 = 14;
const STEP_UP: u32 = 20;
const DROP_PER_BLOCK: u32 = 5;
const DOOR: u32 = 20;

/// What a cell is to a walker.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cell {
    /// Nothing in the way: air, or a block without a hitbox.
    Open,
    /// A block to stand on, and not to walk through.
    Solid,
    /// A block too tall to stand on or to jump over: a fence.
    Tall,
    /// Water, lava or fire: a walker keeps out.
    Hazard,
    /// A half of a door.
    Door { open: bool },
    /// Not loaded: a walker keeps out.
    Unknown,
}

/// What the world's cell at `pos` is to a walker.
pub fn cell_at(chunks: &ChunkMap, pos: IVec3) -> Cell {
    if !chunks.is_chunk_loaded(global_block_to_chunk_pos(&pos)) {
        return Cell::Unknown;
    }
    let Some(block) = chunks.get_block(pos) else {
        return Cell::Open;
    };
    match block.id {
        BlockId::Door | BlockId::DoorTop => Cell::Door { open: false },
        BlockId::OpenDoor | BlockId::OpenDoorTop => Cell::Door { open: true },
        BlockId::Fence => Cell::Tall,
        BlockId::Fire | BlockId::Lava => Cell::Hazard,
        id if id.get_visibility() == BlockTransparency::Liquid => Cell::Hazard,
        id if matches!(id.get_hitbox(), BlockHitbox::None) => Cell::Open,
        _ => Cell::Solid,
    }
}

/// How a walker goes about.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Walker {
    /// How many cells tall its box is.
    pub clearance: i32,
    /// The deepest drop it takes, in blocks.
    pub max_drop: i32,
    /// Whether it opens the doors in its way.
    pub opens_doors: bool,
}

/// The way a search found: the cells the walker's feet go through after its start, in order,
/// and whether the last one is the goal (else it is the cell nearest to it the search met).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Found {
    pub cells: Vec<IVec3>,
    pub reaches: bool,
}

/// The nodes the searches of one tick may still expand, and the doors the hostiles open in
/// that tick, which the caller opens once they are all done (`open_doors`).
#[derive(Debug)]
pub struct Pathing {
    pub nodes_left: usize,
    pub doors: Vec<IVec3>,
}

impl Default for Pathing {
    fn default() -> Self {
        Self {
            nodes_left: TICK_NODES,
            doors: Vec::new(),
        }
    }
}

/// Opens each closed door of `doors`, with its other half.
pub fn open_doors(chunks: &mut ChunkMap, doors: &[IVec3]) {
    for &pos in doors {
        let Some(block) = chunks.get_block(pos).copied() else {
            continue;
        };
        if !matches!(block.id, BlockId::Door | BlockId::DoorTop) {
            continue;
        }
        let Some((opened, step)) = block.id.door() else {
            continue;
        };
        chunks.set_block(pos, shared::world::BlockData::new(opened, block.direction));
        let other = pos + step;
        if let Some(half) = chunks.get_block(other).copied() {
            if let (BlockId::Door | BlockId::DoorTop, Some((opened, _))) = (half.id, half.id.door())
            {
                chunks.set_block(other, shared::world::BlockData::new(opened, half.direction));
            }
        }
    }
}

/// The 8 horizontal steps, the 4 straight ones first.
const STEPS: [IVec3; 8] = [
    IVec3::new(1, 0, 0),
    IVec3::new(-1, 0, 0),
    IVec3::new(0, 0, 1),
    IVec3::new(0, 0, -1),
    IVec3::new(1, 0, 1),
    IVec3::new(1, 0, -1),
    IVec3::new(-1, 0, 1),
    IVec3::new(-1, 0, -1),
];

/// A lower bound of the cost from `from` to `to`.
fn estimate(from: IVec3, to: IVec3) -> u32 {
    let (dx, dz) = (
        (from.x - to.x).unsigned_abs(),
        (from.z - to.z).unsigned_abs(),
    );
    let (long, short) = (dx.max(dz), dx.min(dz));
    FLAT * (long - short) + DIAGONAL * short + DROP_PER_BLOCK * (from.y - to.y).unsigned_abs()
}

/// Whether `node` is as good as the goal: right over or under it, a block apart at most.
fn at_goal(node: IVec3, goal: IVec3) -> bool {
    node.x == goal.x && node.z == goal.z && (node.y - goal.y).abs() <= 1
}

struct Grid<F> {
    cell: F,
    walker: Walker,
    cache: HashMap<IVec3, Cell>,
}

impl<F: FnMut(IVec3) -> Cell> Grid<F> {
    fn get(&mut self, pos: IVec3) -> Cell {
        if let Some(cell) = self.cache.get(&pos) {
            return *cell;
        }
        let cell = (self.cell)(pos);
        self.cache.insert(pos, cell);
        cell
    }

    fn passable(&mut self, pos: IVec3) -> bool {
        match self.get(pos) {
            Cell::Open => true,
            Cell::Door { open } => open || self.walker.opens_doors,
            _ => false,
        }
    }

    /// Whether the walker's box fits with its feet in `feet`.
    fn fits(&mut self, feet: IVec3) -> bool {
        (0..self.walker.clearance).all(|up| self.passable(feet + IVec3::Y * up))
    }

    fn standable(&mut self, feet: IVec3) -> bool {
        self.get(feet - IVec3::Y) == Cell::Solid
    }

    /// The extra cost of the closed doors the walker's box meets with its feet in `feet`.
    fn doors(&mut self, feet: IVec3) -> u32 {
        let closed = (0..self.walker.clearance)
            .any(|up| self.get(feet + IVec3::Y * up) == Cell::Door { open: false });
        if closed {
            DOOR
        } else {
            0
        }
    }

    /// The nodes a walker standing in `node` goes to in one move, with their cost.
    fn moves(&mut self, node: IVec3, out: &mut Vec<(IVec3, u32)>) {
        out.clear();
        for step in STEPS {
            let diagonal = step.x != 0 && step.z != 0;
            let next = node + step;
            if diagonal
                && !(self.fits(node + IVec3::new(step.x, 0, 0))
                    && self.fits(node + IVec3::new(0, 0, step.z)))
            {
                continue;
            }
            let cost = if diagonal { DIAGONAL } else { FLAT };
            if self.fits(next) {
                if self.standable(next) {
                    let doors = self.doors(next);
                    out.push((next, cost + doors));
                    continue;
                }
                // A drop: down the open cells to the first to stand on
                for down in 1..=self.walker.max_drop {
                    let feet = next - IVec3::Y * down;
                    if !self.passable(feet) {
                        break;
                    }
                    if self.standable(feet) {
                        out.push((feet, cost + DROP_PER_BLOCK * down as u32));
                        break;
                    }
                }
            } else if !diagonal {
                // A jump onto a step: room over the walker's head, and over the step
                let up = next + IVec3::Y;
                if self.passable(node + IVec3::Y * self.walker.clearance)
                    && self.standable(up)
                    && self.fits(up)
                {
                    out.push((up, STEP_UP));
                }
            }
        }
    }
}

/// The way of `walker` from the cell of its feet `start` to the cell of feet `goal`, through
/// the cells `cell` tells, expanding at most `max_nodes` nodes within `MAX_RANGE` of `start`.
/// Returns the way found, if any but standing still, and how many nodes it expanded. When the
/// goal is out of reach it is the way to the cell nearest to it the search met, if nearer than
/// the start.
pub fn find_path(
    start: IVec3,
    goal: IVec3,
    walker: Walker,
    max_nodes: usize,
    cell: impl FnMut(IVec3) -> Cell,
) -> (Option<Found>, usize) {
    let mut grid = Grid {
        cell,
        walker,
        cache: HashMap::new(),
    };
    // Each node: its cell, the cost to it, and the node it came from
    let mut nodes: Vec<(IVec3, u32, usize)> = vec![(start, 0, usize::MAX)];
    let mut index: HashMap<IVec3, usize> = HashMap::from([(start, 0)]);
    let mut open = BinaryHeap::from([Reverse((estimate(start, goal), 0u32, 0usize))]);
    let mut best = (estimate(start, goal), 0usize);
    let mut expanded = 0;
    let mut moves = Vec::new();
    let mut reached = None;
    while let Some(Reverse((_, cost, i))) = open.pop() {
        if cost > nodes[i].1 {
            continue;
        }
        let node = nodes[i].0;
        if at_goal(node, goal) {
            reached = Some(i);
            break;
        }
        if expanded >= max_nodes {
            break;
        }
        expanded += 1;
        grid.moves(node, &mut moves);
        for &(next, step) in &moves {
            if (next - start).abs().max_element() > MAX_RANGE {
                continue;
            }
            let cost = cost + step;
            let j = match index.get(&next) {
                Some(&j) if nodes[j].1 <= cost => continue,
                Some(&j) => {
                    nodes[j] = (next, cost, i);
                    j
                }
                None => {
                    nodes.push((next, cost, i));
                    index.insert(next, nodes.len() - 1);
                    nodes.len() - 1
                }
            };
            let left = estimate(next, goal);
            if left < best.0 {
                best = (left, j);
            }
            open.push(Reverse((cost + left, cost, j)));
        }
    }
    let (last, reaches) = match reached {
        Some(i) => (i, true),
        None => (best.1, false),
    };
    if last == 0 {
        return (
            reached.map(|_| Found {
                cells: Vec::new(),
                reaches: true,
            }),
            expanded,
        );
    }
    let mut cells = Vec::new();
    let mut i = last;
    while i != 0 {
        cells.push(nodes[i].0);
        i = nodes[i].2;
    }
    cells.reverse();
    (Some(Found { cells, reaches }), expanded)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    /// A grid with a floor of solid cells at y = 0 over x, z in -20..20, the `blocks` added on
    /// it, and every cell outside open.
    fn grid(blocks: &[(IVec3, Cell)]) -> impl FnMut(IVec3) -> Cell + '_ {
        move |pos: IVec3| {
            if let Some((_, cell)) = blocks.iter().find(|(at, _)| *at == pos) {
                return *cell;
            }
            if pos.y == 0 && pos.x.abs() < 20 && pos.z.abs() < 20 {
                Cell::Solid
            } else {
                Cell::Open
            }
        }
    }

    const GNAWER: Walker = Walker {
        clearance: 1,
        max_drop: 3,
        opens_doors: false,
    };
    const STUMPLING: Walker = Walker {
        clearance: 2,
        max_drop: 3,
        opens_doors: true,
    };

    /// A wall 2 high along x = 3, for z in -`half`..=`half`, with `gap` cells left out.
    fn wall(half: i32, gap: &[IVec3]) -> Vec<(IVec3, Cell)> {
        (-half..=half)
            .flat_map(|z| (1..=2).map(move |y| IVec3::new(3, y, z)))
            .filter(|pos| !gap.contains(pos))
            .map(|pos| (pos, Cell::Solid))
            .collect()
    }

    fn feet(x: i32, z: i32) -> IVec3 {
        IVec3::new(x, 1, z)
    }

    /// Each move of `cells` from `start` is one step to a neighbouring column.
    fn is_walk(start: IVec3, cells: &[IVec3]) -> bool {
        let mut at = start;
        cells.iter().all(|&next| {
            let d = (next - at).abs();
            at = next;
            d.x <= 1 && d.z <= 1 && (d.x + d.z) > 0
        })
    }

    #[test]
    fn the_way_across_open_ground_is_straight() {
        let blocks = [];
        let (found, _) = find_path(feet(0, 0), feet(6, 0), GNAWER, MAX_NODES, grid(&blocks));
        let found = found.unwrap();
        assert!(found.reaches);
        assert_eq!(found.cells, (1..=6).map(|x| feet(x, 0)).collect::<Vec<_>>());
    }

    #[test]
    fn a_walker_goes_round_a_wall() {
        let blocks = wall(4, &[]);
        let (found, _) = find_path(feet(0, 0), feet(6, 0), GNAWER, MAX_NODES, grid(&blocks));
        let found = found.unwrap();
        assert!(found.reaches);
        assert!(is_walk(feet(0, 0), &found.cells));
        assert!(found.cells.iter().all(|cell| cell.y == 1));
        assert!(found
            .cells
            .iter()
            .any(|cell| cell.x == 3 && cell.z.abs() > 4));
        assert_eq!(*found.cells.last().unwrap(), feet(6, 0));
    }

    #[test]
    fn a_walker_jumps_up_a_step_of_one_block_but_not_of_two() {
        let step: Vec<_> = (-19..20)
            .map(|z| (IVec3::new(3, 1, z), Cell::Solid))
            .collect();
        let (found, _) = find_path(
            feet(0, 0),
            IVec3::new(5, 2, 0),
            GNAWER,
            MAX_NODES,
            grid(&step),
        );
        let found = found.unwrap();
        assert!(found.reaches);
        assert!(found.cells.contains(&IVec3::new(3, 2, 0)));

        let cliff: Vec<_> = (-19..20)
            .flat_map(|z| {
                [
                    (IVec3::new(3, 1, z), Cell::Solid),
                    (IVec3::new(3, 2, z), Cell::Solid),
                ]
            })
            .collect();
        let (found, _) = find_path(
            feet(0, 0),
            IVec3::new(5, 3, 0),
            GNAWER,
            MAX_NODES,
            grid(&cliff),
        );
        assert!(!found.is_some_and(|found| found.reaches));
    }

    #[test]
    fn a_walker_drops_down_a_ledge_no_deeper_than_it_may_fall() {
        // A pit of open cells over x in 3..=8, its floor `depth` below the ground
        let pit = |depth: i32| -> Vec<(IVec3, Cell)> {
            (3..=8)
                .flat_map(|x| (-19..20).map(move |z| (x, z)))
                .flat_map(|(x, z)| {
                    (1 - depth..=0)
                        .map(move |y| (IVec3::new(x, y, z), Cell::Open))
                        .chain([(IVec3::new(x, -depth, z), Cell::Solid)])
                })
                .collect()
        };
        let shallow = pit(3);
        let goal = IVec3::new(6, -2, 0);
        let (found, _) = find_path(feet(0, 0), goal, GNAWER, MAX_NODES, grid(&shallow));
        let found = found.unwrap();
        assert!(found.reaches);
        assert_eq!(*found.cells.last().unwrap(), goal);

        let deep = pit(5);
        let goal = IVec3::new(6, -4, 0);
        let (found, _) = find_path(feet(0, 0), goal, GNAWER, MAX_NODES, grid(&deep));
        assert!(!found.is_some_and(|found| found.reaches));
    }

    #[test]
    fn only_a_walker_that_opens_doors_goes_through_a_closed_one() {
        // A wall as wide as the floor, with a door in it
        let door = [feet(3, 0), IVec3::new(3, 2, 0)];
        let mut blocks = wall(19, &door);
        blocks.push((door[0], Cell::Door { open: false }));
        blocks.push((door[1], Cell::Door { open: false }));
        let (found, _) = find_path(feet(0, 0), feet(6, 0), STUMPLING, MAX_NODES, grid(&blocks));
        let found = found.unwrap();
        assert!(found.reaches);
        assert!(found.cells.contains(&feet(3, 0)));

        let (found, _) = find_path(feet(0, 0), feet(6, 0), GNAWER, MAX_NODES, grid(&blocks));
        assert!(!found.is_some_and(|found| found.reaches));
    }

    #[test]
    fn a_tall_walker_does_not_go_under_a_low_ceiling() {
        // A gap one block high in the wall
        let mut blocks = wall(19, &[feet(3, 0)]);
        blocks.retain(|(pos, _)| *pos != feet(3, 0));
        let (found, _) = find_path(feet(0, 0), feet(6, 0), GNAWER, MAX_NODES, grid(&blocks));
        assert!(found.unwrap().cells.contains(&feet(3, 0)));
        let (found, _) = find_path(feet(0, 0), feet(6, 0), STUMPLING, MAX_NODES, grid(&blocks));
        assert!(!found.is_some_and(|found| found.reaches));
    }

    #[test]
    fn a_search_out_of_nodes_heads_for_the_nearest_cell_it_met() {
        // A wall too long to go round in 40 nodes
        let blocks = wall(19, &[]);
        let (found, expanded) = find_path(feet(0, 0), feet(6, 0), GNAWER, 40, grid(&blocks));
        assert!(expanded <= 40);
        let found = found.unwrap();
        assert!(!found.reaches);
        assert_eq!(*found.cells.last().unwrap(), feet(2, 0));
    }

    #[test]
    fn a_walker_cuts_no_corner_and_keeps_out_of_water() {
        // Water all along x = 3 but a bridge at z = 5
        let blocks: Vec<_> = (-19..20)
            .filter(|z| *z != 5)
            .map(|z| (IVec3::new(3, 0, z), Cell::Hazard))
            .collect();
        let (found, _) = find_path(feet(0, 0), feet(6, 0), GNAWER, MAX_NODES, grid(&blocks));
        let found = found.unwrap();
        assert!(found.reaches);
        let crossed: HashSet<_> = found.cells.iter().filter(|cell| cell.x == 3).collect();
        assert_eq!(crossed, HashSet::from([&feet(3, 5)]));
    }
}
