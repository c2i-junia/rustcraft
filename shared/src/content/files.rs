//! The override files of the content: each `*.ron` of a folder such as `data/content/rustcraft/`
//! may change some properties of existing blocks and items, named as the commands name them
//! (`oak_log`). A field is written as the property it replaces is: `light_emission: 10`,
//! `break_time: Some(20)`, `drops: [(1, Cobblestone, 1)]`.
//!
//! ```ron
//! (
//!     blocks: { "torch": (light_emission: 10), "stone": (break_time: Some(20)) },
//!     items: { "apple": (max_stack: 16) },
//! )
//! ```

use std::collections::HashMap;
use std::fmt;
use std::marker::PhantomData;
use std::path::{Path, PathBuf};

use bevy::math::bounding::Aabb3d;
use bevy::math::Vec3A;
use serde::de::{self, Deserializer, MapAccess, Visitor};
use serde::Deserialize;

use super::{tool_bit, BlockProps, Content, ItemProps};
use crate::players::effects::EffectKind;
use crate::world::{
    BlockHitbox, BlockId, BlockTransparency, ItemCategory, ItemId, RequiredTool, ToolKind,
};

/// The highest level of light, which a block may give or take.
const MAX_LIGHT: u8 = 15;

/// The far corner of the cell, for a box: a fence or a wall is 1.5 high, as in Minecraft.
const BOX_TOP: Vec3A = Vec3A::new(1.0, 1.5, 1.0);

