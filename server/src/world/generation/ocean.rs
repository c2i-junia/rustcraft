//! The life of the seas, for the chunks generated since it came: seagrass and kelp on the sea
//! floor, reefs of three colours in patches of the warm shallows, shoals of fish, and the wrecks
//! of ships on the bottom, each with a chest that holds a treasure map to a chest buried nearby.
//!
//! Like the rest of the generation, everything is decided from the seed and the place: a column
//! decides its plants and its reef, a column of chunks its shoal, and a region of `WRECK_REGION`
//! blocks its wreck and where its treasure lies.

use std::collections::HashMap;
use std::f32::consts::TAU;
use std::sync::{Arc, Mutex};

use bevy::prelude::*;
use shared::world::{
    structure_loot, tags, BlockData, BlockDirection, BlockId, Chunk, Container, ItemId, ItemStack,
    ItemType, MobKind, ServerMob, TerrainNoises, CHEST_SLOTS, SEA_LEVEL,
};
use shared::CHUNK_SIZE;

use super::super::caves::Noise3;
use super::structure::{fill_container, rotate, StructureBlocks, BLOCK_DIRECTIONS};
use super::{ChunkColumns, Column, PositionRandom};

const SALT_REEFS: u64 = 0x4200;
const SALT_PLANT: u64 = 0x4201;
const SALT_KELP: u64 = 0x4202;
const SALT_REEF_HEIGHT: u64 = 0x4203;
const SALT_REEF_COLOUR: u64 = 0x4204;
const SALT_SHOAL: u64 = 0x4210;
const SALT_WRECK: u64 = 0x4220;
const SALT_DECAY: u64 = 0x4230;
const SALT_TREASURE: u64 = 0x4240;

/// The reefs grow where their noise, of this wavelength, exceeds `REEF_FROM`, in water from
/// `REEF_DEPTH` deep.
const REEF_WAVELENGTH: f64 = 40.0;
const REEF_FROM: f64 = 0.42;
const REEF_DEPTH: std::ops::RangeInclusive<i32> = 4..=22;
/// The share of the columns of the sea floor that grow kelp, from `KELP_DEPTH` deep, and
/// seagrass.
const KELP_CHANCE: f32 = 0.06;
const KELP_DEPTH: i32 = 6;
const SEAGRASS_CHANCE: f32 = 0.2;

/// The side of the regions that hold a wreck each at most, in blocks, and the chance they do.
pub const WRECK_REGION: i32 = 320;
const WRECK_CHANCE: f32 = 0.7;
/// A wreck lies at least this far inside its region.
const WRECK_MARGIN: i32 = 24;
/// The depth of water a wreck lies in.
const WRECK_DEPTH: std::ops::RangeInclusive<i32> = 9..=40;
/// How far from its wreck the treasure of its map is buried, in blocks.
const TREASURE_DISTANCE: std::ops::Range<i32> = 24..72;

/// The noises and the wrecks of the seas of a world.
pub struct Seas {
    seed: u32,
    reefs: Noise3,
    wrecks: Mutex<HashMap<IVec2, Option<Arc<Wreck>>>>,
}

/// A wreck on the sea floor, and the treasure its map leads to.
pub struct Wreck {
    /// The chest in its hull.
    pub chest: IVec3,
    /// The chest buried on its map.
    pub treasure: IVec3,
    blocks: StructureBlocks,
}

fn is_ocean(column: &Column) -> bool {
    column.def().tags.iter().any(|tag| tag == tags::OCEAN)
}

impl Seas {
    pub fn new(seed: u32) -> Self {
        Self {
            seed,
            reefs: Noise3::new(seed, SALT_REEFS, REEF_WAVELENGTH, REEF_WAVELENGTH),
            wrecks: Mutex::default(),
        }
    }

