//! The plan of a village: a square around a well, streets that wander out of it and branch, and
//! buildings along them, their doors toward the street.
//!
//! Everything is drawn from the seed and the centre of the village, and read from the shape of
//! the terrain (`TerrainNoises`), so that every chunk lays out the same plan.

use std::collections::{HashMap, HashSet, VecDeque};

use bevy::prelude::*;
use shared::world::{
    BlockData, BlockDirection, ColumnShape, Container, ServerMob, TerrainNoises, VillageStyle,
};
use shared::CHUNK_SIZE;

use super::super::PositionRandom;
use super::pieces::{material, road_block, Cell, Piece, PieceKind, Role, LAMP, PIECES, WELL};
use super::placement::{dry, Site, REACH};
use super::{loot, PlacedPiece, VillagePlan};

/// Every draw of the layout takes the next salt from here.
const SALT_LAYOUT: u64 = 0x1100;
/// The third coordinate of the draws of a layout, which no block of a world reaches.
const LAYOUT_TAG: i32 = -0x7EE;

/// Half the side of the square, around the centre.
const SQUARE_HALF: i32 = 4;
/// A street is this many columns on each side of its middle.
const STREET_HALF: i32 = 1;
/// Air is kept this high above the top of a building, a street or a path.
const CLEAR_ABOVE: i32 = 3;
/// Most blocks between the lowest and the highest column under a building.
const MOST_SPREAD: i32 = 3;
/// Most buildings along the streets of a village.
const MOST_PIECES: usize = 14;
/// A lamp stands every this many steps of a street.
const LAMP_EVERY: usize = 9;
/// The most branches the streets of a village make.
const MOST_BRANCHES: usize = 5;

/// The four directions on the ground, in the order of `BlockDirection`: front (-Z), right (+X),
/// back (+Z), left (-X).
const DIRECTIONS: [IVec2; 4] = [IVec2::NEG_Y, IVec2::X, IVec2::Y, IVec2::NEG_X];
const BLOCK_DIRECTIONS: [BlockDirection; 4] = [
    BlockDirection::Front,
    BlockDirection::Right,
    BlockDirection::Back,
    BlockDirection::Left,
];

/// Turns `v` by `turns` quarter turns, front toward right.
fn rotate(v: IVec2, turns: usize) -> IVec2 {
    (0..turns % 4).fold(v, |v, _| IVec2::new(-v.y, v.x))
}

/// Turns a direction by `turns` quarter turns, as `rotate` turns its step.
fn rotate_direction(direction: BlockDirection, turns: usize) -> BlockDirection {
    let index = BLOCK_DIRECTIONS
        .iter()
        .position(|d| *d == direction)
        .unwrap();
    BLOCK_DIRECTIONS[(index + turns) % 4]
}

/// The draws of a layout, one salt after the other.
struct Draws {
    random: PositionRandom<3>,
    next: u64,
}

impl Draws {
    fn unit(&mut self) -> f32 {
        self.next += 1;
        self.random.unit(SALT_LAYOUT + self.next)
    }

    fn below(&mut self, count: u64) -> i32 {
        self.next += 1;
        self.random.below(SALT_LAYOUT + self.next, count)
    }
}

/// The shape of the columns around a village.
struct Land {
    origin: IVec2,
    side: i32,
    shapes: Vec<ColumnShape>,
}

impl Land {
    fn height(&self, at: IVec2) -> i32 {
        let d = at - self.origin;
        self.shapes[(d.y * self.side + d.x) as usize].height
    }

    fn dry(&self, at: IVec2) -> bool {
        dry(self.height(at))
    }
}

/// A street being drawn.
struct Walker {
    at: IVec2,
    direction: IVec2,
    /// Steps it has left.
    budget: i32,
    /// Steps before it may turn or branch.
    segment: i32,
}

