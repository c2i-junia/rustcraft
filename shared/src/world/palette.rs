//! The palette of the builder: the slabs, stairs, doors, trapdoors and fence gates of each wood,
//! the slabs, stairs and low walls of each rock, the polished and chiseled rocks, and the sixteen
//! dyes with the wool, glass, terracotta and concrete they colour. The content files write them
//! as families (`data/content/rustcraft/palette.ron`, `palette_dyes.ron`); what is left here is
//! what the engine does with them.
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
}

impl BlockId {
    /// What the block is, if it is one of the palette.
    #[inline]
    pub fn palette(self) -> Option<Palette> {
        self.props().palette
    }

    /// For a half of a door: the same half once opened or shut, and the step to the other half.
    pub fn door(&self) -> Option<(BlockId, IVec3)> {
        let props = self.props();
        let Form::Door { top, .. } = props.palette?.form else {
            return None;
        };
        Some((props.toggled?, if top { IVec3::NEG_Y } else { IVec3::Y }))
    }

    /// For the lower half of a shut door: its top half, placed above it.
    pub fn door_top(&self) -> Option<BlockId> {
        self.props().door_top
    }

    /// For a trapdoor or a fence gate: the same once opened or shut.
    pub fn toggled(&self) -> Option<BlockId> {
        let props = self.props();
        match props.palette?.form {
            Form::Trapdoor { .. } | Form::FenceGate { .. } => props.toggled,
            _ => None,
        }
    }

    /// For a concrete powder: the concrete it hardens into where water touches it.
    pub fn hardened(&self) -> Option<BlockId> {
        self.props().hardens
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::ItemType;

    /// The woods of `palette.ron`.
    const WOODS: [&str; 5] = ["oak", "spruce", "jungle", "willow", "acacia"];

    /// The colours of `palette_dyes.ron`.
    const COLOURS: [&str; 16] = [
        "white",
        "orange",
        "magenta",
        "light_blue",
        "yellow",
        "lime",
        "pink",
        "gray",
        "light_gray",
        "cyan",
        "purple",
        "blue",
        "brown",
        "green",
        "red",
        "black",
    ];

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
        for id in BlockId::ENGINE {
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
    fn a_door_a_trapdoor_and_a_gate_open_and_shut_again() {
        let item = |name: String| ItemId::from_name(&name).unwrap_or_else(|| panic!("{name}"));
        for wood in WOODS {
            let ItemType::Block(door) = item(format!("{wood}_door")).get_default_type() else {
                panic!("{wood}");
            };
            let top = door.door_top().unwrap();
            let (open, step) = door.door().unwrap();
            assert_eq!(step, IVec3::Y);
            assert_eq!(open.door(), Some((door, IVec3::Y)));
            assert_eq!(top.door().map(|(_, step)| step), Some(IVec3::NEG_Y));
            assert_eq!(door.toggled(), None, "a door opens by halves");
            for item in [
                item(format!("{wood}_trapdoor")),
                item(format!("{wood}_fence_gate")),
            ] {
                let ItemType::Block(shut) = item.get_default_type() else {
                    panic!("{item:?}");
                };
                let open = shut.toggled().unwrap();
                assert_ne!(open, shut);
                assert_eq!(open.toggled(), Some(shut));
                assert!(shut.is_usable() && open.is_usable(), "{item:?}");
            }
        }
        // the oak door is the door older than the palette
        assert_eq!(ItemId::from_name("oak_door"), Some(ItemId::Door));
    }

    #[test]
    fn each_concrete_powder_hardens_into_the_concrete_of_its_colour() {
        for color in COLOURS {
            let block =
                |name: String| BlockId::from_name(&name).unwrap_or_else(|| panic!("{name}"));
            assert_eq!(
                block(format!("{color}_concrete_powder")).hardened(),
                Some(block(format!("{color}_concrete")))
            );
        }
        assert_eq!(BlockId::Sand.hardened(), None);
    }
}
