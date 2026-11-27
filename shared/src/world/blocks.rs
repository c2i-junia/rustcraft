use std::collections::HashMap;

use super::{palette::Form, GameElementId, ItemId, Tool, ToolKind};
use crate::content::{Behaviour, BlockFlags};
use bevy::math::{bounding::Aabb3d, IVec3, Vec3A};
use rand::Rng;
use serde::{Deserialize, Serialize};

/// Append only: never reorder, insert before the end or remove a variant. The variant's position
/// is what chunk files and network messages store (bincode), so any other change turns the
/// blocks of every existing world into other blocks. Bump `PROTOCOL_ID` and, if the saved
/// encoding changes, `FORMAT_VERSION`. The test `block_encoding_is_pinned` guards this.
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
    CraftingTable,
    SprucePlanks,
    Furnace,
    Chest,
    CoalOre,
    IronOre,
    GoldOre,
    Gravel,
    Torch,
    JungleLog,
    JunglePlanks,
    JungleLeaves,
    Fern,
    Vine,
    Cocoa,
    /// A furnace while it burns: it glows, and turns back into a `Furnace` once out.
    LitFurnace,
    DiamondOre,
    IronBlock,
    GoldBlock,
    DiamondBlock,
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
    #[serde(alias = "WoodenStairs")]
    OakStairs,
    HewnStoneStairs,
    BrickStairs,
    Door,
    DoorTop,
    OpenDoor,
    OpenDoorTop,
    Ladder,
    Fence,
    GlassPane,
    HangingLantern,
    OakSapling,
    SpruceSapling,
    JungleSapling,
    WillowSapling,
    AcaciaSapling,
    Wool,
    BerryBush,
    RipeBerryBush,
    TilledSoil,
    RyeSprout,
    YoungRye,
    RipeRye,
    Bedroll,
    Barrel,
    RainCistern,
    FullRainCistern,
    Campfire,
    UnlitCampfire,
    Sign,
    /// Its age, from 0 to `fire::MAX_FIRE_AGE`, is its `BlockData::level`.
    Fire,
    /// A liquid like water, that flows slower and shorter (see `liquid::LAVA`), glows, and
    /// burns whoever swims in it.
    Lava,
    /// What a lava source becomes where water meets it.
    Obsidian,
    /// The stages of carrots, from planted to ripe, as rye's.
    CarrotSprout,
    YoungCarrots,
    TallCarrots,
    RipeCarrots,
    /// The stages of potatoes, from planted to ripe.
    PotatoSprout,
    YoungPotatoes,
    TallPotatoes,
    RipePotatoes,
    /// The trophy of the Ember Mother: it glows.
    EmberHeart,
    /// Enchants, for levels of experience: see `players::enchanting`.
    EngravingTable,
    /// The rock under height 0 of the worlds with deep undergrounds: darker and harder than
    /// stone.
    Gloomrock,
    /// What gloomrock breaks into, as stone into cobblestone.
    CobbledGloomrock,
    /// Repairs, merges and renames what wears, for levels of experience: see `players::anvil`.
    Anvil,
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
    SpruceDoorTop,
    OpenSpruceDoor,
    OpenSpruceDoorTop,
    JungleDoor,
    JungleDoorTop,
    OpenJungleDoor,
    OpenJungleDoorTop,
    WillowDoor,
    WillowDoorTop,
    OpenWillowDoor,
    OpenWillowDoorTop,
    AcaciaDoor,
    AcaciaDoorTop,
    OpenAcaciaDoor,
    OpenAcaciaDoorTop,
    OakTrapdoor,
    OpenOakTrapdoor,
    SpruceTrapdoor,
    OpenSpruceTrapdoor,
    JungleTrapdoor,
    OpenJungleTrapdoor,
    WillowTrapdoor,
    OpenWillowTrapdoor,
    AcaciaTrapdoor,
    OpenAcaciaTrapdoor,
    OakFenceGate,
    OpenOakFenceGate,
    SpruceFenceGate,
    OpenSpruceFenceGate,
    JungleFenceGate,
    OpenJungleFenceGate,
    WillowFenceGate,
    OpenWillowFenceGate,
    AcaciaFenceGate,
    OpenAcaciaFenceGate,
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
    /// A board of planks that shows the paper map put on it (`cartography`).
    MapFrame,
    /// The leaves of an apple tree, which drop apples now and then (`server::world::ticks`).
    AppleLeaves,
    AppleSapling,
    // The oceans and the deep caves (`is_submerged`: the plants that grow under water)
    /// A long weed of the sea floor, grown in columns, which sways under the water.
    Kelp,
    /// A tuft of grass of the sea floor.
    Seagrass,
    /// The blocks of the reefs, of three colours.
    AzureReef,
    RoseReef,
    AmberReef,
    /// The soft green floor of the damp caves.
    Moss,
    /// A vine that hangs from the roof of the damp caves.
    CaveVine,
    /// A block of crystal that glows, from the crystal caverns: it breaks into shards.
    Glowcrystal,
    /// A cluster of crystal points that grows on a floor or a roof of the crystal caverns, and
    /// glows a little.
    CrystalCluster,
    /// The pale stone shell around the crystal caverns.
    Geodestone,
    /// The dark rock of the galleries near the lava.
    Basalt,
    PolishedBasalt,
    /// A crust of cooling lava: it glows, and burns who stands on it without sneaking.
    Magma,
    // The furniture and the decoration of a home: see `decor`
    /// The bedrolls of the other colours: the first one, `Bedroll`, is red.
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
    /// A table and a chair of each wood: a player sits on a chair (`players::sitting`).
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
    /// Shelves of books, which the engraving table may one day count.
    Bookshelf,
    /// A thin rug of wool of each colour.
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
    /// Shows the plant put in it (`decor::Displays`).
    FlowerPot,
    /// Hung on a wall, it shows the item put in it, turned at each click.
    ItemFrame,
    /// The canvases of one block, hung on a wall: a click shows the next one.
    DawnPainting,
    MeadowPainting,
    HarbourPainting,
    StillLifePainting,
    /// The canvases of two blocks, their left half and their right half.
    ValleyPainting,
    ValleyPaintingRight,
    SeascapePainting,
    SeascapePaintingRight,
    ForestPainting,
    ForestPaintingRight,
    /// A banner of each colour hung on a wall, and the patterns dyed on it (`decor::BannerPatterns`).
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
    /// A lantern of each colour, standing and hung.
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
    HangingWhiteLantern,
    HangingOrangeLantern,
    HangingMagentaLantern,
    HangingLightBlueLantern,
    HangingYellowLantern,
    HangingLimeLantern,
    HangingPinkLantern,
    HangingGrayLantern,
    HangingLightGrayLantern,
    HangingCyanLantern,
    HangingPurpleLantern,
    HangingBlueLantern,
    HangingBrownLantern,
    HangingGreenLantern,
    HangingRedLantern,
    HangingBlackLantern,
    /// Distils the potions: it holds a `Furnace`, which it ticks by `alchemy::distilled`.
    Alembic,
    /// A torch held on the side of a block: its direction names the side of its cell the block
    /// holding it lies on, as a vine's. `Torch` is the one standing on the block below.
    WallTorch,
}

/// Append only, for the same reason as `BlockId`: the position of the variant is what is stored
/// in chunk files and sent on the network.
///
/// A block drawn turned is turned from its front by the direction: `Right` turns its front
/// toward +X. The direction of a vine or a cocoa pod names the side of its cell the block
/// holding it lies on.
#[derive(Debug, Copy, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum BlockDirection {
    Front,
    Right,
    Back,
    Left,
}

impl BlockDirection {
    /// The step toward the side of a cell the direction names: the front is north, −Z, the
    /// back +Z, the left −X and the right +X.
    pub fn offset(self) -> IVec3 {
        match self {
            Self::Front => IVec3::NEG_Z,
            Self::Right => IVec3::X,
            Self::Back => IVec3::Z,
            Self::Left => IVec3::NEG_X,
        }
    }

    /// The horizontal vector `(x, z)` turned from the front to this direction, by quarter turns
    /// about the vertical axis: `Right` turns the front, -Z, toward +X.
    pub fn turn(self, [x, z]: [f32; 2]) -> [f32; 2] {
        match self {
            Self::Front => [x, z],
            Self::Right => [-z, x],
            Self::Back => [-x, -z],
            Self::Left => [z, -x],
        }
    }

    /// The direction whose `offset` is `step`, `None` for anything but a horizontal step to a
    /// neighbouring cell.
    pub fn toward(step: IVec3) -> Option<Self> {
        [Self::Front, Self::Right, Self::Back, Self::Left]
            .into_iter()
            .find(|direction| direction.offset() == step)
    }
}

/// How far a vine's plate lies from the face of the block that holds it, inside its own cell.
pub const VINE_GAP: f32 = 1.0 / 16.0;

/// The lower half of its cell.
pub const SLAB: Aabb3d = Aabb3d {
    min: Vec3A::ZERO,
    max: Vec3A::new(1.0, 0.5, 1.0),
};

/// The box of a lantern standing in the middle of its cell.
pub const LANTERN: Aabb3d = Aabb3d {
    min: Vec3A::new(5.0 / 16.0, 0.0, 5.0 / 16.0),
    max: Vec3A::new(11.0 / 16.0, 10.0 / 16.0, 11.0 / 16.0),
};

/// The back half of a stair, above its lower half.
pub const STAIR_BACK: Aabb3d = Aabb3d {
    min: Vec3A::new(0.0, 0.5, 0.5),
    max: Vec3A::ONE,
};
/// A shut door lies on the front of its cell, an open one on its left.
pub const DOOR: Aabb3d = Aabb3d {
    min: Vec3A::ZERO,
    max: Vec3A::new(1.0, 1.0, 3.0 / 16.0),
};
pub const OPEN_DOOR: Aabb3d = Aabb3d {
    min: Vec3A::ZERO,
    max: Vec3A::new(3.0 / 16.0, 1.0, 1.0),
};
/// Higher than its cell: see `ChunkMap::check_collision_box`.
pub const FENCE: Aabb3d = Aabb3d {
    min: Vec3A::ZERO,
    max: Vec3A::new(1.0, 1.5, 1.0),
};
/// A shut trapdoor lies on the floor of its cell; an open one stands on its front, as a shut door.
pub const TRAPDOOR: Aabb3d = Aabb3d {
    min: Vec3A::ZERO,
    max: Vec3A::new(1.0, 3.0 / 16.0, 1.0),
};
/// A shut fence gate, across the middle of its cell: as high as a fence, see `FENCE`.
pub const FENCE_GATE: Aabb3d = Aabb3d {
    min: Vec3A::new(0.0, 0.0, 6.0 / 16.0),
    max: Vec3A::new(1.0, 1.5, 10.0 / 16.0),
};
pub const LADDER: Aabb3d = Aabb3d {
    min: Vec3A::ZERO,
    max: Vec3A::new(1.0, 1.0, 2.0 / 16.0),
};
pub const SIGN: Aabb3d = Aabb3d {
    min: Vec3A::new(1.0 / 16.0, 0.5, 7.0 / 16.0),
    max: Vec3A::new(15.0 / 16.0, 15.0 / 16.0, 9.0 / 16.0),
};
/// A lantern hung under a block.
pub const HANGING_LANTERN: Aabb3d = Aabb3d {
    min: Vec3A::new(5.0 / 16.0, 6.0 / 16.0, 5.0 / 16.0),
    max: Vec3A::new(11.0 / 16.0, 1.0, 11.0 / 16.0),
};
pub const BEDROLL: Aabb3d = Aabb3d {
    min: Vec3A::new(1.0 / 16.0, 0.0, 0.0),
    max: Vec3A::new(15.0 / 16.0, 3.0 / 16.0, 1.0),
};
pub const CAMPFIRE: Aabb3d = Aabb3d {
    min: Vec3A::new(1.0 / 16.0, 0.0, 1.0 / 16.0),
    max: Vec3A::new(15.0 / 16.0, 5.0 / 16.0, 15.0 / 16.0),
};

