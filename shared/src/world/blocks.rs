use std::collections::HashMap;

use super::{GameElementId, ItemId};
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
    pub flipped: bool,
    pub direction: BlockDirection,
}

impl BlockData {
    pub fn new(id: BlockId, flipped: bool, direction: BlockDirection) -> Self {
        BlockData {
            id,
            flipped,
            direction,
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

impl BlockId {
    pub fn has_hitbox(&self) -> bool {
        !matches!(
            *self,
            BlockId::Dandelion | BlockId::Poppy | BlockId::TallGrass
        )
    }

    pub fn is_biome_colored() -> bool {
        false
    }

    pub fn get_break_time(&self) -> f32 {
        match *self {
            Self::Bedrock => -1.,
            _ => 5.,
        }
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
            BlockId::Poppy => vec![(1, ItemId::Dandelion, 1)],
            BlockId::TallGrass => vec![(1, ItemId::TallGrass, 1)],
            BlockId::SpruceLog => vec![(1, ItemId::SpruceLog, 1)],
            BlockId::Snow => vec![(1, ItemId::Snowball, 4)],
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
            _ => BlockTransparency::Solid,
        }
    }
}

impl GameElementId for BlockId {}
