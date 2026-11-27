//! The override files of the content: each `*.ron` of a folder such as `data/content/rustcraft/`
//! may change some properties of existing blocks and items, named as the commands name them
//! (`oak_log`), in the vocabulary of `docs/design/content-registry.md` (2.1-2.2): `light`,
//! `break`, `tools`, `needs_tool`, `drops`, `look`, `collision`, `outline`, `replaceable`, and
//! `stack` for an item. A drop names its item the same way: `(chance, "item", count)`.
//!
//! ```ron
//! (
//!     blocks: { "torch": (light: 10), "stone": (break: 20, drops: [(1, "gravel", 1)]) },
//!     items: { "apple": (stack: 16) },
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
use crate::utils::snake_case;
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

        let blocks: HashMap<String, BlockId> = BlockId::ALL
            .into_iter()
            .map(|id| (snake_case(&format!("{id:?}")), id))
            .collect();
        let items: HashMap<String, ItemId> =
            ItemId::ALL.into_iter().map(|id| (id.name(), id)).collect();
        let mut block_files: HashMap<BlockId, &Path> = HashMap::new();
        let mut item_files: HashMap<ItemId, &Path> = HashMap::new();
        for file in &files {
            let fail = |reason: String| ContentError {
                file: file.clone(),
                reason,
            };
            let text = std::fs::read_to_string(file).map_err(|err| fail(err.to_string()))?;
            let overrides: OverrideFile =
                ron::de::from_str(&text).map_err(|err| fail(err.to_string()))?;
            for (name, block) in overrides.blocks.0 {
                let id = *blocks
                    .get(&name)
                    .ok_or_else(|| fail(format!("no block is named \"{name}\"")))?;
                if let Some(other) = block_files.insert(id, file) {
                    return Err(fail(format!(
                        "block \"{name}\" is also overridden in {}",
                        other.display()
                    )));
                }
                let props = &mut content.blocks[id as usize];
                block
                    .apply(props, &items)
                    .and_then(|()| check_block(props))
                    .map_err(|reason| fail(format!("block \"{name}\": {reason}")))?;
            }
            for (name, item) in overrides.items.0 {
                let id = *items
                    .get(&name)
                    .ok_or_else(|| fail(format!("no item is named \"{name}\"")))?;
                if let Some(other) = item_files.insert(id, file) {
                    return Err(fail(format!(
                        "item \"{name}\" is also overridden in {}",
                        other.display()
                    )));
                }
                let props = &mut content.items[id as usize];
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
/// the block is (`palette`), nor what the engine does with it (`usable`, `container`...). A
/// property that may be absent in the code (`break`, `needs_tool`, `opacity`) is given by its
/// value: a file cannot take it away.
#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct BlockOverride {
    /// `visibility`: `Solid`, `Transparent`, `Liquid` or `Decoration`.
    #[serde(default, deserialize_with = "present")]
    look: Option<BlockTransparency>,
    /// `light_emission`.
    #[serde(default, deserialize_with = "present")]
    light: Option<u8>,
    /// `light_opacity`.
    #[serde(default, deserialize_with = "present")]
    opacity: Option<u8>,
    /// `break_time`: the hits a hand breaks the block in.
    #[serde(default, rename = "break", deserialize_with = "present")]
    break_hits: Option<u8>,
    /// `faster_tools`: the tools that mine the block faster.
    #[serde(default, deserialize_with = "present")]
    tools: Option<Vec<ToolKind>>,
    /// `required_tool`: `(kind: Pickaxe, level: 1)`.
    #[serde(default, deserialize_with = "present")]
    needs_tool: Option<RequiredTool>,
    /// `drops`: `(chance, "item", count)`, the item named as the commands name it.
    #[serde(default, deserialize_with = "present")]
    drops: Option<Vec<(u32, String, u32)>>,
    /// `hitbox`.
    #[serde(default, deserialize_with = "present")]
    collision: Option<BlockHitbox>,
    /// `ray_hitbox`.
    #[serde(default, deserialize_with = "present")]
    outline: Option<BlockHitbox>,
    #[serde(default, deserialize_with = "present")]
    replaceable: Option<bool>,
}

impl BlockOverride {
    /// Writes the fields into `props`, the items of the drops found in `items` by name.
    fn apply(self, props: &mut BlockProps, items: &HashMap<String, ItemId>) -> Result<(), String> {
        let Self {
            look,
            light,
            opacity,
            break_hits,
            tools,
            needs_tool,
            drops,
            collision,
            outline,
            replaceable,
        } = self;
        let drops = drops
            .map(|drops| {
                drops
                    .into_iter()
                    .map(|(chance, item, count)| match items.get(&item) {
                        Some(&item) => Ok((chance, item, count)),
                        None => Err(format!("drops: no item is named \"{item}\"")),
                    })
                    .collect::<Result<Vec<_>, _>>()
            })
            .transpose()?;
        set(&mut props.visibility, look);
        set(&mut props.light_emission, light);
        set(&mut props.light_opacity, opacity.map(Some));
        set(&mut props.break_time, break_hits.map(Some));
        set(
            &mut props.faster_tools,
            tools.map(|kinds| {
                kinds
                    .into_iter()
                    .fold(0, |mask, kind| mask | tool_bit(kind))
            }),
        );
        set(&mut props.required_tool, needs_tool.map(Some));
        set(&mut props.drops, drops);
        set(&mut props.hitbox, collision);
        set(&mut props.ray_hitbox, outline);
        set(&mut props.replaceable, replaceable);
        Ok(())
    }
}

/// The fields of the properties of an item that a file may override: the pure data, not what
/// the item is (`default_type`, `tool`) nor what it turns into. As for a block, a property that
/// may be absent in the code is given by its value.
#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct ItemOverride {
    /// `max_stack`.
    #[serde(default, deserialize_with = "present")]
    stack: Option<u32>,
    #[serde(default, deserialize_with = "present")]
    attack_damage: Option<u32>,
    /// `(hunger, saturation)`.
    #[serde(default, deserialize_with = "present")]
    food_value: Option<(u8, f32)>,
    /// `(effect, level, ticks)`.
    #[serde(default, deserialize_with = "present")]
    food_effect: Option<(EffectKind, u8, u32)>,
    #[serde(default, deserialize_with = "present")]
    horse_armour: Option<u32>,
    #[serde(default, deserialize_with = "present")]
    durability: Option<u32>,
    #[serde(default, deserialize_with = "present")]
    category: Option<ItemCategory>,
}

