//! The furniture and the decoration of a home: bedrolls of every colour, a table and a chair of
//! each wood (a chair is sat on: `players::sitting`), bookshelves, carpets, flower pots, item
//! frames, paintings, banners and lanterns of every colour.
//!
//! Each of these blocks has a `Decor`: its kind, and a `base`, the plain block it breaks, sounds
//! and burns like. What a frame or a pot shows and the patterns of a banner are not in the
//! block: the server keeps them by position (`Decorations`) and sends them to every player.

use bevy::math::IVec3;
use serde::{Deserialize, Serialize};

use bevy::math::{bounding::Aabb3d, Vec3A};

use super::blocks::{BEDROLL, HANGING_LANTERN, LANTERN};
use super::{
    BlockDirection, BlockHitbox, BlockId, BlockTransparency, DyeColor, ItemId, ItemStack, ItemType,
    Wood,
};

/// What a block of the furniture is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecorKind {
    /// A bedroll of a colour: slept on as the first one.
    Bedroll,
    Table,
    /// Sat on by a right click; the sneak key stands up.
    Chair,
    Bookshelf,
    /// A thin rug on the floor of its cell.
    Carpet,
    /// Holds a plant (`ItemId::fits_flower_pot`).
    FlowerPot,
    /// Hung on a wall, it shows any item put in it.
    ItemFrame,
    /// A canvas hung on a wall, of one block or of two (`wide`): `right` is the right half of
    /// one of two, as seen from in front of it.
    Painting {
        wide: bool,
        right: bool,
    },
    /// Hung on a wall, with the patterns dyed on it.
    Banner,
    /// Of a colour, standing or hung under a block.
    Lantern {
        hanging: bool,
    },
}

/// The box from `a` to `b`, in sixteenths of a block.
const fn sixteenths(a: [f32; 3], b: [f32; 3]) -> Aabb3d {
    Aabb3d {
        min: Vec3A::new(a[0] / 16.0, a[1] / 16.0, a[2] / 16.0),
        max: Vec3A::new(b[0] / 16.0, b[1] / 16.0, b[2] / 16.0),
    }
}

/// The boxes of the furniture turned to the front, as `BlockData::turned` turns them: a chair's
/// back and what hangs on a wall are on the back, toward +Z for a chair, on the front, toward
/// -Z, for what hangs on a wall, as a vine.
pub const TABLE: Aabb3d = sixteenths([0.0, 0.0, 0.0], [16.0, 16.0, 16.0]);
pub const CHAIR_SEAT: Aabb3d = sixteenths([2.0, 0.0, 2.0], [14.0, 8.0, 14.0]);
pub const CHAIR: Aabb3d = sixteenths([2.0, 0.0, 2.0], [14.0, 16.0, 14.0]);
pub const CARPET: Aabb3d = sixteenths([0.0, 0.0, 0.0], [16.0, 1.0, 16.0]);
pub const FLOWER_POT: Aabb3d = sixteenths([5.0, 0.0, 5.0], [11.0, 6.0, 11.0]);
pub const ITEM_FRAME: Aabb3d = sixteenths([2.0, 2.0, 0.0], [14.0, 14.0, 1.0]);
pub const PAINTING: Aabb3d = sixteenths([0.0, 0.0, 0.0], [16.0, 16.0, 1.0]);
pub const BANNER: Aabb3d = sixteenths([1.0, 0.0, 0.0], [15.0, 15.0, 2.0]);
/// Where a seated player's hips are above the floor of a chair's cell, in blocks.
pub const SEAT_HEIGHT: f32 = 0.5;

impl DecorKind {
    /// Whether it is hung on a wall, toward the side its direction names, as a vine.
    pub fn on_wall(self) -> bool {
        matches!(self, Self::ItemFrame | Self::Painting { .. } | Self::Banner)
    }

    /// Whether it is turned toward the player who places it.
    pub fn faces_placer(self) -> bool {
        matches!(self, Self::Bedroll | Self::Chair)
    }

    /// Its collision box, turned to the front.
    pub fn hitbox(self) -> BlockHitbox {
        match self {
            Self::Bedroll => BlockHitbox::Aabb(BEDROLL),
            Self::Table => BlockHitbox::Aabb(TABLE),
            Self::Chair => BlockHitbox::Aabb(CHAIR_SEAT),
            Self::Bookshelf => BlockHitbox::FullBlock,
            Self::Carpet => BlockHitbox::Aabb(CARPET),
            Self::FlowerPot => BlockHitbox::Aabb(FLOWER_POT),
            Self::ItemFrame | Self::Painting { .. } | Self::Banner => BlockHitbox::None,
            Self::Lantern { hanging: false } => BlockHitbox::Aabb(LANTERN),
            Self::Lantern { hanging: true } => BlockHitbox::Aabb(HANGING_LANTERN),
        }
    }

    /// The box rays hit, turned to the front.
    pub fn ray_hitbox(self) -> BlockHitbox {
        match self {
            Self::Chair => BlockHitbox::Aabb(CHAIR),
            Self::ItemFrame => BlockHitbox::Aabb(ITEM_FRAME),
            Self::Painting { .. } => BlockHitbox::Aabb(PAINTING),
            Self::Banner => BlockHitbox::Aabb(BANNER),
            other => other.hitbox(),
        }
    }

    pub fn visibility(self) -> BlockTransparency {
        match self {
            Self::Bookshelf => BlockTransparency::Solid,
            _ => BlockTransparency::Decoration,
        }
    }

    /// Whether a right click uses it: a bedroll is slept on, a chair sat on, a pot and a frame
    /// take an item, and a canvas shows the next.
    pub fn is_usable(self) -> bool {
        matches!(
            self,
            Self::Bedroll | Self::Chair | Self::FlowerPot | Self::ItemFrame | Self::Painting { .. }
        )
    }
}

/// What a block of the furniture is: its kind, the plain block it is made of, and its item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Decor {
    pub kind: DecorKind,
    pub base: BlockId,
    pub item: ItemId,
}

