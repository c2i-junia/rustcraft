use std::collections::HashMap;

use super::{GameElementId, ItemId};
use bevy::math::{bounding::Aabb3d, Vec3A};
use rand::Rng;
use serde::{Deserialize, Serialize};

/// Append only: never reorder, insert before the end or remove a variant. The variant's position
/// is what chunk files and network messages store (bincode), so any other change turns the
/// blocks of every existing world into other blocks. Bump `PROTOCOL_ID` and, if the saved
/// encoding changes, `FORMAT_VERSION`. The test `block_encoding_is_pinned` guards this.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    serde::Serialize,
    serde::Deserialize,
    Hash,
    Default,
)]
pub enum BlockId {
    #[default]
    Dirt,
    Debug,
    Grass,
    Stone,
    OakLog,
    OakPlanks,
    OakLeaves,
    Sand,
    Cactus,
    Ice,
    Glass,
    Bedrock,
    Dandelion,
    Poppy,
    TallGrass,
    Cobblestone,
    Snow,
    SpruceLeaves,
    SpruceLog,
    Water,
    CraftingTable,
    SprucePlanks,
    Furnace,
}

/// Append only, for the same reason as `BlockId`: the position of the variant is what is stored
/// in chunk files and sent on the network.
#[derive(Debug, Copy, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum BlockDirection {
    Front,
    Right,
    Back,
    Left,
}

/// Data associated with a given `BlockId`
#[derive(Debug, Copy, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BlockData {
    pub id: BlockId,
    pub direction: BlockDirection,
    pub breaking_progress: u8,
    /// For a liquid, how far it is from its source (see `liquid`); 0 for any other block.
    /// The worlds saved in RON before it existed read it as 0.
    #[serde(default)]
    pub level: u8,
}

/// Last breaking stage: there are ten crack textures, `DestroyStage0` to `DestroyStage9`.
pub const MAX_BREAKING_LEVEL: u8 = 9;

impl BlockData {
    pub fn new(id: BlockId, direction: BlockDirection) -> Self {
        BlockData {
            id,
            direction,
            breaking_progress: 0,
            level: 0,
        }
    }

    /// Breaking stage from 0 to `MAX_BREAKING_LEVEL` (9, the last crack texture); always 0 for
    /// an unbreakable block. A progress at or beyond the break time, which a chunk file or a
    /// server can carry, gives the last stage.
    pub fn get_breaking_level(&self) -> u8 {
        match self.id.get_break_time() {
            Some(break_time) => (((self.breaking_progress as u16 * 10) / break_time as u16) as u8)
                .min(MAX_BREAKING_LEVEL),
            None => 0,
        }
    }
}

#[derive(PartialEq, Eq, Debug)]
pub enum BlockTransparency {
    Transparent,
    Liquid,
    Solid,
    Decoration,
}

pub enum BlockHitbox {
    FullBlock,
    Aabb(Aabb3d),
    None,
}

impl BlockId {
    pub fn get_hitbox(&self) -> BlockHitbox {
        match *self {
            Self::Water | Self::TallGrass | Self::Poppy | Self::Dandelion => BlockHitbox::None,
            _ => BlockHitbox::FullBlock,
        }
    }

    pub fn get_ray_hitbox(&self) -> BlockHitbox {
        match *self {
            Self::Water => BlockHitbox::None,
            Self::TallGrass | Self::Poppy | Self::Dandelion => BlockHitbox::Aabb(Aabb3d::new(
                Vec3A::splat(0.5).with_y(0.3),
                Vec3A::splat(0.3),
            )),
            _ => BlockHitbox::FullBlock,
        }
    }

    /// Number of mining steps needed to break the block by hand, `None` if it cannot be broken.
    pub fn get_break_time(&self) -> Option<u8> {
        let steps: u8 = match *self {
            Self::Dirt => 5,
            Self::Debug => 7,
            Self::Grass => 6,
            Self::Stone => 10,
            Self::OakLog => 10,
            Self::OakPlanks => 10,
            Self::OakLeaves => 2,
            Self::Sand => 5,
            Self::Cactus => 4,
            Self::Ice => 5,
            Self::Glass => 3,
            Self::Dandelion => 1,
            Self::Poppy => 1,
            Self::TallGrass => 1,
            Self::Cobblestone => 2,
            Self::Snow => 9,
            Self::SpruceLeaves => 2,
            Self::SpruceLog => 10,
            Self::CraftingTable => 10,
            Self::SprucePlanks => 10,
            Self::Furnace => 15,
            Self::Bedrock | Self::Water => return None,
        };
        Some(6 * steps)
    }

