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
//! A block that leaves every field out is a solid whole block that cannot be broken, drops the
//! item of its name, if there is one, and shows the texture of its name (`OakLog` for
//! `oak_log`) on its six faces. An item that leaves every field out stacks by 64,
//! deals 1 point of damage, is filed under `Blocks`, and places the block of its name, if there
//! is one: such an item needs no entry at all. See the fields of `BlockDef` and `ItemDef`.
//!
//! Each block and each item is defined in one file only; the file names carry no meaning.

use std::collections::{BTreeMap, HashMap};
use std::fmt;
use std::marker::PhantomData;
use std::path::{Path, PathBuf};

use bevy::math::bounding::Aabb3d;
use bevy::math::Vec3A;
use serde::de::{self, Deserializer, MapAccess, Visitor};
use serde::Deserialize;

use super::families::{FamilyDef, Texts};
use super::recipes::{CraftingDefs, FuelDef, RecipeDef, TagDef};
use super::{
    tool_bit, Behaviour, BlockFlags, BlockProps, Content, Crafting, DisplayName, ItemProps, Look,
    Shape, Sway, Tint,
};
use crate::players::effects::EffectKind;
use crate::utils::{pascal_case, snake_case};
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

/// A block or an item that only the files define and that a load numbered anew: what to tell
/// whoever edits them, as a name mistyped defines a new block or item instead of changing the
/// one it meant. The item of a new block is not told apart from it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewName {
    /// "block" or "item".
    pub kind: &'static str,
    pub name: String,
    /// The file that defines it.
    pub file: PathBuf,
    /// The name of another of its kind it is close to (`MAX_TYPO` edits or fewer), the one
    /// meant if it was mistyped.
    pub near: Option<String>,
}

/// Most edits that turn a new name into an existing one for `NewName::near`.
const MAX_TYPO: usize = 2;

impl fmt::Display for NewName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let file = self.file.file_name().unwrap_or(self.file.as_os_str());
        write!(
            f,
            "{} \"{}\" in {} is new",
            self.kind,
            self.name,
            file.to_string_lossy()
        )?;
        if let Some(near) = &self.near {
            write!(f, "; did you mean \"{near}\"?")?;
        }
        Ok(())
    }
}

/// The fewest insertions, removals and replacements of a character that turn `a` into `b`.
fn edit_distance(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut diagonal = row[0];
        row[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let replaced = diagonal + usize::from(ca != *cb);
            diagonal = row[j + 1];
            row[j + 1] = replaced.min(row[j] + 1).min(diagonal + 1);
        }
    }
    row[b.len()]
}

impl Content {
    /// The content the files of `folder` define, read in the order of their names, the blocks
    /// and the items numbered as at the start of a process: those of the engine first
    /// (`BlockId::ENGINE`, `ItemId::ENGINE`), then the others in the order of their names, the
    /// item of each block only the files define among them. Every block of the engine must be
    /// defined, and every item of the engine that is not the item of a block. Nothing is
    /// installed: the caller decides.
    pub fn load(folder: &Path) -> Result<Content, ContentError> {
        Self::load_telling(folder, None).map(|(content, _)| content)
    }

    /// The content the files of `folder` define, as `load` reads it, but numbered as `previous`
    /// is, the content it is to replace in a process: a block or an item keeps its number, a new
    /// one takes the next, and one `previous` has that the files no longer define keeps its
    /// number as a placeholder (`BlockProps::placeholder`, `ItemProps::PLACEHOLDER`).
    pub fn reload(folder: &Path, previous: &Content) -> Result<Content, ContentError> {
        Self::load_telling(folder, Some(previous)).map(|(content, _)| content)
    }

