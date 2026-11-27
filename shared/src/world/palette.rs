//! The palette of the builder: the slabs, stairs, doors, trapdoors and fence gates of each wood,
//! the slabs, stairs and low walls of each rock, the polished and chiseled rocks, and the sixteen
//! dyes with the wool, glass, terracotta and concrete they colour.
//!
//! Each of these blocks has a `Form`, and a `base`: the plain block it is made of, which it breaks,
//! sounds and burns like, and whose texture a slab, a stair or a low wall shows.

use bevy::math::IVec3;
use serde::{Deserialize, Serialize};

use super::{BlockId, ItemId};

/// The shape of a block of the palette.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Form {
    /// The lower half of its cell.
    Slab,
    /// A slab under the back half of a cell.
    Stairs,
    /// A post that reaches out to the low walls and the whole blocks around it, as a fence does.
    LowWall,
    /// A half of a door: the lower one, or the `top`.
    Door { top: bool, open: bool },
    /// A plate on the floor of its cell, or standing on its front once open.
    Trapdoor { open: bool },
    /// A gate across its cell between two posts, which fences and low walls reach out to.
    FenceGate { open: bool },
    /// A pane of glass, which reaches out to the panes, the glass and the whole blocks around it.
    Pane,
    /// A whole block.
    Cube,
}

impl Form {
    /// Whether a right click opens or shuts the block.
    pub fn opens(self) -> bool {
        matches!(
            self,
            Self::Door { .. } | Self::Trapdoor { .. } | Self::FenceGate { .. }
        )
    }

    /// Whether the block is turned toward the player who places it.
    pub fn faces_placer(self) -> bool {
        matches!(
            self,
            Self::Stairs | Self::Door { .. } | Self::Trapdoor { .. } | Self::FenceGate { .. }
        )
    }
}

/// What a block of the palette is: its form, the plain block it is made of, and its item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Palette {
    pub form: Form,
    pub base: BlockId,
    pub item: ItemId,
}

/// The species of wood, whose planks make their own slabs, stairs, doors, trapdoors and gates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Wood {
    Oak,
    Spruce,
    Jungle,
    Willow,
    Acacia,
}

impl Wood {
    pub const ALL: [Wood; 5] = [
        Self::Oak,
        Self::Spruce,
        Self::Jungle,
        Self::Willow,
        Self::Acacia,
    ];

    /// The planks of the species.
    pub fn planks(self) -> ItemId {
        match self {
            Self::Oak => ItemId::OakPlanks,
            Self::Spruce => ItemId::SprucePlanks,
            Self::Jungle => ItemId::JunglePlanks,
            Self::Willow => ItemId::WillowPlanks,
            Self::Acacia => ItemId::AcaciaPlanks,
        }
    }

    /// Its slab.
    pub fn slab(self) -> ItemId {
        match self {
            Self::Oak => ItemId::OakSlab,
            Self::Spruce => ItemId::SpruceSlab,
            Self::Jungle => ItemId::JungleSlab,
            Self::Willow => ItemId::WillowSlab,
            Self::Acacia => ItemId::AcaciaSlab,
        }
    }

    /// Its stairs.
    pub fn stairs(self) -> ItemId {
        match self {
            Self::Oak => ItemId::OakStairs,
            Self::Spruce => ItemId::SpruceStairs,
            Self::Jungle => ItemId::JungleStairs,
            Self::Willow => ItemId::WillowStairs,
            Self::Acacia => ItemId::AcaciaStairs,
        }
    }

    /// Its door.
    pub fn door(self) -> ItemId {
        match self {
            Self::Oak => ItemId::Door,
            Self::Spruce => ItemId::SpruceDoor,
            Self::Jungle => ItemId::JungleDoor,
            Self::Willow => ItemId::WillowDoor,
            Self::Acacia => ItemId::AcaciaDoor,
        }
    }

    /// Its trapdoor.
    pub fn trapdoor(self) -> ItemId {
        match self {
            Self::Oak => ItemId::OakTrapdoor,
            Self::Spruce => ItemId::SpruceTrapdoor,
            Self::Jungle => ItemId::JungleTrapdoor,
            Self::Willow => ItemId::WillowTrapdoor,
            Self::Acacia => ItemId::AcaciaTrapdoor,
        }
    }

    /// Its fence gate.
    pub fn fence_gate(self) -> ItemId {
        match self {
            Self::Oak => ItemId::OakFenceGate,
            Self::Spruce => ItemId::SpruceFenceGate,
            Self::Jungle => ItemId::JungleFenceGate,
            Self::Willow => ItemId::WillowFenceGate,
            Self::Acacia => ItemId::AcaciaFenceGate,
        }
    }
}

/// The rocks cut into slabs, stairs and low walls.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Rock {
    Stone,
    Cobblestone,
    HewnStone,
    Limestone,
    Slate,
    Brick,
    Gloomrock,
    CobbledGloomrock,
}

impl Rock {
    pub const ALL: [Rock; 8] = [
        Self::Stone,
        Self::Cobblestone,
        Self::HewnStone,
        Self::Limestone,
        Self::Slate,
        Self::Brick,
        Self::Gloomrock,
        Self::CobbledGloomrock,
    ];

    /// The block it is cut from.
    pub fn material(self) -> ItemId {
        match self {
            Self::Stone => ItemId::Stone,
            Self::Cobblestone => ItemId::Cobblestone,
            Self::HewnStone => ItemId::HewnStone,
            Self::Limestone => ItemId::Limestone,
            Self::Slate => ItemId::Slate,
            Self::Brick => ItemId::BrickWall,
            Self::Gloomrock => ItemId::Gloomrock,
            Self::CobbledGloomrock => ItemId::CobbledGloomrock,
        }
    }

    /// Its slab.
    pub fn slab(self) -> ItemId {
        match self {
            Self::Stone => ItemId::StoneSlab,
            Self::Cobblestone => ItemId::CobblestoneSlab,
            Self::HewnStone => ItemId::HewnStoneSlab,
            Self::Limestone => ItemId::LimestoneSlab,
            Self::Slate => ItemId::SlateSlab,
            Self::Brick => ItemId::BrickSlab,
            Self::Gloomrock => ItemId::GloomrockSlab,
            Self::CobbledGloomrock => ItemId::CobbledGloomrockSlab,
        }
    }

    /// Its stairs.
    pub fn stairs(self) -> ItemId {
        match self {
            Self::Stone => ItemId::StoneStairs,
            Self::Cobblestone => ItemId::CobblestoneStairs,
            Self::HewnStone => ItemId::HewnStoneStairs,
            Self::Limestone => ItemId::LimestoneStairs,
            Self::Slate => ItemId::SlateStairs,
            Self::Brick => ItemId::BrickStairs,
            Self::Gloomrock => ItemId::GloomrockStairs,
            Self::CobbledGloomrock => ItemId::CobbledGloomrockStairs,
        }
    }

    /// Its low wall.
    pub fn low_wall(self) -> ItemId {
        match self {
            Self::Stone => ItemId::StoneLowWall,
            Self::Cobblestone => ItemId::CobblestoneLowWall,
            Self::HewnStone => ItemId::HewnStoneLowWall,
            Self::Limestone => ItemId::LimestoneLowWall,
            Self::Slate => ItemId::SlateLowWall,
            Self::Brick => ItemId::BrickLowWall,
            Self::Gloomrock => ItemId::GloomrockLowWall,
            Self::CobbledGloomrock => ItemId::CobbledGloomrockLowWall,
        }
    }

    /// Its polished and its chiseled blocks, for the rocks that have them.
    pub fn polished_and_chiseled(self) -> Option<(ItemId, ItemId)> {
        match self {
            Self::Stone => Some((ItemId::PolishedStone, ItemId::ChiseledStone)),
            Self::Limestone => Some((ItemId::PolishedLimestone, ItemId::ChiseledLimestone)),
            Self::Slate => Some((ItemId::PolishedSlate, ItemId::ChiseledSlate)),
            Self::Gloomrock => Some((ItemId::PolishedGloomrock, ItemId::ChiseledGloomrock)),
            _ => None,
        }
    }
}

/// The sixteen colours of the dyes, in the order of Minecraft's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DyeColor {
    White,
    Orange,
    Magenta,
    LightBlue,
    Yellow,
    Lime,
    Pink,
    Gray,
    LightGray,
    Cyan,
    Purple,
    Blue,
    Brown,
    Green,
    Red,
    Black,
}

impl DyeColor {
    pub const ALL: [DyeColor; 16] = [
        Self::White,
        Self::Orange,
        Self::Magenta,
        Self::LightBlue,
        Self::Yellow,
        Self::Lime,
        Self::Pink,
        Self::Gray,
        Self::LightGray,
        Self::Cyan,
        Self::Purple,
        Self::Blue,
        Self::Brown,
        Self::Green,
        Self::Red,
        Self::Black,
    ];

    /// The dye of the colour.
    pub fn dye(self) -> ItemId {
        match self {
            Self::White => ItemId::WhiteDye,
            Self::Orange => ItemId::OrangeDye,
            Self::Magenta => ItemId::MagentaDye,
            Self::LightBlue => ItemId::LightBlueDye,
            Self::Yellow => ItemId::YellowDye,
            Self::Lime => ItemId::LimeDye,
            Self::Pink => ItemId::PinkDye,
            Self::Gray => ItemId::GrayDye,
            Self::LightGray => ItemId::LightGrayDye,
            Self::Cyan => ItemId::CyanDye,
            Self::Purple => ItemId::PurpleDye,
            Self::Blue => ItemId::BlueDye,
            Self::Brown => ItemId::BrownDye,
            Self::Green => ItemId::GreenDye,
            Self::Red => ItemId::RedDye,
            Self::Black => ItemId::BlackDye,
        }
    }