    /// Grows the plants and the reef of `column` that fall in the chunk at `chunk_origin`, in its
    /// water.
    pub(super) fn decorate(&self, chunk: &mut Chunk, chunk_origin: IVec3, column: &Column) {
        if !is_ocean(column) || column.def().ground.frozen {
            return;
        }
        let depth = SEA_LEVEL - column.height;
        if depth < 2 {
            return;
        }
        let random = PositionRandom {
            seed: self.seed,
            coords: [column.x, column.z],
        };
        let mut set = |y: i32, id: BlockId| {
            let local = IVec3::new(column.x, y, column.z) - chunk_origin;
            if !Chunk::contains(local) {
                return;
            }
            let water = chunk
                .get(local)
                .is_some_and(|block| block.id == BlockId::Water);
            if water || y == column.height {
                chunk.set(local, BlockData::new(id, BlockDirection::Front));
            }
        };
        let floor = column.height;
        let reef = REEF_DEPTH.contains(&depth)
            && self.reefs.get(IVec3::new(column.x, 0, column.z)) > REEF_FROM;
        if reef {
            let colour = |y: i32| {
                let random = PositionRandom {
                    seed: self.seed,
                    coords: [column.x, y, column.z],
                };
                [BlockId::AzureReef, BlockId::RoseReef, BlockId::AmberReef]
                    [random.below(SALT_REEF_COLOUR, 3) as usize]
            };
            let height = random.below(SALT_REEF_HEIGHT, 4).min(depth - 3);
            for y in floor..=floor + height {
                set(y, colour(y));
            }
            if random.unit(SALT_PLANT) < SEAGRASS_CHANCE {
                set(floor + height + 1, BlockId::Seagrass);
            }
            return;
        }
        let soil = matches!(
            column.surface,
            BlockId::Sand | BlockId::Gravel | BlockId::Clay | BlockId::Dirt
        );
        let roll = random.unit(SALT_PLANT);
        if depth >= KELP_DEPTH && roll < KELP_CHANCE {
            // a column of kelp, its top a block under the surface at least
            let length = 2 + random.below(SALT_KELP, (depth - 3) as u64);
            for y in floor + 1..=(floor + length).min(SEA_LEVEL - 1) {
                set(y, BlockId::Kelp);
            }
        } else if soil && roll < KELP_CHANCE + SEAGRASS_CHANCE {
            set(floor + 1, BlockId::Seagrass);
        }
    }

    /// The fish born with the chunk at `chunk_pos`: a shoal of one kind in a column of chunks
    /// of the sea now and then, in the chunk that holds the middle of its water.
    pub(super) fn chunk_fish(&self, chunk_pos: IVec3, columns: &ChunkColumns) -> Vec<ServerMob> {
        let random = PositionRandom {
            seed: self.seed,
            coords: [chunk_pos.x, chunk_pos.z],
        };
        if random.unit(SALT_SHOAL) >= 0.3 {
            return Vec::new();
        }
        let (cx, cz) = (
            random.below(SALT_SHOAL + 1, CHUNK_SIZE as u64),
            random.below(SALT_SHOAL + 2, CHUNK_SIZE as u64),
        );
        let centre = columns.at(cx, cz);
        let depth = SEA_LEVEL - centre.height;
        if !is_ocean(centre) || depth < 4 {
            return Vec::new();
        }
        let y = centre.height + 1 + random.below(SALT_SHOAL + 3, (depth - 2) as u64);
        if y.div_euclid(CHUNK_SIZE) != chunk_pos.y {
            return Vec::new();
        }
        // Glintfins on the reefs and the warm shallows, Shoalbacks in the open sea
        let reef = self.reefs.get(IVec3::new(centre.x, 0, centre.z)) > REEF_FROM - 0.1;
        let kind = if reef || depth < 10 {
            MobKind::Glintfin
        } else {
            MobKind::Shoalback
        };
        let size = 3 + random.below(SALT_SHOAL + 4, 4);
        (0..size as u64)
            .filter_map(|i| {
                let salt = SALT_SHOAL + 16 + 4 * i;
                let offset = Vec3::new(
                    random.unit(salt) * 3.0 - 1.5,
                    random.unit(salt + 1) - 0.5,
                    random.unit(salt + 2) * 3.0 - 1.5,
                );
                let position =
                    Vec3::new(centre.x as f32 + 0.5, y as f32 + 0.5, centre.z as f32 + 0.5)
                        + offset;
                let column = columns.at(
                    (position.x.floor() as i32 - chunk_pos.x * CHUNK_SIZE).clamp(0, CHUNK_SIZE - 1),
                    (position.z.floor() as i32 - chunk_pos.z * CHUNK_SIZE).clamp(0, CHUNK_SIZE - 1),
                );
                // in the water of its column, under the surface
                (position.y > column.height as f32 + 1.2 && position.y < SEA_LEVEL as f32 - 0.2)
                    .then(|| ServerMob {
                        rotation: Quat::from_rotation_y(random.unit(salt + 3) * TAU),
                        action_ticks: 1 + i as u32 * 7,
                        ..ServerMob::new(kind, position)
                    })
            })
            .collect()
    }