impl DyeColor {
    /// Its bedroll: the red one is the first bedroll.
    pub fn bedroll(self) -> ItemId {
        match self {
            Self::White => ItemId::WhiteBedroll,
            Self::Orange => ItemId::OrangeBedroll,
            Self::Magenta => ItemId::MagentaBedroll,
            Self::LightBlue => ItemId::LightBlueBedroll,
            Self::Yellow => ItemId::YellowBedroll,
            Self::Lime => ItemId::LimeBedroll,
            Self::Pink => ItemId::PinkBedroll,
            Self::Gray => ItemId::GrayBedroll,
            Self::LightGray => ItemId::LightGrayBedroll,
            Self::Cyan => ItemId::CyanBedroll,
            Self::Purple => ItemId::PurpleBedroll,
            Self::Blue => ItemId::BlueBedroll,
            Self::Brown => ItemId::BrownBedroll,
            Self::Green => ItemId::GreenBedroll,
            Self::Red => ItemId::Bedroll,
            Self::Black => ItemId::BlackBedroll,
        }
    }
    /// Its carpet.
    pub fn carpet(self) -> ItemId {
        match self {
            Self::White => ItemId::WhiteCarpet,
            Self::Orange => ItemId::OrangeCarpet,
            Self::Magenta => ItemId::MagentaCarpet,
            Self::LightBlue => ItemId::LightBlueCarpet,
            Self::Yellow => ItemId::YellowCarpet,
            Self::Lime => ItemId::LimeCarpet,
            Self::Pink => ItemId::PinkCarpet,
            Self::Gray => ItemId::GrayCarpet,
            Self::LightGray => ItemId::LightGrayCarpet,
            Self::Cyan => ItemId::CyanCarpet,
            Self::Purple => ItemId::PurpleCarpet,
            Self::Blue => ItemId::BlueCarpet,
            Self::Brown => ItemId::BrownCarpet,
            Self::Green => ItemId::GreenCarpet,
            Self::Red => ItemId::RedCarpet,
            Self::Black => ItemId::BlackCarpet,
        }
    }
    /// Its banner.
    pub fn banner(self) -> ItemId {
        match self {
            Self::White => ItemId::WhiteBanner,
            Self::Orange => ItemId::OrangeBanner,
            Self::Magenta => ItemId::MagentaBanner,
            Self::LightBlue => ItemId::LightBlueBanner,
            Self::Yellow => ItemId::YellowBanner,
            Self::Lime => ItemId::LimeBanner,
            Self::Pink => ItemId::PinkBanner,
            Self::Gray => ItemId::GrayBanner,
            Self::LightGray => ItemId::LightGrayBanner,
            Self::Cyan => ItemId::CyanBanner,
            Self::Purple => ItemId::PurpleBanner,
            Self::Blue => ItemId::BlueBanner,
            Self::Brown => ItemId::BrownBanner,
            Self::Green => ItemId::GreenBanner,
            Self::Red => ItemId::RedBanner,
            Self::Black => ItemId::BlackBanner,
        }
    }
    /// Its lantern.
    pub fn lantern(self) -> ItemId {
        match self {
            Self::White => ItemId::WhiteLantern,
            Self::Orange => ItemId::OrangeLantern,
            Self::Magenta => ItemId::MagentaLantern,
            Self::LightBlue => ItemId::LightBlueLantern,
            Self::Yellow => ItemId::YellowLantern,
            Self::Lime => ItemId::LimeLantern,
            Self::Pink => ItemId::PinkLantern,
            Self::Gray => ItemId::GrayLantern,
            Self::LightGray => ItemId::LightGrayLantern,
            Self::Cyan => ItemId::CyanLantern,
            Self::Purple => ItemId::PurpleLantern,
            Self::Blue => ItemId::BlueLantern,
            Self::Brown => ItemId::BrownLantern,
            Self::Green => ItemId::GreenLantern,
            Self::Red => ItemId::RedLantern,
            Self::Black => ItemId::BlackLantern,
        }
    }
    /// The colour of the dye, as a texture of its wool shows it.
    pub fn rgb(self) -> [u8; 3] {
        match self {
            Self::White => [0xF9, 0xFF, 0xFE],
            Self::Orange => [0xF9, 0x80, 0x1D],
            Self::Magenta => [0xC7, 0x4E, 0xBD],
            Self::LightBlue => [0x3A, 0xB3, 0xDA],
            Self::Yellow => [0xFE, 0xD8, 0x3D],
            Self::Lime => [0x80, 0xC7, 0x1F],
            Self::Pink => [0xF3, 0x8B, 0xAA],
            Self::Gray => [0x47, 0x4F, 0x52],
            Self::LightGray => [0x9D, 0x9D, 0x97],
            Self::Cyan => [0x16, 0x9C, 0x9C],
            Self::Purple => [0x89, 0x32, 0xB8],
            Self::Blue => [0x3C, 0x44, 0xAA],
            Self::Brown => [0x83, 0x54, 0x32],
            Self::Green => [0x5E, 0x7C, 0x16],
            Self::Red => [0xB0, 0x2E, 0x26],
            Self::Black => [0x1D, 0x1D, 0x21],
        }
    }

    /// The colour of `dye`, if it is a dye.
    pub fn of_dye(dye: ItemId) -> Option<Self> {
        Self::ALL.into_iter().find(|color| color.dye() == dye)
    }
}

impl Wood {
    /// Its table.
    pub fn table(self) -> ItemId {
        match self {
            Self::Oak => ItemId::OakTable,
            Self::Spruce => ItemId::SpruceTable,
            Self::Jungle => ItemId::JungleTable,
            Self::Willow => ItemId::WillowTable,
            Self::Acacia => ItemId::AcaciaTable,
        }
    }

    /// Its chair.
    pub fn chair(self) -> ItemId {
        match self {
            Self::Oak => ItemId::OakChair,
            Self::Spruce => ItemId::SpruceChair,
            Self::Jungle => ItemId::JungleChair,
            Self::Willow => ItemId::WillowChair,
            Self::Acacia => ItemId::AcaciaChair,
        }
    }
}

/// The canvases of one block, in the order a click goes through them.
pub const SMALL_CANVASES: [BlockId; 4] = [
    BlockId::DawnPainting,
    BlockId::MeadowPainting,
    BlockId::HarbourPainting,
    BlockId::StillLifePainting,
];

/// The canvases of two blocks, their left half and their right half.
pub const WIDE_CANVASES: [(BlockId, BlockId); 3] = [
    (BlockId::ValleyPainting, BlockId::ValleyPaintingRight),
    (BlockId::SeascapePainting, BlockId::SeascapePaintingRight),
    (BlockId::ForestPainting, BlockId::ForestPaintingRight),
];

