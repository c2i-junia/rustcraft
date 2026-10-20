//! The blocks of the game, as the base mod declares them in its `blocks.toml`. The engine
//! knows no block of its own: it reads them all from there, once, the first time a block is
//! asked about, and every property of a `BlockId` is a read in this table.
//!
//! Still in the engine, for now: the names of the base blocks (`BlockId::Stone`), which the
//! terrain and the mechanics use, and which reading the file checks; their items; how each is
//! drawn and sounds. See "Base mod" in docs/ARCHITECTURE.md.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use bevy::math::{bounding::Aabb3d, Vec3A};
use serde::Deserialize;

use super::{BlockId, BlockTransparency, ItemId, ToolKind};
use crate::mods::{parse_manifest, MANIFEST_FILE};

/// The id of the base mod, and the name of its folder in `mods/` of the data folder.
pub const BASE_MOD: &str = "rustcraft";
/// The blocks of a mod, beside its `mod.toml`.
pub const BLOCKS_FILE: &str = "blocks.toml";
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
    /// In camel case, as debug messages and the saves in RON write it: `OakLog`.
    pub name: &'static str,
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

/// The blocks, by number.
#[derive(Debug, Clone, PartialEq)]
pub struct BlockRegistry {
    blocks: Vec<BlockDef>,
    by_name: HashMap<String, BlockId>,
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
    item: ItemId,
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

impl BlockRegistry {
    /// The blocks of `blocks.toml`, numbered in their order. Each base block the engine names
    /// must be at its number, and the drop counts and chances above 0.
    pub fn parse(text: &str) -> Result<Self, String> {
        let file: BlocksFile = toml::from_str(text).map_err(|err| err.to_string())?;
        if file.blocks.len() > u16::MAX as usize {
            return Err(format!(
                "{} blocks, more than {}",
                file.blocks.len(),
                u16::MAX
            ));
        }
        let mut registry = BlockRegistry {
            blocks: Vec::with_capacity(file.blocks.len()),
            by_name: HashMap::new(),
        };
        for (number, entry) in file.blocks.into_iter().enumerate() {
            let id = BlockId::from_number(number as u16);
            if entry.id.is_empty()
                || !entry
                    .id
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
            {
                return Err(format!(
                    "block {number}: \"{}\" is not an id in snake case",
                    entry.id
                ));
            }
            let name = camel_case(&entry.id);
            if registry.by_name.contains_key(&entry.id) {
                return Err(format!("block {number}: \"{}\" is there twice", entry.id));
            }
            if let Some(drop) = entry.drops.iter().find(|d| d.count == 0 || d.chance == 0) {
                return Err(format!(
                    "{}: the drop of {:?} has a count or a chance of 0",
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
            registry.by_name.insert(entry.id.clone(), id);
            registry.by_name.insert(name.clone(), id);
            registry.blocks.push(BlockDef {
                // a few dozen, read once: what Debug and serde need, for the whole run
                name: Box::leak(name.into_boxed_str()),
                id: entry.id,
                break_time: entry.break_steps.map(|steps| steps.saturating_mul(2)),
                tools: entry.tools,
                needs_tool: entry.needs_tool,
                drops: entry
                    .drops
                    .iter()
                    .map(|d| (d.chance, d.item, d.count))
                    .collect(),
                look: entry.look,
                collision: entry.collision.shape(),
                outline: entry.outline.shape(),
                light: entry.light,
                light_opacity: entry.light_opacity,
                usable: entry.usable,
                replaceable: entry.replaceable,
            });
        }
        for (expected, id) in BlockId::BASE_NAMES.iter().zip(BlockId::ALL) {
            match registry.blocks.get(id.number() as usize) {
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
        Ok(registry)
    }

    /// The blocks of the mod in `folder`, from its `blocks.toml`, once its `mod.toml` reads.
    pub fn read(folder: &Path) -> Result<Self, String> {
        let manifest = folder.join(MANIFEST_FILE);
        let text = std::fs::read_to_string(&manifest)
            .map_err(|err| format!("{}: {err}", manifest.display()))?;
        parse_manifest(&text).map_err(|err| format!("{}: {err}", manifest.display()))?;
        let blocks = folder.join(BLOCKS_FILE);
        let text = std::fs::read_to_string(&blocks)
            .map_err(|err| format!("{}: {err}", blocks.display()))?;
        Self::parse(&text).map_err(|err| format!("{}: {err}", blocks.display()))
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

    /// The block named so, in snake case (`oak_log`) or in camel case (`OakLog`).
    pub fn by_name(&self, name: &str) -> Option<BlockId> {
        self.by_name.get(name).copied()
    }

    /// Every block, in the order of their numbers.
    pub fn ids(&self) -> impl Iterator<Item = BlockId> + '_ {
        (0..self.blocks.len()).map(|number| BlockId::from_number(number as u16))
    }
}

static BLOCKS: OnceLock<BlockRegistry> = OnceLock::new();

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

/// The blocks of the game, read from the base mod the first time they are asked for. The game
/// cannot run without them: if they do not read, it stops there and says why.
pub fn blocks() -> &'static BlockRegistry {
    BLOCKS.get_or_init(|| {
        let folder = base_mod_folder();
        BlockRegistry::read(&folder).unwrap_or_else(|err| {
            panic!(
                "the blocks of the base mod in {} do not read: {err}",
                folder.display()
            )
        })
    })
}

/// Reads the blocks now, so that a broken base mod stops the game as it starts rather than at
/// the first block.
pub fn load_blocks() -> Result<&'static BlockRegistry, String> {
    if let Some(registry) = BLOCKS.get() {
        return Ok(registry);
    }
    let registry = BlockRegistry::read(&base_mod_folder())?;
    Ok(BLOCKS.get_or_init(|| registry))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_text() -> String {
        std::fs::read_to_string(base_mod_folder().join(BLOCKS_FILE)).unwrap()
    }

    #[test]
    fn the_base_mod_reads_and_its_blocks_are_where_the_engine_expects_them() {
        let registry = BlockRegistry::read(&base_mod_folder()).unwrap();
        assert_eq!(registry.len(), BlockId::ALL.len());
        assert_eq!(registry.by_name("oak_log"), Some(BlockId::OakLog));
        assert_eq!(
            registry.by_name("CraftingTable"),
            Some(BlockId::CraftingTable)
        );
        assert_eq!(registry.get(BlockId::Torch).unwrap().light, 14);
    }

    #[test]
    fn camel_case_joins_the_words() {
        assert_eq!(camel_case("oak_log"), "OakLog");
        assert_eq!(camel_case("dirt"), "Dirt");
        assert_eq!(camel_case("tall_grass"), "TallGrass");
    }

    #[test]
    fn a_block_out_of_place_is_refused() {
        // stone and grass swapped: every grass of the saved worlds would become stone
        let text = base_text()
            .replacen("id = \"grass\"", "id = \"swap\"", 1)
            .replacen("id = \"stone\"", "id = \"grass\"", 1)
            .replacen("id = \"swap\"", "id = \"stone\"", 1);
        let err = BlockRegistry::parse(&text).unwrap_err();
        assert!(err.contains("block 2 is \"stone\""), "{err}");
    }

    #[test]
    fn a_misspelt_key_is_refused() {
        let text = base_text().replacen("break_steps = 5", "break_step = 5", 1);
        let err = BlockRegistry::parse(&text).unwrap_err();
        assert!(err.contains("break_step"), "{err}");
    }

    #[test]
    fn an_unknown_item_is_refused() {
        let text = base_text().replacen("item = \"Sand\"", "item = \"Sandd\"", 1);
        assert!(BlockRegistry::parse(&text).is_err());
    }

    #[test]
    fn blocks_after_the_base_ones_take_the_next_numbers() {
        let text = base_text()
            + "\n[[blocks]]\nid = \"lantern\"\nbreak_steps = 3\nlook = \"decoration\"\n\
               light = 15\n";
        let registry = BlockRegistry::parse(&text).unwrap();
        let lantern = registry.by_name("lantern").unwrap();
        assert_eq!(lantern.number() as usize, BlockId::ALL.len());
        let def = registry.get(lantern).unwrap();
        assert_eq!(
            (def.name, def.light, def.break_time),
            ("Lantern", 15, Some(6))
        );
        assert_eq!(def.collision, BlockShape::Full);
    }

    #[test]
    fn a_missing_base_block_is_refused() {
        let text = base_text();
        let cut = text.rfind("[[blocks]]").unwrap();
        let err = BlockRegistry::parse(&text[..cut]).unwrap_err();
        assert!(err.contains("\"torch\" is missing"), "{err}");
    }
}
