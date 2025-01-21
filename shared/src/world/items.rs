use std::fmt::Debug;

use serde::{Deserialize, Serialize};

use super::{BlockId, GameElementId};

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
pub enum ItemId {
    #[default]
    Dirt,
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
    TallGrass,
    Poppy,
    Cobblestone,
    Snow,
    Snowball,
    SpruceLog,
}

impl ItemId {
    pub fn get_max_stack(&self) -> u32 {
        64
    }

    pub fn get_default_type(&self) -> ItemType {
        match *self {
            Self::Dirt => ItemType::Block(BlockId::Dirt),
            Self::Bedrock => ItemType::Block(BlockId::Bedrock),
            Self::Grass => ItemType::Block(BlockId::Grass),
            Self::Stone => ItemType::Block(BlockId::Stone),
            Self::OakLog => ItemType::Block(BlockId::OakLog),
            Self::OakPlanks => ItemType::Block(BlockId::OakPlanks),
            Self::Sand => ItemType::Block(BlockId::Sand),
            Self::Cactus => ItemType::Block(BlockId::Cactus),
            Self::Ice => ItemType::Block(BlockId::Ice),
            Self::OakLeaves => ItemType::Block(BlockId::OakLeaves),
            Self::Glass => ItemType::Block(BlockId::Glass),
            Self::Dandelion => ItemType::Block(BlockId::Dandelion),
            Self::Poppy => ItemType::Block(BlockId::Poppy),
            Self::TallGrass => ItemType::Block(BlockId::TallGrass),
            Self::Cobblestone => ItemType::Block(BlockId::Cobblestone),
            Self::Snow => ItemType::Block(BlockId::Snow),
            Self::SpruceLog => ItemType::Block(BlockId::SpruceLog),

            Self::Snowball => ItemType::Generic,
        }
    }
}

impl GameElementId for ItemId {}

/// Temporary struct for deserialization purposes
#[derive(Debug, Serialize, Deserialize)]
pub struct TempBlock {
    pub id: String,
    pub drops: Vec<(u16, String)>,
    pub break_time: f32,
    pub uvs: [f32; 4],
}

/// Type of armor piece
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone, Copy)]
pub enum ArmorType {
    Helmet,
    Chestplate,
    Leggings,
    Boots,
}

/// Type of item
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone, Copy)]
pub enum ItemType {
    Generic,
    Block(BlockId),
    Tool { durability: u16 },
    Armor(ArmorType),
}

impl Default for ItemType {
    fn default() -> Self {
        ItemType::Block(BlockId::Dirt)
    }
}
