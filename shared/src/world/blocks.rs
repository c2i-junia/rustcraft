use std::collections::HashMap;

use super::registry::{content, BlockDef, BlockShape};
use bevy_log::warn_once;
use super::{GameElementId, ItemId, ToolKind};
use bevy::math::bounding::Aabb3d;
use rand::Rng;
use serde::de::{self, DeserializeSeed, EnumAccess, VariantAccess, Visitor};
use serde::ser::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// A block, by its number in the base mod's `blocks.toml`, where its properties are (see
/// `registry`). The engine names the base blocks with the constants below, in the order of that
/// file.
///
/// Append only: never reorder, insert before the end or remove a block. The number is what chunk
/// files and network messages store (bincode, as the index of a variant, as when this was an
/// enum), so any other change turns the blocks of every existing world into other blocks. Bump
/// `PROTOCOL_ID` and, if the saved encoding changes, `FORMAT_VERSION`. The test
/// `block_encoding_is_pinned` guards this. The saves written in RON store its name, `OakLog`.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct BlockId(u16);

/// The base blocks the engine names, with their number and their id in `blocks.toml`.
macro_rules! base_blocks {
    ($($number:literal $name:ident $id:literal,)*) => {
        #[allow(non_upper_case_globals)]
        impl BlockId {
            $(pub const $name: BlockId = BlockId($number);)*

            /// The base blocks the engine names, in the order of their numbers.
            pub const ALL: [BlockId; [$($number),*].len()] = [$(BlockId::$name),*];

            /// Their ids in `blocks.toml`, which reading it checks.
            pub const BASE_NAMES: [&'static str; [$($number),*].len()] = [$($id),*];
        }
    };
}

base_blocks! {
    0 Dirt "dirt",
    1 Debug "debug",
    2 Grass "grass",
    3 Stone "stone",
    4 OakLog "oak_log",
    5 OakPlanks "oak_planks",
    6 OakLeaves "oak_leaves",
    7 Sand "sand",
    8 Cactus "cactus",
    9 Ice "ice",
    10 Glass "glass",
    11 Bedrock "bedrock",
    12 Dandelion "dandelion",
    13 Poppy "poppy",
    14 TallGrass "tall_grass",
    15 Cobblestone "cobblestone",
    16 Snow "snow",
    17 SpruceLeaves "spruce_leaves",
    18 SpruceLog "spruce_log",
    19 Water "water",
    20 CraftingTable "crafting_table",
    21 SprucePlanks "spruce_planks",
    22 Furnace "furnace",
    23 Chest "chest",
    24 CoalOre "coal_ore",
    25 IronOre "iron_ore",
    26 GoldOre "gold_ore",
    27 Gravel "gravel",
    28 Torch "torch",
}

/// The variant under which a block of a mod is encoded, with its full id, `beacon:lamp`: its
/// number may change from a start of the game to the next, as mods are added or turned off. It
/// is no block's number, which stays below `u16::MAX`.
const MOD_BLOCK_TAG: u32 = u16::MAX as u32;
const MOD_BLOCK_VARIANT: &str = "Mod";

impl BlockId {
    /// Whether the engine names it, as a base block of `BlockId::ALL`. The other blocks come
    /// from mods, and have an item of their own, `ItemId::Block`.
    pub const fn is_named(&self) -> bool {
        (self.0 as usize) < BlockId::ALL.len()
    }

    /// What a player reads: `Oak Log`, `Lamp`.
    pub fn display_name(&self) -> String {
        self.def().display.clone()
    }

    /// The block of a mod called `full_id` in a save or a message, or, when that mod is not
    /// loaded, the debug block in its place.
    fn of_mod(full_id: &str) -> BlockId {
        content().by_name(full_id).unwrap_or_else(|| {
            warn_once!("The blocks of mods that are not loaded, as {full_id}, become debug blocks");
            BlockId::Debug
        })
    }
}

impl std::fmt::Debug for BlockId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match content().get(*self) {
            Some(def) => f.write_str(def.name),
            None => write!(f, "BlockId({})", self.0),
        }
    }
}

/// A base block as the enum it replaces: the index of a variant in bincode, its name in RON.
/// A block of a mod as the variant `Mod` holding its full id.
impl Serialize for BlockId {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        if !self.is_named() {
            let def = content()
                .get(*self)
                .ok_or_else(|| S::Error::custom(format!("no block numbered {}", self.0)))?;
            return serializer.serialize_newtype_variant(
                "BlockId",
                MOD_BLOCK_TAG,
                MOD_BLOCK_VARIANT,
                &def.full_id(),
            );
        }
        let name = if serializer.is_human_readable() {
            content()
                .get(*self)
                .map(|def| def.name)
                .ok_or_else(|| S::Error::custom(format!("no block numbered {}", self.0)))?
        } else {
            ""
        };
        serializer.serialize_unit_variant("BlockId", self.0 as u32, name)
    }
}