    /// The wreck of `region`, if it holds one, from the cache or laid out.
    pub fn wreck(&self, terrain: &TerrainNoises, region: IVec2) -> Option<Arc<Wreck>> {
        if let Some(wreck) = self.wrecks.lock().unwrap().get(&region) {
            return wreck.clone();
        }
        let wreck = layout(self.seed, terrain, region).map(Arc::new);
        self.wrecks.lock().unwrap().insert(region, wreck.clone());
        wreck
    }

    /// The wrecks whose blocks, or whose treasure, may fall in the column of chunks
    /// `chunk_column`: those of its region and of the regions around.
    pub(super) fn wrecks_near(
        &self,
        terrain: &TerrainNoises,
        chunk_column: IVec2,
    ) -> Vec<Arc<Wreck>> {
        let region = region_of(chunk_column * CHUNK_SIZE);
        let mut wrecks = Vec::new();
        for dx in -1..=1 {
            for dz in -1..=1 {
                wrecks.extend(self.wreck(terrain, region + IVec2::new(dx, dz)));
            }
        }
        wrecks
    }
}

/// The region of wrecks the column `(x, z)` lies in.
pub fn region_of(column: IVec2) -> IVec2 {
    column.div_euclid(IVec2::splat(WRECK_REGION))
}

/// Where the wreck of `region` lies, its hull turned `turns` quarter turns, if it holds one: on
/// a sea floor `WRECK_DEPTH` deep.
pub fn site(seed: u32, terrain: &TerrainNoises, region: IVec2) -> Option<(IVec3, usize)> {
    let random = PositionRandom {
        seed,
        coords: [region.x, region.y],
    };
    if random.unit(SALT_WRECK) >= WRECK_CHANCE {
        return None;
    }
    let span = (WRECK_REGION - 2 * WRECK_MARGIN) as u64;
    let column = region * WRECK_REGION
        + IVec2::new(
            WRECK_MARGIN + random.below(SALT_WRECK + 1, span),
            WRECK_MARGIN + random.below(SALT_WRECK + 2, span),
        );
    let shape = terrain.column_shape(column.x, column.y);
    let ocean = terrain
        .biomes()
        .get(shape.biome)
        .tags
        .iter()
        .any(|tag| tag == tags::OCEAN);
    if !ocean || !WRECK_DEPTH.contains(&(SEA_LEVEL - shape.height)) {
        return None;
    }
    let turns = random.below(SALT_WRECK + 3, 4) as usize;
    // On the lowest of its ends and its middle, sunk into the higher ones
    let lowest = [IVec2::ZERO, IVec2::new(0, 5), IVec2::new(0, -5)]
        .into_iter()
        .map(|end| {
            let at = column + rotate(end, turns);
            terrain.column_shape(at.x, at.y).height
        })
        .min()?;
    Some((IVec3::new(column.x, lowest, column.y), turns))
}

/// The hull of a wreck, from its middle at `origin` (its keel), along the local z: each cell
/// `(x, y, z)` from -2 to 2, 0 to 3, -5 to 5, and whether it is planks; the cells inside it are
/// water.
fn hull_cell(x: i32, y: i32, z: i32) -> Option<bool> {
    let (ax, az) = (x.abs(), z.abs());
    let outline = |ax: i32, az: i32| match az {
        0..=3 => ax <= 2,
        4 => ax <= 1,
        5 => ax == 0,
        _ => false,
    };
    if !outline(ax, az) {
        return None;
    }
    let edge = !outline(ax + 1, az) || !outline(ax, az + 1);
    let stern_deck = (1..=3).contains(&z) && y == 3;
    match y {
        0 => Some(true),
        1 | 2 => Some(edge),
        3 => Some(edge && z >= 0 || stern_deck),
        _ => None,
    }
}

