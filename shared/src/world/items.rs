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
    JungleLog,
    JunglePlanks,
    JungleLeaves,
    Fern,
    Vine,
    CocoaBeans,
    IronIngot,
    GoldIngot,
    Diamond,
    Charcoal,
    DiamondOre,
    IronBlock,
    GoldBlock,
    DiamondBlock,
    StonePickaxe,
    StoneAxe,
    StoneShovel,
    StoneHoe,
    StoneSword,
    IronPickaxe,
    IronAxe,
    IronShovel,
    IronHoe,
    IronSword,
    GoldenPickaxe,
    GoldenAxe,
    GoldenShovel,
    GoldenHoe,
    GoldenSword,
    DiamondPickaxe,
    DiamondAxe,
    DiamondShovel,
    DiamondHoe,
    DiamondSword,
    Bucket,
    WaterBucket,
    CowSpawnEgg,
    PigSpawnEgg,
    ChickenSpawnEgg,
    RabbitSpawnEgg,
    WolfSpawnEgg,
    HorseSpawnEgg,
    GoatSpawnEgg,
    CatSpawnEgg,
    SpruceLeaves,
    HewnStone,
    Limestone,
    Slate,
    Clay,
    BrickWall,
    RammedEarth,
    Thatch,
    CoalBlock,
    CopperOre,
    TinOre,
    CopperBlock,
    TinBlock,
    BronzeBlock,
    WillowLog,
    WillowPlanks,
    WillowLeaves,
    AcaciaLog,
    AcaciaPlanks,
    AcaciaLeaves,
    Reed,
    Heather,
    Mushroom,
    WoodenSlab,
    HewnStoneSlab,
    BrickSlab,
    Lantern,
    FiredBrick,
    Fiber,
    Rope,
    Flint,
    CopperIngot,
    TinIngot,
    BronzeIngot,
    RoastMushroom,
    BronzePickaxe,
    BronzeAxe,
    BronzeShovel,
    BronzeHoe,
    BronzeSword,
    WoodenStairs,
    HewnStoneStairs,
    BrickStairs,
    Door,
    Ladder,
    Fence,
    GlassPane,
    OakSapling,
    SpruceSapling,
    JungleSapling,
    WillowSapling,
    AcaciaSapling,
    Wool,
    BerryBush,
    Bedroll,
    Barrel,
    RainCistern,
    Campfire,
    Sign,
    Rye,
    RyeBread,
    RawMutton,
    RoastMutton,
    Berries,
    Spear,
    PortalGun,
    /// Lights a fire. It wears out, though it is not a mining tool: see `durability`.
    FlintAndSteel,
    Obsidian,
    LavaBucket,
    RawPork,
    RoastPork,
    RawBeef,
    RoastBeef,
    RawChicken,
    RoastChicken,
    RawRabbit,
    RoastRabbit,
    Leather,
    Feather,
    /// Eaten raw, or planted on tilled soil.
    Carrot,
    /// Eaten raw, planted on tilled soil, or baked.
    Potato,
    BakedPotato,
    GnawerSpawnEgg,
    BrambleSpawnEgg,
    RollerSpawnEgg,
    StumplingSpawnEgg,
    DrownerSpawnEgg,
    CinderlingSpawnEgg,
    GustOwlSpawnEgg,
    /// The armour: see `armour`.
    LeatherHelmet,
    LeatherChestplate,
    LeatherLeggings,
    LeatherBoots,
    BronzeHelmet,
    BronzeChestplate,
    BronzeLeggings,
    BronzeBoots,
    IronHelmet,
    IronChestplate,
    IronLeggings,
    IronBoots,
    DiamondHelmet,
    DiamondChestplate,
    DiamondLeggings,
    DiamondBoots,
    /// Shoots arrows, drawn by holding the right button (`players::bow`).
    Bow,
    Arrow,
    /// What the villagers trade in: see `trade`.
    Coin,
    FarmerSpawnEgg,
    SmithSpawnEgg,
    CartographerSpawnEgg,
    /// Its needle points to the lair (`lair::centre`) in hand and in the hotbar.
    DepthsCompass,
    /// The trophy of the Ember Mother.
    EmberHeart,
}

impl ItemId {
    /// Every item, in the order of the variants.
    pub const ALL: [ItemId; 193] = [
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
        Self::JungleLog,
        Self::JunglePlanks,
        Self::JungleLeaves,
        Self::Fern,
        Self::Vine,
        Self::CocoaBeans,
        Self::IronIngot,
        Self::GoldIngot,
        Self::Diamond,
        Self::Charcoal,
        Self::DiamondOre,
        Self::IronBlock,
        Self::GoldBlock,
        Self::DiamondBlock,
        Self::StonePickaxe,
        Self::StoneAxe,
        Self::StoneShovel,
        Self::StoneHoe,
        Self::StoneSword,
        Self::IronPickaxe,
        Self::IronAxe,
        Self::IronShovel,
        Self::IronHoe,
        Self::IronSword,
        Self::GoldenPickaxe,
        Self::GoldenAxe,
        Self::GoldenShovel,
        Self::GoldenHoe,
        Self::GoldenSword,
        Self::DiamondPickaxe,
        Self::DiamondAxe,
        Self::DiamondShovel,
        Self::DiamondHoe,
        Self::DiamondSword,
        Self::Bucket,
        Self::WaterBucket,
        Self::CowSpawnEgg,
        Self::PigSpawnEgg,
        Self::ChickenSpawnEgg,
        Self::RabbitSpawnEgg,
        Self::WolfSpawnEgg,
        Self::HorseSpawnEgg,
        Self::GoatSpawnEgg,
        Self::CatSpawnEgg,
        Self::SpruceLeaves,
        Self::HewnStone,
        Self::Limestone,
        Self::Slate,
        Self::Clay,
        Self::BrickWall,
        Self::RammedEarth,
        Self::Thatch,
        Self::CoalBlock,
        Self::CopperOre,
        Self::TinOre,
        Self::CopperBlock,
        Self::TinBlock,
        Self::BronzeBlock,
        Self::WillowLog,
        Self::WillowPlanks,
        Self::WillowLeaves,
        Self::AcaciaLog,
        Self::AcaciaPlanks,
        Self::AcaciaLeaves,
        Self::Reed,
        Self::Heather,
        Self::Mushroom,
        Self::WoodenSlab,
        Self::HewnStoneSlab,
        Self::BrickSlab,
        Self::Lantern,
        Self::FiredBrick,
        Self::Fiber,
        Self::Rope,
        Self::Flint,
        Self::CopperIngot,
        Self::TinIngot,
        Self::BronzeIngot,
        Self::RoastMushroom,
        Self::BronzePickaxe,
        Self::BronzeAxe,
        Self::BronzeShovel,
        Self::BronzeHoe,
        Self::BronzeSword,
        Self::WoodenStairs,
        Self::HewnStoneStairs,
        Self::BrickStairs,
        Self::Door,
        Self::Ladder,
        Self::Fence,
        Self::GlassPane,
        Self::OakSapling,
        Self::SpruceSapling,
        Self::JungleSapling,
        Self::WillowSapling,
        Self::AcaciaSapling,
        Self::Wool,
        Self::BerryBush,
        Self::Bedroll,
        Self::Barrel,
        Self::RainCistern,
        Self::Campfire,
        Self::Sign,
        Self::Rye,
        Self::RyeBread,
        Self::RawMutton,
        Self::RoastMutton,
        Self::Berries,
        Self::Spear,
        Self::PortalGun,
        Self::FlintAndSteel,
        Self::Obsidian,
        Self::LavaBucket,
        Self::RawPork,
        Self::RoastPork,
        Self::RawBeef,
        Self::RoastBeef,
        Self::RawChicken,
        Self::RoastChicken,
        Self::RawRabbit,
        Self::RoastRabbit,
        Self::Leather,
        Self::Feather,
        Self::Carrot,
        Self::Potato,
        Self::BakedPotato,
        Self::GnawerSpawnEgg,
        Self::BrambleSpawnEgg,
        Self::RollerSpawnEgg,
        Self::StumplingSpawnEgg,
        Self::DrownerSpawnEgg,
        Self::CinderlingSpawnEgg,
        Self::GustOwlSpawnEgg,
        Self::LeatherHelmet,
        Self::LeatherChestplate,
        Self::LeatherLeggings,
        Self::LeatherBoots,
        Self::BronzeHelmet,
        Self::BronzeChestplate,
        Self::BronzeLeggings,
        Self::BronzeBoots,
        Self::IronHelmet,
        Self::IronChestplate,
        Self::IronLeggings,
        Self::IronBoots,
        Self::DiamondHelmet,
        Self::DiamondChestplate,
        Self::DiamondLeggings,
        Self::DiamondBoots,
        Self::Bow,
        Self::Arrow,
        Self::Coin,
        Self::FarmerSpawnEgg,
        Self::SmithSpawnEgg,
        Self::CartographerSpawnEgg,
        Self::DepthsCompass,
        Self::EmberHeart,
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
        match *self {
            Self::Bucket => 16,
            Self::WaterBucket | Self::LavaBucket | Self::Spear | Self::PortalGun => 1,
            _ if self.durability().is_some() => 1,
            _ => 64,
        }
    }

