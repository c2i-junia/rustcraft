use std::fmt;

use serde::de::{self, EnumAccess, VariantAccess, Visitor};
use serde::{ser, Deserialize, Deserializer, Serialize, Serializer};

use super::{BlockId, Enchantments, GameElementId, ItemName, MobKind};
use crate::content::content;
use bevy::math::IVec3;

/// A kind of item: its number in the content of the process (`crate::content::Content`), which
/// the stacks hold and the network carries. The items the engine names have a constant each,
/// spelled as the variants of the enum it once was (`ItemId::Stick`), and numbered at compile
/// time by `engine_items!`; the items that only the content files define, as the item of a
/// block only they define, are numbered after them as the files are read
/// (`docs/design/content-registry.md`, 1.3 and 3.1).
///
/// The number of an item is the process's own: a world saves its items by name
/// (`LEGACY_ITEM_NAMES` for the worlds saved before), and a client takes the numbers of its
/// server.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct ItemId(u16);

/// Declares the items the engine names: a constant of `ItemId` each, numbered in the order
/// given from 0, and `ItemId::ENGINE`, every one of them in that order.
macro_rules! engine_items {
    ($($(#[$meta:meta])* $name:ident,)*) => {
        /// Numbers the constants: the position of each in the list.
        #[allow(dead_code)]
        #[repr(u16)]
        enum Engine {
            $($name,)*
        }

        #[allow(non_upper_case_globals)]
        impl ItemId {
            $(
                $(#[$meta])*
                pub const $name: ItemId = ItemId(Engine::$name as u16);
            )*

            /// The Rust names of the engine items, by number.
            const ENGINE_IDENTS: &[&str] = &[$(stringify!($name),)*];

            /// Every item the engine names, by number.
            pub const ENGINE: [ItemId; Self::ENGINE_IDENTS.len()] = [$(ItemId::$name,)*];
        }
    };
}

engine_items! {
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
GnawerFang,}

/// The Rust names the former variants of `ItemId` were also read by, in the RON files.
const FORMER_IDENTS: [(&str, ItemId); 2] = [
    ("WoodenSlab", ItemId::OakSlab),
    ("WoodenStairs", ItemId::OakStairs),
];

impl ItemId {
    /// The number of the item, where the table of the content holds its properties.
    #[inline]
    pub const fn index(self) -> usize {
        self.0 as usize
    }

    /// The item numbered `index`, which the content may not define.
    #[inline]
    pub const fn from_index(index: u16) -> Self {
        Self(index)
    }

    /// The Rust name of the item, as the constant of an engine item spells it: `OakLog`.
    /// `None` for a number the content of the process does not define.
    fn ident(self) -> Option<&'static str> {
        Self::ENGINE_IDENTS
            .get(self.index())
            .copied()
            .or_else(|| content().item_idents.get(self.index()).map(String::as_str))
    }

    /// The item of the Rust name `ident`, in the content of the process.
    fn of_ident(ident: &str) -> Option<Self> {
        let place = |index: usize| Self(index as u16);
        Self::ENGINE_IDENTS
            .iter()
            .position(|known| *known == ident)
            .map(place)
            .or_else(|| {
                FORMER_IDENTS
                    .iter()
                    .find(|(former, _)| *former == ident)
                    .map(|(_, id)| *id)
            })
            .or_else(|| {
                content()
                    .item_idents
                    .iter()
                    .position(|known| known == ident)
                    .map(place)
            })
    }

    /// The name of the item in the commands, like in Minecraft: `oak_log`.
    pub fn name(&self) -> String {
        match self.content_name() {
            Some(name) => name.to_string(),
            None => crate::utils::snake_case(&format!("{self:?}")),
        }
    }

    /// The item called `name`: `oak_log` like in Minecraft, in any case, or `OakLog` like the
    /// variant.
    pub fn from_name(name: &str) -> Option<ItemId> {
        content()
            .item_named(&name.to_lowercase())
            .or_else(|| Self::of_ident(name))
            .or_else(|| {
                // a former name: `wooden_slab`
                let mut pascal = String::new();
                for word in name.split('_') {
                    let mut chars = word.chars();
                    pascal.extend(chars.next()?.to_uppercase());
                    pascal.push_str(&chars.as_str().to_lowercase());
                }
                Self::of_ident(&pascal)
            })
    }

    #[inline]
    pub fn get_max_stack(&self) -> u32 {
        self.props().max_stack
    }

    /// The points of health a blow with the item takes from a mob.
    #[inline]
    pub fn attack_damage(&self) -> u32 {
        self.props().attack_damage
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
    #[inline]
    pub fn food_value(&self) -> Option<(u8, f32)> {
        self.props().food_value
    }

    /// The status effect eating the item gives, if any: its kind, its level and its ticks. The
    /// healing foods may be eaten with all the food a player can have.
    #[inline]
    pub fn food_effect(&self) -> Option<(crate::players::effects::EffectKind, u8, u32)> {
        self.props().food_effect
    }

    /// What is left in the hand once the item is eaten or drunk: the bowl of a soup.
    #[inline]
    pub fn eaten_remains(&self) -> Option<ItemId> {
        self.props().eaten_remains
    }

    /// What is left in its cell of the crafting grid once the item is used by a craft: the
    /// bucket of a bucket of milk.
    #[inline]
    pub fn crafting_remains(&self) -> Option<ItemId> {
        self.props().crafting_remains
    }

    /// The points of protection of a horse armour (`mobs::armoured_damage`): Minecraft's.
    #[inline]
    pub fn horse_armour(&self) -> Option<u32> {
        self.props().horse_armour
    }

    /// The mob a spawn egg makes appear, used on a block.
    #[inline]
    pub fn spawned_mob(&self) -> Option<MobKind> {
        self.props().spawned_mob
    }

    /// What the item does as a tool, if it is one.
    #[inline]
    pub fn tool(&self) -> Option<Tool> {
        self.props().tool
    }

    /// The wear at which the item breaks, for the items that wear out: the tools, the
    /// armour, which wears by the hits it absorbs, the flint and steel, which wears by the fires
    /// it lights, and the bow, by its shots.
    #[inline]
    pub fn durability(&self) -> Option<u32> {
        self.props().durability
    }

    /// The category the item browser files the item under.
    #[inline]
    pub fn category(self) -> ItemCategory {
        self.props().category
    }

    #[inline]
    pub fn get_default_type(&self) -> ItemType {
        self.props().default_type
    }
}

impl GameElementId for ItemId {}

/// The Rust name of the item, which also names its texture: `OakLog`.
impl fmt::Debug for ItemId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.ident() {
            Some(ident) => f.write_str(ident),
            None => write!(f, "ItemId({})", self.0),
        }
    }
}

/// An item is its number in bincode, as in the chest files and on the network, and its Rust
/// name in a human-readable format such as RON, where it reads as the variant it once was:
/// `Stick`.
impl Serialize for ItemId {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        if !serializer.is_human_readable() {
            return serializer.serialize_u16(self.0);
        }
        let ident = self
            .ident()
            .ok_or_else(|| ser::Error::custom(format!("{self:?} has no name")))?;
        serializer.serialize_unit_variant("ItemId", u32::from(self.0), ident)
    }
}

