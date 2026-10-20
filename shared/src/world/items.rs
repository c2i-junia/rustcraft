use std::fmt::Debug;

use serde::de::value::{Error as ValueError, StrDeserializer};
use serde::{Deserialize, Serialize};

use super::{BlockId, GameElementId, MobKind};

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
    WoodenPickaxe,
    WoodenAxe,
    WoodenShovel,
    WoodenHoe,
    WoodenSword,
    Coal,
    CoalOre,
    IronOre,
    GoldOre,
    Gravel,
    FoxSpawnEgg,
    SheepSpawnEgg,
    Torch,
}

impl ItemId {
    /// Every item, in the order of the variants.
    pub const ALL: [ItemId; 36] = [
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
        Self::WoodenPickaxe,
        Self::WoodenAxe,
        Self::WoodenShovel,
        Self::WoodenHoe,
        Self::WoodenSword,
        Self::Coal,
        Self::CoalOre,
        Self::IronOre,
        Self::GoldOre,
        Self::Gravel,
        Self::FoxSpawnEgg,
        Self::SheepSpawnEgg,
        Self::Torch,
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
        if self.tool().is_some() {
            1
        } else {
            64
        }
    }

    /// The mob a spawn egg makes appear, used on a block.
    pub fn spawned_mob(&self) -> Option<MobKind> {
        match *self {
            Self::FoxSpawnEgg => Some(MobKind::Fox),
            Self::SheepSpawnEgg => Some(MobKind::Sheep),
            _ => None,
        }
    }

    /// What the item does as a tool, if it is one.
    pub fn tool(&self) -> Option<Tool> {
        let (kind, wear_per_block) = match *self {
            Self::WoodenPickaxe => (ToolKind::Pickaxe, 1),
            Self::WoodenAxe => (ToolKind::Axe, 1),
            Self::WoodenShovel => (ToolKind::Shovel, 1),
            Self::WoodenHoe => (ToolKind::Hoe, 1),
            // A weapon: like in Minecraft, mining wears it twice as fast
            Self::WoodenSword => (ToolKind::Sword, 2),
            _ => return None,
        };
        Some(Tool {
            kind,
            speed: WOODEN_TOOL_SPEED,
            durability: WOODEN_TOOL_DURABILITY,
            wear_per_block,
        })
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
            Self::CoalOre => ItemType::Block(BlockId::CoalOre),
            Self::IronOre => ItemType::Block(BlockId::IronOre),
            Self::GoldOre => ItemType::Block(BlockId::GoldOre),
            Self::Gravel => ItemType::Block(BlockId::Gravel),
            Self::Torch => ItemType::Block(BlockId::Torch),

            Self::Snowball | Self::Stick | Self::Coal | Self::FoxSpawnEgg | Self::SheepSpawnEgg => {
                ItemType::Generic
            }

            Self::WoodenPickaxe
            | Self::WoodenAxe
            | Self::WoodenShovel
            | Self::WoodenHoe
            | Self::WoodenSword => ItemType::Tool { wear: 0 },
        }
    }
}

impl GameElementId for ItemId {}

/// How many times faster a wooden tool breaks the blocks it is made for than a hand.
pub const WOODEN_TOOL_SPEED: u8 = 2;
/// Blocks a wooden tool breaks before it breaks itself, like in Minecraft.
pub const WOODEN_TOOL_DURABILITY: u32 = 59;

/// The kinds of tools: each one breaks some blocks faster, see `BlockId::get_tool_kind`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ToolKind {
    Pickaxe,
    Axe,
    Shovel,
    Hoe,
    Sword,
}

/// What a tool item does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tool {
    pub kind: ToolKind,
    /// Mining hits each hit counts for on a block of its kind.
    pub speed: u8,
    /// The wear at which the tool breaks.
    pub durability: u32,
    /// The wear a broken block adds.
    pub wear_per_block: u32,
}