impl BlockId {
    /// What the block is, if it is one of the furniture.
    pub fn decor(self) -> Option<Decor> {
        let (kind, base, item) = match self {
            Self::WhiteBedroll => (DecorKind::Bedroll, Self::Wool, ItemId::WhiteBedroll),
            Self::OrangeBedroll => (DecorKind::Bedroll, Self::Wool, ItemId::OrangeBedroll),
            Self::MagentaBedroll => (DecorKind::Bedroll, Self::Wool, ItemId::MagentaBedroll),
            Self::LightBlueBedroll => (DecorKind::Bedroll, Self::Wool, ItemId::LightBlueBedroll),
            Self::YellowBedroll => (DecorKind::Bedroll, Self::Wool, ItemId::YellowBedroll),
            Self::LimeBedroll => (DecorKind::Bedroll, Self::Wool, ItemId::LimeBedroll),
            Self::PinkBedroll => (DecorKind::Bedroll, Self::Wool, ItemId::PinkBedroll),
            Self::GrayBedroll => (DecorKind::Bedroll, Self::Wool, ItemId::GrayBedroll),
            Self::LightGrayBedroll => (DecorKind::Bedroll, Self::Wool, ItemId::LightGrayBedroll),
            Self::CyanBedroll => (DecorKind::Bedroll, Self::Wool, ItemId::CyanBedroll),
            Self::PurpleBedroll => (DecorKind::Bedroll, Self::Wool, ItemId::PurpleBedroll),
            Self::BlueBedroll => (DecorKind::Bedroll, Self::Wool, ItemId::BlueBedroll),
            Self::BrownBedroll => (DecorKind::Bedroll, Self::Wool, ItemId::BrownBedroll),
            Self::GreenBedroll => (DecorKind::Bedroll, Self::Wool, ItemId::GreenBedroll),
            Self::BlackBedroll => (DecorKind::Bedroll, Self::Wool, ItemId::BlackBedroll),
            Self::OakTable => (DecorKind::Table, Self::OakPlanks, ItemId::OakTable),
            Self::SpruceTable => (DecorKind::Table, Self::SprucePlanks, ItemId::SpruceTable),
            Self::JungleTable => (DecorKind::Table, Self::JunglePlanks, ItemId::JungleTable),
            Self::WillowTable => (DecorKind::Table, Self::WillowPlanks, ItemId::WillowTable),
            Self::AcaciaTable => (DecorKind::Table, Self::AcaciaPlanks, ItemId::AcaciaTable),
            Self::OakChair => (DecorKind::Chair, Self::OakPlanks, ItemId::OakChair),
            Self::SpruceChair => (DecorKind::Chair, Self::SprucePlanks, ItemId::SpruceChair),
            Self::JungleChair => (DecorKind::Chair, Self::JunglePlanks, ItemId::JungleChair),
            Self::WillowChair => (DecorKind::Chair, Self::WillowPlanks, ItemId::WillowChair),
            Self::AcaciaChair => (DecorKind::Chair, Self::AcaciaPlanks, ItemId::AcaciaChair),
            Self::Bookshelf => (DecorKind::Bookshelf, Self::OakPlanks, ItemId::Bookshelf),
            Self::WhiteCarpet => (DecorKind::Carpet, Self::Wool, ItemId::WhiteCarpet),
            Self::OrangeCarpet => (DecorKind::Carpet, Self::OrangeWool, ItemId::OrangeCarpet),
            Self::MagentaCarpet => (DecorKind::Carpet, Self::MagentaWool, ItemId::MagentaCarpet),
            Self::LightBlueCarpet => (
                DecorKind::Carpet,
                Self::LightBlueWool,
                ItemId::LightBlueCarpet,
            ),
            Self::YellowCarpet => (DecorKind::Carpet, Self::YellowWool, ItemId::YellowCarpet),
            Self::LimeCarpet => (DecorKind::Carpet, Self::LimeWool, ItemId::LimeCarpet),
            Self::PinkCarpet => (DecorKind::Carpet, Self::PinkWool, ItemId::PinkCarpet),
            Self::GrayCarpet => (DecorKind::Carpet, Self::GrayWool, ItemId::GrayCarpet),
            Self::LightGrayCarpet => (
                DecorKind::Carpet,
                Self::LightGrayWool,
                ItemId::LightGrayCarpet,
            ),
            Self::CyanCarpet => (DecorKind::Carpet, Self::CyanWool, ItemId::CyanCarpet),
            Self::PurpleCarpet => (DecorKind::Carpet, Self::PurpleWool, ItemId::PurpleCarpet),
            Self::BlueCarpet => (DecorKind::Carpet, Self::BlueWool, ItemId::BlueCarpet),
            Self::BrownCarpet => (DecorKind::Carpet, Self::BrownWool, ItemId::BrownCarpet),
            Self::GreenCarpet => (DecorKind::Carpet, Self::GreenWool, ItemId::GreenCarpet),
            Self::RedCarpet => (DecorKind::Carpet, Self::RedWool, ItemId::RedCarpet),
            Self::BlackCarpet => (DecorKind::Carpet, Self::BlackWool, ItemId::BlackCarpet),
            Self::FlowerPot => (DecorKind::FlowerPot, Self::Clay, ItemId::FlowerPot),
            Self::ItemFrame => (DecorKind::ItemFrame, Self::OakPlanks, ItemId::ItemFrame),
            Self::DawnPainting => (
                DecorKind::Painting {
                    wide: false,
                    right: false,
                },
                Self::OakPlanks,
                ItemId::Painting,
            ),
            Self::MeadowPainting => (
                DecorKind::Painting {
                    wide: false,
                    right: false,
                },
                Self::OakPlanks,
                ItemId::Painting,
            ),
            Self::HarbourPainting => (
                DecorKind::Painting {
                    wide: false,
                    right: false,
                },
                Self::OakPlanks,
                ItemId::Painting,
            ),
            Self::StillLifePainting => (
                DecorKind::Painting {
                    wide: false,
                    right: false,
                },
                Self::OakPlanks,
                ItemId::Painting,
            ),
            Self::ValleyPainting => (
                DecorKind::Painting {
                    wide: true,
                    right: false,
                },
                Self::OakPlanks,
                ItemId::WidePainting,
            ),
            Self::ValleyPaintingRight => (
                DecorKind::Painting {
                    wide: true,
                    right: true,
                },
                Self::OakPlanks,
                ItemId::WidePainting,
            ),
            Self::SeascapePainting => (
                DecorKind::Painting {
                    wide: true,
                    right: false,
                },
                Self::OakPlanks,
                ItemId::WidePainting,
            ),
            Self::SeascapePaintingRight => (
                DecorKind::Painting {
                    wide: true,
                    right: true,
                },
                Self::OakPlanks,
                ItemId::WidePainting,
            ),
            Self::ForestPainting => (
                DecorKind::Painting {
                    wide: true,
                    right: false,
                },
                Self::OakPlanks,
                ItemId::WidePainting,
            ),
            Self::ForestPaintingRight => (
                DecorKind::Painting {
                    wide: true,
                    right: true,
                },
                Self::OakPlanks,
                ItemId::WidePainting,
            ),
            Self::WhiteBanner => (DecorKind::Banner, Self::Wool, ItemId::WhiteBanner),
            Self::OrangeBanner => (DecorKind::Banner, Self::OrangeWool, ItemId::OrangeBanner),
            Self::MagentaBanner => (DecorKind::Banner, Self::MagentaWool, ItemId::MagentaBanner),
            Self::LightBlueBanner => (
                DecorKind::Banner,
                Self::LightBlueWool,
                ItemId::LightBlueBanner,
            ),
            Self::YellowBanner => (DecorKind::Banner, Self::YellowWool, ItemId::YellowBanner),
            Self::LimeBanner => (DecorKind::Banner, Self::LimeWool, ItemId::LimeBanner),
            Self::PinkBanner => (DecorKind::Banner, Self::PinkWool, ItemId::PinkBanner),
            Self::GrayBanner => (DecorKind::Banner, Self::GrayWool, ItemId::GrayBanner),
            Self::LightGrayBanner => (
                DecorKind::Banner,
                Self::LightGrayWool,
                ItemId::LightGrayBanner,
            ),
            Self::CyanBanner => (DecorKind::Banner, Self::CyanWool, ItemId::CyanBanner),
            Self::PurpleBanner => (DecorKind::Banner, Self::PurpleWool, ItemId::PurpleBanner),
            Self::BlueBanner => (DecorKind::Banner, Self::BlueWool, ItemId::BlueBanner),
            Self::BrownBanner => (DecorKind::Banner, Self::BrownWool, ItemId::BrownBanner),
            Self::GreenBanner => (DecorKind::Banner, Self::GreenWool, ItemId::GreenBanner),
            Self::RedBanner => (DecorKind::Banner, Self::RedWool, ItemId::RedBanner),
            Self::BlackBanner => (DecorKind::Banner, Self::BlackWool, ItemId::BlackBanner),
            Self::WhiteLantern => (
                DecorKind::Lantern { hanging: false },
                Self::Lantern,
                ItemId::WhiteLantern,
            ),
            Self::OrangeLantern => (
                DecorKind::Lantern { hanging: false },
                Self::Lantern,
                ItemId::OrangeLantern,
            ),
            Self::MagentaLantern => (
                DecorKind::Lantern { hanging: false },
                Self::Lantern,
                ItemId::MagentaLantern,
            ),
            Self::LightBlueLantern => (
                DecorKind::Lantern { hanging: false },
                Self::Lantern,
                ItemId::LightBlueLantern,
            ),
            Self::YellowLantern => (
                DecorKind::Lantern { hanging: false },
                Self::Lantern,
                ItemId::YellowLantern,
            ),
            Self::LimeLantern => (
                DecorKind::Lantern { hanging: false },
                Self::Lantern,
                ItemId::LimeLantern,
            ),
            Self::PinkLantern => (
                DecorKind::Lantern { hanging: false },
                Self::Lantern,
                ItemId::PinkLantern,
            ),
            Self::GrayLantern => (
                DecorKind::Lantern { hanging: false },
                Self::Lantern,
                ItemId::GrayLantern,
            ),
            Self::LightGrayLantern => (
                DecorKind::Lantern { hanging: false },
                Self::Lantern,
                ItemId::LightGrayLantern,
            ),
            Self::CyanLantern => (
                DecorKind::Lantern { hanging: false },
                Self::Lantern,
                ItemId::CyanLantern,
            ),
            Self::PurpleLantern => (
                DecorKind::Lantern { hanging: false },
                Self::Lantern,
                ItemId::PurpleLantern,
            ),
            Self::BlueLantern => (
                DecorKind::Lantern { hanging: false },
                Self::Lantern,
                ItemId::BlueLantern,
            ),
            Self::BrownLantern => (
                DecorKind::Lantern { hanging: false },
                Self::Lantern,
                ItemId::BrownLantern,
            ),
            Self::GreenLantern => (
                DecorKind::Lantern { hanging: false },
                Self::Lantern,
                ItemId::GreenLantern,
            ),
            Self::RedLantern => (
                DecorKind::Lantern { hanging: false },
                Self::Lantern,
                ItemId::RedLantern,
            ),
            Self::BlackLantern => (
                DecorKind::Lantern { hanging: false },
                Self::Lantern,
                ItemId::BlackLantern,
            ),
            Self::HangingWhiteLantern => (
                DecorKind::Lantern { hanging: true },
                Self::Lantern,
                ItemId::WhiteLantern,
            ),
            Self::HangingOrangeLantern => (
                DecorKind::Lantern { hanging: true },
                Self::Lantern,
                ItemId::OrangeLantern,
            ),
            Self::HangingMagentaLantern => (
                DecorKind::Lantern { hanging: true },
                Self::Lantern,
                ItemId::MagentaLantern,
            ),
            Self::HangingLightBlueLantern => (
                DecorKind::Lantern { hanging: true },
                Self::Lantern,
                ItemId::LightBlueLantern,
            ),
            Self::HangingYellowLantern => (
                DecorKind::Lantern { hanging: true },
                Self::Lantern,
                ItemId::YellowLantern,
            ),
            Self::HangingLimeLantern => (
                DecorKind::Lantern { hanging: true },
                Self::Lantern,
                ItemId::LimeLantern,
            ),
            Self::HangingPinkLantern => (
                DecorKind::Lantern { hanging: true },
                Self::Lantern,
                ItemId::PinkLantern,
            ),
            Self::HangingGrayLantern => (
                DecorKind::Lantern { hanging: true },
                Self::Lantern,
                ItemId::GrayLantern,
            ),
            Self::HangingLightGrayLantern => (
                DecorKind::Lantern { hanging: true },
                Self::Lantern,
                ItemId::LightGrayLantern,
            ),
            Self::HangingCyanLantern => (
                DecorKind::Lantern { hanging: true },
                Self::Lantern,
                ItemId::CyanLantern,
            ),
            Self::HangingPurpleLantern => (
                DecorKind::Lantern { hanging: true },
                Self::Lantern,
                ItemId::PurpleLantern,
            ),
            Self::HangingBlueLantern => (
                DecorKind::Lantern { hanging: true },
                Self::Lantern,
                ItemId::BlueLantern,
            ),
            Self::HangingBrownLantern => (
                DecorKind::Lantern { hanging: true },
                Self::Lantern,
                ItemId::BrownLantern,
            ),
            Self::HangingGreenLantern => (
                DecorKind::Lantern { hanging: true },
                Self::Lantern,
                ItemId::GreenLantern,
            ),
            Self::HangingRedLantern => (
                DecorKind::Lantern { hanging: true },
                Self::Lantern,
                ItemId::RedLantern,
            ),
            Self::HangingBlackLantern => (
                DecorKind::Lantern { hanging: true },
                Self::Lantern,
                ItemId::BlackLantern,
            ),
            _ => return None,
        };
        Some(Decor { kind, base, item })
    }

