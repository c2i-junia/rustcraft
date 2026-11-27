//! The properties of a block, as the table of `Content` holds them.

use serde::{Deserialize, Serialize};

use super::{content, Behaviour, BlockFlags, Look};
use crate::world::{
    BlockHitbox, BlockId, BlockTransparency, ItemId, Palette, RequiredTool, ToolKind,
};

/// What a block is and does, as the getters of `BlockId` read it: see each getter.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BlockProps {
    /// `BlockId::palette`.
    pub palette: Option<Palette>,
    /// `BlockId::get_visibility`.
    pub visibility: BlockTransparency,
    /// `BlockId::light_opacity`.
    pub light_opacity: Option<u8>,
    /// `BlockId::light_emission`.
    pub light_emission: u8,
    /// `BlockId::get_break_time`.
    pub break_time: Option<u8>,
    /// `BlockId::is_mined_faster_by`: a bit per `ToolKind`, `1 << kind as u8`.
    pub faster_tools: u8,
    /// `BlockId::get_required_tool`.
    pub required_tool: Option<RequiredTool>,
    /// `BlockId::get_drop_table`.
    pub drops: Vec<(u32, ItemId, u32)>,
    /// `BlockId::get_hitbox`.
    pub hitbox: BlockHitbox,
    /// `BlockId::get_ray_hitbox`.
    pub ray_hitbox: BlockHitbox,
    /// `BlockId::is_usable`, `is_replaceable`, `is_tall`, `is_glass`, `is_container`,
    /// `is_submerged`, `hangs`.
    pub flags: BlockFlags,
    /// `BlockId::behaviour`.
    pub behaviour: Behaviour,
    /// `BlockId::sapling`.
    pub sapling: Option<ItemId>,
    /// `BlockId::soil`.
    pub soil: Option<Vec<BlockId>>,
    /// `BlockId::look`.
    pub look: Look,
}

impl BlockProps {
    /// What a block is that a world holds but the content no longer defines, made from the
    /// properties of `BlockId::Debug`, `debug`: it looks like it, breaks as it does, and drops
    /// nothing. Its name is kept, so that the world saves it back as it was.
    pub fn placeholder(debug: &BlockProps) -> Self {
        Self {
            drops: Vec::new(),
            ..debug.clone()
        }
    }
}

impl BlockId {
    /// Every block of the content of the process, by number: those of the engine first.
    pub fn all() -> impl Iterator<Item = BlockId> {
        (0..content().blocks.len()).map(|index| BlockId::from_index(index as u16))
    }

    /// The block named `name` in the content of the process, as the files name it: `oak_log`.
    pub fn from_name(name: &str) -> Option<BlockId> {
        content().block_named(name)
    }

    /// The name of the block in the content of the process, as the files name it: `oak_log`.
    /// `None` for a number the content does not define.
    pub fn name(self) -> Option<&'static str> {
        content().block_names.get(self.index()).map(String::as_str)
    }

    /// The properties of the block in the content of the process.
    #[inline]
    pub fn props(self) -> &'static BlockProps {
        content().block(self)
    }

    /// Whether the block has every flag of `flags`.
    #[inline]
    pub fn has(self, flags: BlockFlags) -> bool {
        self.props().flags.contains(flags)
    }

    /// What the engine does with the block beyond its properties.
    #[inline]
    pub fn behaviour(self) -> Behaviour {
        self.props().behaviour
    }

    /// How the block is drawn.
    #[inline]
    pub fn look(self) -> &'static Look {
        &self.props().look
    }
}

/// The bit of `kind` in `BlockProps::faster_tools`.
#[inline]
pub(crate) fn tool_bit(kind: ToolKind) -> u8 {
    1 << kind as u8
}