    /// The points of health a blow with the item takes from a mob.
    pub fn attack_damage(&self) -> u32 {
        if *self == Self::Spear {
            return 4;
        }
        let Some(tool) = self.tool() else {
            return 1;
        };
        let tier = match tool.tier {
            ToolTier::Wood | ToolTier::Gold => 0,
            ToolTier::Stone | ToolTier::Bronze => 1,
            ToolTier::Iron => 2,
            ToolTier::Diamond => 3,
        };
        match tool.kind {
            ToolKind::Sword => 4 + tier,
            ToolKind::Axe => 3 + tier,
            _ => 2 + tier,
        }
    }

    /// How much a blow on a mob wears the item: a sword is made for it, another tool wears as
    /// it does on a block it is not made for; the rest does not wear.
    pub fn blow_wear(&self) -> u32 {
        match self.tool() {
            Some(tool) if tool.kind == ToolKind::Sword => 1,
            Some(_) => 2,
            None => 0,
        }
    }

    /// The food points and the saturation eating the item gives, if it is eaten: Minecraft's
    /// where Minecraft has the food, the saturation twice the food by its modifier.
    pub fn food_value(&self) -> Option<(u8, f32)> {
        match *self {
            Self::Berries => Some((2, 0.4)),
            Self::RoastMushroom => Some((4, 4.8)),
            Self::RyeBread => Some((5, 6.0)),
            Self::RawMutton | Self::RawChicken => Some((2, 1.2)),
            Self::RawPork | Self::RawBeef | Self::RawRabbit => Some((3, 1.8)),
            Self::RoastMutton => Some((6, 9.6)),
            Self::RoastChicken => Some((6, 7.2)),
            Self::RoastRabbit => Some((5, 6.0)),
            Self::RoastPork | Self::RoastBeef => Some((8, 12.8)),
            Self::Carrot => Some((3, 3.6)),
            Self::Potato => Some((1, 0.6)),
            Self::BakedPotato => Some((5, 6.0)),
            _ => None,
        }
    }

    /// The mob a spawn egg makes appear, used on a block.
    pub fn spawned_mob(&self) -> Option<MobKind> {
        match *self {
            Self::FoxSpawnEgg => Some(MobKind::Fox),
            Self::SheepSpawnEgg => Some(MobKind::Sheep),
            Self::CowSpawnEgg => Some(MobKind::Cow),
            Self::PigSpawnEgg => Some(MobKind::Pig),
            Self::ChickenSpawnEgg => Some(MobKind::Chicken),
            Self::RabbitSpawnEgg => Some(MobKind::Rabbit),
            Self::WolfSpawnEgg => Some(MobKind::Wolf),
            Self::HorseSpawnEgg => Some(MobKind::Horse),
            Self::GoatSpawnEgg => Some(MobKind::Goat),
            Self::CatSpawnEgg => Some(MobKind::Cat),
            Self::GnawerSpawnEgg => Some(MobKind::Gnawer),
            Self::BrambleSpawnEgg => Some(MobKind::Bramble),
            Self::RollerSpawnEgg => Some(MobKind::Roller),
            Self::StumplingSpawnEgg => Some(MobKind::Stumpling),
            Self::DrownerSpawnEgg => Some(MobKind::Drowner),
            Self::CinderlingSpawnEgg => Some(MobKind::Cinderling),
            Self::GustOwlSpawnEgg => Some(MobKind::GustOwl),
            Self::FarmerSpawnEgg => Some(MobKind::Farmer),
            Self::SmithSpawnEgg => Some(MobKind::Smith),
            Self::CartographerSpawnEgg => Some(MobKind::Cartographer),
            _ => None,
        }
    }

    /// What the item does as a tool, if it is one.
    pub fn tool(&self) -> Option<Tool> {
        use ToolKind::*;
        use ToolTier::*;
        let (tier, kind) = match *self {
            Self::WoodenPickaxe => (Wood, Pickaxe),
            Self::WoodenAxe => (Wood, Axe),
            Self::WoodenShovel => (Wood, Shovel),
            Self::WoodenHoe => (Wood, Hoe),
            Self::WoodenSword => (Wood, Sword),
            Self::StonePickaxe => (Stone, Pickaxe),
            Self::StoneAxe => (Stone, Axe),
            Self::StoneShovel => (Stone, Shovel),
            Self::StoneHoe => (Stone, Hoe),
            Self::StoneSword => (Stone, Sword),
            Self::IronPickaxe => (Iron, Pickaxe),
            Self::IronAxe => (Iron, Axe),
            Self::IronShovel => (Iron, Shovel),
            Self::IronHoe => (Iron, Hoe),
            Self::IronSword => (Iron, Sword),
            Self::GoldenPickaxe => (Gold, Pickaxe),
            Self::GoldenAxe => (Gold, Axe),
            Self::GoldenShovel => (Gold, Shovel),
            Self::GoldenHoe => (Gold, Hoe),
            Self::GoldenSword => (Gold, Sword),
            Self::DiamondPickaxe => (Diamond, Pickaxe),
            Self::DiamondAxe => (Diamond, Axe),
            Self::DiamondShovel => (Diamond, Shovel),
            Self::DiamondHoe => (Diamond, Hoe),
            Self::DiamondSword => (Diamond, Sword),
            Self::BronzePickaxe => (Bronze, Pickaxe),
            Self::BronzeAxe => (Bronze, Axe),
            Self::BronzeShovel => (Bronze, Shovel),
            Self::BronzeHoe => (Bronze, Hoe),
            Self::BronzeSword => (Bronze, Sword),
            _ => return None,
        };
        Some(Tool {
            kind,
            tier,
            speed: tier.speed(),
            durability: tier.durability(),
            // A weapon: like in Minecraft, mining wears it twice as fast
            wear_per_block: if kind == Sword { 2 } else { 1 },
        })
    }