    /// Whether it is a bedroll, the first one or one of another colour: a player sleeps on it.
    pub fn is_bedroll(self) -> bool {
        self == Self::Bedroll
            || self
                .decor()
                .is_some_and(|decor| decor.kind == DecorKind::Bedroll)
    }

    /// The kind of the block, if it is one of the furniture.
    pub fn decor_kind(self) -> Option<DecorKind> {
        self.decor().map(|decor| decor.kind)
    }

    /// For a standing lantern of a colour: the same, hung under a block.
    pub fn hanging_lantern(self) -> Option<BlockId> {
        match self {
            Self::WhiteLantern => Some(Self::HangingWhiteLantern),
            Self::OrangeLantern => Some(Self::HangingOrangeLantern),
            Self::MagentaLantern => Some(Self::HangingMagentaLantern),
            Self::LightBlueLantern => Some(Self::HangingLightBlueLantern),
            Self::YellowLantern => Some(Self::HangingYellowLantern),
            Self::LimeLantern => Some(Self::HangingLimeLantern),
            Self::PinkLantern => Some(Self::HangingPinkLantern),
            Self::GrayLantern => Some(Self::HangingGrayLantern),
            Self::LightGrayLantern => Some(Self::HangingLightGrayLantern),
            Self::CyanLantern => Some(Self::HangingCyanLantern),
            Self::PurpleLantern => Some(Self::HangingPurpleLantern),
            Self::BlueLantern => Some(Self::HangingBlueLantern),
            Self::BrownLantern => Some(Self::HangingBrownLantern),
            Self::GreenLantern => Some(Self::HangingGreenLantern),
            Self::RedLantern => Some(Self::HangingRedLantern),
            Self::BlackLantern => Some(Self::HangingBlackLantern),
            _ => None,
        }
    }

    /// For a canvas: the next one of its size that a click shows, both halves for one of two
    /// blocks (the left one first).
    pub fn next_canvas(self) -> Option<(BlockId, Option<BlockId>)> {
        if let Some(index) = SMALL_CANVASES.iter().position(|&canvas| canvas == self) {
            return Some((SMALL_CANVASES[(index + 1) % SMALL_CANVASES.len()], None));
        }
        let index = WIDE_CANVASES
            .iter()
            .position(|&(left, right)| left == self || right == self)?;
        let (left, right) = WIDE_CANVASES[(index + 1) % WIDE_CANVASES.len()];
        Some((left, Some(right)))
    }

    /// For a half of a canvas of two blocks: the other half, and the step to it from this one,
    /// turned to `direction`, the side of the wall it hangs on.
    pub fn other_painting_half(self, direction: BlockDirection) -> Option<(BlockId, IVec3)> {
        let right = painting_right(direction);
        WIDE_CANVASES.iter().find_map(|&(l, r)| {
            if self == l {
                Some((r, right))
            } else if self == r {
                Some((l, -right))
            } else {
                None
            }
        })
    }
}

/// The step to the right of a canvas hung on the side `direction` of its cell, as seen by who
/// faces it: looking toward the wall.
pub fn painting_right(direction: BlockDirection) -> IVec3 {
    let toward = direction.offset();
    IVec3::new(-toward.z, 0, toward.x)
}

/// The heading of a player sitting on a chair turned to `direction`, as `riding::Riding::heading`:
/// a chair's back is on the back of its cell, so its sitter looks toward its front.
pub fn chair_heading(direction: BlockDirection) -> f32 {
    let front = direction.offset().as_vec3();
    front.x.atan2(front.z)
}