    /// The wool of the colour: the white one is the plain wool of the sheep.
    pub fn wool(self) -> ItemId {
        match self {
            Self::White => ItemId::Wool,
            Self::Orange => ItemId::OrangeWool,
            Self::Magenta => ItemId::MagentaWool,
            Self::LightBlue => ItemId::LightBlueWool,
            Self::Yellow => ItemId::YellowWool,
            Self::Lime => ItemId::LimeWool,
            Self::Pink => ItemId::PinkWool,
            Self::Gray => ItemId::GrayWool,
            Self::LightGray => ItemId::LightGrayWool,
            Self::Cyan => ItemId::CyanWool,
            Self::Purple => ItemId::PurpleWool,
            Self::Blue => ItemId::BlueWool,
            Self::Brown => ItemId::BrownWool,
            Self::Green => ItemId::GreenWool,
            Self::Red => ItemId::RedWool,
            Self::Black => ItemId::BlackWool,
        }
    }

    /// The glass of the colour.
    pub fn stained_glass(self) -> ItemId {
        match self {
            Self::White => ItemId::WhiteStainedGlass,
            Self::Orange => ItemId::OrangeStainedGlass,
            Self::Magenta => ItemId::MagentaStainedGlass,
            Self::LightBlue => ItemId::LightBlueStainedGlass,
            Self::Yellow => ItemId::YellowStainedGlass,
            Self::Lime => ItemId::LimeStainedGlass,
            Self::Pink => ItemId::PinkStainedGlass,
            Self::Gray => ItemId::GrayStainedGlass,
            Self::LightGray => ItemId::LightGrayStainedGlass,
            Self::Cyan => ItemId::CyanStainedGlass,
            Self::Purple => ItemId::PurpleStainedGlass,
            Self::Blue => ItemId::BlueStainedGlass,
            Self::Brown => ItemId::BrownStainedGlass,
            Self::Green => ItemId::GreenStainedGlass,
            Self::Red => ItemId::RedStainedGlass,
            Self::Black => ItemId::BlackStainedGlass,
        }
    }

    /// The pane of the colour.
    pub fn stained_glass_pane(self) -> ItemId {
        match self {
            Self::White => ItemId::WhiteStainedGlassPane,
            Self::Orange => ItemId::OrangeStainedGlassPane,
            Self::Magenta => ItemId::MagentaStainedGlassPane,
            Self::LightBlue => ItemId::LightBlueStainedGlassPane,
            Self::Yellow => ItemId::YellowStainedGlassPane,
            Self::Lime => ItemId::LimeStainedGlassPane,
            Self::Pink => ItemId::PinkStainedGlassPane,
            Self::Gray => ItemId::GrayStainedGlassPane,
            Self::LightGray => ItemId::LightGrayStainedGlassPane,
            Self::Cyan => ItemId::CyanStainedGlassPane,
            Self::Purple => ItemId::PurpleStainedGlassPane,
            Self::Blue => ItemId::BlueStainedGlassPane,
            Self::Brown => ItemId::BrownStainedGlassPane,
            Self::Green => ItemId::GreenStainedGlassPane,
            Self::Red => ItemId::RedStainedGlassPane,
            Self::Black => ItemId::BlackStainedGlassPane,
        }
    }

    /// The terracotta of the colour.
    pub fn terracotta(self) -> ItemId {
        match self {
            Self::White => ItemId::WhiteTerracotta,
            Self::Orange => ItemId::OrangeTerracotta,
            Self::Magenta => ItemId::MagentaTerracotta,
            Self::LightBlue => ItemId::LightBlueTerracotta,
            Self::Yellow => ItemId::YellowTerracotta,
            Self::Lime => ItemId::LimeTerracotta,
            Self::Pink => ItemId::PinkTerracotta,
            Self::Gray => ItemId::GrayTerracotta,
            Self::LightGray => ItemId::LightGrayTerracotta,
            Self::Cyan => ItemId::CyanTerracotta,
            Self::Purple => ItemId::PurpleTerracotta,
            Self::Blue => ItemId::BlueTerracotta,
            Self::Brown => ItemId::BrownTerracotta,
            Self::Green => ItemId::GreenTerracotta,
            Self::Red => ItemId::RedTerracotta,
            Self::Black => ItemId::BlackTerracotta,
        }
    }

    /// The concrete of the colour.
    pub fn concrete(self) -> ItemId {
        match self {
            Self::White => ItemId::WhiteConcrete,
            Self::Orange => ItemId::OrangeConcrete,
            Self::Magenta => ItemId::MagentaConcrete,
            Self::LightBlue => ItemId::LightBlueConcrete,
            Self::Yellow => ItemId::YellowConcrete,
            Self::Lime => ItemId::LimeConcrete,
            Self::Pink => ItemId::PinkConcrete,
            Self::Gray => ItemId::GrayConcrete,
            Self::LightGray => ItemId::LightGrayConcrete,
            Self::Cyan => ItemId::CyanConcrete,
            Self::Purple => ItemId::PurpleConcrete,
            Self::Blue => ItemId::BlueConcrete,
            Self::Brown => ItemId::BrownConcrete,
            Self::Green => ItemId::GreenConcrete,
            Self::Red => ItemId::RedConcrete,
            Self::Black => ItemId::BlackConcrete,
        }
    }

    /// The concrete powder of the colour.
    pub fn concrete_powder(self) -> ItemId {
        match self {
            Self::White => ItemId::WhiteConcretePowder,
            Self::Orange => ItemId::OrangeConcretePowder,
            Self::Magenta => ItemId::MagentaConcretePowder,
            Self::LightBlue => ItemId::LightBlueConcretePowder,
            Self::Yellow => ItemId::YellowConcretePowder,
            Self::Lime => ItemId::LimeConcretePowder,
            Self::Pink => ItemId::PinkConcretePowder,
            Self::Gray => ItemId::GrayConcretePowder,
            Self::LightGray => ItemId::LightGrayConcretePowder,
            Self::Cyan => ItemId::CyanConcretePowder,
            Self::Purple => ItemId::PurpleConcretePowder,
            Self::Blue => ItemId::BlueConcretePowder,
            Self::Brown => ItemId::BrownConcretePowder,
            Self::Green => ItemId::GreenConcretePowder,
            Self::Red => ItemId::RedConcretePowder,
            Self::Black => ItemId::BlackConcretePowder,
        }
    }
}

/// The wool of every colour, in the order of `DyeColor::ALL`.
pub const WOOLS: [ItemId; 16] = [
    ItemId::Wool,
    ItemId::OrangeWool,
    ItemId::MagentaWool,
    ItemId::LightBlueWool,
    ItemId::YellowWool,
    ItemId::LimeWool,
    ItemId::PinkWool,
    ItemId::GrayWool,
    ItemId::LightGrayWool,
    ItemId::CyanWool,
    ItemId::PurpleWool,
    ItemId::BlueWool,
    ItemId::BrownWool,
    ItemId::GreenWool,
    ItemId::RedWool,
    ItemId::BlackWool,
];

impl BlockId {
    /// What the block is, if it is one of the palette.
    #[inline]
    pub fn palette(self) -> Option<Palette> {
        self.props().palette
    }

