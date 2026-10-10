use std::collections::HashMap;

use crate::HALF_BLOCK;

use super::{GameElementId, ItemId};
use bevy::math::{bounding::Aabb3d, IVec3, Vec3, Vec3A};
use rand::Rng;
use serde::{Deserialize, Serialize};

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
}

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
}

impl BlockData {
    pub fn new(id: BlockId, direction: BlockDirection) -> Self {
        BlockData {
            id,
            direction,
            breaking_progress: 0,
        }
    }

    /// Breaking stage from 0 to 10; always 0 for an unbreakable block.
    pub fn get_breaking_level(&self) -> u8 {
        match self.id.get_break_time() {
            Some(break_time) => ((self.breaking_progress as u16 * 10) / break_time as u16) as u8,
            None => 0,
        }
    }
}

pub enum BlockTags {
    Solid,
    Stone,
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

    pub fn is_biome_colored() -> bool {
        false
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
            Self::Bedrock | Self::Water => return None,
        };
        Some(6 * steps)
    }

    pub fn get_color(&self) -> [f32; 4] {
        match *self {
            Self::Grass => [0.1, 1.0, 0.25, 1.],
            _ => [1., 1., 1., 1.],
        }
    }

    pub fn get_drops(&self, nb_drops: u32) -> HashMap<ItemId, u32> {
        let mut drops = HashMap::new();
        let table = self.get_drop_table();

        if table.is_empty() {
            return drops;
        }

        let total = table
            .clone()
            .into_iter()
            .reduce(|a, b| (a.0 + b.0, a.1, a.2))
            .unwrap()
            .0;

        // Choose drop items
        for _ in 0..nb_drops {
            let mut nb = rand::thread_rng().gen_range(0..total);
            for item in table.iter() {
                if nb < item.0 {
                    drops.insert(item.1, *drops.get(&item.1).unwrap_or(&0) + item.2);
                } else {
                    nb -= item.0;
                }
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
            BlockId::Water => vec![],
            _ => vec![],
        }
    }

    pub fn get_tags(&self) -> Vec<BlockTags> {
        match *self {
            BlockId::Stone => vec![BlockTags::Stone, BlockTags::Solid],
            _ => vec![BlockTags::Solid],
        }
    }

    pub fn get_visibility(&self) -> BlockTransparency {
        match *self {
            Self::Dandelion | Self::Poppy | Self::TallGrass => BlockTransparency::Decoration,
            Self::Glass | Self::OakLeaves | Self::SpruceLeaves => BlockTransparency::Transparent,
            Self::Water => BlockTransparency::Liquid,
            _ => BlockTransparency::Solid,
        }
    }

    pub fn get_interaction_box(&self, position: &IVec3) -> Aabb3d {
        let pos = Vec3::new(position.x as f32, position.y as f32, position.z as f32);
        match *self {
            Self::Dandelion | Self::Poppy | Self::TallGrass => {
                Aabb3d::new(pos - Vec3::new(0f32, 0.25, 0f32), HALF_BLOCK / 2.0)
            }
            _ => Aabb3d::new(pos, HALF_BLOCK),
        }
    }
}

impl GameElementId for BlockId {}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL_BLOCKS: [BlockId; 20] = [
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
    ];

    /// Blocks that cannot be broken by hand.
    const UNBREAKABLE_BLOCKS: [BlockId; 2] = [BlockId::Bedrock, BlockId::Water];

    /// Fails to compile when a `BlockId` variant is added without updating `ALL_BLOCKS`.
    #[allow(dead_code)]
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
            | BlockId::Water => {}
        }
    }

    fn regular_blocks() -> impl Iterator<Item = BlockId> {
        ALL_BLOCKS
            .into_iter()
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

    #[test]
    fn get_breaking_level_stays_within_zero_to_ten() {
        for id in regular_blocks() {
            let break_time = id.get_break_time().unwrap();
            for progress in 0..=break_time {
                let mut block = BlockData::new(id, BlockDirection::Front);
                block.breaking_progress = progress;
                assert!(block.get_breaking_level() <= 10, "{id:?} at {progress}");
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
        assert_eq!(block.get_breaking_level(), 10);
    }

    #[test]
    fn get_breaking_level_does_not_panic_for_any_block() {
        for id in ALL_BLOCKS {
            let block = BlockData::new(id, BlockDirection::Front);
            assert!(block.get_breaking_level() <= 10, "{id:?}");
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
}
