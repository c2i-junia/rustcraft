//! Dungeons: a ruin with its cellar on the surface, a crypt off the caves, a sanctuary by the
//! lava of the bottom of the world.
//!
//! As the villages, the world is cut into regions, of `REGION` blocks, each holding at most one
//! dungeon of each kind: its place is drawn from the seed and the region and read from the
//! noises alone (`placement`), its plan is laid out whole from layers of characters
//! (`blueprints`), and each chunk it touches places the blocks that fall in it, after the ores.
//! Its hostiles are not born with the chunks, which are loaded well beyond where a hostile
//! despawns: they appear at its `guards` while players are near (`mob::guards`).
//!
//! The lair of a boss, unique, is to come at the bottom of the world: `placement::lair_region`
//! is kept clear of sanctuaries for it.

use std::collections::HashSet;

use bevy::prelude::*;
use shared::world::{BlockData, BlockDirection, BlockId, Chunk, Container, MobKind, TerrainNoises};

use super::structure::{fill_container, rotate, rotate_direction, StructureBlocks};
use super::PositionRandom;
use crate::world::caves::Caves;

mod blueprints;
mod loot;
mod placement;

use blueprints::{Blueprint, Cell};
pub use placement::{region_of, site, REGION};

/// The kinds of dungeons.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DungeonKind {
    Ruin,
    Crypt,
    Sanctuary,
}

impl DungeonKind {
    pub const ALL: [DungeonKind; 3] = [Self::Ruin, Self::Crypt, Self::Sanctuary];
    /// Those under the ground, which change no column of the surface.
    pub const UNDERGROUND: [DungeonKind; 2] = [Self::Crypt, Self::Sanctuary];

    /// Its name, as `/locate` takes it.
    pub fn name(self) -> &'static str {
        match self {
            Self::Ruin => "ruin",
            Self::Crypt => "crypt",
            Self::Sanctuary => "sanctuary",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.name() == name)
    }

    /// Whether a chunk whose lowest layer is at `y` may hold a block of such a dungeon.
    pub fn heights_meet(self, y: i32) -> bool {
        let heights = placement::heights(self);
        y <= *heights.end() && y + shared::CHUNK_SIZE > *heights.start()
    }
}

/// What the place of a dungeon is read from.
pub struct World<'a> {
    pub seed: u32,
    pub terrain: &'a TerrainNoises,
    pub caves: &'a Caves,
    /// Whether the world has villages, which the ruins keep away from.
    pub villages: bool,
}

/// Where a dungeon stands, before it is laid out.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Site {
    /// The floor under its door.
    pub door: IVec3,
    /// The quarter turns from a door facing `-Z`, front toward right.
    pub turns: usize,
}

/// A dungeon as it is built.
pub struct DungeonPlan {
    /// The corners of its cells, inclusive.
    pub min: IVec3,
    pub max: IVec3,
    /// Where its hostiles appear: the cell of their feet, and their kind.
    pub guards: Vec<(IVec3, MobKind)>,
    blocks: StructureBlocks,
    /// The columns where nothing grows.
    footprint: HashSet<IVec2>,
}

impl DungeonPlan {
    /// Whether nothing grows on the column `(x, z)`.
    pub fn covers(&self, x: i32, z: i32) -> bool {
        self.footprint.contains(&IVec2::new(x, z))
    }

    /// Whether the dungeon has cells between `min` and `max`.
    pub fn touches(&self, min: IVec3, max: IVec3) -> bool {
        self.min.cmple(max).all() && self.max.cmpge(min).all()
    }

    /// Places its blocks that fall in the chunk at `chunk_pos`.
    pub fn place_blocks(&self, chunk: &mut Chunk, chunk_pos: IVec3) {
        self.blocks.place(chunk, chunk_pos);
    }

    /// What its containers in the chunk at `chunk_pos` hold, by their position in the chunk.
    pub fn chunk_containers(
        &self,
        chunk_pos: IVec3,
    ) -> impl Iterator<Item = (IVec3, Container)> + '_ {
        self.blocks.chunk_containers(chunk_pos)
    }
}