struct Builder<'a> {
    seed: u32,
    site: Site,
    land: &'a Land,
    draws: Draws,
    blocks: Vec<(IVec3, Option<BlockData>)>,
    /// The columns of the streets, the paths and the buildings.
    taken: HashSet<IVec2>,
    /// The columns of the buildings alone.
    built: HashSet<IVec2>,
    footprint: HashSet<IVec2>,
    /// The middle of each step of the streets and its direction, in the order drawn.
    steps: Vec<(IVec2, IVec2)>,
    pieces: Vec<PlacedPiece>,
    counts: HashMap<PieceKind, usize>,
    animals: Vec<ServerMob>,
    containers: Vec<(IVec3, Container)>,
}

/// The plan of the village of `site`, in the world of `seed` shaped by `terrain`.
pub fn layout(seed: u32, terrain: &TerrainNoises, site: Site) -> VillagePlan {
    let origin = site.centre - IVec2::splat(REACH);
    let side = 2 * REACH + 1;
    let land = Land {
        origin,
        side,
        shapes: terrain.column_shapes((origin.x, origin.y), side, side),
    };
    let mut builder = Builder {
        seed,
        site,
        land: &land,
        draws: Draws {
            random: PositionRandom {
                seed,
                coords: [site.centre.x, LAYOUT_TAG, site.centre.y],
            },
            next: 0,
        },
        blocks: Vec::new(),
        taken: HashSet::new(),
        built: HashSet::new(),
        footprint: HashSet::new(),
        steps: Vec::new(),
        pieces: Vec::new(),
        counts: HashMap::new(),
        animals: Vec::new(),
        containers: Vec::new(),
    };
    builder.square();
    builder.streets();
    builder.buildings();
    builder.finish()
}

