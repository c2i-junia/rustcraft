//! The content files: each `*.ron` of a folder such as `data/content/rustcraft/` defines some of
//! the blocks and items, named as the commands name them (`oak_log`), in the vocabulary of
//! `docs/design/content-registry.md` (2.1-2.2). A file holds a map of blocks and a map of
//! items; every field of a definition may be left out, for its default:
//!
//! ```ron
//! (
//!     blocks: {
//!         "stone": (break: 20, tools: [Pickaxe], needs_tool: (kind: Pickaxe, level: 0),
//!                   drops: [(1, "cobblestone", 1)]),
//!         "torch": (look: Decoration, light: 14, break: 2, collision: None),
//!     },
//!     items: { "apple": (food_value: (4, 2.4), category: Nature) },
//! )
//! ```
//!
//! A block that leaves every field out is a solid whole block that cannot be broken and drops
//! the item of its name, if there is one. An item that leaves every field out stacks by 64,
//! deals 1 point of damage, is filed under `Blocks`, and places the block of its name, if there
//! is one: such an item needs no entry at all. See the fields of `BlockDef` and `ItemDef`.
//!
//! Each block and each item is defined in one file only; the file names carry no meaning.

use std::collections::HashMap;
use std::fmt;
use std::marker::PhantomData;
use std::path::{Path, PathBuf};

use bevy::math::bounding::Aabb3d;
use bevy::math::Vec3A;
use serde::de::{self, Deserializer, MapAccess, Visitor};
use serde::ser::Serializer;
use serde::{Deserialize, Serialize};

use super::{tool_bit, BlockProps, Content, ItemProps};
use crate::players::effects::EffectKind;
use crate::utils::snake_case;
use crate::world::{
    BlockHitbox, BlockId, BlockTransparency, Form, ItemCategory, ItemId, ItemType, MobKind,
    Palette, RequiredTool, Tool, ToolKind,
};

/// The highest level of light, which a block may give or take.
const MAX_LIGHT: u8 = 15;

/// The far corner of the cell, for a box: a fence or a wall is 1.5 high, as in Minecraft.
const BOX_TOP: Vec3A = Vec3A::new(1.0, 1.5, 1.0);

/// Why a set of content files cannot be installed: the file, then the block or item and what
/// is wrong with it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentError {
    /// The file at fault, or the folder when no file is.
    pub file: PathBuf,
    /// What is wrong in it, which names the block or item when there is one.
    pub reason: String,
}

impl fmt::Display for ContentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.file.display(), self.reason)
    }
}

impl std::error::Error for ContentError {}

impl Content {
    /// The content the files of `folder` define, read in the order of their names. Every block
    /// and every item must be defined. Nothing is installed: the caller decides.
    pub fn load(folder: &Path) -> Result<Content, ContentError> {
        let in_folder = |reason: String| ContentError {
            file: folder.to_path_buf(),
            reason,
        };
        let mut files = std::fs::read_dir(folder)
            .map_err(|err| in_folder(err.to_string()))?
            .filter_map(|entry| Some(entry.ok()?.path()))
            .filter(|path| path.extension().is_some_and(|ext| ext == "ron"))
            .collect::<Vec<_>>();
        files.sort();

        let names = Names::new();
        let mut blocks: HashMap<BlockId, (BlockProps, &Path)> = HashMap::new();
        let mut items: HashMap<ItemId, (ItemProps, &Path)> = HashMap::new();
        for file in &files {
            let fail = |reason: String| ContentError {
                file: file.clone(),
                reason,
            };
            let text = std::fs::read_to_string(file).map_err(|err| fail(err.to_string()))?;
            let defs: ContentFile =
                ron::de::from_str(&text).map_err(|err| fail(err.to_string()))?;
            for (name, def) in defs.blocks.0 {
                let id = *names
                    .blocks
                    .get(&name)
                    .ok_or_else(|| fail(format!("no block is named \"{name}\"")))?;
                let props = def
                    .build(&name, &names)
                    .and_then(|props| check_block(&props).map(|()| props))
                    .map_err(|reason| fail(format!("block \"{name}\": {reason}")))?;
                if let Some((_, other)) = blocks.insert(id, (props, file)) {
                    return Err(fail(format!(
                        "block \"{name}\" is also defined in {}",
                        other.display()
                    )));
                }
            }
            for (name, def) in defs.items.0 {
                let id = *names
                    .items
                    .get(&name)
                    .ok_or_else(|| fail(format!("no item is named \"{name}\"")))?;
                let props = def
                    .build(&name, &names)
                    .and_then(|props| check_item(&props).map(|()| props))
                    .map_err(|reason| fail(format!("item \"{name}\": {reason}")))?;
                if let Some((_, other)) = items.insert(id, (props, file)) {
                    return Err(fail(format!(
                        "item \"{name}\" is also defined in {}",
                        other.display()
                    )));
                }
            }
        }

        let mut content = Content {
            generation: 0,
            blocks: Vec::with_capacity(BlockId::ALL.len()),
            items: Vec::with_capacity(ItemId::ALL.len()),
        };
        for id in BlockId::ALL {
            let (props, _) = blocks.remove(&id).ok_or_else(|| {
                in_folder(format!(
                    "block \"{}\" is defined in no file",
                    block_name(id)
                ))
            })?;
            content.blocks.push(props);
        }
        for id in ItemId::ALL {
            let name = id.name();
            let props = match items.remove(&id) {
                Some((props, _)) => props,
                // the item of a block needs no entry
                None if names.blocks.contains_key(&name) => ItemDef::default()
                    .build(&name, &names)
                    .map_err(|reason| in_folder(format!("item \"{name}\": {reason}")))?,
                None => return Err(in_folder(format!("item \"{name}\" is defined in no file"))),
            };
            content.items.push(props);
        }
        Ok(content)
    }

