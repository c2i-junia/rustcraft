//! The levels of a liquid, stored in `BlockData::level`: a source is at level 0, the liquid
//! that flows away from it on the ground at levels 1 to 7, and the liquid that falls at level 8.

use bevy::math::IVec3;

use crate::world::{BlockData, BlockTransparency, ChunkMap};

/// Level of a source.
pub const SOURCE_LEVEL: u8 = 0;
/// Level of the thinnest flowing liquid: it spreads no further.
pub const MAX_FLOW_LEVEL: u8 = 7;
/// Level of a liquid fed from above.
pub const FALLING_LEVEL: u8 = 8;

/// Height of the surface of a liquid at `level` with no liquid above it, in blocks: 8/9 for a
/// source and a falling liquid, one ninth less for each level of flow.
pub fn surface_height(level: u8) -> f32 {
    let ninths = match level {
        1..=MAX_FLOW_LEVEL => FALLING_LEVEL - level,
        _ => FALLING_LEVEL,
    };
    ninths as f32 / 9.0
}

/// Height of the surface of the liquid at `pos` above the bottom of its cell, in blocks:
/// the whole cell under more liquid, else `surface_height` of its level. `None` without a
/// liquid at `pos`.
pub fn liquid_height(world_map: &ChunkMap, pos: IVec3) -> Option<f32> {
    let is_liquid = |block: &BlockData| block.id.get_visibility() == BlockTransparency::Liquid;
    let block = world_map.get_block(pos).filter(|block| is_liquid(block))?;
    if world_map.get_block(pos + IVec3::Y).is_some_and(is_liquid) {
        Some(1.0)
    } else {
        Some(surface_height(block.level))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{BlockDirection, BlockId};

    #[test]
    fn a_liquid_under_more_liquid_fills_its_cell() {
        let mut world = ChunkMap::with_loaded_region(IVec3::ZERO, IVec3::splat(15));
        let bottom = IVec3::new(1, 1, 1);
        assert_eq!(liquid_height(&world, bottom), None);

        world.set_block(
            bottom,
            BlockData {
                level: 3,
                ..BlockData::new(BlockId::Water, BlockDirection::Front)
            },
        );
        assert_eq!(liquid_height(&world, bottom), Some(5.0 / 9.0));

        world.fill_box(bottom + IVec3::Y, bottom + IVec3::Y, BlockId::Water);
        assert_eq!(liquid_height(&world, bottom), Some(1.0));
        assert_eq!(liquid_height(&world, bottom + IVec3::Y), Some(8.0 / 9.0));
    }

    #[test]
    fn the_surface_goes_down_as_the_liquid_flows_away() {
        assert_eq!(surface_height(SOURCE_LEVEL), 8.0 / 9.0);
        assert_eq!(surface_height(FALLING_LEVEL), 8.0 / 9.0);
        assert_eq!(surface_height(MAX_FLOW_LEVEL), 1.0 / 9.0);
        for level in SOURCE_LEVEL..MAX_FLOW_LEVEL {
            assert!(surface_height(level + 1) < surface_height(level), "{level}");
        }
    }
}
