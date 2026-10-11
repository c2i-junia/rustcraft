use std::fmt::Debug;

use serde::{Deserialize, Serialize};

use super::{BlockId, GameElementId};

/// Append only: never reorder or remove a variant. Player files store items by name (RON), but
/// network messages carry the position of the variant (bincode), so another order breaks the
/// protocol: bump `PROTOCOL_ID` when this enum changes.
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

/// Type of item
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone, Copy)]
pub enum ItemType {
    Generic,
    Block(BlockId),
}

impl Default for ItemType {
    fn default() -> Self {
        ItemType::Block(BlockId::Dirt)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bincode::Options;

    #[test]
    fn item_encoding_is_pinned() {
        let table = [
            (ItemId::Dirt, 0u8),
            (ItemId::Grass, 1),
            (ItemId::Stone, 2),
            (ItemId::OakLog, 3),
            (ItemId::OakPlanks, 4),
            (ItemId::OakLeaves, 5),
            (ItemId::Sand, 6),
            (ItemId::Cactus, 7),
            (ItemId::Ice, 8),
            (ItemId::Glass, 9),
            (ItemId::Bedrock, 10),
            (ItemId::Dandelion, 11),
            (ItemId::TallGrass, 12),
            (ItemId::Poppy, 13),
            (ItemId::Cobblestone, 14),
            (ItemId::Snow, 15),
            (ItemId::Snowball, 16),
            (ItemId::SpruceLog, 17),
        ];
        // Fails to compile when a variant is added without a line in the table
        for (id, _) in table {
            match id {
                ItemId::Dirt
                | ItemId::Grass
                | ItemId::Stone
                | ItemId::OakLog
                | ItemId::OakPlanks
                | ItemId::OakLeaves
                | ItemId::Sand
                | ItemId::Cactus
                | ItemId::Ice
                | ItemId::Glass
                | ItemId::Bedrock
                | ItemId::Dandelion
                | ItemId::TallGrass
                | ItemId::Poppy
                | ItemId::Cobblestone
                | ItemId::Snow
                | ItemId::Snowball
                | ItemId::SpruceLog => {}
            }
        }
        for (id, expected) in table {
            assert_eq!(
                bincode::options().serialize(&id).unwrap(),
                [expected],
                "{id:?}: the encoded value of an existing variant changed, which breaks the \
                protocol. New variants go at the END of the enum; add them to this table and \
                bump PROTOCOL_ID."
            );
        }
    }
}