    /// The wear at which the item breaks, for the items that wear out: the tools, the
    /// armour, which wears by the hits it absorbs, the flint and steel, which wears by the fires
    /// it lights, and the bow, by its shots.
    pub fn durability(&self) -> Option<u32> {
        match *self {
            Self::FlintAndSteel => Some(64),
            Self::Bow => Some(320),
            _ => match self.armour() {
                Some(armour) => Some(armour.durability()),
                None => self.tool().map(|tool| tool.durability),
            },
        }
    }

    /// The category the item browser files the item under.
    pub fn category(self) -> ItemCategory {
        // No wildcard: a new item does not compile until it has a category
        match self {
            Self::Dirt
            | Self::Grass
            | Self::Stone
            | Self::Cobblestone
            | Self::Gravel
            | Self::Sand
            | Self::Glass
            | Self::Ice
            | Self::Snow
            | Self::Torch
            | Self::Bedrock
            | Self::IronBlock
            | Self::GoldBlock
            | Self::DiamondBlock
            | Self::HewnStone
            | Self::Limestone
            | Self::Slate
            | Self::Clay
            | Self::BrickWall
            | Self::RammedEarth
            | Self::Thatch
            | Self::CoalBlock
            | Self::CopperBlock
            | Self::TinBlock
            | Self::BronzeBlock
            | Self::WoodenSlab
            | Self::HewnStoneSlab
            | Self::BrickSlab
            | Self::Lantern
            | Self::WoodenStairs
            | Self::HewnStoneStairs
            | Self::BrickStairs
            | Self::Door
            | Self::Ladder
            | Self::Fence
            | Self::GlassPane
            | Self::Wool
            | Self::Sign
            | Self::Obsidian
            | Self::EmberHeart => ItemCategory::Blocks,
            Self::OakLog
            | Self::SpruceLog
            | Self::JungleLog
            | Self::OakPlanks
            | Self::SprucePlanks
            | Self::JunglePlanks
            | Self::WillowLog
            | Self::WillowPlanks
            | Self::AcaciaLog
            | Self::AcaciaPlanks => ItemCategory::Wood,
            Self::OakLeaves
            | Self::JungleLeaves
            | Self::Cactus
            | Self::Dandelion
            | Self::Poppy
            | Self::TallGrass
            | Self::Fern
            | Self::Vine
            | Self::SpruceLeaves
            | Self::WillowLeaves
            | Self::AcaciaLeaves
            | Self::Reed
            | Self::Heather
            | Self::Mushroom
            | Self::OakSapling
            | Self::SpruceSapling
            | Self::JungleSapling
            | Self::WillowSapling
            | Self::AcaciaSapling
            | Self::BerryBush => ItemCategory::Nature,
            Self::CoalOre
            | Self::IronOre
            | Self::GoldOre
            | Self::DiamondOre
            | Self::CopperOre
            | Self::TinOre => ItemCategory::Ores,
            Self::Coal
            | Self::Charcoal
            | Self::IronIngot
            | Self::GoldIngot
            | Self::Diamond
            | Self::Stick
            | Self::Snowball
            | Self::CocoaBeans
            | Self::FiredBrick
            | Self::Fiber
            | Self::Rope
            | Self::Flint
            | Self::CopperIngot
            | Self::TinIngot
            | Self::BronzeIngot
            | Self::RoastMushroom
            | Self::Rye
            | Self::RyeBread
            | Self::RawMutton
            | Self::RoastMutton
            | Self::Berries
            | Self::RawPork
            | Self::RoastPork
            | Self::RawBeef
            | Self::RoastBeef
            | Self::RawChicken
            | Self::RoastChicken
            | Self::RawRabbit
            | Self::RoastRabbit
            | Self::Leather
            | Self::Feather
            | Self::Coin
            | Self::Carrot
            | Self::Potato
            | Self::BakedPotato => ItemCategory::Materials,
            Self::CraftingTable
            | Self::Furnace
            | Self::Chest
            | Self::Bedroll
            | Self::Barrel
            | Self::RainCistern
            | Self::Campfire => ItemCategory::Stations,
            Self::WoodenPickaxe
            | Self::WoodenAxe
            | Self::WoodenShovel
            | Self::WoodenHoe
            | Self::WoodenSword
            | Self::StonePickaxe
            | Self::StoneAxe
            | Self::StoneShovel
            | Self::StoneHoe
            | Self::StoneSword
            | Self::IronPickaxe
            | Self::IronAxe
            | Self::IronShovel
            | Self::IronHoe
            | Self::IronSword
            | Self::GoldenPickaxe
            | Self::GoldenAxe
            | Self::GoldenShovel
            | Self::GoldenHoe
            | Self::GoldenSword
            | Self::DiamondPickaxe
            | Self::DiamondAxe
            | Self::DiamondShovel
            | Self::DiamondHoe
            | Self::DiamondSword
            | Self::Bucket
            | Self::WaterBucket
            | Self::LavaBucket
            | Self::BronzePickaxe
            | Self::BronzeAxe
            | Self::BronzeShovel
            | Self::BronzeHoe
            | Self::BronzeSword
            | Self::Spear
            | Self::PortalGun
            | Self::FlintAndSteel
            | Self::LeatherHelmet
            | Self::LeatherChestplate
            | Self::LeatherLeggings
            | Self::LeatherBoots
            | Self::BronzeHelmet
            | Self::BronzeChestplate
            | Self::BronzeLeggings
            | Self::BronzeBoots
            | Self::IronHelmet
            | Self::IronChestplate
            | Self::IronLeggings
            | Self::IronBoots
            | Self::DiamondHelmet
            | Self::DiamondChestplate
            | Self::DiamondLeggings
            | Self::DiamondBoots
            | Self::Bow
            | Self::Arrow
            | Self::DepthsCompass => ItemCategory::Tools,
            Self::FoxSpawnEgg
            | Self::SheepSpawnEgg
            | Self::CowSpawnEgg
            | Self::PigSpawnEgg
            | Self::ChickenSpawnEgg
            | Self::RabbitSpawnEgg
            | Self::WolfSpawnEgg
            | Self::HorseSpawnEgg
            | Self::GoatSpawnEgg
            | Self::CatSpawnEgg
            | Self::GnawerSpawnEgg
            | Self::BrambleSpawnEgg
            | Self::RollerSpawnEgg
            | Self::StumplingSpawnEgg
            | Self::DrownerSpawnEgg
            | Self::CinderlingSpawnEgg
            | Self::GustOwlSpawnEgg
            | Self::FarmerSpawnEgg
            | Self::SmithSpawnEgg
            | Self::CartographerSpawnEgg => ItemCategory::Mobs,
        }
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
            Self::JungleLog => ItemType::Block(BlockId::JungleLog),
            Self::JunglePlanks => ItemType::Block(BlockId::JunglePlanks),
            Self::JungleLeaves => ItemType::Block(BlockId::JungleLeaves),
            Self::Fern => ItemType::Block(BlockId::Fern),
            Self::Vine => ItemType::Block(BlockId::Vine),
            Self::DiamondOre => ItemType::Block(BlockId::DiamondOre),
            Self::IronBlock => ItemType::Block(BlockId::IronBlock),
            Self::GoldBlock => ItemType::Block(BlockId::GoldBlock),
            Self::DiamondBlock => ItemType::Block(BlockId::DiamondBlock),
            Self::SpruceLeaves => ItemType::Block(BlockId::SpruceLeaves),
            Self::HewnStone => ItemType::Block(BlockId::HewnStone),
            Self::Limestone => ItemType::Block(BlockId::Limestone),
            Self::Slate => ItemType::Block(BlockId::Slate),
            Self::Clay => ItemType::Block(BlockId::Clay),
            Self::BrickWall => ItemType::Block(BlockId::BrickWall),
            Self::RammedEarth => ItemType::Block(BlockId::RammedEarth),
            Self::Thatch => ItemType::Block(BlockId::Thatch),
            Self::CoalBlock => ItemType::Block(BlockId::CoalBlock),
            Self::CopperOre => ItemType::Block(BlockId::CopperOre),
            Self::TinOre => ItemType::Block(BlockId::TinOre),
            Self::CopperBlock => ItemType::Block(BlockId::CopperBlock),
            Self::TinBlock => ItemType::Block(BlockId::TinBlock),
            Self::BronzeBlock => ItemType::Block(BlockId::BronzeBlock),
            Self::WillowLog => ItemType::Block(BlockId::WillowLog),
            Self::WillowPlanks => ItemType::Block(BlockId::WillowPlanks),
            Self::WillowLeaves => ItemType::Block(BlockId::WillowLeaves),
            Self::AcaciaLog => ItemType::Block(BlockId::AcaciaLog),
            Self::AcaciaPlanks => ItemType::Block(BlockId::AcaciaPlanks),
            Self::AcaciaLeaves => ItemType::Block(BlockId::AcaciaLeaves),
            Self::Reed => ItemType::Block(BlockId::Reed),
            Self::Heather => ItemType::Block(BlockId::Heather),
            Self::Mushroom => ItemType::Block(BlockId::Mushroom),
            Self::WoodenSlab => ItemType::Block(BlockId::WoodenSlab),
            Self::HewnStoneSlab => ItemType::Block(BlockId::HewnStoneSlab),
            Self::BrickSlab => ItemType::Block(BlockId::BrickSlab),
            Self::Lantern => ItemType::Block(BlockId::Lantern),
            Self::WoodenStairs => ItemType::Block(BlockId::WoodenStairs),
            Self::HewnStoneStairs => ItemType::Block(BlockId::HewnStoneStairs),
            Self::BrickStairs => ItemType::Block(BlockId::BrickStairs),
            Self::Door => ItemType::Block(BlockId::Door),
            Self::Ladder => ItemType::Block(BlockId::Ladder),
            Self::Fence => ItemType::Block(BlockId::Fence),
            Self::GlassPane => ItemType::Block(BlockId::GlassPane),
            Self::OakSapling => ItemType::Block(BlockId::OakSapling),
            Self::SpruceSapling => ItemType::Block(BlockId::SpruceSapling),
            Self::JungleSapling => ItemType::Block(BlockId::JungleSapling),
            Self::WillowSapling => ItemType::Block(BlockId::WillowSapling),
            Self::AcaciaSapling => ItemType::Block(BlockId::AcaciaSapling),
            Self::Wool => ItemType::Block(BlockId::Wool),
            Self::BerryBush => ItemType::Block(BlockId::BerryBush),
            Self::Bedroll => ItemType::Block(BlockId::Bedroll),
            Self::Barrel => ItemType::Block(BlockId::Barrel),
            Self::RainCistern => ItemType::Block(BlockId::RainCistern),
            Self::Campfire => ItemType::Block(BlockId::Campfire),
            Self::Sign => ItemType::Block(BlockId::Sign),
            Self::Obsidian => ItemType::Block(BlockId::Obsidian),
            Self::EmberHeart => ItemType::Block(BlockId::EmberHeart),
            Self::Rye => ItemType::Block(BlockId::RyeSprout),
            Self::Carrot => ItemType::Block(BlockId::CarrotSprout),
            Self::Potato => ItemType::Block(BlockId::PotatoSprout),

            Self::Snowball
            | Self::Stick
            | Self::Coal
            | Self::FoxSpawnEgg
            | Self::SheepSpawnEgg
            | Self::CowSpawnEgg
            | Self::PigSpawnEgg
            | Self::ChickenSpawnEgg
            | Self::RabbitSpawnEgg
            | Self::WolfSpawnEgg
            | Self::HorseSpawnEgg
            | Self::GoatSpawnEgg
            | Self::CatSpawnEgg
            | Self::GnawerSpawnEgg
            | Self::BrambleSpawnEgg
            | Self::RollerSpawnEgg
            | Self::StumplingSpawnEgg
            | Self::DrownerSpawnEgg
            | Self::CinderlingSpawnEgg
            | Self::GustOwlSpawnEgg
            | Self::Coin
            | Self::FarmerSpawnEgg
            | Self::SmithSpawnEgg
            | Self::CartographerSpawnEgg
            | Self::CocoaBeans
            | Self::IronIngot
            | Self::GoldIngot
            | Self::Diamond
            | Self::Charcoal
            | Self::Bucket
            | Self::WaterBucket
            | Self::FiredBrick
            | Self::Fiber
            | Self::Rope
            | Self::Flint
            | Self::CopperIngot
            | Self::TinIngot
            | Self::BronzeIngot
            | Self::RoastMushroom
            | Self::RyeBread
            | Self::RawMutton
            | Self::RoastMutton
            | Self::Berries
            | Self::Spear
            | Self::Arrow
            | Self::DepthsCompass
            | Self::PortalGun
            | Self::LavaBucket
            | Self::RawPork
            | Self::RoastPork
            | Self::RawBeef
            | Self::RoastBeef
            | Self::RawChicken
            | Self::RoastChicken
            | Self::RawRabbit
            | Self::RoastRabbit
            | Self::Leather
            | Self::Feather
            | Self::BakedPotato => ItemType::Generic,

            Self::WoodenPickaxe
            | Self::WoodenAxe
            | Self::WoodenShovel
            | Self::WoodenHoe
            | Self::WoodenSword
            | Self::StonePickaxe
            | Self::StoneAxe
            | Self::StoneShovel
            | Self::StoneHoe
            | Self::StoneSword
            | Self::IronPickaxe
            | Self::IronAxe
            | Self::IronShovel
            | Self::IronHoe
            | Self::IronSword
            | Self::GoldenPickaxe
            | Self::GoldenAxe
            | Self::GoldenShovel
            | Self::GoldenHoe
            | Self::GoldenSword
            | Self::DiamondPickaxe
            | Self::DiamondAxe
            | Self::DiamondShovel
            | Self::DiamondHoe
            | Self::DiamondSword
            | Self::BronzePickaxe
            | Self::BronzeAxe
            | Self::BronzeShovel
            | Self::BronzeHoe
            | Self::BronzeSword
            | Self::FlintAndSteel
            | Self::LeatherHelmet
            | Self::LeatherChestplate
            | Self::LeatherLeggings
            | Self::LeatherBoots
            | Self::BronzeHelmet
            | Self::BronzeChestplate
            | Self::BronzeLeggings
            | Self::BronzeBoots
            | Self::IronHelmet
            | Self::IronChestplate
            | Self::IronLeggings
            | Self::IronBoots
            | Self::DiamondHelmet
            | Self::DiamondChestplate
            | Self::DiamondLeggings
            | Self::DiamondBoots
            | Self::Bow => ItemType::Tool { wear: 0 },
        }
    }
}

