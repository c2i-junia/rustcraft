//! The recipes, and what the crafting grid makes.

use std::sync::OnceLock;

use crate::{
    players::Inventory,
    world::{palette::WOOLS, DyeColor, ItemId, ItemStack, Rock, Wood},
    MAX_INVENTORY_SLOTS,
};

/// The crafting grid is held in the inventory, in the slots after those of the inventory.
pub const CRAFTING_GRID_START: u32 = MAX_INVENTORY_SLOTS;
/// Width and height of the largest grid, that of a crafting table. The inventory alone only
/// uses its top left 2x2 corner.
pub const CRAFTING_GRID_SIZE: u32 = 3;

/// What the cells of the grid hold, row after row.
pub type CraftingGrid = [Option<ItemId>; (CRAFTING_GRID_SIZE * CRAFTING_GRID_SIZE) as usize];

/// The inventory slot of the cell of the grid at `row`, `col`.
pub const fn grid_slot(row: u32, col: u32) -> u32 {
    CRAFTING_GRID_START + row * CRAFTING_GRID_SIZE + col
}

/// A family of items any of which a recipe takes in a cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemTag {
    Planks,
    /// The broken rocks: cobblestone, and the gloomrock of the depths.
    Cobble,
    /// The wool of every colour, which a dye colours again.
    Wool,
    /// The five fish of `world::fishing`, raw.
    RawFish,
    /// The meats roasted.
    RoastMeat,
}

impl ItemTag {
    /// The items of the family, in the order of `ItemId::ALL`. A new species of wood joins
    /// `Planks` by being listed here.
    pub fn members(self) -> &'static [ItemId] {
        match self {
            Self::Planks => &[
                ItemId::OakPlanks,
                ItemId::SprucePlanks,
                ItemId::JunglePlanks,
                ItemId::WillowPlanks,
                ItemId::AcaciaPlanks,
            ],
            Self::Cobble => &[ItemId::Cobblestone, ItemId::CobbledGloomrock],
            Self::Wool => &WOOLS,
            Self::RawFish => &[
                ItemId::RawRoach,
                ItemId::RawPerch,
                ItemId::RawPike,
                ItemId::RawEel,
                ItemId::RawGlowfin,
            ],
            Self::RoastMeat => &[
                ItemId::RoastMutton,
                ItemId::RoastPork,
                ItemId::RoastBeef,
                ItemId::RoastChicken,
                ItemId::RoastRabbit,
            ],
        }
    }

    /// The name of the family, as the UI writes it.
    pub fn name(self) -> &'static str {
        match self {
            Self::Planks => "Planks",
            Self::Cobble => "Cobble",
            Self::Wool => "Wool",
            Self::RawFish => "RawFish",
            Self::RoastMeat => "RoastMeat",
        }
    }
}

/// What a cell of a recipe takes: an item, or any item of a family.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ingredient {
    Item(ItemId),
    Tag(ItemTag),
}

impl Ingredient {
    /// Whether a cell of this ingredient takes `item`.
    pub fn matches(self, item: ItemId) -> bool {
        self.members().contains(&item)
    }

    /// The items a cell of this ingredient takes: the item alone, or the family's.
    pub fn members(&self) -> &[ItemId] {
        match self {
            Self::Item(item) => std::slice::from_ref(item),
            Self::Tag(tag) => tag.members(),
        }
    }
}

/// A shaped recipe: its cells, row after row, must match the filled part of the grid exactly,
/// wherever that part sits in the grid, or match it mirrored left to right. A shapeless one only
/// needs its ingredients in the grid, one per cell, anywhere.
#[derive(Clone, Copy)]
struct Recipe {
    width: usize,
    cells: &'static [Option<Ingredient>],
    output: ItemId,
    nb: u32,
    shapeless: bool,
}

/// A cell of a recipe taking `item`.
const fn item(item: ItemId) -> Option<Ingredient> {
    Some(Ingredient::Item(item))
}

const OAK_LOG: Option<Ingredient> = item(ItemId::OakLog);
const SPRUCE_LOG: Option<Ingredient> = item(ItemId::SpruceLog);
const JUNGLE_LOG: Option<Ingredient> = item(ItemId::JungleLog);
const PLANKS: Option<Ingredient> = Some(Ingredient::Tag(ItemTag::Planks));
const COBBLE: Option<Ingredient> = Some(Ingredient::Tag(ItemTag::Cobble));
const STICK: Option<Ingredient> = item(ItemId::Stick);
const COAL: Option<Ingredient> = item(ItemId::Coal);
const CHARCOAL: Option<Ingredient> = item(ItemId::Charcoal);
const IRON: Option<Ingredient> = item(ItemId::IronIngot);
const GOLD: Option<Ingredient> = item(ItemId::GoldIngot);
const BRONZE: Option<Ingredient> = item(ItemId::BronzeIngot);
const DIAMOND: Option<Ingredient> = item(ItemId::Diamond);
const FLINT: Option<Ingredient> = item(ItemId::Flint);

/// The tools' shapes, Minecraft's, made of `m` and sticks.
const fn pickaxe(m: Option<Ingredient>) -> [Option<Ingredient>; 9] {
    [m, m, m, None, STICK, None, None, STICK, None]
}
const fn axe(m: Option<Ingredient>) -> [Option<Ingredient>; 6] {
    [m, m, m, STICK, None, STICK]
}
const fn shovel(m: Option<Ingredient>) -> [Option<Ingredient>; 3] {
    [m, STICK, STICK]
}
const fn hoe(m: Option<Ingredient>) -> [Option<Ingredient>; 6] {
    [m, m, None, STICK, None, STICK]
}
const fn sword(m: Option<Ingredient>) -> [Option<Ingredient>; 3] {
    [m, m, STICK]
}
const LEATHER: Option<Ingredient> = item(ItemId::Leather);

/// The armour's shapes, the pieces as they are worn, made of `m`.
const fn helmet(m: Option<Ingredient>) -> [Option<Ingredient>; 6] {
    [m, m, m, m, None, m]
}
const fn chestplate(m: Option<Ingredient>) -> [Option<Ingredient>; 9] {
    [m, None, m, m, m, m, m, m, m]
}
const fn leggings(m: Option<Ingredient>) -> [Option<Ingredient>; 9] {
    [m, m, m, m, None, m, m, None, m]
}
const fn boots(m: Option<Ingredient>) -> [Option<Ingredient>; 6] {
    [m, None, m, m, None, m]
}
const GLASS: Option<Ingredient> = item(ItemId::Glass);
const BOWL: Option<Ingredient> = item(ItemId::Bowl);
const WOOL: Option<Ingredient> = Some(Ingredient::Tag(ItemTag::Wool));