/// Type of item. Append only, like `ItemId`: chest files and network messages store the
/// position of the variant (bincode).
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone, Copy)]
pub enum ItemType {
    Generic,
    Block(BlockId),
    /// A tool, and the wear of this one: it breaks when the wear reaches its durability. It is
    /// held by the stack, of one tool, so that it follows the tool wherever it goes.
    Tool {
        wear: u32,
    },
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
            (ItemId::WoodenPickaxe, 23),
            (ItemId::WoodenAxe, 24),
            (ItemId::WoodenShovel, 25),
            (ItemId::WoodenHoe, 26),
            (ItemId::WoodenSword, 27),
            (ItemId::Coal, 28),
            (ItemId::CoalOre, 29),
            (ItemId::IronOre, 30),
            (ItemId::GoldOre, 31),
            (ItemId::Gravel, 32),
            (ItemId::FoxSpawnEgg, 33),
            (ItemId::SheepSpawnEgg, 34),
            (ItemId::Torch, 35),
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
                | ItemId::Chest
                | ItemId::WoodenPickaxe
                | ItemId::WoodenAxe
                | ItemId::WoodenShovel
                | ItemId::WoodenHoe
                | ItemId::WoodenSword
                | ItemId::Coal
                | ItemId::CoalOre
                | ItemId::IronOre
                | ItemId::GoldOre
                | ItemId::Gravel
                | ItemId::FoxSpawnEgg
                | ItemId::SheepSpawnEgg
                | ItemId::Torch => {}
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

    #[test]
    fn a_spawn_egg_names_its_mob_and_is_named_after_it() {
        assert_eq!(ItemId::FoxSpawnEgg.spawned_mob(), Some(MobKind::Fox));
        assert_eq!(ItemId::SheepSpawnEgg.spawned_mob(), Some(MobKind::Sheep));
        assert_eq!(
            ItemId::from_name("sheep_spawn_egg"),
            Some(ItemId::SheepSpawnEgg)
        );
        // every mob has its egg
        for kind in MobKind::ALL {
            let eggs = ItemId::ALL
                .iter()
                .filter(|item| item.spawned_mob() == Some(kind));
            assert_eq!(eggs.count(), 1, "{kind:?}");
        }
        assert_eq!(ItemId::Stone.spawned_mob(), None);
    }

    #[test]
    fn item_type_encoding_is_pinned() {
        // The chest files saved before the tools hold these: they must still read the same
        let encoded = |item_type: ItemType| bincode::options().serialize(&item_type).unwrap();
        assert_eq!(encoded(ItemType::Generic), [0]);
        assert_eq!(encoded(ItemType::Block(BlockId::Stone)), [1, 3]);
        assert_eq!(encoded(ItemType::Tool { wear: 5 }), [2, 5]);
    }

    #[test]
    fn the_wooden_tools_are_unworn_tools_that_stack_to_one() {
        let tools = [
            (ItemId::WoodenPickaxe, ToolKind::Pickaxe),
            (ItemId::WoodenAxe, ToolKind::Axe),
            (ItemId::WoodenShovel, ToolKind::Shovel),
            (ItemId::WoodenHoe, ToolKind::Hoe),
            (ItemId::WoodenSword, ToolKind::Sword),
        ];
        for (item, kind) in tools {
            let tool = item.tool().unwrap();
            assert_eq!(tool.kind, kind);
            assert_eq!(tool.speed, WOODEN_TOOL_SPEED);
            assert_eq!(tool.durability, WOODEN_TOOL_DURABILITY);
            assert_eq!(item.get_max_stack(), 1, "{item:?}");
            assert_eq!(item.get_default_type(), ItemType::Tool { wear: 0 });
        }
        assert_eq!(ItemId::WoodenSword.tool().unwrap().wear_per_block, 2);
        let others = ItemId::ALL
            .iter()
            .filter(|item| !tools.iter().any(|(t, _)| t == *item));
        for item in others {
            assert_eq!(item.tool(), None, "{item:?}");
            assert_eq!(item.get_max_stack(), 64, "{item:?}");
        }
    }
}
