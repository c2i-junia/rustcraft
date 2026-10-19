use std::fmt::Debug;

use serde::de::value::{Error as ValueError, StrDeserializer};
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
    Stick,
    CraftingTable,
    SprucePlanks,
    Furnace,
    Chest,
}

impl ItemId {
    /// Every item, in the order of the variants.
    pub const ALL: [ItemId; 23] = [
        Self::Dirt,
        Self::Grass,
        Self::Stone,
        Self::OakLog,
        Self::OakPlanks,
        Self::OakLeaves,
        Self::Sand,
        Self::Cactus,
        Self::Ice,
        Self::Glass,
        Self::Bedrock,
        Self::Dandelion,
        Self::TallGrass,
        Self::Poppy,
        Self::Cobblestone,
        Self::Snow,
        Self::Snowball,
        Self::SpruceLog,
        Self::Stick,
        Self::CraftingTable,
        Self::SprucePlanks,
        Self::Furnace,
        Self::Chest,
    ];

    /// The name of the item in the commands, like in Minecraft: `oak_log`.
    pub fn name(&self) -> String {
        let mut name = String::new();
        for c in format!("{self:?}").chars() {
            if c.is_uppercase() && !name.is_empty() {
                name.push('_');
            }
            name.extend(c.to_lowercase());
        }
        name
    }

    /// The item called `name`: `oak_log` like in Minecraft, in any case, or `OakLog` like the
    /// variant.
    pub fn from_name(name: &str) -> Option<ItemId> {
        let variant =
            |name: &str| ItemId::deserialize(StrDeserializer::<ValueError>::new(name)).ok();
        variant(name).or_else(|| {
            let mut pascal = String::new();
            for word in name.split('_') {
                let mut chars = word.chars();
                pascal.extend(chars.next()?.to_uppercase());
                pascal.push_str(&chars.as_str().to_lowercase());
            }
            variant(&pascal)
        })
    }

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
            Self::CraftingTable => ItemType::Block(BlockId::CraftingTable),
            Self::SprucePlanks => ItemType::Block(BlockId::SprucePlanks),
            Self::Furnace => ItemType::Block(BlockId::Furnace),
            Self::Chest => ItemType::Block(BlockId::Chest),

            Self::Snowball | Self::Stick => ItemType::Generic,
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
            (ItemId::Stick, 18),
            (ItemId::CraftingTable, 19),
            (ItemId::SprucePlanks, 20),
            (ItemId::Furnace, 21),
            (ItemId::Chest, 22),
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
                | ItemId::SpruceLog
                | ItemId::Stick
                | ItemId::CraftingTable
                | ItemId::SprucePlanks
                | ItemId::Furnace
                | ItemId::Chest => {}
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

    #[test]
    fn all_lists_every_item_in_order() {
        for (index, item) in ItemId::ALL.iter().enumerate() {
            assert_eq!(bincode::options().serialize(item).unwrap(), [index as u8]);
        }
        // the variant after the last one listed does not exist
        let past = [ItemId::ALL.len() as u8];
        assert!(bincode::options().deserialize::<ItemId>(&past).is_err());
    }

    #[test]
    fn an_item_is_named_like_in_minecraft_and_found_by_its_name() {
        assert_eq!(ItemId::OakLog.name(), "oak_log");
        assert_eq!(ItemId::Dirt.name(), "dirt");
        for item in ItemId::ALL {
            assert_eq!(ItemId::from_name(&item.name()), Some(item), "{item:?}");
        }
        assert_eq!(ItemId::from_name("OAK_LOG"), Some(ItemId::OakLog));
        assert_eq!(ItemId::from_name("OakLog"), Some(ItemId::OakLog));
        // only the names of the items, not anything serde would accept
        assert_eq!(ItemId::from_name("oak__log"), None);
        assert_eq!(ItemId::from_name("diamond"), None);
    }
}