#[rustfmt::skip]
const RECIPES: &[Recipe] = &[
    Recipe { width: 1, cells: &[OAK_LOG], output: ItemId::OakPlanks, nb: 4, shapeless: false },
    Recipe { width: 1, cells: &[SPRUCE_LOG], output: ItemId::SprucePlanks, nb: 4, shapeless: false },
    Recipe { width: 1, cells: &[JUNGLE_LOG], output: ItemId::JunglePlanks, nb: 4, shapeless: false },
    Recipe { width: 1, cells: &[PLANKS, PLANKS], output: ItemId::Stick, nb: 4, shapeless: false },
    Recipe { width: 1, cells: &[COAL, STICK], output: ItemId::Torch, nb: 4, shapeless: false },
    Recipe { width: 2, cells: &[PLANKS; 4], output: ItemId::CraftingTable, nb: 1, shapeless: false },
    Recipe {
        width: 3,
        cells: &[
            COBBLE, COBBLE, COBBLE,
            COBBLE, None, COBBLE,
            COBBLE, COBBLE, COBBLE,
        ],
        output: ItemId::Furnace,
        nb: 1,
        shapeless: false,
    },
    Recipe {
        width: 3,
        cells: &[
            PLANKS, PLANKS, PLANKS,
            PLANKS, None, PLANKS,
            PLANKS, PLANKS, PLANKS,
        ],
        output: ItemId::Chest,
        nb: 1,
        shapeless: false,
    },
    Recipe { width: 3, cells: &pickaxe(PLANKS), output: ItemId::WoodenPickaxe, nb: 1, shapeless: false },
    Recipe { width: 2, cells: &axe(PLANKS), output: ItemId::WoodenAxe, nb: 1, shapeless: false },
    Recipe { width: 1, cells: &shovel(PLANKS), output: ItemId::WoodenShovel, nb: 1, shapeless: false },
    Recipe { width: 2, cells: &hoe(PLANKS), output: ItemId::WoodenHoe, nb: 1, shapeless: false },
    Recipe { width: 1, cells: &sword(PLANKS), output: ItemId::WoodenSword, nb: 1, shapeless: false },
    Recipe { width: 3, cells: &pickaxe(COBBLE), output: ItemId::StonePickaxe, nb: 1, shapeless: false },
    Recipe { width: 2, cells: &axe(COBBLE), output: ItemId::StoneAxe, nb: 1, shapeless: false },
    Recipe { width: 1, cells: &shovel(COBBLE), output: ItemId::StoneShovel, nb: 1, shapeless: false },
    Recipe { width: 2, cells: &hoe(COBBLE), output: ItemId::StoneHoe, nb: 1, shapeless: false },
    Recipe { width: 1, cells: &sword(COBBLE), output: ItemId::StoneSword, nb: 1, shapeless: false },
    Recipe { width: 3, cells: &pickaxe(IRON), output: ItemId::IronPickaxe, nb: 1, shapeless: false },
    Recipe { width: 2, cells: &axe(IRON), output: ItemId::IronAxe, nb: 1, shapeless: false },
    Recipe { width: 1, cells: &shovel(IRON), output: ItemId::IronShovel, nb: 1, shapeless: false },
    Recipe { width: 2, cells: &hoe(IRON), output: ItemId::IronHoe, nb: 1, shapeless: false },
    Recipe { width: 1, cells: &sword(IRON), output: ItemId::IronSword, nb: 1, shapeless: false },
    Recipe { width: 3, cells: &pickaxe(GOLD), output: ItemId::GoldenPickaxe, nb: 1, shapeless: false },
    Recipe { width: 2, cells: &axe(GOLD), output: ItemId::GoldenAxe, nb: 1, shapeless: false },
    Recipe { width: 1, cells: &shovel(GOLD), output: ItemId::GoldenShovel, nb: 1, shapeless: false },
    Recipe { width: 2, cells: &hoe(GOLD), output: ItemId::GoldenHoe, nb: 1, shapeless: false },
    Recipe { width: 1, cells: &sword(GOLD), output: ItemId::GoldenSword, nb: 1, shapeless: false },
    Recipe { width: 3, cells: &pickaxe(DIAMOND), output: ItemId::DiamondPickaxe, nb: 1, shapeless: false },
    Recipe { width: 2, cells: &axe(DIAMOND), output: ItemId::DiamondAxe, nb: 1, shapeless: false },
    Recipe { width: 1, cells: &shovel(DIAMOND), output: ItemId::DiamondShovel, nb: 1, shapeless: false },
    Recipe { width: 2, cells: &hoe(DIAMOND), output: ItemId::DiamondHoe, nb: 1, shapeless: false },
    Recipe { width: 1, cells: &sword(DIAMOND), output: ItemId::DiamondSword, nb: 1, shapeless: false },
    Recipe { width: 3, cells: &[IRON; 9], output: ItemId::IronBlock, nb: 1, shapeless: false },
    Recipe { width: 3, cells: &[GOLD; 9], output: ItemId::GoldBlock, nb: 1, shapeless: false },
    Recipe { width: 3, cells: &[DIAMOND; 9], output: ItemId::DiamondBlock, nb: 1, shapeless: false },
    Recipe { width: 1, cells: &[item(ItemId::IronBlock)], output: ItemId::IronIngot, nb: 9, shapeless: false },
    Recipe { width: 1, cells: &[item(ItemId::GoldBlock)], output: ItemId::GoldIngot, nb: 9, shapeless: false },
    Recipe { width: 1, cells: &[item(ItemId::DiamondBlock)], output: ItemId::Diamond, nb: 9, shapeless: false },
    Recipe { width: 1, cells: &[CHARCOAL, STICK], output: ItemId::Torch, nb: 4, shapeless: false },
    Recipe { width: 3, cells: &[IRON, None, IRON, None, IRON, None], output: ItemId::Bucket, nb: 1, shapeless: false },
    Recipe { width: 1, cells: &[item(ItemId::WillowLog)], output: ItemId::WillowPlanks, nb: 4, shapeless: false },
    Recipe { width: 1, cells: &[item(ItemId::AcaciaLog)], output: ItemId::AcaciaPlanks, nb: 4, shapeless: false },
    Recipe { width: 1, cells: &[item(ItemId::Stone), item(ItemId::Stone)], output: ItemId::HewnStone, nb: 2, shapeless: false },
    Recipe { width: 2, cells: &[item(ItemId::FiredBrick), item(ItemId::FiredBrick), item(ItemId::FiredBrick), item(ItemId::FiredBrick)], output: ItemId::BrickWall, nb: 1, shapeless: false },
    Recipe { width: 1, cells: &[item(ItemId::Dirt), item(ItemId::Dirt), item(ItemId::Gravel)], output: ItemId::RammedEarth, nb: 2, shapeless: false },
    Recipe { width: 2, cells: &[item(ItemId::Fiber), item(ItemId::Fiber), item(ItemId::Fiber), item(ItemId::Fiber)], output: ItemId::Thatch, nb: 1, shapeless: false },
    Recipe { width: 1, cells: &[item(ItemId::Fiber), item(ItemId::Fiber), item(ItemId::Fiber)], output: ItemId::Rope, nb: 2, shapeless: false },
    Recipe { width: 2, cells: &[item(ItemId::CopperIngot), item(ItemId::CopperIngot), item(ItemId::CopperIngot), item(ItemId::TinIngot)], output: ItemId::BronzeIngot, nb: 4, shapeless: false },
    Recipe { width: 3, cells: &[item(ItemId::Coal), item(ItemId::Coal), item(ItemId::Coal), item(ItemId::Coal), item(ItemId::Coal), item(ItemId::Coal), item(ItemId::Coal), item(ItemId::Coal), item(ItemId::Coal)], output: ItemId::CoalBlock, nb: 1, shapeless: false },
    Recipe { width: 1, cells: &[item(ItemId::CoalBlock)], output: ItemId::Coal, nb: 9, shapeless: false },
    Recipe { width: 3, cells: &[item(ItemId::CopperIngot), item(ItemId::CopperIngot), item(ItemId::CopperIngot), item(ItemId::CopperIngot), item(ItemId::CopperIngot), item(ItemId::CopperIngot), item(ItemId::CopperIngot), item(ItemId::CopperIngot), item(ItemId::CopperIngot)], output: ItemId::CopperBlock, nb: 1, shapeless: false },
    Recipe { width: 1, cells: &[item(ItemId::CopperBlock)], output: ItemId::CopperIngot, nb: 9, shapeless: false },
    Recipe { width: 3, cells: &[item(ItemId::TinIngot), item(ItemId::TinIngot), item(ItemId::TinIngot), item(ItemId::TinIngot), item(ItemId::TinIngot), item(ItemId::TinIngot), item(ItemId::TinIngot), item(ItemId::TinIngot), item(ItemId::TinIngot)], output: ItemId::TinBlock, nb: 1, shapeless: false },
    Recipe { width: 1, cells: &[item(ItemId::TinBlock)], output: ItemId::TinIngot, nb: 9, shapeless: false },
    Recipe { width: 3, cells: &[item(ItemId::BronzeIngot), item(ItemId::BronzeIngot), item(ItemId::BronzeIngot), item(ItemId::BronzeIngot), item(ItemId::BronzeIngot), item(ItemId::BronzeIngot), item(ItemId::BronzeIngot), item(ItemId::BronzeIngot), item(ItemId::BronzeIngot)], output: ItemId::BronzeBlock, nb: 1, shapeless: false },
    Recipe { width: 1, cells: &[item(ItemId::BronzeBlock)], output: ItemId::BronzeIngot, nb: 9, shapeless: false },
    Recipe { width: 3, cells: &[item(ItemId::HewnStone), item(ItemId::HewnStone), item(ItemId::HewnStone)], output: ItemId::HewnStoneSlab, nb: 6, shapeless: false },
    Recipe { width: 3, cells: &[item(ItemId::BrickWall), item(ItemId::BrickWall), item(ItemId::BrickWall)], output: ItemId::BrickSlab, nb: 6, shapeless: false },
    Recipe { width: 3, cells: &[None, item(ItemId::IronIngot), None, item(ItemId::IronIngot), item(ItemId::Torch), item(ItemId::IronIngot), None, item(ItemId::IronIngot), None], output: ItemId::Lantern, nb: 1, shapeless: false },
    Recipe { width: 3, cells: &pickaxe(BRONZE), output: ItemId::BronzePickaxe, nb: 1, shapeless: false },
    Recipe { width: 2, cells: &axe(BRONZE), output: ItemId::BronzeAxe, nb: 1, shapeless: false },
    Recipe { width: 1, cells: &shovel(BRONZE), output: ItemId::BronzeShovel, nb: 1, shapeless: false },
    Recipe { width: 2, cells: &hoe(BRONZE), output: ItemId::BronzeHoe, nb: 1, shapeless: false },
    Recipe { width: 1, cells: &sword(BRONZE), output: ItemId::BronzeSword, nb: 1, shapeless: false },
    Recipe { width: 2, cells: &[item(ItemId::HewnStone), None, item(ItemId::HewnStone), item(ItemId::HewnStone), item(ItemId::HewnStone), item(ItemId::HewnStone)], output: ItemId::HewnStoneStairs, nb: 4, shapeless: false },
    Recipe { width: 2, cells: &[item(ItemId::BrickWall), None, item(ItemId::BrickWall), item(ItemId::BrickWall), item(ItemId::BrickWall), item(ItemId::BrickWall)], output: ItemId::BrickStairs, nb: 4, shapeless: false },
    Recipe { width: 3, cells: &[STICK, item(ItemId::Rope), STICK, STICK, STICK, STICK, None, item(ItemId::Rope), None], output: ItemId::Ladder, nb: 3, shapeless: false },
    Recipe { width: 3, cells: &[STICK, PLANKS, STICK, STICK, PLANKS, STICK], output: ItemId::Fence, nb: 3, shapeless: false },
    Recipe { width: 3, cells: &[item(ItemId::Glass), item(ItemId::Glass), item(ItemId::Glass)], output: ItemId::GlassPane, nb: 8, shapeless: false },
    Recipe { width: 2, cells: &[item(ItemId::Flint), item(ItemId::Rope), STICK, None, STICK, None], output: ItemId::Spear, nb: 1, shapeless: false },
    Recipe { width: 3, cells: &[item(ItemId::Rye), item(ItemId::Rye), item(ItemId::Rye)], output: ItemId::RyeBread, nb: 1, shapeless: false },
    Recipe { width: 3, cells: &[item(ItemId::Wool), item(ItemId::Wool), item(ItemId::Wool), item(ItemId::Rope), None, item(ItemId::Rope)], output: ItemId::Bedroll, nb: 1, shapeless: false },
    Recipe { width: 3, cells: &[PLANKS, item(ItemId::Rope), PLANKS, PLANKS, None, PLANKS, PLANKS, item(ItemId::Rope), PLANKS], output: ItemId::Barrel, nb: 1, shapeless: false },
    Recipe { width: 3, cells: &[item(ItemId::HewnStone), None, item(ItemId::HewnStone), item(ItemId::HewnStone), item(ItemId::Bucket), item(ItemId::HewnStone), item(ItemId::HewnStone), item(ItemId::HewnStone), item(ItemId::HewnStone)], output: ItemId::RainCistern, nb: 1, shapeless: false },
    Recipe { width: 3, cells: &[None, STICK, None, STICK, None, STICK, item(ItemId::OakLog), item(ItemId::OakLog), item(ItemId::OakLog)], output: ItemId::Campfire, nb: 1, shapeless: false },
    Recipe { width: 3, cells: &[None, STICK, None, STICK, None, STICK, item(ItemId::SpruceLog), item(ItemId::SpruceLog), item(ItemId::SpruceLog)], output: ItemId::Campfire, nb: 1, shapeless: false },
    Recipe { width: 3, cells: &[None, STICK, None, STICK, None, STICK, item(ItemId::JungleLog), item(ItemId::JungleLog), item(ItemId::JungleLog)], output: ItemId::Campfire, nb: 1, shapeless: false },
    Recipe { width: 3, cells: &[None, STICK, None, STICK, None, STICK, item(ItemId::WillowLog), item(ItemId::WillowLog), item(ItemId::WillowLog)], output: ItemId::Campfire, nb: 1, shapeless: false },
    Recipe { width: 3, cells: &[None, STICK, None, STICK, None, STICK, item(ItemId::AcaciaLog), item(ItemId::AcaciaLog), item(ItemId::AcaciaLog)], output: ItemId::Campfire, nb: 1, shapeless: false },
    Recipe { width: 3, cells: &[PLANKS, PLANKS, PLANKS, PLANKS, PLANKS, PLANKS, None, STICK, None], output: ItemId::Sign, nb: 1, shapeless: false },
    // A lens, a body of iron round a core of gold, and a handle
    Recipe {
        width: 3,
        cells: &[
            GLASS, IRON, IRON,
            None, GOLD, IRON,
            None, None, STICK,
        ],
        output: ItemId::PortalGun,
        nb: 1,
        shapeless: false,
    },
    Recipe {
        width: 2,
        cells: &[IRON, FLINT],
        output: ItemId::FlintAndSteel,
        nb: 1,
        shapeless: true,
    },
    Recipe {
        width: 3,
        cells: &[
            item(ItemId::CopperIngot), DIAMOND, item(ItemId::CopperIngot),
            item(ItemId::HewnStone), item(ItemId::Obsidian), item(ItemId::HewnStone),
            item(ItemId::HewnStone), None, item(ItemId::HewnStone),
        ],
        output: ItemId::EngravingTable,
        nb: 1,
        shapeless: false,
    },
    Recipe {
        width: 3,
        cells: &[
            item(ItemId::IronBlock), item(ItemId::IronBlock), item(ItemId::IronBlock),
            None, IRON, None,
            IRON, IRON, IRON,
        ],
        output: ItemId::Anvil,
        nb: 1,
        shapeless: false,
    },
    // Minecraft's boat, of any planks
    Recipe { width: 3, cells: &[PLANKS, None, PLANKS, PLANKS, PLANKS, PLANKS], output: ItemId::Boat, nb: 1, shapeless: false },
    // Minecraft has none: leather over an iron tree, its girth of rope
    Recipe {
        width: 3,
        cells: &[LEATHER, LEATHER, LEATHER, item(ItemId::Rope), IRON, item(ItemId::Rope)],
        output: ItemId::Saddle,
        nb: 1,
        shapeless: false,
    },
    // Minecraft's shield, of any planks around an iron boss
    Recipe { width: 3, cells: &[PLANKS, IRON, PLANKS, PLANKS, PLANKS, PLANKS, None, PLANKS, None], output: ItemId::Shield, nb: 1, shapeless: false },
    // Minecraft's crossbow, its tripwire hook an iron latch, its strings of rope
    Recipe {
        width: 3,
        cells: &[STICK, IRON, STICK, item(ItemId::Rope), IRON, item(ItemId::Rope), None, STICK, None],
        output: ItemId::Crossbow,
        nb: 1,
        shapeless: false,
    },
    Recipe { width: 1, cells: &[IRON, STICK, item(ItemId::Feather)], output: ItemId::Bolt, nb: 8, shapeless: false },
    Recipe { width: 2, cells: &[IRON, IRON, None, STICK], output: ItemId::ThrowingAxe, nb: 4, shapeless: false },
    Recipe { width: 2, cells: &[IRON, IRON, IRON, IRON, None, STICK], output: ItemId::Mace, nb: 1, shapeless: false },
    Recipe { width: 3, cells: &helmet(LEATHER), output: ItemId::LeatherHelmet, nb: 1, shapeless: false },
    Recipe { width: 3, cells: &chestplate(LEATHER), output: ItemId::LeatherChestplate, nb: 1, shapeless: false },
    Recipe { width: 3, cells: &leggings(LEATHER), output: ItemId::LeatherLeggings, nb: 1, shapeless: false },
    Recipe { width: 3, cells: &boots(LEATHER), output: ItemId::LeatherBoots, nb: 1, shapeless: false },
    Recipe { width: 3, cells: &helmet(BRONZE), output: ItemId::BronzeHelmet, nb: 1, shapeless: false },
    Recipe { width: 3, cells: &chestplate(BRONZE), output: ItemId::BronzeChestplate, nb: 1, shapeless: false },
    Recipe { width: 3, cells: &leggings(BRONZE), output: ItemId::BronzeLeggings, nb: 1, shapeless: false },
    Recipe { width: 3, cells: &boots(BRONZE), output: ItemId::BronzeBoots, nb: 1, shapeless: false },
    Recipe { width: 3, cells: &helmet(IRON), output: ItemId::IronHelmet, nb: 1, shapeless: false },
    Recipe { width: 3, cells: &chestplate(IRON), output: ItemId::IronChestplate, nb: 1, shapeless: false },
    Recipe { width: 3, cells: &leggings(IRON), output: ItemId::IronLeggings, nb: 1, shapeless: false },
    Recipe { width: 3, cells: &boots(IRON), output: ItemId::IronBoots, nb: 1, shapeless: false },
    Recipe { width: 3, cells: &helmet(DIAMOND), output: ItemId::DiamondHelmet, nb: 1, shapeless: false },
    Recipe { width: 3, cells: &chestplate(DIAMOND), output: ItemId::DiamondChestplate, nb: 1, shapeless: false },
    Recipe { width: 3, cells: &leggings(DIAMOND), output: ItemId::DiamondLeggings, nb: 1, shapeless: false },
    Recipe { width: 3, cells: &boots(DIAMOND), output: ItemId::DiamondBoots, nb: 1, shapeless: false },
    Recipe { width: 3, cells: &[STICK, STICK, None, item(ItemId::Rope), None, STICK, STICK, STICK, None], output: ItemId::Bow, nb: 1, shapeless: false },
    Recipe { width: 1, cells: &[FLINT, STICK, item(ItemId::Feather)], output: ItemId::Arrow, nb: 4, shapeless: false },
    Recipe { width: 3, cells: &[None, item(ItemId::GoldIngot), None, item(ItemId::GoldIngot), item(ItemId::Diamond), item(ItemId::GoldIngot), None, item(ItemId::GoldIngot), None], output: ItemId::DepthsCompass, nb: 1, shapeless: false },
    // The farm: Minecraft's rod, its line of rope
    Recipe { width: 3, cells: &[None, None, STICK, None, STICK, item(ItemId::Rope), STICK, None, item(ItemId::Rope)], output: ItemId::FishingRod, nb: 1, shapeless: false },
    Recipe { width: 3, cells: &[PLANKS, None, PLANKS, None, PLANKS, None], output: ItemId::Bowl, nb: 4, shapeless: false },
    Recipe { width: 2, cells: &[BOWL, item(ItemId::Mushroom), item(ItemId::Mushroom), None], output: ItemId::MushroomSoup, nb: 1, shapeless: true },
    Recipe { width: 3, cells: &[BOWL, item(ItemId::Carrot), item(ItemId::Potato), item(ItemId::Mushroom), Some(Ingredient::Tag(ItemTag::RoastMeat)), None], output: ItemId::Stew, nb: 1, shapeless: true },
    Recipe { width: 2, cells: &[BOWL, item(ItemId::Berries), item(ItemId::Berries), item(ItemId::Apple)], output: ItemId::BerrySalad, nb: 1, shapeless: true },
    Recipe { width: 2, cells: &[BOWL, item(ItemId::MilkBucket), Some(Ingredient::Tag(ItemTag::RawFish)), item(ItemId::Potato)], output: ItemId::FishChowder, nb: 1, shapeless: true },
    Recipe { width: 3, cells: &[item(ItemId::MilkBucket), item(ItemId::Egg), item(ItemId::CocoaBeans), item(ItemId::Rye), item(ItemId::Rye), None], output: ItemId::CakeBatter, nb: 1, shapeless: true },
    Recipe { width: 2, cells: &[item(ItemId::Apple), item(ItemId::Apple), item(ItemId::Egg), item(ItemId::Rye)], output: ItemId::RawPie, nb: 1, shapeless: true },
    Recipe { width: 2, cells: &[item(ItemId::Fiber), item(ItemId::Fiber), item(ItemId::Dirt), None], output: ItemId::Compost, nb: 2, shapeless: true },
    // The oceans and the deep caves
    Recipe { width: 2, cells: &[SHARD, SHARD, SHARD, SHARD], output: ItemId::Glowcrystal, nb: 1, shapeless: false },
    Recipe { width: 2, cells: &[BASALT, BASALT, BASALT, BASALT], output: ItemId::PolishedBasalt, nb: 4, shapeless: false },
    Recipe { width: 3, cells: &[KELP, KELP, KELP], output: ItemId::Paper, nb: 2, shapeless: false },
];