impl<'de> Deserialize<'de> for ItemId {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        if !deserializer.is_human_readable() {
            return u16::deserialize(deserializer).map(Self);
        }

        struct ItemVisitor;

        impl<'de> Visitor<'de> for ItemVisitor {
            type Value = ItemId;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("the name of an item")
            }

            fn visit_enum<A: EnumAccess<'de>>(self, data: A) -> Result<ItemId, A::Error> {
                let (super::blocks::Ident(ident), variant) = data.variant()?;
                variant.unit_variant()?;
                ItemId::of_ident(&ident)
                    .ok_or_else(|| de::Error::custom(format!("no item is named {ident}")))
            }
        }

        deserializer.deserialize_enum("ItemId", ItemId::ENGINE_IDENTS, ItemVisitor)
    }
}

/// The categories of the item browser, which files every item under one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ToolKind {
    Pickaxe,
    Axe,
    Shovel,
    Hoe,
    Sword,
}

impl ToolKind {
    /// Every kind of tool.
    pub const ALL: [ToolKind; 5] = [
        Self::Pickaxe,
        Self::Axe,
        Self::Shovel,
        Self::Hoe,
        Self::Sword,
    ];
}

/// What a tool item does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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

    /// `ToolKind::ALL` is written by hand: it must hold every kind, each at the place of its
    /// number (`content::tool_bit` shifts by it).
    #[test]
    fn tool_kind_all_holds_every_kind_in_order() {
        // A new kind does not compile here until it is counted
        let kinds = |kind: ToolKind| match kind {
            ToolKind::Pickaxe
            | ToolKind::Axe
            | ToolKind::Shovel
            | ToolKind::Hoe
            | ToolKind::Sword => 5,
        };
        assert_eq!(ToolKind::ALL.len(), kinds(ToolKind::Pickaxe));
        for (place, kind) in ToolKind::ALL.into_iter().enumerate() {
            assert_eq!(kind as usize, place, "{kind:?}");
        }
    }

    /// The bytes of `value` as bincode's variable length encoding writes it: one byte below 251,
    /// else the marker 251 and two bytes.
    fn varint(value: u16) -> Vec<u8> {
        match u8::try_from(value) {
            Ok(byte) if byte < 251 => vec![byte],
            _ => [vec![251], value.to_le_bytes().to_vec()].concat(),
        }
    }

    #[test]
    fn an_item_is_encoded_as_its_number() {
        for item in ItemId::ENGINE {
            assert_eq!(
                bincode::options().serialize(&item).unwrap(),
                varint(item.index() as u16),
                "{item:?}"
            );
        }
        assert_eq!(bincode::options().serialize(&ItemId::Stone).unwrap(), [2]);
        assert_eq!(
            bincode::options().serialize(&ItemId::GnawerFang).unwrap(),
            [251, 6, 2]
        );
    }

    #[test]
    fn an_item_is_named_like_in_minecraft_and_found_by_its_name() {
        assert_eq!(ItemId::OakLog.name(), "oak_log");
        assert_eq!(ItemId::Dirt.name(), "dirt");
        for item in ItemId::ENGINE {
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
            let eggs = ItemId::ENGINE
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
        let vials: Vec<ItemId> = ItemId::ENGINE
            .into_iter()
            .filter(|item| item.is_vial())
            .collect();
        for vial in &vials {
            let max = if vial.is_vial_filled() { 1 } else { 16 };
            assert_eq!(vial.get_max_stack(), max, "{vial:?}");
        }
        let buckets = [buckets.as_slice(), &vials].concat();
        let others = ItemId::ENGINE
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
        for item in ItemId::ENGINE {
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
        for item in ItemId::ENGINE {
            assert!(ItemCategory::ALL.contains(&item.category()), "{item:?}");
        }
        for category in ItemCategory::ALL {
            assert!(
                ItemId::ENGINE
                    .iter()
                    .any(|item| item.category() == category),
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
        for item in ItemId::ENGINE {
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
        assert_eq!(ItemId::ENGINE[145], ItemId::PortalGun);
    }
}
