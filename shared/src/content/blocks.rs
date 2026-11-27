//! The properties of a block, as the table of `Content` holds them.

use serde::{Deserialize, Serialize};

use super::content;
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
    /// `BlockId::is_usable`.
    pub usable: bool,
    /// `BlockId::is_replaceable`.
    pub replaceable: bool,
    /// `BlockId::is_tall`.
    pub tall: bool,
    /// `BlockId::is_glass`.
    pub glass: bool,
    /// `BlockId::is_container`.
    pub container: bool,
    /// `BlockId::sapling`.
    pub sapling: Option<ItemId>,
    /// `BlockId::soil`.
    pub soil: Option<Vec<BlockId>>,
}

impl BlockId {
    /// The properties of the block in the content of the process.
    #[inline]
    pub fn props(self) -> &'static BlockProps {
        content().block(self)
    }
}

/// The bit of `kind` in `BlockProps::faster_tools`.
#[inline]
pub(crate) fn tool_bit(kind: ToolKind) -> u8 {
    1 << kind as u8
}