    /// `palette` as the code writes it, which `Content::builtin` fills the table with.
    pub(crate) fn builtin_palette(self) -> Option<Palette> {
        let (form, base, item) = match self {
            Self::OakSlab => (Form::Slab, Self::OakPlanks, ItemId::OakSlab),
            Self::HewnStoneSlab => (Form::Slab, Self::HewnStone, ItemId::HewnStoneSlab),
            Self::BrickSlab => (Form::Slab, Self::BrickWall, ItemId::BrickSlab),
            Self::OakStairs => (Form::Stairs, Self::OakPlanks, ItemId::OakStairs),
            Self::HewnStoneStairs => (Form::Stairs, Self::HewnStone, ItemId::HewnStoneStairs),
            Self::BrickStairs => (Form::Stairs, Self::BrickWall, ItemId::BrickStairs),
            Self::Door => (
                Form::Door {
                    top: false,
                    open: false,
                },
                Self::OakPlanks,
                ItemId::Door,
            ),
            Self::DoorTop => (
                Form::Door {
                    top: true,
                    open: false,
                },
                Self::OakPlanks,
                ItemId::Door,
            ),
            Self::OpenDoor => (
                Form::Door {
                    top: false,
                    open: true,
                },
                Self::OakPlanks,
                ItemId::Door,
            ),
            Self::OpenDoorTop => (
                Form::Door {
                    top: true,
                    open: true,
                },
                Self::OakPlanks,
                ItemId::Door,
            ),
            Self::GlassPane => (Form::Pane, Self::Glass, ItemId::GlassPane),
            Self::SpruceSlab => (Form::Slab, Self::SprucePlanks, ItemId::SpruceSlab),
            Self::JungleSlab => (Form::Slab, Self::JunglePlanks, ItemId::JungleSlab),
            Self::WillowSlab => (Form::Slab, Self::WillowPlanks, ItemId::WillowSlab),
            Self::AcaciaSlab => (Form::Slab, Self::AcaciaPlanks, ItemId::AcaciaSlab),
            Self::SpruceStairs => (Form::Stairs, Self::SprucePlanks, ItemId::SpruceStairs),
            Self::JungleStairs => (Form::Stairs, Self::JunglePlanks, ItemId::JungleStairs),
            Self::WillowStairs => (Form::Stairs, Self::WillowPlanks, ItemId::WillowStairs),
            Self::AcaciaStairs => (Form::Stairs, Self::AcaciaPlanks, ItemId::AcaciaStairs),
            Self::SpruceDoor => (
                Form::Door {
                    top: false,
                    open: false,
                },
                Self::SprucePlanks,
                ItemId::SpruceDoor,
            ),
            Self::SpruceDoorTop => (
                Form::Door {
                    top: true,
                    open: false,
                },
                Self::SprucePlanks,
                ItemId::SpruceDoor,
            ),
            Self::OpenSpruceDoor => (
                Form::Door {
                    top: false,
                    open: true,
                },
                Self::SprucePlanks,
                ItemId::SpruceDoor,
            ),
            Self::OpenSpruceDoorTop => (
                Form::Door {
                    top: true,
                    open: true,
                },
                Self::SprucePlanks,
                ItemId::SpruceDoor,
            ),
            Self::JungleDoor => (
                Form::Door {
                    top: false,
                    open: false,
                },
                Self::JunglePlanks,
                ItemId::JungleDoor,
            ),
            Self::JungleDoorTop => (
                Form::Door {
                    top: true,
                    open: false,
                },
                Self::JunglePlanks,
                ItemId::JungleDoor,
            ),
            Self::OpenJungleDoor => (
                Form::Door {
                    top: false,
                    open: true,
                },
                Self::JunglePlanks,
                ItemId::JungleDoor,
            ),
            Self::OpenJungleDoorTop => (
                Form::Door {
                    top: true,
                    open: true,
                },
                Self::JunglePlanks,
                ItemId::JungleDoor,
            ),
            Self::WillowDoor => (
                Form::Door {
                    top: false,
                    open: false,
                },
                Self::WillowPlanks,
                ItemId::WillowDoor,
            ),
            Self::WillowDoorTop => (
                Form::Door {
                    top: true,
                    open: false,
                },
                Self::WillowPlanks,
                ItemId::WillowDoor,
            ),
            Self::OpenWillowDoor => (
                Form::Door {
                    top: false,
                    open: true,
                },
                Self::WillowPlanks,
                ItemId::WillowDoor,
            ),
            Self::OpenWillowDoorTop => (
                Form::Door {
                    top: true,
                    open: true,
                },
                Self::WillowPlanks,
                ItemId::WillowDoor,
            ),
            Self::AcaciaDoor => (
                Form::Door {
                    top: false,
                    open: false,
                },
                Self::AcaciaPlanks,
                ItemId::AcaciaDoor,
            ),
            Self::AcaciaDoorTop => (
                Form::Door {
                    top: true,
                    open: false,
                },
                Self::AcaciaPlanks,
                ItemId::AcaciaDoor,
            ),
            Self::OpenAcaciaDoor => (
                Form::Door {
                    top: false,
                    open: true,
                },
                Self::AcaciaPlanks,
                ItemId::AcaciaDoor,
            ),
            Self::OpenAcaciaDoorTop => (
                Form::Door {
                    top: true,
                    open: true,
                },
                Self::AcaciaPlanks,
                ItemId::AcaciaDoor,
            ),
            Self::OakTrapdoor => (
                Form::Trapdoor { open: false },
                Self::OakPlanks,
                ItemId::OakTrapdoor,
            ),
            Self::OpenOakTrapdoor => (
                Form::Trapdoor { open: true },
                Self::OakPlanks,
                ItemId::OakTrapdoor,
            ),
            Self::SpruceTrapdoor => (
                Form::Trapdoor { open: false },
                Self::SprucePlanks,
                ItemId::SpruceTrapdoor,
            ),
            Self::OpenSpruceTrapdoor => (
                Form::Trapdoor { open: true },
                Self::SprucePlanks,
                ItemId::SpruceTrapdoor,
            ),
            Self::JungleTrapdoor => (
                Form::Trapdoor { open: false },
                Self::JunglePlanks,
                ItemId::JungleTrapdoor,
            ),
            Self::OpenJungleTrapdoor => (
                Form::Trapdoor { open: true },
                Self::JunglePlanks,
                ItemId::JungleTrapdoor,
            ),
            Self::WillowTrapdoor => (
                Form::Trapdoor { open: false },
                Self::WillowPlanks,
                ItemId::WillowTrapdoor,
            ),
            Self::OpenWillowTrapdoor => (
                Form::Trapdoor { open: true },
                Self::WillowPlanks,
                ItemId::WillowTrapdoor,
            ),
            Self::AcaciaTrapdoor => (
                Form::Trapdoor { open: false },
                Self::AcaciaPlanks,
                ItemId::AcaciaTrapdoor,
            ),
            Self::OpenAcaciaTrapdoor => (
                Form::Trapdoor { open: true },
                Self::AcaciaPlanks,
                ItemId::AcaciaTrapdoor,
            ),
            Self::OakFenceGate => (
                Form::FenceGate { open: false },
                Self::OakPlanks,
                ItemId::OakFenceGate,
            ),
            Self::OpenOakFenceGate => (
                Form::FenceGate { open: true },
                Self::OakPlanks,
                ItemId::OakFenceGate,
            ),
            Self::SpruceFenceGate => (
                Form::FenceGate { open: false },
                Self::SprucePlanks,
                ItemId::SpruceFenceGate,
            ),
            Self::OpenSpruceFenceGate => (
                Form::FenceGate { open: true },
                Self::SprucePlanks,
                ItemId::SpruceFenceGate,
            ),
            Self::JungleFenceGate => (
                Form::FenceGate { open: false },
                Self::JunglePlanks,
                ItemId::JungleFenceGate,
            ),
            Self::OpenJungleFenceGate => (
                Form::FenceGate { open: true },
                Self::JunglePlanks,
                ItemId::JungleFenceGate,
            ),
            Self::WillowFenceGate => (
                Form::FenceGate { open: false },
                Self::WillowPlanks,
                ItemId::WillowFenceGate,
            ),
            Self::OpenWillowFenceGate => (
                Form::FenceGate { open: true },
                Self::WillowPlanks,
                ItemId::WillowFenceGate,
            ),
            Self::AcaciaFenceGate => (
                Form::FenceGate { open: false },
                Self::AcaciaPlanks,
                ItemId::AcaciaFenceGate,
            ),
            Self::OpenAcaciaFenceGate => (
                Form::FenceGate { open: true },
                Self::AcaciaPlanks,
                ItemId::AcaciaFenceGate,
            ),
            Self::StoneSlab => (Form::Slab, Self::Stone, ItemId::StoneSlab),
            Self::CobblestoneSlab => (Form::Slab, Self::Cobblestone, ItemId::CobblestoneSlab),
            Self::LimestoneSlab => (Form::Slab, Self::Limestone, ItemId::LimestoneSlab),
            Self::SlateSlab => (Form::Slab, Self::Slate, ItemId::SlateSlab),
            Self::GloomrockSlab => (Form::Slab, Self::Gloomrock, ItemId::GloomrockSlab),
            Self::CobbledGloomrockSlab => (
                Form::Slab,
                Self::CobbledGloomrock,
                ItemId::CobbledGloomrockSlab,
            ),
            Self::StoneStairs => (Form::Stairs, Self::Stone, ItemId::StoneStairs),
            Self::CobblestoneStairs => (Form::Stairs, Self::Cobblestone, ItemId::CobblestoneStairs),
            Self::LimestoneStairs => (Form::Stairs, Self::Limestone, ItemId::LimestoneStairs),
            Self::SlateStairs => (Form::Stairs, Self::Slate, ItemId::SlateStairs),
            Self::GloomrockStairs => (Form::Stairs, Self::Gloomrock, ItemId::GloomrockStairs),
            Self::CobbledGloomrockStairs => (
                Form::Stairs,
                Self::CobbledGloomrock,
                ItemId::CobbledGloomrockStairs,
            ),
            Self::StoneLowWall => (Form::LowWall, Self::Stone, ItemId::StoneLowWall),
            Self::CobblestoneLowWall => {
                (Form::LowWall, Self::Cobblestone, ItemId::CobblestoneLowWall)
            }
            Self::HewnStoneLowWall => (Form::LowWall, Self::HewnStone, ItemId::HewnStoneLowWall),
            Self::LimestoneLowWall => (Form::LowWall, Self::Limestone, ItemId::LimestoneLowWall),
            Self::SlateLowWall => (Form::LowWall, Self::Slate, ItemId::SlateLowWall),
            Self::BrickLowWall => (Form::LowWall, Self::BrickWall, ItemId::BrickLowWall),
            Self::GloomrockLowWall => (Form::LowWall, Self::Gloomrock, ItemId::GloomrockLowWall),
            Self::CobbledGloomrockLowWall => (
                Form::LowWall,
                Self::CobbledGloomrock,
                ItemId::CobbledGloomrockLowWall,
            ),
            Self::PolishedStone => (Form::Cube, Self::Stone, ItemId::PolishedStone),
            Self::PolishedLimestone => (Form::Cube, Self::Limestone, ItemId::PolishedLimestone),
            Self::PolishedSlate => (Form::Cube, Self::Slate, ItemId::PolishedSlate),
            Self::PolishedGloomrock => (Form::Cube, Self::Gloomrock, ItemId::PolishedGloomrock),
            Self::ChiseledStone => (Form::Cube, Self::Stone, ItemId::ChiseledStone),
            Self::ChiseledLimestone => (Form::Cube, Self::Limestone, ItemId::ChiseledLimestone),
            Self::ChiseledSlate => (Form::Cube, Self::Slate, ItemId::ChiseledSlate),
            Self::ChiseledGloomrock => (Form::Cube, Self::Gloomrock, ItemId::ChiseledGloomrock),
            Self::OrangeWool => (Form::Cube, Self::Wool, ItemId::OrangeWool),
            Self::MagentaWool => (Form::Cube, Self::Wool, ItemId::MagentaWool),
            Self::LightBlueWool => (Form::Cube, Self::Wool, ItemId::LightBlueWool),
            Self::YellowWool => (Form::Cube, Self::Wool, ItemId::YellowWool),
            Self::LimeWool => (Form::Cube, Self::Wool, ItemId::LimeWool),
            Self::PinkWool => (Form::Cube, Self::Wool, ItemId::PinkWool),
            Self::GrayWool => (Form::Cube, Self::Wool, ItemId::GrayWool),
            Self::LightGrayWool => (Form::Cube, Self::Wool, ItemId::LightGrayWool),
            Self::CyanWool => (Form::Cube, Self::Wool, ItemId::CyanWool),
            Self::PurpleWool => (Form::Cube, Self::Wool, ItemId::PurpleWool),
            Self::BlueWool => (Form::Cube, Self::Wool, ItemId::BlueWool),
            Self::BrownWool => (Form::Cube, Self::Wool, ItemId::BrownWool),
            Self::GreenWool => (Form::Cube, Self::Wool, ItemId::GreenWool),
            Self::RedWool => (Form::Cube, Self::Wool, ItemId::RedWool),
            Self::BlackWool => (Form::Cube, Self::Wool, ItemId::BlackWool),
            Self::WhiteStainedGlass => (Form::Cube, Self::Glass, ItemId::WhiteStainedGlass),
            Self::OrangeStainedGlass => (Form::Cube, Self::Glass, ItemId::OrangeStainedGlass),
            Self::MagentaStainedGlass => (Form::Cube, Self::Glass, ItemId::MagentaStainedGlass),
            Self::LightBlueStainedGlass => (Form::Cube, Self::Glass, ItemId::LightBlueStainedGlass),
            Self::YellowStainedGlass => (Form::Cube, Self::Glass, ItemId::YellowStainedGlass),
            Self::LimeStainedGlass => (Form::Cube, Self::Glass, ItemId::LimeStainedGlass),
            Self::PinkStainedGlass => (Form::Cube, Self::Glass, ItemId::PinkStainedGlass),
            Self::GrayStainedGlass => (Form::Cube, Self::Glass, ItemId::GrayStainedGlass),
            Self::LightGrayStainedGlass => (Form::Cube, Self::Glass, ItemId::LightGrayStainedGlass),
            Self::CyanStainedGlass => (Form::Cube, Self::Glass, ItemId::CyanStainedGlass),
            Self::PurpleStainedGlass => (Form::Cube, Self::Glass, ItemId::PurpleStainedGlass),
            Self::BlueStainedGlass => (Form::Cube, Self::Glass, ItemId::BlueStainedGlass),
            Self::BrownStainedGlass => (Form::Cube, Self::Glass, ItemId::BrownStainedGlass),
            Self::GreenStainedGlass => (Form::Cube, Self::Glass, ItemId::GreenStainedGlass),
            Self::RedStainedGlass => (Form::Cube, Self::Glass, ItemId::RedStainedGlass),
            Self::BlackStainedGlass => (Form::Cube, Self::Glass, ItemId::BlackStainedGlass),
            Self::WhiteStainedGlassPane => (Form::Pane, Self::Glass, ItemId::WhiteStainedGlassPane),
            Self::OrangeStainedGlassPane => {
                (Form::Pane, Self::Glass, ItemId::OrangeStainedGlassPane)
            }
            Self::MagentaStainedGlassPane => {
                (Form::Pane, Self::Glass, ItemId::MagentaStainedGlassPane)
            }
            Self::LightBlueStainedGlassPane => {
                (Form::Pane, Self::Glass, ItemId::LightBlueStainedGlassPane)
            }
            Self::YellowStainedGlassPane => {
                (Form::Pane, Self::Glass, ItemId::YellowStainedGlassPane)
            }
            Self::LimeStainedGlassPane => (Form::Pane, Self::Glass, ItemId::LimeStainedGlassPane),
            Self::PinkStainedGlassPane => (Form::Pane, Self::Glass, ItemId::PinkStainedGlassPane),
            Self::GrayStainedGlassPane => (Form::Pane, Self::Glass, ItemId::GrayStainedGlassPane),
            Self::LightGrayStainedGlassPane => {
                (Form::Pane, Self::Glass, ItemId::LightGrayStainedGlassPane)
            }
            Self::CyanStainedGlassPane => (Form::Pane, Self::Glass, ItemId::CyanStainedGlassPane),
            Self::PurpleStainedGlassPane => {
                (Form::Pane, Self::Glass, ItemId::PurpleStainedGlassPane)
            }
            Self::BlueStainedGlassPane => (Form::Pane, Self::Glass, ItemId::BlueStainedGlassPane),
            Self::BrownStainedGlassPane => (Form::Pane, Self::Glass, ItemId::BrownStainedGlassPane),
            Self::GreenStainedGlassPane => (Form::Pane, Self::Glass, ItemId::GreenStainedGlassPane),
            Self::RedStainedGlassPane => (Form::Pane, Self::Glass, ItemId::RedStainedGlassPane),
            Self::BlackStainedGlassPane => (Form::Pane, Self::Glass, ItemId::BlackStainedGlassPane),
            Self::Terracotta => (Form::Cube, Self::Stone, ItemId::Terracotta),
            Self::WhiteTerracotta => (Form::Cube, Self::Stone, ItemId::WhiteTerracotta),
            Self::OrangeTerracotta => (Form::Cube, Self::Stone, ItemId::OrangeTerracotta),
            Self::MagentaTerracotta => (Form::Cube, Self::Stone, ItemId::MagentaTerracotta),
            Self::LightBlueTerracotta => (Form::Cube, Self::Stone, ItemId::LightBlueTerracotta),
            Self::YellowTerracotta => (Form::Cube, Self::Stone, ItemId::YellowTerracotta),
            Self::LimeTerracotta => (Form::Cube, Self::Stone, ItemId::LimeTerracotta),
            Self::PinkTerracotta => (Form::Cube, Self::Stone, ItemId::PinkTerracotta),
            Self::GrayTerracotta => (Form::Cube, Self::Stone, ItemId::GrayTerracotta),
            Self::LightGrayTerracotta => (Form::Cube, Self::Stone, ItemId::LightGrayTerracotta),
            Self::CyanTerracotta => (Form::Cube, Self::Stone, ItemId::CyanTerracotta),
            Self::PurpleTerracotta => (Form::Cube, Self::Stone, ItemId::PurpleTerracotta),
            Self::BlueTerracotta => (Form::Cube, Self::Stone, ItemId::BlueTerracotta),
            Self::BrownTerracotta => (Form::Cube, Self::Stone, ItemId::BrownTerracotta),
            Self::GreenTerracotta => (Form::Cube, Self::Stone, ItemId::GreenTerracotta),
            Self::RedTerracotta => (Form::Cube, Self::Stone, ItemId::RedTerracotta),
            Self::BlackTerracotta => (Form::Cube, Self::Stone, ItemId::BlackTerracotta),
            Self::WhiteConcrete => (Form::Cube, Self::Stone, ItemId::WhiteConcrete),
            Self::OrangeConcrete => (Form::Cube, Self::Stone, ItemId::OrangeConcrete),
            Self::MagentaConcrete => (Form::Cube, Self::Stone, ItemId::MagentaConcrete),
            Self::LightBlueConcrete => (Form::Cube, Self::Stone, ItemId::LightBlueConcrete),
            Self::YellowConcrete => (Form::Cube, Self::Stone, ItemId::YellowConcrete),
            Self::LimeConcrete => (Form::Cube, Self::Stone, ItemId::LimeConcrete),
            Self::PinkConcrete => (Form::Cube, Self::Stone, ItemId::PinkConcrete),
            Self::GrayConcrete => (Form::Cube, Self::Stone, ItemId::GrayConcrete),
            Self::LightGrayConcrete => (Form::Cube, Self::Stone, ItemId::LightGrayConcrete),
            Self::CyanConcrete => (Form::Cube, Self::Stone, ItemId::CyanConcrete),
            Self::PurpleConcrete => (Form::Cube, Self::Stone, ItemId::PurpleConcrete),
            Self::BlueConcrete => (Form::Cube, Self::Stone, ItemId::BlueConcrete),
            Self::BrownConcrete => (Form::Cube, Self::Stone, ItemId::BrownConcrete),
            Self::GreenConcrete => (Form::Cube, Self::Stone, ItemId::GreenConcrete),
            Self::RedConcrete => (Form::Cube, Self::Stone, ItemId::RedConcrete),
            Self::BlackConcrete => (Form::Cube, Self::Stone, ItemId::BlackConcrete),
            Self::WhiteConcretePowder => (Form::Cube, Self::Sand, ItemId::WhiteConcretePowder),
            Self::OrangeConcretePowder => (Form::Cube, Self::Sand, ItemId::OrangeConcretePowder),
            Self::MagentaConcretePowder => (Form::Cube, Self::Sand, ItemId::MagentaConcretePowder),
            Self::LightBlueConcretePowder => {
                (Form::Cube, Self::Sand, ItemId::LightBlueConcretePowder)
            }
            Self::YellowConcretePowder => (Form::Cube, Self::Sand, ItemId::YellowConcretePowder),
            Self::LimeConcretePowder => (Form::Cube, Self::Sand, ItemId::LimeConcretePowder),
            Self::PinkConcretePowder => (Form::Cube, Self::Sand, ItemId::PinkConcretePowder),
            Self::GrayConcretePowder => (Form::Cube, Self::Sand, ItemId::GrayConcretePowder),
            Self::LightGrayConcretePowder => {
                (Form::Cube, Self::Sand, ItemId::LightGrayConcretePowder)
            }
            Self::CyanConcretePowder => (Form::Cube, Self::Sand, ItemId::CyanConcretePowder),
            Self::PurpleConcretePowder => (Form::Cube, Self::Sand, ItemId::PurpleConcretePowder),
            Self::BlueConcretePowder => (Form::Cube, Self::Sand, ItemId::BlueConcretePowder),
            Self::BrownConcretePowder => (Form::Cube, Self::Sand, ItemId::BrownConcretePowder),
            Self::GreenConcretePowder => (Form::Cube, Self::Sand, ItemId::GreenConcretePowder),
            Self::RedConcretePowder => (Form::Cube, Self::Sand, ItemId::RedConcretePowder),
            Self::BlackConcretePowder => (Form::Cube, Self::Sand, ItemId::BlackConcretePowder),
            _ => return None,
        };
        Some(Palette { form, base, item })
    }