/// The wreck of `region`, laid out, if it holds one.
fn layout(seed: u32, terrain: &TerrainNoises, region: IVec2) -> Option<Wreck> {
    let (origin, turns) = site(seed, terrain, region)?;
    let random = PositionRandom {
        seed,
        coords: [origin.x, origin.y, origin.z],
    };
    let planks = if random.unit(SALT_WRECK + 4) < 0.5 {
        BlockId::OakPlanks
    } else {
        BlockId::SprucePlanks
    };
    let at = |x: i32, y: i32, z: i32| {
        let flat = rotate(IVec2::new(x, z), turns);
        origin + IVec3::new(flat.x, y, flat.y)
    };
    let block = |id| Some(BlockData::new(id, BlockDirection::Front));
    let mut blocks = Vec::new();
    for x in -2..=2 {
        for z in -5..=5 {
            for y in 0..=3 {
                let Some(wall) = hull_cell(x, y, z) else {
                    continue;
                };
                let pos = at(x, y, z);
                let decay = PositionRandom {
                    seed,
                    coords: pos.to_array(),
                };
                // The rot opens holes in its sides and deck, more toward the bow
                let rotten = y > 0 && decay.unit(SALT_DECAY) < if z < -2 { 0.35 } else { 0.15 };
                if wall && !rotten {
                    blocks.push((pos, block(planks)));
                } else if pos.y < SEA_LEVEL {
                    blocks.push((pos, block(BlockId::Water)));
                }
            }
        }
    }
    // Its broken mast, standing under the surface
    let mast = (1 + random.below(SALT_WRECK + 5, 5)).min(SEA_LEVEL - 2 - origin.y);
    for y in 1..=mast {
        blocks.push((at(0, y, -1), block(BlockId::SpruceLog)));
    }
    // The chest, under the deck of the stern
    let chest = at(0, 1, 2);
    let facing = BLOCK_DIRECTIONS[(2 + turns) % 4];
    blocks.push((chest, Some(BlockData::new(BlockId::Chest, facing))));

    let treasure = treasure_of(seed, terrain, origin);
    blocks.push((treasure, block(BlockId::Chest)));

    let mut loot = fill_container(seed, chest, structure_loot::SHIPWRECK);
    let start = random.below(SALT_WRECK + 6, CHEST_SLOTS as u64) as usize;
    let slots = CHEST_SLOTS as usize;
    if let Some(slot) = (0..slots)
        .map(|k| (start + k) % slots)
        .find(|&slot| loot.slots[slot].is_none())
    {
        loot.slots[slot] = Some(ItemStack {
            item_id: ItemId::TreasureMap,
            item_type: ItemType::Treasure { at: treasure },
            nb: 1,
        });
    }
    let containers = vec![
        (chest, loot),
        (
            treasure,
            fill_container(seed, treasure, structure_loot::BURIED_TREASURE),
        ),
    ];
    Some(Wreck {
        chest,
        treasure,
        blocks: StructureBlocks::new(blocks, containers),
    })
}

/// Where the treasure of the wreck at `origin` is buried: under two blocks of the ground of a
/// column `TREASURE_DISTANCE` away, on a shore or in shallow water if one of a few tried is.
fn treasure_of(seed: u32, terrain: &TerrainNoises, origin: IVec3) -> IVec3 {
    let random = PositionRandom {
        seed,
        coords: [origin.x, origin.z],
    };
    let mut found = None;
    for attempt in 0..12u64 {
        let salt = SALT_TREASURE + 4 * attempt;
        let angle = random.unit(salt) * TAU;
        let distance = (TREASURE_DISTANCE.start
            + random.below(
                salt + 1,
                (TREASURE_DISTANCE.end - TREASURE_DISTANCE.start) as u64,
            )) as f32;
        let column = origin.xz() + (Vec2::from_angle(angle) * distance).as_ivec2();
        let height = terrain.column_shape(column.x, column.y).height;
        let at = IVec3::new(column.x, height - 2, column.y);
        found = Some(at);
        if (SEA_LEVEL - 3..=SEA_LEVEL + 6).contains(&height) {
            break;
        }
    }
    found.unwrap_or(origin)
}