/// Data associated with a given `BlockId`
#[derive(Debug, Copy, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BlockData {
    pub id: BlockId,
    pub direction: BlockDirection,
    pub breaking_progress: u8,
    /// For a liquid, how far it is from its source (see `liquid`); 0 for any other block.
    /// The worlds saved in RON before it existed read it as 0.
    #[serde(default)]
    pub level: u8,
}

/// Last breaking stage: there are ten crack textures, `DestroyStage0` to `DestroyStage9`.
pub const MAX_BREAKING_LEVEL: u8 = 9;

impl BlockData {
    pub fn new(id: BlockId, direction: BlockDirection) -> Self {
        BlockData {
            id,
            direction,
            breaking_progress: 0,
            level: 0,
        }
    }

    /// The cell of the block that holds this one up, which it falls with: a torch stands on the
    /// block below, a wall torch is held on the side its direction names. `None` for the others.
    pub fn support(&self, at: IVec3) -> Option<IVec3> {
        match self.id {
            BlockId::Torch => Some(at - IVec3::Y),
            BlockId::WallTorch => Some(at + self.direction.offset()),
            _ => None,
        }
    }

    /// The box rays hit, as `BlockId::get_ray_hitbox`, but turned to the direction of the
    /// block, as it is drawn: for a vine, a slab around its plate, on the side of the block
    /// holding it.
    pub fn get_ray_hitbox(&self) -> BlockHitbox {
        self.turned(self.id.get_ray_hitbox())
    }

    /// The collision box, as `BlockId::get_hitbox`, but turned to the direction of the block:
    /// for a cocoa pod, its box against the block holding it.
    pub fn get_hitbox(&self) -> BlockHitbox {
        self.turned(self.id.get_hitbox())
    }

    /// The second collision box of a block made of two, such as a stair.
    pub fn get_second_hitbox(&self) -> Option<Aabb3d> {
        if !self.id.is_stairs() {
            return None;
        }
        match self.turned(BlockHitbox::Aabb(STAIR_BACK)) {
            BlockHitbox::Aabb(hitbox) => Some(hitbox),
            _ => None,
        }
    }

    /// `hitbox`, the box of a block turned to the front, turned to the direction of this one
    /// about the centre of the cell. Nothing changes for a whole block or a centred box.
    fn turned(&self, hitbox: BlockHitbox) -> BlockHitbox {
        let BlockHitbox::Aabb(front) = hitbox else {
            return hitbox;
        };
        let turn = |corner: Vec3A| {
            let [x, z] = self.direction.turn([corner.x - 0.5, corner.z - 0.5]);
            Vec3A::new(x + 0.5, corner.y, z + 0.5)
        };
        let (a, b) = (turn(front.min), turn(front.max));
        BlockHitbox::Aabb(Aabb3d {
            min: a.min(b),
            max: a.max(b),
        })
    }