    /// For a half of a door: the same half once opened or shut, and the step to the other half.
    pub fn door(&self) -> Option<(BlockId, IVec3)> {
        let (turned, step) = match *self {
            Self::Door => (Self::OpenDoor, IVec3::Y),
            Self::DoorTop => (Self::OpenDoorTop, IVec3::NEG_Y),
            Self::OpenDoor => (Self::Door, IVec3::Y),
            Self::OpenDoorTop => (Self::DoorTop, IVec3::NEG_Y),
            Self::SpruceDoor => (Self::OpenSpruceDoor, IVec3::Y),
            Self::SpruceDoorTop => (Self::OpenSpruceDoorTop, IVec3::NEG_Y),
            Self::OpenSpruceDoor => (Self::SpruceDoor, IVec3::Y),
            Self::OpenSpruceDoorTop => (Self::SpruceDoorTop, IVec3::NEG_Y),
            Self::JungleDoor => (Self::OpenJungleDoor, IVec3::Y),
            Self::JungleDoorTop => (Self::OpenJungleDoorTop, IVec3::NEG_Y),
            Self::OpenJungleDoor => (Self::JungleDoor, IVec3::Y),
            Self::OpenJungleDoorTop => (Self::JungleDoorTop, IVec3::NEG_Y),
            Self::WillowDoor => (Self::OpenWillowDoor, IVec3::Y),
            Self::WillowDoorTop => (Self::OpenWillowDoorTop, IVec3::NEG_Y),
            Self::OpenWillowDoor => (Self::WillowDoor, IVec3::Y),
            Self::OpenWillowDoorTop => (Self::WillowDoorTop, IVec3::NEG_Y),
            Self::AcaciaDoor => (Self::OpenAcaciaDoor, IVec3::Y),
            Self::AcaciaDoorTop => (Self::OpenAcaciaDoorTop, IVec3::NEG_Y),
            Self::OpenAcaciaDoor => (Self::AcaciaDoor, IVec3::Y),
            Self::OpenAcaciaDoorTop => (Self::AcaciaDoorTop, IVec3::NEG_Y),
            _ => return None,
        };
        Some((turned, step))
    }