impl ItemId {
    /// The block a new item of the furniture places, `None` for the book and the other items.
    /// A painting places the first canvas of its size, then shows the next at each click.
    pub fn decor_block(self) -> Option<BlockId> {
        match self {
            Self::WhiteBedroll => Some(BlockId::WhiteBedroll),
            Self::OrangeBedroll => Some(BlockId::OrangeBedroll),
            Self::MagentaBedroll => Some(BlockId::MagentaBedroll),
            Self::LightBlueBedroll => Some(BlockId::LightBlueBedroll),
            Self::YellowBedroll => Some(BlockId::YellowBedroll),
            Self::LimeBedroll => Some(BlockId::LimeBedroll),
            Self::PinkBedroll => Some(BlockId::PinkBedroll),
            Self::GrayBedroll => Some(BlockId::GrayBedroll),
            Self::LightGrayBedroll => Some(BlockId::LightGrayBedroll),
            Self::CyanBedroll => Some(BlockId::CyanBedroll),
            Self::PurpleBedroll => Some(BlockId::PurpleBedroll),
            Self::BlueBedroll => Some(BlockId::BlueBedroll),
            Self::BrownBedroll => Some(BlockId::BrownBedroll),
            Self::GreenBedroll => Some(BlockId::GreenBedroll),
            Self::BlackBedroll => Some(BlockId::BlackBedroll),
            Self::OakTable => Some(BlockId::OakTable),
            Self::OakChair => Some(BlockId::OakChair),
            Self::SpruceTable => Some(BlockId::SpruceTable),
            Self::SpruceChair => Some(BlockId::SpruceChair),
            Self::JungleTable => Some(BlockId::JungleTable),
            Self::JungleChair => Some(BlockId::JungleChair),
            Self::WillowTable => Some(BlockId::WillowTable),
            Self::WillowChair => Some(BlockId::WillowChair),
            Self::AcaciaTable => Some(BlockId::AcaciaTable),
            Self::AcaciaChair => Some(BlockId::AcaciaChair),
            Self::Bookshelf => Some(BlockId::Bookshelf),
            Self::WhiteCarpet => Some(BlockId::WhiteCarpet),
            Self::OrangeCarpet => Some(BlockId::OrangeCarpet),
            Self::MagentaCarpet => Some(BlockId::MagentaCarpet),
            Self::LightBlueCarpet => Some(BlockId::LightBlueCarpet),
            Self::YellowCarpet => Some(BlockId::YellowCarpet),
            Self::LimeCarpet => Some(BlockId::LimeCarpet),
            Self::PinkCarpet => Some(BlockId::PinkCarpet),
            Self::GrayCarpet => Some(BlockId::GrayCarpet),
            Self::LightGrayCarpet => Some(BlockId::LightGrayCarpet),
            Self::CyanCarpet => Some(BlockId::CyanCarpet),
            Self::PurpleCarpet => Some(BlockId::PurpleCarpet),
            Self::BlueCarpet => Some(BlockId::BlueCarpet),
            Self::BrownCarpet => Some(BlockId::BrownCarpet),
            Self::GreenCarpet => Some(BlockId::GreenCarpet),
            Self::RedCarpet => Some(BlockId::RedCarpet),
            Self::BlackCarpet => Some(BlockId::BlackCarpet),
            Self::FlowerPot => Some(BlockId::FlowerPot),
            Self::ItemFrame => Some(BlockId::ItemFrame),
            Self::Painting => Some(BlockId::DawnPainting),
            Self::WidePainting => Some(BlockId::ValleyPainting),
            Self::WhiteBanner => Some(BlockId::WhiteBanner),
            Self::OrangeBanner => Some(BlockId::OrangeBanner),
            Self::MagentaBanner => Some(BlockId::MagentaBanner),
            Self::LightBlueBanner => Some(BlockId::LightBlueBanner),
            Self::YellowBanner => Some(BlockId::YellowBanner),
            Self::LimeBanner => Some(BlockId::LimeBanner),
            Self::PinkBanner => Some(BlockId::PinkBanner),
            Self::GrayBanner => Some(BlockId::GrayBanner),
            Self::LightGrayBanner => Some(BlockId::LightGrayBanner),
            Self::CyanBanner => Some(BlockId::CyanBanner),
            Self::PurpleBanner => Some(BlockId::PurpleBanner),
            Self::BlueBanner => Some(BlockId::BlueBanner),
            Self::BrownBanner => Some(BlockId::BrownBanner),
            Self::GreenBanner => Some(BlockId::GreenBanner),
            Self::RedBanner => Some(BlockId::RedBanner),
            Self::BlackBanner => Some(BlockId::BlackBanner),
            Self::WhiteLantern => Some(BlockId::WhiteLantern),
            Self::OrangeLantern => Some(BlockId::OrangeLantern),
            Self::MagentaLantern => Some(BlockId::MagentaLantern),
            Self::LightBlueLantern => Some(BlockId::LightBlueLantern),
            Self::YellowLantern => Some(BlockId::YellowLantern),
            Self::LimeLantern => Some(BlockId::LimeLantern),
            Self::PinkLantern => Some(BlockId::PinkLantern),
            Self::GrayLantern => Some(BlockId::GrayLantern),
            Self::LightGrayLantern => Some(BlockId::LightGrayLantern),
            Self::CyanLantern => Some(BlockId::CyanLantern),
            Self::PurpleLantern => Some(BlockId::PurpleLantern),
            Self::BlueLantern => Some(BlockId::BlueLantern),
            Self::BrownLantern => Some(BlockId::BrownLantern),
            Self::GreenLantern => Some(BlockId::GreenLantern),
            Self::RedLantern => Some(BlockId::RedLantern),
            Self::BlackLantern => Some(BlockId::BlackLantern),
            _ => None,
        }
    }

    /// Whether the item is a plant that a flower pot holds.
    pub fn fits_flower_pot(self) -> bool {
        matches!(
            self,
            Self::Dandelion
                | Self::Poppy
                | Self::Heather
                | Self::Fern
                | Self::Mushroom
                | Self::Cactus
                | Self::OakSapling
                | Self::SpruceSapling
                | Self::JungleSapling
                | Self::WillowSapling
                | Self::AcaciaSapling
                | Self::AppleSapling
        )
    }

    /// The colour of a banner.
    pub fn banner_color(self) -> Option<DyeColor> {
        DyeColor::ALL
            .into_iter()
            .find(|color| color.banner() == self)
    }
}

/// The patterns a banner holds at most, dyed one over the other.
pub const MAX_BANNER_LAYERS: usize = 6;

/// A pattern dyed on a banner, at a crafting grid: the dyes laid in its shape, the banner in any
/// other cell (`BannerPattern::shape`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BannerPattern {
    /// A stripe across the top.
    Chief,
    /// A stripe across the bottom.
    Base,
    /// A stripe across the middle.
    Fess,
    /// A stripe down the middle.
    Pale,
    /// A stripe from the top left to the bottom right.
    Bend,
    /// A stripe from the top right to the bottom left.
    BendSinister,
    Cross,
    Saltire,
    Border,
    /// A disc in the middle.
    Roundel,
}

impl BannerPattern {
    pub const ALL: [BannerPattern; 10] = [
        Self::Chief,
        Self::Base,
        Self::Fess,
        Self::Pale,
        Self::Bend,
        Self::BendSinister,
        Self::Cross,
        Self::Saltire,
        Self::Border,
        Self::Roundel,
    ];

