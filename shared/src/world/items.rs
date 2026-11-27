use std::fmt::Debug;

use serde::de::value::{Error as ValueError, StrDeserializer};
use serde::{Deserialize, Serialize};

use super::{BlockId, Enchantments, GameElementId, ItemName, MobKind};
use bevy::math::IVec3;

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
    /// The slab of oak, once the only wooden slab.
    #[serde(alias = "WoodenSlab")]
    OakSlab,
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
    #[serde(alias = "WoodenStairs")]
    OakStairs,
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
    EngravingTable,
    Gloomrock,
    CobbledGloomrock,
    /// Repairs, merges and renames what wears: see `anvil`.
    Anvil,
    /// Put on a tamed horse, it lets its owner ride it (`players::riding`).
    Saddle,
    /// Put on the water, a boat of planks for two (`players::riding`).
    Boat,
    // The palette: see `palette`
    SpruceSlab,
    JungleSlab,
    WillowSlab,
    AcaciaSlab,
    SpruceStairs,
    JungleStairs,
    WillowStairs,
    AcaciaStairs,
    SpruceDoor,
    JungleDoor,
    WillowDoor,
    AcaciaDoor,
    OakTrapdoor,
    SpruceTrapdoor,
    JungleTrapdoor,
    WillowTrapdoor,
    AcaciaTrapdoor,
    OakFenceGate,
    SpruceFenceGate,
    JungleFenceGate,
    WillowFenceGate,
    AcaciaFenceGate,
    StoneSlab,
    CobblestoneSlab,
    LimestoneSlab,
    SlateSlab,
    GloomrockSlab,
    CobbledGloomrockSlab,
    StoneStairs,
    CobblestoneStairs,
    LimestoneStairs,
    SlateStairs,
    GloomrockStairs,
    CobbledGloomrockStairs,
    StoneLowWall,
    CobblestoneLowWall,
    HewnStoneLowWall,
    LimestoneLowWall,
    SlateLowWall,
    BrickLowWall,
    GloomrockLowWall,
    CobbledGloomrockLowWall,
    PolishedStone,
    PolishedLimestone,
    PolishedSlate,
    PolishedGloomrock,
    ChiseledStone,
    ChiseledLimestone,
    ChiseledSlate,
    ChiseledGloomrock,
    OrangeWool,
    MagentaWool,
    LightBlueWool,
    YellowWool,
    LimeWool,
    PinkWool,
    GrayWool,
    LightGrayWool,
    CyanWool,
    PurpleWool,
    BlueWool,
    BrownWool,
    GreenWool,
    RedWool,
    BlackWool,
    WhiteStainedGlass,
    OrangeStainedGlass,
    MagentaStainedGlass,
    LightBlueStainedGlass,
    YellowStainedGlass,
    LimeStainedGlass,
    PinkStainedGlass,
    GrayStainedGlass,
    LightGrayStainedGlass,
    CyanStainedGlass,
    PurpleStainedGlass,
    BlueStainedGlass,
    BrownStainedGlass,
    GreenStainedGlass,
    RedStainedGlass,
    BlackStainedGlass,
    WhiteStainedGlassPane,
    OrangeStainedGlassPane,
    MagentaStainedGlassPane,
    LightBlueStainedGlassPane,
    YellowStainedGlassPane,
    LimeStainedGlassPane,
    PinkStainedGlassPane,
    GrayStainedGlassPane,
    LightGrayStainedGlassPane,
    CyanStainedGlassPane,
    PurpleStainedGlassPane,
    BlueStainedGlassPane,
    BrownStainedGlassPane,
    GreenStainedGlassPane,
    RedStainedGlassPane,
    BlackStainedGlassPane,
    Terracotta,
    WhiteTerracotta,
    OrangeTerracotta,
    MagentaTerracotta,
    LightBlueTerracotta,
    YellowTerracotta,
    LimeTerracotta,
    PinkTerracotta,
    GrayTerracotta,
    LightGrayTerracotta,
    CyanTerracotta,
    PurpleTerracotta,
    BlueTerracotta,
    BrownTerracotta,
    GreenTerracotta,
    RedTerracotta,
    BlackTerracotta,
    WhiteConcrete,
    OrangeConcrete,
    MagentaConcrete,
    LightBlueConcrete,
    YellowConcrete,
    LimeConcrete,
    PinkConcrete,
    GrayConcrete,
    LightGrayConcrete,
    CyanConcrete,
    PurpleConcrete,
    BlueConcrete,
    BrownConcrete,
    GreenConcrete,
    RedConcrete,
    BlackConcrete,
    WhiteConcretePowder,
    OrangeConcretePowder,
    MagentaConcretePowder,
    LightBlueConcretePowder,
    YellowConcretePowder,
    LimeConcretePowder,
    PinkConcretePowder,
    GrayConcretePowder,
    LightGrayConcretePowder,
    CyanConcretePowder,
    PurpleConcretePowder,
    BlueConcretePowder,
    BrownConcretePowder,
    GreenConcretePowder,
    RedConcretePowder,
    BlackConcretePowder,
    WhiteDye,
    OrangeDye,
    MagentaDye,
    LightBlueDye,
    YellowDye,
    LimeDye,
    PinkDye,
    GrayDye,
    LightGrayDye,
    CyanDye,
    PurpleDye,
    BlueDye,
    BrownDye,
    GreenDye,
    RedDye,
    BlackDye,
    /// Raised by holding the right button, it parries what comes from the front
    /// (`players::shield`).
    Shield,
    /// Loaded slowly by holding the right button, it shoots a bolt once loaded (`players::bow`).
    Crossbow,
    /// What a crossbow shoots: it pierces a creature and goes on.
    Bolt,
    /// Thrown as an arrow is shot, it falls back as itself (`players::bow`).
    ThrowingAxe,
    /// A heavy weapon, slow, which throws back far (`players::melee`).
    Mace,
    /// Drunk, it takes every status effect away (`players::effects`).
    MilkBucket,
    // The maps: see `cartography`
    /// Made from reeds, for the maps.
    Paper,
    /// Drawn at once where it is used: it becomes a `FilledMap`.
    EmptyMap,
    /// A paper map, of `ItemType::Map`: shown while it is held, it fills as its holder walks.
    FilledMap,
    /// Its needle points to the bedroll of its holder, or to the spawn of the world.
    Compass,
    /// Shows the paper map put on it.
    MapFrame,
    /// Cast by a right click into the water, it catches what bites when drawn back in time
    /// (`world::fishing`). It wears by its catches.
    FishingRod,
    /// The fish of the rivers and lakes (`world::fishing`), raw and grilled.
    RawRoach,
    GrilledRoach,
    RawPerch,
    GrilledPerch,
    RawPike,
    GrilledPike,
    RawEel,
    GrilledEel,
    /// A rare fish whose scales glow: grilled, it heals a little (`food_effect`).
    RawGlowfin,
    GrilledGlowfin,
    /// Laid by the chickens now and then.
    Egg,
    FriedEgg,
    /// Fallen from the leaves of an apple tree.
    Apple,
    AppleSapling,
    AppleLeaves,
    /// What the soups and the stews are served in: eaten, they give it back (`eaten_remains`).
    Bowl,
    MushroomSoup,
    Stew,
    BerrySalad,
    /// A soup of fish and milk which heals (`food_effect`).
    FishChowder,
    /// Baked into a cocoa cake.
    CakeBatter,
    CocoaCake,
    /// Baked into an apple pie.
    RawPie,
    ApplePie,
    /// Grows a crop by a stage, ripens a bush, and may grow a sapling at once.
    Compost,
    // The oceans and the deep caves
    Kelp,
    Seagrass,
    AzureReef,
    RoseReef,
    AmberReef,
    Moss,
    CaveVine,
    Glowcrystal,
    CrystalCluster,
    Geodestone,
    Basalt,
    PolishedBasalt,
    Magma,
    /// What a glowing crystal breaks into.
    CrystalShard,
    /// Found in a wreck, of `ItemType::Treasure`: held, it marks where its chest is buried on
    /// the maps of its holder.
    TreasureMap,
    GlintfinSpawnEgg,
    ShoalbackSpawnEgg,
    MossbackSpawnEgg,
    ShardlingSpawnEgg,
    SlagcrawlerSpawnEgg,
    /// Put on a tamed horse, it spares it a part of the blows (`ItemId::horse_armour`) and
    /// shows on it.
    LeatherHorseArmour,
    IronHorseArmour,
    GoldHorseArmour,
    DiamondHorseArmour,
    /// Ties an animal to the hand, which it follows, or to a fence post (`ServerMob::leash`).
    Lead,
    DonkeySpawnEgg,
    MuleSpawnEgg,
    // The furniture and the decoration of a home: see `decor`
    WhiteBedroll,
    OrangeBedroll,
    MagentaBedroll,
    LightBlueBedroll,
    YellowBedroll,
    LimeBedroll,
    PinkBedroll,
    GrayBedroll,
    LightGrayBedroll,
    CyanBedroll,
    PurpleBedroll,
    BlueBedroll,
    BrownBedroll,
    GreenBedroll,
    BlackBedroll,
    OakTable,
    SpruceTable,
    JungleTable,
    WillowTable,
    AcaciaTable,
    OakChair,
    SpruceChair,
    JungleChair,
    WillowChair,
    AcaciaChair,
    /// Paper bound in leather, which shelves hold.
    Book,
    Bookshelf,
    WhiteCarpet,
    OrangeCarpet,
    MagentaCarpet,
    LightBlueCarpet,
    YellowCarpet,
    LimeCarpet,
    PinkCarpet,
    GrayCarpet,
    LightGrayCarpet,
    CyanCarpet,
    PurpleCarpet,
    BlueCarpet,
    BrownCarpet,
    GreenCarpet,
    RedCarpet,
    BlackCarpet,
    FlowerPot,
    ItemFrame,
    /// Hung on a wall, one of the canvases of one block, then of two blocks.
    Painting,
    WidePainting,
    /// Of `ItemType::Banner` once patterns are dyed on it.
    WhiteBanner,
    OrangeBanner,
    MagentaBanner,
    LightBlueBanner,
    YellowBanner,
    LimeBanner,
    PinkBanner,
    GrayBanner,
    LightGrayBanner,
    CyanBanner,
    PurpleBanner,
    BlueBanner,
    BrownBanner,
    GreenBanner,
    RedBanner,
    BlackBanner,
    WhiteLantern,
    OrangeLantern,
    MagentaLantern,
    LightBlueLantern,
    YellowLantern,
    LimeLantern,
    PinkLantern,
    GrayLantern,
    LightGrayLantern,
    CyanLantern,
    PurpleLantern,
    BlueLantern,
    BrownLantern,
    GreenLantern,
    RedLantern,
    BlackLantern,
    // Alchemy: see `alchemy`
    /// Distils the ingredients into potions, burning a fuel (`alchemy::distilled`).
    Alembic,
    /// Filled at a source of water.
    GlassVial,
    WaterVial,
    /// Water distilled with a mushroom, which every potion is made of.
    NeutralBase,
    /// Drunk, of `ItemType::Generic` or `ItemType::Brewed` (`alchemy::Brew`).
    HealingPotion,
    NightVisionPotion,
    WaterBreathingPotion,
    FireResistancePotion,
    SwiftnessPotion,
    StrengthPotion,
    RegenerationPotion,
    /// Thrown, they burst where they land and give their effect around (`alchemy::SPLASH_RADIUS`).
    SplashPoisonPotion,
    SplashSlownessPotion,
    SplashWeaknessPotion,
    /// The fang of a Gnawer, which distils into strength.
    GnawerFang,
}