impl Wreck {
    /// Places the blocks of the wreck and of its treasure that fall in the chunk at `chunk_pos`.
    pub fn place_blocks(&self, chunk: &mut Chunk, chunk_pos: IVec3) {
        self.blocks.place(chunk, chunk_pos);
    }

    /// What its chests in the chunk at `chunk_pos` hold, by their position in the chunk.
    pub fn chunk_containers(
        &self,
        chunk_pos: IVec3,
    ) -> impl Iterator<Item = (IVec3, Container)> + '_ {
        self.blocks.chunk_containers(chunk_pos)
    }
}

#[cfg(test)]
mod tests {
    use super::super::WorldGenerator;
    use super::*;

    fn wrecks(generator: &WorldGenerator) -> Vec<Arc<Wreck>> {
        (-4..4)
            .flat_map(|x| (-4..4).map(move |z| IVec2::new(x, z)))
            .filter_map(|region| generator.seas.wreck(generator.terrain(), region))
            .collect()
    }

    #[test]
    fn the_wrecks_and_their_treasure_depend_only_on_the_seed() {
        let generator = WorldGenerator::new(3);
        let found = wrecks(&generator);
        assert!(!found.is_empty(), "no wreck in 64 regions");
        let again = wrecks(&WorldGenerator::new(3));
        let places = |w: &[Arc<Wreck>]| -> Vec<(IVec3, IVec3)> {
            w.iter().map(|w| (w.chest, w.treasure)).collect()
        };
        assert_eq!(places(&found), places(&again));
        for wreck in &found {
            assert!(wreck.chest.y < SEA_LEVEL - 4, "{}", wreck.chest);
            let distance = (wreck.treasure.xz() - wreck.chest.xz()).as_vec2().length();
            assert!(distance < TREASURE_DISTANCE.end as f32 + 8.0, "{distance}");
        }
    }

    #[test]
    fn a_wreck_chest_holds_a_map_to_its_buried_treasure() {
        let generator = WorldGenerator::new(3);
        let wreck = wrecks(&generator).into_iter().next().unwrap();
        let chunk_pos = wreck.chest.div_euclid(IVec3::splat(CHUNK_SIZE));
        let chunk = generator.generate_chunk(chunk_pos);
        let local = wreck.chest - chunk_pos * CHUNK_SIZE;
        assert_eq!(chunk.get(local).map(|b| b.id), Some(BlockId::Chest));
        let contents = generator.chunk_containers(chunk_pos);
        let (_, chest) = contents.iter().find(|(at, _)| *at == local).unwrap();
        let map = chest
            .slots
            .iter()
            .flatten()
            .find(|s| s.item_id == ItemId::TreasureMap);
        assert_eq!(
            map.map(|stack| stack.item_type),
            Some(ItemType::Treasure { at: wreck.treasure })
        );
        let buried = wreck.treasure.div_euclid(IVec3::splat(CHUNK_SIZE));
        let chunk = generator.generate_chunk(buried);
        let local = wreck.treasure - buried * CHUNK_SIZE;
        assert_eq!(chunk.get(local).map(|b| b.id), Some(BlockId::Chest));
    }

    #[test]
    fn a_hull_has_a_floor_and_sides_and_water_inside() {
        assert_eq!(hull_cell(0, 0, 0), Some(true));
        assert_eq!(hull_cell(2, 1, 0), Some(true));
        assert_eq!(hull_cell(0, 1, 0), Some(false));
        assert_eq!(hull_cell(0, 1, 5), Some(true));
        assert_eq!(hull_cell(3, 1, 0), None);
        assert_eq!(hull_cell(0, 4, 0), None);
    }
}
