//! The content of the game: the blocks and the recipes that mods declare in their `blocks.toml`
//! and `recipes.toml`. The engine knows no block of its own: the base mod declares them, then
//! each mod that is on as the game starts adds its own, after them. They are read once, and
//! every property of a `BlockId` is a read in this table.
//!
//! Still in the engine, for now: the names of the base blocks (`BlockId::Stone`), which the
//! terrain and the mechanics use, and which reading the file checks; the items of the base
//! blocks; how a base block is drawn and sounds. A block of a mod is a full cube with its
//! texture, and its item is `ItemId::Block`. See "Base mod" in docs/ARCHITECTURE.md.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use bevy::math::{bounding::Aabb3d, Vec3A};
use bevy_log::{info, warn};
use serde::Deserialize;

use super::crafting::{Recipe, CRAFTING_GRID_SIZE};
use super::{BlockId, BlockTransparency, ItemId, ToolKind};
use crate::mods::{
    find_game_mods, parse_manifest, plan_loading, read_mods_settings, MANIFEST_FILE,
};

/// The id of the base mod, and the name of its folder in `mods/` of the data folder.
pub const BASE_MOD: &str = "rustcraft";
/// The blocks of a mod, beside its `mod.toml`.
pub const BLOCKS_FILE: &str = "blocks.toml";
/// The recipes of a mod, beside its `mod.toml`.
pub const RECIPES_FILE: &str = "recipes.toml";
/// The textures of the blocks of a mod, in its folder: `lamp.png` for its block `lamp`.
pub const BLOCK_TEXTURES: &str = "textures/blocks";
/// Names the folder of the base mod instead of the one beside the game.
pub const BASE_MOD_VARIABLE: &str = "RUSTCRAFT_BASE_MOD";

/// A box of a block, for what collides with it or what the cursor aims at.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BlockShape {
    Full,
    Empty,
    /// In the cell, from 0 to 1.
    Box(Aabb3d),
}

/// A block, as its mod declares it.
#[derive(Debug, Clone, PartialEq)]
pub struct BlockDef {
    /// In snake case, as in the file: `oak_log`.
    pub id: String,
    /// The id of its mod.
    pub mod_id: String,
    /// What debug messages, the saves in RON and the textures call it: `OakLog` for a base
    /// block the engine names, its full id, `beacon:lamp`, for any other.
    pub name: &'static str,
    /// What a player reads: `Oak Log`, or the `name` of the file.
    pub display: String,
    /// The mining hits it takes by hand, `None` for a block that cannot be broken.
    pub break_time: Option<u8>,
    pub tools: Vec<ToolKind>,
    pub needs_tool: bool,
    /// `(relative_chance, item, count)`.
    pub drops: Vec<(u32, ItemId, u32)>,
    pub look: BlockTransparency,
    pub collision: BlockShape,
    pub outline: BlockShape,
    pub light: u8,
    pub light_opacity: Option<u8>,
    pub usable: bool,
    pub replaceable: bool,
}

impl BlockDef {
    /// `beacon:lamp`.
    pub fn full_id(&self) -> String {
        format!("{}:{}", self.mod_id, self.id)
    }
}

/// A mod that added content, and where it lies.
#[derive(Debug, Clone, PartialEq)]
pub struct ContentMod {
    pub id: String,
    pub folder: PathBuf,
}

/// The blocks, by number, and the recipes the mods add to those of the engine.
#[derive(Debug, Clone, PartialEq)]
pub struct Content {
    blocks: Vec<BlockDef>,
    /// By full id, `rustcraft:stone`, and, for the base blocks, by id and camel case name.
    by_name: HashMap<String, BlockId>,
    pub(crate) recipes: Vec<Recipe>,
    /// The base mod first, then the mods that added content, in the order they did.
    pub mods: Vec<ContentMod>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BlocksFile {
    blocks: Vec<BlockEntry>,
}

/// A block of `blocks.toml`. Unlike a manifest, it refuses keys it does not know: a misspelt
/// one would leave a block with a default the mod did not mean.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BlockEntry {
    id: String,
    name: Option<String>,
    break_steps: Option<u8>,
    #[serde(default)]
    tools: Vec<ToolKind>,
    #[serde(default)]
    needs_tool: bool,
    #[serde(default)]
    drops: Vec<DropEntry>,
    #[serde(default = "solid")]
    look: BlockTransparency,
    #[serde(default)]
    collision: ShapeEntry,
    #[serde(default)]
    outline: ShapeEntry,
    #[serde(default)]
    light: u8,
    light_opacity: Option<u8>,
    #[serde(default)]
    usable: bool,
    #[serde(default)]
    replaceable: bool,
}