/// Why a set of override files cannot be installed: the file, then the block or item and what
/// is wrong with it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentError {
    /// The file at fault.
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
    /// The built-in content with the override files of `folder` applied, in the order of their
    /// names. A missing folder overrides nothing. Nothing is installed: the caller decides.
    pub fn load(folder: &Path) -> Result<Content, ContentError> {
        let mut content = Content::builtin();
        let mut files = match std::fs::read_dir(folder) {
            Ok(entries) => entries
                .filter_map(|entry| Some(entry.ok()?.path()))
                .filter(|path| path.extension().is_some_and(|ext| ext == "ron"))
                .collect::<Vec<_>>(),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Vec::new(),
            Err(err) => {
                return Err(ContentError {
                    file: folder.to_path_buf(),
                    reason: err.to_string(),
                })
            }
        };
        files.sort();

        let blocks = names(BlockId::ALL.into_iter().map(|id| format!("{id:?}")));
        let items = names(ItemId::ALL.into_iter().map(|id| format!("{id:?}")));
        let mut block_files: HashMap<usize, &Path> = HashMap::new();
        let mut item_files: HashMap<usize, &Path> = HashMap::new();
        for file in &files {
            let fail = |reason: String| ContentError {
                file: file.clone(),
                reason,
            };
            let text = std::fs::read_to_string(file).map_err(|err| fail(err.to_string()))?;
            let overrides: OverrideFile =
                ron::de::from_str(&text).map_err(|err| fail(err.to_string()))?;
            for (name, block) in overrides.blocks.0 {
                let index = *blocks
                    .get(&name)
                    .ok_or_else(|| fail(format!("no block is named \"{name}\"")))?;
                if let Some(other) = block_files.insert(index, file) {
                    return Err(fail(format!(
                        "block \"{name}\" is also overridden in {}",
                        other.display()
                    )));
                }
                let props = &mut content.blocks[index];
                block.apply(props);
                check_block(props).map_err(|reason| fail(format!("block \"{name}\": {reason}")))?;
            }
            for (name, item) in overrides.items.0 {
                let index = *items
                    .get(&name)
                    .ok_or_else(|| fail(format!("no item is named \"{name}\"")))?;
                if let Some(other) = item_files.insert(index, file) {
                    return Err(fail(format!(
                        "item \"{name}\" is also overridden in {}",
                        other.display()
                    )));
                }
                let props = &mut content.items[index];
                item.apply(props);
                check_item(props).map_err(|reason| fail(format!("item \"{name}\": {reason}")))?;
            }
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

/// The index of each name in the order of `pascal_names`, by its snake case: `OakLog` is
/// `oak_log`.
fn names(pascal_names: impl Iterator<Item = String>) -> HashMap<String, usize> {
    pascal_names
        .enumerate()
        .map(|(index, pascal)| {
            let mut snake = String::new();
            for c in pascal.chars() {
                if c.is_uppercase() && !snake.is_empty() {
                    snake.push('_');
                }
                snake.extend(c.to_lowercase());
            }
            (snake, index)
        })
        .collect()
}

/// What the game cannot run with in the properties of a block.
fn check_block(props: &BlockProps) -> Result<(), String> {
    if props.light_emission > MAX_LIGHT {
        return Err(format!(
            "light_emission {} is above {MAX_LIGHT}",
            props.light_emission
        ));
    }
    if props
        .light_opacity
        .is_some_and(|opacity| opacity > MAX_LIGHT)
    {
        return Err(format!("light_opacity is above {MAX_LIGHT}"));
    }
    if props.break_time == Some(0) {
        // the breaking stage divides by it
        return Err("break_time Some(0): an instant break is Some(1)".to_string());
    }
    for (field, hitbox) in [("hitbox", props.hitbox), ("ray_hitbox", props.ray_hitbox)] {
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

/// What the game cannot run with in the properties of an item.
fn check_item(props: &ItemProps) -> Result<(), String> {
    if props.max_stack == 0 {
        return Err("max_stack 0: a stack holds at least 1".to_string());
    }
    Ok(())
}

/// One override file.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OverrideFile {
    #[serde(default)]
    blocks: Named<BlockOverride>,
    #[serde(default)]
    items: Named<ItemOverride>,
}

/// The fields of the properties of a block that a file may override: the pure data, not what
/// the block is (`palette`), nor what the engine does with it (`usable`, `container`...).
#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct BlockOverride {
    #[serde(default, deserialize_with = "present")]
    visibility: Option<BlockTransparency>,
    #[serde(default, deserialize_with = "present")]
    light_opacity: Option<Option<u8>>,
    #[serde(default, deserialize_with = "present")]
    light_emission: Option<u8>,
    #[serde(default, deserialize_with = "present")]
    break_time: Option<Option<u8>>,
    /// The tools that mine the block faster: a list of `ToolKind`.
    #[serde(default, deserialize_with = "present")]
    faster_tools: Option<Vec<ToolKind>>,
    #[serde(default, deserialize_with = "present")]
    required_tool: Option<Option<RequiredTool>>,
    #[serde(default, deserialize_with = "present")]
    drops: Option<Vec<(u32, ItemId, u32)>>,
    #[serde(default, deserialize_with = "present")]
    hitbox: Option<BlockHitbox>,
    #[serde(default, deserialize_with = "present")]
    ray_hitbox: Option<BlockHitbox>,
    #[serde(default, deserialize_with = "present")]
    replaceable: Option<bool>,
}

impl BlockOverride {
    fn apply(self, props: &mut BlockProps) {
        let Self {
            visibility,
            light_opacity,
            light_emission,
            break_time,
            faster_tools,
            required_tool,
            drops,
            hitbox,
            ray_hitbox,
            replaceable,
        } = self;
        set(&mut props.visibility, visibility);
        set(&mut props.light_opacity, light_opacity);
        set(&mut props.light_emission, light_emission);
        set(&mut props.break_time, break_time);
        set(
            &mut props.faster_tools,
            faster_tools.map(|kinds| {
                kinds
                    .into_iter()
                    .fold(0, |mask, kind| mask | tool_bit(kind))
            }),
        );
        set(&mut props.required_tool, required_tool);
        set(&mut props.drops, drops);
        set(&mut props.hitbox, hitbox);
        set(&mut props.ray_hitbox, ray_hitbox);
        set(&mut props.replaceable, replaceable);
    }
}

/// The fields of the properties of an item that a file may override: the pure data, not what
/// the item is (`default_type`, `tool`) nor what it turns into.
#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct ItemOverride {
    #[serde(default, deserialize_with = "present")]
    max_stack: Option<u32>,
    #[serde(default, deserialize_with = "present")]
    attack_damage: Option<u32>,
    #[serde(default, deserialize_with = "present")]
    food_value: Option<Option<(u8, f32)>>,
    #[serde(default, deserialize_with = "present")]
    food_effect: Option<Option<(EffectKind, u8, u32)>>,
    #[serde(default, deserialize_with = "present")]
    horse_armour: Option<Option<u32>>,
    #[serde(default, deserialize_with = "present")]
    durability: Option<Option<u32>>,
    #[serde(default, deserialize_with = "present")]
    category: Option<ItemCategory>,
}

impl ItemOverride {
    fn apply(self, props: &mut ItemProps) {
        let Self {
            max_stack,
            attack_damage,
            food_value,
            food_effect,
            horse_armour,
            durability,
            category,
        } = self;
        set(&mut props.max_stack, max_stack);
        set(&mut props.attack_damage, attack_damage);
        set(&mut props.food_value, food_value);
        set(&mut props.food_effect, food_effect);
        set(&mut props.horse_armour, horse_armour);
        set(&mut props.durability, durability);
        set(&mut props.category, category);
    }
}

/// Replaces `property` with the value of the file, if it gives one.
fn set<T>(property: &mut T, value: Option<T>) {
    if let Some(value) = value {
        *property = value;
    }
}

/// A field the file writes: `light_emission: 10` is `Some(10)`, a field it leaves out `None`.
fn present<'de, D: Deserializer<'de>, T: Deserialize<'de>>(
    deserializer: D,
) -> Result<Option<T>, D::Error> {
    T::deserialize(deserializer).map(Some)
}