    /// The cells of a 3 by 3 grid the dyes are laid in, row after row.
    pub fn shape(self) -> &'static [usize] {
        match self {
            Self::Chief => &[0, 1, 2],
            Self::Base => &[6, 7, 8],
            Self::Fess => &[3, 4, 5],
            Self::Pale => &[1, 4, 7],
            Self::Bend => &[0, 4, 8],
            Self::BendSinister => &[2, 4, 6],
            Self::Cross => &[1, 3, 4, 5, 7],
            Self::Saltire => &[0, 2, 4, 6, 8],
            Self::Border => &[0, 1, 2, 3, 5, 6, 7, 8],
            Self::Roundel => &[4],
        }
    }

    /// The pattern whose dyes lie in exactly the cells `dyed` of a 3 by 3 grid.
    pub fn of_cells(dyed: &[usize]) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|pattern| pattern.shape() == dyed)
    }

    /// Whether the pixel at column `x` and row `y` (from the top) of a cloth `width` by
    /// `height` pixels is of the pattern.
    pub fn covers(self, x: u32, y: u32, width: u32, height: u32) -> bool {
        let (fx, fy) = (
            (x as f32 + 0.5) / width as f32,
            (y as f32 + 0.5) / height as f32,
        );
        let near = |a: f32, b: f32, reach: f32| (a - b).abs() < reach;
        match self {
            Self::Chief => fy < 1.0 / 3.0,
            Self::Base => fy > 2.0 / 3.0,
            Self::Fess => (1.0 / 3.0..2.0 / 3.0).contains(&fy),
            Self::Pale => (1.0 / 3.0..2.0 / 3.0).contains(&fx),
            Self::Bend => near(fx, fy, 0.17),
            Self::BendSinister => near(1.0 - fx, fy, 0.17),
            Self::Cross => near(fx, 0.5, 0.13) || near(fy, 0.5, 0.11),
            Self::Saltire => near(fx, fy, 0.11) || near(1.0 - fx, fy, 0.11),
            Self::Border => x < 2 || y < 2 || x + 2 >= width || y + 2 >= height,
            Self::Roundel => {
                let (dx, dy) = ((fx - 0.5) * width as f32, (fy - 0.5) * height as f32);
                dx * dx + dy * dy <= (width as f32 / 4.0 + 0.5).powi(2)
            }
        }
    }
}

/// The patterns dyed on a banner, the first under the others: each layer is the number of its
/// pattern from 1 in its high four bits and its colour in its low four, 0 for none.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct BannerPatterns(pub [u8; MAX_BANNER_LAYERS]);

impl BannerPatterns {
    /// The layers, from the first dyed.
    pub fn layers(&self) -> impl Iterator<Item = (BannerPattern, DyeColor)> + '_ {
        self.0
            .iter()
            .filter(|&&layer| layer != 0)
            .filter_map(|&layer| {
                let pattern = *BannerPattern::ALL.get(usize::from(layer >> 4).checked_sub(1)?)?;
                Some((pattern, DyeColor::ALL[usize::from(layer & 0xF)]))
            })
    }

    pub fn is_empty(&self) -> bool {
        self.layers().next().is_none()
    }

    /// The patterns with `pattern` dyed `color` over them, `None` once they are as many as
    /// `MAX_BANNER_LAYERS`.
    pub fn with(mut self, pattern: BannerPattern, color: DyeColor) -> Option<Self> {
        let free = self.0.iter_mut().find(|layer| **layer == 0)?;
        let number = BannerPattern::ALL.iter().position(|&p| p == pattern)? as u8 + 1;
        let color = DyeColor::ALL.iter().position(|&c| c == color)? as u8;
        *free = number << 4 | color;
        Some(self)
    }
}

/// What a crafting grid of 3 by 3 cells, row after row, makes of a banner and dyes of one
/// colour laid in the shape of a pattern (`BannerPattern::shape`): the banner with the pattern
/// dyed over its others. `None` for any other grid, or a banner that holds as many patterns as
/// it can.
pub fn dye_banner(cells: &[Option<ItemStack>]) -> Option<ItemStack> {
    let filled: Vec<(usize, ItemStack)> = cells
        .iter()
        .enumerate()
        .filter_map(|(index, cell)| cell.map(|stack| (index, stack)))
        .collect();
    let mut banners = filled
        .iter()
        .filter(|(_, stack)| stack.item_id.banner_color().is_some());
    let &(banner_cell, banner) = banners.next()?;
    if banners.next().is_some() {
        return None;
    }
    let dyed: Vec<usize> = filled
        .iter()
        .map(|&(index, _)| index)
        .filter(|&index| index != banner_cell)
        .collect();
    let mut colors = filled
        .iter()
        .filter(|&&(index, _)| index != banner_cell)
        .map(|(_, stack)| DyeColor::of_dye(stack.item_id));
    let color = colors.next()??;
    if !colors.all(|other| other == Some(color)) {
        return None;
    }
    let pattern = BannerPattern::of_cells(&dyed)?;
    let patterns = match banner.item_type {
        ItemType::Banner { patterns } => patterns,
        _ => BannerPatterns::default(),
    };
    Some(ItemStack {
        item_id: banner.item_id,
        item_type: ItemType::Banner {
            patterns: patterns.with(pattern, color)?,
        },
        nb: 1,
    })
}

/// What a frame or a pot shows: an item, and for a frame how many eighths of a turn it is
/// turned.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Shown {
    pub item: ItemId,
    pub turn: u8,
}

/// What the item frames and the flower pots of the world show and the patterns of its banners,
/// which the server sends to every player when they change.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct Decorations {
    pub displays: Vec<(IVec3, Shown)>,
    pub banners: Vec<(IVec3, BannerPatterns)>,
}