    /// `reload` over `previous`, or `load` without, with the blocks and the items only the files
    /// define that it numbered anew, to be told to whoever edits the files.
    pub fn load_telling(
        folder: &Path,
        previous: Option<&Content>,
    ) -> Result<(Content, Vec<NewName>), ContentError> {
        let in_folder = |reason: String| ContentError {
            file: folder.to_path_buf(),
            reason,
        };
        let mut paths = std::fs::read_dir(folder)
            .map_err(|err| in_folder(err.to_string()))?
            .filter_map(|entry| Some(entry.ok()?.path()))
            .filter(|path| path.extension().is_some_and(|ext| ext == "ron"))
            .collect::<Vec<_>>();
        paths.sort();
        let mut files = Vec::with_capacity(paths.len());
        for path in paths {
            let fail = |reason: String| ContentError {
                file: path.clone(),
                reason,
            };
            let text = std::fs::read_to_string(&path).map_err(|err| fail(err.to_string()))?;
            let defs: ContentFile =
                ron::de::from_str(&text).map_err(|err| fail(err.to_string()))?;
            files.push((path, defs));
        }
        // the families written out, then each definition `like` another completed from it
        super::families::expand(&mut files)?;
        complete_likes(&mut files)?;

        // the numbers of the blocks and the items, before any definition refers to one
        let block_defs = || {
            files
                .iter()
                .flat_map(|(path, defs)| defs.blocks.0.iter().map(move |(name, _)| (name, path)))
        };
        let mut block_names: Vec<String> = match previous {
            Some(previous) => previous.block_names.clone(),
            None => BlockId::ENGINE.into_iter().map(engine_name).collect(),
        };
        let new_blocks = number_new("block", &mut block_names, block_defs())?;
        // the item of each block only the files define, then the items only they define
        let mut item_names: Vec<String> = match previous {
            Some(previous) => previous.item_names.clone(),
            None => ItemId::ENGINE.into_iter().map(engine_item_name).collect(),
        };
        let engine_blocks = &block_names[..BlockId::ENGINE.len()];
        let block_items = block_defs().filter(|(name, _)| !engine_blocks.contains(name));
        let item_defs = files
            .iter()
            .flat_map(|(path, defs)| defs.items.0.iter().map(move |(name, _)| (name, path)));
        let new_items = number_new("item", &mut item_names, block_items.chain(item_defs))?;
        let new_names: Vec<NewName> = new_blocks
            .into_iter()
            .map(|(name, file)| NewName::near("block", name, file, &block_names))
            .chain(
                new_items
                    .into_iter()
                    .filter(|(name, _)| !block_names.contains(name))
                    .map(|(name, file)| NewName::near("item", name, file, &item_names)),
            )
            .collect();
        let mut names = Names::new(&block_names, &item_names);
        let (block_aliases, item_aliases) = names.add_aliases(&files)?;

        let (paths, files): (Vec<PathBuf>, Vec<ContentFile>) = files.into_iter().unzip();
        let mut crafting_defs = Vec::with_capacity(files.len());
        let mut blocks: Vec<Option<(BlockProps, &Path)>> = vec![None; block_names.len()];
        let mut items: Vec<Option<(ItemProps, &Path)>> = vec![None; item_names.len()];
        // the names to show, by item; that of a block serves its item
        let mut block_display = HashMap::new();
        let mut item_display = std::collections::BTreeMap::new();
        for (file, defs) in paths.iter().zip(files) {
            let fail = |reason: String| ContentError {
                file: file.clone(),
                reason,
            };
            crafting_defs.push((
                file.as_path(),
                CraftingDefs {
                    tags: defs.tags,
                    recipes: defs.recipes,
                    smelting: defs.smelting,
                    fuel: defs.fuel,
                },
            ));
            for (name, mut def) in defs.blocks.0 {
                let id = names.block("name", &name).map_err(&fail)?;
                if let Some(display) = def.name.take() {
                    block_display.insert(name.clone(), display.into());
                }
                let props = def
                    .build(&name, &names)
                    .and_then(|props| check_block(&props).map(|()| props))
                    .map_err(|reason| fail(format!("block \"{name}\": {reason}")))?;
                if let Some((_, other)) = blocks[id.index()].replace((props, file.as_path())) {
                    return Err(fail(format!(
                        "block \"{name}\" is also defined in {}",
                        other.display()
                    )));
                }
            }
            for (name, mut def) in defs.items.0 {
                let id = names.item("name", &name).map_err(&fail)?;
                if let Some(display) = def.name.take() {
                    item_display.insert(id.index() as u16, display.into());
                }
                let props = def
                    .build(&name, &names)
                    .and_then(|props| check_item(&props).map(|()| props))
                    .map_err(|reason| fail(format!("item \"{name}\": {reason}")))?;
                if let Some((_, other)) = items[id.index()].replace((props, file.as_path())) {
                    return Err(fail(format!(
                        "item \"{name}\" is also defined in {}",
                        other.display()
                    )));
                }
            }
        }

        for (name, block) in block_names.iter().zip(&blocks).take(BlockId::ENGINE.len()) {
            if block.is_none() {
                return Err(in_folder(format!("block \"{name}\" is defined in no file")));
            }
        }
        let mut item_props = Vec::with_capacity(item_names.len());
        for (index, (name, item)) in item_names.iter().zip(items).enumerate() {
            let defined_block = names
                .blocks
                .get(name)
                .is_some_and(|block| blocks[block.index()].is_some());
            let props = match item {
                Some((props, _)) => props,
                // the item of a block needs no entry
                None if defined_block => ItemDef::default()
                    .build(name, &names)
                    .map_err(|reason| in_folder(format!("item \"{name}\": {reason}")))?,
                // one the files defined before, kept in its place
                None if index >= ItemId::ENGINE.len() => ItemProps::PLACEHOLDER,
                None => return Err(in_folder(format!("item \"{name}\" is defined in no file"))),
            };
            item_props.push(props);
            if let Some(display) = block_display.remove(name) {
                item_display.entry(index as u16).or_insert(display);
            }
        }
        let crafting = Crafting::build(crafting_defs, folder, &names, &item_props)?;
        let debug = blocks[BlockId::Debug.index()]
            .as_ref()
            .map(|(props, _)| BlockProps::placeholder(props))
            .expect("an engine block");
        let content = Content {
            generation: 0,
            block_idents: block_names.iter().map(|name| pascal_case(name)).collect(),
            block_names,
            blocks: blocks
                .into_iter()
                .map(|block| block.map_or_else(|| debug.clone(), |(props, _)| props))
                .collect(),
            item_idents: item_names.iter().map(|name| pascal_case(name)).collect(),
            item_names,
            items: item_props,
            item_display,
            block_aliases,
            item_aliases,
            crafting,
        };
        Ok((content, new_names))
    }

    /// Whether `other` holds the same properties, whatever their generations.
    pub fn same_properties(&self, other: &Content) -> bool {
        self.block_names == other.block_names
            && self.blocks == other.blocks
            && self.item_names == other.item_names
            && self.items == other.items
            && self.item_display == other.item_display
            && self.block_aliases == other.block_aliases
            && self.item_aliases == other.item_aliases
            && self.crafting == other.crafting
    }

    /// Whether the chunks are drawn alike with `other`: no block changes the properties the
    /// meshes are made from (`palette`, `visibility`, `light_opacity`, `light_emission`,
    /// `flags`, `look`, and `hitbox`, which a `Shape::Box` is drawn from).
    pub fn draws_like(&self, other: &Content) -> bool {
        if self.block_names != other.block_names {
            return false;
        }
        self.blocks.iter().zip(&other.blocks).all(|(a, b)| {
            a.palette == b.palette
                && a.visibility == b.visibility
                && a.light_opacity == b.light_opacity
                && a.light_emission == b.light_emission
                && a.flags == b.flags
                && a.look == b.look
                && a.hitbox == b.hitbox
        })
    }
}

/// The name of a block of the engine in the files: `oak_log` for `BlockId::OakLog`.
pub(super) fn engine_name(id: BlockId) -> String {
    snake_case(&format!("{id:?}"))
}

/// The name of an item of the engine in the files: `oak_log` for `ItemId::OakLog`.
pub(super) fn engine_item_name(id: ItemId) -> String {
    snake_case(&format!("{id:?}"))
}