/// The dungeon of `kind` in `region`, if it holds one.
pub fn plan(world: &World, kind: DungeonKind, region: IVec2) -> Option<DungeonPlan> {
    site(world, kind, region).map(|site| layout(world, kind, site))
}

/// Air is kept this high above the floor of a ruin, and its foundations go down this far.
const RUIN_CLEAR: i32 = 8;
const RUIN_FOUNDATION: i32 = 6;

/// The salts of the draws of the cells.
const SALT_STONE: u64 = 0x3000;
const SALT_DECAY: u64 = 0x3001;
const SALT_GROUND: u64 = 0x3002;

fn layout(world: &World, kind: DungeonKind, site: Site) -> DungeonPlan {
    let blueprint = Blueprint::of(kind);
    let seed = world.seed;
    let column_of =
        |x: i32, z: i32| site.door.xz() + rotate(IVec2::new(x - blueprint.door, z), site.turns);
    let block = |id| Some(BlockData::new(id, BlockDirection::Front));
    let (width, depth) = blueprint.size();
    let mut blocks = Vec::new();
    let mut footprint = HashSet::new();
    let corners = [column_of(0, 0), column_of(width - 1, depth - 1)];
    let (low, high) = (corners[0].min(corners[1]), corners[0].max(corners[1]));
    let mut bottom = site.door.y + blueprint.lowest;
    let mut top = bottom + blueprint.layers.len() as i32 - 1;

    if kind == DungeonKind::Ruin {
        for x in low.x..=high.x {
            for z in low.y..=high.y {
                footprint.insert(IVec2::new(x, z));
                for y in site.door.y + 1..=site.door.y + RUIN_CLEAR {
                    blocks.push((IVec3::new(x, y, z), None));
                }
                // down to the ground where it lies lower
                let ground = world.terrain.column_shape(x, z).height;
                for y in (ground + 1).max(site.door.y - RUIN_FOUNDATION)..=site.door.y {
                    blocks.push((IVec3::new(x, y, z), block(BlockId::Cobblestone)));
                }
            }
        }
        bottom = bottom.min(site.door.y - RUIN_FOUNDATION);
        top = top.max(site.door.y + RUIN_CLEAR);
    }

    let mut containers = Vec::new();
    let mut guards = Vec::new();
    for (local, cell) in blueprint.cells() {
        let column = column_of(local.x, local.z);
        let pos = IVec3::new(column.x, site.door.y + blueprint.lowest + local.y, column.y);
        let random = PositionRandom {
            seed,
            coords: [pos.x, pos.y, pos.z],
        };
        // how much of the walls and pillars of a ruin still stands, the same up a column
        let stands = |above: i32| {
            let column = PositionRandom {
                seed,
                coords: [pos.x, pos.z],
            };
            let near = PositionRandom {
                seed,
                coords: [pos.x.div_euclid(3), pos.z.div_euclid(3)],
            };
            above <= 1 + near.below(SALT_DECAY, 4) + column.below(SALT_DECAY, 2)
        };
        let placed = match cell {
            Cell::Leave => continue,
            Cell::Air => None,
            Cell::Guard(mob) => {
                guards.push((pos, mob));
                None
            }
            Cell::Masonry => block(masonry(&random)),
            Cell::Ruined => {
                if !stands(pos.y - site.door.y) {
                    continue;
                }
                block(masonry(&random))
            }
            Cell::Paving => match random.below(SALT_GROUND, 5) {
                0 => block(BlockId::Gravel),
                // the ground shows through
                1 | 2 => continue,
                _ => block(masonry(&random)),
            },
            Cell::Block(id, direction) => {
                Some(BlockData::new(id, rotate_direction(direction, site.turns)))
            }
        };
        if placed.is_some_and(|block| block.id.is_container()) {
            containers.push((pos, fill_container(seed, pos, loot::table(kind))));
        }
        blocks.push((pos, placed));
    }
    DungeonPlan {
        min: IVec3::new(low.x, bottom, low.y),
        max: IVec3::new(high.x, top, high.y),
        guards,
        blocks: StructureBlocks::new(blocks, containers),
        footprint,
    }
}