    /// For the lower half of a shut door: its top half, placed above it.
    pub fn door_top(&self) -> Option<BlockId> {
        match *self {
            Self::Door => Some(Self::DoorTop),
            Self::SpruceDoor => Some(Self::SpruceDoorTop),
            Self::JungleDoor => Some(Self::JungleDoorTop),
            Self::WillowDoor => Some(Self::WillowDoorTop),
            Self::AcaciaDoor => Some(Self::AcaciaDoorTop),
            _ => None,
        }
    }

    /// For a trapdoor or a fence gate: the same once opened or shut.
    pub fn toggled(&self) -> Option<BlockId> {
        match *self {
            Self::OakTrapdoor => Some(Self::OpenOakTrapdoor),
            Self::OpenOakTrapdoor => Some(Self::OakTrapdoor),
            Self::OakFenceGate => Some(Self::OpenOakFenceGate),
            Self::OpenOakFenceGate => Some(Self::OakFenceGate),
            Self::SpruceTrapdoor => Some(Self::OpenSpruceTrapdoor),
            Self::OpenSpruceTrapdoor => Some(Self::SpruceTrapdoor),
            Self::SpruceFenceGate => Some(Self::OpenSpruceFenceGate),
            Self::OpenSpruceFenceGate => Some(Self::SpruceFenceGate),
            Self::JungleTrapdoor => Some(Self::OpenJungleTrapdoor),
            Self::OpenJungleTrapdoor => Some(Self::JungleTrapdoor),
            Self::JungleFenceGate => Some(Self::OpenJungleFenceGate),
            Self::OpenJungleFenceGate => Some(Self::JungleFenceGate),
            Self::WillowTrapdoor => Some(Self::OpenWillowTrapdoor),
            Self::OpenWillowTrapdoor => Some(Self::WillowTrapdoor),
            Self::WillowFenceGate => Some(Self::OpenWillowFenceGate),
            Self::OpenWillowFenceGate => Some(Self::WillowFenceGate),
            Self::AcaciaTrapdoor => Some(Self::OpenAcaciaTrapdoor),
            Self::OpenAcaciaTrapdoor => Some(Self::AcaciaTrapdoor),
            Self::AcaciaFenceGate => Some(Self::OpenAcaciaFenceGate),
            Self::OpenAcaciaFenceGate => Some(Self::AcaciaFenceGate),
            _ => None,
        }
    }

    /// For a concrete powder: the concrete it hardens into where water touches it.
    pub fn hardened(&self) -> Option<BlockId> {
        match *self {
            Self::WhiteConcretePowder => Some(Self::WhiteConcrete),
            Self::OrangeConcretePowder => Some(Self::OrangeConcrete),
            Self::MagentaConcretePowder => Some(Self::MagentaConcrete),
            Self::LightBlueConcretePowder => Some(Self::LightBlueConcrete),
            Self::YellowConcretePowder => Some(Self::YellowConcrete),
            Self::LimeConcretePowder => Some(Self::LimeConcrete),
            Self::PinkConcretePowder => Some(Self::PinkConcrete),
            Self::GrayConcretePowder => Some(Self::GrayConcrete),
            Self::LightGrayConcretePowder => Some(Self::LightGrayConcrete),
            Self::CyanConcretePowder => Some(Self::CyanConcrete),
            Self::PurpleConcretePowder => Some(Self::PurpleConcrete),
            Self::BlueConcretePowder => Some(Self::BlueConcrete),
            Self::BrownConcretePowder => Some(Self::BrownConcrete),
            Self::GreenConcretePowder => Some(Self::GreenConcrete),
            Self::RedConcretePowder => Some(Self::RedConcrete),
            Self::BlackConcretePowder => Some(Self::BlackConcrete),
            _ => None,
        }
    }
}