/// What a kind of override is called in the errors.
trait Kind {
    const KIND: &'static str;
}

impl Kind for BlockOverride {
    const KIND: &'static str = "block";
}

impl Kind for ItemOverride {
    const KIND: &'static str = "item";
}

/// The overrides of a file by name, in the order of the file. An error in one names it, and so
/// does a name written twice.
struct Named<T>(Vec<(String, T)>);

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
                write!(f, "a map of {} names to their overrides", T::KIND)
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
mod tests {
    use super::*;

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
    fn no_folder_and_an_empty_folder_are_the_builtin_content() {
        let builtin = Content::builtin();
        assert_eq!(
            Content::load(Path::new("/no/such/folder")),
            Ok(builtin.clone())
        );
        assert_eq!(load(&[]), Ok(builtin.clone()));
        assert_eq!(load(&[("empty.ron", "()")]), Ok(builtin));
    }

    #[test]
    fn an_override_file_changes_the_properties_it_names_and_only_those() {
        let content = load(&[(
            "tuning.ron",
            r#"(
                blocks: {
                    "torch": (light_emission: 9),
                    "stone": (break_time: Some(20), faster_tools: [Pickaxe, Shovel],
                              drops: [(1, Gravel, 2)]),
                },
                items: { "apple": (max_stack: 16, food_value: Some((2, 1.5))) },
            )"#,
        )])
        .unwrap();
        let builtin = Content::builtin();

        let torch = content.block(BlockId::Torch);
        assert_eq!(torch.light_emission, 9);
        assert_eq!(
            BlockProps {
                light_emission: builtin.block(BlockId::Torch).light_emission,
                ..torch.clone()
            },
            *builtin.block(BlockId::Torch)
        );
        let stone = content.block(BlockId::Stone);
        assert_eq!(stone.break_time, Some(20));
        assert_eq!(
            stone.faster_tools,
            tool_bit(ToolKind::Pickaxe) | tool_bit(ToolKind::Shovel)
        );
        assert_eq!(stone.drops, vec![(1, ItemId::Gravel, 2)]);
        let apple = content.item(ItemId::Apple);
        assert_eq!(apple.max_stack, 16);
        assert_eq!(apple.food_value, Some((2, 1.5)));