fn solid() -> BlockTransparency {
    BlockTransparency::Solid
}

fn one() -> u32 {
    1
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DropEntry {
    item: String,
    #[serde(default = "one")]
    count: u32,
    #[serde(default = "one")]
    chance: u32,
}

#[derive(Deserialize, Default)]
#[serde(untagged)]
enum ShapeEntry {
    #[default]
    #[serde(skip)]
    Unset,
    Named(NamedShape),
    Box {
        center: [f32; 3],
        half: [f32; 3],
    },
}

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum NamedShape {
    Full,
    None,
}

impl ShapeEntry {
    fn shape(&self) -> BlockShape {
        match self {
            ShapeEntry::Unset | ShapeEntry::Named(NamedShape::Full) => BlockShape::Full,
            ShapeEntry::Named(NamedShape::None) => BlockShape::Empty,
            ShapeEntry::Box { center, half } => {
                BlockShape::Box(Aabb3d::new(Vec3A::from(*center), Vec3A::from(*half)))
            }
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RecipesFile {
    recipes: Vec<RecipeEntry>,
}

/// A shaped recipe of `recipes.toml`: its rows, a character a cell, a space for an empty one,
/// and the item of each character.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RecipeEntry {
    pattern: Vec<String>,
    key: HashMap<char, String>,
    output: String,
    #[serde(default = "one")]
    count: u32,
}

/// `oak_log` as `OakLog`.
fn camel_case(id: &str) -> String {
    id.split('_')
        .map(|word| {
            let mut chars = word.chars();
            chars.next().map_or_else(String::new, |first| {
                first.to_ascii_uppercase().to_string() + chars.as_str()
            })
        })
        .collect()
}

/// `oak_log` as `Oak Log`.
fn title_case(id: &str) -> String {
    let words: Vec<String> = id.split('_').map(camel_case).collect();
    words.join(" ")
}

fn is_snake_case(id: &str) -> bool {
    !id.is_empty()
        && id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

impl Content {
    fn empty() -> Self {
        Content {
            blocks: Vec::new(),
            by_name: HashMap::new(),
            recipes: Vec::new(),
            mods: Vec::new(),
        }
    }

    /// The blocks of the base mod's `blocks.toml`, numbered in their order. Each base block the
    /// engine names must be at its number.
    pub fn parse(text: &str) -> Result<Self, String> {
        let mut content = Content::empty();
        content.add_blocks(BASE_MOD, text)?;
        for (expected, id) in BlockId::BASE_NAMES.iter().zip(BlockId::ALL) {
            match content.blocks.get(id.number() as usize) {
                Some(def) if def.id == *expected => {}
                Some(def) => {
                    return Err(format!(
                        "block {} is \"{}\", where the engine expects \"{expected}\"",
                        id.number(),
                        def.id
                    ))
                }
                None => return Err(format!("\"{expected}\" is missing")),
            }
        }
        Ok(content)
    }

    /// The content of the base mod in `folder`, from its `blocks.toml` and its `recipes.toml`
    /// if it has one, once its `mod.toml` reads.
    pub fn read(folder: &Path) -> Result<Self, String> {
        let manifest = folder.join(MANIFEST_FILE);
        let text = std::fs::read_to_string(&manifest)
            .map_err(|err| format!("{}: {err}", manifest.display()))?;
        parse_manifest(&text).map_err(|err| format!("{}: {err}", manifest.display()))?;
        let blocks = folder.join(BLOCKS_FILE);
        let text = std::fs::read_to_string(&blocks)
            .map_err(|err| format!("{}: {err}", blocks.display()))?;
        let mut content =
            Self::parse(&text).map_err(|err| format!("{}: {err}", blocks.display()))?;
        content.add_recipes_of(BASE_MOD, folder)?;
        content.mods.push(ContentMod {
            id: BASE_MOD.into(),
            folder: folder.to_path_buf(),
        });
        Ok(content)
    }

    /// Adds the blocks and the recipes of the mod `id` in `folder`, after those there: all of
    /// them, or, when one of its files does not read, none.
    pub fn add_mod(&mut self, id: &str, folder: &Path) -> Result<(), String> {
        let mut with = self.clone();
        let blocks = folder.join(BLOCKS_FILE);
        let mut added = false;
        if blocks.is_file() {
            let text = std::fs::read_to_string(&blocks)
                .map_err(|err| format!("{}: {err}", blocks.display()))?;
            with.add_blocks(id, &text)
                .map_err(|err| format!("{BLOCKS_FILE}: {err}"))?;
            added = true;
        }
        added |= with.add_recipes_of(id, folder)?;
        if added {
            with.mods.push(ContentMod {
                id: id.into(),
                folder: folder.to_path_buf(),
            });
            *self = with;
        }
        Ok(())
    }

    /// Adds the blocks of `text`, a `blocks.toml` of the mod `mod_id`, after those there.
    fn add_blocks(&mut self, mod_id: &str, text: &str) -> Result<(), String> {
        let file: BlocksFile = toml::from_str(text).map_err(|err| err.to_string())?;
        if self.blocks.len() + file.blocks.len() > u16::MAX as usize {
            return Err(format!("more than {} blocks", u16::MAX));
        }
        let first = self.blocks.len();
        // numbered first, so that a drop may name a block declared after its own
        for (at, entry) in file.blocks.iter().enumerate() {
            let number = first + at;
            if !is_snake_case(&entry.id) {
                return Err(format!(
                    "block {number}: \"{}\" is not an id in snake case",
                    entry.id
                ));
            }
            let full_id = format!("{mod_id}:{}", entry.id);
            if self.by_name.contains_key(&full_id) {
                return Err(format!("\"{}\" is there twice", entry.id));
            }
            self.by_name
                .insert(full_id, BlockId::from_number(number as u16));
        }
        for (at, entry) in file.blocks.into_iter().enumerate() {
            let id = BlockId::from_number((first + at) as u16);
            if let Some(drop) = entry.drops.iter().find(|d| d.count == 0 || d.chance == 0) {
                return Err(format!(
                    "{}: the drop of {} has a count or a chance of 0",
                    entry.id, drop.item
                ));
            }
            if entry.needs_tool && entry.tools.is_empty() {
                return Err(format!("{}: needs_tool, but no tool", entry.id));
            }
            if entry.light > 15 {
                return Err(format!(
                    "{}: a light of {}, above 15",
                    entry.id, entry.light
                ));
            }
            let drops = entry
                .drops
                .iter()
                .map(|drop| {
                    let item = self.item_of(mod_id, &drop.item).ok_or_else(|| {
                        format!("{}: no item is called \"{}\"", entry.id, drop.item)
                    })?;
                    Ok((drop.chance, item, drop.count))
                })
                .collect::<Result<_, String>>()?;
            let named = id.is_named();
            let name = match named {
                true => camel_case(&entry.id),
                false => format!("{mod_id}:{}", entry.id),
            };
            if named {
                self.by_name.insert(entry.id.clone(), id);
                self.by_name.insert(name.clone(), id);
            }
            self.blocks.push(BlockDef {
                // a few dozen, read once: what Debug and serde need, for the whole run
                name: Box::leak(name.into_boxed_str()),
                display: entry.name.unwrap_or_else(|| title_case(&entry.id)),
                mod_id: mod_id.into(),
                id: entry.id,
                break_time: entry.break_steps.map(|steps| steps.saturating_mul(2)),
                tools: entry.tools,
                needs_tool: entry.needs_tool,
                drops,
                look: entry.look,
                collision: entry.collision.shape(),
                outline: entry.outline.shape(),
                light: entry.light,
                light_opacity: entry.light_opacity,
                usable: entry.usable,
                replaceable: entry.replaceable,
            });
        }
        Ok(())
    }

    /// Adds the recipes of the `recipes.toml` of the mod `mod_id` in `folder`, if it has one.
    fn add_recipes_of(&mut self, mod_id: &str, folder: &Path) -> Result<bool, String> {
        let path = folder.join(RECIPES_FILE);
        if !path.is_file() {
            return Ok(false);
        }
        let text = std::fs::read_to_string(&path)
            .map_err(|err| format!("{}: {err}", path.display()))?;
        self.add_recipes(mod_id, &text)
            .map_err(|err| format!("{RECIPES_FILE}: {err}"))?;
        Ok(true)
    }

    /// Adds the recipes of `text`, a `recipes.toml` of the mod `mod_id`, after those there.
    fn add_recipes(&mut self, mod_id: &str, text: &str) -> Result<(), String> {
        let file: RecipesFile = toml::from_str(text).map_err(|err| err.to_string())?;
        for (number, entry) in file.recipes.into_iter().enumerate() {
            let recipe = number + 1;
            let size = CRAFTING_GRID_SIZE as usize;
            let rows: Vec<Vec<char>> = entry.pattern.iter().map(|r| r.chars().collect()).collect();
            let width = rows.first().map_or(0, Vec::len);
            if rows.is_empty()
                || rows.len() > size
                || width == 0
                || width > size
                || rows.iter().any(|row| row.len() != width)
            {
                return Err(format!(
                    "recipe {recipe}: the pattern must be 1 to {size} rows of 1 to {size} \
                     characters, all as long"
                ));
            }
            let empty_row = |row: &Vec<char>| row.iter().all(|&c| c == ' ');
            let empty_col = |col: usize| rows.iter().all(|row| row[col] == ' ');
            if empty_row(&rows[0])
                || empty_row(&rows[rows.len() - 1])
                || empty_col(0)
                || empty_col(width - 1)
            {
                return Err(format!(
                    "recipe {recipe}: the pattern has an empty row or column at its edge"
                ));
            }
            let mut cells = Vec::with_capacity(width * rows.len());
            for c in rows.iter().flatten() {
                if *c == ' ' {
                    cells.push(None);
                    continue;
                }
                let name = entry
                    .key
                    .get(c)
                    .ok_or_else(|| format!("recipe {recipe}: '{c}' is not in its key"))?;
                let item = self
                    .item_of(mod_id, name)
                    .ok_or_else(|| format!("recipe {recipe}: no item is called \"{name}\""))?;
                cells.push(Some(item));
            }
            let output = self
                .item_of(mod_id, &entry.output)
                .ok_or_else(|| format!("recipe {recipe}: no item is called \"{}\"", entry.output))?;
            if entry.count == 0 {
                return Err(format!("recipe {recipe}: a count of 0"));
            }
            self.recipes.push(Recipe {
                width,
                // read once, as the names of the blocks
                cells: Box::leak(cells.into_boxed_slice()),
                output,
                nb: entry.count,
            });
        }
        Ok(())
    }

    /// The item `name` names in the files of the mod `mod_id`: a block of that mod by its id,
    /// any block by its full id (`beacon:lamp`), else an item of the engine, `oak_planks` or
    /// `OakPlanks`.
    fn item_of(&self, mod_id: &str, name: &str) -> Option<ItemId> {
        let full_id = match name.contains(':') {
            true => name.to_string(),
            false => format!("{mod_id}:{name}"),
        };
        match self.by_name.get(&full_id) {
            Some(&block) if !block.is_named() => Some(ItemId::Block(block)),
            Some(_) | None => {
                let local = name.rsplit(':').next().unwrap_or(name);
                ItemId::from_base_name(local)
            }
        }
    }

    pub fn len(&self) -> usize {
        self.blocks.len()
    }

    pub fn is_empty(&self) -> bool {
        self.blocks.is_empty()
    }

    /// The block of this number, if there is one.
    pub fn get(&self, id: BlockId) -> Option<&BlockDef> {
        self.blocks.get(id.number() as usize)
    }

    /// The block named so: by its full id, `beacon:lamp`, or, for a base block the engine
    /// names, in snake case (`oak_log`) or in camel case (`OakLog`).
    pub fn by_name(&self, name: &str) -> Option<BlockId> {
        self.by_name.get(name).copied()
    }

    /// Every block, in the order of their numbers.
    pub fn ids(&self) -> impl Iterator<Item = BlockId> + '_ {
        (0..self.blocks.len()).map(|number| BlockId::from_number(number as u16))
    }

    /// The blocks the engine does not name, which the mods added: those with an item of
    /// their own, `ItemId::Block`.
    pub fn mod_blocks(&self) -> impl Iterator<Item = BlockId> + '_ {
        self.ids().filter(|id| !id.is_named())
    }
}

static CONTENT: OnceLock<Content> = OnceLock::new();

/// The folder of the base mod: the one `RUSTCRAFT_BASE_MOD` names, else `data/mods/rustcraft`
/// beside the folder of the game's binary, as in a release, else the one of the repository, for
/// `cargo run` and the tests.
pub fn base_mod_folder() -> PathBuf {
    if let Some(folder) = std::env::var_os(BASE_MOD_VARIABLE) {
        return PathBuf::from(folder);
    }
    let beside = std::env::current_exe()
        .ok()
        .and_then(|exe| Some(exe.parent()?.join("../data/mods").join(BASE_MOD)));
    match beside {
        Some(folder) if folder.is_dir() => folder,
        _ => Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../data/mods")
            .join(BASE_MOD),
    }
}

/// The content of the game: what `load_content` read, else, the first time it is asked for,
/// the base mod's alone. The game cannot run without it: if it does not read, it stops there
/// and says why.
pub fn content() -> &'static Content {
    CONTENT.get_or_init(|| {
        let folder = base_mod_folder();
        Content::read(&folder).unwrap_or_else(|err| {
            panic!(
                "the blocks of the base mod in {} do not read: {err}",
                folder.display()
            )
        })
    })
}

/// Reads the content of the game, as it starts: the base mod's, then that of the mods of
/// `game_folder` that its `mods.ron` leaves on, in the order they load. A mod whose content does
/// not read adds none, and the log says why; a base mod that does not read is an error. Once
/// read, the content stays for the whole run, whatever the Mods screen changes later.
pub fn load_content(game_folder: Option<&Path>) -> Result<&'static Content, String> {
    if let Some(content) = CONTENT.get() {
        return Ok(content);
    }
    let mut content = Content::read(&base_mod_folder())?;
    if let Some(game_folder) = game_folder {
        let mods = find_game_mods(game_folder);
        let settings = read_mods_settings(game_folder).unwrap_or_default();
        let plan = plan_loading(&mods, &settings);
        for found in plan.order.iter().map(|&index| &mods[index]) {
            if found.shipped {
                continue;
            }
            match content.add_mod(&found.id, &found.folder) {
                Ok(()) => (),
                Err(err) => warn!("The content of the mod {} does not read: {err}", found.id),
            }
        }
    }
    info!(
        "{} blocks and {} recipes of mods",
        content.len(),
        content.recipes.len()
    );
    Ok(CONTENT.get_or_init(|| content))
}

