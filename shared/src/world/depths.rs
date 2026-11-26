//! The depth tiers: what lies underground is cut in three, each harder than the one above. This
//! is their one definition, with how deep the underground of a world goes (`Underground`): the
//! spawns, the hostiles, their loot and the announcements follow it.

use bevy::math::IVec3;
use serde::{Deserialize, Serialize};

use crate::players::Player;
use crate::world::{global_block_to_chunk_pos, BlockTransparency, ChunkMap};
use crate::CHUNK_SIZE;

/// Below this height and under a block, the caves, in every world.
pub const CAVE_HEIGHT: i32 = 50;
/// The caves of a deep world are filled with lava up to this height, included; those of a
/// shallow one up to `SHALLOW_LAVA_LEVEL`.
pub const LAVA_LEVEL: i32 = -54;
pub const SHALLOW_LAVA_LEVEL: i32 = 10;
/// The lowest bedrock of any world, that of the deep ones: nothing is lower but the void.
pub const WORLD_BOTTOM: i32 = Underground::Deep.bottom();

/// How deep the underground of a world goes. A new world is deep: its bedrock lies at -64,
/// gloomrock under height 0, the lava at `LAVA_LEVEL`. A world saved before is shallow
/// (`WorldGenSettings::deep`), as it was generated: the bedrock at 0 and the lava at 10, so
/// that the chunks it holds still meet those it generates.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Underground {
    Shallow,
    #[default]
    Deep,
}

impl Underground {
    /// The underground of a world whose settings say `deep`.
    pub const fn of(deep: bool) -> Self {
        if deep {
            Self::Deep
        } else {
            Self::Shallow
        }
    }

    /// The height of the lowest layer of bedrock, whole: the rough bedrock of a deep world
    /// rises `ROUGH_BEDROCK` blocks above it.
    pub const fn bottom(self) -> i32 {
        match self {
            Self::Shallow => 0,
            Self::Deep => -64,
        }
    }

    /// The caves are filled with lava up to this height, included.
    pub const fn lava_level(self) -> i32 {
        match self {
            Self::Shallow => SHALLOW_LAVA_LEVEL,
            Self::Deep => LAVA_LEVEL,
        }
    }

    /// Nothing is carved below this height, so that the bottom of the world stays whole.
    pub const fn cave_floor(self) -> i32 {
        match self {
            Self::Shallow => 6,
            Self::Deep => self.bottom() + ROUGH_BEDROCK + 1,
        }
    }

    /// Whether the rock below height 0 is gloomrock.
    pub const fn has_gloomrock(self) -> bool {
        matches!(self, Self::Deep)
    }

    /// The height below which the cells are in `tier`, or a deeper one.
    pub const fn top(self, tier: DepthTier) -> i32 {
        match (tier, self) {
            (DepthTier::Caves, _) => CAVE_HEIGHT,
            (DepthTier::Depths, Self::Shallow) => 20,
            (DepthTier::Depths, Self::Deep) => 0,
            // the shores of the lakes in a shallow world; a deep one has room for more
            (DepthTier::Furnace, Self::Shallow) => SHALLOW_LAVA_LEVEL + 2,
            (DepthTier::Furnace, Self::Deep) => LAVA_LEVEL + 14,
        }
    }

    /// The tier of the cells at height `y`, where they are underground.
    pub fn tier_at(self, y: i32) -> Option<DepthTier> {
        DepthTier::ALL
            .into_iter()
            .rev()
            .find(|&tier| y < self.top(tier))
    }
}

/// How many layers over the bottom of a deep world are bedrock in part, fewer the higher.
pub const ROUGH_BEDROCK: i32 = 4;

/// How deep underground a place is. Append only: it is saved with the mobs and the players.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DepthTier {
    Caves,
    Depths,
    /// Around and below the lakes of lava: in a shallow world from the cells right over the
    /// ground at its lava level, the shores of the lakes, as all that the caves carve lower is
    /// lava; in a deep one from 14 blocks above the lakes.
    Furnace,
}

impl DepthTier {
    pub const ALL: [DepthTier; 3] = [DepthTier::Caves, DepthTier::Depths, DepthTier::Furnace];

    /// The tier of the cell `pos`, none under the open sky, by the underground of `world`.
    /// Liquids, leaves and decorations do not hide the sky: the bottom of a sea is not
    /// underground.
    pub fn of(world: &ChunkMap, pos: IVec3) -> Option<DepthTier> {
        world
            .underground()
            .tier_at(pos.y)
            .filter(|_| is_underground(world, pos))
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
        for (y, shallow, deep) in [
            (50, None, None),
            (49, Some(DepthTier::Caves), Some(DepthTier::Caves)),
            (20, Some(DepthTier::Caves), Some(DepthTier::Caves)),
            (19, Some(DepthTier::Depths), Some(DepthTier::Caves)),
            (12, Some(DepthTier::Depths), Some(DepthTier::Caves)),
            (11, Some(DepthTier::Furnace), Some(DepthTier::Caves)),
            (0, Some(DepthTier::Furnace), Some(DepthTier::Caves)),
            (-1, Some(DepthTier::Furnace), Some(DepthTier::Depths)),
            (-40, Some(DepthTier::Furnace), Some(DepthTier::Depths)),
            (-41, Some(DepthTier::Furnace), Some(DepthTier::Furnace)),
            (-64, Some(DepthTier::Furnace), Some(DepthTier::Furnace)),
        ] {
            assert_eq!(Underground::Shallow.tier_at(y), shallow, "{y}");
            assert_eq!(Underground::Deep.tier_at(y), deep, "{y}");
        }
        // the furnace holds the shores of the lakes, the lair's floor with them
        for underground in [Underground::Shallow, Underground::Deep] {
            let lava = underground.lava_level();
            assert_eq!(underground.tier_at(lava + 1), Some(DepthTier::Furnace));
            assert!(underground.cave_floor() < lava);
            assert!(underground.bottom() < underground.cave_floor());
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