        let untouched = |id: BlockId| content.block(id) == builtin.block(id);
        assert!(BlockId::ALL
            .into_iter()
            .filter(|id| ![BlockId::Torch, BlockId::Stone].contains(id))
            .all(untouched));
        assert_eq!(content.items.len(), builtin.items.len());
        assert!(!content.same_properties(&builtin));
        assert!(!content.draws_like(&builtin), "the torch gives less light");
    }

    #[test]
    fn a_property_only_change_draws_like_before() {
        let content = load(&[("hard.ron", r#"(blocks: {"dirt": (break_time: Some(50))})"#)]);
        assert!(content.unwrap().draws_like(&Content::builtin()));
    }

    /// The error of a bad file names the file and the block or item, and nothing is loaded.
    #[test]
    fn a_bad_file_is_refused_with_its_name_and_its_key() {
        let cases = [
            (
                r#"(blocks: {"torch": (light_emission: 20)})"#,
                "block \"torch\"",
            ),
            (r#"(blocks: {"torch": (light: 9)})"#, "block \"torch\""),
            (r#"(blocks: {"tourch": (light_emission: 9)})"#, "\"tourch\""),
            (
                r#"(blocks: {"stone": (break_time: Some(0))})"#,
                "block \"stone\"",
            ),
            (r#"(items: {"apple": (max_stack: 0)})"#, "item \"apple\""),
            (
                r#"(items: {"apple": (max_stack: "many")})"#,
                "item \"apple\"",
            ),
            (
                r#"(blocks: {"glass": (hitbox: Aabb((min: (0, 0, 0), max: (1, 2.5, 1))))})"#,
                "block \"glass\"",
            ),
            (
                r#"(blocks: {"torch": (light_emission: 1), "torch": (light_emission: 2)})"#,
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
        let error =
            load(&[("broken.ron", "(blocks: {\"torch\": (light_emission: 9}")]).unwrap_err();
        assert_eq!(error.file.file_name().unwrap(), "broken.ron");
    }

    #[test]
    fn a_block_overridden_in_two_files_names_both() {
        let error = load(&[
            ("a.ron", r#"(blocks: {"torch": (light_emission: 9)})"#),
            ("b.ron", r#"(blocks: {"torch": (light_emission: 8)})"#),
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
    fn the_builtin_content_passes_the_checks() {
        let builtin = Content::builtin();
        for (id, props) in BlockId::ALL.into_iter().zip(&builtin.blocks) {
            assert_eq!(check_block(props), Ok(()), "{id:?}");
        }
        for (id, props) in ItemId::ALL.into_iter().zip(&builtin.items) {
            assert_eq!(check_item(props), Ok(()), "{id:?}");
        }
    }

    #[test]
    fn the_override_files_of_the_game_load() {
        let folder = Path::new(env!("CARGO_MANIFEST_DIR")).join("../data/content/rustcraft");
        assert!(folder.is_dir());
        if let Err(err) = Content::load(&folder) {
            panic!("{err}");
        }
    }

    #[test]
    fn the_example_of_the_override_file_loads() {
        let example = std::fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../data/content/rustcraft/overrides.ron"),
        )
        .unwrap();
        // the commented out example, without its comment marks
        let text: String = example
            .lines()
            .filter_map(|line| line.strip_prefix("// "))
            .skip_while(|line| *line != "(")
            .collect::<Vec<_>>()
            .join("\n");
        let content = load(&[("example.ron", &text)]).unwrap();
        assert_eq!(content.block(BlockId::Torch).light_emission, 10);
    }

    #[test]
    fn the_content_goes_over_the_wire_unchanged() {
        let content = Content::builtin();
        let payload = crate::game_message_to_payload(&content);
        assert_eq!(
            crate::payload_to_game_message::<Content>(&payload).unwrap(),
            content
        );
    }
}