    /// Whether `other` holds the same properties, whatever their generations.
    pub fn same_properties(&self, other: &Content) -> bool {
        self.blocks == other.blocks && self.items == other.items
    }

    /// Whether the chunks are drawn alike with `other`: no block changes the properties the
    /// meshes are made from (`palette`, `visibility`, `light_opacity`, `light_emission`).
    pub fn draws_like(&self, other: &Content) -> bool {
        self.blocks.iter().zip(&other.blocks).all(|(a, b)| {
            a.palette == b.palette
                && a.visibility == b.visibility
                && a.light_opacity == b.light_opacity
                && a.light_emission == b.light_emission
        })
    }
}

/// The name of a block in the files: `oak_log` for `BlockId::OakLog`.
pub(super) fn block_name(id: BlockId) -> String {
    snake_case(&format!("{id:?}"))
}

/// The blocks and items by their names in the files.
pub(super) struct Names {
    pub blocks: HashMap<String, BlockId>,
    pub items: HashMap<String, ItemId>,
}

impl Names {
    pub fn new() -> Self {
        Self {
            blocks: BlockId::ALL
                .into_iter()
                .map(|id| (block_name(id), id))
                .collect(),
            items: ItemId::ALL.into_iter().map(|id| (id.name(), id)).collect(),
        }
    }

    fn block(&self, field: &str, name: &str) -> Result<BlockId, String> {
        self.blocks
            .get(name)
            .copied()
            .ok_or_else(|| format!("{field}: no block is named \"{name}\""))
    }

    fn item(&self, field: &str, name: &str) -> Result<ItemId, String> {
        self.items
            .get(name)
            .copied()
            .ok_or_else(|| format!("{field}: no item is named \"{name}\""))
    }
}

/// What the game cannot run with in the properties of a block, named as the files name them.
fn check_block(props: &BlockProps) -> Result<(), String> {
    if props.light_emission > MAX_LIGHT {
        return Err(format!(
            "light {} is above {MAX_LIGHT}",
            props.light_emission
        ));
    }
    if props
        .light_opacity
        .is_some_and(|opacity| opacity > MAX_LIGHT)
    {
        return Err(format!("opacity is above {MAX_LIGHT}"));
    }
    if props.break_time == Some(0) {
        // the breaking stage divides by it
        return Err("break 0: the quickest break is 1".to_string());
    }
    for (field, hitbox) in [("collision", props.hitbox), ("outline", props.ray_hitbox)] {
        if let BlockHitbox::Aabb(Aabb3d { min, max }) = hitbox {
            let inside =
                min.cmpge(Vec3A::ZERO).all() && max.cmple(BOX_TOP).all() && min.cmple(max).all();
            if !inside {
                return Err(format!(
                    "{field} is not a box inside the cell (0 to 1, up to 1.5 high)"
                ));
            }
        }
    }
    Ok(())
}