/// Why the content of the mod `id` in `folder` would not read, added to `content()`: for the
/// Mods screen, which shows it before the game runs it.
pub fn content_error(id: &str, folder: &Path) -> Option<String> {
    content().clone().add_mod(id, folder).err()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_text() -> String {
        std::fs::read_to_string(base_mod_folder().join(BLOCKS_FILE)).unwrap()
    }

    /// The folder of the example mod `id`.
    fn example(id: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../examples/mods")
            .join(id)
    }

    #[test]
    fn the_base_mod_reads_and_its_blocks_are_where_the_engine_expects_them() {
        let content = Content::read(&base_mod_folder()).unwrap();
        assert_eq!(content.len(), BlockId::ALL.len());
        assert_eq!(content.by_name("oak_log"), Some(BlockId::OakLog));
        assert_eq!(content.by_name("rustcraft:oak_log"), Some(BlockId::OakLog));
        assert_eq!(
            content.by_name("CraftingTable"),
            Some(BlockId::CraftingTable)
        );
        let torch = content.get(BlockId::Torch).unwrap();
        assert_eq!((torch.light, torch.display.as_str()), (14, "Torch"));
        assert_eq!(content.get(BlockId::OakLog).unwrap().display, "Oak Log");
        assert_eq!(content.mod_blocks().count(), 0);
    }

    #[test]
    fn camel_case_joins_the_words() {
        assert_eq!(camel_case("oak_log"), "OakLog");
        assert_eq!(camel_case("dirt"), "Dirt");
        assert_eq!(camel_case("tall_grass"), "TallGrass");
        assert_eq!(title_case("tall_grass"), "Tall Grass");
    }

    #[test]
    fn a_block_out_of_place_is_refused() {
        // stone and grass swapped: every grass of the saved worlds would become stone
        let text = base_text()
            .replacen("id = \"grass\"", "id = \"swap\"", 1)
            .replacen("id = \"stone\"", "id = \"grass\"", 1)
            .replacen("id = \"swap\"", "id = \"stone\"", 1);
        let err = Content::parse(&text).unwrap_err();
        assert!(err.contains("block 2 is \"stone\""), "{err}");
    }

    #[test]
    fn a_misspelt_key_is_refused() {
        let text = base_text().replacen("break_steps = 5", "break_step = 5", 1);
        let err = Content::parse(&text).unwrap_err();
        assert!(err.contains("break_step"), "{err}");
    }

    #[test]
    fn an_unknown_item_is_refused() {
        let text = base_text().replacen("item = \"Sand\"", "item = \"Sandd\"", 1);
        let err = Content::parse(&text).unwrap_err();
        assert!(err.contains("no item is called \"Sandd\""), "{err}");
    }

    #[test]
    fn blocks_after_the_base_ones_take_the_next_numbers() {
        let text = base_text()
            + "\n[[blocks]]\nid = \"lantern\"\nbreak_steps = 3\nlook = \"decoration\"\n\
               light = 15\n";
        let content = Content::parse(&text).unwrap();
        let lantern = content.by_name("rustcraft:lantern").unwrap();
        assert_eq!(lantern.number() as usize, BlockId::ALL.len());
        let def = content.get(lantern).unwrap();
        assert_eq!(
            (def.name, def.light, def.break_time),
            ("rustcraft:lantern", 15, Some(6))
        );
        assert_eq!(def.collision, BlockShape::Full);
    }

    #[test]
    fn a_missing_base_block_is_refused() {
        let text = base_text();
        let cut = text.rfind("[[blocks]]").unwrap();
        let err = Content::parse(&text[..cut]).unwrap_err();
        assert!(err.contains("\"torch\" is missing"), "{err}");
    }

    #[test]
    fn the_beacon_mod_adds_its_lamp_which_drops_itself_and_has_a_recipe() {
        let mut content = Content::read(&base_mod_folder()).unwrap();
        content.add_mod("beacon", &example("beacon")).unwrap();
        let lamp = content.by_name("beacon:lamp").unwrap();
        assert_eq!(lamp.number() as usize, BlockId::ALL.len());
        let def = content.get(lamp).unwrap();
        assert_eq!((def.name, def.display.as_str()), ("beacon:lamp", "Lamp"));
        assert_eq!(def.look, BlockTransparency::Solid);
        assert_eq!(def.collision, BlockShape::Full);
        let torch = content.get(BlockId::Torch).unwrap().light;
        assert!(def.light > torch, "{} against {torch}", def.light);
        assert_eq!(def.drops, [(1, ItemId::Block(lamp), 1)]);

        assert_eq!(content.recipes.len(), 1);
        let recipe = &content.recipes[0];
        assert_eq!(recipe.output, ItemId::Block(lamp));
        let cobble = Some(ItemId::Cobblestone);
        let torch = Some(ItemId::Torch);
        let coal = Some(ItemId::Coal);
        assert_eq!(
            recipe.cells,
            [cobble, torch, cobble, torch, coal, torch, cobble, torch, cobble]
        );
        assert_eq!(content.mods.last().unwrap().id, "beacon");
    }

    #[test]
    fn a_mod_without_content_adds_nothing_and_a_broken_one_nothing_either() {
        let mut content = Content::read(&base_mod_folder()).unwrap();
        let before = content.clone();
        content.add_mod("shapes", &example("shapes")).unwrap();
        assert_eq!(content, before);

        let folder = tempfile::tempdir().unwrap();
        std::fs::write(
            folder.path().join(BLOCKS_FILE),
            "[[blocks]]\nid = \"ok\"\n[[blocks]]\nid = \"bad\"\nlight = 20\n",
        )
        .unwrap();
        let err = content.add_mod("broken", folder.path()).unwrap_err();
        assert!(err.contains("above 15"), "{err}");
        assert_eq!(content, before, "the first block is not added either");
    }

    #[test]
    fn a_recipe_must_be_a_tight_pattern_of_known_items() {
        let mut content = Content::read(&base_mod_folder()).unwrap();
        let recipe = |pattern: &str, key: &str| {
            format!("[[recipes]]\npattern = {pattern}\nkey = {key}\noutput = \"torch\"\n")
        };
        let ok = recipe("[\"C\", \"S\"]", "{ C = \"coal\", S = \"stick\" }");
        content.add_recipes("x", &ok).unwrap();
        let errors = [
            recipe("[\"C \", \"S \"]", "{ C = \"coal\", S = \"stick\" }"),
            recipe("[\"C\", \"X\"]", "{ C = \"coal\" }"),
            recipe("[\"C\"]", "{ C = \"coals\" }"),
            recipe("[\"CCCC\"]", "{ C = \"coal\" }"),
            recipe("[\"CC\", \"C\"]", "{ C = \"coal\" }"),
        ];
        for text in errors {
            assert!(content.add_recipes("x", &text).is_err(), "{text}");
        }
    }
}