/// Numbers after `known` each name of `defined`, with the file that defines it, that `known`
/// lacks, in the order of the names. Returns those, which must be names a block or an item may
/// have (`is_content_name`).
fn number_new<'a>(
    kind: &str,
    known: &mut Vec<String>,
    defined: impl Iterator<Item = (&'a String, &'a PathBuf)>,
) -> Result<Vec<(String, PathBuf)>, ContentError> {
    let mut new: Vec<(String, PathBuf)> = Vec::new();
    for (name, path) in defined {
        if known.contains(name) || new.iter().any(|(other, _)| other == name) {
            continue;
        }
        if !is_content_name(name) {
            return Err(ContentError {
                file: path.clone(),
                reason: format!(
                    "{kind} \"{name}\": a name is words in lowercase joined by _, as oak_log"
                ),
            });
        }
        new.push((name.clone(), path.clone()));
    }
    new.sort();
    known.extend(new.iter().map(|(name, _)| name.clone()));
    Ok(new)
}

impl NewName {
    /// The new `kind` named `name` in `file`, and the name of `known` it is nearest to, if any
    /// is `MAX_TYPO` edits or fewer away.
    fn near(kind: &'static str, name: String, file: PathBuf, known: &[String]) -> Self {
        let near = known
            .iter()
            .filter(|other| **other != name)
            .map(|other| (edit_distance(&name, other), other))
            .filter(|(distance, _)| *distance <= MAX_TYPO)
            .min_by_key(|(distance, _)| *distance)
            .map(|(_, other)| other.clone());
        Self {
            kind,
            name,
            file,
            near,
        }
    }
}

/// Whether `name` may name a block or an item: words in lowercase and digits joined by `_`,
/// which reads back from its Rust name (`pascal_case`), as `oak_log` from `OakLog`.
pub(super) fn is_content_name(name: &str) -> bool {
    name.starts_with(|c: char| c.is_ascii_lowercase())
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
        && snake_case(&pascal_case(name)) == name
}

/// The blocks and items by their names in the files.
pub(super) struct Names {
    pub blocks: HashMap<String, BlockId>,
    pub items: HashMap<String, ItemId>,
}

impl Names {
    /// The blocks of `block_names` and the items of `item_names`, numbered by their places.
    pub fn new(block_names: &[String], item_names: &[String]) -> Self {
        Self {
            blocks: block_names
                .iter()
                .enumerate()
                .map(|(index, name)| (name.clone(), BlockId::from_index(index as u16)))
                .collect(),
            items: item_names
                .iter()
                .enumerate()
                .map(|(index, name)| (name.clone(), ItemId::from_index(index as u16)))
                .collect(),
        }
    }

    fn block(&self, field: &str, name: &str) -> Result<BlockId, String> {
        self.blocks
            .get(name)
            .copied()
            .ok_or_else(|| format!("{field}: no block is named \"{name}\""))
    }

    pub fn item(&self, field: &str, name: &str) -> Result<ItemId, String> {
        self.items
            .get(name)
            .copied()
            .ok_or_else(|| format!("{field}: no item is named \"{name}\""))
    }

    /// Makes the aliases of the definitions of `files` name what they are written on, an alias
    /// of a block also the item of its name, and returns them: those of the blocks, then those
    /// of the items. An alias is a name no block, or no item, has.
    fn add_aliases(
        &mut self,
        files: &[(PathBuf, ContentFile)],
    ) -> Result<(BTreeMap<String, BlockId>, BTreeMap<String, ItemId>), ContentError> {
        let mut block_aliases = BTreeMap::new();
        let mut item_aliases = BTreeMap::new();
        for (path, file) in files {
            let fail = |kind: &str, name: &str, reason: String| ContentError {
                file: path.clone(),
                reason: format!("{kind} \"{name}\": aliases: {reason}"),
            };
            for (name, def) in &file.blocks.0 {
                for alias in def.aliases.iter().flatten() {
                    let fail = |reason: String| fail("block", name, reason);
                    let block = self.block("name", name).map_err(fail)?;
                    if !is_content_name(alias) {
                        return Err(fail(format!(
                            "\"{alias}\" is not words in lowercase joined by _"
                        )));
                    }
                    if self.blocks.insert(alias.clone(), block).is_some() {
                        return Err(fail(format!("\"{alias}\" already names a block")));
                    }
                    block_aliases.insert(alias.clone(), block);
                    if let Some(&item) = self.items.get(name) {
                        if self.items.insert(alias.clone(), item).is_some() {
                            return Err(fail(format!("\"{alias}\" already names an item")));
                        }
                        item_aliases.insert(alias.clone(), item);
                    }
                }
            }
            for (name, def) in &file.items.0 {
                for alias in def.aliases.iter().flatten() {
                    let fail = |reason: String| fail("item", name, reason);
                    let item = self.item("name", name).map_err(fail)?;
                    if !is_content_name(alias) {
                        return Err(fail(format!(
                            "\"{alias}\" is not words in lowercase joined by _"
                        )));
                    }
                    if self.items.insert(alias.clone(), item).is_some() {
                        return Err(fail(format!("\"{alias}\" already names an item")));
                    }
                    item_aliases.insert(alias.clone(), item);
                }
            }
        }
        Ok((block_aliases, item_aliases))
    }
}

/// Completes each block definition `like` another from that one, itself completed first.
fn complete_likes(files: &mut [(PathBuf, ContentFile)]) -> Result<(), ContentError> {
    let defs: HashMap<String, BlockDef> = files
        .iter()
        .flat_map(|(_, file)| file.blocks.0.iter().cloned())
        .collect();
    for (path, file) in files.iter_mut() {
        for (name, def) in &mut file.blocks.0 {
            if def.like.is_some() {
                *def = completed(std::mem::take(def), &defs, &mut vec![name.clone()]).map_err(
                    |reason| ContentError {
                        file: path.clone(),
                        reason: format!("block \"{name}\": {reason}"),
                    },
                )?;
            }
        }
    }
    Ok(())
}