impl GameElementId for ItemId {}

/// The categories of the item browser, which files every item under one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ItemCategory {
    Blocks,
    Wood,
    Nature,
    Ores,
    Materials,
    Stations,
    Tools,
    Mobs,
}

impl ItemCategory {
    /// Every category, in the order of the browser's chips and of its list.
    pub const ALL: [ItemCategory; 8] = [
        Self::Blocks,
        Self::Wood,
        Self::Nature,
        Self::Ores,
        Self::Materials,
        Self::Stations,
        Self::Tools,
        Self::Mobs,
    ];

    /// The name of the category as a player reads it.
    pub fn name(self) -> &'static str {
        match self {
            Self::Blocks => "Blocks",
            Self::Wood => "Wood",
            Self::Nature => "Nature",
            Self::Ores => "Ores",
            Self::Materials => "Materials",
            Self::Stations => "Stations",
            Self::Tools => "Tools",
            Self::Mobs => "Mobs",
        }
    }
}

/// What a tool is made of, which sets how fast it mines, how long it lasts and the blocks it
/// harvests, like Minecraft's tiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ToolTier {
    Wood,
    Stone,
    Iron,
    Diamond,
    Gold,
    /// Between stone and iron: it lasts longer than stone and harvests the same blocks.
    Bronze,
}