const SHARD: Option<Ingredient> = item(ItemId::CrystalShard);
const BASALT: Option<Ingredient> = item(ItemId::Basalt);
const KELP: Option<Ingredient> = item(ItemId::Kelp);

/// The dyes a plant, a fuel or an ore gives, and how many: the green one is smelted from a
/// cactus (`furnace::smelting_result`).
const DYE_SOURCES: [(ItemId, ItemId, u32); 9] = [
    (ItemId::Poppy, ItemId::RedDye, 2),
    (ItemId::Berries, ItemId::RedDye, 1),
    (ItemId::Dandelion, ItemId::YellowDye, 2),
    (ItemId::CocoaBeans, ItemId::BrownDye, 1),
    (ItemId::Heather, ItemId::MagentaDye, 2),
    (ItemId::Coal, ItemId::BlackDye, 1),
    (ItemId::Charcoal, ItemId::BlackDye, 1),
    (ItemId::Limestone, ItemId::WhiteDye, 2),
    // its azurite
    (ItemId::CopperOre, ItemId::BlueDye, 2),
];

/// The dyes mixed from two others, two at a time.
const DYE_MIXES: [(ItemId, ItemId, ItemId); 9] = [
    (ItemId::RedDye, ItemId::YellowDye, ItemId::OrangeDye),
    (ItemId::RedDye, ItemId::WhiteDye, ItemId::PinkDye),
    (ItemId::GreenDye, ItemId::WhiteDye, ItemId::LimeDye),
    (ItemId::BlueDye, ItemId::WhiteDye, ItemId::LightBlueDye),
    (ItemId::BlueDye, ItemId::GreenDye, ItemId::CyanDye),
    (ItemId::RedDye, ItemId::BlueDye, ItemId::PurpleDye),
    (ItemId::PurpleDye, ItemId::PinkDye, ItemId::MagentaDye),
    (ItemId::BlackDye, ItemId::WhiteDye, ItemId::GrayDye),
    (ItemId::GrayDye, ItemId::WhiteDye, ItemId::LightGrayDye),
];