/// `def` completed from the block it is `like`, of `defs`, `chain` the blocks met on the way.
fn completed(
    def: BlockDef,
    defs: &HashMap<String, BlockDef>,
    chain: &mut Vec<String>,
) -> Result<BlockDef, String> {
    let Some(like) = def.like.clone() else {
        return Ok(def);
    };
    if chain.contains(&like) {
        return Err(format!(
            "like: {} is like {like} again",
            chain.join(" is like ")
        ));
    }
    let base = defs
        .get(&like)
        .ok_or_else(|| format!("like: no block is named \"{like}\""))?;
    chain.push(like);
    let base = completed(base.clone(), defs, chain)?;
    Ok(def.or_like(&base))
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
    let form = props.palette.map(|palette| palette.form);
    if props.toggled.is_some() && !form.is_some_and(Form::opens) {
        return Err("toggled: only a door, a trapdoor or a gate opens and shuts".to_string());
    }
    if props.door_top.is_some()
        && form
            != Some(Form::Door {
                top: false,
                open: false,
            })
    {
        return Err("top: only the lower half of a shut door has a top".to_string());
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
#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub(super) struct ContentFile {
    #[serde(default)]
    pub blocks: Named<BlockDef>,
    #[serde(default)]
    pub items: Named<ItemDef>,
    /// The families of items, by name, each a list of item names (`recipes`).
    #[serde(default)]
    pub tags: Named<TagDef>,
    #[serde(default)]
    pub recipes: Vec<RecipeDef>,
    /// `(input, output)`: what a furnace makes of an item, or of each item of a `#tag`.
    #[serde(default)]
    pub smelting: Vec<(String, String)>,
    /// How many ticks an item, or each item of a `#tag`, burns in a furnace.
    #[serde(default)]
    pub fuel: Named<FuelDef>,
    /// Definitions written once for each member of a list (`families`).
    #[serde(default)]
    pub families: Vec<FamilyDef>,
}

/// What a block of the palette is (`Palette`), its base and its item named as in the files.
#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub(super) struct PaletteDef {
    pub form: Form,
    pub base: String,
    pub item: String,
}

/// The definition of a block in a file: each field the file leaves out takes its default.
#[derive(Deserialize, Default, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub(super) struct BlockDef {
    /// The name the block and its item are shown under, by language.
    #[serde(default, deserialize_with = "present")]
    pub name: Option<NameDef>,
    /// The block whose definition this one takes each field it leaves out from, but its name
    /// and its aliases.
    #[serde(default, deserialize_with = "present")]
    pub like: Option<String>,
    /// The other names the block is found by, its former ones: those of a world's list, of a
    /// RON file and of the commands. Its item, if it has one of its name, is found by them too.
    #[serde(default, deserialize_with = "present")]
    pub aliases: Option<Vec<String>>,
    /// `palette`: what the block is, if it is one of the palette. None by default.
    #[serde(default, deserialize_with = "present")]
    pub palette: Option<PaletteDef>,
    /// `visibility`: `Solid` (the default), `Transparent`, `Liquid` or `Decoration`.
    #[serde(default, deserialize_with = "present")]
    pub look: Option<BlockTransparency>,
    /// `light_emission`, 0 by default.
    #[serde(default, deserialize_with = "present")]
    pub light: Option<u8>,
    /// `light_opacity`. By default, from `look`: none goes through a solid block, a decoration
    /// and a glass take 0, and the other blocks 1.
    #[serde(default, deserialize_with = "present")]
    pub opacity: Option<u8>,
    /// `break_time`: the hits a hand breaks the block in. A block without it cannot be broken.
    #[serde(default, rename = "break", deserialize_with = "present")]
    pub break_hits: Option<u8>,
    /// `faster_tools`: the tools that mine the block faster. None by default.
    #[serde(default, deserialize_with = "present")]
    pub tools: Option<Vec<ToolKind>>,
    /// `required_tool`: `(kind: Pickaxe, level: 1)`. None by default.
    #[serde(default, deserialize_with = "present")]
    pub needs_tool: Option<RequiredTool>,
    /// `drops`: `(chance, "item", count)`, the item named as the commands name it. By default
    /// the item of the block's name, if there is one.
    #[serde(default, deserialize_with = "present")]
    pub drops: Option<Vec<(u32, String, u32)>>,
    /// `hitbox`: `FullBlock` (the default), `None` or `Aabb((min: .., max: ..))`.
    #[serde(default, deserialize_with = "present")]
    pub collision: Option<BlockHitbox>,
    /// `ray_hitbox`, the box aimed at; `collision` by default.
    #[serde(default, deserialize_with = "present")]
    pub outline: Option<BlockHitbox>,
    /// `usable`, false by default, as the four below.
    #[serde(default, deserialize_with = "present")]
    pub usable: Option<bool>,
    #[serde(default, deserialize_with = "present")]
    pub replaceable: Option<bool>,
    #[serde(default, deserialize_with = "present")]
    pub tall: Option<bool>,
    #[serde(default, deserialize_with = "present")]
    pub glass: Option<bool>,
    #[serde(default, deserialize_with = "present")]
    pub container: Option<bool>,
    /// `submerged`: the block grows under water, which fills the rest of its cell.
    #[serde(default, deserialize_with = "present")]
    pub submerged: Option<bool>,
    /// `hangs`: the block hangs from the one above it.
    #[serde(default, deserialize_with = "present")]
    pub hangs: Option<bool>,
    /// `behaviour`: `None` (the default) or `Furnace(lit: false)`.
    #[serde(default, deserialize_with = "present")]
    pub behaviour: Option<Behaviour>,
    /// `sapling`: the item the leaves sometimes drop. None by default.
    #[serde(default, deserialize_with = "present")]
    pub sapling: Option<String>,
    /// `soil`: the blocks this one may be planted on, if it only stands on some.
    #[serde(default, deserialize_with = "present")]
    pub soil: Option<Vec<String>>,
    /// `toggled`: for a door, a trapdoor or a gate, the block it becomes once opened or shut.
    #[serde(default, deserialize_with = "present")]
    pub toggled: Option<String>,
    /// `door_top`: for the lower half of a shut door, its top half, placed above it.
    #[serde(default, deserialize_with = "present")]
    pub top: Option<String>,
    /// `hardens`: the block this one becomes where water touches it.
    #[serde(default, deserialize_with = "present")]
    pub hardens: Option<String>,
    /// The `shape` of `look`: `Cube` by default.
    #[serde(default, deserialize_with = "present")]
    pub shape: Option<Shape>,
    /// The textures of the faces: by default, the texture of the block's name (`OakLog` for
    /// `oak_log`) on every face.
    #[serde(default, deserialize_with = "present")]
    pub faces: Option<FacesDef>,
    /// The texture drawn over the sides, see `Look::overlay`.
    #[serde(default, deserialize_with = "present")]
    pub overlay: Option<String>,
    /// `None` (the default), `Grass`, `Foliage`, `Green` or `Water`.
    #[serde(default, deserialize_with = "present")]
    pub tint: Option<Tint>,
    /// `None` (the default), `Whole` or `Top`.
    #[serde(default, deserialize_with = "present")]
    pub sway: Option<Sway>,
}