/// The blocks added with the furniture, as a pattern: for the matches over every block.
#[macro_export]
macro_rules! new_decor_blocks {
    () => {
        $crate::world::BlockId::WhiteBedroll
            | $crate::world::BlockId::OrangeBedroll
            | $crate::world::BlockId::MagentaBedroll
            | $crate::world::BlockId::LightBlueBedroll
            | $crate::world::BlockId::YellowBedroll
            | $crate::world::BlockId::LimeBedroll
            | $crate::world::BlockId::PinkBedroll
            | $crate::world::BlockId::GrayBedroll
            | $crate::world::BlockId::LightGrayBedroll
            | $crate::world::BlockId::CyanBedroll
            | $crate::world::BlockId::PurpleBedroll
            | $crate::world::BlockId::BlueBedroll
            | $crate::world::BlockId::BrownBedroll
            | $crate::world::BlockId::GreenBedroll
            | $crate::world::BlockId::BlackBedroll
            | $crate::world::BlockId::OakTable
            | $crate::world::BlockId::SpruceTable
            | $crate::world::BlockId::JungleTable
            | $crate::world::BlockId::WillowTable
            | $crate::world::BlockId::AcaciaTable
            | $crate::world::BlockId::OakChair
            | $crate::world::BlockId::SpruceChair
            | $crate::world::BlockId::JungleChair
            | $crate::world::BlockId::WillowChair
            | $crate::world::BlockId::AcaciaChair
            | $crate::world::BlockId::Bookshelf
            | $crate::world::BlockId::WhiteCarpet
            | $crate::world::BlockId::OrangeCarpet
            | $crate::world::BlockId::MagentaCarpet
            | $crate::world::BlockId::LightBlueCarpet
            | $crate::world::BlockId::YellowCarpet
            | $crate::world::BlockId::LimeCarpet
            | $crate::world::BlockId::PinkCarpet
            | $crate::world::BlockId::GrayCarpet
            | $crate::world::BlockId::LightGrayCarpet
            | $crate::world::BlockId::CyanCarpet
            | $crate::world::BlockId::PurpleCarpet
            | $crate::world::BlockId::BlueCarpet
            | $crate::world::BlockId::BrownCarpet
            | $crate::world::BlockId::GreenCarpet
            | $crate::world::BlockId::RedCarpet
            | $crate::world::BlockId::BlackCarpet
            | $crate::world::BlockId::FlowerPot
            | $crate::world::BlockId::ItemFrame
            | $crate::world::BlockId::DawnPainting
            | $crate::world::BlockId::MeadowPainting
            | $crate::world::BlockId::HarbourPainting
            | $crate::world::BlockId::StillLifePainting
            | $crate::world::BlockId::ValleyPainting
            | $crate::world::BlockId::ValleyPaintingRight
            | $crate::world::BlockId::SeascapePainting
            | $crate::world::BlockId::SeascapePaintingRight
            | $crate::world::BlockId::ForestPainting
            | $crate::world::BlockId::ForestPaintingRight
            | $crate::world::BlockId::WhiteBanner
            | $crate::world::BlockId::OrangeBanner
            | $crate::world::BlockId::MagentaBanner
            | $crate::world::BlockId::LightBlueBanner
            | $crate::world::BlockId::YellowBanner
            | $crate::world::BlockId::LimeBanner
            | $crate::world::BlockId::PinkBanner
            | $crate::world::BlockId::GrayBanner
            | $crate::world::BlockId::LightGrayBanner
            | $crate::world::BlockId::CyanBanner
            | $crate::world::BlockId::PurpleBanner
            | $crate::world::BlockId::BlueBanner
            | $crate::world::BlockId::BrownBanner
            | $crate::world::BlockId::GreenBanner
            | $crate::world::BlockId::RedBanner
            | $crate::world::BlockId::BlackBanner
            | $crate::world::BlockId::WhiteLantern
            | $crate::world::BlockId::OrangeLantern
            | $crate::world::BlockId::MagentaLantern
            | $crate::world::BlockId::LightBlueLantern
            | $crate::world::BlockId::YellowLantern
            | $crate::world::BlockId::LimeLantern
            | $crate::world::BlockId::PinkLantern
            | $crate::world::BlockId::GrayLantern
            | $crate::world::BlockId::LightGrayLantern
            | $crate::world::BlockId::CyanLantern
            | $crate::world::BlockId::PurpleLantern
            | $crate::world::BlockId::BlueLantern
            | $crate::world::BlockId::BrownLantern
            | $crate::world::BlockId::GreenLantern
            | $crate::world::BlockId::RedLantern
            | $crate::world::BlockId::BlackLantern
            | $crate::world::BlockId::HangingWhiteLantern
            | $crate::world::BlockId::HangingOrangeLantern
            | $crate::world::BlockId::HangingMagentaLantern
            | $crate::world::BlockId::HangingLightBlueLantern
            | $crate::world::BlockId::HangingYellowLantern
            | $crate::world::BlockId::HangingLimeLantern
            | $crate::world::BlockId::HangingPinkLantern
            | $crate::world::BlockId::HangingGrayLantern
            | $crate::world::BlockId::HangingLightGrayLantern
            | $crate::world::BlockId::HangingCyanLantern
            | $crate::world::BlockId::HangingPurpleLantern
            | $crate::world::BlockId::HangingBlueLantern
            | $crate::world::BlockId::HangingBrownLantern
            | $crate::world::BlockId::HangingGreenLantern
            | $crate::world::BlockId::HangingRedLantern
            | $crate::world::BlockId::HangingBlackLantern
    };
}

