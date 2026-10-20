use std::collections::HashMap;

use super::{GameElementId, ItemId, ToolKind};
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
}

/// Append only, for the same reason as `BlockId`: the position of the variant is what is stored
/// in chunk files and sent on the network.
///
/// A block drawn turned is turned from its front by the direction: `Right` turns its front
/// toward +X. A vine's direction names the side of its cell the block holding it lies on.
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

    /// The box rays hit, as `BlockId::get_ray_hitbox`, but turned to the direction of the block
    /// for a vine: a slab around its plate, on the side of the block holding it.
    pub fn get_ray_hitbox(&self) -> BlockHitbox {
        match (self.id, self.id.get_ray_hitbox()) {
            (BlockId::Vine, BlockHitbox::Aabb(front)) => {
                // about the centre of the cell
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
            (_, hitbox) => hitbox,
        }
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

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BlockTransparency {
    Transparent,
    Liquid,
    Solid,
    Decoration,
}

pub enum BlockHitbox {
    FullBlock,
    Aabb(Aabb3d),
    None,
}

impl BlockId {
    /// Every block, in the order of their encoding.
    pub const ALL: [BlockId; 34] = [
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
    ];

    pub fn get_hitbox(&self) -> BlockHitbox {
        match *self {
            Self::Water
            | Self::TallGrass
            | Self::Fern
            | Self::Poppy
            | Self::Dandelion
            | Self::Torch
            | Self::Vine => BlockHitbox::None,
            _ => BlockHitbox::FullBlock,
        }
    }

    pub fn get_ray_hitbox(&self) -> BlockHitbox {
        match *self {
            Self::Water => BlockHitbox::None,
            Self::TallGrass | Self::Fern | Self::Poppy | Self::Dandelion => BlockHitbox::Aabb(
                Aabb3d::new(Vec3A::splat(0.5).with_y(0.3), Vec3A::splat(0.3)),
            ),
            // The stick, a little wider to be easier to aim at
            Self::Torch => BlockHitbox::Aabb(Aabb3d::new(
                Vec3A::new(0.5, 0.3, 0.5),
                Vec3A::new(0.15, 0.3, 0.15),
            )),
            // a slab around the plate of a vine held on its front
            Self::Vine => BlockHitbox::Aabb(Aabb3d::new(
                Vec3A::new(0.5, 0.5, VINE_GAP),
                Vec3A::new(0.5, 0.5, VINE_GAP / 2.0),
            )),
            _ => BlockHitbox::FullBlock,
        }
    }

    /// Number of mining hits needed to break the block by hand, one every `HIT_MS` the button
    /// is held: a block takes `steps / 10` seconds. `None` if it cannot be broken.
    pub fn get_break_time(&self) -> Option<u8> {
        let steps: u8 = match *self {
            Self::Dirt => 5,
            Self::Debug => 7,
            Self::Grass => 6,
            Self::Stone => 10,
            Self::OakLog => 10,
            Self::OakPlanks => 10,
            Self::OakLeaves => 2,
            Self::Sand => 5,
            Self::Cactus => 4,
            Self::Ice => 5,
            Self::Glass => 3,
            Self::Dandelion => 1,
            Self::Poppy => 1,
            Self::TallGrass => 1,
            Self::Cobblestone => 2,
            Self::Snow => 9,
            Self::SpruceLeaves => 2,
            Self::SpruceLog => 10,
            Self::CraftingTable => 10,
            Self::SprucePlanks => 10,
            Self::Furnace => 15,
            Self::Chest => 10,
            Self::CoalOre => 12,
            Self::IronOre => 15,
            Self::GoldOre => 15,
            Self::Gravel => 6,
            Self::Torch => 1,
            Self::JungleLog => 10,
            Self::JunglePlanks => 10,
            Self::JungleLeaves => 2,
            Self::Fern => 1,
            Self::Vine => 1,
            Self::Bedrock | Self::Water => return None,
        };
        Some(2 * steps)
    }

    /// Whether a tool of `kind` breaks this block faster than a hand, at its speed.
    pub fn is_mined_faster_by(&self, kind: ToolKind) -> bool {
        match kind {
            ToolKind::Pickaxe => matches!(
                *self,
                Self::Stone
                    | Self::Cobblestone
                    | Self::Furnace
                    | Self::Ice
                    | Self::CoalOre
                    | Self::IronOre
                    | Self::GoldOre
            ),
            ToolKind::Axe => matches!(
                *self,
                Self::OakLog
                    | Self::SpruceLog
                    | Self::JungleLog
                    | Self::OakPlanks
                    | Self::SprucePlanks
                    | Self::JunglePlanks
                    | Self::CraftingTable
                    | Self::Chest
            ),
            ToolKind::Shovel => matches!(
                *self,
                Self::Dirt | Self::Grass | Self::Sand | Self::Snow | Self::Gravel
            ),
            ToolKind::Hoe | ToolKind::Sword => {
                matches!(
                    *self,
                    Self::OakLeaves | Self::SpruceLeaves | Self::JungleLeaves
                )
            }
        }
    }

    /// The tool this block must be broken with to drop anything, like stone and a pickaxe.
    /// Broken otherwise, it still goes, but drops nothing.
    pub fn get_required_tool(&self) -> Option<ToolKind> {
        match *self {
            Self::Stone
            | Self::Cobblestone
            | Self::Furnace
            | Self::CoalOre
            | Self::IronOre
            | Self::GoldOre => Some(ToolKind::Pickaxe),
            _ => None,
        }
    }

    /// What breaking this block `nb_drops` times gives. Each drop is picked at random in the
    /// drop table; a table of one entry, like every current one, always gives that entry. Only
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
    pub fn get_drop_table(&self) -> Vec<(u32, ItemId, u32)> {
        match *self {
            BlockId::Dirt | BlockId::Grass => vec![(1, ItemId::Dirt, 1)],
            BlockId::Stone => vec![(1, ItemId::Cobblestone, 1)],
            BlockId::Sand => vec![(1, ItemId::Sand, 1)],
            BlockId::Cactus => vec![(1, ItemId::Cactus, 1)],
            BlockId::OakLog => vec![(1, ItemId::OakLog, 1)],
            BlockId::OakPlanks => vec![(1, ItemId::OakPlanks, 1)],
            BlockId::Ice => vec![(1, ItemId::Ice, 1)],
            BlockId::Dandelion => vec![(1, ItemId::Dandelion, 1)],
            BlockId::Poppy => vec![(1, ItemId::Poppy, 1)],
            BlockId::TallGrass => vec![(1, ItemId::TallGrass, 1)],
            BlockId::Fern => vec![(1, ItemId::Fern, 1)],
            BlockId::Vine => vec![(1, ItemId::Vine, 1)],
            BlockId::SpruceLog => vec![(1, ItemId::SpruceLog, 1)],
            BlockId::Snow => vec![(1, ItemId::Snowball, 4)],
            BlockId::CraftingTable => vec![(1, ItemId::CraftingTable, 1)],
            BlockId::SprucePlanks => vec![(1, ItemId::SprucePlanks, 1)],
            BlockId::Furnace => vec![(1, ItemId::Furnace, 1)],
            BlockId::Chest => vec![(1, ItemId::Chest, 1)],
            BlockId::CoalOre => vec![(1, ItemId::Coal, 1)],
            // they will smelt into ingots: until then, they drop themselves
            BlockId::IronOre => vec![(1, ItemId::IronOre, 1)],
            BlockId::GoldOre => vec![(1, ItemId::GoldOre, 1)],
            BlockId::Gravel => vec![(1, ItemId::Gravel, 1)],
            BlockId::Torch => vec![(1, ItemId::Torch, 1)],
            BlockId::JungleLog => vec![(1, ItemId::JungleLog, 1)],
            BlockId::JunglePlanks => vec![(1, ItemId::JunglePlanks, 1)],
            _ => vec![],
        }
    }

    /// Whether a right click on this block uses it, like a crafting table or a chest, instead of
    /// placing a block against it. Sneaking places the block.
    pub fn is_usable(&self) -> bool {
        matches!(*self, Self::CraftingTable | Self::Chest)
    }

    /// Whether a block placed in the cell of this one takes its place.
    pub fn is_replaceable(&self) -> bool {
        matches!(*self, Self::Water)
    }

    pub fn get_visibility(&self) -> BlockTransparency {
        match *self {
            Self::Dandelion
            | Self::Poppy
            | Self::TallGrass
            | Self::Fern
            | Self::Torch
            | Self::Vine => BlockTransparency::Decoration,
            Self::Glass | Self::OakLeaves | Self::SpruceLeaves | Self::JungleLeaves => {
                BlockTransparency::Transparent
            }
            Self::Water => BlockTransparency::Liquid,
            _ => BlockTransparency::Solid,
        }
    }

    /// How much the block dims the light that goes through it, as Minecraft's light opacity:
    /// `None` for a block no light goes through. Light loses a level from one cell to the next,
    /// or this much if more, but the light of the sky that goes straight down loses only this.
    pub fn light_opacity(&self) -> Option<u8> {
        match self.get_visibility() {
            BlockTransparency::Solid => None,
            BlockTransparency::Decoration => Some(0),
            BlockTransparency::Transparent if *self == Self::Glass => Some(0),
            BlockTransparency::Transparent | BlockTransparency::Liquid => Some(1),
        }
    }

    /// The light the block gives, from 0 for none to 15, as Minecraft's light levels.
    pub fn light_emission(&self) -> u8 {
        match *self {
            Self::Torch => 14,
            _ => 0,
        }
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

    const ALL_BLOCKS: [BlockId; 34] = BlockId::ALL;

    /// Blocks that cannot be broken by hand.
    const UNBREAKABLE_BLOCKS: [BlockId; 2] = [BlockId::Bedrock, BlockId::Water];

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
            | BlockId::JungleLog
            | BlockId::JunglePlanks
            | BlockId::JungleLeaves
            | BlockId::Fern
            | BlockId::Vine => {}
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

    #[test]
    fn block_encoding_is_pinned() {
        for id in ALL_BLOCKS {
            let expected: u8 = match id {
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
            };
            assert_eq!(encoded(&id), [expected], "{id:?}: {ENCODING_WARNING}");
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
    fn stone_blocks_need_a_pickaxe_and_the_others_nothing() {
        for id in ALL_BLOCKS {
            let expected = matches!(
                id,
                BlockId::Stone
                    | BlockId::Cobblestone
                    | BlockId::Furnace
                    | BlockId::CoalOre
                    | BlockId::IronOre
                    | BlockId::GoldOre
            )
            .then_some(ToolKind::Pickaxe);
            assert_eq!(id.get_required_tool(), expected, "{id:?}");
        }
    }

    #[test]
    fn a_block_that_needs_a_tool_is_mined_faster_by_it() {
        for id in ALL_BLOCKS {
            if let Some(kind) = id.get_required_tool() {
                assert!(id.is_mined_faster_by(kind), "{id:?}");
            }
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