/// The textures of the faces of a block, each by its name in the textures: `all` for every
/// face, `sides` for the four around, then each face for itself.
#[derive(Deserialize, Default, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub(super) struct FacesDef {
    #[serde(default, deserialize_with = "present")]
    pub all: Option<String>,
    #[serde(default, deserialize_with = "present")]
    pub sides: Option<String>,
    #[serde(default, deserialize_with = "present")]
    pub top: Option<String>,
    #[serde(default, deserialize_with = "present")]
    pub bottom: Option<String>,
    #[serde(default, deserialize_with = "present")]
    pub front: Option<String>,
    #[serde(default, deserialize_with = "present")]
    pub back: Option<String>,
    #[serde(default, deserialize_with = "present")]
    pub left: Option<String>,
    #[serde(default, deserialize_with = "present")]
    pub right: Option<String>,
}

impl FacesDef {
    /// The texture of each face, in the order of `Look::faces`, `own` where nothing is given.
    fn resolve(self, own: &str) -> [String; 6] {
        let Self {
            all,
            sides,
            top,
            bottom,
            front,
            back,
            left,
            right,
        } = self;
        let all = all.unwrap_or_else(|| own.to_string());
        let sides = sides.unwrap_or_else(|| all.clone());
        [
            top.unwrap_or_else(|| all.clone()),
            bottom.unwrap_or(all),
            front.unwrap_or_else(|| sides.clone()),
            back.unwrap_or_else(|| sides.clone()),
            left.unwrap_or_else(|| sides.clone()),
            right.unwrap_or(sides),
        ]
    }
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
            name: _,
            like: _,
            aliases: _,
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
            submerged,
            hangs,
            behaviour,
            sapling,
            soil,
            toggled,
            top,
            hardens,
            shape,
            faces,
            overlay,
            tint,
            sway,
        } = self;
        names.block("name", name)?;
        let own = pascal_case(name);
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
            flags: [
                (BlockFlags::USABLE, usable),
                (BlockFlags::REPLACEABLE, replaceable),
                (BlockFlags::TALL, tall),
                (BlockFlags::GLASS, Some(glass)),
                (BlockFlags::CONTAINER, container),
                (BlockFlags::SUBMERGED, submerged),
                (BlockFlags::HANGS, hangs),
            ]
            .into_iter()
            .fold(BlockFlags::default(), |mut flags, (flag, on)| {
                flags.set(flag, on.unwrap_or(false));
                flags
            }),
            behaviour: behaviour.unwrap_or_default(),
            sapling: sapling
                .map(|sapling| names.item("sapling", &sapling))
                .transpose()?,
            soil,
            toggled: toggled
                .map(|block| names.block("toggled", &block))
                .transpose()?,
            door_top: top.map(|block| names.block("top", &block)).transpose()?,
            hardens: hardens
                .map(|block| names.block("hardens", &block))
                .transpose()?,
            look: Look {
                shape: shape.unwrap_or_default(),
                faces: faces.unwrap_or_default().resolve(&own),
                overlay,
                tint: tint.unwrap_or_default(),
                sway: sway.unwrap_or_default(),
            },
        })
    }
}

impl Texts for BlockDef {
    fn texts(&mut self, each: &mut dyn FnMut(&mut String)) {
        // every field, so that a new one is not forgotten
        let Self {
            name,
            like,
            aliases,
            palette,
            look: _,
            light: _,
            opacity: _,
            break_hits: _,
            tools: _,
            needs_tool: _,
            drops,
            collision: _,
            outline: _,
            usable: _,
            replaceable: _,
            tall: _,
            glass: _,
            container: _,
            submerged: _,
            hangs: _,
            behaviour: _,
            sapling,
            soil,
            toggled,
            top,
            hardens,
            shape: _,
            faces,
            overlay,
            tint: _,
            sway: _,
        } = self;
        name.texts(each);
        like.texts(each);
        aliases.texts(each);
        if let Some(PaletteDef { base, item, .. }) = palette {
            each(base);
            each(item);
        }
        for (_, item, _) in drops.iter_mut().flatten() {
            each(item);
        }
        sapling.texts(each);
        soil.texts(each);
        toggled.texts(each);
        top.texts(each);
        hardens.texts(each);
        if let Some(FacesDef {
            all,
            sides,
            top,
            bottom,
            front,
            back,
            left,
            right,
        }) = faces
        {
            for face in [all, sides, top, bottom, front, back, left, right] {
                face.texts(each);
            }
        }
        overlay.texts(each);
    }
}