    /// Breaking stage from 0 to `MAX_BREAKING_LEVEL` (9, the last crack texture); always 0 for
    /// an unbreakable block. A progress at or beyond the break time, which a chunk file or a
    /// server can carry, gives the last stage.
    pub fn get_breaking_level(&self) -> u8 {
        match self.id.get_break_time() {
            Some(break_time) => (((self.breaking_progress as u16 * 10) / break_time as u16) as u8)
                .min(MAX_BREAKING_LEVEL),
            None => 0,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum BlockTransparency {
    Transparent,
    Liquid,
    Solid,
    Decoration,
}

/// The tool a block must be broken with to drop its items, see `BlockId::get_required_tool`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RequiredTool {
    pub kind: ToolKind,
    /// The lowest `ToolTier::mining_level` that harvests the block.
    pub level: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum BlockHitbox {
    FullBlock,
    Aabb(Aabb3d),
    None,
}

impl BlockId {
    /// Every block, in the order of their encoding.
    pub const ALL: [BlockId; 400] = [
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
        BlockId::CraftingTable,
        BlockId::SprucePlanks,
        BlockId::Furnace,
        BlockId::Chest,
        BlockId::CoalOre,
        BlockId::IronOre,
        BlockId::GoldOre,
        BlockId::Gravel,
        BlockId::Torch,
        BlockId::JungleLog,
        BlockId::JunglePlanks,
        BlockId::JungleLeaves,
        BlockId::Fern,
        BlockId::Vine,
        BlockId::Cocoa,
        BlockId::LitFurnace,
        BlockId::DiamondOre,
        BlockId::IronBlock,
        BlockId::GoldBlock,
        BlockId::DiamondBlock,
        BlockId::HewnStone,
        BlockId::Limestone,
        BlockId::Slate,
        BlockId::Clay,
        BlockId::BrickWall,
        BlockId::RammedEarth,
        BlockId::Thatch,
        BlockId::CoalBlock,
        BlockId::CopperOre,
        BlockId::TinOre,
        BlockId::CopperBlock,
        BlockId::TinBlock,
        BlockId::BronzeBlock,
        BlockId::WillowLog,
        BlockId::WillowPlanks,
        BlockId::WillowLeaves,
        BlockId::AcaciaLog,
        BlockId::AcaciaPlanks,
        BlockId::AcaciaLeaves,
        BlockId::Reed,
        BlockId::Heather,
        BlockId::Mushroom,
        BlockId::OakSlab,
        BlockId::HewnStoneSlab,
        BlockId::BrickSlab,
        BlockId::Lantern,
        BlockId::OakStairs,
        BlockId::HewnStoneStairs,
        BlockId::BrickStairs,
        BlockId::Door,
        BlockId::DoorTop,
        BlockId::OpenDoor,
        BlockId::OpenDoorTop,
        BlockId::Ladder,
        BlockId::Fence,
        BlockId::GlassPane,
        BlockId::HangingLantern,
        BlockId::OakSapling,
        BlockId::SpruceSapling,
        BlockId::JungleSapling,
        BlockId::WillowSapling,
        BlockId::AcaciaSapling,
        BlockId::Wool,
        BlockId::BerryBush,
        BlockId::RipeBerryBush,
        BlockId::TilledSoil,
        BlockId::RyeSprout,
        BlockId::YoungRye,
        BlockId::RipeRye,
        BlockId::Bedroll,
        BlockId::Barrel,
        BlockId::RainCistern,
        BlockId::FullRainCistern,
        BlockId::Campfire,
        BlockId::UnlitCampfire,
        BlockId::Sign,
        BlockId::Fire,
        BlockId::Lava,
        BlockId::Obsidian,
        BlockId::CarrotSprout,
        BlockId::YoungCarrots,
        BlockId::TallCarrots,
        BlockId::RipeCarrots,
        BlockId::PotatoSprout,
        BlockId::YoungPotatoes,
        BlockId::TallPotatoes,
        BlockId::RipePotatoes,
        BlockId::EmberHeart,
        BlockId::EngravingTable,
        BlockId::Gloomrock,
        BlockId::CobbledGloomrock,
        BlockId::Anvil,
        BlockId::SpruceSlab,
        BlockId::JungleSlab,
        BlockId::WillowSlab,
        BlockId::AcaciaSlab,
        BlockId::SpruceStairs,
        BlockId::JungleStairs,
        BlockId::WillowStairs,
        BlockId::AcaciaStairs,
        BlockId::SpruceDoor,
        BlockId::SpruceDoorTop,
        BlockId::OpenSpruceDoor,
        BlockId::OpenSpruceDoorTop,
        BlockId::JungleDoor,
        BlockId::JungleDoorTop,
        BlockId::OpenJungleDoor,
        BlockId::OpenJungleDoorTop,
        BlockId::WillowDoor,
        BlockId::WillowDoorTop,
        BlockId::OpenWillowDoor,
        BlockId::OpenWillowDoorTop,
        BlockId::AcaciaDoor,
        BlockId::AcaciaDoorTop,
        BlockId::OpenAcaciaDoor,
        BlockId::OpenAcaciaDoorTop,
        BlockId::OakTrapdoor,
        BlockId::OpenOakTrapdoor,
        BlockId::SpruceTrapdoor,
        BlockId::OpenSpruceTrapdoor,
        BlockId::JungleTrapdoor,
        BlockId::OpenJungleTrapdoor,
        BlockId::WillowTrapdoor,
        BlockId::OpenWillowTrapdoor,
        BlockId::AcaciaTrapdoor,
        BlockId::OpenAcaciaTrapdoor,
        BlockId::OakFenceGate,
        BlockId::OpenOakFenceGate,
        BlockId::SpruceFenceGate,
        BlockId::OpenSpruceFenceGate,
        BlockId::JungleFenceGate,
        BlockId::OpenJungleFenceGate,
        BlockId::WillowFenceGate,
        BlockId::OpenWillowFenceGate,
        BlockId::AcaciaFenceGate,
        BlockId::OpenAcaciaFenceGate,
        BlockId::StoneSlab,
        BlockId::CobblestoneSlab,
        BlockId::LimestoneSlab,
        BlockId::SlateSlab,
        BlockId::GloomrockSlab,
        BlockId::CobbledGloomrockSlab,
        BlockId::StoneStairs,
        BlockId::CobblestoneStairs,
        BlockId::LimestoneStairs,
        BlockId::SlateStairs,
        BlockId::GloomrockStairs,
        BlockId::CobbledGloomrockStairs,
        BlockId::StoneLowWall,
        BlockId::CobblestoneLowWall,
        BlockId::HewnStoneLowWall,
        BlockId::LimestoneLowWall,
        BlockId::SlateLowWall,
        BlockId::BrickLowWall,
        BlockId::GloomrockLowWall,
        BlockId::CobbledGloomrockLowWall,
        BlockId::PolishedStone,
        BlockId::PolishedLimestone,
        BlockId::PolishedSlate,
        BlockId::PolishedGloomrock,
        BlockId::ChiseledStone,
        BlockId::ChiseledLimestone,
        BlockId::ChiseledSlate,
        BlockId::ChiseledGloomrock,
        BlockId::OrangeWool,
        BlockId::MagentaWool,
        BlockId::LightBlueWool,
        BlockId::YellowWool,
        BlockId::LimeWool,
        BlockId::PinkWool,
        BlockId::GrayWool,
        BlockId::LightGrayWool,
        BlockId::CyanWool,
        BlockId::PurpleWool,
        BlockId::BlueWool,
        BlockId::BrownWool,
        BlockId::GreenWool,
        BlockId::RedWool,
        BlockId::BlackWool,
        BlockId::WhiteStainedGlass,
        BlockId::OrangeStainedGlass,
        BlockId::MagentaStainedGlass,
        BlockId::LightBlueStainedGlass,
        BlockId::YellowStainedGlass,
        BlockId::LimeStainedGlass,
        BlockId::PinkStainedGlass,
        BlockId::GrayStainedGlass,
        BlockId::LightGrayStainedGlass,
        BlockId::CyanStainedGlass,
        BlockId::PurpleStainedGlass,
        BlockId::BlueStainedGlass,
        BlockId::BrownStainedGlass,
        BlockId::GreenStainedGlass,
        BlockId::RedStainedGlass,
        BlockId::BlackStainedGlass,
        BlockId::WhiteStainedGlassPane,
        BlockId::OrangeStainedGlassPane,
        BlockId::MagentaStainedGlassPane,
        BlockId::LightBlueStainedGlassPane,
        BlockId::YellowStainedGlassPane,
        BlockId::LimeStainedGlassPane,
        BlockId::PinkStainedGlassPane,
        BlockId::GrayStainedGlassPane,
        BlockId::LightGrayStainedGlassPane,
        BlockId::CyanStainedGlassPane,
        BlockId::PurpleStainedGlassPane,
        BlockId::BlueStainedGlassPane,
        BlockId::BrownStainedGlassPane,
        BlockId::GreenStainedGlassPane,
        BlockId::RedStainedGlassPane,
        BlockId::BlackStainedGlassPane,
        BlockId::Terracotta,
        BlockId::WhiteTerracotta,
        BlockId::OrangeTerracotta,
        BlockId::MagentaTerracotta,
        BlockId::LightBlueTerracotta,
        BlockId::YellowTerracotta,
        BlockId::LimeTerracotta,
        BlockId::PinkTerracotta,
        BlockId::GrayTerracotta,
        BlockId::LightGrayTerracotta,
        BlockId::CyanTerracotta,
        BlockId::PurpleTerracotta,
        BlockId::BlueTerracotta,
        BlockId::BrownTerracotta,
        BlockId::GreenTerracotta,
        BlockId::RedTerracotta,
        BlockId::BlackTerracotta,
        BlockId::WhiteConcrete,
        BlockId::OrangeConcrete,
        BlockId::MagentaConcrete,
        BlockId::LightBlueConcrete,
        BlockId::YellowConcrete,
        BlockId::LimeConcrete,
        BlockId::PinkConcrete,
        BlockId::GrayConcrete,
        BlockId::LightGrayConcrete,
        BlockId::CyanConcrete,
        BlockId::PurpleConcrete,
        BlockId::BlueConcrete,
        BlockId::BrownConcrete,
        BlockId::GreenConcrete,
        BlockId::RedConcrete,
        BlockId::BlackConcrete,
        BlockId::WhiteConcretePowder,
        BlockId::OrangeConcretePowder,
        BlockId::MagentaConcretePowder,
        BlockId::LightBlueConcretePowder,
        BlockId::YellowConcretePowder,
        BlockId::LimeConcretePowder,
        BlockId::PinkConcretePowder,
        BlockId::GrayConcretePowder,
        BlockId::LightGrayConcretePowder,
        BlockId::CyanConcretePowder,
        BlockId::PurpleConcretePowder,
        BlockId::BlueConcretePowder,
        BlockId::BrownConcretePowder,
        BlockId::GreenConcretePowder,
        BlockId::RedConcretePowder,
        BlockId::BlackConcretePowder,
        BlockId::MapFrame,
        BlockId::AppleLeaves,
        BlockId::AppleSapling,
        BlockId::Kelp,
        BlockId::Seagrass,
        BlockId::AzureReef,
        BlockId::RoseReef,
        BlockId::AmberReef,
        BlockId::Moss,
        BlockId::CaveVine,
        BlockId::Glowcrystal,
        BlockId::CrystalCluster,
        BlockId::Geodestone,
        BlockId::Basalt,
        BlockId::PolishedBasalt,
        BlockId::Magma,
        BlockId::WhiteBedroll,
        BlockId::OrangeBedroll,
        BlockId::MagentaBedroll,
        BlockId::LightBlueBedroll,
        BlockId::YellowBedroll,
        BlockId::LimeBedroll,
        BlockId::PinkBedroll,
        BlockId::GrayBedroll,
        BlockId::LightGrayBedroll,
        BlockId::CyanBedroll,
        BlockId::PurpleBedroll,
        BlockId::BlueBedroll,
        BlockId::BrownBedroll,
        BlockId::GreenBedroll,
        BlockId::BlackBedroll,
        BlockId::OakTable,
        BlockId::SpruceTable,
        BlockId::JungleTable,
        BlockId::WillowTable,
        BlockId::AcaciaTable,
        BlockId::OakChair,
        BlockId::SpruceChair,
        BlockId::JungleChair,
        BlockId::WillowChair,
        BlockId::AcaciaChair,
        BlockId::Bookshelf,
        BlockId::WhiteCarpet,
        BlockId::OrangeCarpet,
        BlockId::MagentaCarpet,
        BlockId::LightBlueCarpet,
        BlockId::YellowCarpet,
        BlockId::LimeCarpet,
        BlockId::PinkCarpet,
        BlockId::GrayCarpet,
        BlockId::LightGrayCarpet,
        BlockId::CyanCarpet,
        BlockId::PurpleCarpet,
        BlockId::BlueCarpet,
        BlockId::BrownCarpet,
        BlockId::GreenCarpet,
        BlockId::RedCarpet,
        BlockId::BlackCarpet,
        BlockId::FlowerPot,
        BlockId::ItemFrame,
        BlockId::DawnPainting,
        BlockId::MeadowPainting,
        BlockId::HarbourPainting,
        BlockId::StillLifePainting,
        BlockId::ValleyPainting,
        BlockId::ValleyPaintingRight,
        BlockId::SeascapePainting,
        BlockId::SeascapePaintingRight,
        BlockId::ForestPainting,
        BlockId::ForestPaintingRight,
        BlockId::WhiteBanner,
        BlockId::OrangeBanner,
        BlockId::MagentaBanner,
        BlockId::LightBlueBanner,
        BlockId::YellowBanner,
        BlockId::LimeBanner,
        BlockId::PinkBanner,
        BlockId::GrayBanner,
        BlockId::LightGrayBanner,
        BlockId::CyanBanner,
        BlockId::PurpleBanner,
        BlockId::BlueBanner,
        BlockId::BrownBanner,
        BlockId::GreenBanner,
        BlockId::RedBanner,
        BlockId::BlackBanner,
        BlockId::WhiteLantern,
        BlockId::OrangeLantern,
        BlockId::MagentaLantern,
        BlockId::LightBlueLantern,
        BlockId::YellowLantern,
        BlockId::LimeLantern,
        BlockId::PinkLantern,
        BlockId::GrayLantern,
        BlockId::LightGrayLantern,
        BlockId::CyanLantern,
        BlockId::PurpleLantern,
        BlockId::BlueLantern,
        BlockId::BrownLantern,
        BlockId::GreenLantern,
        BlockId::RedLantern,
        BlockId::BlackLantern,
        BlockId::HangingWhiteLantern,
        BlockId::HangingOrangeLantern,
        BlockId::HangingMagentaLantern,
        BlockId::HangingLightBlueLantern,
        BlockId::HangingYellowLantern,
        BlockId::HangingLimeLantern,
        BlockId::HangingPinkLantern,
        BlockId::HangingGrayLantern,
        BlockId::HangingLightGrayLantern,
        BlockId::HangingCyanLantern,
        BlockId::HangingPurpleLantern,
        BlockId::HangingBlueLantern,
        BlockId::HangingBrownLantern,
        BlockId::HangingGreenLantern,
        BlockId::HangingRedLantern,
        BlockId::HangingBlackLantern,
        BlockId::Alembic,
        BlockId::WallTorch,
    ];

    pub fn is_stairs(&self) -> bool {
        self.form() == Some(Form::Stairs)
    }

    /// The form of a block of the palette.
    pub fn form(&self) -> Option<Form> {
        self.palette().map(|palette| palette.form)
    }

    /// Whether the cell below reaches into the lowest cells, as a fence does: see
    /// `ChunkMap::check_collision_box`.
    #[inline]
    pub fn is_tall(&self) -> bool {
        self.has(BlockFlags::TALL)
    }

    /// Whether this is a glass, plain or stained, which a pane reaches out to.
    #[inline]
    pub fn is_glass(&self) -> bool {
        self.has(BlockFlags::GLASS)
    }

    /// Whether this block holds a `Container`.
    #[inline]
    pub fn is_container(&self) -> bool {
        self.has(BlockFlags::CONTAINER)
    }

    /// The sapling the leaves sometimes drop.
    #[inline]
    pub fn sapling(&self) -> Option<ItemId> {
        self.props().sapling
    }

    /// The blocks this one may be planted on, if it only stands on some.
    #[inline]
    pub fn soil(&self) -> Option<&'static [BlockId]> {
        self.props().soil.as_deref()
    }

    /// The other blocks a fence, a low wall or a pane reaches out to: the whole blocks, and
    /// those of its kind. Fences and low walls reach out to fence gates.
    pub fn connects_to(&self, other: BlockId) -> bool {
        if other == *self || other.get_visibility() == BlockTransparency::Solid {
            return true;
        }
        let gate = matches!(other.form(), Some(Form::FenceGate { .. }));
        match self.form() {
            Some(Form::LowWall) => gate || other.form() == Some(Form::LowWall),
            Some(Form::Pane) => other.form() == Some(Form::Pane) || other.is_glass(),
            _ => *self == Self::Fence && gate,
        }
    }

    #[inline]
    pub fn get_hitbox(&self) -> BlockHitbox {
        self.props().hitbox
    }

    #[inline]
    pub fn get_ray_hitbox(&self) -> BlockHitbox {
        self.props().ray_hitbox
    }

    /// Number of mining hits needed to break the block by hand, one every `HIT_MS` the button
    /// is held: a block takes `steps / 10` seconds. `None` if it cannot be broken.
    #[inline]
    pub fn get_break_time(&self) -> Option<u8> {
        self.props().break_time
    }

    /// Whether a tool of `kind` breaks this block faster than a hand, at its speed.
    #[inline]
    pub fn is_mined_faster_by(&self, kind: ToolKind) -> bool {
        self.props().faster_tools & crate::content::tool_bit(kind) != 0
    }

    /// The tool this block must be broken with to drop anything, like stone and a pickaxe, and
    /// the mining level its tier must reach, like iron ore and a stone pickaxe. Broken otherwise,
    /// it still goes, but drops nothing.
    #[inline]
    pub fn get_required_tool(&self) -> Option<RequiredTool> {
        self.props().required_tool
    }

    /// Whether this block drops its items when broken holding `tool`, see `get_required_tool`.
    pub fn is_harvested_by(&self, tool: Option<Tool>) -> bool {
        self.get_required_tool().is_none_or(|required| {
            tool.is_some_and(|tool| {
                tool.kind == required.kind && tool.tier.mining_level() >= required.level
            })
        })
    }

    /// What breaking this block `nb_drops` times gives. Each drop is picked at random in the
    /// drop table; a table of one entry always gives that entry. Only
    /// the server rolls it: the items it drops are what the clients are told of.
    pub fn get_drops(&self, nb_drops: u32) -> HashMap<ItemId, u32> {
        let table = self.get_drop_table();
        let total_chance: u32 = table.iter().map(|(chance, ..)| chance).sum();

        let mut drops = HashMap::new();
        if total_chance == 0 {
            return drops;
        }
        for _ in 0..nb_drops {
            let roll = rand::thread_rng().gen_range(0..total_chance);
            if let Some((item, nb)) = pick_drop(&table, roll) {
                *drops.entry(item).or_insert(0) += nb;
            }
        }
        drops
    }

    /// Specifies the drop table of a given block
    /// Drops are specified this way : `(relative_chance, corresponding_item, base_number)`
    #[inline]
    pub fn get_drop_table(&self) -> Vec<(u32, ItemId, u32)> {
        self.props().drops.clone()
    }

    /// Whether a right click on this block uses it, like a crafting table or a chest, instead of
    /// placing a block against it. Sneaking places the block.
    #[inline]
    pub fn is_usable(&self) -> bool {
        self.has(BlockFlags::USABLE)
    }

    /// Whether this block is a furnace, lit or out: both hold a `Furnace` and open alike.
    #[inline]
    pub fn is_furnace(&self) -> bool {
        matches!(self.behaviour(), Behaviour::Furnace { .. })
    }

    /// Whether this block holds a `Furnace`: a furnace, lit or out, or an alembic.
    pub fn holds_furnace(&self) -> bool {
        self.is_furnace() || *self == Self::Alembic
    }

    /// Whether a block placed in the cell of this one takes its place.
    #[inline]
    pub fn is_replaceable(&self) -> bool {
        self.has(BlockFlags::REPLACEABLE)
    }

    /// Whether this block grows under water, which fills the rest of its cell: it is seen,
    /// swum in and breathed in as water, only grows in water, and leaves water when broken.
    #[inline]
    pub fn is_submerged(&self) -> bool {
        self.has(BlockFlags::SUBMERGED)
    }

    /// Whether the cell of this block holds water: water, or a plant that grows in it.
    pub fn is_watery(&self) -> bool {
        *self == Self::Water || self.is_submerged()
    }

    /// Whether this block hangs from the block above it, rather than standing on the one below.
    #[inline]
    pub fn hangs(&self) -> bool {
        self.has(BlockFlags::HANGS)
    }

    #[inline]
    pub fn get_visibility(&self) -> BlockTransparency {
        self.props().visibility
    }

    /// How much the block dims the light that goes through it, as Minecraft's light opacity:
    /// `None` for a block no light goes through. Light loses a level from one cell to the next,
    /// or this much if more, but the light of the sky that goes straight down loses only this.
    #[inline]
    pub fn light_opacity(&self) -> Option<u8> {
        self.props().light_opacity
    }

    /// The light the block gives, from 0 for none to 15, as Minecraft's light levels.
    #[inline]
    pub fn light_emission(&self) -> u8 {
        self.props().light_emission
    }
}

impl GameElementId for BlockId {}

/// The entry of a drop table that `roll` falls on, `roll` being below the sum of the chances:
/// each entry takes as many consecutive rolls as its relative chance.
fn pick_drop(table: &[(u32, ItemId, u32)], mut roll: u32) -> Option<(ItemId, u32)> {
    for &(chance, item, nb) in table {
        if roll < chance {
            return Some((item, nb));
        }
        roll -= chance;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::ItemType;

    const ALL_BLOCKS: [BlockId; 400] = BlockId::ALL;

    /// Blocks that cannot be broken by hand.
    const UNBREAKABLE_BLOCKS: [BlockId; 3] = [BlockId::Bedrock, BlockId::Water, BlockId::Lava];

    /// Fails to compile when a `BlockId` variant is added without updating `ALL_BLOCKS`.
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
            | BlockId::Water
            | BlockId::CraftingTable
            | BlockId::SprucePlanks
            | BlockId::Furnace
            | BlockId::Chest
            | BlockId::CoalOre
            | BlockId::IronOre
            | BlockId::GoldOre
            | BlockId::Gravel
            | BlockId::Torch
            | BlockId::WallTorch
            | BlockId::JungleLog
            | BlockId::JunglePlanks
            | BlockId::JungleLeaves
            | BlockId::Fern
            | BlockId::Vine
            | BlockId::Cocoa
            | BlockId::LitFurnace
            | BlockId::DiamondOre
            | BlockId::IronBlock
            | BlockId::GoldBlock
            | BlockId::DiamondBlock
            | BlockId::HewnStone
            | BlockId::Limestone
            | BlockId::Slate
            | BlockId::Clay
            | BlockId::BrickWall
            | BlockId::RammedEarth
            | BlockId::Thatch
            | BlockId::CoalBlock
            | BlockId::CopperOre
            | BlockId::TinOre
            | BlockId::CopperBlock
            | BlockId::TinBlock
            | BlockId::BronzeBlock
            | BlockId::WillowLog
            | BlockId::WillowPlanks
            | BlockId::WillowLeaves
            | BlockId::AcaciaLog
            | BlockId::AcaciaPlanks
            | BlockId::AcaciaLeaves
            | BlockId::Reed
            | BlockId::Heather
            | BlockId::Mushroom
            | BlockId::OakSlab
            | BlockId::HewnStoneSlab
            | BlockId::BrickSlab
            | BlockId::Lantern
            | BlockId::OakStairs
            | BlockId::HewnStoneStairs
            | BlockId::BrickStairs
            | BlockId::Door
            | BlockId::DoorTop
            | BlockId::OpenDoor
            | BlockId::OpenDoorTop
            | BlockId::Ladder
            | BlockId::Fence
            | BlockId::GlassPane
            | BlockId::HangingLantern
            | BlockId::OakSapling
            | BlockId::SpruceSapling
            | BlockId::JungleSapling
            | BlockId::WillowSapling
            | BlockId::AcaciaSapling
            | BlockId::Wool
            | BlockId::BerryBush
            | BlockId::RipeBerryBush
            | BlockId::TilledSoil
            | BlockId::RyeSprout
            | BlockId::YoungRye
            | BlockId::RipeRye
            | BlockId::Bedroll
            | BlockId::Barrel
            | BlockId::RainCistern
            | BlockId::FullRainCistern
            | BlockId::Campfire
            | BlockId::UnlitCampfire
            | BlockId::Sign
            | BlockId::Fire
            | BlockId::Lava
            | BlockId::Obsidian
            | BlockId::CarrotSprout
            | BlockId::YoungCarrots
            | BlockId::TallCarrots
            | BlockId::RipeCarrots
            | BlockId::PotatoSprout
            | BlockId::YoungPotatoes
            | BlockId::TallPotatoes
            | BlockId::RipePotatoes
            | BlockId::EmberHeart
            | BlockId::EngravingTable
            | BlockId::Gloomrock
            | BlockId::CobbledGloomrock
            | BlockId::Anvil
            | BlockId::MapFrame
            | BlockId::AppleLeaves
            | BlockId::AppleSapling
            | BlockId::Kelp
            | BlockId::Seagrass
            | BlockId::AzureReef
            | BlockId::RoseReef
            | BlockId::AmberReef
            | BlockId::Moss
            | BlockId::CaveVine
            | BlockId::Glowcrystal
            | BlockId::CrystalCluster
            | BlockId::Geodestone
            | BlockId::Basalt
            | BlockId::PolishedBasalt
            | BlockId::Magma
            | crate::new_palette_blocks!()
            | crate::new_decor_blocks!()
            | BlockId::Alembic => {}
        }
    }

    fn regular_blocks() -> impl Iterator<Item = BlockId> {
        ALL_BLOCKS
            .into_iter()
            .inspect(|id| all_blocks_is_exhaustive(*id))
            .filter(|id| !UNBREAKABLE_BLOCKS.contains(id))
    }

    #[test]
    fn get_break_time_of_regular_blocks() {
        assert_eq!(BlockId::Dirt.get_break_time(), Some(10));
        assert_eq!(BlockId::Stone.get_break_time(), Some(20));
        assert_eq!(BlockId::Dandelion.get_break_time(), Some(2));
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

    const ENCODING_WARNING: &str = "the encoded value of an existing variant changed: this \
        corrupts the saved worlds and breaks the protocol. New variants go at the END of the \
        enum; then add them to this table (and bump PROTOCOL_ID).";

    fn encoded<T: serde::Serialize>(value: &T) -> Vec<u8> {
        use bincode::Options;
        bincode::options().serialize(value).unwrap()
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
    fn block_encoding_is_pinned() {
        for id in ALL_BLOCKS {
            let expected: u16 = match id {
                BlockId::Dirt => 0,
                BlockId::Debug => 1,
                BlockId::Grass => 2,
                BlockId::Stone => 3,
                BlockId::OakLog => 4,
                BlockId::OakPlanks => 5,
                BlockId::OakLeaves => 6,
                BlockId::Sand => 7,
                BlockId::Cactus => 8,
                BlockId::Ice => 9,
                BlockId::Glass => 10,
                BlockId::Bedrock => 11,
                BlockId::Dandelion => 12,
                BlockId::Poppy => 13,
                BlockId::TallGrass => 14,
                BlockId::Cobblestone => 15,
                BlockId::Snow => 16,
                BlockId::SpruceLeaves => 17,
                BlockId::SpruceLog => 18,
                BlockId::Water => 19,
                BlockId::CraftingTable => 20,
                BlockId::SprucePlanks => 21,
                BlockId::Furnace => 22,
                BlockId::Chest => 23,
                BlockId::CoalOre => 24,
                BlockId::IronOre => 25,
                BlockId::GoldOre => 26,
                BlockId::Gravel => 27,
                BlockId::Torch => 28,
                BlockId::JungleLog => 29,
                BlockId::JunglePlanks => 30,
                BlockId::JungleLeaves => 31,
                BlockId::Fern => 32,
                BlockId::Vine => 33,
                BlockId::Cocoa => 34,
                BlockId::LitFurnace => 35,
                BlockId::DiamondOre => 36,
                BlockId::IronBlock => 37,
                BlockId::GoldBlock => 38,
                BlockId::DiamondBlock => 39,
                BlockId::HewnStone => 40,
                BlockId::Limestone => 41,
                BlockId::Slate => 42,
                BlockId::Clay => 43,
                BlockId::BrickWall => 44,
                BlockId::RammedEarth => 45,
                BlockId::Thatch => 46,
                BlockId::CoalBlock => 47,
                BlockId::CopperOre => 48,
                BlockId::TinOre => 49,
                BlockId::CopperBlock => 50,
                BlockId::TinBlock => 51,
                BlockId::BronzeBlock => 52,
                BlockId::WillowLog => 53,
                BlockId::WillowPlanks => 54,
                BlockId::WillowLeaves => 55,
                BlockId::AcaciaLog => 56,
                BlockId::AcaciaPlanks => 57,
                BlockId::AcaciaLeaves => 58,
                BlockId::Reed => 59,
                BlockId::Heather => 60,
                BlockId::Mushroom => 61,
                BlockId::OakSlab => 62,
                BlockId::HewnStoneSlab => 63,
                BlockId::BrickSlab => 64,
                BlockId::Lantern => 65,
                BlockId::OakStairs => 66,
                BlockId::HewnStoneStairs => 67,
                BlockId::BrickStairs => 68,
                BlockId::Door => 69,
                BlockId::DoorTop => 70,
                BlockId::OpenDoor => 71,
                BlockId::OpenDoorTop => 72,
                BlockId::Ladder => 73,
                BlockId::Fence => 74,
                BlockId::GlassPane => 75,
                BlockId::HangingLantern => 76,
                BlockId::OakSapling => 77,
                BlockId::SpruceSapling => 78,
                BlockId::JungleSapling => 79,
                BlockId::WillowSapling => 80,
                BlockId::AcaciaSapling => 81,
                BlockId::Wool => 82,
                BlockId::BerryBush => 83,
                BlockId::RipeBerryBush => 84,
                BlockId::TilledSoil => 85,
                BlockId::RyeSprout => 86,
                BlockId::YoungRye => 87,
                BlockId::RipeRye => 88,
                BlockId::Bedroll => 89,
                BlockId::Barrel => 90,
                BlockId::RainCistern => 91,
                BlockId::FullRainCistern => 92,
                BlockId::Campfire => 93,
                BlockId::UnlitCampfire => 94,
                BlockId::Sign => 95,
                BlockId::Fire => 96,
                BlockId::Lava => 97,
                BlockId::Obsidian => 98,
                BlockId::CarrotSprout => 99,
                BlockId::YoungCarrots => 100,
                BlockId::TallCarrots => 101,
                BlockId::RipeCarrots => 102,
                BlockId::PotatoSprout => 103,
                BlockId::YoungPotatoes => 104,
                BlockId::TallPotatoes => 105,
                BlockId::RipePotatoes => 106,
                BlockId::EmberHeart => 107,
                BlockId::EngravingTable => 108,
                BlockId::Gloomrock => 109,
                BlockId::CobbledGloomrock => 110,
                BlockId::Anvil => 111,
                BlockId::SpruceSlab => 112,
                BlockId::JungleSlab => 113,
                BlockId::WillowSlab => 114,
                BlockId::AcaciaSlab => 115,
                BlockId::SpruceStairs => 116,
                BlockId::JungleStairs => 117,
                BlockId::WillowStairs => 118,
                BlockId::AcaciaStairs => 119,
                BlockId::SpruceDoor => 120,
                BlockId::SpruceDoorTop => 121,
                BlockId::OpenSpruceDoor => 122,
                BlockId::OpenSpruceDoorTop => 123,
                BlockId::JungleDoor => 124,
                BlockId::JungleDoorTop => 125,
                BlockId::OpenJungleDoor => 126,
                BlockId::OpenJungleDoorTop => 127,
                BlockId::WillowDoor => 128,
                BlockId::WillowDoorTop => 129,
                BlockId::OpenWillowDoor => 130,
                BlockId::OpenWillowDoorTop => 131,
                BlockId::AcaciaDoor => 132,
                BlockId::AcaciaDoorTop => 133,
                BlockId::OpenAcaciaDoor => 134,
                BlockId::OpenAcaciaDoorTop => 135,
                BlockId::OakTrapdoor => 136,
                BlockId::OpenOakTrapdoor => 137,
                BlockId::SpruceTrapdoor => 138,
                BlockId::OpenSpruceTrapdoor => 139,
                BlockId::JungleTrapdoor => 140,
                BlockId::OpenJungleTrapdoor => 141,
                BlockId::WillowTrapdoor => 142,
                BlockId::OpenWillowTrapdoor => 143,
                BlockId::AcaciaTrapdoor => 144,
                BlockId::OpenAcaciaTrapdoor => 145,
                BlockId::OakFenceGate => 146,
                BlockId::OpenOakFenceGate => 147,
                BlockId::SpruceFenceGate => 148,
                BlockId::OpenSpruceFenceGate => 149,
                BlockId::JungleFenceGate => 150,
                BlockId::OpenJungleFenceGate => 151,
                BlockId::WillowFenceGate => 152,
                BlockId::OpenWillowFenceGate => 153,
                BlockId::AcaciaFenceGate => 154,
                BlockId::OpenAcaciaFenceGate => 155,
                BlockId::StoneSlab => 156,
                BlockId::CobblestoneSlab => 157,
                BlockId::LimestoneSlab => 158,
                BlockId::SlateSlab => 159,
                BlockId::GloomrockSlab => 160,
                BlockId::CobbledGloomrockSlab => 161,
                BlockId::StoneStairs => 162,
                BlockId::CobblestoneStairs => 163,
                BlockId::LimestoneStairs => 164,
                BlockId::SlateStairs => 165,
                BlockId::GloomrockStairs => 166,
                BlockId::CobbledGloomrockStairs => 167,
                BlockId::StoneLowWall => 168,
                BlockId::CobblestoneLowWall => 169,
                BlockId::HewnStoneLowWall => 170,
                BlockId::LimestoneLowWall => 171,
                BlockId::SlateLowWall => 172,
                BlockId::BrickLowWall => 173,
                BlockId::GloomrockLowWall => 174,
                BlockId::CobbledGloomrockLowWall => 175,
                BlockId::PolishedStone => 176,
                BlockId::PolishedLimestone => 177,
                BlockId::PolishedSlate => 178,
                BlockId::PolishedGloomrock => 179,
                BlockId::ChiseledStone => 180,
                BlockId::ChiseledLimestone => 181,
                BlockId::ChiseledSlate => 182,
                BlockId::ChiseledGloomrock => 183,
                BlockId::OrangeWool => 184,
                BlockId::MagentaWool => 185,
                BlockId::LightBlueWool => 186,
                BlockId::YellowWool => 187,
                BlockId::LimeWool => 188,
                BlockId::PinkWool => 189,
                BlockId::GrayWool => 190,
                BlockId::LightGrayWool => 191,
                BlockId::CyanWool => 192,
                BlockId::PurpleWool => 193,
                BlockId::BlueWool => 194,
                BlockId::BrownWool => 195,
                BlockId::GreenWool => 196,
                BlockId::RedWool => 197,
                BlockId::BlackWool => 198,
                BlockId::WhiteStainedGlass => 199,
                BlockId::OrangeStainedGlass => 200,
                BlockId::MagentaStainedGlass => 201,
                BlockId::LightBlueStainedGlass => 202,
                BlockId::YellowStainedGlass => 203,
                BlockId::LimeStainedGlass => 204,
                BlockId::PinkStainedGlass => 205,
                BlockId::GrayStainedGlass => 206,
                BlockId::LightGrayStainedGlass => 207,
                BlockId::CyanStainedGlass => 208,
                BlockId::PurpleStainedGlass => 209,
                BlockId::BlueStainedGlass => 210,
                BlockId::BrownStainedGlass => 211,
                BlockId::GreenStainedGlass => 212,
                BlockId::RedStainedGlass => 213,
                BlockId::BlackStainedGlass => 214,
                BlockId::WhiteStainedGlassPane => 215,
                BlockId::OrangeStainedGlassPane => 216,
                BlockId::MagentaStainedGlassPane => 217,
                BlockId::LightBlueStainedGlassPane => 218,
                BlockId::YellowStainedGlassPane => 219,
                BlockId::LimeStainedGlassPane => 220,
                BlockId::PinkStainedGlassPane => 221,
                BlockId::GrayStainedGlassPane => 222,
                BlockId::LightGrayStainedGlassPane => 223,
                BlockId::CyanStainedGlassPane => 224,
                BlockId::PurpleStainedGlassPane => 225,
                BlockId::BlueStainedGlassPane => 226,
                BlockId::BrownStainedGlassPane => 227,
                BlockId::GreenStainedGlassPane => 228,
                BlockId::RedStainedGlassPane => 229,
                BlockId::BlackStainedGlassPane => 230,
                BlockId::Terracotta => 231,
                BlockId::WhiteTerracotta => 232,
                BlockId::OrangeTerracotta => 233,
                BlockId::MagentaTerracotta => 234,
                BlockId::LightBlueTerracotta => 235,
                BlockId::YellowTerracotta => 236,
                BlockId::LimeTerracotta => 237,
                BlockId::PinkTerracotta => 238,
                BlockId::GrayTerracotta => 239,
                BlockId::LightGrayTerracotta => 240,
                BlockId::CyanTerracotta => 241,
                BlockId::PurpleTerracotta => 242,
                BlockId::BlueTerracotta => 243,
                BlockId::BrownTerracotta => 244,
                BlockId::GreenTerracotta => 245,
                BlockId::RedTerracotta => 246,
                BlockId::BlackTerracotta => 247,
                BlockId::WhiteConcrete => 248,
                BlockId::OrangeConcrete => 249,
                BlockId::MagentaConcrete => 250,
                BlockId::LightBlueConcrete => 251,
                BlockId::YellowConcrete => 252,
                BlockId::LimeConcrete => 253,
                BlockId::PinkConcrete => 254,
                BlockId::GrayConcrete => 255,
                BlockId::LightGrayConcrete => 256,
                BlockId::CyanConcrete => 257,
                BlockId::PurpleConcrete => 258,
                BlockId::BlueConcrete => 259,
                BlockId::BrownConcrete => 260,
                BlockId::GreenConcrete => 261,
                BlockId::RedConcrete => 262,
                BlockId::BlackConcrete => 263,
                BlockId::WhiteConcretePowder => 264,
                BlockId::OrangeConcretePowder => 265,
                BlockId::MagentaConcretePowder => 266,
                BlockId::LightBlueConcretePowder => 267,
                BlockId::YellowConcretePowder => 268,
                BlockId::LimeConcretePowder => 269,
                BlockId::PinkConcretePowder => 270,
                BlockId::GrayConcretePowder => 271,
                BlockId::LightGrayConcretePowder => 272,
                BlockId::CyanConcretePowder => 273,
                BlockId::PurpleConcretePowder => 274,
                BlockId::BlueConcretePowder => 275,
                BlockId::BrownConcretePowder => 276,
                BlockId::GreenConcretePowder => 277,
                BlockId::RedConcretePowder => 278,
                BlockId::BlackConcretePowder => 279,
                BlockId::MapFrame => 280,
                BlockId::AppleLeaves => 281,
                BlockId::AppleSapling => 282,
                BlockId::Kelp => 283,
                BlockId::Seagrass => 284,
                BlockId::AzureReef => 285,
                BlockId::RoseReef => 286,
                BlockId::AmberReef => 287,
                BlockId::Moss => 288,
                BlockId::CaveVine => 289,
                BlockId::Glowcrystal => 290,
                BlockId::CrystalCluster => 291,
                BlockId::Geodestone => 292,
                BlockId::Basalt => 293,
                BlockId::PolishedBasalt => 294,
                BlockId::Magma => 295,
                BlockId::WhiteBedroll => 296,
                BlockId::OrangeBedroll => 297,
                BlockId::MagentaBedroll => 298,
                BlockId::LightBlueBedroll => 299,
                BlockId::YellowBedroll => 300,
                BlockId::LimeBedroll => 301,
                BlockId::PinkBedroll => 302,
                BlockId::GrayBedroll => 303,
                BlockId::LightGrayBedroll => 304,
                BlockId::CyanBedroll => 305,
                BlockId::PurpleBedroll => 306,
                BlockId::BlueBedroll => 307,
                BlockId::BrownBedroll => 308,
                BlockId::GreenBedroll => 309,
                BlockId::BlackBedroll => 310,
                BlockId::OakTable => 311,
                BlockId::SpruceTable => 312,
                BlockId::JungleTable => 313,
                BlockId::WillowTable => 314,
                BlockId::AcaciaTable => 315,
                BlockId::OakChair => 316,
                BlockId::SpruceChair => 317,
                BlockId::JungleChair => 318,
                BlockId::WillowChair => 319,
                BlockId::AcaciaChair => 320,
                BlockId::Bookshelf => 321,
                BlockId::WhiteCarpet => 322,
                BlockId::OrangeCarpet => 323,
                BlockId::MagentaCarpet => 324,
                BlockId::LightBlueCarpet => 325,
                BlockId::YellowCarpet => 326,
                BlockId::LimeCarpet => 327,
                BlockId::PinkCarpet => 328,
                BlockId::GrayCarpet => 329,
                BlockId::LightGrayCarpet => 330,
                BlockId::CyanCarpet => 331,
                BlockId::PurpleCarpet => 332,
                BlockId::BlueCarpet => 333,
                BlockId::BrownCarpet => 334,
                BlockId::GreenCarpet => 335,
                BlockId::RedCarpet => 336,
                BlockId::BlackCarpet => 337,
                BlockId::FlowerPot => 338,
                BlockId::ItemFrame => 339,
                BlockId::DawnPainting => 340,
                BlockId::MeadowPainting => 341,
                BlockId::HarbourPainting => 342,
                BlockId::StillLifePainting => 343,
                BlockId::ValleyPainting => 344,
                BlockId::ValleyPaintingRight => 345,
                BlockId::SeascapePainting => 346,
                BlockId::SeascapePaintingRight => 347,
                BlockId::ForestPainting => 348,
                BlockId::ForestPaintingRight => 349,
                BlockId::WhiteBanner => 350,
                BlockId::OrangeBanner => 351,
                BlockId::MagentaBanner => 352,
                BlockId::LightBlueBanner => 353,
                BlockId::YellowBanner => 354,
                BlockId::LimeBanner => 355,
                BlockId::PinkBanner => 356,
                BlockId::GrayBanner => 357,
                BlockId::LightGrayBanner => 358,
                BlockId::CyanBanner => 359,
                BlockId::PurpleBanner => 360,
                BlockId::BlueBanner => 361,
                BlockId::BrownBanner => 362,
                BlockId::GreenBanner => 363,
                BlockId::RedBanner => 364,
                BlockId::BlackBanner => 365,
                BlockId::WhiteLantern => 366,
                BlockId::OrangeLantern => 367,
                BlockId::MagentaLantern => 368,
                BlockId::LightBlueLantern => 369,
                BlockId::YellowLantern => 370,
                BlockId::LimeLantern => 371,
                BlockId::PinkLantern => 372,
                BlockId::GrayLantern => 373,
                BlockId::LightGrayLantern => 374,
                BlockId::CyanLantern => 375,
                BlockId::PurpleLantern => 376,
                BlockId::BlueLantern => 377,
                BlockId::BrownLantern => 378,
                BlockId::GreenLantern => 379,
                BlockId::RedLantern => 380,
                BlockId::BlackLantern => 381,
                BlockId::HangingWhiteLantern => 382,
                BlockId::HangingOrangeLantern => 383,
                BlockId::HangingMagentaLantern => 384,
                BlockId::HangingLightBlueLantern => 385,
                BlockId::HangingYellowLantern => 386,
                BlockId::HangingLimeLantern => 387,
                BlockId::HangingPinkLantern => 388,
                BlockId::HangingGrayLantern => 389,
                BlockId::HangingLightGrayLantern => 390,
                BlockId::HangingCyanLantern => 391,
                BlockId::HangingPurpleLantern => 392,
                BlockId::HangingBlueLantern => 393,
                BlockId::HangingBrownLantern => 394,
                BlockId::HangingGreenLantern => 395,
                BlockId::HangingRedLantern => 396,
                BlockId::HangingBlackLantern => 397,
                BlockId::Alembic => 398,
                BlockId::WallTorch => 399,
            };
            assert_eq!(encoded(&id), varint(expected), "{id:?}: {ENCODING_WARNING}");
        }
    }

    #[test]
    fn new_blocks_drop_their_own_item() {
        for (block, item) in [
            (BlockId::CraftingTable, ItemId::CraftingTable),
            (BlockId::SprucePlanks, ItemId::SprucePlanks),
            (BlockId::Furnace, ItemId::Furnace),
            (BlockId::Chest, ItemId::Chest),
        ] {
            assert_eq!(block.get_drops(1), HashMap::from([(item, 1)]));
            assert_eq!(item.get_default_type(), ItemType::Block(block));
        }
        assert_eq!(ItemId::Stick.get_default_type(), ItemType::Generic);
    }

    #[test]
    fn fire_is_a_glowing_decoration_put_out_by_a_hit_that_drops_nothing() {
        let fire = BlockId::Fire;
        assert!(matches!(fire.get_hitbox(), BlockHitbox::None));
        let BlockHitbox::Aabb(ray) = fire.get_ray_hitbox() else {
            panic!("a fire is aimed at");
        };
        assert!(ray.max.y < 0.5, "a small box at the bottom of the cell");
        assert_eq!(fire.get_break_time(), Some(1));
        assert_eq!(fire.get_required_tool(), None);
        assert!(fire.get_drops(10).is_empty());
        assert!(fire.is_replaceable());
        assert_eq!(fire.get_visibility(), BlockTransparency::Decoration);
        assert_eq!(fire.light_emission(), 15);
        // its last stage of breaking is reached at the first hit, without a division by zero
        let hit = BlockData {
            breaking_progress: 1,
            ..BlockData::new(fire, BlockDirection::Front)
        };
        assert_eq!(hit.get_breaking_level(), MAX_BREAKING_LEVEL);
    }

    #[test]
    fn lava_is_a_glowing_unbreakable_liquid_that_a_block_replaces() {
        let lava = BlockId::Lava;
        assert_eq!(lava.get_visibility(), BlockTransparency::Liquid);
        assert!(matches!(lava.get_hitbox(), BlockHitbox::None));
        assert!(matches!(lava.get_ray_hitbox(), BlockHitbox::None));
        assert_eq!(lava.get_break_time(), None);
        assert!(lava.is_replaceable());
        assert_eq!(lava.light_emission(), 15);
        assert!(lava.get_drops(5).is_empty());
    }

    #[test]
    fn obsidian_takes_long_needs_a_diamond_pickaxe_and_drops_itself() {
        let obsidian = BlockId::Obsidian;
        assert_eq!(obsidian.get_break_time(), Some(250));
        assert!(obsidian.is_mined_faster_by(ToolKind::Pickaxe));
        assert_eq!(
            obsidian.get_required_tool(),
            Some(RequiredTool {
                kind: ToolKind::Pickaxe,
                level: 3
            })
        );
        assert!(obsidian.is_harvested_by(ItemId::DiamondPickaxe.tool()));
        assert!(!obsidian.is_harvested_by(ItemId::IronPickaxe.tool()));
        assert_eq!(
            obsidian.get_drops(1),
            HashMap::from([(ItemId::Obsidian, 1)])
        );
        assert_eq!(obsidian.get_visibility(), BlockTransparency::Solid);
        assert_eq!(
            ItemId::Obsidian.get_default_type(),
            ItemType::Block(obsidian)
        );
    }

    #[test]
    fn carrots_and_potatoes_are_crops_of_tilled_soil_that_drop_more_once_ripe() {
        for (stages, item) in [
            (
                [
                    BlockId::CarrotSprout,
                    BlockId::YoungCarrots,
                    BlockId::TallCarrots,
                    BlockId::RipeCarrots,
                ],
                ItemId::Carrot,
            ),
            (
                [
                    BlockId::PotatoSprout,
                    BlockId::YoungPotatoes,
                    BlockId::TallPotatoes,
                    BlockId::RipePotatoes,
                ],
                ItemId::Potato,
            ),
        ] {
            assert_eq!(item.get_default_type(), ItemType::Block(stages[0]));
            assert_eq!(stages[0].soil(), Some(&[BlockId::TilledSoil][..]));
            for stage in stages {
                assert!(matches!(stage.get_hitbox(), BlockHitbox::None), "{stage:?}");
                assert!(matches!(stage.get_ray_hitbox(), BlockHitbox::Aabb(_)));
                assert_eq!(stage.get_visibility(), BlockTransparency::Decoration);
                assert_eq!(stage.get_break_time(), Some(2));
            }
            for unripe in &stages[..3] {
                assert_eq!(unripe.get_drop_table(), vec![(1, item, 1)], "{unripe:?}");
            }
            let ripe = stages[3];
            assert_eq!(
                ripe.get_drop_table(),
                vec![(1, item, 2), (1, item, 3), (1, item, 4)]
            );
            let counts: std::collections::HashSet<u32> =
                (0..100).map(|_| ripe.get_drops(1)[&item]).collect();
            assert_eq!(counts, [2, 3, 4].into());
        }
    }

    #[test]
    fn tall_grass_drops_a_carrot_or_a_potato_one_time_in_fifty_each() {
        let table = BlockId::TallGrass.get_drop_table();
        let total: u32 = table.iter().map(|(chance, ..)| chance).sum();
        assert_eq!(total, 50);
        let chance = |wanted| {
            table
                .iter()
                .find(|(_, item, _)| *item == wanted)
                .map(|(chance, ..)| *chance)
        };
        assert_eq!(chance(ItemId::Carrot), Some(1));
        assert_eq!(chance(ItemId::Potato), Some(1));
        // Main's three keep their ratios
        assert_eq!(chance(ItemId::TallGrass), Some(24));
        assert_eq!(chance(ItemId::Fiber), Some(16));
        assert_eq!(chance(ItemId::Rye), Some(8));
    }

    #[test]
    fn gravel_drops_flint_one_time_in_ten() {
        assert_eq!(
            BlockId::Gravel.get_drop_table(),
            vec![(9, ItemId::Gravel, 1), (1, ItemId::Flint, 1)]
        );
        let drops = BlockId::Gravel.get_drops(2000);
        let flint = drops.get(&ItemId::Flint).copied().unwrap_or(0);
        assert!((100..300).contains(&flint), "{flint} flint in 2000");
        assert_eq!(drops[&ItemId::Gravel] + flint, 2000);
    }

    #[test]
    fn the_lit_furnace_is_a_furnace_that_glows() {
        let (lit, out) = (BlockId::LitFurnace, BlockId::Furnace);
        assert_eq!(lit.get_break_time(), out.get_break_time());
        assert_eq!(lit.get_required_tool(), out.get_required_tool());
        assert!(lit.get_required_tool().is_some());
        assert!(lit.is_mined_faster_by(ToolKind::Pickaxe));
        assert_eq!(lit.get_drops(1), HashMap::from([(ItemId::Furnace, 1)]));
        assert_eq!(lit.get_visibility(), BlockTransparency::Solid);
        assert_eq!((lit.light_emission(), out.light_emission()), (13, 0));
        for id in [lit, out] {
            assert!(id.is_usable(), "{id:?}");
            assert!(id.is_furnace(), "{id:?}");
        }
        assert!(!BlockId::Chest.is_furnace());
    }

    #[test]
    fn jungle_wood_breaks_like_oak_wood() {
        for (jungle, oak) in [
            (BlockId::JungleLog, BlockId::OakLog),
            (BlockId::JunglePlanks, BlockId::OakPlanks),
            (BlockId::JungleLeaves, BlockId::OakLeaves),
        ] {
            assert_eq!(jungle.get_break_time(), oak.get_break_time(), "{jungle:?}");
            assert_eq!(jungle.get_required_tool(), None, "{jungle:?}");
        }
        assert!(BlockId::JungleLog.is_mined_faster_by(ToolKind::Axe));
        assert!(BlockId::JunglePlanks.is_mined_faster_by(ToolKind::Axe));
        assert!(BlockId::JungleLeaves.is_mined_faster_by(ToolKind::Hoe));
        assert!(BlockId::JungleLeaves.is_mined_faster_by(ToolKind::Sword));
        assert!(!BlockId::JungleLeaves.is_mined_faster_by(ToolKind::Axe));
    }

    #[test]
    fn jungle_wood_drops_itself_and_its_leaves_drop_nothing() {
        for (block, item) in [
            (BlockId::JungleLog, ItemId::JungleLog),
            (BlockId::JunglePlanks, ItemId::JunglePlanks),
        ] {
            assert_eq!(block.get_drops(1), HashMap::from([(item, 1)]));
        }
        assert!(BlockId::JungleLeaves.get_drops(1).is_empty());
        for (block, item) in [
            (BlockId::JungleLog, ItemId::JungleLog),
            (BlockId::JunglePlanks, ItemId::JunglePlanks),
            (BlockId::JungleLeaves, ItemId::JungleLeaves),
        ] {
            assert_eq!(item.get_default_type(), ItemType::Block(block));
        }
    }

    #[test]
    fn jungle_leaves_are_seen_through_like_the_other_leaves() {
        assert_eq!(
            BlockId::JungleLeaves.get_visibility(),
            BlockTransparency::Transparent
        );
        // and dim the light as they do, while the wood stops it as the oak's
        for (jungle, oak) in [
            (BlockId::JungleLeaves, BlockId::OakLeaves),
            (BlockId::JungleLog, BlockId::OakLog),
            (BlockId::JunglePlanks, BlockId::OakPlanks),
        ] {
            assert_eq!(jungle.light_opacity(), oak.light_opacity(), "{jungle:?}");
        }
        assert_eq!(
            BlockId::JungleLog.get_visibility(),
            BlockTransparency::Solid
        );
        assert_eq!(
            BlockId::JunglePlanks.get_visibility(),
            BlockTransparency::Solid
        );
    }

    #[test]
    fn the_fern_behaves_like_tall_grass() {
        let (fern, grass) = (BlockId::Fern, BlockId::TallGrass);
        assert!(matches!(fern.get_hitbox(), BlockHitbox::None));
        assert!(matches!(fern.get_ray_hitbox(), BlockHitbox::Aabb(_)));
        assert_eq!(fern.get_break_time(), grass.get_break_time());
        assert_eq!(fern.get_required_tool(), None);
        assert_eq!(fern.get_visibility(), BlockTransparency::Decoration);
        assert_eq!(fern.light_opacity(), grass.light_opacity());
        assert_eq!(fern.get_drops(1), HashMap::from([(ItemId::Fern, 1)]));
        assert_eq!(ItemId::Fern.get_default_type(), ItemType::Block(fern));
        assert_eq!(ItemId::Fern.category(), ItemId::TallGrass.category());
    }

    #[test]
    fn an_open_door_of_any_wood_is_walked_through_and_still_aimed_at() {
        for door in [
            BlockId::OpenDoor,
            BlockId::OpenSpruceDoorTop,
            BlockId::OpenAcaciaDoor,
        ] {
            assert!(matches!(door.get_hitbox(), BlockHitbox::None), "{door:?}");
            assert!(
                matches!(door.get_ray_hitbox(), BlockHitbox::Aabb(b) if b == OPEN_DOOR),
                "{door:?}"
            );
        }
        for door in [BlockId::Door, BlockId::SpruceDoor] {
            assert!(
                matches!(door.get_hitbox(), BlockHitbox::Aabb(b) if b == DOOR),
                "{door:?}"
            );
        }
    }

    #[test]
    fn the_vine_is_a_decoration_without_collision_broken_at_once() {
        let vine = BlockId::Vine;
        assert!(matches!(vine.get_hitbox(), BlockHitbox::None));
        assert!(matches!(vine.get_ray_hitbox(), BlockHitbox::Aabb(_)));
        assert_eq!(vine.get_break_time(), BlockId::TallGrass.get_break_time());
        assert_eq!(vine.get_required_tool(), None);
        assert_eq!(vine.get_visibility(), BlockTransparency::Decoration);
        assert_eq!(vine.get_drops(1), HashMap::from([(ItemId::Vine, 1)]));
        assert_eq!(ItemId::Vine.get_default_type(), ItemType::Block(vine));
        assert_eq!(vine.light_opacity(), BlockId::TallGrass.light_opacity());
        assert_eq!(ItemId::Vine.category(), ItemId::TallGrass.category());
    }

    #[test]
    fn the_cocoa_is_a_pod_broken_in_two_steps_for_two_beans() {
        let cocoa = BlockId::Cocoa;
        assert_eq!(cocoa.get_visibility(), BlockTransparency::Decoration);
        assert_eq!(cocoa.get_break_time(), Some(2 * 2));
        assert_eq!(cocoa.get_required_tool(), None);
        assert!(cocoa.is_mined_faster_by(ToolKind::Axe));
        assert_eq!(cocoa.get_drops(1), HashMap::from([(ItemId::CocoaBeans, 2)]));
        assert_eq!(ItemId::CocoaBeans.get_default_type(), ItemType::Generic);
        assert_eq!(ItemId::CocoaBeans.get_max_stack(), 64);
        assert_eq!(ItemId::CocoaBeans.category(), ItemId::Coal.category());
        // the light goes through it, as through a plant
        assert_eq!(cocoa.light_opacity(), BlockId::TallGrass.light_opacity());
    }

    #[test]
    fn a_cocoa_pod_is_boxed_where_it_hangs() {
        for direction in [
            BlockDirection::Front,
            BlockDirection::Right,
            BlockDirection::Back,
            BlockDirection::Left,
        ] {
            let pod = BlockData::new(BlockId::Cocoa, direction);
            let (BlockHitbox::Aabb(hitbox), BlockHitbox::Aabb(ray_hitbox)) =
                (pod.get_hitbox(), pod.get_ray_hitbox())
            else {
                panic!("{direction:?}: a pod has a box");
            };
            assert_eq!(hitbox, ray_hitbox, "{direction:?}");
            // the support lies on the side `toward` steps to, `along` runs beside it
            let toward = Vec3A::from(direction.offset().as_vec3());
            let along = Vec3A::new(toward.z, 0.0, toward.x);
            let size = hitbox.max - hitbox.min;
            let sixteenths = |length: f32| (length * 16.0 * 1000.0).round() / 1000.0;
            assert_eq!(sixteenths(size.dot(along).abs()), 8.0, "{direction:?}");
            assert_eq!(sixteenths(size.y), 9.0, "{direction:?}");
            assert_eq!(sixteenths(size.dot(toward).abs()), 8.0, "{direction:?}");
            assert_eq!(sixteenths(hitbox.min.y), 3.0, "{direction:?}");
            // centred along its support, 1/16 off it
            let centre = (hitbox.min + hitbox.max) / 2.0 - 0.5;
            assert!(centre.dot(along).abs() < 1e-6, "{direction:?}");
            let near_side = 0.5 - (centre.dot(toward) + size.dot(toward).abs() / 2.0);
            assert_eq!(sixteenths(near_side), 1.0, "{direction:?}");
        }
    }

    #[test]
    fn a_direction_names_the_side_of_its_step() {
        let sides = [
            (BlockDirection::Front, IVec3::NEG_Z),
            (BlockDirection::Back, IVec3::Z),
            (BlockDirection::Left, IVec3::NEG_X),
            (BlockDirection::Right, IVec3::X),
        ];
        for (direction, step) in sides {
            assert_eq!(direction.offset(), step, "{direction:?}");
            let turned = direction.turn([0.0, -1.0]).map(|c| c as i32);
            assert_eq!(turned, [step.x, step.z], "the front turned {direction:?}");
            assert_eq!(BlockDirection::toward(step), Some(direction), "{step}");
        }
        for step in [
            IVec3::ZERO,
            IVec3::Y,
            IVec3::NEG_Y,
            IVec3::new(1, 0, 1),
            IVec3::X * 2,
        ] {
            assert_eq!(BlockDirection::toward(step), None, "{step}");
        }
    }

    #[test]
    fn coal_ore_drops_coal_and_the_metal_ores_drop_themselves() {
        assert_eq!(
            BlockId::CoalOre.get_drops(1),
            HashMap::from([(ItemId::Coal, 1)])
        );
        assert_eq!(ItemId::Coal.get_default_type(), ItemType::Generic);
        for (block, item) in [
            (BlockId::IronOre, ItemId::IronOre),
            (BlockId::GoldOre, ItemId::GoldOre),
        ] {
            assert_eq!(block.get_drops(1), HashMap::from([(item, 1)]));
        }
        for (block, item) in [
            (BlockId::CoalOre, ItemId::CoalOre),
            (BlockId::IronOre, ItemId::IronOre),
            (BlockId::GoldOre, ItemId::GoldOre),
        ] {
            assert_eq!(item.get_default_type(), ItemType::Block(block));
            let stone = BlockId::Stone.get_break_time().unwrap();
            assert!(block.get_break_time().unwrap() > stone, "{block:?}");
        }
    }

    #[test]
    fn block_direction_encoding_is_pinned() {
        for direction in [
            BlockDirection::Front,
            BlockDirection::Right,
            BlockDirection::Back,
            BlockDirection::Left,
        ] {
            let expected: u8 = match direction {
                BlockDirection::Front => 0,
                BlockDirection::Right => 1,
                BlockDirection::Back => 2,
                BlockDirection::Left => 3,
            };
            assert_eq!(
                encoded(&direction),
                [expected],
                "{direction:?}: {ENCODING_WARNING}"
            );
        }
    }

    #[test]
    fn block_data_encoding_is_pinned() {
        let block = BlockData {
            breaking_progress: 3,
            level: 5,
            ..BlockData::new(BlockId::Stone, BlockDirection::Right)
        };
        assert_eq!(
            encoded(&block),
            [3, 1, 3, 5],
            "the fields of BlockData changed: bump PROTOCOL_ID and FORMAT_VERSION, and convert \
            the chunk files of the previous version"
        );
    }

    #[test]
    fn get_breaking_level_stays_within_zero_to_nine() {
        for id in regular_blocks() {
            let break_time = id.get_break_time().unwrap();
            for progress in 0..=break_time {
                let mut block = BlockData::new(id, BlockDirection::Front);
                block.breaking_progress = progress;
                assert!(
                    block.get_breaking_level() <= MAX_BREAKING_LEVEL,
                    "{id:?} at {progress}"
                );
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
        assert_eq!(block.get_breaking_level(), MAX_BREAKING_LEVEL);
        block.breaking_progress = u8::MAX;
        assert_eq!(block.get_breaking_level(), MAX_BREAKING_LEVEL);
    }

    #[test]
    fn get_breaking_level_does_not_panic_for_any_block() {
        for id in ALL_BLOCKS {
            let block = BlockData::new(id, BlockDirection::Front);
            assert!(block.get_breaking_level() <= MAX_BREAKING_LEVEL, "{id:?}");
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

    #[test]
    fn a_roll_picks_exactly_one_entry_in_proportion_to_its_chance() {
        let table = [(1, ItemId::Dirt, 1), (3, ItemId::Sand, 2)];
        assert_eq!(pick_drop(&table, 0), Some((ItemId::Dirt, 1)));
        for roll in 1..4 {
            assert_eq!(pick_drop(&table, roll), Some((ItemId::Sand, 2)), "{roll}");
        }
        assert_eq!(pick_drop(&table, 4), None);
        assert_eq!(pick_drop(&[], 0), None);
    }

    #[test]
    fn a_block_without_drop_table_drops_nothing() {
        assert!(BlockId::Water.get_drops(5).is_empty());
        assert_eq!(BlockId::Snow.get_drops(2)[&ItemId::Snowball], 8);
    }

    #[test]
    fn stone_and_metal_blocks_need_a_pickaxe_of_their_level_and_the_others_nothing() {
        for id in ALL_BLOCKS {
            // a block of the palette needs what the block it is made of needs
            if let Some(palette) = id
                .palette()
                .filter(|_| matches!(id, crate::new_palette_blocks!()))
            {
                assert_eq!(
                    id.get_required_tool(),
                    palette.base.get_required_tool(),
                    "{id:?}"
                );
                continue;
            }
            let level = match id {
                BlockId::Stone
                | BlockId::Cobblestone
                | BlockId::Gloomrock
                | BlockId::CobbledGloomrock
                | BlockId::Furnace
                | BlockId::LitFurnace
                | BlockId::CoalOre
                | BlockId::HewnStone
                | BlockId::Limestone
                | BlockId::Slate
                | BlockId::BrickWall
                | BlockId::CoalBlock
                | BlockId::CopperOre
                | BlockId::CopperBlock
                | BlockId::HewnStoneSlab
                | BlockId::BrickSlab
                | BlockId::HewnStoneStairs
                | BlockId::BrickStairs
                | BlockId::EngravingTable
                | BlockId::Anvil
                | BlockId::Geodestone
                | BlockId::Basalt
                | BlockId::PolishedBasalt
                | BlockId::Magma
                | BlockId::Alembic => Some(0),
                BlockId::IronOre
                | BlockId::IronBlock
                | BlockId::TinOre
                | BlockId::TinBlock
                | BlockId::BronzeBlock => Some(1),
                BlockId::GoldOre
                | BlockId::DiamondOre
                | BlockId::GoldBlock
                | BlockId::DiamondBlock => Some(2),
                BlockId::Obsidian => Some(3),
                _ => None,
            };
            let expected = level.map(|level| RequiredTool {
                kind: ToolKind::Pickaxe,
                level,
            });
            assert_eq!(id.get_required_tool(), expected, "{id:?}");
        }
    }

    #[test]
    fn a_block_that_needs_a_tool_is_mined_faster_by_it() {
        for id in ALL_BLOCKS {
            if let Some(required) = id.get_required_tool() {
                assert!(id.is_mined_faster_by(required.kind), "{id:?}");
            }
        }
    }

    #[test]
    fn a_block_is_harvested_by_a_tool_of_its_kind_and_level() {
        for (block, held, harvested) in [
            (BlockId::Stone, None, false),
            (BlockId::Stone, Some(ItemId::WoodenPickaxe), true),
            (BlockId::Stone, Some(ItemId::GoldenPickaxe), true),
            (BlockId::Stone, Some(ItemId::DiamondAxe), false),
            (BlockId::IronOre, Some(ItemId::WoodenPickaxe), false),
            (BlockId::IronOre, Some(ItemId::GoldenPickaxe), false),
            (BlockId::IronOre, Some(ItemId::StonePickaxe), true),
            (BlockId::IronBlock, Some(ItemId::StonePickaxe), true),
            (BlockId::GoldOre, Some(ItemId::StonePickaxe), false),
            (BlockId::GoldOre, Some(ItemId::IronPickaxe), true),
            (BlockId::DiamondOre, Some(ItemId::StonePickaxe), false),
            (BlockId::DiamondOre, Some(ItemId::IronPickaxe), true),
            (BlockId::DiamondOre, Some(ItemId::DiamondPickaxe), true),
            (BlockId::GoldBlock, Some(ItemId::GoldenPickaxe), false),
            (BlockId::DiamondBlock, Some(ItemId::IronPickaxe), true),
            (BlockId::Dirt, None, true),
            (BlockId::Dirt, Some(ItemId::WoodenSword), true),
        ] {
            let tool = held.and_then(|item| item.tool());
            assert_eq!(
                block.is_harvested_by(tool),
                harvested,
                "{block:?} with {held:?}"
            );
        }
    }

    #[test]
    fn diamond_ore_drops_a_diamond_and_the_storage_blocks_themselves() {
        assert_eq!(
            BlockId::DiamondOre.get_drops(1),
            HashMap::from([(ItemId::Diamond, 1)])
        );
        for (block, item) in [
            (BlockId::IronBlock, ItemId::IronBlock),
            (BlockId::GoldBlock, ItemId::GoldBlock),
            (BlockId::DiamondBlock, ItemId::DiamondBlock),
        ] {
            assert_eq!(block.get_drops(1), HashMap::from([(item, 1)]), "{block:?}");
        }
    }

    #[test]
    fn diamond_ore_and_the_storage_blocks_break_by_minecraft_s_hardness() {
        // Hardness 3, as the other ores, then 3 for gold and 5 for iron and diamond
        assert_eq!(
            BlockId::DiamondOre.get_break_time(),
            BlockId::GoldOre.get_break_time()
        );
        assert_eq!(BlockId::GoldBlock.get_break_time(), Some(2 * 15));
        assert_eq!(BlockId::IronBlock.get_break_time(), Some(2 * 25));
        assert_eq!(BlockId::DiamondBlock.get_break_time(), Some(2 * 25));
        for id in [
            BlockId::DiamondOre,
            BlockId::IronBlock,
            BlockId::GoldBlock,
            BlockId::DiamondBlock,
        ] {
            assert!(id.is_mined_faster_by(ToolKind::Pickaxe), "{id:?}");
            assert_eq!(id.get_visibility(), BlockTransparency::Solid, "{id:?}");
            assert!(!id.is_usable(), "{id:?}");
        }
    }

    #[test]
    fn each_tool_is_made_for_its_own_blocks() {
        assert!(BlockId::Stone.is_mined_faster_by(ToolKind::Pickaxe));
        assert!(BlockId::OakLog.is_mined_faster_by(ToolKind::Axe));
        assert!(BlockId::Grass.is_mined_faster_by(ToolKind::Shovel));
        assert!(BlockId::SpruceLeaves.is_mined_faster_by(ToolKind::Hoe));
        assert!(BlockId::OakLeaves.is_mined_faster_by(ToolKind::Sword));
        assert!(!BlockId::Stone.is_mined_faster_by(ToolKind::Axe));
        assert!(!BlockId::Dirt.is_mined_faster_by(ToolKind::Pickaxe));
        assert!(!BlockId::Glass.is_mined_faster_by(ToolKind::Pickaxe));
    }
}