impl ItemId {
    /// The block a new item of the palette places, `None` for the dyes and the other items.
    pub fn palette_block(self) -> Option<BlockId> {
        match self {
            Self::SpruceSlab => Some(BlockId::SpruceSlab),
            Self::JungleSlab => Some(BlockId::JungleSlab),
            Self::WillowSlab => Some(BlockId::WillowSlab),
            Self::AcaciaSlab => Some(BlockId::AcaciaSlab),
            Self::SpruceStairs => Some(BlockId::SpruceStairs),
            Self::JungleStairs => Some(BlockId::JungleStairs),
            Self::WillowStairs => Some(BlockId::WillowStairs),
            Self::AcaciaStairs => Some(BlockId::AcaciaStairs),
            Self::SpruceDoor => Some(BlockId::SpruceDoor),
            Self::JungleDoor => Some(BlockId::JungleDoor),
            Self::WillowDoor => Some(BlockId::WillowDoor),
            Self::AcaciaDoor => Some(BlockId::AcaciaDoor),
            Self::OakTrapdoor => Some(BlockId::OakTrapdoor),
            Self::SpruceTrapdoor => Some(BlockId::SpruceTrapdoor),
            Self::JungleTrapdoor => Some(BlockId::JungleTrapdoor),
            Self::WillowTrapdoor => Some(BlockId::WillowTrapdoor),
            Self::AcaciaTrapdoor => Some(BlockId::AcaciaTrapdoor),
            Self::OakFenceGate => Some(BlockId::OakFenceGate),
            Self::SpruceFenceGate => Some(BlockId::SpruceFenceGate),
            Self::JungleFenceGate => Some(BlockId::JungleFenceGate),
            Self::WillowFenceGate => Some(BlockId::WillowFenceGate),
            Self::AcaciaFenceGate => Some(BlockId::AcaciaFenceGate),
            Self::StoneSlab => Some(BlockId::StoneSlab),
            Self::CobblestoneSlab => Some(BlockId::CobblestoneSlab),
            Self::LimestoneSlab => Some(BlockId::LimestoneSlab),
            Self::SlateSlab => Some(BlockId::SlateSlab),
            Self::GloomrockSlab => Some(BlockId::GloomrockSlab),
            Self::CobbledGloomrockSlab => Some(BlockId::CobbledGloomrockSlab),
            Self::StoneStairs => Some(BlockId::StoneStairs),
            Self::CobblestoneStairs => Some(BlockId::CobblestoneStairs),
            Self::LimestoneStairs => Some(BlockId::LimestoneStairs),
            Self::SlateStairs => Some(BlockId::SlateStairs),
            Self::GloomrockStairs => Some(BlockId::GloomrockStairs),
            Self::CobbledGloomrockStairs => Some(BlockId::CobbledGloomrockStairs),
            Self::StoneLowWall => Some(BlockId::StoneLowWall),
            Self::CobblestoneLowWall => Some(BlockId::CobblestoneLowWall),
            Self::HewnStoneLowWall => Some(BlockId::HewnStoneLowWall),
            Self::LimestoneLowWall => Some(BlockId::LimestoneLowWall),
            Self::SlateLowWall => Some(BlockId::SlateLowWall),
            Self::BrickLowWall => Some(BlockId::BrickLowWall),
            Self::GloomrockLowWall => Some(BlockId::GloomrockLowWall),
            Self::CobbledGloomrockLowWall => Some(BlockId::CobbledGloomrockLowWall),
            Self::PolishedStone => Some(BlockId::PolishedStone),
            Self::PolishedLimestone => Some(BlockId::PolishedLimestone),
            Self::PolishedSlate => Some(BlockId::PolishedSlate),
            Self::PolishedGloomrock => Some(BlockId::PolishedGloomrock),
            Self::ChiseledStone => Some(BlockId::ChiseledStone),
            Self::ChiseledLimestone => Some(BlockId::ChiseledLimestone),
            Self::ChiseledSlate => Some(BlockId::ChiseledSlate),
            Self::ChiseledGloomrock => Some(BlockId::ChiseledGloomrock),
            Self::OrangeWool => Some(BlockId::OrangeWool),
            Self::MagentaWool => Some(BlockId::MagentaWool),
            Self::LightBlueWool => Some(BlockId::LightBlueWool),
            Self::YellowWool => Some(BlockId::YellowWool),
            Self::LimeWool => Some(BlockId::LimeWool),
            Self::PinkWool => Some(BlockId::PinkWool),
            Self::GrayWool => Some(BlockId::GrayWool),
            Self::LightGrayWool => Some(BlockId::LightGrayWool),
            Self::CyanWool => Some(BlockId::CyanWool),
            Self::PurpleWool => Some(BlockId::PurpleWool),
            Self::BlueWool => Some(BlockId::BlueWool),
            Self::BrownWool => Some(BlockId::BrownWool),
            Self::GreenWool => Some(BlockId::GreenWool),
            Self::RedWool => Some(BlockId::RedWool),
            Self::BlackWool => Some(BlockId::BlackWool),
            Self::WhiteStainedGlass => Some(BlockId::WhiteStainedGlass),
            Self::OrangeStainedGlass => Some(BlockId::OrangeStainedGlass),
            Self::MagentaStainedGlass => Some(BlockId::MagentaStainedGlass),
            Self::LightBlueStainedGlass => Some(BlockId::LightBlueStainedGlass),
            Self::YellowStainedGlass => Some(BlockId::YellowStainedGlass),
            Self::LimeStainedGlass => Some(BlockId::LimeStainedGlass),
            Self::PinkStainedGlass => Some(BlockId::PinkStainedGlass),
            Self::GrayStainedGlass => Some(BlockId::GrayStainedGlass),
            Self::LightGrayStainedGlass => Some(BlockId::LightGrayStainedGlass),
            Self::CyanStainedGlass => Some(BlockId::CyanStainedGlass),
            Self::PurpleStainedGlass => Some(BlockId::PurpleStainedGlass),
            Self::BlueStainedGlass => Some(BlockId::BlueStainedGlass),
            Self::BrownStainedGlass => Some(BlockId::BrownStainedGlass),
            Self::GreenStainedGlass => Some(BlockId::GreenStainedGlass),
            Self::RedStainedGlass => Some(BlockId::RedStainedGlass),
            Self::BlackStainedGlass => Some(BlockId::BlackStainedGlass),
            Self::WhiteStainedGlassPane => Some(BlockId::WhiteStainedGlassPane),
            Self::OrangeStainedGlassPane => Some(BlockId::OrangeStainedGlassPane),
            Self::MagentaStainedGlassPane => Some(BlockId::MagentaStainedGlassPane),
            Self::LightBlueStainedGlassPane => Some(BlockId::LightBlueStainedGlassPane),
            Self::YellowStainedGlassPane => Some(BlockId::YellowStainedGlassPane),
            Self::LimeStainedGlassPane => Some(BlockId::LimeStainedGlassPane),
            Self::PinkStainedGlassPane => Some(BlockId::PinkStainedGlassPane),
            Self::GrayStainedGlassPane => Some(BlockId::GrayStainedGlassPane),
            Self::LightGrayStainedGlassPane => Some(BlockId::LightGrayStainedGlassPane),
            Self::CyanStainedGlassPane => Some(BlockId::CyanStainedGlassPane),
            Self::PurpleStainedGlassPane => Some(BlockId::PurpleStainedGlassPane),
            Self::BlueStainedGlassPane => Some(BlockId::BlueStainedGlassPane),
            Self::BrownStainedGlassPane => Some(BlockId::BrownStainedGlassPane),
            Self::GreenStainedGlassPane => Some(BlockId::GreenStainedGlassPane),
            Self::RedStainedGlassPane => Some(BlockId::RedStainedGlassPane),
            Self::BlackStainedGlassPane => Some(BlockId::BlackStainedGlassPane),
            Self::Terracotta => Some(BlockId::Terracotta),
            Self::WhiteTerracotta => Some(BlockId::WhiteTerracotta),
            Self::OrangeTerracotta => Some(BlockId::OrangeTerracotta),
            Self::MagentaTerracotta => Some(BlockId::MagentaTerracotta),
            Self::LightBlueTerracotta => Some(BlockId::LightBlueTerracotta),
            Self::YellowTerracotta => Some(BlockId::YellowTerracotta),
            Self::LimeTerracotta => Some(BlockId::LimeTerracotta),
            Self::PinkTerracotta => Some(BlockId::PinkTerracotta),
            Self::GrayTerracotta => Some(BlockId::GrayTerracotta),
            Self::LightGrayTerracotta => Some(BlockId::LightGrayTerracotta),
            Self::CyanTerracotta => Some(BlockId::CyanTerracotta),
            Self::PurpleTerracotta => Some(BlockId::PurpleTerracotta),
            Self::BlueTerracotta => Some(BlockId::BlueTerracotta),
            Self::BrownTerracotta => Some(BlockId::BrownTerracotta),
            Self::GreenTerracotta => Some(BlockId::GreenTerracotta),
            Self::RedTerracotta => Some(BlockId::RedTerracotta),
            Self::BlackTerracotta => Some(BlockId::BlackTerracotta),
            Self::WhiteConcrete => Some(BlockId::WhiteConcrete),
            Self::OrangeConcrete => Some(BlockId::OrangeConcrete),
            Self::MagentaConcrete => Some(BlockId::MagentaConcrete),
            Self::LightBlueConcrete => Some(BlockId::LightBlueConcrete),
            Self::YellowConcrete => Some(BlockId::YellowConcrete),
            Self::LimeConcrete => Some(BlockId::LimeConcrete),
            Self::PinkConcrete => Some(BlockId::PinkConcrete),
            Self::GrayConcrete => Some(BlockId::GrayConcrete),
            Self::LightGrayConcrete => Some(BlockId::LightGrayConcrete),
            Self::CyanConcrete => Some(BlockId::CyanConcrete),
            Self::PurpleConcrete => Some(BlockId::PurpleConcrete),
            Self::BlueConcrete => Some(BlockId::BlueConcrete),
            Self::BrownConcrete => Some(BlockId::BrownConcrete),
            Self::GreenConcrete => Some(BlockId::GreenConcrete),
            Self::RedConcrete => Some(BlockId::RedConcrete),
            Self::BlackConcrete => Some(BlockId::BlackConcrete),
            Self::WhiteConcretePowder => Some(BlockId::WhiteConcretePowder),
            Self::OrangeConcretePowder => Some(BlockId::OrangeConcretePowder),
            Self::MagentaConcretePowder => Some(BlockId::MagentaConcretePowder),
            Self::LightBlueConcretePowder => Some(BlockId::LightBlueConcretePowder),
            Self::YellowConcretePowder => Some(BlockId::YellowConcretePowder),
            Self::LimeConcretePowder => Some(BlockId::LimeConcretePowder),
            Self::PinkConcretePowder => Some(BlockId::PinkConcretePowder),
            Self::GrayConcretePowder => Some(BlockId::GrayConcretePowder),
            Self::LightGrayConcretePowder => Some(BlockId::LightGrayConcretePowder),
            Self::CyanConcretePowder => Some(BlockId::CyanConcretePowder),
            Self::PurpleConcretePowder => Some(BlockId::PurpleConcretePowder),
            Self::BlueConcretePowder => Some(BlockId::BlueConcretePowder),
            Self::BrownConcretePowder => Some(BlockId::BrownConcretePowder),
            Self::GreenConcretePowder => Some(BlockId::GreenConcretePowder),
            Self::RedConcretePowder => Some(BlockId::RedConcretePowder),
            Self::BlackConcretePowder => Some(BlockId::BlackConcretePowder),
            _ => None,
        }
    }
}