impl BlockDef {
    /// This definition, with each field it leaves out taken from `base`, but the name and the
    /// aliases: what `like` gives.
    fn or_like(self, base: &BlockDef) -> BlockDef {
        let Self {
            name,
            like,
            aliases,
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
            submerged,
            hangs,
            behaviour,
            sapling,
            soil,
            toggled,
            top,
            hardens,
            shape,
            faces,
            overlay,
            tint,
            sway,
        } = self;
        BlockDef {
            name,
            like,
            aliases,
            palette: palette.or_else(|| base.palette.clone()),
            look: look.or(base.look),
            light: light.or(base.light),
            opacity: opacity.or(base.opacity),
            break_hits: break_hits.or(base.break_hits),
            tools: tools.or_else(|| base.tools.clone()),
            needs_tool: needs_tool.or(base.needs_tool),
            drops: drops.or_else(|| base.drops.clone()),
            collision: collision.or(base.collision),
            outline: outline.or(base.outline),
            usable: usable.or(base.usable),
            replaceable: replaceable.or(base.replaceable),
            tall: tall.or(base.tall),
            glass: glass.or(base.glass),
            container: container.or(base.container),
            submerged: submerged.or(base.submerged),
            hangs: hangs.or(base.hangs),
            behaviour: behaviour.or(base.behaviour),
            sapling: sapling.or_else(|| base.sapling.clone()),
            soil: soil.or_else(|| base.soil.clone()),
            toggled: toggled.or_else(|| base.toggled.clone()),
            top: top.or_else(|| base.top.clone()),
            hardens: hardens.or_else(|| base.hardens.clone()),
            shape: shape.or(base.shape),
            faces: faces.or_else(|| base.faces.clone()),
            overlay: overlay.or_else(|| base.overlay.clone()),
            tint: tint.or(base.tint),
            sway: sway.or(base.sway),
        }
    }
}

impl Texts for NameDef {
    fn texts(&mut self, each: &mut dyn FnMut(&mut String)) {
        let Self { en, fr } = self;
        en.texts(each);
        fr.texts(each);
    }
}

impl Texts for ItemDef {
    fn texts(&mut self, each: &mut dyn FnMut(&mut String)) {
        // every field, so that a new one is not forgotten
        let Self {
            name,
            aliases,
            stack: _,
            attack_damage: _,
            food_value: _,
            food_effect: _,
            eaten_remains,
            crafting_remains,
            horse_armour: _,
            spawned_mob: _,
            tool: _,
            durability: _,
            category: _,
            places,
            default_type: _,
        } = self;
        name.texts(each);
        aliases.texts(each);
        eaten_remains.texts(each);
        crafting_remains.texts(each);
        places.texts(each);
    }
}

/// The name of a block or an item in each language, as a file writes it: `(en: "Amber", fr:
/// "Ambre")`. A language it leaves out takes the name of the language files.
#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub(super) struct NameDef {
    #[serde(default, deserialize_with = "present")]
    pub en: Option<String>,
    #[serde(default, deserialize_with = "present")]
    pub fr: Option<String>,
}

impl From<NameDef> for DisplayName {
    fn from(NameDef { en, fr }: NameDef) -> Self {
        Self { en, fr }
    }
}

/// The definition of an item in a file: each field the file leaves out takes its default.
#[derive(Deserialize, Default, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub(super) struct ItemDef {
    /// The name the item is shown under, by language.
    #[serde(default, deserialize_with = "present")]
    pub name: Option<NameDef>,
    /// The other names the item is found by, as those of a block (`BlockDef::aliases`).
    #[serde(default, deserialize_with = "present")]
    pub aliases: Option<Vec<String>>,
    /// `max_stack`, 64 by default.
    #[serde(default, deserialize_with = "present")]
    pub stack: Option<u32>,
    /// 1 by default.
    #[serde(default, deserialize_with = "present")]
    pub attack_damage: Option<u32>,
    /// `(hunger, saturation)`.
    #[serde(default, deserialize_with = "present")]
    pub food_value: Option<(u8, f32)>,
    /// `(effect, level, ticks)`.
    #[serde(default, deserialize_with = "present")]
    pub food_effect: Option<(EffectKind, u8, u32)>,
    /// The item left in the hand once this one is eaten.
    #[serde(default, deserialize_with = "present")]
    pub eaten_remains: Option<String>,
    /// The item left in the crafting grid once this one is used.
    #[serde(default, deserialize_with = "present")]
    pub crafting_remains: Option<String>,
    #[serde(default, deserialize_with = "present")]
    pub horse_armour: Option<u32>,
    #[serde(default, deserialize_with = "present")]
    pub spawned_mob: Option<MobKind>,
    #[serde(default, deserialize_with = "present")]
    pub tool: Option<Tool>,
    #[serde(default, deserialize_with = "present")]
    pub durability: Option<u32>,
    /// `Blocks` by default.
    #[serde(default, deserialize_with = "present")]
    pub category: Option<ItemCategory>,
    /// The block the item places: by default the block of its name, if there is one.
    #[serde(default, deserialize_with = "present")]
    pub places: Option<String>,
    /// `default_type`, for an item that places no block: `Generic` by default, `Tool(wear:
    /// 0)` for one that wears out.
    #[serde(default, deserialize_with = "present")]
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
            name: _,
            aliases: _,
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
            placeholder: false,
        })
    }
}