/// The items added with the furniture, as a pattern.
#[macro_export]
macro_rules! new_decor_items {
    () => {
        $crate::world::ItemId::WhiteBedroll
            | $crate::world::ItemId::OrangeBedroll
            | $crate::world::ItemId::MagentaBedroll
            | $crate::world::ItemId::LightBlueBedroll
            | $crate::world::ItemId::YellowBedroll
            | $crate::world::ItemId::LimeBedroll
            | $crate::world::ItemId::PinkBedroll
            | $crate::world::ItemId::GrayBedroll
            | $crate::world::ItemId::LightGrayBedroll
            | $crate::world::ItemId::CyanBedroll
            | $crate::world::ItemId::PurpleBedroll
            | $crate::world::ItemId::BlueBedroll
            | $crate::world::ItemId::BrownBedroll
            | $crate::world::ItemId::GreenBedroll
            | $crate::world::ItemId::BlackBedroll
            | $crate::world::ItemId::OakTable
            | $crate::world::ItemId::SpruceTable
            | $crate::world::ItemId::JungleTable
            | $crate::world::ItemId::WillowTable
            | $crate::world::ItemId::AcaciaTable
            | $crate::world::ItemId::OakChair
            | $crate::world::ItemId::SpruceChair
            | $crate::world::ItemId::JungleChair
            | $crate::world::ItemId::WillowChair
            | $crate::world::ItemId::AcaciaChair
            | $crate::world::ItemId::Book
            | $crate::world::ItemId::Bookshelf
            | $crate::world::ItemId::WhiteCarpet
            | $crate::world::ItemId::OrangeCarpet
            | $crate::world::ItemId::MagentaCarpet
            | $crate::world::ItemId::LightBlueCarpet
            | $crate::world::ItemId::YellowCarpet
            | $crate::world::ItemId::LimeCarpet
            | $crate::world::ItemId::PinkCarpet
            | $crate::world::ItemId::GrayCarpet
            | $crate::world::ItemId::LightGrayCarpet
            | $crate::world::ItemId::CyanCarpet
            | $crate::world::ItemId::PurpleCarpet
            | $crate::world::ItemId::BlueCarpet
            | $crate::world::ItemId::BrownCarpet
            | $crate::world::ItemId::GreenCarpet
            | $crate::world::ItemId::RedCarpet
            | $crate::world::ItemId::BlackCarpet
            | $crate::world::ItemId::FlowerPot
            | $crate::world::ItemId::ItemFrame
            | $crate::world::ItemId::Painting
            | $crate::world::ItemId::WidePainting
            | $crate::world::ItemId::WhiteBanner
            | $crate::world::ItemId::OrangeBanner
            | $crate::world::ItemId::MagentaBanner
            | $crate::world::ItemId::LightBlueBanner
            | $crate::world::ItemId::YellowBanner
            | $crate::world::ItemId::LimeBanner
            | $crate::world::ItemId::PinkBanner
            | $crate::world::ItemId::GrayBanner
            | $crate::world::ItemId::LightGrayBanner
            | $crate::world::ItemId::CyanBanner
            | $crate::world::ItemId::PurpleBanner
            | $crate::world::ItemId::BlueBanner
            | $crate::world::ItemId::BrownBanner
            | $crate::world::ItemId::GreenBanner
            | $crate::world::ItemId::RedBanner
            | $crate::world::ItemId::BlackBanner
            | $crate::world::ItemId::WhiteLantern
            | $crate::world::ItemId::OrangeLantern
            | $crate::world::ItemId::MagentaLantern
            | $crate::world::ItemId::LightBlueLantern
            | $crate::world::ItemId::YellowLantern
            | $crate::world::ItemId::LimeLantern
            | $crate::world::ItemId::PinkLantern
            | $crate::world::ItemId::GrayLantern
            | $crate::world::ItemId::LightGrayLantern
            | $crate::world::ItemId::CyanLantern
            | $crate::world::ItemId::PurpleLantern
            | $crate::world::ItemId::BlueLantern
            | $crate::world::ItemId::BrownLantern
            | $crate::world::ItemId::GreenLantern
            | $crate::world::ItemId::RedLantern
            | $crate::world::ItemId::BlackLantern
    };
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{BlockHitbox, BlockTransparency, ItemType};

    #[test]
    fn every_block_of_the_furniture_is_placed_by_its_item_and_drops_it() {
        for id in BlockId::ALL {
            let Some(decor) = id.decor() else {
                continue;
            };
            assert!(matches!(id, crate::new_decor_blocks!()), "{id:?}");
            assert_eq!(id.get_drop_table(), vec![(1, decor.item, 1)], "{id:?}");
            let placed = decor.item.decor_block().expect("placed");
            assert_eq!(placed.decor().map(|d| d.item), Some(decor.item));
            assert_eq!(decor.item.get_default_type(), ItemType::Block(placed));
            assert!(id.get_break_time().is_some(), "{id:?}");
        }
        // every new block but the old bedroll's colours is one of them
        assert!(BlockId::ALL
            .into_iter()
            .filter(|id| matches!(id, crate::new_decor_blocks!()))
            .all(|id| id.decor().is_some()));
    }

    #[test]
    fn every_colour_has_a_bedroll_the_red_one_being_the_first() {
        assert_eq!(DyeColor::Red.bedroll(), ItemId::Bedroll);
        for color in DyeColor::ALL {
            let block = match color.bedroll().get_default_type() {
                ItemType::Block(block) => block,
                other => panic!("{other:?}"),
            };
            assert!(block.is_bedroll(), "{color:?}");
            assert!(block.is_usable());
        }
    }

    #[test]
    fn a_click_on_a_canvas_shows_the_next_of_its_size_and_comes_back_to_the_first() {
        let mut canvas = BlockId::DawnPainting;
        for _ in 0..SMALL_CANVASES.len() {
            let (next, right) = canvas.next_canvas().unwrap();
            assert_eq!(right, None);
            canvas = next;
        }
        assert_eq!(canvas, BlockId::DawnPainting);
        let (left, right) = BlockId::ValleyPaintingRight.next_canvas().unwrap();
        assert_eq!(
            (left, right),
            (
                BlockId::SeascapePainting,
                Some(BlockId::SeascapePaintingRight)
            )
        );
    }

    #[test]
    fn the_halves_of_a_wide_canvas_find_each_other_on_every_wall() {
        for direction in [
            BlockDirection::Front,
            BlockDirection::Right,
            BlockDirection::Back,
            BlockDirection::Left,
        ] {
            let (right, step) = BlockId::ForestPainting
                .other_painting_half(direction)
                .unwrap();
            assert_eq!(right, BlockId::ForestPaintingRight);
            let (left, back) = right.other_painting_half(direction).unwrap();
            assert_eq!((left, back), (BlockId::ForestPainting, -step));
            // along the wall
            assert_eq!(step.dot(direction.offset()), 0);
            assert_eq!(step.abs().max_element(), 1);
        }
        // seen from the south, looking north, the right is to the east
        assert_eq!(painting_right(BlockDirection::Front), IVec3::X);
    }

    #[test]
    fn the_furniture_is_seen_through_and_the_lanterns_glow() {
        for id in BlockId::ALL.into_iter().filter(|id| id.decor().is_some()) {
            let expected = match id.decor_kind() {
                Some(DecorKind::Bookshelf) => BlockTransparency::Solid,
                _ => BlockTransparency::Decoration,
            };
            assert_eq!(id.get_visibility(), expected, "{id:?}");
        }
        assert_eq!(BlockId::RedLantern.light_emission(), 15);
        assert_eq!(BlockId::HangingBlueLantern.light_emission(), 15);
        assert!(matches!(BlockId::ItemFrame.get_hitbox(), BlockHitbox::None));
        assert!(matches!(
            BlockId::ItemFrame.get_ray_hitbox(),
            BlockHitbox::Aabb(_)
        ));
    }

    #[test]
    fn the_patterns_of_a_banner_are_dyed_one_over_the_other_up_to_six() {
        let mut patterns = BannerPatterns::default();
        assert!(patterns.is_empty());
        for (n, pattern) in BannerPattern::ALL
            .into_iter()
            .take(MAX_BANNER_LAYERS)
            .enumerate()
        {
            patterns = patterns.with(pattern, DyeColor::ALL[n]).unwrap();
        }
        assert_eq!(patterns.with(BannerPattern::Roundel, DyeColor::Red), None);
        let layers: Vec<_> = patterns.layers().collect();
        assert_eq!(layers.len(), MAX_BANNER_LAYERS);
        assert_eq!(layers[0], (BannerPattern::Chief, DyeColor::White));
        assert_eq!(layers[5], (BannerPattern::BendSinister, DyeColor::Lime));
    }

    #[test]
    fn each_pattern_has_a_shape_of_its_own_and_covers_part_of_the_cloth() {
        for pattern in BannerPattern::ALL {
            assert_eq!(BannerPattern::of_cells(pattern.shape()), Some(pattern));
            let covered = (0..12)
                .flat_map(|x| (0..14).map(move |y| (x, y)))
                .filter(|&(x, y)| pattern.covers(x, y, 12, 14))
                .count();
            assert!(covered > 4 && covered < 12 * 14, "{pattern:?}: {covered}");
        }
    }

    #[test]
    fn dyes_laid_in_a_shape_around_a_banner_dye_its_pattern_over_the_others() {
        let stack = |item_id: ItemId| {
            Some(ItemStack {
                item_id,
                item_type: item_id.get_default_type(),
                nb: 1,
            })
        };
        let mut cells = [None; 9];
        cells[0] = stack(ItemId::BlueDye);
        cells[1] = stack(ItemId::BlueDye);
        cells[2] = stack(ItemId::BlueDye);
        cells[4] = stack(ItemId::WhiteBanner);
        let striped = dye_banner(&cells).unwrap();
        let ItemType::Banner { patterns } = striped.item_type else {
            panic!("{striped:?}");
        };
        assert_eq!(
            patterns.layers().collect::<Vec<_>>(),
            vec![(BannerPattern::Chief, DyeColor::Blue)]
        );
        // a red roundel over it
        let mut cells = [None; 9];
        cells[4] = stack(ItemId::RedDye);
        cells[7] = Some(striped);
        let ItemType::Banner { patterns } = dye_banner(&cells).unwrap().item_type else {
            panic!();
        };
        assert_eq!(patterns.layers().count(), 2);
        // dyes of two colours, or in no shape, make nothing
        cells[3] = stack(ItemId::BlueDye);
        assert_eq!(dye_banner(&cells), None);
        let mut cells = [None; 9];
        cells[0] = stack(ItemId::RedDye);
        cells[5] = stack(ItemId::RedDye);
        cells[4] = stack(ItemId::RedBanner);
        assert_eq!(dye_banner(&cells), None);
    }

    #[test]
    fn a_pot_holds_the_flowers_the_saplings_and_the_mushrooms_only() {
        assert!(ItemId::Poppy.fits_flower_pot());
        assert!(ItemId::OakSapling.fits_flower_pot());
        assert!(ItemId::Mushroom.fits_flower_pot());
        assert!(!ItemId::Stone.fits_flower_pot());
        assert!(!ItemId::Stick.fits_flower_pot());
    }
}