impl<'de> Deserialize<'de> for BlockId {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_enum("BlockId", &[], BlockIdVisitor)
    }
}

struct BlockIdVisitor;

impl<'de> Visitor<'de> for BlockIdVisitor {
    type Value = BlockId;

    fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str("a block")
    }

    fn visit_enum<A: EnumAccess<'de>>(self, data: A) -> Result<BlockId, A::Error> {
        match data.variant_seed(BlockTag)? {
            (Tag::Block(id), variant) => {
                variant.unit_variant()?;
                Ok(id)
            }
            (Tag::Mod, variant) => {
                let full_id: String = variant.newtype_variant()?;
                Ok(BlockId::of_mod(&full_id))
            }
        }
    }

    fn visit_str<E: de::Error>(self, name: &str) -> Result<BlockId, E> {
        content()
            .by_name(name)
            .ok_or_else(|| E::custom(format!("no block named {name}")))
    }
}

/// The variant of a `BlockId`: a base block, or a block of a mod, whose full id follows.
enum Tag {
    Block(BlockId),
    Mod,
}

/// Reads the variant of a `BlockId`: its number, or its name.
struct BlockTag;

impl<'de> DeserializeSeed<'de> for BlockTag {
    type Value = Tag;

    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<Tag, D::Error> {
        deserializer.deserialize_identifier(self)
    }
}

impl<'de> Visitor<'de> for BlockTag {
    type Value = Tag;

    fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str("the number or the name of a block")
    }

    fn visit_u64<E: de::Error>(self, number: u64) -> Result<Tag, E> {
        if number == MOD_BLOCK_TAG as u64 {
            return Ok(Tag::Mod);
        }
        u16::try_from(number)
            .ok()
            .map(BlockId)
            .filter(|id| content().get(*id).is_some())
            .map(Tag::Block)
            .ok_or_else(|| E::custom(format!("no block numbered {number}")))
    }

    fn visit_str<E: de::Error>(self, name: &str) -> Result<Tag, E> {
        if name == MOD_BLOCK_VARIANT {
            return Ok(Tag::Mod);
        }
        content()
            .by_name(name)
            .map(Tag::Block)
            .ok_or_else(|| E::custom(format!("no block named {name}")))
    }
}

/// Append only, for the same reason as `BlockId`: the position of the variant is what is stored
/// in chunk files and sent on the network.
#[derive(Debug, Copy, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum BlockDirection {
    Front,
    Right,
    Back,
    Left,
}

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

#[derive(Clone, Copy, PartialEq, Eq, Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
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
    /// The block of this number in `blocks.toml`.
    pub const fn from_number(number: u16) -> Self {
        BlockId(number)
    }

    pub const fn number(&self) -> u16 {
        self.0
    }

    /// Its properties. A number with no block, which only a broken file or message could give,
    /// takes those of the first.
    fn def(&self) -> &'static BlockDef {
        let registry = content();
        registry
            .get(*self)
            .or_else(|| registry.get(BlockId::default()))
            .expect("the base mod declares at least one block")
    }

    pub fn get_hitbox(&self) -> BlockHitbox {
        self.def().collision.into()
    }

    pub fn get_ray_hitbox(&self) -> BlockHitbox {
        self.def().outline.into()
    }

    /// Number of mining hits needed to break the block by hand, one every `HIT_MS` the button
    /// is held: a block takes `steps / 10` seconds. `None` if it cannot be broken.
    pub fn get_break_time(&self) -> Option<u8> {
        self.def().break_time
    }

    /// Whether a tool of `kind` breaks this block faster than a hand, at its speed.
    pub fn is_mined_faster_by(&self, kind: ToolKind) -> bool {
        self.def().tools.contains(&kind)
    }

    /// The tool this block must be broken with to drop anything, like stone and a pickaxe.
    /// Broken otherwise, it still goes, but drops nothing.
    pub fn get_required_tool(&self) -> Option<ToolKind> {
        let def = self.def();
        def.needs_tool.then(|| def.tools.first().copied()).flatten()
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
        self.def().drops.clone()
    }

    /// Whether a right click on this block uses it, like a crafting table or a chest, instead of
    /// placing a block against it. Sneaking places the block.
    pub fn is_usable(&self) -> bool {
        self.def().usable
    }

    /// Whether a block placed in the cell of this one takes its place.
    pub fn is_replaceable(&self) -> bool {
        self.def().replaceable
    }

    pub fn get_visibility(&self) -> BlockTransparency {
        self.def().look
    }

    /// How much the block dims the light that goes through it, as Minecraft's light opacity:
    /// `None` for a block no light goes through. Light loses a level from one cell to the next,
    /// or this much if more, but the light of the sky that goes straight down loses only this.
    /// A block may set it; else a decoration dims nothing, and glass-like and liquid blocks 1.
    pub fn light_opacity(&self) -> Option<u8> {
        let def = self.def();
        match def.look {
            BlockTransparency::Solid => None,
            BlockTransparency::Decoration => Some(def.light_opacity.unwrap_or(0)),
            BlockTransparency::Transparent | BlockTransparency::Liquid => {
                Some(def.light_opacity.unwrap_or(1))
            }
        }
    }

    /// The light the block gives, from 0 for none to 15, as Minecraft's light levels.
    pub fn light_emission(&self) -> u8 {
        self.def().light
    }
}