/// A field the file writes: `light: 10` is `Some(10)`, a field it leaves out `None`.
pub(super) fn present<'de, D: Deserializer<'de>, T: Deserialize<'de>>(
    deserializer: D,
) -> Result<Option<T>, D::Error> {
    T::deserialize(deserializer).map(Some)
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
                 outline: Aabb((min: (0.35, 0, 0.35), max: (0.65, 0.6, 0.65))), shape: Torch)",
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

        // and the blocks like the stone, which take what their files leave out from it
        let touched: Vec<BlockId> = BlockId::ENGINE
            .into_iter()
            .filter(|&id| content.block(id) != game.block(id))
            .collect();
        assert_eq!(
            touched,
            [
                BlockId::Stone,
                BlockId::Torch,
                BlockId::StoneSlab,
                BlockId::StoneStairs,
                BlockId::StoneLowWall,
                BlockId::PolishedStone,
                BlockId::ChiseledStone,
            ]
        );
        let slab = content.block(BlockId::StoneSlab);
        assert_eq!(slab.required_tool, stone.required_tool);
        assert_eq!(slab.drops, game.block(BlockId::StoneSlab).drops);
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
        let game = Content::load(&game_folder()).unwrap();
        let names = Names::new(&game.block_names, &game.item_names);
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

    #[test]
    fn the_look_of_a_block_reads_its_faces_from_the_widest_to_the_narrowest() {
        let game = Content::load(&game_folder()).unwrap();
        let names = Names::new(&game.block_names, &game.item_names);
        let def: BlockDef = ron::de::from_str(
            r#"(faces: (all: "A", sides: "S", front: "F", top: "T"), shape: Flora,
                tint: Foliage, sway: Top, overlay: "O")"#,
        )
        .unwrap();
        let look = def.build("dirt", &names).unwrap().look;
        assert_eq!(look.faces, ["T", "A", "F", "S", "S", "S"].map(String::from));
        assert_eq!(look.shape, Shape::Flora);
        assert_eq!(look.tint, Tint::Foliage);
        assert_eq!(look.sway, Sway::Top);
        assert_eq!(look.overlay.as_deref(), Some("O"));

        let plain = BlockDef::default().build("oak_log", &names).unwrap().look;
        assert_eq!(plain.faces, ["OakLog"; 6].map(String::from));
        assert_eq!(plain.shape, Shape::Cube);
        assert_eq!(
            (plain.tint, plain.sway, plain.overlay),
            (Tint::None, Sway::None, None)
        );
    }

    #[test]
    fn the_flags_and_the_behaviour_of_the_game_blocks() {
        let game = Content::load(&game_folder()).unwrap();
        let flagged = |flag: BlockFlags| -> Vec<BlockId> {
            BlockId::ENGINE
                .into_iter()
                .filter(|id| game.block(*id).flags.contains(flag))
                .collect()
        };
        assert_eq!(
            flagged(BlockFlags::SUBMERGED),
            [BlockId::Kelp, BlockId::Seagrass]
        );
        assert_eq!(flagged(BlockFlags::HANGS), [BlockId::CaveVine]);
        assert_eq!(
            flagged(BlockFlags::CONTAINER),
            [BlockId::Chest, BlockId::Barrel]
        );
        let furnaces: Vec<_> = BlockId::ENGINE
            .into_iter()
            .filter_map(|id| match game.block(id).behaviour {
                Behaviour::Furnace { lit } => Some((id, lit)),
                Behaviour::None => None,
            })
            .collect();
        assert_eq!(
            furnaces,
            [(BlockId::Furnace, false), (BlockId::LitFurnace, true)]
        );
    }

    #[test]
    fn a_new_collision_box_draws_otherwise() {
        let folder = edited_game_folder(&[(
            "lantern",
            "(look: Decoration, light: 15, break: 8, tools: [Pickaxe], shape: Box, \
             collision: Aabb((min: (0.25, 0, 0.25), max: (0.75, 0.5, 0.75))))",
        )]);
        let content = Content::load(folder.path()).unwrap();
        assert!(!content.draws_like(&Content::load(&game_folder()).unwrap()));
    }

    #[test]
    fn a_new_look_draws_otherwise() {
        let folder = edited_game_folder(&[("dirt", "(break: 10, tools: [Shovel], tint: Grass)")]);
        let content = Content::load(folder.path()).unwrap();
        assert!(!content.draws_like(&Content::load(&game_folder()).unwrap()));
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
            (
                r#"(blocks: {"Tourch": (light: 9)})"#,
                "block \"Tourch\": a name is",
            ),
            (
                r#"(blocks: {"tourch_2": (light: 9)})"#,
                "block \"tourch_2\"",
            ),
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
            (
                r#"(blocks: {"stone": (toggled: "dirt")})"#,
                "block \"stone\": toggled: only a door, a trapdoor or a gate",
            ),
            (
                r#"(blocks: {"stone": (top: "dirt")})"#,
                "block \"stone\": top: only the lower half of a shut door",
            ),
            (
                r#"(blocks: {"sand": (hardens: "sandstone")})"#,
                "block \"sand\": hardens: no block is named \"sandstone\"",
            ),
        ];
        for (text, key) in cases {
            let error = load(&[("good.ron", "()"), ("bad.ron", text)]).unwrap_err();
            assert_eq!(error.file.file_name().unwrap(), "bad.ron", "{text}");
            assert!(error.to_string().contains(key), "{text}: {error}");
        }
    }

    /// A copy of the content files of the game, with the file `extra.ron` of `text`.
    pub fn game_folder_with(text: &str) -> tempfile::TempDir {
        let folder = edited_game_folder(&[]);
        std::fs::write(folder.path().join("extra.ron"), text).unwrap();
        folder
    }

    #[test]
    fn a_block_only_the_files_define_is_numbered_after_the_engine_by_name() {
        let folder =
            game_folder_with(r#"(blocks: {"glow_moss": (light: 7, break: 3), "amber": ()})"#);
        let content = Content::load(folder.path()).unwrap();
        let engine = BlockId::ENGINE.len();
        let [amber, moss] = [engine, engine + 1].map(|index| BlockId::from_index(index as u16));
        assert_eq!(content.blocks.len(), engine + 2);
        assert_eq!(content.block_named("amber"), Some(amber));
        assert_eq!(content.block_named("glow_moss"), Some(moss));
        assert_eq!(content.block_named("stone"), Some(BlockId::Stone));
        assert_eq!(content.block_idents[moss.index()], "GlowMoss");
        let moss = content.block(moss);
        assert_eq!((moss.light_emission, moss.break_time), (7, Some(3)));
        assert_eq!(moss.look.faces, ["GlowMoss"; 6].map(String::from));
        // each has its item, numbered after those of the engine, which places it
        let items = ItemId::ENGINE.len();
        let moss_item = ItemId::from_index(items as u16 + 1);
        assert_eq!(content.items.len(), items + 2);
        assert_eq!(content.item_names[items..], ["amber", "glow_moss"]);
        assert_eq!(content.item_idents[items + 1], "GlowMoss");
        assert_eq!(content.item_named("glow_moss"), Some(moss_item));
        assert_eq!(moss.drops, vec![(1, moss_item, 1)], "it drops its item");
        let item = content.item(moss_item);
        assert_eq!(
            item.default_type,
            ItemType::Block(BlockId::from_index(engine as u16 + 1))
        );
        assert_eq!((item.max_stack, item.category), (64, ItemCategory::Blocks));
        assert!(!content.same_properties(&Content::load(&game_folder()).unwrap()));
        assert!(!content.draws_like(&Content::load(&game_folder()).unwrap()));
    }

    #[test]
    fn an_item_only_the_files_define_is_numbered_after_the_items_of_the_blocks_by_name() {
        let folder = game_folder_with(
            r#"(
                blocks: {"glow_moss": (light: 7, drops: [(1, "moss_spore", 2)])},
                items: {
                    "moss_spore": (stack: 16, category: Nature, places: "glow_moss"),
                    "glow_moss": (category: Nature),
                },
            )"#,
        );
        let content = Content::load(folder.path()).unwrap();
        let items = ItemId::ENGINE.len();
        let [moss, spore] = [items, items + 1].map(|index| ItemId::from_index(index as u16));
        assert_eq!(content.item_names[items..], ["glow_moss", "moss_spore"]);
        // the entry of the item of a block changes it
        assert_eq!(content.item(moss).category, ItemCategory::Nature);
        let spore = content.item(spore);
        assert_eq!(
            (spore.max_stack, spore.category),
            (16, ItemCategory::Nature)
        );
        let block = content.block_named("glow_moss").unwrap();
        assert_eq!(spore.default_type, ItemType::Block(block));
        assert_eq!(
            content.block(block).drops,
            vec![(1, ItemId::from_index(items as u16 + 1), 2)]
        );
        // an engine block has no item but the engine's
        assert_eq!(content.item_named("water"), None);
        let error = load(&[("bad.ron", r#"(items: {"Spore": ()})"#)]).unwrap_err();
        assert!(
            error
                .reason
                .contains("item \"Spore\": a name is words in lowercase"),
            "{error}"
        );
    }

    #[test]
    fn a_reload_keeps_the_numbers_of_the_items_and_holds_the_place_of_a_removed_one() {
        let before = Content::load(
            game_folder_with(r#"(blocks: {"amber": ()}, items: {"ruby": (stack: 8)})"#).path(),
        )
        .unwrap();
        let after = Content::reload(
            game_folder_with(r#"(items: {"opal": (), "ruby": (stack: 4)})"#).path(),
            &before,
        )
        .unwrap();
        let items = ItemId::ENGINE.len();
        assert_eq!(after.item_names[items..], ["amber", "ruby", "opal"]);
        assert_eq!(
            after.items[items],
            ItemProps::PLACEHOLDER,
            "the block is gone"
        );
        assert_eq!(after.items[items + 1].max_stack, 4);
        assert_eq!(after.items[items + 2].default_type, ItemType::Generic);
        assert_eq!(after.flaw(), None);
    }

    #[test]
    fn a_reload_keeps_the_numbers_and_holds_the_place_of_a_removed_block() {
        let before = Content::load(
            game_folder_with(r#"(blocks: {"glow_moss": (light: 7), "amber": ()})"#).path(),
        )
        .unwrap();
        let after = Content::reload(
            game_folder_with(r#"(blocks: {"zinc": (break: 5), "glow_moss": (light: 3)})"#).path(),
            &before,
        )
        .unwrap();
        let engine = BlockId::ENGINE.len();
        assert_eq!(after.block_names[engine..], ["amber", "glow_moss", "zinc"]);
        assert_eq!(after.blocks[engine + 1].light_emission, 3);
        assert_eq!(after.blocks[engine + 2].break_time, Some(5));
        let debug = after.block(BlockId::Debug);
        assert_eq!(after.blocks[engine], BlockProps::placeholder(debug));
        assert_eq!(after.blocks[engine].look, debug.look);
        assert_eq!(after.blocks[engine].drops, vec![]);
        // the same files numbered afresh
        let fresh = Content::load(
            game_folder_with(r#"(blocks: {"zinc": (break: 5), "glow_moss": (light: 3)})"#).path(),
        )
        .unwrap();
        assert_eq!(fresh.block_names[engine..], ["glow_moss", "zinc"]);
    }

    #[test]
    fn the_engine_blocks_cannot_leave_the_files() {
        let folder = edited_game_folder(&[]);
        let blocks = folder.path().join("blocks.ron");
        let text = std::fs::read_to_string(&blocks).unwrap();
        let without_debug: String = text
            .lines()
            .filter(|line| !line.trim_start().starts_with("\"debug\":"))
            .map(|line| format!("{line}\n"))
            .collect();
        std::fs::write(&blocks, without_debug).unwrap();
        let game = Content::load(&game_folder()).unwrap();
        let error = Content::reload(folder.path(), &game).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("block \"debug\" is defined in no file"),
            "{error}"
        );
    }

    #[test]
    fn a_new_block_is_told_and_one_named_close_to_another_may_be_a_typo() {
        let folder = game_folder_with(
            r#"(blocks: {"tourch": (light: 9), "glow_moss": ()}, items: {"stik": (stack: 2)})"#,
        );
        let (content, new) = Content::load_telling(folder.path(), None).unwrap();
        let told: Vec<String> = new.iter().map(ToString::to_string).collect();
        assert_eq!(
            told,
            [
                "block \"glow_moss\" in extra.ron is new",
                "block \"tourch\" in extra.ron is new; did you mean \"torch\"?",
                "item \"stik\" in extra.ron is new; did you mean \"stick\"?",
            ],
            "the items of the new blocks are not told apart"
        );
        assert_eq!(
            content.item(ItemId::Stick).max_stack,
            64,
            "the stick is untouched"
        );
        assert_eq!(
            content.block(BlockId::Torch).light_emission,
            14,
            "the torch is untouched"
        );
        // a reload tells only the blocks it numbers anew
        let (_, again) = Content::load_telling(folder.path(), Some(&content)).unwrap();
        assert_eq!(again, []);
        assert_eq!(edit_distance("tourch", "torch"), 1);
        assert_eq!(edit_distance("stone", "tones"), 2);
        assert_eq!(edit_distance("", "ab"), 2);
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