/// What the game cannot run with in the properties of an item, named as the files name them.
fn check_item(props: &ItemProps) -> Result<(), String> {
    if props.max_stack == 0 {
        return Err("stack 0: a stack holds at least 1".to_string());
    }
    Ok(())
}

/// One content file.
#[derive(Deserialize, Serialize, Default)]
#[serde(deny_unknown_fields)]
pub(super) struct ContentFile {
    #[serde(default)]
    pub blocks: Named<BlockDef>,
    #[serde(default)]
    pub items: Named<ItemDef>,
}

/// What a block of the palette is (`Palette`), its base and its item named as in the files.
#[derive(Deserialize, Serialize, Debug, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
pub(super) struct PaletteDef {
    pub form: Form,
    pub base: String,
    pub item: String,
}

/// The definition of a block in a file: each field the file leaves out takes its default.
#[derive(Deserialize, Serialize, Default, Debug, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
pub(super) struct BlockDef {
    /// `palette`: what the block is, if it is one of the palette. None by default.
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    #[serde(serialize_with = "inner")]
    pub palette: Option<PaletteDef>,
    /// `visibility`: `Solid` (the default), `Transparent`, `Liquid` or `Decoration`.
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    #[serde(serialize_with = "inner")]
    pub look: Option<BlockTransparency>,
    /// `light_emission`, 0 by default.
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    #[serde(serialize_with = "inner")]
    pub light: Option<u8>,
    /// `light_opacity`. By default, from `look`: none goes through a solid block, a decoration
    /// and a glass take 0, and the other blocks 1.
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    #[serde(serialize_with = "inner")]
    pub opacity: Option<u8>,
    /// `break_time`: the hits a hand breaks the block in. A block without it cannot be broken.
    #[serde(default, rename = "break", deserialize_with = "present")]
    #[serde(skip_serializing_if = "Option::is_none", serialize_with = "inner")]
    pub break_hits: Option<u8>,
    /// `faster_tools`: the tools that mine the block faster. None by default.
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    #[serde(serialize_with = "inner")]
    pub tools: Option<Vec<ToolKind>>,
    /// `required_tool`: `(kind: Pickaxe, level: 1)`. None by default.
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    #[serde(serialize_with = "inner")]
    pub needs_tool: Option<RequiredTool>,
    /// `drops`: `(chance, "item", count)`, the item named as the commands name it. By default
    /// the item of the block's name, if there is one.
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    #[serde(serialize_with = "inner")]
    pub drops: Option<Vec<(u32, String, u32)>>,
    /// `hitbox`: `FullBlock` (the default), `None` or `Aabb((min: .., max: ..))`.
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    #[serde(serialize_with = "inner")]
    pub collision: Option<BlockHitbox>,
    /// `ray_hitbox`, the box aimed at; `collision` by default.
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    #[serde(serialize_with = "inner")]
    pub outline: Option<BlockHitbox>,
    /// `usable`, false by default, as the four below.
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    #[serde(serialize_with = "inner")]
    pub usable: Option<bool>,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    #[serde(serialize_with = "inner")]
    pub replaceable: Option<bool>,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    #[serde(serialize_with = "inner")]
    pub tall: Option<bool>,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    #[serde(serialize_with = "inner")]
    pub glass: Option<bool>,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    #[serde(serialize_with = "inner")]
    pub container: Option<bool>,
    /// `sapling`: the item the leaves sometimes drop. None by default.
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    #[serde(serialize_with = "inner")]
    pub sapling: Option<String>,
    /// `soil`: the blocks this one may be planted on, if it only stands on some.
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    #[serde(serialize_with = "inner")]
    pub soil: Option<Vec<String>>,
}

/// The light a block of `visibility` lets through when its file does not say: see
/// `BlockId::light_opacity`.
fn default_opacity(visibility: BlockTransparency, glass: bool) -> Option<u8> {
    match visibility {
        BlockTransparency::Solid => None,
        BlockTransparency::Decoration => Some(0),
        BlockTransparency::Transparent if glass => Some(0),
        BlockTransparency::Transparent | BlockTransparency::Liquid => Some(1),
    }
}

/// What a block of `name` drops when its file does not say: the item of its name, if any.
fn default_drops(name: &str, names: &Names) -> Vec<(u32, ItemId, u32)> {
    names
        .items
        .get(name)
        .map(|&item| vec![(1, item, 1)])
        .unwrap_or_default()
}