/// The blocks added with the palette, as a pattern: for the matches over every block.
#[macro_export]
macro_rules! new_palette_blocks {
    () => {
        $crate::world::BlockId::SpruceSlab
            | $crate::world::BlockId::JungleSlab
            | $crate::world::BlockId::WillowSlab
            | $crate::world::BlockId::AcaciaSlab
            | $crate::world::BlockId::SpruceStairs
            | $crate::world::BlockId::JungleStairs
            | $crate::world::BlockId::WillowStairs
            | $crate::world::BlockId::AcaciaStairs
            | $crate::world::BlockId::SpruceDoor
            | $crate::world::BlockId::SpruceDoorTop
            | $crate::world::BlockId::OpenSpruceDoor
            | $crate::world::BlockId::OpenSpruceDoorTop
            | $crate::world::BlockId::JungleDoor
            | $crate::world::BlockId::JungleDoorTop
            | $crate::world::BlockId::OpenJungleDoor
            | $crate::world::BlockId::OpenJungleDoorTop
            | $crate::world::BlockId::WillowDoor
            | $crate::world::BlockId::WillowDoorTop
            | $crate::world::BlockId::OpenWillowDoor
            | $crate::world::BlockId::OpenWillowDoorTop
            | $crate::world::BlockId::AcaciaDoor
            | $crate::world::BlockId::AcaciaDoorTop
            | $crate::world::BlockId::OpenAcaciaDoor
            | $crate::world::BlockId::OpenAcaciaDoorTop
            | $crate::world::BlockId::OakTrapdoor
            | $crate::world::BlockId::OpenOakTrapdoor
            | $crate::world::BlockId::SpruceTrapdoor
            | $crate::world::BlockId::OpenSpruceTrapdoor
            | $crate::world::BlockId::JungleTrapdoor
            | $crate::world::BlockId::OpenJungleTrapdoor
            | $crate::world::BlockId::WillowTrapdoor
            | $crate::world::BlockId::OpenWillowTrapdoor
            | $crate::world::BlockId::AcaciaTrapdoor
            | $crate::world::BlockId::OpenAcaciaTrapdoor
            | $crate::world::BlockId::OakFenceGate
            | $crate::world::BlockId::OpenOakFenceGate
            | $crate::world::BlockId::SpruceFenceGate
            | $crate::world::BlockId::OpenSpruceFenceGate
            | $crate::world::BlockId::JungleFenceGate
            | $crate::world::BlockId::OpenJungleFenceGate
            | $crate::world::BlockId::WillowFenceGate
            | $crate::world::BlockId::OpenWillowFenceGate
            | $crate::world::BlockId::AcaciaFenceGate
            | $crate::world::BlockId::OpenAcaciaFenceGate
            | $crate::world::BlockId::StoneSlab
            | $crate::world::BlockId::CobblestoneSlab
            | $crate::world::BlockId::LimestoneSlab
            | $crate::world::BlockId::SlateSlab
            | $crate::world::BlockId::GloomrockSlab
            | $crate::world::BlockId::CobbledGloomrockSlab
            | $crate::world::BlockId::StoneStairs
            | $crate::world::BlockId::CobblestoneStairs
            | $crate::world::BlockId::LimestoneStairs
            | $crate::world::BlockId::SlateStairs
            | $crate::world::BlockId::GloomrockStairs
            | $crate::world::BlockId::CobbledGloomrockStairs
            | $crate::world::BlockId::StoneLowWall
            | $crate::world::BlockId::CobblestoneLowWall
            | $crate::world::BlockId::HewnStoneLowWall
            | $crate::world::BlockId::LimestoneLowWall
            | $crate::world::BlockId::SlateLowWall
            | $crate::world::BlockId::BrickLowWall
            | $crate::world::BlockId::GloomrockLowWall
            | $crate::world::BlockId::CobbledGloomrockLowWall
            | $crate::world::BlockId::PolishedStone
            | $crate::world::BlockId::PolishedLimestone
            | $crate::world::BlockId::PolishedSlate
            | $crate::world::BlockId::PolishedGloomrock
            | $crate::world::BlockId::ChiseledStone
            | $crate::world::BlockId::ChiseledLimestone
            | $crate::world::BlockId::ChiseledSlate
            | $crate::world::BlockId::ChiseledGloomrock
            | $crate::world::BlockId::OrangeWool
            | $crate::world::BlockId::MagentaWool
            | $crate::world::BlockId::LightBlueWool
            | $crate::world::BlockId::YellowWool
            | $crate::world::BlockId::LimeWool
            | $crate::world::BlockId::PinkWool
            | $crate::world::BlockId::GrayWool
            | $crate::world::BlockId::LightGrayWool
            | $crate::world::BlockId::CyanWool
            | $crate::world::BlockId::PurpleWool
            | $crate::world::BlockId::BlueWool
            | $crate::world::BlockId::BrownWool
            | $crate::world::BlockId::GreenWool
            | $crate::world::BlockId::RedWool
            | $crate::world::BlockId::BlackWool
            | $crate::world::BlockId::WhiteStainedGlass
            | $crate::world::BlockId::OrangeStainedGlass
            | $crate::world::BlockId::MagentaStainedGlass
            | $crate::world::BlockId::LightBlueStainedGlass
            | $crate::world::BlockId::YellowStainedGlass
            | $crate::world::BlockId::LimeStainedGlass
            | $crate::world::BlockId::PinkStainedGlass
            | $crate::world::BlockId::GrayStainedGlass
            | $crate::world::BlockId::LightGrayStainedGlass
            | $crate::world::BlockId::CyanStainedGlass
            | $crate::world::BlockId::PurpleStainedGlass
            | $crate::world::BlockId::BlueStainedGlass
            | $crate::world::BlockId::BrownStainedGlass
            | $crate::world::BlockId::GreenStainedGlass
            | $crate::world::BlockId::RedStainedGlass
            | $crate::world::BlockId::BlackStainedGlass
            | $crate::world::BlockId::WhiteStainedGlassPane
            | $crate::world::BlockId::OrangeStainedGlassPane
            | $crate::world::BlockId::MagentaStainedGlassPane
            | $crate::world::BlockId::LightBlueStainedGlassPane
            | $crate::world::BlockId::YellowStainedGlassPane
            | $crate::world::BlockId::LimeStainedGlassPane
            | $crate::world::BlockId::PinkStainedGlassPane
            | $crate::world::BlockId::GrayStainedGlassPane
            | $crate::world::BlockId::LightGrayStainedGlassPane
            | $crate::world::BlockId::CyanStainedGlassPane
            | $crate::world::BlockId::PurpleStainedGlassPane
            | $crate::world::BlockId::BlueStainedGlassPane
            | $crate::world::BlockId::BrownStainedGlassPane
            | $crate::world::BlockId::GreenStainedGlassPane
            | $crate::world::BlockId::RedStainedGlassPane
            | $crate::world::BlockId::BlackStainedGlassPane
            | $crate::world::BlockId::Terracotta
            | $crate::world::BlockId::WhiteTerracotta
            | $crate::world::BlockId::OrangeTerracotta
            | $crate::world::BlockId::MagentaTerracotta
            | $crate::world::BlockId::LightBlueTerracotta
            | $crate::world::BlockId::YellowTerracotta
            | $crate::world::BlockId::LimeTerracotta
            | $crate::world::BlockId::PinkTerracotta
            | $crate::world::BlockId::GrayTerracotta
            | $crate::world::BlockId::LightGrayTerracotta
            | $crate::world::BlockId::CyanTerracotta
            | $crate::world::BlockId::PurpleTerracotta
            | $crate::world::BlockId::BlueTerracotta
            | $crate::world::BlockId::BrownTerracotta
            | $crate::world::BlockId::GreenTerracotta
            | $crate::world::BlockId::RedTerracotta
            | $crate::world::BlockId::BlackTerracotta
            | $crate::world::BlockId::WhiteConcrete
            | $crate::world::BlockId::OrangeConcrete
            | $crate::world::BlockId::MagentaConcrete
            | $crate::world::BlockId::LightBlueConcrete
            | $crate::world::BlockId::YellowConcrete
            | $crate::world::BlockId::LimeConcrete
            | $crate::world::BlockId::PinkConcrete
            | $crate::world::BlockId::GrayConcrete
            | $crate::world::BlockId::LightGrayConcrete
            | $crate::world::BlockId::CyanConcrete
            | $crate::world::BlockId::PurpleConcrete
            | $crate::world::BlockId::BlueConcrete
            | $crate::world::BlockId::BrownConcrete
            | $crate::world::BlockId::GreenConcrete
            | $crate::world::BlockId::RedConcrete
            | $crate::world::BlockId::BlackConcrete
            | $crate::world::BlockId::WhiteConcretePowder
            | $crate::world::BlockId::OrangeConcretePowder
            | $crate::world::BlockId::MagentaConcretePowder
            | $crate::world::BlockId::LightBlueConcretePowder
            | $crate::world::BlockId::YellowConcretePowder
            | $crate::world::BlockId::LimeConcretePowder
            | $crate::world::BlockId::PinkConcretePowder
            | $crate::world::BlockId::GrayConcretePowder
            | $crate::world::BlockId::LightGrayConcretePowder
            | $crate::world::BlockId::CyanConcretePowder
            | $crate::world::BlockId::PurpleConcretePowder
            | $crate::world::BlockId::BlueConcretePowder
            | $crate::world::BlockId::BrownConcretePowder
            | $crate::world::BlockId::GreenConcretePowder
            | $crate::world::BlockId::RedConcretePowder
            | $crate::world::BlockId::BlackConcretePowder
    };
}