impl Builder<'_> {
    /// Whether `at` is no farther than `REACH - margin` from the centre along x and z.
    fn within(&self, at: IVec2, margin: i32) -> bool {
        (at - self.site.centre).abs().max_element() <= REACH - margin
    }

    fn block(&mut self, pos: IVec3, block: Option<BlockData>) {
        self.blocks.push((pos, block));
    }

    fn keep_clear(&mut self, at: IVec2, ring: i32) {
        for dx in -ring..=ring {
            for dz in -ring..=ring {
                self.footprint.insert(at + IVec2::new(dx, dz));
            }
        }
    }

    /// Covers the surface of the column `at` with the street, and clears the air above.
    fn street(&mut self, at: IVec2) {
        if !self.taken.insert(at) {
            return;
        }
        self.keep_clear(at, 1);
        let height = self.land.height(at);
        let road = road_block(self.site.style);
        self.block(
            IVec3::new(at.x, height, at.y),
            Some(BlockData::new(road, BlockDirection::Front)),
        );
        for y in height + 1..=height + CLEAR_ABOVE {
            self.block(IVec3::new(at.x, y, at.y), None);
        }
    }

    /// The square, paved as a street, around a well.
    fn square(&mut self) {
        let centre = self.site.centre;
        for dx in -SQUARE_HALF..=SQUARE_HALF {
            for dz in -SQUARE_HALF..=SQUARE_HALF {
                let at = centre + IVec2::new(dx, dz);
                if self.land.dry(at) {
                    self.street(at);
                }
            }
        }
        let floor = self.site.height;
        self.build(&WELL, centre - IVec2::new(0, 2), 0, floor);
    }

    /// The streets: from two to four sides of the square, each wandering out, turning, and
    /// branching off, until it has walked its steps, reaches the water, a slope or the edge of
    /// the village, or joins another street.
    fn streets(&mut self) {
        let mut walkers = VecDeque::new();
        for (i, direction) in DIRECTIONS.into_iter().enumerate() {
            let kept = self.draws.unit() < 0.8 || walkers.len() + (4 - i) <= 2;
            if kept {
                let budget = 22 + self.draws.below(30);
                let segment = 8 + self.draws.below(10);
                walkers.push_back(Walker {
                    at: self.site.centre + direction * SQUARE_HALF,
                    direction,
                    budget,
                    segment,
                });
            }
        }
        let mut branches = 0;
        while let Some(mut walker) = walkers.pop_front() {
            let mut walked = 0;
            while walker.budget > 0 {
                // ahead, else to one side or the other where the way ahead is blocked
                let mut ways = vec![walker.direction];
                if walked > 0 {
                    let across = IVec2::new(-walker.direction.y, walker.direction.x);
                    if self.draws.unit() < 0.5 {
                        ways.extend([across, -across]);
                    } else {
                        ways.extend([-across, across]);
                    }
                }
                let Some(direction) = ways.into_iter().find(|&way| self.open(walker.at, way))
                else {
                    break;
                };
                walker.direction = direction;
                let next = walker.at + direction;
                let across = IVec2::new(-direction.y, direction.x);
                let cells: Vec<IVec2> = (-STREET_HALF..=STREET_HALF)
                    .map(|k| next + across * k)
                    .collect();
                let joins = walked > 2 && self.taken.contains(&next);
                for cell in cells {
                    self.street(cell);
                }
                if joins {
                    break;
                }
                self.steps.push((next, walker.direction));
                walker.at = next;
                walker.budget -= 1;
                walker.segment -= 1;
                walked += 1;
                if walker.segment > 0 {
                    continue;
                }
                walker.segment = 8 + self.draws.below(10);
                let turn = self.draws.unit();
                let branch = self.draws.unit();
                let side = if self.draws.unit() < 0.5 {
                    across
                } else {
                    -across
                };
                if branches < MOST_BRANCHES && branch < 0.5 {
                    branches += 1;
                    let segment = 6 + self.draws.below(8);
                    walkers.push_back(Walker {
                        at: walker.at + side * STREET_HALF,
                        direction: side,
                        budget: walker.budget / 2 + 6,
                        segment,
                    });
                }
                if turn < 0.2 {
                    walker.direction = across;
                } else if turn < 0.4 {
                    walker.direction = -across;
                }
            }
        }
    }

    /// Whether a street may step from `at` toward `direction`: the street there stays in the
    /// village, on dry land, off the buildings, and climbs or falls one block at most.
    fn open(&self, at: IVec2, direction: IVec2) -> bool {
        let next = at + direction;
        let across = IVec2::new(-direction.y, direction.x);
        self.within(next, STREET_HALF + 2)
            && (self.land.height(next) - self.land.height(at)).abs() <= 1
            && (-STREET_HALF..=STREET_HALF).all(|k| {
                let cell = next + across * k;
                self.land.dry(cell) && !self.built.contains(&cell)
            })
    }

    /// The buildings along the streets, and their lamps.
    fn buildings(&mut self) {
        let steps = self.steps.clone();
        for (i, &(at, direction)) in steps.iter().enumerate() {
            let across = IVec2::new(-direction.y, direction.x);
            for side in [across, -across] {
                let buildings: usize = self.counts.values().sum();
                if buildings < MOST_PIECES && self.draws.unit() < 0.45 {
                    self.try_piece(at, side);
                }
            }
            if i % LAMP_EVERY == LAMP_EVERY / 2 {
                let side = if self.draws.unit() < 0.5 {
                    across
                } else {
                    -across
                };
                self.try_lamp(at + side * (STREET_HALF + 1));
            }
        }
    }

    /// A piece drawn by weight among those the village may still hold.
    fn draw_piece(&mut self) -> Option<&'static Piece> {
        let allowed: Vec<&'static Piece> = PIECES
            .iter()
            .filter(|piece| self.counts.get(&piece.kind).copied().unwrap_or(0) < piece.most)
            .collect();
        let total: u32 = allowed.iter().map(|piece| piece.weight).sum();
        if total == 0 {
            return None;
        }
        let mut roll = self.draws.below(total as u64) as u32;
        for piece in allowed {
            if roll < piece.weight {
                return Some(piece);
            }
            roll -= piece.weight;
        }
        None
    }

    /// The columns of `piece` turned `turns` times, its door column at `anchor`.
    fn columns_of(piece: &Piece, anchor: IVec2, turns: usize) -> Vec<IVec2> {
        let (width, depth, _) = piece.size();
        (0..depth)
            .flat_map(|z| (0..width).map(move |x| (x, z)))
            .map(|(x, z)| anchor + rotate(IVec2::new(x - piece.door, z), turns))
            .collect()
    }

    /// Builds a piece drawn at random on the `side` of the street step `at`, its front toward
    /// the street and a path from its door to it, if the land there allows.
    fn try_piece(&mut self, at: IVec2, side: IVec2) {
        let Some(piece) = self.draw_piece() else {
            return;
        };
        let gap = at + side * (STREET_HALF + 1);
        let anchor = at + side * (STREET_HALF + 2);
        // the front, toward the street, is the opposite of the side
        let turns = DIRECTIONS.iter().position(|d| *d == -side).unwrap();
        let columns = Self::columns_of(piece, anchor, turns);
        let free = |builder: &Self, cell: IVec2| {
            builder.within(cell, 2) && builder.land.dry(cell) && !builder.taken.contains(&cell)
        };
        if !free(self, gap) || !columns.iter().all(|&cell| free(self, cell)) {
            return;
        }
        // one column at least between two buildings
        let crowded = columns.iter().any(|&cell| {
            DIRECTIONS
                .iter()
                .chain(&[
                    IVec2::ONE,
                    -IVec2::ONE,
                    IVec2::new(1, -1),
                    IVec2::new(-1, 1),
                ])
                .any(|&d| self.built.contains(&(cell + d)))
        });
        if crowded {
            return;
        }
        let mut heights: Vec<i32> = columns.iter().map(|&cell| self.land.height(cell)).collect();
        heights.sort();
        let (low, high) = (heights[0], heights[heights.len() - 1]);
        let floor = heights[heights.len() / 2];
        if high - low > MOST_SPREAD || (self.land.height(gap) - floor).abs() > 1 {
            return;
        }
        *self.counts.entry(piece.kind).or_default() += 1;
        self.path(gap, floor);
        self.build(piece, anchor, turns, floor);
    }

    /// A lamp on the column `at`, beside a street, if it is free.
    fn try_lamp(&mut self, at: IVec2) {
        if !self.within(at, 2) || !self.land.dry(at) || self.taken.contains(&at) {
            return;
        }
        let floor = self.land.height(at);
        self.build(&LAMP, at, 0, floor);
    }

    /// The path from a door to the street: a column of the street's block at the height of the
    /// floor, on a foundation.
    fn path(&mut self, at: IVec2, floor: i32) {
        self.taken.insert(at);
        self.keep_clear(at, 1);
        let height = self.land.height(at);
        let foundation = material(self.site.style, Role::Foundation);
        for y in height + 1..floor {
            self.block(
                IVec3::new(at.x, y, at.y),
                Some(BlockData::new(foundation, BlockDirection::Front)),
            );
        }
        let road = road_block(self.site.style);
        self.block(
            IVec3::new(at.x, floor, at.y),
            Some(BlockData::new(road, BlockDirection::Front)),
        );
        for y in floor + 1..=floor.max(height) + CLEAR_ABOVE {
            self.block(IVec3::new(at.x, y, at.y), None);
        }
    }

    /// Builds `piece`, turned `turns` times, its door column at `anchor` and its floor at
    /// `floor`: a foundation down to the ground under it, the air cleared above, then its cells,
    /// what its containers hold, and the animals of a pen.
    fn build(&mut self, piece: &Piece, anchor: IVec2, turns: usize, floor: i32) {
        let style = self.site.style;
        let (width, depth, height) = piece.size();
        let columns = Self::columns_of(piece, anchor, turns);
        let foundation = material(style, Role::Foundation);
        for &cell in &columns {
            self.taken.insert(cell);
            self.built.insert(cell);
            self.keep_clear(cell, 2);
            let ground = self.land.height(cell);
            for y in ground + 1..floor {
                self.block(
                    IVec3::new(cell.x, y, cell.y),
                    Some(BlockData::new(foundation, BlockDirection::Front)),
                );
            }
            for y in floor + 1..=(floor + height - 1).max(ground) + CLEAR_ABOVE {
                self.block(IVec3::new(cell.x, y, cell.y), None);
            }
        }
        for ((x, y, z), cell) in piece.cells() {
            let Cell::Block(role, direction) = cell else {
                continue;
            };
            let column = anchor + rotate(IVec2::new(x - piece.door, z), turns);
            let pos = IVec3::new(column.x, floor + y, column.y);
            let id = material(style, role);
            self.block(
                pos,
                Some(BlockData::new(id, rotate_direction(direction, turns))),
            );
            if id.is_container() {
                self.containers
                    .push((pos, loot::container(self.seed, pos, piece.kind)));
            }
        }
        self.pen_animals(piece, anchor, turns, floor, style);
        let min = columns.iter().copied().reduce(IVec2::min).unwrap();
        let max = columns.iter().copied().reduce(IVec2::max).unwrap();
        debug_assert_eq!(
            (max - min + IVec2::ONE).element_product(),
            width * depth,
            "{:?}",
            piece.kind
        );
        self.pieces.push(PlacedPiece {
            kind: piece.kind,
            min,
            max,
            floor,
        });
    }

    /// The animals of a pen: two to four, each on its own column inside the fence.
    fn pen_animals(
        &mut self,
        piece: &Piece,
        anchor: IVec2,
        turns: usize,
        floor: i32,
        style: VillageStyle,
    ) {
        let kinds = piece.animals(style);
        if kinds.is_empty() {
            return;
        }
        let (width, depth, _) = piece.size();
        let kind = kinds[self.draws.below(kinds.len() as u64) as usize];
        let count = 2 + self.draws.below(3);
        let mut taken = Vec::new();
        for _ in 0..count * 3 {
            if taken.len() == count as usize {
                break;
            }
            let x = 1 + self.draws.below((width - 2) as u64);
            let z = 1 + self.draws.below((depth - 2) as u64);
            let rotation = self.draws.unit();
            let ticks = self.draws.below(200);
            if taken.contains(&(x, z)) {
                continue;
            }
            taken.push((x, z));
            let column = anchor + rotate(IVec2::new(x - piece.door, z), turns);
            let position = ServerMob::standing_at(
                kind,
                column.x as f32 + 0.5,
                (floor + 1) as f32,
                column.y as f32 + 0.5,
            );
            self.animals.push(ServerMob {
                rotation: Quat::from_rotation_y(rotation * std::f32::consts::TAU),
                action_ticks: 1 + ticks as u32,
                ..ServerMob::new(kind, position)
            });
        }
    }

    fn finish(self) -> VillagePlan {
        let min = self.footprint.iter().copied().reduce(IVec2::min).unwrap();
        let max = self.footprint.iter().copied().reduce(IVec2::max).unwrap();
        let top = self
            .blocks
            .iter()
            .filter(|(_, block)| block.is_some())
            .map(|(pos, _)| pos.y)
            .max()
            .unwrap();
        let mut blocks: HashMap<IVec2, Vec<(IVec3, Option<BlockData>)>> = HashMap::new();
        for (pos, block) in self.blocks {
            let chunk_column = IVec2::new(pos.x, pos.z).div_euclid(IVec2::splat(CHUNK_SIZE));
            blocks.entry(chunk_column).or_default().push((pos, block));
        }
        VillagePlan {
            centre: self.site.centre,
            style: self.site.style,
            min,
            max,
            top,
            blocks,
            footprint: self.footprint,
            animals: self.animals,
            containers: self.containers,
            pieces: self.pieces,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_turn_moves_the_front_to_the_right() {
        for turns in 0..4 {
            let direction = BLOCK_DIRECTIONS[turns];
            let step = direction.offset();
            assert_eq!(IVec2::new(step.x, step.z), DIRECTIONS[turns]);
            assert_eq!(rotate(IVec2::NEG_Y, turns), DIRECTIONS[turns]);
            assert_eq!(rotate_direction(BlockDirection::Front, turns), direction);
        }
    }
}