impl ItemId {
    /// Every item, in the order of the variants.
    pub const ALL: [ItemId; 519] = [
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
        Self::OakSlab,
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
        Self::OakStairs,
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
        Self::EngravingTable,
        Self::Gloomrock,
        Self::CobbledGloomrock,
        Self::Anvil,
        Self::Saddle,
        Self::Boat,
        Self::SpruceSlab,
        Self::JungleSlab,
        Self::WillowSlab,
        Self::AcaciaSlab,
        Self::SpruceStairs,
        Self::JungleStairs,
        Self::WillowStairs,
        Self::AcaciaStairs,
        Self::SpruceDoor,
        Self::JungleDoor,
        Self::WillowDoor,
        Self::AcaciaDoor,
        Self::OakTrapdoor,
        Self::SpruceTrapdoor,
        Self::JungleTrapdoor,
        Self::WillowTrapdoor,
        Self::AcaciaTrapdoor,
        Self::OakFenceGate,
        Self::SpruceFenceGate,
        Self::JungleFenceGate,
        Self::WillowFenceGate,
        Self::AcaciaFenceGate,
        Self::StoneSlab,
        Self::CobblestoneSlab,
        Self::LimestoneSlab,
        Self::SlateSlab,
        Self::GloomrockSlab,
        Self::CobbledGloomrockSlab,
        Self::StoneStairs,
        Self::CobblestoneStairs,
        Self::LimestoneStairs,
        Self::SlateStairs,
        Self::GloomrockStairs,
        Self::CobbledGloomrockStairs,
        Self::StoneLowWall,
        Self::CobblestoneLowWall,
        Self::HewnStoneLowWall,
        Self::LimestoneLowWall,
        Self::SlateLowWall,
        Self::BrickLowWall,
        Self::GloomrockLowWall,
        Self::CobbledGloomrockLowWall,
        Self::PolishedStone,
        Self::PolishedLimestone,
        Self::PolishedSlate,
        Self::PolishedGloomrock,
        Self::ChiseledStone,
        Self::ChiseledLimestone,
        Self::ChiseledSlate,
        Self::ChiseledGloomrock,
        Self::OrangeWool,
        Self::MagentaWool,
        Self::LightBlueWool,
        Self::YellowWool,
        Self::LimeWool,
        Self::PinkWool,
        Self::GrayWool,
        Self::LightGrayWool,
        Self::CyanWool,
        Self::PurpleWool,
        Self::BlueWool,
        Self::BrownWool,
        Self::GreenWool,
        Self::RedWool,
        Self::BlackWool,
        Self::WhiteStainedGlass,
        Self::OrangeStainedGlass,
        Self::MagentaStainedGlass,
        Self::LightBlueStainedGlass,
        Self::YellowStainedGlass,
        Self::LimeStainedGlass,
        Self::PinkStainedGlass,
        Self::GrayStainedGlass,
        Self::LightGrayStainedGlass,
        Self::CyanStainedGlass,
        Self::PurpleStainedGlass,
        Self::BlueStainedGlass,
        Self::BrownStainedGlass,
        Self::GreenStainedGlass,
        Self::RedStainedGlass,
        Self::BlackStainedGlass,
        Self::WhiteStainedGlassPane,
        Self::OrangeStainedGlassPane,
        Self::MagentaStainedGlassPane,
        Self::LightBlueStainedGlassPane,
        Self::YellowStainedGlassPane,
        Self::LimeStainedGlassPane,
        Self::PinkStainedGlassPane,
        Self::GrayStainedGlassPane,
        Self::LightGrayStainedGlassPane,
        Self::CyanStainedGlassPane,
        Self::PurpleStainedGlassPane,
        Self::BlueStainedGlassPane,
        Self::BrownStainedGlassPane,
        Self::GreenStainedGlassPane,
        Self::RedStainedGlassPane,
        Self::BlackStainedGlassPane,
        Self::Terracotta,
        Self::WhiteTerracotta,
        Self::OrangeTerracotta,
        Self::MagentaTerracotta,
        Self::LightBlueTerracotta,
        Self::YellowTerracotta,
        Self::LimeTerracotta,
        Self::PinkTerracotta,
        Self::GrayTerracotta,
        Self::LightGrayTerracotta,
        Self::CyanTerracotta,
        Self::PurpleTerracotta,
        Self::BlueTerracotta,
        Self::BrownTerracotta,
        Self::GreenTerracotta,
        Self::RedTerracotta,
        Self::BlackTerracotta,
        Self::WhiteConcrete,
        Self::OrangeConcrete,
        Self::MagentaConcrete,
        Self::LightBlueConcrete,
        Self::YellowConcrete,
        Self::LimeConcrete,
        Self::PinkConcrete,
        Self::GrayConcrete,
        Self::LightGrayConcrete,
        Self::CyanConcrete,
        Self::PurpleConcrete,
        Self::BlueConcrete,
        Self::BrownConcrete,
        Self::GreenConcrete,
        Self::RedConcrete,
        Self::BlackConcrete,
        Self::WhiteConcretePowder,
        Self::OrangeConcretePowder,
        Self::MagentaConcretePowder,
        Self::LightBlueConcretePowder,
        Self::YellowConcretePowder,
        Self::LimeConcretePowder,
        Self::PinkConcretePowder,
        Self::GrayConcretePowder,
        Self::LightGrayConcretePowder,
        Self::CyanConcretePowder,
        Self::PurpleConcretePowder,
        Self::BlueConcretePowder,
        Self::BrownConcretePowder,
        Self::GreenConcretePowder,
        Self::RedConcretePowder,
        Self::BlackConcretePowder,
        Self::WhiteDye,
        Self::OrangeDye,
        Self::MagentaDye,
        Self::LightBlueDye,
        Self::YellowDye,
        Self::LimeDye,
        Self::PinkDye,
        Self::GrayDye,
        Self::LightGrayDye,
        Self::CyanDye,
        Self::PurpleDye,
        Self::BlueDye,
        Self::BrownDye,
        Self::GreenDye,
        Self::RedDye,
        Self::BlackDye,
        Self::Shield,
        Self::Crossbow,
        Self::Bolt,
        Self::ThrowingAxe,
        Self::Mace,
        Self::MilkBucket,
        Self::Paper,
        Self::EmptyMap,
        Self::FilledMap,
        Self::Compass,
        Self::MapFrame,
        Self::FishingRod,
        Self::RawRoach,
        Self::GrilledRoach,
        Self::RawPerch,
        Self::GrilledPerch,
        Self::RawPike,
        Self::GrilledPike,
        Self::RawEel,
        Self::GrilledEel,
        Self::RawGlowfin,
        Self::GrilledGlowfin,
        Self::Egg,
        Self::FriedEgg,
        Self::Apple,
        Self::AppleSapling,
        Self::AppleLeaves,
        Self::Bowl,
        Self::MushroomSoup,
        Self::Stew,
        Self::BerrySalad,
        Self::FishChowder,
        Self::CakeBatter,
        Self::CocoaCake,
        Self::RawPie,
        Self::ApplePie,
        Self::Compost,
        Self::Kelp,
        Self::Seagrass,
        Self::AzureReef,
        Self::RoseReef,
        Self::AmberReef,
        Self::Moss,
        Self::CaveVine,
        Self::Glowcrystal,
        Self::CrystalCluster,
        Self::Geodestone,
        Self::Basalt,
        Self::PolishedBasalt,
        Self::Magma,
        Self::CrystalShard,
        Self::TreasureMap,
        Self::GlintfinSpawnEgg,
        Self::ShoalbackSpawnEgg,
        Self::MossbackSpawnEgg,
        Self::ShardlingSpawnEgg,
        Self::SlagcrawlerSpawnEgg,
        Self::LeatherHorseArmour,
        Self::IronHorseArmour,
        Self::GoldHorseArmour,
        Self::DiamondHorseArmour,
        Self::Lead,
        Self::DonkeySpawnEgg,
        Self::MuleSpawnEgg,
        Self::WhiteBedroll,
        Self::OrangeBedroll,
        Self::MagentaBedroll,
        Self::LightBlueBedroll,
        Self::YellowBedroll,
        Self::LimeBedroll,
        Self::PinkBedroll,
        Self::GrayBedroll,
        Self::LightGrayBedroll,
        Self::CyanBedroll,
        Self::PurpleBedroll,
        Self::BlueBedroll,
        Self::BrownBedroll,
        Self::GreenBedroll,
        Self::BlackBedroll,
        Self::OakTable,
        Self::SpruceTable,
        Self::JungleTable,
        Self::WillowTable,
        Self::AcaciaTable,
        Self::OakChair,
        Self::SpruceChair,
        Self::JungleChair,
        Self::WillowChair,
        Self::AcaciaChair,
        Self::Book,
        Self::Bookshelf,
        Self::WhiteCarpet,
        Self::OrangeCarpet,
        Self::MagentaCarpet,
        Self::LightBlueCarpet,
        Self::YellowCarpet,
        Self::LimeCarpet,
        Self::PinkCarpet,
        Self::GrayCarpet,
        Self::LightGrayCarpet,
        Self::CyanCarpet,
        Self::PurpleCarpet,
        Self::BlueCarpet,
        Self::BrownCarpet,
        Self::GreenCarpet,
        Self::RedCarpet,
        Self::BlackCarpet,
        Self::FlowerPot,
        Self::ItemFrame,
        Self::Painting,
        Self::WidePainting,
        Self::WhiteBanner,
        Self::OrangeBanner,
        Self::MagentaBanner,
        Self::LightBlueBanner,
        Self::YellowBanner,
        Self::LimeBanner,
        Self::PinkBanner,
        Self::GrayBanner,
        Self::LightGrayBanner,
        Self::CyanBanner,
        Self::PurpleBanner,
        Self::BlueBanner,
        Self::BrownBanner,
        Self::GreenBanner,
        Self::RedBanner,
        Self::BlackBanner,
        Self::WhiteLantern,
        Self::OrangeLantern,
        Self::MagentaLantern,
        Self::LightBlueLantern,
        Self::YellowLantern,
        Self::LimeLantern,
        Self::PinkLantern,
        Self::GrayLantern,
        Self::LightGrayLantern,
        Self::CyanLantern,
        Self::PurpleLantern,
        Self::BlueLantern,
        Self::BrownLantern,
        Self::GreenLantern,
        Self::RedLantern,
        Self::BlackLantern,
        Self::Alembic,
        Self::GlassVial,
        Self::WaterVial,
        Self::NeutralBase,
        Self::HealingPotion,
        Self::NightVisionPotion,
        Self::WaterBreathingPotion,
        Self::FireResistancePotion,
        Self::SwiftnessPotion,
        Self::StrengthPotion,
        Self::RegenerationPotion,
        Self::SplashPoisonPotion,
        Self::SplashSlownessPotion,
        Self::SplashWeaknessPotion,
        Self::GnawerFang,
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
            Self::WaterBucket
            | Self::LavaBucket
            | Self::Spear
            | Self::PortalGun
            | Self::Saddle
            | Self::Boat
            | Self::LeatherHorseArmour
            | Self::IronHorseArmour
            | Self::GoldHorseArmour
            | Self::DiamondHorseArmour
            | Self::MilkBucket
            // each map is its own, and a compass is one
            | Self::FilledMap
            | Self::Compass
            | Self::TreasureMap
            | Self::MushroomSoup
            | Self::Stew
            | Self::BerrySalad
            | Self::FishChowder => 1,
            Self::ThrowingAxe | Self::Egg | Self::GlassVial => 16,
            // each banner may hold patterns of its own
            _ if self.banner_color().is_some() => 1,
            // A potion is one, so that its `ItemType::Brewed` never mixes with another's
            item if item.is_vial_filled() => 1,
            _ if self.durability().is_some() => 1,
            _ => 64,
        }
    }

    /// The points of health a blow with the item takes from a mob.
    pub fn attack_damage(&self) -> u32 {
        match *self {
            Self::Spear | Self::ThrowingAxe => return 5,
            Self::Mace => return 10,
            _ => {}
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
            // Heavy: slower than a sword (`melee::WeaponClass`), but harder
            ToolKind::Axe => 6 + tier,
            _ => 2 + tier,
        }
    }

    /// How much a blow on a mob wears the item: a sword is made for it, another tool wears as
    /// it does on a block it is not made for; the rest does not wear.
    pub fn blow_wear(&self) -> u32 {
        if *self == Self::Mace {
            return 1;
        }
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
            Self::RawRoach | Self::RawPerch | Self::RawEel => Some((2, 0.4)),
            Self::RawPike => Some((3, 1.2)),
            Self::RawGlowfin => Some((1, 0.2)),
            Self::GrilledRoach | Self::GrilledGlowfin => Some((5, 6.0)),
            Self::GrilledPerch => Some((6, 7.2)),
            Self::GrilledEel => Some((6, 9.6)),
            Self::GrilledPike => Some((7, 9.6)),
            Self::FriedEgg => Some((4, 4.8)),
            Self::Apple => Some((4, 2.4)),
            // More than their ingredients eaten one by one
            Self::MushroomSoup => Some((7, 8.4)),
            Self::Stew => Some((14, 16.8)),
            Self::BerrySalad => Some((10, 9.6)),
            Self::FishChowder => Some((10, 12.0)),
            Self::CocoaCake => Some((12, 14.4)),
            Self::ApplePie => Some((12, 12.0)),
            _ => None,
        }
    }

    /// The status effect eating the item gives, if any: its kind, its level and its ticks. The
    /// healing foods may be eaten with all the food a player can have.
    pub fn food_effect(&self) -> Option<(crate::players::effects::EffectKind, u8, u32)> {
        use crate::players::effects::EffectKind::Regeneration;
        let seconds = |s: u32| s * crate::TICKS_PER_SECOND as u32;
        match *self {
            Self::FishChowder => Some((Regeneration, 2, seconds(10))),
            Self::GrilledGlowfin => Some((Regeneration, 1, seconds(8))),
            _ => None,
        }
    }

    /// What is left in the hand once the item is eaten or drunk: the bowl of a soup.
    pub fn eaten_remains(&self) -> Option<ItemId> {
        match *self {
            Self::MushroomSoup | Self::Stew | Self::BerrySalad | Self::FishChowder => {
                Some(Self::Bowl)
            }
            _ => None,
        }
    }

    /// What is left in its cell of the crafting grid once the item is used by a craft: the
    /// bucket of a bucket of milk.
    pub fn crafting_remains(&self) -> Option<ItemId> {
        match *self {
            Self::MilkBucket | Self::WaterBucket => Some(Self::Bucket),
            _ => None,
        }
    }

    /// The points of protection of a horse armour (`mobs::armoured_damage`): Minecraft's.
    pub fn horse_armour(&self) -> Option<u32> {
        match *self {
            Self::LeatherHorseArmour => Some(3),
            Self::IronHorseArmour => Some(5),
            Self::GoldHorseArmour => Some(7),
            Self::DiamondHorseArmour => Some(11),
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
            Self::DonkeySpawnEgg => Some(MobKind::Donkey),
            Self::MuleSpawnEgg => Some(MobKind::Mule),
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
            Self::GlintfinSpawnEgg => Some(MobKind::Glintfin),
            Self::ShoalbackSpawnEgg => Some(MobKind::Shoalback),
            Self::MossbackSpawnEgg => Some(MobKind::Mossback),
            Self::ShardlingSpawnEgg => Some(MobKind::Shardling),
            Self::SlagcrawlerSpawnEgg => Some(MobKind::Slagcrawler),
            // Put down as a spawn egg, on the water it aims at
            Self::Boat => Some(MobKind::Boat),
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
            Self::Shield => Some(crate::players::shield::SHIELD_DURABILITY),
            Self::Crossbow => Some(465),
            Self::Mace => Some(500),
            Self::FishingRod => Some(64),
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
            Self::Book => ItemCategory::Materials,
            // the book was filed above
            #[allow(unreachable_patterns)]
            crate::new_decor_items!() if self.decor_block().is_some_and(BlockId::is_bedroll) => {
                ItemCategory::Stations
            }
            #[allow(unreachable_patterns)]
            crate::new_decor_items!() => ItemCategory::Blocks,
            Self::Dirt
            | Self::Grass
            | Self::Stone
            | Self::Cobblestone
            | Self::Gloomrock
            | Self::CobbledGloomrock
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
            | Self::OakSlab
            | Self::HewnStoneSlab
            | Self::BrickSlab
            | Self::Lantern
            | Self::OakStairs
            | Self::HewnStoneStairs
            | Self::BrickStairs
            | Self::Door
            | Self::Ladder
            | Self::Fence
            | Self::GlassPane
            | Self::Wool
            | Self::Sign
            | Self::Obsidian
            | Self::EmberHeart
            | Self::AzureReef
            | Self::RoseReef
            | Self::AmberReef
            | Self::Glowcrystal
            | Self::Geodestone
            | Self::Basalt
            | Self::PolishedBasalt
            | Self::Magma
            | crate::new_palette_block_items!() => ItemCategory::Blocks,
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
            | Self::BerryBush
            | Self::AppleSapling
            | Self::AppleLeaves
            | Self::Kelp
            | Self::Seagrass
            | Self::Moss
            | Self::CaveVine
            | Self::CrystalCluster => ItemCategory::Nature,
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
            | Self::BakedPotato
            | Self::Paper
            | Self::RawRoach
            | Self::GrilledRoach
            | Self::RawPerch
            | Self::GrilledPerch
            | Self::RawPike
            | Self::GrilledPike
            | Self::RawEel
            | Self::GrilledEel
            | Self::RawGlowfin
            | Self::GrilledGlowfin
            | Self::Egg
            | Self::FriedEgg
            | Self::Apple
            | Self::Bowl
            | Self::MushroomSoup
            | Self::Stew
            | Self::BerrySalad
            | Self::FishChowder
            | Self::CakeBatter
            | Self::CocoaCake
            | Self::RawPie
            | Self::ApplePie
            | Self::Compost
            | Self::CrystalShard
            | Self::GlassVial
            | Self::WaterVial
            | Self::NeutralBase
            | Self::HealingPotion
            | Self::NightVisionPotion
            | Self::WaterBreathingPotion
            | Self::FireResistancePotion
            | Self::SwiftnessPotion
            | Self::StrengthPotion
            | Self::RegenerationPotion
            | Self::SplashPoisonPotion
            | Self::SplashSlownessPotion
            | Self::SplashWeaknessPotion
            | Self::GnawerFang
            | crate::dye_items!() => ItemCategory::Materials,
            Self::CraftingTable
            | Self::Alembic
            | Self::EngravingTable
            | Self::Anvil
            | Self::Furnace
            | Self::Chest
            | Self::Bedroll
            | Self::Barrel
            | Self::RainCistern
            | Self::Campfire
            | Self::MapFrame => ItemCategory::Stations,
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
            | Self::DepthsCompass
            | Self::Saddle
            | Self::LeatherHorseArmour
            | Self::IronHorseArmour
            | Self::GoldHorseArmour
            | Self::DiamondHorseArmour
            | Self::Lead
            | Self::Boat
            | Self::Shield
            | Self::Crossbow
            | Self::Bolt
            | Self::ThrowingAxe
            | Self::Mace
            | Self::MilkBucket
            | Self::EmptyMap
            | Self::FilledMap
            | Self::TreasureMap
            | Self::Compass => ItemCategory::Tools,
            Self::FishingRod => ItemCategory::Tools,
            Self::FoxSpawnEgg
            | Self::SheepSpawnEgg
            | Self::CowSpawnEgg
            | Self::PigSpawnEgg
            | Self::ChickenSpawnEgg
            | Self::RabbitSpawnEgg
            | Self::WolfSpawnEgg
            | Self::HorseSpawnEgg
            | Self::DonkeySpawnEgg
            | Self::MuleSpawnEgg
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
            | Self::CartographerSpawnEgg
            | Self::GlintfinSpawnEgg
            | Self::ShoalbackSpawnEgg
            | Self::MossbackSpawnEgg
            | Self::ShardlingSpawnEgg
            | Self::SlagcrawlerSpawnEgg => ItemCategory::Mobs,
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
            Self::OakSlab => ItemType::Block(BlockId::OakSlab),
            Self::HewnStoneSlab => ItemType::Block(BlockId::HewnStoneSlab),
            Self::BrickSlab => ItemType::Block(BlockId::BrickSlab),
            Self::Lantern => ItemType::Block(BlockId::Lantern),
            Self::OakStairs => ItemType::Block(BlockId::OakStairs),
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
            Self::EngravingTable => ItemType::Block(BlockId::EngravingTable),
            Self::Gloomrock => ItemType::Block(BlockId::Gloomrock),
            Self::CobbledGloomrock => ItemType::Block(BlockId::CobbledGloomrock),
            Self::Anvil => ItemType::Block(BlockId::Anvil),
            Self::Rye => ItemType::Block(BlockId::RyeSprout),
            Self::Carrot => ItemType::Block(BlockId::CarrotSprout),
            Self::Potato => ItemType::Block(BlockId::PotatoSprout),
            Self::AppleSapling => ItemType::Block(BlockId::AppleSapling),
            Self::AppleLeaves => ItemType::Block(BlockId::AppleLeaves),
            crate::new_decor_items!() => match self.decor_block() {
                Some(block) => ItemType::Block(block),
                None => ItemType::Generic,
            },
            crate::new_palette_block_items!() => match self.palette_block() {
                Some(block) => ItemType::Block(block),
                None => ItemType::Generic,
            },

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
            | Self::DonkeySpawnEgg
            | Self::MuleSpawnEgg
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
            | Self::GlintfinSpawnEgg
            | Self::ShoalbackSpawnEgg
            | Self::MossbackSpawnEgg
            | Self::ShardlingSpawnEgg
            | Self::SlagcrawlerSpawnEgg
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
            | Self::BakedPotato
            | Self::Saddle
            | Self::LeatherHorseArmour
            | Self::IronHorseArmour
            | Self::GoldHorseArmour
            | Self::DiamondHorseArmour
            | Self::Lead
            | Self::Boat
            | Self::Paper
            | Self::EmptyMap
            | Self::Compass
            | crate::dye_items!()
            | Self::Bolt
            | Self::ThrowingAxe
            | Self::MilkBucket
            | Self::RawRoach
            | Self::GrilledRoach
            | Self::RawPerch
            | Self::GrilledPerch
            | Self::RawPike
            | Self::GrilledPike
            | Self::RawEel
            | Self::GrilledEel
            | Self::RawGlowfin
            | Self::GrilledGlowfin
            | Self::Egg
            | Self::FriedEgg
            | Self::Apple
            | Self::Bowl
            | Self::MushroomSoup
            | Self::Stew
            | Self::BerrySalad
            | Self::FishChowder
            | Self::CakeBatter
            | Self::CocoaCake
            | Self::RawPie
            | Self::ApplePie
            | Self::Compost => ItemType::Generic,
            Self::Shield | Self::Crossbow | Self::Mace | Self::FishingRod => {
                ItemType::Tool { wear: 0 }
            }
            // the server gives each its id as it is drawn (`cartography`)
            Self::FilledMap => ItemType::Map { id: 0 },
            Self::MapFrame => ItemType::Block(BlockId::MapFrame),
            Self::Kelp => ItemType::Block(BlockId::Kelp),
            Self::Seagrass => ItemType::Block(BlockId::Seagrass),
            Self::AzureReef => ItemType::Block(BlockId::AzureReef),
            Self::RoseReef => ItemType::Block(BlockId::RoseReef),
            Self::AmberReef => ItemType::Block(BlockId::AmberReef),
            Self::Moss => ItemType::Block(BlockId::Moss),
            Self::CaveVine => ItemType::Block(BlockId::CaveVine),
            Self::Glowcrystal => ItemType::Block(BlockId::Glowcrystal),
            Self::CrystalCluster => ItemType::Block(BlockId::CrystalCluster),
            Self::Geodestone => ItemType::Block(BlockId::Geodestone),
            Self::Basalt => ItemType::Block(BlockId::Basalt),
            Self::PolishedBasalt => ItemType::Block(BlockId::PolishedBasalt),
            Self::Magma => ItemType::Block(BlockId::Magma),
            Self::CrystalShard => ItemType::Generic,
            Self::Alembic => ItemType::Block(BlockId::Alembic),
            Self::GlassVial
            | Self::WaterVial
            | Self::NeutralBase
            | Self::HealingPotion
            | Self::NightVisionPotion
            | Self::WaterBreathingPotion
            | Self::FireResistancePotion
            | Self::SwiftnessPotion
            | Self::StrengthPotion
            | Self::RegenerationPotion
            | Self::SplashPoisonPotion
            | Self::SplashSlownessPotion
            | Self::SplashWeaknessPotion
            | Self::GnawerFang => ItemType::Generic,
            // the wreck that holds it gives it the place of its chest
            Self::TreasureMap => ItemType::Treasure { at: IVec3::ZERO },

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
    /// A tool, a weapon or a piece of armour with enchantments, and its wear.
    Enchanted {
        wear: u32,
        enchantments: Enchantments,
    },
    /// One worked at an anvil (`players::anvil`): its wear, its enchantments, how many times it
    /// was repaired or merged, which raises the price of the next time, and the name given to it.
    Forged {
        wear: u32,
        enchantments: Enchantments,
        repairs: u8,
        name: ItemName,
    },
    /// A paper map, and the id the server keeps its cells by (`cartography`).
    Map {
        id: super::cartography::MapId,
    },
    /// A treasure map, and where its chest is buried.
    Treasure {
        at: IVec3,
    },
    /// A banner, and the patterns dyed on it (`decor`).
    Banner {
        patterns: super::decor::BannerPatterns,
    },
    /// A potion distilled again with a modifier (`alchemy::Brew`): it lasts longer, or is
    /// stronger.
    Brewed {
        longer: bool,
        stronger: bool,
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

    /// The bytes of `value` as bincode's variable length encoding writes it: one byte below 251,
    /// else the marker 251 and two bytes.
    fn varint(value: u16) -> Vec<u8> {
        match u8::try_from(value) {
            Ok(byte) if byte < 251 => vec![byte],
            _ => [vec![251], value.to_le_bytes().to_vec()].concat(),
        }
    }

    #[test]
    fn item_encoding_is_pinned() {
        let table = [
            (ItemId::Dirt, 0u16),
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
            (ItemId::OakSlab, 103),
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
            (ItemId::OakStairs, 120),
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
            (ItemId::EngravingTable, 193),
            (ItemId::Gloomrock, 194),
            (ItemId::CobbledGloomrock, 195),
            (ItemId::Anvil, 196),
            (ItemId::Saddle, 197),
            (ItemId::Boat, 198),
            (ItemId::SpruceSlab, 199),
            (ItemId::JungleSlab, 200),
            (ItemId::WillowSlab, 201),
            (ItemId::AcaciaSlab, 202),
            (ItemId::SpruceStairs, 203),
            (ItemId::JungleStairs, 204),
            (ItemId::WillowStairs, 205),
            (ItemId::AcaciaStairs, 206),
            (ItemId::SpruceDoor, 207),
            (ItemId::JungleDoor, 208),
            (ItemId::WillowDoor, 209),
            (ItemId::AcaciaDoor, 210),
            (ItemId::OakTrapdoor, 211),
            (ItemId::SpruceTrapdoor, 212),
            (ItemId::JungleTrapdoor, 213),
            (ItemId::WillowTrapdoor, 214),
            (ItemId::AcaciaTrapdoor, 215),
            (ItemId::OakFenceGate, 216),
            (ItemId::SpruceFenceGate, 217),
            (ItemId::JungleFenceGate, 218),
            (ItemId::WillowFenceGate, 219),
            (ItemId::AcaciaFenceGate, 220),
            (ItemId::StoneSlab, 221),
            (ItemId::CobblestoneSlab, 222),
            (ItemId::LimestoneSlab, 223),
            (ItemId::SlateSlab, 224),
            (ItemId::GloomrockSlab, 225),
            (ItemId::CobbledGloomrockSlab, 226),
            (ItemId::StoneStairs, 227),
            (ItemId::CobblestoneStairs, 228),
            (ItemId::LimestoneStairs, 229),
            (ItemId::SlateStairs, 230),
            (ItemId::GloomrockStairs, 231),
            (ItemId::CobbledGloomrockStairs, 232),
            (ItemId::StoneLowWall, 233),
            (ItemId::CobblestoneLowWall, 234),
            (ItemId::HewnStoneLowWall, 235),
            (ItemId::LimestoneLowWall, 236),
            (ItemId::SlateLowWall, 237),
            (ItemId::BrickLowWall, 238),
            (ItemId::GloomrockLowWall, 239),
            (ItemId::CobbledGloomrockLowWall, 240),
            (ItemId::PolishedStone, 241),
            (ItemId::PolishedLimestone, 242),
            (ItemId::PolishedSlate, 243),
            (ItemId::PolishedGloomrock, 244),
            (ItemId::ChiseledStone, 245),
            (ItemId::ChiseledLimestone, 246),
            (ItemId::ChiseledSlate, 247),
            (ItemId::ChiseledGloomrock, 248),
            (ItemId::OrangeWool, 249),
            (ItemId::MagentaWool, 250),
            (ItemId::LightBlueWool, 251),
            (ItemId::YellowWool, 252),
            (ItemId::LimeWool, 253),
            (ItemId::PinkWool, 254),
            (ItemId::GrayWool, 255),
            (ItemId::LightGrayWool, 256),
            (ItemId::CyanWool, 257),
            (ItemId::PurpleWool, 258),
            (ItemId::BlueWool, 259),
            (ItemId::BrownWool, 260),
            (ItemId::GreenWool, 261),
            (ItemId::RedWool, 262),
            (ItemId::BlackWool, 263),
            (ItemId::WhiteStainedGlass, 264),
            (ItemId::OrangeStainedGlass, 265),
            (ItemId::MagentaStainedGlass, 266),
            (ItemId::LightBlueStainedGlass, 267),
            (ItemId::YellowStainedGlass, 268),
            (ItemId::LimeStainedGlass, 269),
            (ItemId::PinkStainedGlass, 270),
            (ItemId::GrayStainedGlass, 271),
            (ItemId::LightGrayStainedGlass, 272),
            (ItemId::CyanStainedGlass, 273),
            (ItemId::PurpleStainedGlass, 274),
            (ItemId::BlueStainedGlass, 275),
            (ItemId::BrownStainedGlass, 276),
            (ItemId::GreenStainedGlass, 277),
            (ItemId::RedStainedGlass, 278),
            (ItemId::BlackStainedGlass, 279),
            (ItemId::WhiteStainedGlassPane, 280),
            (ItemId::OrangeStainedGlassPane, 281),
            (ItemId::MagentaStainedGlassPane, 282),
            (ItemId::LightBlueStainedGlassPane, 283),
            (ItemId::YellowStainedGlassPane, 284),
            (ItemId::LimeStainedGlassPane, 285),
            (ItemId::PinkStainedGlassPane, 286),
            (ItemId::GrayStainedGlassPane, 287),
            (ItemId::LightGrayStainedGlassPane, 288),
            (ItemId::CyanStainedGlassPane, 289),
            (ItemId::PurpleStainedGlassPane, 290),
            (ItemId::BlueStainedGlassPane, 291),
            (ItemId::BrownStainedGlassPane, 292),
            (ItemId::GreenStainedGlassPane, 293),
            (ItemId::RedStainedGlassPane, 294),
            (ItemId::BlackStainedGlassPane, 295),
            (ItemId::Terracotta, 296),
            (ItemId::WhiteTerracotta, 297),
            (ItemId::OrangeTerracotta, 298),
            (ItemId::MagentaTerracotta, 299),
            (ItemId::LightBlueTerracotta, 300),
            (ItemId::YellowTerracotta, 301),
            (ItemId::LimeTerracotta, 302),
            (ItemId::PinkTerracotta, 303),
            (ItemId::GrayTerracotta, 304),
            (ItemId::LightGrayTerracotta, 305),
            (ItemId::CyanTerracotta, 306),
            (ItemId::PurpleTerracotta, 307),
            (ItemId::BlueTerracotta, 308),
            (ItemId::BrownTerracotta, 309),
            (ItemId::GreenTerracotta, 310),
            (ItemId::RedTerracotta, 311),
            (ItemId::BlackTerracotta, 312),
            (ItemId::WhiteConcrete, 313),
            (ItemId::OrangeConcrete, 314),
            (ItemId::MagentaConcrete, 315),
            (ItemId::LightBlueConcrete, 316),
            (ItemId::YellowConcrete, 317),
            (ItemId::LimeConcrete, 318),
            (ItemId::PinkConcrete, 319),
            (ItemId::GrayConcrete, 320),
            (ItemId::LightGrayConcrete, 321),
            (ItemId::CyanConcrete, 322),
            (ItemId::PurpleConcrete, 323),
            (ItemId::BlueConcrete, 324),
            (ItemId::BrownConcrete, 325),
            (ItemId::GreenConcrete, 326),
            (ItemId::RedConcrete, 327),
            (ItemId::BlackConcrete, 328),
            (ItemId::WhiteConcretePowder, 329),
            (ItemId::OrangeConcretePowder, 330),
            (ItemId::MagentaConcretePowder, 331),
            (ItemId::LightBlueConcretePowder, 332),
            (ItemId::YellowConcretePowder, 333),
            (ItemId::LimeConcretePowder, 334),
            (ItemId::PinkConcretePowder, 335),
            (ItemId::GrayConcretePowder, 336),
            (ItemId::LightGrayConcretePowder, 337),
            (ItemId::CyanConcretePowder, 338),
            (ItemId::PurpleConcretePowder, 339),
            (ItemId::BlueConcretePowder, 340),
            (ItemId::BrownConcretePowder, 341),
            (ItemId::GreenConcretePowder, 342),
            (ItemId::RedConcretePowder, 343),
            (ItemId::BlackConcretePowder, 344),
            (ItemId::WhiteDye, 345),
            (ItemId::OrangeDye, 346),
            (ItemId::MagentaDye, 347),
            (ItemId::LightBlueDye, 348),
            (ItemId::YellowDye, 349),
            (ItemId::LimeDye, 350),
            (ItemId::PinkDye, 351),
            (ItemId::GrayDye, 352),
            (ItemId::LightGrayDye, 353),
            (ItemId::CyanDye, 354),
            (ItemId::PurpleDye, 355),
            (ItemId::BlueDye, 356),
            (ItemId::BrownDye, 357),
            (ItemId::GreenDye, 358),
            (ItemId::RedDye, 359),
            (ItemId::BlackDye, 360),
            (ItemId::Shield, 361),
            (ItemId::Crossbow, 362),
            (ItemId::Bolt, 363),
            (ItemId::ThrowingAxe, 364),
            (ItemId::Mace, 365),
            (ItemId::MilkBucket, 366),
            (ItemId::Paper, 367),
            (ItemId::EmptyMap, 368),
            (ItemId::FilledMap, 369),
            (ItemId::Compass, 370),
            (ItemId::MapFrame, 371),
            (ItemId::FishingRod, 372),
            (ItemId::RawRoach, 373),
            (ItemId::GrilledRoach, 374),
            (ItemId::RawPerch, 375),
            (ItemId::GrilledPerch, 376),
            (ItemId::RawPike, 377),
            (ItemId::GrilledPike, 378),
            (ItemId::RawEel, 379),
            (ItemId::GrilledEel, 380),
            (ItemId::RawGlowfin, 381),
            (ItemId::GrilledGlowfin, 382),
            (ItemId::Egg, 383),
            (ItemId::FriedEgg, 384),
            (ItemId::Apple, 385),
            (ItemId::AppleSapling, 386),
            (ItemId::AppleLeaves, 387),
            (ItemId::Bowl, 388),
            (ItemId::MushroomSoup, 389),
            (ItemId::Stew, 390),
            (ItemId::BerrySalad, 391),
            (ItemId::FishChowder, 392),
            (ItemId::CakeBatter, 393),
            (ItemId::CocoaCake, 394),
            (ItemId::RawPie, 395),
            (ItemId::ApplePie, 396),
            (ItemId::Compost, 397),
            (ItemId::Kelp, 398),
            (ItemId::Seagrass, 399),
            (ItemId::AzureReef, 400),
            (ItemId::RoseReef, 401),
            (ItemId::AmberReef, 402),
            (ItemId::Moss, 403),
            (ItemId::CaveVine, 404),
            (ItemId::Glowcrystal, 405),
            (ItemId::CrystalCluster, 406),
            (ItemId::Geodestone, 407),
            (ItemId::Basalt, 408),
            (ItemId::PolishedBasalt, 409),
            (ItemId::Magma, 410),
            (ItemId::CrystalShard, 411),
            (ItemId::TreasureMap, 412),
            (ItemId::GlintfinSpawnEgg, 413),
            (ItemId::ShoalbackSpawnEgg, 414),
            (ItemId::MossbackSpawnEgg, 415),
            (ItemId::ShardlingSpawnEgg, 416),
            (ItemId::SlagcrawlerSpawnEgg, 417),
            (ItemId::LeatherHorseArmour, 418),
            (ItemId::IronHorseArmour, 419),
            (ItemId::GoldHorseArmour, 420),
            (ItemId::DiamondHorseArmour, 421),
            (ItemId::Lead, 422),
            (ItemId::DonkeySpawnEgg, 423),
            (ItemId::MuleSpawnEgg, 424),
            (ItemId::WhiteBedroll, 425),
            (ItemId::OrangeBedroll, 426),
            (ItemId::MagentaBedroll, 427),
            (ItemId::LightBlueBedroll, 428),
            (ItemId::YellowBedroll, 429),
            (ItemId::LimeBedroll, 430),
            (ItemId::PinkBedroll, 431),
            (ItemId::GrayBedroll, 432),
            (ItemId::LightGrayBedroll, 433),
            (ItemId::CyanBedroll, 434),
            (ItemId::PurpleBedroll, 435),
            (ItemId::BlueBedroll, 436),
            (ItemId::BrownBedroll, 437),
            (ItemId::GreenBedroll, 438),
            (ItemId::BlackBedroll, 439),
            (ItemId::OakTable, 440),
            (ItemId::SpruceTable, 441),
            (ItemId::JungleTable, 442),
            (ItemId::WillowTable, 443),
            (ItemId::AcaciaTable, 444),
            (ItemId::OakChair, 445),
            (ItemId::SpruceChair, 446),
            (ItemId::JungleChair, 447),
            (ItemId::WillowChair, 448),
            (ItemId::AcaciaChair, 449),
            (ItemId::Book, 450),
            (ItemId::Bookshelf, 451),
            (ItemId::WhiteCarpet, 452),
            (ItemId::OrangeCarpet, 453),
            (ItemId::MagentaCarpet, 454),
            (ItemId::LightBlueCarpet, 455),
            (ItemId::YellowCarpet, 456),
            (ItemId::LimeCarpet, 457),
            (ItemId::PinkCarpet, 458),
            (ItemId::GrayCarpet, 459),
            (ItemId::LightGrayCarpet, 460),
            (ItemId::CyanCarpet, 461),
            (ItemId::PurpleCarpet, 462),
            (ItemId::BlueCarpet, 463),
            (ItemId::BrownCarpet, 464),
            (ItemId::GreenCarpet, 465),
            (ItemId::RedCarpet, 466),
            (ItemId::BlackCarpet, 467),
            (ItemId::FlowerPot, 468),
            (ItemId::ItemFrame, 469),
            (ItemId::Painting, 470),
            (ItemId::WidePainting, 471),
            (ItemId::WhiteBanner, 472),
            (ItemId::OrangeBanner, 473),
            (ItemId::MagentaBanner, 474),
            (ItemId::LightBlueBanner, 475),
            (ItemId::YellowBanner, 476),
            (ItemId::LimeBanner, 477),
            (ItemId::PinkBanner, 478),
            (ItemId::GrayBanner, 479),
            (ItemId::LightGrayBanner, 480),
            (ItemId::CyanBanner, 481),
            (ItemId::PurpleBanner, 482),
            (ItemId::BlueBanner, 483),
            (ItemId::BrownBanner, 484),
            (ItemId::GreenBanner, 485),
            (ItemId::RedBanner, 486),
            (ItemId::BlackBanner, 487),
            (ItemId::WhiteLantern, 488),
            (ItemId::OrangeLantern, 489),
            (ItemId::MagentaLantern, 490),
            (ItemId::LightBlueLantern, 491),
            (ItemId::YellowLantern, 492),
            (ItemId::LimeLantern, 493),
            (ItemId::PinkLantern, 494),
            (ItemId::GrayLantern, 495),
            (ItemId::LightGrayLantern, 496),
            (ItemId::CyanLantern, 497),
            (ItemId::PurpleLantern, 498),
            (ItemId::BlueLantern, 499),
            (ItemId::BrownLantern, 500),
            (ItemId::GreenLantern, 501),
            (ItemId::RedLantern, 502),
            (ItemId::BlackLantern, 503),
            (ItemId::Alembic, 504),
            (ItemId::GlassVial, 505),
            (ItemId::WaterVial, 506),
            (ItemId::NeutralBase, 507),
            (ItemId::HealingPotion, 508),
            (ItemId::NightVisionPotion, 509),
            (ItemId::WaterBreathingPotion, 510),
            (ItemId::FireResistancePotion, 511),
            (ItemId::SwiftnessPotion, 512),
            (ItemId::StrengthPotion, 513),
            (ItemId::RegenerationPotion, 514),
            (ItemId::SplashPoisonPotion, 515),
            (ItemId::SplashSlownessPotion, 516),
            (ItemId::SplashWeaknessPotion, 517),
            (ItemId::GnawerFang, 518),
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
                | ItemId::DonkeySpawnEgg
                | ItemId::MuleSpawnEgg
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
                | ItemId::OakSlab
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
                | ItemId::OakStairs
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
                | ItemId::EmberHeart
                | ItemId::EngravingTable
                | ItemId::Gloomrock
                | ItemId::CobbledGloomrock
                | ItemId::Anvil
                | ItemId::Saddle
                | ItemId::LeatherHorseArmour
                | ItemId::IronHorseArmour
                | ItemId::GoldHorseArmour
                | ItemId::DiamondHorseArmour
                | ItemId::Lead
                | ItemId::Boat
                | crate::new_palette_block_items!()
                | crate::dye_items!()
                | ItemId::Shield
                | ItemId::Crossbow
                | ItemId::Bolt
                | ItemId::ThrowingAxe
                | ItemId::Mace
                | ItemId::MilkBucket
                | ItemId::Paper
                | ItemId::EmptyMap
                | ItemId::FilledMap
                | ItemId::Compass
                | ItemId::MapFrame
                | ItemId::FishingRod
                | ItemId::RawRoach
                | ItemId::GrilledRoach
                | ItemId::RawPerch
                | ItemId::GrilledPerch
                | ItemId::RawPike
                | ItemId::GrilledPike
                | ItemId::RawEel
                | ItemId::GrilledEel
                | ItemId::RawGlowfin
                | ItemId::GrilledGlowfin
                | ItemId::Egg
                | ItemId::FriedEgg
                | ItemId::Apple
                | ItemId::AppleSapling
                | ItemId::AppleLeaves
                | ItemId::Bowl
                | ItemId::MushroomSoup
                | ItemId::Stew
                | ItemId::BerrySalad
                | ItemId::FishChowder
                | ItemId::CakeBatter
                | ItemId::CocoaCake
                | ItemId::RawPie
                | ItemId::ApplePie
                | ItemId::Compost
                | ItemId::Kelp
                | ItemId::Seagrass
                | ItemId::AzureReef
                | ItemId::RoseReef
                | ItemId::AmberReef
                | ItemId::Moss
                | ItemId::CaveVine
                | ItemId::Glowcrystal
                | ItemId::CrystalCluster
                | ItemId::Geodestone
                | ItemId::Basalt
                | ItemId::PolishedBasalt
                | ItemId::Magma
                | ItemId::CrystalShard
                | ItemId::TreasureMap
                | ItemId::GlintfinSpawnEgg
                | ItemId::ShoalbackSpawnEgg
                | ItemId::MossbackSpawnEgg
                | ItemId::ShardlingSpawnEgg
                | ItemId::SlagcrawlerSpawnEgg
                | crate::new_decor_items!()
                | ItemId::Alembic
                | ItemId::GlassVial
                | ItemId::WaterVial
                | ItemId::NeutralBase
                | ItemId::HealingPotion
                | ItemId::NightVisionPotion
                | ItemId::WaterBreathingPotion
                | ItemId::FireResistancePotion
                | ItemId::SwiftnessPotion
                | ItemId::StrengthPotion
                | ItemId::RegenerationPotion
                | ItemId::SplashPoisonPotion
                | ItemId::SplashSlownessPotion
                | ItemId::SplashWeaknessPotion
                | ItemId::GnawerFang => {}
            }
        }
        for (id, expected) in table {
            assert_eq!(
                bincode::options().serialize(&id).unwrap(),
                varint(expected),
                "{id:?}: the encoded value of an existing variant changed, which breaks the \
                protocol. New variants go at the END of the enum; add them to this table and \
                bump PROTOCOL_ID."
            );
        }
    }

    #[test]
    fn all_lists_every_item_in_order() {
        for (index, item) in ItemId::ALL.iter().enumerate() {
            assert_eq!(
                bincode::options().serialize(item).unwrap(),
                varint(index as u16)
            );
        }
        // the variant after the last one listed does not exist
        let past = varint(ItemId::ALL.len() as u16);
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
        let enchantments = Enchantments::default();
        assert_eq!(
            encoded(ItemType::Enchanted {
                wear: 5,
                enchantments
            }),
            [3, 5, 0]
        );
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
            ItemId::Saddle,
            ItemId::LeatherHorseArmour,
            ItemId::IronHorseArmour,
            ItemId::GoldHorseArmour,
            ItemId::DiamondHorseArmour,
            ItemId::Boat,
            ItemId::Shield,
            ItemId::Crossbow,
            ItemId::Bolt,
            ItemId::ThrowingAxe,
            ItemId::Mace,
            ItemId::MilkBucket,
            ItemId::FilledMap,
            ItemId::TreasureMap,
            ItemId::Compass,
            ItemId::FishingRod,
            ItemId::MushroomSoup,
            ItemId::Stew,
            ItemId::BerrySalad,
            ItemId::FishChowder,
            ItemId::Egg,
        ];
        // The vials stack to 16, filled to one
        let vials: Vec<ItemId> = ItemId::ALL
            .into_iter()
            .filter(|item| item.is_vial())
            .collect();
        for vial in &vials {
            let max = if vial.is_vial_filled() { 1 } else { 16 };
            assert_eq!(vial.get_max_stack(), max, "{vial:?}");
        }
        let buckets = [buckets.as_slice(), &vials].concat();
        let others = ItemId::ALL
            .iter()
            .filter(|item| !tools.iter().any(|(t, ..)| t == *item) && !buckets.contains(item))
            .filter(|item| **item != ItemId::PortalGun && item.armour().is_none())
            // a banner may hold patterns of its own
            .filter(|item| item.banner_color().is_none());
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
                    | ItemId::Saddle
                    | ItemId::LeatherHorseArmour
                    | ItemId::IronHorseArmour
                    | ItemId::GoldHorseArmour
                    | ItemId::DiamondHorseArmour
                    | ItemId::Lead
                    | ItemId::Boat
                    | ItemId::Shield
                    | ItemId::Crossbow
                    | ItemId::Bolt
                    | ItemId::ThrowingAxe
                    | ItemId::Mace
                    | ItemId::MilkBucket
                    | ItemId::EmptyMap
                    | ItemId::FilledMap
                    | ItemId::TreasureMap
                    | ItemId::Compass
                    | ItemId::FishingRod
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