/// The items added with the palette that place a block, as a pattern.
#[macro_export]
macro_rules! new_palette_block_items {
    () => {
        $crate::world::ItemId::SpruceSlab
            | $crate::world::ItemId::JungleSlab
            | $crate::world::ItemId::WillowSlab
            | $crate::world::ItemId::AcaciaSlab
            | $crate::world::ItemId::SpruceStairs
            | $crate::world::ItemId::JungleStairs
            | $crate::world::ItemId::WillowStairs
            | $crate::world::ItemId::AcaciaStairs
            | $crate::world::ItemId::SpruceDoor
            | $crate::world::ItemId::JungleDoor
            | $crate::world::ItemId::WillowDoor
            | $crate::world::ItemId::AcaciaDoor
            | $crate::world::ItemId::OakTrapdoor
            | $crate::world::ItemId::SpruceTrapdoor
            | $crate::world::ItemId::JungleTrapdoor
            | $crate::world::ItemId::WillowTrapdoor
            | $crate::world::ItemId::AcaciaTrapdoor
            | $crate::world::ItemId::OakFenceGate
            | $crate::world::ItemId::SpruceFenceGate
            | $crate::world::ItemId::JungleFenceGate
            | $crate::world::ItemId::WillowFenceGate
            | $crate::world::ItemId::AcaciaFenceGate
            | $crate::world::ItemId::StoneSlab
            | $crate::world::ItemId::CobblestoneSlab
            | $crate::world::ItemId::LimestoneSlab
            | $crate::world::ItemId::SlateSlab
            | $crate::world::ItemId::GloomrockSlab
            | $crate::world::ItemId::CobbledGloomrockSlab
            | $crate::world::ItemId::StoneStairs
            | $crate::world::ItemId::CobblestoneStairs
            | $crate::world::ItemId::LimestoneStairs
            | $crate::world::ItemId::SlateStairs
            | $crate::world::ItemId::GloomrockStairs
            | $crate::world::ItemId::CobbledGloomrockStairs
            | $crate::world::ItemId::StoneLowWall
            | $crate::world::ItemId::CobblestoneLowWall
            | $crate::world::ItemId::HewnStoneLowWall
            | $crate::world::ItemId::LimestoneLowWall
            | $crate::world::ItemId::SlateLowWall
            | $crate::world::ItemId::BrickLowWall
            | $crate::world::ItemId::GloomrockLowWall
            | $crate::world::ItemId::CobbledGloomrockLowWall
            | $crate::world::ItemId::PolishedStone
            | $crate::world::ItemId::PolishedLimestone
            | $crate::world::ItemId::PolishedSlate
            | $crate::world::ItemId::PolishedGloomrock
            | $crate::world::ItemId::ChiseledStone
            | $crate::world::ItemId::ChiseledLimestone
            | $crate::world::ItemId::ChiseledSlate
            | $crate::world::ItemId::ChiseledGloomrock
            | $crate::world::ItemId::OrangeWool
            | $crate::world::ItemId::MagentaWool
            | $crate::world::ItemId::LightBlueWool
            | $crate::world::ItemId::YellowWool
            | $crate::world::ItemId::LimeWool
            | $crate::world::ItemId::PinkWool
            | $crate::world::ItemId::GrayWool
            | $crate::world::ItemId::LightGrayWool
            | $crate::world::ItemId::CyanWool
            | $crate::world::ItemId::PurpleWool
            | $crate::world::ItemId::BlueWool
            | $crate::world::ItemId::BrownWool
            | $crate::world::ItemId::GreenWool
            | $crate::world::ItemId::RedWool
            | $crate::world::ItemId::BlackWool
            | $crate::world::ItemId::WhiteStainedGlass
            | $crate::world::ItemId::OrangeStainedGlass
            | $crate::world::ItemId::MagentaStainedGlass
            | $crate::world::ItemId::LightBlueStainedGlass
            | $crate::world::ItemId::YellowStainedGlass
            | $crate::world::ItemId::LimeStainedGlass
            | $crate::world::ItemId::PinkStainedGlass
            | $crate::world::ItemId::GrayStainedGlass
            | $crate::world::ItemId::LightGrayStainedGlass
            | $crate::world::ItemId::CyanStainedGlass
            | $crate::world::ItemId::PurpleStainedGlass
            | $crate::world::ItemId::BlueStainedGlass
            | $crate::world::ItemId::BrownStainedGlass
            | $crate::world::ItemId::GreenStainedGlass
            | $crate::world::ItemId::RedStainedGlass
            | $crate::world::ItemId::BlackStainedGlass
            | $crate::world::ItemId::WhiteStainedGlassPane
            | $crate::world::ItemId::OrangeStainedGlassPane
            | $crate::world::ItemId::MagentaStainedGlassPane
            | $crate::world::ItemId::LightBlueStainedGlassPane
            | $crate::world::ItemId::YellowStainedGlassPane
            | $crate::world::ItemId::LimeStainedGlassPane
            | $crate::world::ItemId::PinkStainedGlassPane
            | $crate::world::ItemId::GrayStainedGlassPane
            | $crate::world::ItemId::LightGrayStainedGlassPane
            | $crate::world::ItemId::CyanStainedGlassPane
            | $crate::world::ItemId::PurpleStainedGlassPane
            | $crate::world::ItemId::BlueStainedGlassPane
            | $crate::world::ItemId::BrownStainedGlassPane
            | $crate::world::ItemId::GreenStainedGlassPane
            | $crate::world::ItemId::RedStainedGlassPane
            | $crate::world::ItemId::BlackStainedGlassPane
            | $crate::world::ItemId::Terracotta
            | $crate::world::ItemId::WhiteTerracotta
            | $crate::world::ItemId::OrangeTerracotta
            | $crate::world::ItemId::MagentaTerracotta
            | $crate::world::ItemId::LightBlueTerracotta
            | $crate::world::ItemId::YellowTerracotta
            | $crate::world::ItemId::LimeTerracotta
            | $crate::world::ItemId::PinkTerracotta
            | $crate::world::ItemId::GrayTerracotta
            | $crate::world::ItemId::LightGrayTerracotta
            | $crate::world::ItemId::CyanTerracotta
            | $crate::world::ItemId::PurpleTerracotta
            | $crate::world::ItemId::BlueTerracotta
            | $crate::world::ItemId::BrownTerracotta
            | $crate::world::ItemId::GreenTerracotta
            | $crate::world::ItemId::RedTerracotta
            | $crate::world::ItemId::BlackTerracotta
            | $crate::world::ItemId::WhiteConcrete
            | $crate::world::ItemId::OrangeConcrete
            | $crate::world::ItemId::MagentaConcrete
            | $crate::world::ItemId::LightBlueConcrete
            | $crate::world::ItemId::YellowConcrete
            | $crate::world::ItemId::LimeConcrete
            | $crate::world::ItemId::PinkConcrete
            | $crate::world::ItemId::GrayConcrete
            | $crate::world::ItemId::LightGrayConcrete
            | $crate::world::ItemId::CyanConcrete
            | $crate::world::ItemId::PurpleConcrete
            | $crate::world::ItemId::BlueConcrete
            | $crate::world::ItemId::BrownConcrete
            | $crate::world::ItemId::GreenConcrete
            | $crate::world::ItemId::RedConcrete
            | $crate::world::ItemId::BlackConcrete
            | $crate::world::ItemId::WhiteConcretePowder
            | $crate::world::ItemId::OrangeConcretePowder
            | $crate::world::ItemId::MagentaConcretePowder
            | $crate::world::ItemId::LightBlueConcretePowder
            | $crate::world::ItemId::YellowConcretePowder
            | $crate::world::ItemId::LimeConcretePowder
            | $crate::world::ItemId::PinkConcretePowder
            | $crate::world::ItemId::GrayConcretePowder
            | $crate::world::ItemId::LightGrayConcretePowder
            | $crate::world::ItemId::CyanConcretePowder
            | $crate::world::ItemId::PurpleConcretePowder
            | $crate::world::ItemId::BlueConcretePowder
            | $crate::world::ItemId::BrownConcretePowder
            | $crate::world::ItemId::GreenConcretePowder
            | $crate::world::ItemId::RedConcretePowder
            | $crate::world::ItemId::BlackConcretePowder
    };
}

/// The dyes, as a pattern.
#[macro_export]
macro_rules! dye_items {
    () => {
        $crate::world::ItemId::WhiteDye
            | $crate::world::ItemId::OrangeDye
            | $crate::world::ItemId::MagentaDye
            | $crate::world::ItemId::LightBlueDye
            | $crate::world::ItemId::YellowDye
            | $crate::world::ItemId::LimeDye
            | $crate::world::ItemId::PinkDye
            | $crate::world::ItemId::GrayDye
            | $crate::world::ItemId::LightGrayDye
            | $crate::world::ItemId::CyanDye
            | $crate::world::ItemId::PurpleDye
            | $crate::world::ItemId::BlueDye
            | $crate::world::ItemId::BrownDye
            | $crate::world::ItemId::GreenDye
            | $crate::world::ItemId::RedDye
            | $crate::world::ItemId::BlackDye
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::ItemType;

    #[test]
    fn the_wooden_slab_and_stairs_of_old_worlds_load_as_those_of_oak() {
        use bincode::Options;
        // chunk files and network messages hold the position of the variant
        let block = |byte: u8| bincode::options().deserialize::<BlockId>(&[byte]).unwrap();
        assert_eq!(block(62), BlockId::OakSlab);
        assert_eq!(block(66), BlockId::OakStairs);
        let item = |byte: u8| bincode::options().deserialize::<ItemId>(&[byte]).unwrap();
        assert_eq!(item(103), ItemId::OakSlab);
        assert_eq!(item(120), ItemId::OakStairs);
        // player files and worlds saved in RON hold the name of the variant
        let as_ron = ron::to_string(&ItemId::OakSlab).unwrap();
        assert_eq!(
            ron::from_str::<ItemId>(&as_ron).unwrap(),
            ItemId::OakSlab,
            "{as_ron}"
        );
        let old = ron::from_str::<ItemId>("WoodenSlab");
        assert!(old.is_ok(), "{old:?}");
        assert_eq!(
            ron::from_str::<ItemId>("WoodenSlab").unwrap(),
            ItemId::OakSlab
        );
        assert_eq!(
            ron::from_str::<ItemId>("WoodenStairs").unwrap(),
            ItemId::OakStairs
        );
        assert_eq!(
            ron::from_str::<BlockId>("WoodenSlab").unwrap(),
            BlockId::OakSlab
        );
        assert_eq!(
            ron::from_str::<BlockId>("WoodenStairs").unwrap(),
            BlockId::OakStairs
        );
        // and the commands, the name of the item
        assert_eq!(ItemId::from_name("wooden_slab"), Some(ItemId::OakSlab));
        assert_eq!(ItemId::from_name("wooden_stairs"), Some(ItemId::OakStairs));
        assert_eq!(ItemId::OakSlab.name(), "oak_slab");
        // what they were stays what they are
        let slab = BlockId::OakSlab.palette().unwrap();
        assert_eq!((slab.form, slab.base), (Form::Slab, BlockId::OakPlanks));
    }

    #[test]
    fn every_block_of_the_palette_is_placed_by_its_item_and_drops_it_or_nothing() {
        for id in BlockId::ALL {
            let Some(palette) = id.palette() else {
                continue;
            };
            assert!(palette.base.palette().is_none(), "{id:?}");
            let placed = match palette.item.get_default_type() {
                ItemType::Block(placed) => placed,
                other => panic!("{id:?}: {other:?}"),
            };
            assert_eq!(
                placed.palette().map(|p| p.item),
                Some(palette.item),
                "{id:?}"
            );
            let drops = id.get_drop_table();
            if palette.base == BlockId::Glass {
                assert!(drops.is_empty(), "{id:?}");
            } else {
                assert_eq!(drops, vec![(1, palette.item, 1)], "{id:?}");
            }
        }
    }

    #[test]
    fn every_new_item_of_the_palette_places_its_block() {
        for item in ItemId::ALL {
            if let Some(block) = item.palette_block() {
                assert_eq!(item.get_default_type(), ItemType::Block(block), "{item:?}");
                assert_eq!(block.palette().map(|p| p.item), Some(item), "{item:?}");
            }
        }
    }

    #[test]
    fn a_door_a_trapdoor_and_a_gate_open_and_shut_again() {
        for wood in Wood::ALL {
            let ItemType::Block(door) = wood.door().get_default_type() else {
                panic!("{wood:?}");
            };
            let top = door.door_top().unwrap();
            let (open, step) = door.door().unwrap();
            assert_eq!(step, IVec3::Y);
            assert_eq!(open.door(), Some((door, IVec3::Y)));
            assert_eq!(top.door().map(|(_, step)| step), Some(IVec3::NEG_Y));
            for item in [wood.trapdoor(), wood.fence_gate()] {
                let ItemType::Block(shut) = item.get_default_type() else {
                    panic!("{item:?}");
                };
                let open = shut.toggled().unwrap();
                assert_ne!(open, shut);
                assert_eq!(open.toggled(), Some(shut));
                assert!(shut.is_usable() && open.is_usable(), "{item:?}");
            }
        }
    }

    #[test]
    fn each_concrete_powder_hardens_into_the_concrete_of_its_colour() {
        for color in DyeColor::ALL {
            let block = |item: ItemId| match item.get_default_type() {
                ItemType::Block(block) => block,
                other => panic!("{item:?}: {other:?}"),
            };
            assert_eq!(
                block(color.concrete_powder()).hardened(),
                Some(block(color.concrete()))
            );
        }
    }
}