impl BlockDef {
    /// The properties of the block `name` this definition gives.
    pub fn build(self, name: &str, names: &Names) -> Result<BlockProps, String> {
        let Self {
            palette,
            look,
            light,
            opacity,
            break_hits,
            tools,
            needs_tool,
            drops,
            collision,
            outline,
            usable,
            replaceable,
            tall,
            glass,
            container,
            sapling,
            soil,
        } = self;
        let palette = palette
            .map(|PaletteDef { form, base, item }| {
                Ok::<_, String>(Palette {
                    form,
                    base: names.block("palette", &base)?,
                    item: names.item("palette", &item)?,
                })
            })
            .transpose()?;
        let drops = match drops {
            Some(drops) => drops
                .into_iter()
                .map(|(chance, item, count)| Ok((chance, names.item("drops", &item)?, count)))
                .collect::<Result<Vec<_>, String>>()?,
            None => default_drops(name, names),
        };
        let soil = soil
            .map(|soil| {
                soil.iter()
                    .map(|block| names.block("soil", block))
                    .collect::<Result<Vec<_>, _>>()
            })
            .transpose()?;
        let visibility = look.unwrap_or(BlockTransparency::Solid);
        let glass = glass.unwrap_or(false);
        let hitbox = collision.unwrap_or(BlockHitbox::FullBlock);
        Ok(BlockProps {
            palette,
            visibility,
            light_opacity: opacity.map_or_else(|| default_opacity(visibility, glass), Some),
            light_emission: light.unwrap_or(0),
            break_time: break_hits,
            faster_tools: tools
                .unwrap_or_default()
                .into_iter()
                .fold(0, |mask, kind| mask | tool_bit(kind)),
            required_tool: needs_tool,
            drops,
            hitbox,
            ray_hitbox: outline.unwrap_or(hitbox),
            usable: usable.unwrap_or(false),
            replaceable: replaceable.unwrap_or(false),
            tall: tall.unwrap_or(false),
            glass,
            container: container.unwrap_or(false),
            sapling: sapling
                .map(|sapling| names.item("sapling", &sapling))
                .transpose()?,
            soil,
        })
    }
}

/// The definition of an item in a file: each field the file leaves out takes its default.
#[derive(Deserialize, Serialize, Default, Debug, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
pub(super) struct ItemDef {
    /// `max_stack`, 64 by default.
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    #[serde(serialize_with = "inner")]
    pub stack: Option<u32>,
    /// 1 by default.
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    #[serde(serialize_with = "inner")]
    pub attack_damage: Option<u32>,
    /// `(hunger, saturation)`.
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    #[serde(serialize_with = "inner")]
    pub food_value: Option<(u8, f32)>,
    /// `(effect, level, ticks)`.
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    #[serde(serialize_with = "inner")]
    pub food_effect: Option<(EffectKind, u8, u32)>,
    /// The item left in the hand once this one is eaten.
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    #[serde(serialize_with = "inner")]
    pub eaten_remains: Option<String>,
    /// The item left in the crafting grid once this one is used.
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    #[serde(serialize_with = "inner")]
    pub crafting_remains: Option<String>,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    #[serde(serialize_with = "inner")]
    pub horse_armour: Option<u32>,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    #[serde(serialize_with = "inner")]
    pub spawned_mob: Option<MobKind>,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    #[serde(serialize_with = "inner")]
    pub tool: Option<Tool>,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    #[serde(serialize_with = "inner")]
    pub durability: Option<u32>,
    /// `Blocks` by default.
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    #[serde(serialize_with = "inner")]
    pub category: Option<ItemCategory>,
    /// The block the item places: by default the block of its name, if there is one.
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    #[serde(serialize_with = "inner")]
    pub places: Option<String>,
    /// `default_type`, for an item that places no block: `Generic` by default, `Tool(wear:
    /// 0)` for one that wears out.
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    #[serde(serialize_with = "inner")]
    pub default_type: Option<ItemType>,
}

/// The type of a new stack of an item of `name` whose file does not say: it places the block
/// of its name, if there is one.
fn default_type(name: &str, names: &Names) -> ItemType {
    names
        .blocks
        .get(name)
        .map_or(ItemType::Generic, |&block| ItemType::Block(block))
}