/// The recipes of the palette (`palette`), which follow those of `RECIPES`: the slabs, stairs,
/// doors, trapdoors and fence gates of each wood from its own planks, the slabs, stairs and low
/// walls of each rock, its polished and chiseled blocks, the dyes and what they colour.
fn palette_recipes() -> Vec<Recipe> {
    let mut recipes = Vec::new();
    let mut add = |width, cells: Vec<Option<Ingredient>>, output, nb, shapeless| {
        recipes.push(Recipe {
            width,
            cells: Vec::leak(cells),
            output,
            nb,
            shapeless,
        })
    };
    for wood in Wood::ALL {
        let p = item(wood.planks());
        add(3, vec![p; 3], wood.slab(), 6, false);
        add(2, vec![p, None, p, p, p, p], wood.stairs(), 4, false);
        add(2, vec![p; 6], wood.door(), 1, false);
        add(3, vec![p; 6], wood.trapdoor(), 2, false);
        add(3, vec![p, STICK, p, p, STICK, p], wood.fence_gate(), 1, false);
    }
    for rock in Rock::ALL {
        let m = item(rock.material());
        // the slabs and the stairs of hewn stone and of bricks are in `RECIPES`
        if !matches!(rock, Rock::HewnStone | Rock::Brick) {
            add(3, vec![m; 3], rock.slab(), 6, false);
            add(2, vec![m, None, m, m, m, m], rock.stairs(), 4, false);
        }
        add(3, vec![m; 6], rock.low_wall(), 6, false);
        if let Some((polished, chiseled)) = rock.polished_and_chiseled() {
            add(2, vec![m; 4], polished, 4, false);
            add(1, vec![item(rock.slab()); 2], chiseled, 1, false);
        }
    }
    for (source, dye, nb) in DYE_SOURCES {
        add(1, vec![item(source)], dye, nb, false);
    }
    for (a, b, dye) in DYE_MIXES {
        add(2, vec![item(a), item(b)], dye, 2, true);
    }
    // eight blocks around a dye
    let ring = |block: ItemId, dye: ItemId| {
        let mut cells = vec![item(block); 9];
        cells[4] = item(dye);
        cells
    };
    for color in DyeColor::ALL {
        let dye = color.dye();
        add(2, vec![item(dye), WOOL], color.wool(), 1, true);
        add(3, ring(ItemId::Glass, dye), color.stained_glass(), 8, false);
        add(3, vec![item(color.stained_glass()); 3], color.stained_glass_pane(), 8, false);
        add(3, ring(ItemId::Terracotta, dye), color.terracotta(), 8, false);
        let mut powder = vec![item(dye)];
        powder.extend([item(ItemId::Sand); 4]);
        powder.extend([item(ItemId::Gravel); 4]);
        add(3, powder, color.concrete_powder(), 8, true);
    }
    add(2, vec![item(ItemId::Clay), item(ItemId::FiredBrick)], ItemId::Terracotta, 2, true);
    recipes
}

/// The recipes of the maps (`cartography`): paper from reeds, the empty map, its copy, the
/// compass and the frame of a map.
#[rustfmt::skip]
const MAP_RECIPES: &[Recipe] = &[
    Recipe { width: 3, cells: &[item(ItemId::Reed), item(ItemId::Reed), item(ItemId::Reed)], output: ItemId::Paper, nb: 3, shapeless: false },
    Recipe { width: 3, cells: &[PAPER, PAPER, PAPER, PAPER, COAL, PAPER, PAPER, PAPER, PAPER], output: ItemId::EmptyMap, nb: 1, shapeless: false },
    Recipe { width: 3, cells: &[PAPER, PAPER, PAPER, PAPER, CHARCOAL, PAPER, PAPER, PAPER, PAPER], output: ItemId::EmptyMap, nb: 1, shapeless: false },
    // A copy: the map drawn stays in the grid (`inventory::take_craft_result`)
    Recipe { width: 2, cells: &[item(ItemId::FilledMap), item(ItemId::EmptyMap)], output: ItemId::FilledMap, nb: 1, shapeless: true },
    Recipe { width: 3, cells: &[None, IRON, None, IRON, item(ItemId::CopperIngot), IRON, None, IRON, None], output: ItemId::Compass, nb: 1, shapeless: false },
    Recipe { width: 3, cells: &[STICK, STICK, STICK, STICK, LEATHER, STICK, STICK, STICK, STICK], output: ItemId::MapFrame, nb: 1, shapeless: false },
];

const PAPER: Option<Ingredient> = item(ItemId::Paper);

/// Whether the grid makes a copy of a paper map, which keeps the map in its cell.
pub fn copies_a_map(grid: &CraftingGrid) -> bool {
    grid.iter().flatten().any(|&cell| cell == ItemId::FilledMap)
        && match_recipe(grid).is_some_and(|result| result.item_id == ItemId::FilledMap)
}

/// Every recipe: those of `RECIPES`, then those of the palette, then those of the maps. Made
/// once.
fn all_recipes() -> &'static [Recipe] {
    static ALL: OnceLock<Vec<Recipe>> = OnceLock::new();
    ALL.get_or_init(|| {
        RECIPES
            .iter()
            .copied()
            .chain(palette_recipes())
            .chain(MAP_RECIPES.iter().copied())
            .collect()
    })
}

/// What the grid makes, if it matches a recipe.
pub fn match_recipe(grid: &CraftingGrid) -> Option<ItemStack> {
    let size = CRAFTING_GRID_SIZE as usize;
    let cell = |row: usize, col: usize| grid[row * size + col];

    // The bounding box of the filled cells
    let rows: Vec<usize> = (0..size)
        .filter(|&row| (0..size).any(|col| cell(row, col).is_some()))
        .collect();
    let cols: Vec<usize> = (0..size)
        .filter(|&col| (0..size).any(|row| cell(row, col).is_some()))
        .collect();
    let (&top, &bottom, &left, &right) = (rows.first()?, rows.last()?, cols.first()?, cols.last()?);
    let (height, width) = (bottom - top + 1, right - left + 1);

    let matches = |recipe: &Recipe, mirrored: bool| {
        (0..height).all(|row| {
            (0..width).all(|col| {
                let recipe_col = if mirrored { width - 1 - col } else { col };
                match (
                    recipe.cells[row * width + recipe_col],
                    cell(top + row, left + col),
                ) {
                    (Some(ingredient), Some(item)) => ingredient.matches(item),
                    (None, None) => true,
                    _ => false,
                }
            })
        })
    };
    let filled: Vec<ItemId> = grid.iter().flatten().copied().collect();
    let recipe = all_recipes().iter().find(|recipe| {
        if recipe.shapeless {
            return matches_shapeless(recipe, &filled);
        }
        recipe.width == width
            && recipe.cells.len() == width * height
            && (matches(recipe, false) || matches(recipe, true))
    })?;
    Some(ItemStack {
        item_id: recipe.output,
        item_type: recipe.output.get_default_type(),
        nb: recipe.nb,
    })
}

/// Whether the items `filled` in a grid are the ingredients of the shapeless `recipe`, one for
/// each, in any order.
fn matches_shapeless(recipe: &Recipe, filled: &[ItemId]) -> bool {
    let ingredients: Vec<Ingredient> = recipe.cells.iter().flatten().copied().collect();
    if ingredients.len() != filled.len() {
        return false;
    }
    let mut used = vec![false; filled.len()];
    // An ingredient takes the first item left it matches: the recipes have no family that
    // another ingredient of theirs is in, so no other choice could match where this fails
    ingredients.iter().all(|ingredient| {
        let found = (0..filled.len()).find(|&i| !used[i] && ingredient.matches(filled[i]));
        found.inspect(|&i| used[i] = true).is_some()
    })
}

/// A recipe as the item browser reads it. Its index is its position in the table, stable for a
/// protocol version.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RecipeView {
    pub index: u16,
    pub width: usize,
    pub height: usize,
    /// Its cells, row after row.
    pub cells: &'static [Option<Ingredient>],
    pub output: ItemStack,
}

impl RecipeView {
    /// Whether the recipe fits a crafting grid of `grid_size` by `grid_size`.
    pub fn fits(&self, grid_size: u32) -> bool {
        self.width <= grid_size as usize && self.height <= grid_size as usize
    }

    /// The ingredients of the recipe, one per cell, in the order of their first cell.
    pub fn ingredients(&self) -> Vec<(Ingredient, u32)> {
        let mut ingredients: Vec<(Ingredient, u32)> = Vec::new();
        for ingredient in self.cells.iter().flatten() {
            match ingredients
                .iter_mut()
                .find(|(other, _)| other == ingredient)
            {
                Some((_, count)) => *count += 1,
                None => ingredients.push((*ingredient, 1)),
            }
        }
        ingredients
    }
}

/// Every recipe, in the order of the table.
pub fn recipes() -> impl Iterator<Item = RecipeView> {
    all_recipes()
        .iter()
        .enumerate()
        .map(|(index, recipe)| RecipeView {
            index: index as u16,
            width: recipe.width,
            height: recipe.cells.len() / recipe.width,
            cells: recipe.cells,
            output: ItemStack {
                item_id: recipe.output,
                item_type: recipe.output.get_default_type(),
                nb: recipe.nb,
            },
        })
}

/// The recipes making `item`.
pub fn recipes_for(item: ItemId) -> impl Iterator<Item = RecipeView> {
    recipes().filter(move |recipe| recipe.output.item_id == item)
}

/// The recipes `item` goes into: those with a cell taking it.
pub fn uses_of(item: ItemId) -> impl Iterator<Item = RecipeView> {
    recipes().filter(move |recipe| {
        recipe
            .cells
            .iter()
            .flatten()
            .any(|ingredient| ingredient.matches(item))
    })
}