impl ItemOverride {
    fn apply(self, props: &mut ItemProps) {
        let Self {
            stack,
            attack_damage,
            food_value,
            food_effect,
            horse_armour,
            durability,
            category,
        } = self;
        set(&mut props.max_stack, stack);
        set(&mut props.attack_damage, attack_damage);
        set(&mut props.food_value, food_value.map(Some));
        set(&mut props.food_effect, food_effect.map(Some));
        set(&mut props.horse_armour, horse_armour.map(Some));
        set(&mut props.durability, durability.map(Some));
        set(&mut props.category, category);
    }
}

/// Replaces `property` with the value of the file, if it gives one.
fn set<T>(property: &mut T, value: Option<T>) {
    if let Some(value) = value {
        *property = value;
    }
}

/// A field the file writes: `light: 10` is `Some(10)`, a field it leaves out `None`.
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
                    "torch": (light: 9),
                    "stone": (break: 20, tools: [Pickaxe, Shovel], drops: [(1, "gravel", 2)],
                              needs_tool: (kind: Shovel, level: 2), look: Decoration),
                },
                items: { "apple": (stack: 16, food_value: (2, 1.5)) },
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
        assert_eq!(
            stone.required_tool,
            Some(RequiredTool {
                kind: ToolKind::Shovel,
                level: 2
            })
        );
        assert_eq!(stone.visibility, BlockTransparency::Decoration);
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
        let content = load(&[("hard.ron", r#"(blocks: {"dirt": (break: 50)})"#)]);
        assert!(content.unwrap().draws_like(&Content::builtin()));
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
            (r#"(items: {"apple": (stack: 0)})"#, "item \"apple\""),
            (r#"(items: {"apple": (stack: "many")})"#, "item \"apple\""),
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
    fn a_block_overridden_in_two_files_names_both() {
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