    /// What breaking this block `nb_drops` times gives. Each drop is picked at random in the
    /// drop table; a table of one entry, like every current one, always gives that entry, so
    /// the client's prediction and the server agree.
    pub fn get_drops(&self, nb_drops: u32) -> HashMap<ItemId, u32> {
        let table = self.get_drop_table();
        let total_chance: u32 = table.iter().map(|(chance, ..)| chance).sum();

        let mut drops = HashMap::new();
        if total_chance == 0 {
            return drops;
        }
        for _ in 0..nb_drops {
            let roll = rand::thread_rng().gen_range(0..total_chance);
            if let Some((item, nb)) = pick_drop(&table, roll) {
                *drops.entry(item).or_insert(0) += nb;
            }
        }
        drops
    }

    /// Specifies the drop table of a given block
    /// Drops are specified this way : `(relative_chance, corresponding_item, base_number)`
    pub fn get_drop_table(&self) -> Vec<(u32, ItemId, u32)> {
        match *self {
            BlockId::Dirt | BlockId::Grass => vec![(1, ItemId::Dirt, 1)],
            BlockId::Stone => vec![(1, ItemId::Cobblestone, 1)],
            BlockId::Sand => vec![(1, ItemId::Sand, 1)],
            BlockId::Cactus => vec![(1, ItemId::Cactus, 1)],
            BlockId::OakLog => vec![(1, ItemId::OakLog, 1)],
            BlockId::OakPlanks => vec![(1, ItemId::OakPlanks, 1)],
            BlockId::Ice => vec![(1, ItemId::Ice, 1)],
            BlockId::Dandelion => vec![(1, ItemId::Dandelion, 1)],
            BlockId::Poppy => vec![(1, ItemId::Poppy, 1)],
            BlockId::TallGrass => vec![(1, ItemId::TallGrass, 1)],
            BlockId::SpruceLog => vec![(1, ItemId::SpruceLog, 1)],
            BlockId::Snow => vec![(1, ItemId::Snowball, 4)],
            BlockId::CraftingTable => vec![(1, ItemId::CraftingTable, 1)],
            BlockId::SprucePlanks => vec![(1, ItemId::SprucePlanks, 1)],
            BlockId::Furnace => vec![(1, ItemId::Furnace, 1)],
            _ => vec![],
        }
    }

    /// Whether a block placed in the cell of this one takes its place.
    pub fn is_replaceable(&self) -> bool {
        matches!(*self, Self::Water)
    }

    pub fn get_visibility(&self) -> BlockTransparency {
        match *self {
            Self::Dandelion | Self::Poppy | Self::TallGrass => BlockTransparency::Decoration,
            Self::Glass | Self::OakLeaves | Self::SpruceLeaves => BlockTransparency::Transparent,
            Self::Water => BlockTransparency::Liquid,
            _ => BlockTransparency::Solid,
        }
    }
}

impl GameElementId for BlockId {}