/// The stone of a wall, worn unevenly.
fn masonry(random: &PositionRandom<3>) -> BlockId {
    match random.below(SALT_STONE, 10) {
        0..=3 => BlockId::Cobblestone,
        4 => BlockId::Slate,
        _ => BlockId::HewnStone,
    }
}

#[cfg(test)]
mod tests {
    use super::super::WorldGenerator;
    use super::*;
    use shared::CHUNK_SIZE;
    use std::collections::HashMap;

    /// The plans of `kind` in the regions around the origin of the world of `generator`.
    fn plans(generator: &WorldGenerator, kind: DungeonKind) -> Vec<std::sync::Arc<DungeonPlan>> {
        (-6..6)
            .flat_map(|x| (-6..6).map(move |z| IVec2::new(x, z)))
            .filter_map(|region| generator.dungeon(kind, region))
            .collect()
    }

    #[test]
    fn a_dungeon_is_built_whole_whatever_the_order_of_its_chunks() {
        for kind in DungeonKind::ALL {
            let generator = WorldGenerator::new(42);
            let found = plans(&generator, kind);
            assert!(found.len() >= 10, "{kind:?}: {} in 144", found.len());
            let plan = &found[0];
            let side = IVec3::splat(CHUNK_SIZE);
            let (first, last) = (plan.min.div_euclid(side), plan.max.div_euclid(side));
            let mut positions = Vec::new();
            for x in first.x..=last.x {
                for y in first.y..=last.y {
                    for z in first.z..=last.z {
                        positions.push(IVec3::new(x, y, z));
                    }
                }
            }
            let backwards = WorldGenerator::new(42);
            let made: HashMap<IVec3, Chunk> = positions
                .iter()
                .rev()
                .map(|&pos| (pos, backwards.generate_chunk(pos)))
                .collect();
            for (pos, chunk) in &made {
                assert!(generator.generate_chunk(*pos) == *chunk, "{kind:?} {pos}");
            }
            // the last block of a cell wins
            let mut wanted = HashMap::new();
            wanted.extend(plan.blocks.all());
            for (pos, block) in wanted {
                let chunk = &made[&pos.div_euclid(side)];
                assert_eq!(chunk.get(pos.rem_euclid(side)), block.as_ref(), "{pos}");
            }
            // its guards have a floor, and room
            assert!(!plan.guards.is_empty());
            for (feet, _) in &plan.guards {
                let at = |pos: IVec3| made[&pos.div_euclid(side)].get(pos.rem_euclid(side));
                assert!(at(*feet - IVec3::Y).is_some(), "{kind:?} {feet}");
                assert!(at(*feet).is_none() && at(*feet + IVec3::Y).is_none());
            }
            // its containers are filled, the same ones again
            let mut filled = 0;
            for &pos in &positions {
                let contents = generator.chunk_containers(pos);
                assert_eq!(contents, backwards.chunk_containers(pos));
                for (local, container) in contents {
                    assert!(made[&pos].get(local).unwrap().id.is_container());
                    filled += usize::from(!container.is_empty());
                }
            }
            assert!(filled >= 2, "{kind:?}: {filled} containers hold something");
        }
    }

    #[test]
    fn each_kind_lies_at_its_depth_and_a_world_saved_before_has_none() {
        let generator = WorldGenerator::new(7);
        for kind in DungeonKind::ALL {
            let heights = placement::heights(kind);
            for plan in plans(&generator, kind) {
                assert!(heights.contains(&plan.min.y), "{kind:?} {}", plan.min);
                assert!(heights.contains(&plan.max.y), "{kind:?} {}", plan.max);
                let inside = IVec2::splat(super::super::STRUCTURE_MARGIN);
                assert_eq!(
                    region_of(plan.min.x - inside.x, plan.min.z - inside.y),
                    region_of(plan.max.x + inside.x, plan.max.z + inside.y)
                );
            }
        }
        let settings = shared::world::WorldGenSettings::legacy();
        let before = WorldGenerator::with(7, &settings, shared::world::Biomes::vanilla());
        for kind in DungeonKind::ALL {
            assert!(plans(&before, kind).is_empty());
        }
    }
}