impl From<BlockShape> for BlockHitbox {
    fn from(shape: BlockShape) -> Self {
        match shape {
            BlockShape::Full => BlockHitbox::FullBlock,
            BlockShape::Empty => BlockHitbox::None,
            BlockShape::Box(aabb) => BlockHitbox::Aabb(aabb),
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

    const ALL_BLOCKS: [BlockId; 29] = BlockId::ALL;

    /// Blocks that cannot be broken by hand.
    const UNBREAKABLE_BLOCKS: [BlockId; 2] = [BlockId::Bedrock, BlockId::Water];

    fn regular_blocks() -> impl Iterator<Item = BlockId> {
        ALL_BLOCKS
            .into_iter()
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
                other => panic!("{other:?} is not a base block"),
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

    fn hitbox_text(hitbox: BlockHitbox) -> String {
        match hitbox {
            BlockHitbox::FullBlock => "full".into(),
            BlockHitbox::None => "none".into(),
            BlockHitbox::Aabb(aabb) => format!("{:?}..{:?}", aabb.min, aabb.max),
        }
    }

    /// Every property of every block, a line each.
    fn tables_text() -> String {
        let tools = [
            ToolKind::Pickaxe,
            ToolKind::Axe,
            ToolKind::Shovel,
            ToolKind::Hoe,
            ToolKind::Sword,
        ];
        let mut out = String::new();
        for id in ALL_BLOCKS {
            let faster: Vec<_> = tools
                .into_iter()
                .filter(|kind| id.is_mined_faster_by(*kind))
                .collect();
            out += &format!(
                "{id:?}: collision={} outline={} break={:?} faster={faster:?} required={:?} \
                 drops={:?} usable={} replaceable={} look={:?} opacity={:?} light={}\n",
                hitbox_text(id.get_hitbox()),
                hitbox_text(id.get_ray_hitbox()),
                id.get_break_time(),
                id.get_required_tool(),
                id.get_drop_table(),
                id.is_usable(),
                id.is_replaceable(),
                id.get_visibility(),
                id.light_opacity(),
                id.light_emission(),
            );
        }
        out
    }

    /// The base mod gives every block the properties the Rust tables it replaced gave them,
    /// which `blocks_before_the_base_mod.txt` recorded before they went.
    #[test]
    fn the_base_mod_gives_the_blocks_of_the_tables_it_replaced() {
        let before = include_str!("blocks_before_the_base_mod.txt");
        for (now, then) in tables_text().lines().zip(before.lines()) {
            assert_eq!(now, then);
        }
        assert_eq!(tables_text().lines().count(), before.lines().count());
    }

    #[test]
    fn the_saves_in_ron_keep_the_names_of_the_blocks() {
        let text = ron::to_string(&BlockData::new(BlockId::OakLog, BlockDirection::Front)).unwrap();
        assert!(text.contains("id:OakLog"), "{text}");
        let block: BlockData =
            ron::from_str("(id:CraftingTable,direction:Back,breaking_progress:0)").unwrap();
        assert_eq!(block.id, BlockId::CraftingTable);
        assert!(ron::from_str::<BlockId>("Lantern").is_err());
    }

    #[test]
    fn a_number_without_block_does_not_decode() {
        use bincode::Options;
        let decoded = bincode::options().deserialize::<BlockId>(&[200]);
        assert!(decoded.is_err());
        assert_eq!(
            bincode::options().deserialize::<BlockId>(&[3]).unwrap(),
            BlockId::Stone
        );
    }
}
