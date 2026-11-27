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

impl BlockProps {
    /// The properties of `id` written in the code.
    pub fn builtin(id: BlockId) -> Self {
        Self {
            palette: id.builtin_palette(),
            visibility: id.builtin_visibility(),
            light_opacity: id.builtin_light_opacity(),
            light_emission: id.builtin_light_emission(),
            break_time: id.builtin_break_time(),
            faster_tools: ToolKind::ALL
                .into_iter()
                .filter(|kind| id.builtin_is_mined_faster_by(*kind))
                .fold(0, |mask, kind| mask | tool_bit(kind)),
            required_tool: id.builtin_required_tool(),
            drops: id.builtin_drop_table(),
            hitbox: id.builtin_hitbox(),
            ray_hitbox: id.builtin_ray_hitbox(),
            usable: id.builtin_is_usable(),
            replaceable: id.builtin_is_replaceable(),
            tall: id.builtin_is_tall(),
            glass: id.builtin_is_glass(),
            container: id.builtin_is_container(),
            sapling: id.builtin_sapling(),
            soil: id.builtin_soil().map(<[BlockId]>::to_vec),
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Each getter reads its own entry of the table: it says what its `builtin_*` says, for
    /// every block.
    #[test]
    fn the_getters_of_every_block_say_what_the_code_says() {
        for id in BlockId::ALL {
            assert_eq!(id.palette(), id.builtin_palette(), "{id:?}");
            assert_eq!(id.get_visibility(), id.builtin_visibility(), "{id:?}");
            assert_eq!(id.light_opacity(), id.builtin_light_opacity(), "{id:?}");
            assert_eq!(id.light_emission(), id.builtin_light_emission(), "{id:?}");
            assert_eq!(id.get_break_time(), id.builtin_break_time(), "{id:?}");
            for kind in ToolKind::ALL {
                assert_eq!(
                    id.is_mined_faster_by(kind),
                    id.builtin_is_mined_faster_by(kind),
                    "{id:?} {kind:?}"
                );
            }
            assert_eq!(id.get_required_tool(), id.builtin_required_tool(), "{id:?}");
            assert_eq!(id.get_drop_table(), id.builtin_drop_table(), "{id:?}");
            assert_eq!(id.get_hitbox(), id.builtin_hitbox(), "{id:?}");
            assert_eq!(id.get_ray_hitbox(), id.builtin_ray_hitbox(), "{id:?}");
            assert_eq!(id.is_usable(), id.builtin_is_usable(), "{id:?}");
            assert_eq!(id.is_replaceable(), id.builtin_is_replaceable(), "{id:?}");
            assert_eq!(id.is_tall(), id.builtin_is_tall(), "{id:?}");
            assert_eq!(id.is_glass(), id.builtin_is_glass(), "{id:?}");
            assert_eq!(id.is_container(), id.builtin_is_container(), "{id:?}");
            assert_eq!(id.sapling(), id.builtin_sapling(), "{id:?}");
            assert_eq!(id.soil(), id.builtin_soil(), "{id:?}");
        }
    }
}