/// What `inventory` lacks to make `recipe` once, in the order of its ingredients, the items of
/// a family counted together. Its slots and its crafting grid count; a chest, and the stack
/// held by the cursor, do not.
pub fn missing(inventory: &Inventory, recipe: &RecipeView) -> Vec<(Ingredient, u32)> {
    let grid = CRAFTING_GRID_START..CRAFTING_GRID_START + CRAFTING_GRID_SIZE * CRAFTING_GRID_SIZE;
    let held = |ingredient: Ingredient| -> u32 {
        inventory
            .inner
            .iter()
            .filter(|(slot, stack)| {
                (**slot < MAX_INVENTORY_SLOTS || grid.contains(slot))
                    && ingredient.matches(stack.item_id)
            })
            .map(|(_, stack)| stack.nb)
            .sum()
    };
    recipe
        .ingredients()
        .into_iter()
        .filter_map(|(ingredient, needed)| {
            let lacking = needed.saturating_sub(held(ingredient));
            (lacking > 0).then_some((ingredient, lacking))
        })
        .collect()
}

/// Whether `inventory` holds the ingredients of `recipe`, counted as `missing` does.
pub fn craftable(inventory: &Inventory, recipe: &RecipeView) -> bool {
    missing(inventory, recipe).is_empty()
}

/// What filling the grid with a recipe puts into it: `rounds` items of one item in each cell.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FillPlan {
    /// How many items each cell gets.
    pub rounds: u32,
    /// The item each cell of the recipe gets, row after row, `None` for its empty cells.
    pub cells: Vec<Option<ItemId>>,
}

/// The most rounds a fill makes: a stack in each cell.
const MAX_FILL_ROUNDS: u32 = 64;

/// How `inventory` fills the grid with `recipe`, `times` rounds at most, from its slots alone
/// (not the grid, which the fill puts back first, nor a chest or the cursor): the most rounds,
/// up to a stack, for which every cell can get that many of one item. A cell of a family takes
/// the item the previous cell of that family took while it holds enough, else the one the
/// inventory holds the most of after the cells already served, the first of the family on a
/// tie: one species is used whenever it is enough. `None` without one round.
pub fn plan_fill(inventory: &Inventory, recipe: &RecipeView, times: u32) -> Option<FillPlan> {
    let held = held_items(inventory);
    (1..=times.min(MAX_FILL_ROUNDS)).rev().find_map(|rounds| {
        let cells = serve_cells(held.clone(), recipe, rounds)
            .into_iter()
            .map(|cell| match cell {
                Cell::Empty => Some(None),
                Cell::Served(item) => Some(Some(item)),
                Cell::Lacking => None,
            })
            .collect::<Option<Vec<_>>>()?;
        Some(FillPlan { rounds, cells })
    })
}

/// The cells of `recipe` that one round leaves without their ingredient, row after row, served
/// from the slots of `inventory` as `plan_fill` serves them: `true` for a cell the inventory has
/// nothing left for once the cells before it are served.
pub fn lacking_cells(inventory: &Inventory, recipe: &RecipeView) -> Vec<bool> {
    serve_cells(held_items(inventory), recipe, 1)
        .into_iter()
        .map(|cell| cell == Cell::Lacking)
        .collect()
}

/// How many of each item the slots `0..36` of `inventory` hold, in the order first met.
fn held_items(inventory: &Inventory) -> Vec<(ItemId, u32)> {
    let mut held: Vec<(ItemId, u32)> = Vec::new();
    for stack in (0..MAX_INVENTORY_SLOTS).filter_map(|slot| inventory.inner.get(&slot)) {
        match held.iter_mut().find(|(item, _)| *item == stack.item_id) {
            Some((_, nb)) => *nb += stack.nb,
            None => held.push((stack.item_id, stack.nb)),
        }
    }
    held
}

/// What a cell of a recipe gets in a fill.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Cell {
    /// The recipe leaves it empty.
    Empty,
    /// It gets the rounds of this item.
    Served(ItemId),
    /// Nothing left serves it.
    Lacking,
}