impl ItemDef {
    /// The properties of the item `name` this definition gives.
    pub fn build(self, name: &str, names: &Names) -> Result<ItemProps, String> {
        let Self {
            stack,
            attack_damage,
            food_value,
            food_effect,
            eaten_remains,
            crafting_remains,
            horse_armour,
            spawned_mob,
            tool,
            durability,
            category,
            places,
            default_type: given_type,
        } = self;
        let default_type = match (places, given_type) {
            (Some(_), Some(_)) => {
                return Err("places and default_type: an item has one type".to_string())
            }
            (Some(block), None) => ItemType::Block(names.block("places", &block)?),
            (None, Some(ItemType::Block(_))) => {
                return Err("default_type: a block is given by places".to_string())
            }
            (None, Some(given)) => given,
            (None, None) => default_type(name, names),
        };
        Ok(ItemProps {
            max_stack: stack.unwrap_or(64),
            attack_damage: attack_damage.unwrap_or(1),
            food_value,
            food_effect,
            eaten_remains: eaten_remains
                .map(|item| names.item("eaten_remains", &item))
                .transpose()?,
            crafting_remains: crafting_remains
                .map(|item| names.item("crafting_remains", &item))
                .transpose()?,
            horse_armour,
            spawned_mob,
            tool,
            durability,
            category: category.unwrap_or(ItemCategory::Blocks),
            default_type,
        })
    }
}

/// A field the file writes: `light: 10` is `Some(10)`, a field it leaves out `None`.
fn present<'de, D: Deserializer<'de>, T: Deserialize<'de>>(
    deserializer: D,
) -> Result<Option<T>, D::Error> {
    T::deserialize(deserializer).map(Some)
}

/// Writes a field the definition gives as its value, `light: 10`, the way `present` reads it.
/// Only called on `Some`: the fields that are `None` are skipped.
fn inner<S: Serializer, T: Serialize>(value: &Option<T>, serializer: S) -> Result<S::Ok, S::Error> {
    match value {
        Some(value) => value.serialize(serializer),
        None => serializer.serialize_none(),
    }
}

/// What a kind of definition is called in the errors.
pub(super) trait Kind {
    const KIND: &'static str;
}

impl Kind for BlockDef {
    const KIND: &'static str = "block";
}

impl Kind for ItemDef {
    const KIND: &'static str = "item";
}

/// The definitions of a file by name, in the order of the file. An error in one names it, and
/// so does a name written twice.
pub(super) struct Named<T>(pub Vec<(String, T)>);

impl<T> Default for Named<T> {
    fn default() -> Self {
        Self(Vec::new())
    }
}

impl<T: Serialize> Serialize for Named<T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_map(self.0.iter().map(|(name, def)| (name, def)))
    }
}