impl ToolTier {
    /// How many times faster a tool of this tier breaks the blocks it is made for than a hand.
    pub fn speed(self) -> u8 {
        match self {
            Self::Wood => 2,
            Self::Stone => 4,
            Self::Iron => 6,
            Self::Diamond => 8,
            Self::Gold => 12,
            Self::Bronze => 5,
        }
    }

    /// Blocks a tool of this tier breaks before it breaks itself.
    pub fn durability(self) -> u32 {
        match self {
            Self::Wood => 59,
            Self::Stone => 131,
            Self::Iron => 250,
            Self::Diamond => 1561,
            Self::Gold => 32,
            Self::Bronze => 190,
        }
    }

    /// The highest level of the blocks a tool of this tier harvests, see
    /// `BlockId::get_required_tool`: gold is as soft as wood.
    pub fn mining_level(self) -> u8 {
        match self {
            Self::Wood | Self::Gold => 0,
            Self::Stone | Self::Bronze => 1,
            Self::Iron => 2,
            Self::Diamond => 3,
        }
    }
}

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
    pub tier: ToolTier,
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
    /// A tool or a piece of armour, and the wear of this one: it breaks when the wear reaches its durability. It is
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
            (ItemId::JungleLog, 36),
            (ItemId::JunglePlanks, 37),
            (ItemId::JungleLeaves, 38),
            (ItemId::Fern, 39),
            (ItemId::Vine, 40),
            (ItemId::CocoaBeans, 41),
            (ItemId::IronIngot, 42),
            (ItemId::GoldIngot, 43),
            (ItemId::Diamond, 44),
            (ItemId::Charcoal, 45),
            (ItemId::DiamondOre, 46),
            (ItemId::IronBlock, 47),
            (ItemId::GoldBlock, 48),
            (ItemId::DiamondBlock, 49),
            (ItemId::StonePickaxe, 50),
            (ItemId::StoneAxe, 51),
            (ItemId::StoneShovel, 52),
            (ItemId::StoneHoe, 53),
            (ItemId::StoneSword, 54),
            (ItemId::IronPickaxe, 55),
            (ItemId::IronAxe, 56),
            (ItemId::IronShovel, 57),
            (ItemId::IronHoe, 58),
            (ItemId::IronSword, 59),
            (ItemId::GoldenPickaxe, 60),
            (ItemId::GoldenAxe, 61),
            (ItemId::GoldenShovel, 62),
            (ItemId::GoldenHoe, 63),
            (ItemId::GoldenSword, 64),
            (ItemId::DiamondPickaxe, 65),
            (ItemId::DiamondAxe, 66),
            (ItemId::DiamondShovel, 67),
            (ItemId::DiamondHoe, 68),
            (ItemId::DiamondSword, 69),
            (ItemId::Bucket, 70),
            (ItemId::WaterBucket, 71),
            (ItemId::CowSpawnEgg, 72),
            (ItemId::PigSpawnEgg, 73),
            (ItemId::ChickenSpawnEgg, 74),
            (ItemId::RabbitSpawnEgg, 75),
            (ItemId::WolfSpawnEgg, 76),
            (ItemId::HorseSpawnEgg, 77),
            (ItemId::GoatSpawnEgg, 78),
            (ItemId::CatSpawnEgg, 79),
            (ItemId::SpruceLeaves, 80),
            (ItemId::HewnStone, 81),
            (ItemId::Limestone, 82),
            (ItemId::Slate, 83),
            (ItemId::Clay, 84),
            (ItemId::BrickWall, 85),
            (ItemId::RammedEarth, 86),
            (ItemId::Thatch, 87),
            (ItemId::CoalBlock, 88),
            (ItemId::CopperOre, 89),
            (ItemId::TinOre, 90),
            (ItemId::CopperBlock, 91),
            (ItemId::TinBlock, 92),
            (ItemId::BronzeBlock, 93),
            (ItemId::WillowLog, 94),
            (ItemId::WillowPlanks, 95),
            (ItemId::WillowLeaves, 96),
            (ItemId::AcaciaLog, 97),
            (ItemId::AcaciaPlanks, 98),
            (ItemId::AcaciaLeaves, 99),
            (ItemId::Reed, 100),
            (ItemId::Heather, 101),
            (ItemId::Mushroom, 102),
            (ItemId::WoodenSlab, 103),
            (ItemId::HewnStoneSlab, 104),
            (ItemId::BrickSlab, 105),
            (ItemId::Lantern, 106),
            (ItemId::FiredBrick, 107),
            (ItemId::Fiber, 108),
            (ItemId::Rope, 109),
            (ItemId::Flint, 110),
            (ItemId::CopperIngot, 111),
            (ItemId::TinIngot, 112),
            (ItemId::BronzeIngot, 113),
            (ItemId::RoastMushroom, 114),
            (ItemId::BronzePickaxe, 115),
            (ItemId::BronzeAxe, 116),
            (ItemId::BronzeShovel, 117),
            (ItemId::BronzeHoe, 118),
            (ItemId::BronzeSword, 119),
            (ItemId::WoodenStairs, 120),
            (ItemId::HewnStoneStairs, 121),
            (ItemId::BrickStairs, 122),
            (ItemId::Door, 123),
            (ItemId::Ladder, 124),
            (ItemId::Fence, 125),
            (ItemId::GlassPane, 126),
            (ItemId::OakSapling, 127),
            (ItemId::SpruceSapling, 128),
            (ItemId::JungleSapling, 129),
            (ItemId::WillowSapling, 130),
            (ItemId::AcaciaSapling, 131),
            (ItemId::Wool, 132),
            (ItemId::BerryBush, 133),
            (ItemId::Bedroll, 134),
            (ItemId::Barrel, 135),
            (ItemId::RainCistern, 136),
            (ItemId::Campfire, 137),
            (ItemId::Sign, 138),
            (ItemId::Rye, 139),
            (ItemId::RyeBread, 140),
            (ItemId::RawMutton, 141),
            (ItemId::RoastMutton, 142),
            (ItemId::Berries, 143),
            (ItemId::Spear, 144),
            (ItemId::PortalGun, 145),
            (ItemId::FlintAndSteel, 146),
            (ItemId::Obsidian, 147),
            (ItemId::LavaBucket, 148),
            (ItemId::RawPork, 149),
            (ItemId::RoastPork, 150),
            (ItemId::RawBeef, 151),
            (ItemId::RoastBeef, 152),
            (ItemId::RawChicken, 153),
            (ItemId::RoastChicken, 154),
            (ItemId::RawRabbit, 155),
            (ItemId::RoastRabbit, 156),
            (ItemId::Leather, 157),
            (ItemId::Feather, 158),
            (ItemId::Carrot, 159),
            (ItemId::Potato, 160),
            (ItemId::BakedPotato, 161),
            (ItemId::GnawerSpawnEgg, 162),
            (ItemId::BrambleSpawnEgg, 163),
            (ItemId::RollerSpawnEgg, 164),
            (ItemId::StumplingSpawnEgg, 165),
            (ItemId::DrownerSpawnEgg, 166),
            (ItemId::CinderlingSpawnEgg, 167),
            (ItemId::GustOwlSpawnEgg, 168),
            (ItemId::LeatherHelmet, 169),
            (ItemId::LeatherChestplate, 170),
            (ItemId::LeatherLeggings, 171),
            (ItemId::LeatherBoots, 172),
            (ItemId::BronzeHelmet, 173),
            (ItemId::BronzeChestplate, 174),
            (ItemId::BronzeLeggings, 175),
            (ItemId::BronzeBoots, 176),
            (ItemId::IronHelmet, 177),
            (ItemId::IronChestplate, 178),
            (ItemId::IronLeggings, 179),
            (ItemId::IronBoots, 180),
            (ItemId::DiamondHelmet, 181),
            (ItemId::DiamondChestplate, 182),
            (ItemId::DiamondLeggings, 183),
            (ItemId::DiamondBoots, 184),
            (ItemId::Bow, 185),
            (ItemId::Arrow, 186),
            (ItemId::Coin, 187),
            (ItemId::FarmerSpawnEgg, 188),
            (ItemId::SmithSpawnEgg, 189),
            (ItemId::CartographerSpawnEgg, 190),
            (ItemId::DepthsCompass, 191),
            (ItemId::EmberHeart, 192),
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
                | ItemId::CowSpawnEgg
                | ItemId::PigSpawnEgg
                | ItemId::ChickenSpawnEgg
                | ItemId::RabbitSpawnEgg
                | ItemId::WolfSpawnEgg
                | ItemId::HorseSpawnEgg
                | ItemId::GoatSpawnEgg
                | ItemId::CatSpawnEgg
                | ItemId::GnawerSpawnEgg
                | ItemId::BrambleSpawnEgg
                | ItemId::RollerSpawnEgg
                | ItemId::StumplingSpawnEgg
                | ItemId::DrownerSpawnEgg
                | ItemId::CinderlingSpawnEgg
                | ItemId::GustOwlSpawnEgg
                | ItemId::LeatherHelmet
                | ItemId::LeatherChestplate
                | ItemId::LeatherLeggings
                | ItemId::LeatherBoots
                | ItemId::BronzeHelmet
                | ItemId::BronzeChestplate
                | ItemId::BronzeLeggings
                | ItemId::BronzeBoots
                | ItemId::IronHelmet
                | ItemId::IronChestplate
                | ItemId::IronLeggings
                | ItemId::IronBoots
                | ItemId::DiamondHelmet
                | ItemId::DiamondChestplate
                | ItemId::DiamondLeggings
                | ItemId::DiamondBoots
                | ItemId::Coin
                | ItemId::FarmerSpawnEgg
                | ItemId::SmithSpawnEgg
                | ItemId::CartographerSpawnEgg
                | ItemId::Torch
                | ItemId::JungleLog
                | ItemId::JunglePlanks
                | ItemId::JungleLeaves
                | ItemId::Fern
                | ItemId::Vine
                | ItemId::CocoaBeans
                | ItemId::IronIngot
                | ItemId::GoldIngot
                | ItemId::Diamond
                | ItemId::Charcoal
                | ItemId::DiamondOre
                | ItemId::IronBlock
                | ItemId::GoldBlock
                | ItemId::DiamondBlock
                | ItemId::StonePickaxe
                | ItemId::StoneAxe
                | ItemId::StoneShovel
                | ItemId::StoneHoe
                | ItemId::StoneSword
                | ItemId::IronPickaxe
                | ItemId::IronAxe
                | ItemId::IronShovel
                | ItemId::IronHoe
                | ItemId::IronSword
                | ItemId::GoldenPickaxe
                | ItemId::GoldenAxe
                | ItemId::GoldenShovel
                | ItemId::GoldenHoe
                | ItemId::GoldenSword
                | ItemId::DiamondPickaxe
                | ItemId::DiamondAxe
                | ItemId::DiamondShovel
                | ItemId::DiamondHoe
                | ItemId::DiamondSword
                | ItemId::Bucket
                | ItemId::WaterBucket
                | ItemId::SpruceLeaves
                | ItemId::HewnStone
                | ItemId::Limestone
                | ItemId::Slate
                | ItemId::Clay
                | ItemId::BrickWall
                | ItemId::RammedEarth
                | ItemId::Thatch
                | ItemId::CoalBlock
                | ItemId::CopperOre
                | ItemId::TinOre
                | ItemId::CopperBlock
                | ItemId::TinBlock
                | ItemId::BronzeBlock
                | ItemId::WillowLog
                | ItemId::WillowPlanks
                | ItemId::WillowLeaves
                | ItemId::AcaciaLog
                | ItemId::AcaciaPlanks
                | ItemId::AcaciaLeaves
                | ItemId::Reed
                | ItemId::Heather
                | ItemId::Mushroom
                | ItemId::WoodenSlab
                | ItemId::HewnStoneSlab
                | ItemId::BrickSlab
                | ItemId::Lantern
                | ItemId::FiredBrick
                | ItemId::Fiber
                | ItemId::Rope
                | ItemId::Flint
                | ItemId::CopperIngot
                | ItemId::TinIngot
                | ItemId::BronzeIngot
                | ItemId::RoastMushroom
                | ItemId::BronzePickaxe
                | ItemId::BronzeAxe
                | ItemId::BronzeShovel
                | ItemId::BronzeHoe
                | ItemId::BronzeSword
                | ItemId::WoodenStairs
                | ItemId::HewnStoneStairs
                | ItemId::BrickStairs
                | ItemId::Door
                | ItemId::Ladder
                | ItemId::Fence
                | ItemId::GlassPane
                | ItemId::OakSapling
                | ItemId::SpruceSapling
                | ItemId::JungleSapling
                | ItemId::WillowSapling
                | ItemId::AcaciaSapling
                | ItemId::Wool
                | ItemId::BerryBush
                | ItemId::Bedroll
                | ItemId::Barrel
                | ItemId::RainCistern
                | ItemId::Campfire
                | ItemId::Sign
                | ItemId::Rye
                | ItemId::RyeBread
                | ItemId::RawMutton
                | ItemId::RoastMutton
                | ItemId::Berries
                | ItemId::Spear
                | ItemId::PortalGun
                | ItemId::FlintAndSteel
                | ItemId::Obsidian
                | ItemId::LavaBucket
                | ItemId::RawPork
                | ItemId::RoastPork
                | ItemId::RawBeef
                | ItemId::RoastBeef
                | ItemId::RawChicken
                | ItemId::RoastChicken
                | ItemId::RawRabbit
                | ItemId::RoastRabbit
                | ItemId::Leather
                | ItemId::Feather
                | ItemId::Carrot
                | ItemId::Potato
                | ItemId::BakedPotato
                | ItemId::Bow
                | ItemId::Arrow
                | ItemId::DepthsCompass
                | ItemId::EmberHeart => {}
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
        assert_eq!(ItemId::from_name("emerald"), None);
    }

    #[test]
    fn the_flint_and_steel_wears_out_in_64_fires_and_is_no_mining_tool() {
        let lighter = ItemId::FlintAndSteel;
        assert_eq!(lighter.durability(), Some(64));
        assert_eq!(lighter.tool(), None);
        assert_eq!(lighter.get_max_stack(), 1);
        assert_eq!(lighter.get_default_type(), ItemType::Tool { wear: 0 });
        assert_eq!(lighter.category(), ItemCategory::Tools);
        assert_eq!(lighter.name(), "flint_and_steel");
        let flint = ItemId::Flint;
        assert_eq!(flint.durability(), None);
        assert_eq!(flint.get_default_type(), ItemType::Generic);
        assert_eq!(flint.category(), ItemCategory::Materials);
        assert_eq!(flint.get_max_stack(), 64);
        // a tool lasts as long as its tier
        let pickaxe = ItemId::IronPickaxe;
        assert_eq!(
            pickaxe.durability(),
            Some(pickaxe.tool().unwrap().durability)
        );
    }

    #[test]
    fn a_spawn_egg_names_its_mob_and_is_named_after_it() {
        assert_eq!(ItemId::FoxSpawnEgg.spawned_mob(), Some(MobKind::Fox));
        assert_eq!(ItemId::SheepSpawnEgg.spawned_mob(), Some(MobKind::Sheep));
        assert_eq!(
            ItemId::from_name("sheep_spawn_egg"),
            Some(ItemId::SheepSpawnEgg)
        );
        // every mob has its egg, but a projectile and the boss
        for kind in MobKind::ALL {
            let eggs = ItemId::ALL
                .iter()
                .filter(|item| item.spawned_mob() == Some(kind));
            let expected = usize::from(!kind.is_projectile() && !kind.is_boss());
            assert_eq!(eggs.count(), expected, "{kind:?}");
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
    fn the_tiers_keep_their_numbers() {
        for (tier, speed, durability, mining_level) in [
            (ToolTier::Wood, 2, 59, 0),
            (ToolTier::Stone, 4, 131, 1),
            (ToolTier::Iron, 6, 250, 2),
            (ToolTier::Diamond, 8, 1561, 3),
            (ToolTier::Gold, 12, 32, 0),
            (ToolTier::Bronze, 5, 190, 1),
        ] {
            assert_eq!(
                (tier.speed(), tier.durability(), tier.mining_level()),
                (speed, durability, mining_level),
                "{tier:?}"
            );
        }
    }

    /// Every tool, its tier and its kind.
    fn every_tool() -> Vec<(ItemId, ToolTier, ToolKind)> {
        let tiers = [
            (
                ToolTier::Wood,
                [
                    ItemId::WoodenPickaxe,
                    ItemId::WoodenAxe,
                    ItemId::WoodenShovel,
                    ItemId::WoodenHoe,
                    ItemId::WoodenSword,
                ],
            ),
            (
                ToolTier::Stone,
                [
                    ItemId::StonePickaxe,
                    ItemId::StoneAxe,
                    ItemId::StoneShovel,
                    ItemId::StoneHoe,
                    ItemId::StoneSword,
                ],
            ),
            (
                ToolTier::Iron,
                [
                    ItemId::IronPickaxe,
                    ItemId::IronAxe,
                    ItemId::IronShovel,
                    ItemId::IronHoe,
                    ItemId::IronSword,
                ],
            ),
            (
                ToolTier::Gold,
                [
                    ItemId::GoldenPickaxe,
                    ItemId::GoldenAxe,
                    ItemId::GoldenShovel,
                    ItemId::GoldenHoe,
                    ItemId::GoldenSword,
                ],
            ),
            (
                ToolTier::Diamond,
                [
                    ItemId::DiamondPickaxe,
                    ItemId::DiamondAxe,
                    ItemId::DiamondShovel,
                    ItemId::DiamondHoe,
                    ItemId::DiamondSword,
                ],
            ),
            (
                ToolTier::Bronze,
                [
                    ItemId::BronzePickaxe,
                    ItemId::BronzeAxe,
                    ItemId::BronzeShovel,
                    ItemId::BronzeHoe,
                    ItemId::BronzeSword,
                ],
            ),
        ];
        let kinds = [
            ToolKind::Pickaxe,
            ToolKind::Axe,
            ToolKind::Shovel,
            ToolKind::Hoe,
            ToolKind::Sword,
        ];
        tiers
            .into_iter()
            .flat_map(|(tier, items)| {
                items
                    .into_iter()
                    .zip(kinds)
                    .map(move |(item, kind)| (item, tier, kind))
            })
            .collect()
    }

    #[test]
    fn every_tool_is_an_unworn_tool_of_its_tier_that_stacks_to_one() {
        let tools = every_tool();
        for &(item, tier, kind) in &tools {
            let tool = item.tool().unwrap();
            assert_eq!((tool.kind, tool.tier), (kind, tier), "{item:?}");
            assert_eq!(
                (tool.speed, tool.durability),
                (tier.speed(), tier.durability())
            );
            // A weapon: mining wears it twice as fast
            let wear = if kind == ToolKind::Sword { 2 } else { 1 };
            assert_eq!(tool.wear_per_block, wear, "{item:?}");
            assert_eq!(item.get_max_stack(), 1, "{item:?}");
            assert_eq!(item.get_default_type(), ItemType::Tool { wear: 0 });
        }
        let buckets = [
            ItemId::Bucket,
            ItemId::WaterBucket,
            ItemId::LavaBucket,
            ItemId::Spear,
            ItemId::FlintAndSteel,
            ItemId::Bow,
        ];
        let others = ItemId::ALL
            .iter()
            .filter(|item| !tools.iter().any(|(t, ..)| t == *item) && !buckets.contains(item))
            .filter(|item| **item != ItemId::PortalGun && item.armour().is_none());
        for item in others {
            assert_eq!(item.tool(), None, "{item:?}");
            assert_eq!(item.get_max_stack(), 64, "{item:?}");
        }
    }

    #[test]
    fn ingots_and_diamonds_are_materials_and_their_blocks_are_blocks() {
        for item in [
            ItemId::IronIngot,
            ItemId::GoldIngot,
            ItemId::Diamond,
            ItemId::Charcoal,
        ] {
            assert_eq!(item.category(), ItemCategory::Materials, "{item:?}");
            assert_eq!(item.get_default_type(), ItemType::Generic, "{item:?}");
        }
        for (item, block, category) in [
            (ItemId::DiamondOre, BlockId::DiamondOre, ItemCategory::Ores),
            (ItemId::IronBlock, BlockId::IronBlock, ItemCategory::Blocks),
            (ItemId::GoldBlock, BlockId::GoldBlock, ItemCategory::Blocks),
            (
                ItemId::DiamondBlock,
                BlockId::DiamondBlock,
                ItemCategory::Blocks,
            ),
        ] {
            assert_eq!(item.category(), category, "{item:?}");
            assert_eq!(item.get_default_type(), ItemType::Block(block), "{item:?}");
        }
        assert_eq!(ItemId::GoldenPickaxe.name(), "golden_pickaxe");
    }

    #[test]
    fn a_bucket_stacks_to_16_and_a_full_bucket_to_1() {
        for (item, name, max_stack) in [
            (ItemId::Bucket, "bucket", 16),
            (ItemId::WaterBucket, "water_bucket", 1),
            (ItemId::LavaBucket, "lava_bucket", 1),
        ] {
            assert_eq!(item.get_max_stack(), max_stack, "{item:?}");
            assert_eq!(item.name(), name);
            assert_eq!(item.category(), ItemCategory::Tools, "{item:?}");
            assert_eq!(item.get_default_type(), ItemType::Generic, "{item:?}");
            assert_eq!(item.tool(), None, "{item:?}");
        }
    }

    #[test]
    fn jungle_wood_is_filed_with_oak_wood() {
        for (jungle, oak) in [
            (ItemId::JungleLog, ItemId::OakLog),
            (ItemId::JunglePlanks, ItemId::OakPlanks),
            (ItemId::JungleLeaves, ItemId::OakLeaves),
        ] {
            assert_eq!(jungle.category(), oak.category(), "{jungle:?}");
        }
    }

    #[test]
    fn the_meats_are_eaten_raw_or_roasted_and_leather_and_feathers_are_materials() {
        for (item, food) in [
            (ItemId::RawMutton, Some(2)),
            (ItemId::RoastMutton, Some(6)),
            (ItemId::RawPork, Some(3)),
            (ItemId::RoastPork, Some(8)),
            (ItemId::RawBeef, Some(3)),
            (ItemId::RoastBeef, Some(8)),
            (ItemId::RawChicken, Some(2)),
            (ItemId::RoastChicken, Some(6)),
            (ItemId::RawRabbit, Some(3)),
            (ItemId::RoastRabbit, Some(5)),
            (ItemId::Leather, None),
            (ItemId::Feather, None),
        ] {
            assert_eq!(item.food_value().map(|(f, _)| f), food, "{item:?}");
            assert_eq!(item.category(), ItemCategory::Materials, "{item:?}");
            assert_eq!(item.get_default_type(), ItemType::Generic, "{item:?}");
            assert_eq!(item.get_max_stack(), 64, "{item:?}");
        }
        assert_eq!(ItemId::RawPork.name(), "raw_pork");
    }

    #[test]
    fn every_food_gives_food_and_a_saturation_no_greater_than_twice_its_food() {
        for (item, value) in [
            (ItemId::Berries, (2, 0.4)),
            (ItemId::RoastMushroom, (4, 4.8)),
            (ItemId::RyeBread, (5, 6.0)),
            (ItemId::RawMutton, (2, 1.2)),
            (ItemId::RoastMutton, (6, 9.6)),
            (ItemId::RawPork, (3, 1.8)),
            (ItemId::RoastPork, (8, 12.8)),
            (ItemId::RawBeef, (3, 1.8)),
            (ItemId::RoastBeef, (8, 12.8)),
            (ItemId::RawChicken, (2, 1.2)),
            (ItemId::RoastChicken, (6, 7.2)),
            (ItemId::RawRabbit, (3, 1.8)),
            (ItemId::RoastRabbit, (5, 6.0)),
            (ItemId::Carrot, (3, 3.6)),
            (ItemId::Potato, (1, 0.6)),
            (ItemId::BakedPotato, (5, 6.0)),
        ] {
            assert_eq!(item.food_value(), Some(value), "{item:?}");
        }
        for item in ItemId::ALL {
            if let Some((food, saturation)) = item.food_value() {
                assert!(food > 0 && saturation <= 2.0 * food as f32, "{item:?}");
            }
        }
        assert_eq!(ItemId::Stick.food_value(), None);
    }

    #[test]
    fn a_blow_wears_a_sword_by_one_another_tool_by_two_and_nothing_else() {
        for (item, wear) in [
            (ItemId::IronSword, 1),
            (ItemId::WoodenSword, 1),
            (ItemId::IronPickaxe, 2),
            (ItemId::StoneAxe, 2),
            (ItemId::GoldenHoe, 2),
            (ItemId::Spear, 0),
            (ItemId::FlintAndSteel, 0),
            (ItemId::Stick, 0),
        ] {
            assert_eq!(item.blow_wear(), wear, "{item:?}");
        }
    }

    #[test]
    fn every_item_has_a_category() {
        for item in ItemId::ALL {
            assert!(ItemCategory::ALL.contains(&item.category()), "{item:?}");
        }
        for category in ItemCategory::ALL {
            assert!(
                ItemId::ALL.iter().any(|item| item.category() == category),
                "{category:?} holds no item"
            );
        }
    }

    #[test]
    fn items_fall_in_the_categories_of_the_browser() {
        for (item, category) in [
            (ItemId::Dirt, ItemCategory::Blocks),
            (ItemId::Bedrock, ItemCategory::Blocks),
            (ItemId::OakPlanks, ItemCategory::Wood),
            (ItemId::Poppy, ItemCategory::Nature),
            (ItemId::IronOre, ItemCategory::Ores),
            (ItemId::Stick, ItemCategory::Materials),
            (ItemId::Chest, ItemCategory::Stations),
            (ItemId::WoodenSword, ItemCategory::Tools),
            (ItemId::SheepSpawnEgg, ItemCategory::Mobs),
        ] {
            assert_eq!(item.category(), category, "{item:?}");
        }
        // The tools, the buckets and the flint and steel, as Minecraft's "Tools & Utilities"
        // The portal gun is filed with the tools, though it breaks no block
        assert_eq!(ItemId::PortalGun.category(), ItemCategory::Tools);
        for item in ItemId::ALL {
            let bucket = matches!(
                item,
                ItemId::Bucket
                    | ItemId::WaterBucket
                    | ItemId::LavaBucket
                    | ItemId::Spear
                    | ItemId::FlintAndSteel
                    | ItemId::Bow
                    | ItemId::Arrow
                    | ItemId::DepthsCompass
            );
            assert_eq!(
                item.category() == ItemCategory::Tools,
                item.tool().is_some()
                    || item.armour().is_some()
                    || bucket
                    || item == ItemId::PortalGun,
                "{item:?}"
            );
        }
    }

    #[test]
    fn categories_have_names() {
        assert_eq!(
            ItemCategory::ALL.map(ItemCategory::name),
            [
                "Blocks",
                "Wood",
                "Nature",
                "Ores",
                "Materials",
                "Stations",
                "Tools",
                "Mobs"
            ]
        );
    }

    #[test]
    fn the_portal_gun_is_no_tool_and_stacks_to_one() {
        assert_eq!(ItemId::PortalGun.tool(), None);
        assert_eq!(ItemId::PortalGun.get_max_stack(), 1);
        assert_eq!(ItemId::PortalGun.get_default_type(), ItemType::Generic);
        assert_eq!(ItemId::PortalGun.name(), "portal_gun");
        assert_eq!(ItemId::ALL[145], ItemId::PortalGun);
    }
}