/// How the items `left` serve each cell of `recipe` with `rounds` items of one item, in the
/// order of the cells; a cell they cannot serve takes nothing.
fn serve_cells(mut left: Vec<(ItemId, u32)>, recipe: &RecipeView, rounds: u32) -> Vec<Cell> {
    let count = |left: &[(ItemId, u32)], item: ItemId| {
        left.iter()
            .find(|(other, _)| *other == item)
            .map_or(0, |(_, nb)| *nb)
    };
    // The item the last cell of each ingredient took
    let mut last: Vec<(Ingredient, ItemId)> = Vec::new();
    let mut cells = Vec::with_capacity(recipe.cells.len());
    for cell in recipe.cells {
        let Some(ingredient) = *cell else {
            cells.push(Cell::Empty);
            continue;
        };
        let previous = last
            .iter()
            .find(|(other, _)| *other == ingredient)
            .map(|(_, item)| *item)
            .filter(|&item| count(&left, item) >= rounds);
        // the first of the most held
        let most = ingredient.members().iter().copied().reduce(|best, item| {
            if count(&left, item) > count(&left, best) {
                item
            } else {
                best
            }
        });
        let Some(item) = previous.or(most) else {
            cells.push(Cell::Lacking);
            continue;
        };
        if count(&left, item) < rounds || item.get_max_stack() < rounds {
            cells.push(Cell::Lacking);
            continue;
        }
        if let Some((_, nb)) = left.iter_mut().find(|(other, _)| *other == item) {
            *nb -= rounds;
        }
        match last.iter_mut().find(|(other, _)| *other == ingredient) {
            Some((_, taken)) => *taken = item,
            None => last.push((ingredient, item)),
        }
        cells.push(Cell::Served(item));
    }
    cells
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::players::Inventory;

    const LOG: Option<ItemId> = Some(ItemId::OakLog);
    const OAK: Option<ItemId> = Some(ItemId::OakPlanks);
    const SPRUCE: Option<ItemId> = Some(ItemId::SprucePlanks);
    const JUNGLE: Option<ItemId> = Some(ItemId::JunglePlanks);
    const COBBLE: Option<ItemId> = Some(ItemId::Cobblestone);

    fn grid(cells: [Option<ItemId>; 9]) -> CraftingGrid {
        cells
    }

    #[test]
    fn a_map_is_copied_with_an_empty_one_and_reeds_make_paper() {
        let (m, e) = (Some(ItemId::FilledMap), Some(ItemId::EmptyMap));
        let copy = grid([e, None, None, None, m, None, None, None, None]);
        assert_eq!(
            match_recipe(&copy).map(|r| r.item_id),
            Some(ItemId::FilledMap)
        );
        assert!(copies_a_map(&copy));
        assert!(!copies_a_map(&grid([
            e, None, None, None, None, None, None, None, None
        ])));
        let r = Some(ItemId::Reed);
        let paper = grid([r, r, r, None, None, None, None, None, None]);
        assert_eq!(
            match_recipe(&paper).map(|r| (r.item_id, r.nb)),
            Some((ItemId::Paper, 3))
        );
    }

    fn made(item_id: ItemId, nb: u32) -> Option<ItemStack> {
        Some(ItemStack {
            item_id,
            item_type: item_id.get_default_type(),
            nb,
        })
    }

    #[test]
    fn a_log_makes_four_planks() {
        assert_eq!(
            match_recipe(&grid([LOG, None, None, None, None, None, None, None, None])),
            made(ItemId::OakPlanks, 4)
        );
        let spruce_log = Some(ItemId::SpruceLog);
        assert_eq!(
            match_recipe(&grid([
                spruce_log, None, None, None, None, None, None, None, None
            ])),
            made(ItemId::SprucePlanks, 4)
        );
        let jungle_log = Some(ItemId::JungleLog);
        assert_eq!(
            match_recipe(&grid([
                jungle_log, None, None, None, None, None, None, None, None
            ])),
            made(ItemId::JunglePlanks, 4)
        );
    }

    #[test]
    fn two_planks_one_above_the_other_make_four_sticks() {
        for planks in [OAK, SPRUCE, JUNGLE] {
            assert_eq!(
                match_recipe(&grid([
                    planks, None, None, planks, None, None, None, None, None
                ])),
                made(ItemId::Stick, 4)
            );
        }
    }

    #[test]
    fn coal_on_a_stick_makes_four_torches() {
        let coal = Some(ItemId::Coal);
        let stick = Some(ItemId::Stick);
        assert_eq!(
            match_recipe(&grid([
                coal, None, None, stick, None, None, None, None, None
            ])),
            made(ItemId::Torch, 4)
        );
        assert_eq!(
            match_recipe(&grid([
                stick, None, None, coal, None, None, None, None, None
            ])),
            None
        );
    }

    #[test]
    fn four_planks_in_a_square_make_a_crafting_table() {
        for planks in [OAK, SPRUCE, JUNGLE] {
            assert_eq!(
                match_recipe(&grid([
                    planks, planks, None, planks, planks, None, None, None, None
                ])),
                made(ItemId::CraftingTable, 1)
            );
        }
    }

    #[test]
    fn a_ring_of_cobblestone_makes_a_furnace() {
        assert_eq!(
            match_recipe(&grid([
                COBBLE, COBBLE, COBBLE, COBBLE, None, COBBLE, COBBLE, COBBLE, COBBLE
            ])),
            made(ItemId::Furnace, 1)
        );
    }

    #[test]
    fn a_ring_of_planks_makes_a_chest() {
        for planks in [OAK, SPRUCE, JUNGLE] {
            assert_eq!(
                match_recipe(&grid([
                    planks, planks, planks, planks, None, planks, planks, planks, planks
                ])),
                made(ItemId::Chest, 1)
            );
        }
        assert_eq!(
            match_recipe(&grid([OAK, OAK, OAK, JUNGLE, None, OAK, OAK, OAK, SPRUCE])),
            made(ItemId::Chest, 1)
        );
    }

    #[test]
    fn a_recipe_matches_anywhere_in_the_grid() {
        assert_eq!(
            match_recipe(&grid([None, None, None, None, None, None, None, None, LOG])),
            made(ItemId::OakPlanks, 4)
        );
        assert_eq!(
            match_recipe(&grid([None, None, None, None, OAK, OAK, None, OAK, OAK])),
            made(ItemId::CraftingTable, 1)
        );
        assert_eq!(
            match_recipe(&grid([None, None, OAK, None, None, OAK, None, None, None])),
            made(ItemId::Stick, 4)
        );
    }

    #[test]
    fn planks_side_by_side_make_nothing() {
        assert_eq!(
            match_recipe(&grid([OAK, OAK, None, None, None, None, None, None, None])),
            None
        );
    }

    #[test]
    fn planks_of_any_kind_one_above_the_other_make_four_sticks() {
        assert_eq!(
            match_recipe(&grid([
                OAK, None, None, SPRUCE, None, None, None, None, None
            ])),
            made(ItemId::Stick, 4)
        );
        assert_eq!(
            match_recipe(&grid([
                None, None, None, None, JUNGLE, None, None, OAK, None
            ])),
            made(ItemId::Stick, 4)
        );
    }

    #[test]
    fn an_extra_item_makes_nothing() {
        assert_eq!(
            match_recipe(&grid([
                LOG, None, None, None, None, None, None, None, COBBLE
            ])),
            None
        );
        assert_eq!(
            match_recipe(&grid([
                COBBLE, COBBLE, COBBLE, COBBLE, COBBLE, COBBLE, COBBLE, COBBLE, COBBLE
            ])),
            None
        );
    }

    #[test]
    fn an_empty_grid_makes_nothing() {
        assert_eq!(match_recipe(&[None; 9]), None);
    }

    #[test]
    fn the_grid_starts_after_the_inventory() {
        assert_eq!(CRAFTING_GRID_START, MAX_INVENTORY_SLOTS);
        assert_eq!(grid_slot(0, 0), MAX_INVENTORY_SLOTS);
        assert_eq!(grid_slot(2, 1), MAX_INVENTORY_SLOTS + 7);
    }

    const STICK: Option<ItemId> = Some(ItemId::Stick);

    #[test]
    fn planks_and_sticks_make_the_wooden_tools() {
        for p in [OAK, SPRUCE, JUNGLE] {
            for (cells, tool) in [
                (
                    [p, p, p, None, STICK, None, None, STICK, None],
                    ItemId::WoodenPickaxe,
                ),
                (
                    [p, p, None, p, STICK, None, None, STICK, None],
                    ItemId::WoodenAxe,
                ),
                (
                    [p, None, None, STICK, None, None, STICK, None, None],
                    ItemId::WoodenShovel,
                ),
                (
                    [p, p, None, None, STICK, None, None, STICK, None],
                    ItemId::WoodenHoe,
                ),
                (
                    [p, None, None, p, None, None, STICK, None, None],
                    ItemId::WoodenSword,
                ),
            ] {
                assert_eq!(match_recipe(&grid(cells)), made(tool, 1), "{tool:?}");
            }
        }
    }

    #[test]
    fn cobblestone_ingots_and_diamonds_make_the_tools_of_their_tier() {
        for (m, [pickaxe, axe, shovel, hoe, sword]) in [
            (
                COBBLE,
                [
                    ItemId::StonePickaxe,
                    ItemId::StoneAxe,
                    ItemId::StoneShovel,
                    ItemId::StoneHoe,
                    ItemId::StoneSword,
                ],
            ),
            (
                Some(ItemId::IronIngot),
                [
                    ItemId::IronPickaxe,
                    ItemId::IronAxe,
                    ItemId::IronShovel,
                    ItemId::IronHoe,
                    ItemId::IronSword,
                ],
            ),
            (
                Some(ItemId::GoldIngot),
                [
                    ItemId::GoldenPickaxe,
                    ItemId::GoldenAxe,
                    ItemId::GoldenShovel,
                    ItemId::GoldenHoe,
                    ItemId::GoldenSword,
                ],
            ),
            (
                Some(ItemId::Diamond),
                [
                    ItemId::DiamondPickaxe,
                    ItemId::DiamondAxe,
                    ItemId::DiamondShovel,
                    ItemId::DiamondHoe,
                    ItemId::DiamondSword,
                ],
            ),
        ] {
            for (cells, tool) in [
                ([m, m, m, None, STICK, None, None, STICK, None], pickaxe),
                ([m, m, None, m, STICK, None, None, STICK, None], axe),
                (
                    [m, None, None, STICK, None, None, STICK, None, None],
                    shovel,
                ),
                ([m, m, None, None, STICK, None, None, STICK, None], hoe),
                ([m, None, None, m, None, None, STICK, None, None], sword),
            ] {
                assert_eq!(match_recipe(&grid(cells)), made(tool, 1), "{tool:?}");
            }
        }
    }

    #[test]
    fn nine_ingots_or_diamonds_make_a_block_which_gives_them_back() {
        for (material, block) in [
            (ItemId::IronIngot, ItemId::IronBlock),
            (ItemId::GoldIngot, ItemId::GoldBlock),
            (ItemId::Diamond, ItemId::DiamondBlock),
        ] {
            assert_eq!(match_recipe(&grid([Some(material); 9])), made(block, 1));
            let alone = [Some(block), None, None, None, None, None, None, None, None];
            assert_eq!(match_recipe(&grid(alone)), made(material, 9));
            // eight are not enough
            let mut eight = [Some(material); 9];
            eight[4] = None;
            assert_eq!(match_recipe(&grid(eight)), None, "{material:?}");
        }
    }

    #[test]
    fn charcoal_on_a_stick_makes_four_torches_as_coal() {
        let charcoal = Some(ItemId::Charcoal);
        assert_eq!(
            match_recipe(&grid([
                charcoal, None, None, STICK, None, None, None, None, None
            ])),
            made(ItemId::Torch, 4)
        );
    }

    #[test]
    fn three_iron_ingots_in_a_v_make_a_bucket() {
        let iron = Some(ItemId::IronIngot);
        for v in [
            [iron, None, iron, None, iron, None, None, None, None],
            [None, None, None, iron, None, iron, None, iron, None],
        ] {
            assert_eq!(match_recipe(&grid(v)), made(ItemId::Bucket, 1));
        }
        // upside down, it is nothing
        assert_eq!(
            match_recipe(&grid([
                None, iron, None, iron, None, iron, None, None, None
            ])),
            None
        );
        // three wide, it needs a crafting table
        assert!(!recipe_of(ItemId::Bucket).fits(2));
    }

    #[test]
    fn an_iron_ingot_and_a_flint_anywhere_make_a_flint_and_steel() {
        let (iron, flint) = (Some(ItemId::IronIngot), Some(ItemId::Flint));
        for cells in [
            [iron, flint, None, None, None, None, None, None, None],
            [flint, iron, None, None, None, None, None, None, None],
            [None, None, flint, None, None, None, None, iron, None],
            [iron, None, None, None, None, None, None, None, flint],
            [None, flint, None, iron, None, None, None, None, None],
        ] {
            assert_eq!(
                match_recipe(&grid(cells)),
                made(ItemId::FlintAndSteel, 1),
                "{cells:?}"
            );
        }
        // not with one more ingredient, nor one less
        assert_eq!(
            match_recipe(&grid([
                iron, flint, flint, None, None, None, None, None, None
            ])),
            None
        );
        assert_eq!(
            match_recipe(&grid([
                flint, None, None, None, None, None, None, None, None
            ])),
            None
        );
        // it fits the grid of the inventory, as a row of two
        let recipe = recipe_of(ItemId::FlintAndSteel);
        assert!(recipe.fits(2));
        assert_eq!(recipe.cells, &[IRON, FLINT]);
    }

    #[test]
    fn a_recipe_also_matches_mirrored_left_to_right() {
        assert_eq!(
            match_recipe(&grid([None, OAK, OAK, None, STICK, OAK, None, STICK, None])),
            made(ItemId::WoodenAxe, 1)
        );
        assert_eq!(
            match_recipe(&grid([
                OAK, OAK, None, STICK, None, None, STICK, None, None
            ])),
            made(ItemId::WoodenHoe, 1)
        );
        // but not upside down
        assert_eq!(
            match_recipe(&grid([None, STICK, None, None, STICK, None, OAK, OAK, OAK])),
            None
        );
    }

    #[test]
    fn the_dishes_fill_more_than_their_ingredients() {
        for recipe in recipes() {
            let Some((dish, _)) = recipe.output.item_id.food_value() else {
                continue;
            };
            let eaten: u32 = recipe
                .ingredients()
                .iter()
                .map(|(ingredient, nb)| {
                    let best = ingredient.members().iter().filter_map(|i| i.food_value());
                    best.map(|(food, _)| food as u32).max().unwrap_or(0) * nb
                })
                .sum();
            assert!(u32::from(dish) * recipe.output.nb > eaten, "{recipe:?}");
        }
    }

    #[test]
    fn every_recipe_is_listed_with_its_index_and_size() {
        let all: Vec<RecipeView> = recipes().collect();
        // + 3: the glowing crystal, the polished basalt and the paper of kelp
        assert_eq!(all.len(), 109 - 3 + 152 + 5 + 9 + 6 + 3);
        for (position, recipe) in all.iter().enumerate() {
            assert_eq!(recipe.index as usize, position);
            assert_eq!(recipe.cells.len(), recipe.width * recipe.height);
            // what the grid makes when it holds the recipe in its top left corner, each cell
            // holding the first item of its ingredient
            let grid = grid_of(recipe, false, |ingredient| ingredient.members()[0]);
            assert_eq!(match_recipe(&grid), Some(recipe.output), "{recipe:?}");
        }
        let furnace = all.iter().find(|r| r.output.item_id == ItemId::Furnace);
        assert_eq!(furnace.map(|r| (r.width, r.height)), Some((3, 3)));
        assert_eq!((all[3].width, all[3].height), (1, 2));
        assert_eq!(all[3].output, made(ItemId::Stick, 4).unwrap());
    }

    #[test]
    fn recipes_for_and_uses_of_find_the_recipes_of_an_item() {
        let counts = |item| (recipes_for(item).count(), uses_of(item).count());
        assert_eq!(counts(ItemId::WoodenPickaxe), (1, 0));
        assert_eq!(counts(ItemId::Stick), (1, 55));
        // the recipes of any planks (the shield and the bowl among them), then the slab, stairs, door, trapdoor and gate of the species
        assert_eq!(counts(ItemId::OakPlanks), (1, 14 + 5));
        assert_eq!(counts(ItemId::SprucePlanks), (1, 14 + 5));
        assert_eq!(counts(ItemId::JunglePlanks), (1, 14 + 5));
        let uses = |item| uses_of(item).map(|r| r.index).collect::<Vec<_>>();
        assert_eq!(uses(ItemId::OakPlanks)[..14], uses(ItemId::SprucePlanks)[..14]);
        assert_eq!(uses(ItemId::OakPlanks)[..14], uses(ItemId::JunglePlanks)[..14]);
        assert_ne!(uses(ItemId::OakPlanks)[14..], uses(ItemId::SprucePlanks)[14..]);
        // the tools, the furnace, and the slab, stairs and low wall
        assert_eq!(counts(ItemId::Cobblestone), (0, 6 + 3));
        assert_eq!(counts(ItemId::IronIngot), (1, 22));
        assert_eq!(counts(ItemId::Torch), (2, 1));
        assert_eq!(counts(ItemId::Bedrock), (0, 0));
        assert!(recipes_for(ItemId::Chest).all(|r| r.output.item_id == ItemId::Chest));
        assert!(uses_of(ItemId::Stick)
            .all(|r| r.cells.contains(&Some(Ingredient::Item(ItemId::Stick)))));
    }

    /// What the grid holding `cells`, row after row in a grid `width` wide, makes.
    fn made_of(width: usize, cells: &[Option<ItemId>]) -> Option<ItemStack> {
        let mut grid = [None; 9];
        for (index, cell) in cells.iter().enumerate() {
            grid[index / width * 3 + index % width] = *cell;
        }
        match_recipe(&grid)
    }

    #[test]
    fn each_wood_makes_its_own_slabs_stairs_doors_trapdoors_and_gates() {
        for wood in Wood::ALL {
            let p = Some(wood.planks());
            let s = Some(ItemId::Stick);
            assert_eq!(made_of(3, &[p, p, p]), made(wood.slab(), 6), "{wood:?}");
            assert_eq!(made_of(2, &[p, None, p, p, p, p]), made(wood.stairs(), 4));
            assert_eq!(made_of(2, &[p; 6]), made(wood.door(), 1));
            assert_eq!(made_of(3, &[p; 6]), made(wood.trapdoor(), 2));
            assert_eq!(made_of(3, &[p, s, p, p, s, p]), made(wood.fence_gate(), 1));
        }
        // the oak ones are those there were before
        assert_eq!(Wood::Oak.slab(), ItemId::OakSlab);
        assert_eq!(Wood::Oak.door(), ItemId::Door);
        // a slab of spruce takes spruce only
        let (oak, spruce) = (Some(ItemId::OakPlanks), Some(ItemId::SprucePlanks));
        assert_eq!(made_of(3, &[oak, spruce, oak]), None);
    }

    #[test]
    fn each_rock_makes_slabs_stairs_low_walls_and_its_polished_and_chiseled_blocks() {
        for rock in Rock::ALL {
            let m = Some(rock.material());
            assert_eq!(made_of(3, &[m, m, m]), made(rock.slab(), 6), "{rock:?}");
            assert_eq!(made_of(2, &[m, None, m, m, m, m]), made(rock.stairs(), 4));
            assert_eq!(made_of(3, &[m; 6]), made(rock.low_wall(), 6));
            if let Some((polished, chiseled)) = rock.polished_and_chiseled() {
                assert_eq!(made_of(2, &[m; 4]), made(polished, 4));
                let slab = Some(rock.slab());
                assert_eq!(made_of(1, &[slab, slab]), made(chiseled, 1));
            }
        }
        assert!(Rock::Gloomrock.polished_and_chiseled().is_some());
    }

    #[test]
    fn plants_give_dyes_which_mix_and_colour_wool_glass_terracotta_and_concrete() {
        let one = |item| made_of(1, &[Some(item)]);
        assert_eq!(one(ItemId::Poppy), made(ItemId::RedDye, 2));
        assert_eq!(one(ItemId::Dandelion), made(ItemId::YellowDye, 2));
        // the cocoa beans, of no use before
        assert_eq!(one(ItemId::CocoaBeans), made(ItemId::BrownDye, 1));
        let mix = |a, b| made_of(2, &[Some(b), Some(a)]);
        assert_eq!(mix(ItemId::RedDye, ItemId::YellowDye), made(ItemId::OrangeDye, 2));
        assert_eq!(mix(ItemId::BlueDye, ItemId::GreenDye), made(ItemId::CyanDye, 2));
        // every dye is made
        for color in DyeColor::ALL {
            assert!(recipes_for(color.dye()).next().is_some()
                || crate::world::furnace::smelting_inputs(color.dye()).next().is_some(),
                "{color:?}");
        }
        for color in DyeColor::ALL {
            let dye = Some(color.dye());
            // any wool, of any colour, takes the dye
            for wool in [ItemId::Wool, ItemId::RedWool, ItemId::BlackWool] {
                assert_eq!(made_of(2, &[Some(wool), dye]), made(color.wool(), 1), "{color:?}");
            }
            let glass = Some(ItemId::Glass);
            let ring = [glass, glass, glass, glass, dye, glass, glass, glass, glass];
            assert_eq!(made_of(3, &ring), made(color.stained_glass(), 8));
            let stained = Some(color.stained_glass());
            assert_eq!(made_of(3, &[stained; 3]), made(color.stained_glass_pane(), 8));
            let t = Some(ItemId::Terracotta);
            assert_eq!(
                made_of(3, &[t, t, t, t, dye, t, t, t, t]),
                made(color.terracotta(), 8)
            );
            let (sand, gravel) = (Some(ItemId::Sand), Some(ItemId::Gravel));
            assert_eq!(
                made_of(3, &[sand, gravel, sand, gravel, dye, gravel, sand, gravel, sand]),
                made(color.concrete_powder(), 8)
            );
        }
        let (clay, brick) = (Some(ItemId::Clay), Some(ItemId::FiredBrick));
        assert_eq!(made_of(2, &[brick, clay]), made(ItemId::Terracotta, 2));
    }

    /// The first recipe making `item`.
    fn recipe_of(item: ItemId) -> RecipeView {
        recipes_for(item).next().unwrap()
    }

    #[test]
    fn a_recipe_fits_a_grid_as_large_as_it() {
        assert!(recipe_of(ItemId::Stick).fits(2));
        assert!(recipe_of(ItemId::WoodenPickaxe).fits(3));
        assert!(!recipe_of(ItemId::WoodenPickaxe).fits(2));
        assert!(recipe_of(ItemId::WoodenAxe).fits(3));
        assert!(!recipe_of(ItemId::WoodenAxe).fits(2));
    }

    #[test]
    fn the_ingredients_of_a_recipe_are_counted_by_cell() {
        assert_eq!(
            recipe_of(ItemId::WoodenPickaxe).ingredients(),
            [(PLANKS, 3), (Ingredient::Item(ItemId::Stick), 2)]
        );
        assert_eq!(
            recipe_of(ItemId::Furnace).ingredients(),
            [(Ingredient::Tag(ItemTag::Cobble), 8)]
        );
    }

    fn put(inventory: &mut Inventory, slot: u32, item_id: ItemId, nb: u32) {
        inventory.inner.insert(slot, made(item_id, nb).unwrap());
    }

    #[test]
    fn missing_counts_the_inventory_and_the_grid() {
        let pickaxe = recipe_of(ItemId::WoodenPickaxe);
        let mut inventory = Inventory::new();
        let stick = Ingredient::Item(ItemId::Stick);
        assert_eq!(missing(&inventory, &pickaxe), [(PLANKS, 3), (stick, 2)]);
        assert!(!craftable(&inventory, &pickaxe));
        put(&mut inventory, 0, ItemId::OakPlanks, 1);
        put(&mut inventory, grid_slot(1, 1), ItemId::OakPlanks, 2);
        put(&mut inventory, 20, ItemId::Stick, 1);
        assert_eq!(missing(&inventory, &pickaxe), [(stick, 1)]);
        assert!(!craftable(&inventory, &pickaxe));
        put(&mut inventory, MAX_INVENTORY_SLOTS - 1, ItemId::Stick, 1);
        assert!(missing(&inventory, &pickaxe).is_empty());
        assert!(craftable(&inventory, &pickaxe));
    }

    #[test]
    fn the_chest_and_the_floating_stack_do_not_count() {
        let pickaxe = recipe_of(ItemId::WoodenPickaxe);
        let mut inventory = Inventory::new();
        let chest = grid_slot(2, 2) + 1;
        put(&mut inventory, chest, ItemId::OakPlanks, 64);
        put(&mut inventory, chest + 1, ItemId::Stick, 64);
        inventory.floating = made(ItemId::Stick, 64);
        assert_eq!(
            missing(&inventory, &pickaxe),
            [(PLANKS, 3), (Ingredient::Item(ItemId::Stick), 2)]
        );
    }

    #[test]
    fn glass_ingots_and_a_stick_make_the_portal_gun() {
        let glass = Some(ItemId::Glass);
        let iron = Some(ItemId::IronIngot);
        let gold = Some(ItemId::GoldIngot);
        let stick = Some(ItemId::Stick);
        assert_eq!(
            match_recipe(&grid([
                glass, iron, iron, None, gold, iron, None, None, stick
            ])),
            made(ItemId::PortalGun, 1)
        );
        assert_eq!(
            match_recipe(&grid([
                iron, iron, glass, iron, gold, None, stick, None, None
            ])),
            made(ItemId::PortalGun, 1)
        );
        // Iron where the gold goes makes nothing
        assert_eq!(
            match_recipe(&grid([
                glass, iron, iron, None, iron, iron, None, None, stick
            ])),
            None
        );
        // Nor do the ores, which must be smelted first
        let (iron, gold) = (Some(ItemId::IronOre), Some(ItemId::GoldOre));
        assert_eq!(
            match_recipe(&grid([
                glass, iron, iron, None, gold, iron, None, None, stick
            ])),
            None
        );
    }

    #[test]
    fn a_tool_takes_planks_of_any_kind() {
        assert_eq!(
            match_recipe(&grid([
                OAK, OAK, SPRUCE, None, STICK, None, None, STICK, None
            ])),
            made(ItemId::WoodenPickaxe, 1)
        );
        assert_eq!(
            match_recipe(&grid([
                JUNGLE, JUNGLE, OAK, None, STICK, None, None, STICK, None
            ])),
            made(ItemId::WoodenPickaxe, 1)
        );
        // mirrored, too
        assert_eq!(
            match_recipe(&grid([
                None, SPRUCE, JUNGLE, None, STICK, OAK, None, STICK, None
            ])),
            made(ItemId::WoodenAxe, 1)
        );
    }

    const PLANKS: Ingredient = Ingredient::Tag(ItemTag::Planks);

    /// The grid holding `recipe` in its top left corner, each cell holding the item `pick`
    /// chooses for its ingredient, mirrored left to right if `mirrored`.
    fn grid_of(
        recipe: &RecipeView,
        mirrored: bool,
        pick: impl Fn(Ingredient) -> ItemId,
    ) -> CraftingGrid {
        let mut grid = [None; 9];
        for row in 0..recipe.height {
            for col in 0..recipe.width {
                let recipe_col = if mirrored {
                    recipe.width - 1 - col
                } else {
                    col
                };
                grid[row * 3 + col] = recipe.cells[row * recipe.width + recipe_col].map(&pick);
            }
        }
        grid
    }

    #[test]
    fn every_recipe_with_planks_is_made_by_each_species() {
        let with_planks: Vec<RecipeView> = recipes()
            .filter(|recipe| recipe.cells.contains(&Some(PLANKS)))
            .collect();
        let outputs: Vec<ItemId> = with_planks.iter().map(|r| r.output.item_id).collect();
        assert_eq!(
            outputs,
            [
                ItemId::Stick,
                ItemId::CraftingTable,
                ItemId::Chest,
                ItemId::WoodenPickaxe,
                ItemId::WoodenAxe,
                ItemId::WoodenShovel,
                ItemId::WoodenHoe,
                ItemId::WoodenSword,
                ItemId::Fence,
                ItemId::Barrel,
                ItemId::Sign,
                ItemId::Boat,
                ItemId::Shield,
                ItemId::Bowl,
            ]
        );
        for recipe in &with_planks {
            for species in [
                ItemId::OakPlanks,
                ItemId::SprucePlanks,
                ItemId::JunglePlanks,
                ItemId::WillowPlanks,
                ItemId::AcaciaPlanks,
            ] {
                let pick = |ingredient: Ingredient| match ingredient {
                    Ingredient::Item(item) => item,
                    Ingredient::Tag(_) => species,
                };
                for mirrored in [false, true] {
                    assert_eq!(
                        match_recipe(&grid_of(recipe, mirrored, pick)),
                        Some(recipe.output),
                        "{species:?} {recipe:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn the_planks_tag_holds_every_planks_item() {
        let planks: Vec<ItemId> = ItemId::ALL
            .into_iter()
            .filter(|item| item.name().ends_with("_planks"))
            .collect();
        assert_eq!(ItemTag::Planks.members(), planks);
        assert_eq!(
            planks,
            [
                ItemId::OakPlanks,
                ItemId::SprucePlanks,
                ItemId::JunglePlanks,
                ItemId::WillowPlanks,
                ItemId::AcaciaPlanks
            ]
        );
        assert_eq!(ItemTag::Planks.name(), "Planks");
    }

    #[test]
    fn an_ingredient_matches_its_items() {
        for planks in ItemTag::Planks.members() {
            assert!(PLANKS.matches(*planks));
        }
        assert!(!PLANKS.matches(ItemId::OakLog));
        assert!(!PLANKS.matches(ItemId::Stick));
        let stick = Ingredient::Item(ItemId::Stick);
        assert!(stick.matches(ItemId::Stick));
        assert!(!stick.matches(ItemId::OakPlanks));
        assert_eq!(stick.members(), [ItemId::Stick]);
        assert_eq!(PLANKS.members(), ItemTag::Planks.members());
    }

    #[test]
    fn no_recipe_has_two_ingredients_sharing_an_item() {
        for recipe in recipes() {
            let ingredients = recipe.ingredients();
            for (index, (first, _)) in ingredients.iter().enumerate() {
                for (second, _) in &ingredients[index + 1..] {
                    assert!(
                        !first.members().iter().any(|item| second.matches(*item)),
                        "{recipe:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn missing_counts_every_member_of_a_tag_together() {
        let pickaxe = recipe_of(ItemId::WoodenPickaxe);
        let stick = Ingredient::Item(ItemId::Stick);
        let mut inventory = Inventory::new();
        put(&mut inventory, 0, ItemId::OakPlanks, 1);
        put(&mut inventory, 30, ItemId::SprucePlanks, 1);
        assert_eq!(missing(&inventory, &pickaxe), [(PLANKS, 1), (stick, 2)]);
        put(&mut inventory, grid_slot(2, 2), ItemId::JunglePlanks, 1);
        assert_eq!(missing(&inventory, &pickaxe), [(stick, 2)]);
        put(&mut inventory, 1, ItemId::Stick, 2);
        assert!(craftable(&inventory, &pickaxe));
    }

    /// An inventory holding `planks` in its slots from 0, a stack each.
    fn holding(planks: &[(ItemId, u32)]) -> Inventory {
        let mut inventory = Inventory::new();
        for (slot, &(item, nb)) in planks.iter().enumerate() {
            put(&mut inventory, slot as u32, item, nb);
        }
        inventory
    }

    /// The items of the cells of the chest's ring, in the order of the recipe.
    fn ring(oak: usize, spruce: usize) -> Vec<Option<ItemId>> {
        let mut species = std::iter::repeat_n(ItemId::OakPlanks, oak)
            .chain(std::iter::repeat_n(ItemId::SprucePlanks, spruce));
        (0..9)
            .map(|cell| if cell == 4 { None } else { species.next() })
            .collect()
    }

    #[test]
    fn a_fill_takes_one_species_when_it_is_enough() {
        let chest = recipe_of(ItemId::Chest);
        let plan = |planks: &[(ItemId, u32)]| plan_fill(&holding(planks), &chest, 1);
        // the most held
        let only_oak = plan(&[(ItemId::OakPlanks, 10), (ItemId::SprucePlanks, 3)]);
        assert_eq!(
            only_oak,
            Some(FillPlan {
                rounds: 1,
                cells: ring(8, 0)
            })
        );
        let only_spruce = plan(&[(ItemId::OakPlanks, 8), (ItemId::SprucePlanks, 9)]);
        assert_eq!(
            only_spruce,
            Some(FillPlan {
                rounds: 1,
                cells: ring(0, 8)
            })
        );
        // the first of the family on a tie
        let tie = plan(&[(ItemId::SprucePlanks, 8), (ItemId::OakPlanks, 8)]);
        assert_eq!(
            tie,
            Some(FillPlan {
                rounds: 1,
                cells: ring(8, 0)
            })
        );
    }

    #[test]
    fn a_fill_mixes_species_when_none_is_enough() {
        let chest = recipe_of(ItemId::Chest);
        let inventory = holding(&[(ItemId::OakPlanks, 5), (ItemId::SprucePlanks, 4)]);
        assert_eq!(
            plan_fill(&inventory, &chest, 1),
            Some(FillPlan {
                rounds: 1,
                cells: ring(5, 3)
            })
        );
        let sticks = recipe_of(ItemId::Stick);
        let inventory = holding(&[(ItemId::JunglePlanks, 1), (ItemId::OakPlanks, 1)]);
        assert_eq!(
            plan_fill(&inventory, &sticks, 1),
            Some(FillPlan {
                rounds: 1,
                cells: vec![Some(ItemId::OakPlanks), Some(ItemId::JunglePlanks)]
            })
        );
    }

    #[test]
    fn lacking_cells_serves_one_round_cell_by_cell() {
        let pickaxe = recipe_of(ItemId::WoodenPickaxe);
        // Two planks for three cells, one stick for two: the last of each lacks
        let mut inventory = holding(&[(ItemId::OakPlanks, 2)]);
        put(&mut inventory, 5, ItemId::Stick, 1);
        assert_eq!(
            lacking_cells(&inventory, &pickaxe),
            [false, false, true, false, false, false, false, true, false]
        );
        // Any species serves a cell of planks
        let mut inventory = holding(&[
            (ItemId::OakPlanks, 1),
            (ItemId::SprucePlanks, 1),
            (ItemId::JunglePlanks, 1),
        ]);
        put(&mut inventory, 5, ItemId::Stick, 2);
        assert_eq!(lacking_cells(&inventory, &pickaxe), [false; 9]);
        assert!(plan_fill(&inventory, &pickaxe, 1).is_some());
        // Nothing serves anything; the grid does not count
        let mut inventory = Inventory::new();
        put(&mut inventory, grid_slot(0, 0), ItemId::Coal, 1);
        let torch = recipe_of(ItemId::Torch);
        assert_eq!(lacking_cells(&inventory, &torch), [true, true]);
    }

    #[test]
    fn a_fill_makes_as_many_rounds_as_every_cell_can_get_of_one_item() {
        let chest = recipe_of(ItemId::Chest);
        // 3 rounds: 6 cells of oak and 2 of spruce; 4 would need 8 cells of 4
        let inventory = holding(&[(ItemId::OakPlanks, 20), (ItemId::SprucePlanks, 10)]);
        assert_eq!(
            plan_fill(&inventory, &chest, u32::MAX),
            Some(FillPlan {
                rounds: 3,
                cells: ring(6, 2)
            })
        );
        // as many as asked
        assert_eq!(
            plan_fill(&inventory, &chest, 2).map(|plan| plan.rounds),
            Some(2)
        );
        // a stack at most
        let pickaxe = recipe_of(ItemId::WoodenPickaxe);
        let mut inventory = Inventory::new();
        for slot in 0..10 {
            put(&mut inventory, slot, ItemId::OakPlanks, 64);
        }
        for slot in 10..14 {
            put(&mut inventory, slot, ItemId::Stick, 64);
        }
        assert_eq!(
            plan_fill(&inventory, &pickaxe, u32::MAX).map(|plan| plan.rounds),
            Some(64)
        );
        // the grid and a chest do not count: the fill puts the grid back first
        let mut inventory = holding(&[(ItemId::OakPlanks, 7)]);
        put(&mut inventory, grid_slot(0, 0), ItemId::OakPlanks, 1);
        put(&mut inventory, grid_slot(2, 2) + 1, ItemId::OakPlanks, 64);
        assert_eq!(plan_fill(&inventory, &chest, 1), None);
        assert_eq!(plan_fill(&Inventory::new(), &chest, 1), None);
        assert_eq!(
            plan_fill(&holding(&[(ItemId::OakPlanks, 8)]), &chest, 0),
            None
        );
    }
}