/// The entry of a drop table that `roll` falls on, `roll` being below the sum of the chances:
/// each entry takes as many consecutive rolls as its relative chance.
fn pick_drop(table: &[(u32, ItemId, u32)], mut roll: u32) -> Option<(ItemId, u32)> {
    for &(chance, item, nb) in table {
        if roll < chance {
            return Some((item, nb));
        }
        roll -= chance;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::ItemType;

    const ALL_BLOCKS: [BlockId; 23] = [
        BlockId::Dirt,
        BlockId::Debug,
        BlockId::Grass,
        BlockId::Stone,
        BlockId::OakLog,
        BlockId::OakPlanks,
        BlockId::OakLeaves,
        BlockId::Sand,
        BlockId::Cactus,
        BlockId::Ice,
        BlockId::Glass,
        BlockId::Bedrock,
        BlockId::Dandelion,
        BlockId::Poppy,
        BlockId::TallGrass,
        BlockId::Cobblestone,
        BlockId::Snow,
        BlockId::SpruceLeaves,
        BlockId::SpruceLog,
        BlockId::Water,
        BlockId::CraftingTable,
        BlockId::SprucePlanks,
        BlockId::Furnace,
    ];

    /// Blocks that cannot be broken by hand.
    const UNBREAKABLE_BLOCKS: [BlockId; 2] = [BlockId::Bedrock, BlockId::Water];

    /// Fails to compile when a `BlockId` variant is added without updating `ALL_BLOCKS`.
    fn all_blocks_is_exhaustive(id: BlockId) {
        match id {
            BlockId::Dirt
            | BlockId::Debug
            | BlockId::Grass
            | BlockId::Stone
            | BlockId::OakLog
            | BlockId::OakPlanks
            | BlockId::OakLeaves
            | BlockId::Sand
            | BlockId::Cactus
            | BlockId::Ice
            | BlockId::Glass
            | BlockId::Bedrock
            | BlockId::Dandelion
            | BlockId::Poppy
            | BlockId::TallGrass
            | BlockId::Cobblestone
            | BlockId::Snow
            | BlockId::SpruceLeaves
            | BlockId::SpruceLog
            | BlockId::Water
            | BlockId::CraftingTable
            | BlockId::SprucePlanks
            | BlockId::Furnace => {}
        }
    }

    fn regular_blocks() -> impl Iterator<Item = BlockId> {
        ALL_BLOCKS
            .into_iter()
            .inspect(|id| all_blocks_is_exhaustive(*id))
            .filter(|id| !UNBREAKABLE_BLOCKS.contains(id))
    }

    #[test]
    fn get_break_time_of_regular_blocks() {
        assert_eq!(BlockId::Dirt.get_break_time(), Some(30));
        assert_eq!(BlockId::Stone.get_break_time(), Some(60));
        assert_eq!(BlockId::Dandelion.get_break_time(), Some(6));
        for id in regular_blocks() {
            assert!(id.get_break_time().is_some_and(|t| t > 0), "{id:?}");
        }
    }

    #[test]
    fn get_break_time_does_not_panic_for_any_block() {
        for id in ALL_BLOCKS {
            let _ = id.get_break_time();
        }
    }

    #[test]
    fn bedrock_and_water_are_unbreakable() {
        for id in UNBREAKABLE_BLOCKS {
            assert_eq!(id.get_break_time(), None, "{id:?}");
        }
    }

    const ENCODING_WARNING: &str = "the encoded value of an existing variant changed: this \
        corrupts the saved worlds and breaks the protocol. New variants go at the END of the \
        enum; then add them to this table (and bump PROTOCOL_ID).";

    fn encoded<T: serde::Serialize>(value: &T) -> Vec<u8> {
        use bincode::Options;
        bincode::options().serialize(value).unwrap()
    }

    #[test]
    fn block_encoding_is_pinned() {
        for id in ALL_BLOCKS {
            let expected: u8 = match id {
                BlockId::Dirt => 0,
                BlockId::Debug => 1,
                BlockId::Grass => 2,
                BlockId::Stone => 3,
                BlockId::OakLog => 4,
                BlockId::OakPlanks => 5,
                BlockId::OakLeaves => 6,
                BlockId::Sand => 7,
                BlockId::Cactus => 8,
                BlockId::Ice => 9,
                BlockId::Glass => 10,
                BlockId::Bedrock => 11,
                BlockId::Dandelion => 12,
                BlockId::Poppy => 13,
                BlockId::TallGrass => 14,
                BlockId::Cobblestone => 15,
                BlockId::Snow => 16,
                BlockId::SpruceLeaves => 17,
                BlockId::SpruceLog => 18,
                BlockId::Water => 19,
                BlockId::CraftingTable => 20,
                BlockId::SprucePlanks => 21,
                BlockId::Furnace => 22,
            };
            assert_eq!(encoded(&id), [expected], "{id:?}: {ENCODING_WARNING}");
        }
    }

    #[test]
    fn new_blocks_drop_their_own_item() {
        for (block, item) in [
            (BlockId::CraftingTable, ItemId::CraftingTable),
            (BlockId::SprucePlanks, ItemId::SprucePlanks),
            (BlockId::Furnace, ItemId::Furnace),
        ] {
            assert_eq!(block.get_drops(1), HashMap::from([(item, 1)]));
            assert_eq!(item.get_default_type(), ItemType::Block(block));
        }
        assert_eq!(ItemId::Stick.get_default_type(), ItemType::Generic);
    }

    #[test]
    fn block_direction_encoding_is_pinned() {
        for direction in [
            BlockDirection::Front,
            BlockDirection::Right,
            BlockDirection::Back,
            BlockDirection::Left,
        ] {
            let expected: u8 = match direction {
                BlockDirection::Front => 0,
                BlockDirection::Right => 1,
                BlockDirection::Back => 2,
                BlockDirection::Left => 3,
            };
            assert_eq!(
                encoded(&direction),
                [expected],
                "{direction:?}: {ENCODING_WARNING}"
            );
        }
    }

    #[test]
    fn block_data_encoding_is_pinned() {
        let block = BlockData {
            breaking_progress: 3,
            level: 5,
            ..BlockData::new(BlockId::Stone, BlockDirection::Right)
        };
        assert_eq!(
            encoded(&block),
            [3, 1, 3, 5],
            "the fields of BlockData changed: bump PROTOCOL_ID and FORMAT_VERSION, and convert \
            the chunk files of the previous version"
        );
    }

    #[test]
    fn get_breaking_level_stays_within_zero_to_nine() {
        for id in regular_blocks() {
            let break_time = id.get_break_time().unwrap();
            for progress in 0..=break_time {
                let mut block = BlockData::new(id, BlockDirection::Front);
                block.breaking_progress = progress;
                assert!(
                    block.get_breaking_level() <= MAX_BREAKING_LEVEL,
                    "{id:?} at {progress}"
                );
            }
        }
    }

    #[test]
    fn get_breaking_level_of_untouched_and_fully_broken_block() {
        let mut block = BlockData::new(BlockId::Stone, BlockDirection::Front);
        assert_eq!(block.get_breaking_level(), 0);
        block.breaking_progress = block.id.get_break_time().unwrap() / 2;
        assert_eq!(block.get_breaking_level(), 5);
        block.breaking_progress = block.id.get_break_time().unwrap();
        assert_eq!(block.get_breaking_level(), MAX_BREAKING_LEVEL);
        block.breaking_progress = u8::MAX;
        assert_eq!(block.get_breaking_level(), MAX_BREAKING_LEVEL);
    }

    #[test]
    fn get_breaking_level_does_not_panic_for_any_block() {
        for id in ALL_BLOCKS {
            let block = BlockData::new(id, BlockDirection::Front);
            assert!(block.get_breaking_level() <= MAX_BREAKING_LEVEL, "{id:?}");
        }
    }

    #[test]
    fn get_drops_of_single_entry_tables() {
        assert_eq!(
            BlockId::Stone.get_drops(1),
            HashMap::from([(ItemId::Cobblestone, 1)])
        );
        assert_eq!(
            BlockId::Grass.get_drops(1),
            HashMap::from([(ItemId::Dirt, 1)])
        );
        assert_eq!(
            BlockId::Snow.get_drops(1),
            HashMap::from([(ItemId::Snowball, 4)])
        );
    }

    #[test]
    fn get_drops_accumulates_over_several_drops() {
        assert_eq!(
            BlockId::Sand.get_drops(3),
            HashMap::from([(ItemId::Sand, 3)])
        );
    }

    #[test]
    fn get_drops_is_empty_without_drop_table_or_drops() {
        assert!(BlockId::Water.get_drops(1).is_empty());
        assert!(BlockId::Glass.get_drops(1).is_empty());
        assert!(BlockId::Stone.get_drops(0).is_empty());
    }

    #[test]
    fn poppy_drops_a_poppy() {
        assert_eq!(
            BlockId::Poppy.get_drops(1),
            HashMap::from([(ItemId::Poppy, 1)])
        );
    }

    #[test]
    fn a_roll_picks_exactly_one_entry_in_proportion_to_its_chance() {
        let table = [(1, ItemId::Dirt, 1), (3, ItemId::Sand, 2)];
        assert_eq!(pick_drop(&table, 0), Some((ItemId::Dirt, 1)));
        for roll in 1..4 {
            assert_eq!(pick_drop(&table, roll), Some((ItemId::Sand, 2)), "{roll}");
        }
        assert_eq!(pick_drop(&table, 4), None);
        assert_eq!(pick_drop(&[], 0), None);
    }

    #[test]
    fn a_block_without_drop_table_drops_nothing() {
        assert!(BlockId::Water.get_drops(5).is_empty());
        assert_eq!(BlockId::Snow.get_drops(2)[&ItemId::Snowball], 8);
    }
}