impl<'de, T: Deserialize<'de> + Kind> Deserialize<'de> for Named<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct NamedVisitor<T>(PhantomData<T>);

        impl<'de, T: Deserialize<'de> + Kind> Visitor<'de> for NamedVisitor<T> {
            type Value = Named<T>;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                write!(f, "a map of {} names to their definitions", T::KIND)
            }

            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Named<T>, A::Error> {
                let mut entries: Vec<(String, T)> = Vec::new();
                while let Some(name) = map.next_key::<String>()? {
                    let value = map.next_value::<T>().map_err(|err| {
                        de::Error::custom(format!("{} \"{name}\": {err}", T::KIND))
                    })?;
                    if entries.iter().any(|(other, _)| *other == name) {
                        return Err(de::Error::custom(format!(
                            "{} \"{name}\" is written twice",
                            T::KIND
                        )));
                    }
                    entries.push((name, value));
                }
                Ok(Named(entries))
            }
        }

        deserializer.deserialize_map(NamedVisitor(PhantomData))
    }
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;

    /// The content files of the game, in the source tree.
    pub fn game_folder() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../data/content/rustcraft")
    }

    /// A copy of the content files of the game, in which the entry of each `(name, entry)` of
    /// `edits` is replaced: the first entry of that name, a block's before an item's.
    pub fn edited_game_folder(edits: &[(&str, &str)]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let mut texts: Vec<(PathBuf, String)> = std::fs::read_dir(game_folder())
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|path| path.extension().is_some_and(|ext| ext == "ron"))
            .map(|path| {
                let text = std::fs::read_to_string(&path).unwrap();
                (dir.path().join(path.file_name().unwrap()), text)
            })
            .collect();
        texts.sort();
        for (name, entry) in edits {
            let key = format!("\"{name}\": ");
            let (text, start) = texts
                .iter_mut()
                .find_map(|(_, text)| {
                    let start = text
                        .lines()
                        .find(|line| line.trim_start().starts_with(&key))?;
                    let start = start.as_ptr() as usize - text.as_ptr() as usize;
                    Some((text, start))
                })
                .unwrap_or_else(|| panic!("no entry \"{name}\""));
            let end = start + text[start..].find('\n').unwrap();
            let indent = text[start..].len() - text[start..].trim_start().len();
            text.replace_range(start + indent..end, &format!("{key}{entry},"));
        }
        for (path, text) in texts {
            std::fs::write(path, text).unwrap();
        }
        dir
    }

    /// A folder holding `files`, by name and text.
    fn folder(files: &[(&str, &str)]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        for (name, text) in files {
            std::fs::write(dir.path().join(name), text).unwrap();
        }
        dir
    }

    fn load(files: &[(&str, &str)]) -> Result<Content, ContentError> {
        Content::load(folder(files).path())
    }

    #[test]
    fn the_content_files_of_the_game_load() {
        if let Err(err) = Content::load(&game_folder()) {
            panic!("{err}");
        }
    }

    #[test]
    fn no_folder_and_a_folder_short_of_a_block_are_refused() {
        assert!(Content::load(Path::new("/no/such/folder")).is_err());
        let error = load(&[("empty.ron", "()")]).unwrap_err();
        assert!(error.to_string().contains("defined in no file"), "{error}");
    }

    #[test]
    fn an_edited_definition_changes_the_properties_it_names_and_only_those() {
        let game = Content::load(&game_folder()).unwrap();
        let folder = edited_game_folder(&[
            (
                "torch",
                "(look: Decoration, light: 9, break: 2, collision: None, \
                 outline: Aabb((min: (0.35, 0, 0.35), max: (0.65, 0.6, 0.65))))",
            ),
            (
                "stone",
                r#"(break: 20, tools: [Pickaxe, Shovel], drops: [(1, "gravel", 2)],
                    needs_tool: (kind: Shovel, level: 2), look: Decoration)"#,
            ),
            (
                "apple",
                "(stack: 16, food_value: (2, 1.5), category: Nature, default_type: Generic)",
            ),
        ]);
        let content = Content::load(folder.path()).unwrap();

        let torch = content.block(BlockId::Torch);
        assert_eq!(torch.light_emission, 9);
        assert_eq!(
            BlockProps {
                light_emission: game.block(BlockId::Torch).light_emission,
                ray_hitbox: game.block(BlockId::Torch).ray_hitbox,
                ..torch.clone()
            },
            *game.block(BlockId::Torch)
        );
        let stone = content.block(BlockId::Stone);
        assert_eq!(stone.break_time, Some(20));
        assert_eq!(
            stone.faster_tools,
            tool_bit(ToolKind::Pickaxe) | tool_bit(ToolKind::Shovel)
        );
        assert_eq!(stone.drops, vec![(1, ItemId::Gravel, 2)]);
        assert_eq!(
            stone.required_tool,
            Some(RequiredTool {
                kind: ToolKind::Shovel,
                level: 2
            })
        );
        assert_eq!(stone.visibility, BlockTransparency::Decoration);
        // from the look, as it is not given
        assert_eq!(stone.light_opacity, Some(0));
        let apple = content.item(ItemId::Apple);
        assert_eq!(apple.max_stack, 16);
        assert_eq!(apple.food_value, Some((2, 1.5)));

        let untouched = |id: BlockId| content.block(id) == game.block(id);
        assert!(BlockId::ALL
            .into_iter()
            .filter(|id| ![BlockId::Torch, BlockId::Stone].contains(id))
            .all(untouched));
        assert_eq!(content.items.len(), game.items.len());
        assert!(!content.same_properties(&game));
        assert!(!content.draws_like(&game), "the torch gives less light");
    }

    #[test]
    fn a_property_only_change_draws_like_before() {
        let folder = edited_game_folder(&[("dirt", "(break: 50, tools: [Shovel])")]);
        let content = Content::load(folder.path()).unwrap();
        assert!(content.draws_like(&Content::load(&game_folder()).unwrap()));
    }

    #[test]
    fn the_defaults_of_a_definition() {
        let names = Names::new();
        let dirt = BlockDef::default().build("dirt", &names).unwrap();
        assert_eq!(dirt.visibility, BlockTransparency::Solid);
        assert_eq!(dirt.light_opacity, None);
        assert_eq!(dirt.break_time, None);
        assert_eq!(dirt.drops, vec![(1, ItemId::Dirt, 1)]);
        assert_eq!(dirt.hitbox, BlockHitbox::FullBlock);
        assert_eq!(dirt.ray_hitbox, BlockHitbox::FullBlock);
        let water = BlockDef::default().build("water", &names).unwrap();
        assert_eq!(water.drops, vec![], "no item is named water");
        let leaves = BlockDef {
            look: Some(BlockTransparency::Transparent),
            collision: Some(BlockHitbox::None),
            ..Default::default()
        }
        .build("oak_leaves", &names)
        .unwrap();
        assert_eq!(leaves.light_opacity, Some(1));
        assert_eq!(leaves.ray_hitbox, BlockHitbox::None);

        let dirt = ItemDef::default().build("dirt", &names).unwrap();
        assert_eq!(dirt.max_stack, 64);
        assert_eq!(dirt.attack_damage, 1);
        assert_eq!(dirt.category, ItemCategory::Blocks);
        assert_eq!(dirt.default_type, ItemType::Block(BlockId::Dirt));
        let stick = ItemDef::default().build("stick", &names).unwrap();
        assert_eq!(stick.default_type, ItemType::Generic);
    }

    /// The error of a bad file names the file and the block or item, and nothing is loaded.
    #[test]
    fn a_bad_file_is_refused_with_its_name_and_its_key() {
        let cases = [
            (r#"(blocks: {"torch": (light: 20)})"#, "block \"torch\""),
            (
                r#"(blocks: {"torch": (light_emission: 9)})"#,
                "block \"torch\"",
            ),
            (r#"(blocks: {"tourch": (light: 9)})"#, "\"tourch\""),
            (r#"(blocks: {"stone": (break: 0)})"#, "block \"stone\""),
            (
                r#"(blocks: {"stone": (drops: [(1, "gravell", 1)])})"#,
                "block \"stone\": drops: no item is named \"gravell\"",
            ),
            (
                r#"(blocks: {"stone": (drops: [(1, Gravel, 1)])})"#,
                "block \"stone\"",
            ),
            (
                r#"(blocks: {"oak_slab": (palette: (form: Slab, base: "oak", item: "oak_slab"))})"#,
                "block \"oak_slab\": palette: no block is named \"oak\"",
            ),
            (r#"(items: {"apple": (stack: 0)})"#, "item \"apple\""),
            (r#"(items: {"apple": (stack: "many")})"#, "item \"apple\""),
            (
                r#"(items: {"apple": (places: "dirt", default_type: Generic)})"#,
                "item \"apple\"",
            ),
            (
                r#"(blocks: {"glass": (collision: Aabb((min: (0, 0, 0), max: (1, 2.5, 1))))})"#,
                "block \"glass\"",
            ),
            (
                r#"(blocks: {"torch": (light: 1), "torch": (light: 2)})"#,
                "\"torch\" is written twice",
            ),
        ];
        for (text, key) in cases {
            let error = load(&[("good.ron", "()"), ("bad.ron", text)]).unwrap_err();
            assert_eq!(error.file.file_name().unwrap(), "bad.ron", "{text}");
            assert!(error.to_string().contains(key), "{text}: {error}");
        }
    }

    #[test]
    fn a_syntax_error_names_the_file() {
        let error = load(&[("broken.ron", "(blocks: {\"torch\": (light: 9}")]).unwrap_err();
        assert_eq!(error.file.file_name().unwrap(), "broken.ron");
    }

    #[test]
    fn a_block_defined_in_two_files_names_both() {
        let error = load(&[
            ("a.ron", r#"(blocks: {"torch": (light: 9)})"#),
            ("b.ron", r#"(blocks: {"torch": (light: 8)})"#),
        ])
        .unwrap_err();
        let message = error.to_string();
        assert!(
            message.contains("b.ron") && message.contains("a.ron"),
            "{message}"
        );
        assert!(message.contains("\"torch\""), "{message}");
    }

    #[test]
    fn the_content_goes_over_the_wire_unchanged() {
        let content = Content::load(&game_folder()).unwrap();
        let payload = crate::game_message_to_payload(&content);
        assert_eq!(
            crate::payload_to_game_message::<Content>(&payload).unwrap(),
            content
        );
    }
}
