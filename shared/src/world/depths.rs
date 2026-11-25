//! The depth tiers: what lies underground is cut in three, each harder than the one above. This
//! is their one definition: the spawns, the hostiles, their loot and the announcements follow it.

use bevy::math::IVec3;
use serde::{Deserialize, Serialize};

use crate::players::Player;
use crate::world::{global_block_to_chunk_pos, BlockTransparency, ChunkMap};
use crate::CHUNK_SIZE;

/// Below this height and under a block, the caves.
pub const CAVE_HEIGHT: i32 = 50;
/// Below this height and under a block, the depths: the Cinderlings' home.
pub const CINDER_HEIGHT: i32 = 20;
/// The caves are filled with lava up to this height, included.
pub const LAVA_LEVEL: i32 = 10;

/// How deep underground a place is. Append only: it is saved with the mobs and the players.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DepthTier {
    Caves,
    Depths,
    /// Around and below the lakes of lava: from the cells right over the ground at `LAVA_LEVEL`,
    /// the shores of the lakes, as all that the caves carve lower is lava.
    Furnace,
}

impl DepthTier {
    pub const ALL: [DepthTier; 3] = [DepthTier::Caves, DepthTier::Depths, DepthTier::Furnace];

    /// The height below which the cells are in this tier, or a deeper one.
    pub fn top(self) -> i32 {
        match self {
            DepthTier::Caves => CAVE_HEIGHT,
            DepthTier::Depths => CINDER_HEIGHT,
            DepthTier::Furnace => LAVA_LEVEL + 2,
        }
    }

    /// The tier of the cells at height `y`, where they are underground.
    pub fn at(y: i32) -> Option<DepthTier> {
        Self::ALL.into_iter().rev().find(|tier| y < tier.top())
    }

    /// The tier of the cell `pos`, none under the open sky. Liquids, leaves and decorations do
    /// not hide the sky: the bottom of a sea is not underground.
    pub fn of(world: &ChunkMap, pos: IVec3) -> Option<DepthTier> {
        Self::at(pos.y).filter(|_| is_underground(world, pos))
    }

    /// The tier of the feet of `player`.
    pub fn of_player(world: &ChunkMap, player: &Player) -> Option<DepthTier> {
        let feet = player.position.y - player.height / 2.0 + 0.01;
        Self::of(world, player.position.with_y(feet).floor().as_ivec3())
    }

    /// How strong the hostiles of this tier are, and how many, in percent of those of the caves,
    /// which are as strong as at the surface.
    pub fn strength(self) -> u32 {
        match self {
            DepthTier::Caves => 100,
            DepthTier::Depths => 150,
            DepthTier::Furnace => 200,
        }
    }

    /// What the player reads when it first reaches this tier.
    pub fn title(self) -> &'static str {
        match self {
            DepthTier::Caves => "The Caves",
            DepthTier::Depths => "The Depths",
            DepthTier::Furnace => "The Furnace",
        }
    }
}

/// The strength of `tier` (`DepthTier::strength`), 100 % out of the tiers.
pub fn strength(tier: Option<DepthTier>) -> u32 {
    tier.map_or(100, DepthTier::strength)
}

/// `amount` times the strength of `tier`, rounded up.
pub fn strengthened(amount: u32, tier: Option<DepthTier>) -> u32 {
    (amount * strength(tier)).div_ceil(100)
}

/// Whether the ground is above the cell `pos`, in the chunks loaded. Air, liquids, leaves and
/// decorations are not the ground.
fn is_underground(world: &ChunkMap, pos: IVec3) -> bool {
    let mut cell = pos + IVec3::Y;
    while let Some(chunk) = world.chunk(global_block_to_chunk_pos(&cell)) {
        if chunk.uniform() == Some(None) {
            cell.y = (cell.y.div_euclid(CHUNK_SIZE) + 1) * CHUNK_SIZE;
            continue;
        }
        let is_ground = world.get_block(cell).is_some_and(|block| {
            !matches!(
                block.id.get_visibility(),
                BlockTransparency::Decoration | BlockTransparency::Liquid
            ) && block.id.sapling().is_none()
        });
        if is_ground {
            return true;
        }
        cell.y += 1;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::BlockId;

    #[test]
    fn the_tiers_go_by_height_under_the_ground_only() {
        for (y, tier) in [
            (50, None),
            (49, Some(DepthTier::Caves)),
            (20, Some(DepthTier::Caves)),
            (19, Some(DepthTier::Depths)),
            (12, Some(DepthTier::Depths)),
            (11, Some(DepthTier::Furnace)),
            (0, Some(DepthTier::Furnace)),
        ] {
            assert_eq!(DepthTier::at(y), tier, "{y}");
        }
        let mut world = ChunkMap::with_loaded_region(IVec3::splat(-16), IVec3::splat(63));
        let pos = IVec3::new(0, 30, 0);
        for (cover, tier) in [
            (BlockId::Water, None),
            (BlockId::OakLeaves, None),
            (BlockId::Stone, Some(DepthTier::Caves)),
        ] {
            world.fill_box(pos + IVec3::Y * 20, pos + IVec3::Y * 20, cover);
            assert_eq!(DepthTier::of(&world, pos), tier, "{cover:?}");
        }
        assert_eq!(strengthened(3, None), 3);
        assert_eq!(strengthened(3, Some(DepthTier::Depths)), 5);
        assert_eq!(strengthened(3, Some(DepthTier::Furnace)), 6);
    }
}
