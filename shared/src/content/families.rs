//! The families of a content file (`docs/design/content-registry.md`, 2.3): definitions written
//! once for each member of a list, such as the slabs and stairs of each wood.
//!
//! ```ron
//! families: [
//!     (
//!         each: {"w": ["oak", "spruce"]},
//!         blocks: {"{w}_slab": (like: "{w}_planks", palette: (form: Slab, base: "{w}_planks",
//!                  item: "{w}_slab"), faces: (all: "{W}Planks"))},
//!         recipes: [(out: "{w}_slab", count: 6, pattern: ["PPP"], key: {'P': "{w}_planks"})],
//!         fuel: {"{w}_slab": 150},
//!     ),
//! ]
//! ```
//!
//! Each member is a word of each list of `each`, the lists read side by side, so that `each:
//! {"r": ["stone", "brick"], "m": ["stone", "brick_wall"]}` has two members. In every name and
//! every text of the definitions, `{w}` stands for the word of the member and `{W}` for its Rust
//! name (`OakPlanks` for `oak_planks`). The substitution is made on the definitions as read,
//! before they are checked, which is the text of the file read for each member; `ron` 0.6 reads
//! a value without the names of its variants, so it cannot be made on a `ron::Value`.
//!
//! A family adds its blocks, items, tags, smelting and fuel to its file, and its recipes after
//! those of the file, member after member. What a member does not have, the family's `except`
//! names: no block, item, recipe, smelting or fuel of that name is made. A block or an item a
//! file writes out under the name a family gives it, or under one of its `aliases`, is the
//! file's: the family's is dropped, as for a member whose block is not like the others (the oak
//! door is `door`), while its recipes stay.

use std::collections::{BTreeMap, HashSet};
use std::path::PathBuf;

use serde::Deserialize;

use super::files::{BlockDef, ContentError, ContentFile, ItemDef, Named};
use super::recipes::{FuelDef, RecipeDef, TagDef};
use crate::utils::pascal_case;

/// A family in a file: its members, then the definitions written for each.
#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub(super) struct FamilyDef {
    /// The words of the members, by the name each stands for between braces.
    pub each: BTreeMap<String, Vec<String>>,
    /// The names, once a member's words are in them, that the family does not make.
    #[serde(default)]
    pub except: Vec<String>,
    #[serde(default)]
    pub blocks: Named<BlockDef>,
    #[serde(default)]
    pub items: Named<ItemDef>,
    /// Each member adds the items it names to the tag, in the order of the members.
    #[serde(default)]
    pub tags: Named<TagDef>,
    #[serde(default)]
    pub recipes: Vec<RecipeDef>,
    #[serde(default)]
    pub smelting: Vec<(String, String)>,
    #[serde(default)]
    pub fuel: Named<FuelDef>,
}

/// What can have its texts written for a member: each text a definition holds, the names it
/// refers to and those it shows.
pub(super) trait Texts {
    fn texts(&mut self, each: &mut dyn FnMut(&mut String));
}

impl Texts for String {
    fn texts(&mut self, each: &mut dyn FnMut(&mut String)) {
        each(self);
    }
}

impl<T: Texts> Texts for Option<T> {
    fn texts(&mut self, each: &mut dyn FnMut(&mut String)) {
        if let Some(value) = self {
            value.texts(each);
        }
    }
}

impl<T: Texts> Texts for Vec<T> {
    fn texts(&mut self, each: &mut dyn FnMut(&mut String)) {
        for value in self {
            value.texts(each);
        }
    }
}

/// The words of one member, by the name each stands for.
struct Member<'a> {
    words: Vec<(&'a str, &'a str)>,
}

impl Member<'_> {
    /// `text` with the words of the member in it: `{w}` for the word of `w`, `{W}` for its
    /// Rust name. A name between braces the family does not know is an error.
    fn write(&self, text: &str) -> Result<String, String> {
        let mut out = text.to_string();
        for (name, word) in &self.words {
            out = out.replace(&format!("{{{name}}}"), word);
            out = out.replace(&format!("{{{}}}", pascal_case(name)), &pascal_case(word));
        }
        match (out.find('{'), out.find('}')) {
            (None, None) => Ok(out),
            _ => Err(format!(
                "\"{text}\": the family has no word in braces in \"{out}\""
            )),
        }
    }

    /// `def` with the words of the member in each of its texts.
    fn write_all<T: Texts + Clone>(&self, def: &T) -> Result<T, String> {
        let mut def = def.clone();
        let mut error = None;
        def.texts(&mut |text| match self.write(text) {
            Ok(written) => *text = written,
            Err(err) => {
                error.get_or_insert(err);
            }
        });
        error.map_or(Ok(def), Err)
    }
}

/// The blocks and the items the files write out, by their names and their aliases: a family
/// does not make those.
struct Written {
    blocks: HashSet<String>,
    items: HashSet<String>,
}

impl Written {
    fn of(files: &[(PathBuf, ContentFile)]) -> Self {
        let mut written = Self {
            blocks: HashSet::new(),
            items: HashSet::new(),
        };
        for (_, file) in files {
            for (name, def) in &file.blocks.0 {
                written.blocks.insert(name.clone());
                // the alias of a block also names its item
                for alias in def.aliases.iter().flatten() {
                    written.blocks.insert(alias.clone());
                    written.items.insert(alias.clone());
                }
            }
            for (name, def) in &file.items.0 {
                written.items.insert(name.clone());
                written.items.extend(def.aliases.iter().flatten().cloned());
            }
        }
        written
    }
}

/// Writes out the families of every file into the definitions of their files, in place.
pub(super) fn expand(files: &mut [(PathBuf, ContentFile)]) -> Result<(), ContentError> {
    let written = Written::of(files);
    for (path, file) in files.iter_mut() {
        for (number, family) in std::mem::take(&mut file.families).into_iter().enumerate() {
            expand_family(family, file, &written).map_err(|reason| ContentError {
                file: path.clone(),
                reason: format!("family {}: {reason}", number + 1),
            })?;
        }
    }
    Ok(())
}

/// Writes `family` out into `file` for each of its members.
fn expand_family(
    family: FamilyDef,
    file: &mut ContentFile,
    written: &Written,
) -> Result<(), String> {
    let members = members(&family.each)?;
    let mut excepted: Vec<(&str, bool)> = family
        .except
        .iter()
        .map(|name| (name.as_str(), false))
        .collect();
    // whether the family makes nothing of `name`, which it would make
    let mut skip = |name: &str| {
        excepted
            .iter_mut()
            .find(|(except, _)| *except == name)
            .map(|(_, met)| *met = true)
            .is_some()
    };
    let mut tags: Vec<(String, TagDef)> = Vec::new();
    for member in &members {
        let words: Vec<&str> = member.words.iter().map(|(_, word)| *word).collect();
        let words = words.join(", ");
        let context = |what: String| {
            let words = &words;
            move |err: String| format!("member {words}: {what} {err}")
        };
        for (name, def) in &family.blocks.0 {
            let name = member.write(name).map_err(context("block".into()))?;
            let def = member
                .write_all(def)
                .map_err(context(format!("block \"{name}\":")))?;
            if !skip(&name) && !written.blocks.contains(&name) {
                file.blocks.0.push((name, def));
            }
        }
        for (name, def) in &family.items.0 {
            let name = member.write(name).map_err(context("item".into()))?;
            let def = member
                .write_all(def)
                .map_err(context(format!("item \"{name}\":")))?;
            if !skip(&name) && !written.items.contains(&name) {
                file.items.0.push((name, def));
            }
        }
        for (name, TagDef(members)) in &family.tags.0 {
            let name = member.write(name).map_err(context("tag".into()))?;
            let mut added = Vec::new();
            for item in members {
                let item = member
                    .write(item)
                    .map_err(context(format!("tag \"{name}\":")))?;
                if !skip(&item) {
                    added.push(item);
                }
            }
            match tags.iter_mut().find(|(known, _)| *known == name) {
                Some((_, TagDef(known))) => known.extend(added),
                None => tags.push((name, TagDef(added))),
            }
        }
        for def in &family.recipes {
            let recipe = member.write_all(def).map_err(context("recipe".into()))?;
            if !skip(&recipe.out) {
                file.recipes.push(recipe);
            }
        }
        for (input, output) in &family.smelting {
            let input = member.write(input).map_err(context("smelting".into()))?;
            let output = member.write(output).map_err(context("smelting".into()))?;
            if !skip(&input) {
                file.smelting.push((input, output));
            }
        }
        for (name, FuelDef(ticks)) in &family.fuel.0 {
            let name = member.write(name).map_err(context("fuel".into()))?;
            if !skip(&name) {
                file.fuel.0.push((name, FuelDef(*ticks)));
            }
        }
    }
    if let Some((name, _)) = excepted.iter().find(|(_, met)| !met) {
        return Err(format!("except: the family makes nothing named \"{name}\""));
    }
    file.tags.0.extend(tags);
    Ok(())
}

/// The members of a family of `each`: the words of each list side by side.
fn members(each: &BTreeMap<String, Vec<String>>) -> Result<Vec<Member<'_>>, String> {
    let Some(count) = each.values().next().map(Vec::len) else {
        return Err("each: a family has one list of words at least".to_string());
    };
    for (name, words) in each {
        if name.is_empty() || !name.chars().all(|c| c.is_ascii_lowercase()) {
            return Err(format!(
                "each: \"{name}\" is not a name of lowercase letters, as w"
            ));
        }
        if words.len() != count {
            return Err(format!(
                "each: the lists do not all have {count} words, as the first"
            ));
        }
    }
    if count == 0 {
        return Err("each: a family has one member at least".to_string());
    }
    Ok((0..count)
        .map(|index| Member {
            words: each
                .iter()
                .map(|(name, words)| (name.as_str(), words[index].as_str()))
                .collect(),
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use crate::content::files::tests::game_folder_with;
    use crate::content::{Content, ContentError};
    use crate::world::{Ingredient, ItemCategory, ItemId, ItemType};

    fn load(text: &str) -> Result<Content, ContentError> {
        Content::load(game_folder_with(text).path())
    }

    /// The ticks `item` burns in a furnace with `content`.
    fn fuel_ticks_in(content: &Content, item: ItemId) -> Option<u16> {
        content
            .crafting
            .fuels
            .iter()
            .find(|(fuel, _)| *fuel == item)
            .map(|&(_, ticks)| ticks)
    }

    #[test]
    fn a_family_is_what_its_file_would_be_with_each_member_written_out() {
        let family = load(
            r#"(families: [(
                each: {"g": ["amber", "jade"], "o": ["sand", "gravel"]},
                blocks: {
                    "{g}_brick": (break: 4, tools: [Pickaxe], faces: (all: "{G}Brick")),
                    "{g}_tile": (like: "{g}_brick", drops: [(1, "{o}", 2)]),
                },
                items: {"{g}_brick": (category: Nature, name: (en: "{G} brick"))},
                tags: {"gem_bricks": ["{g}_brick"]},
                recipes: [(out: "{g}_tile", count: 2, pattern: ["B"], key: {'B': "{g}_brick"})],
                smelting: [("{g}_brick", "{o}")],
                fuel: {"{g}_tile": 50},
            )])"#,
        )
        .unwrap();
        let written = load(
            r#"(
                blocks: {
                    "amber_brick": (break: 4, tools: [Pickaxe], faces: (all: "AmberBrick")),
                    "amber_tile": (break: 4, tools: [Pickaxe], faces: (all: "AmberBrick"),
                                   drops: [(1, "sand", 2)]),
                    "jade_brick": (break: 4, tools: [Pickaxe], faces: (all: "JadeBrick")),
                    "jade_tile": (break: 4, tools: [Pickaxe], faces: (all: "JadeBrick"),
                                  drops: [(1, "gravel", 2)]),
                },
                items: {
                    "amber_brick": (category: Nature, name: (en: "Amber brick")),
                    "jade_brick": (category: Nature, name: (en: "Jade brick")),
                },
                tags: {"gem_bricks": ["amber_brick", "jade_brick"]},
                recipes: [
                    (out: "amber_tile", count: 2, pattern: ["B"], key: {'B': "amber_brick"}),
                    (out: "jade_tile", count: 2, pattern: ["B"], key: {'B': "jade_brick"}),
                ],
                smelting: [("amber_brick", "sand"), ("jade_brick", "gravel")],
                fuel: {"amber_tile": 50, "jade_tile": 50},
            )"#,
        )
        .unwrap();
        assert!(family.same_properties(&written));
        let tile = family.block_named("jade_tile").unwrap();
        assert_eq!(family.block(tile).break_time, Some(4));
        let brick = family.item_named("jade_brick").unwrap();
        assert_eq!(family.item(brick).category, ItemCategory::Nature);
        let tile = family.item_named("jade_tile").unwrap();
        assert_eq!(fuel_ticks_in(&family, tile), Some(50));
    }

    #[test]
    fn a_member_has_none_of_what_is_excepted_and_its_own_of_what_a_file_writes_out() {
        let content = load(
            r#"(
                blocks: {
                    "jade_brick": (break: 9),
                    "amber": (break: 2, aliases: ["amber_brick"]),
                },
                families: [(
                    each: {"g": ["amber", "jade", "onyx"]},
                    except: ["onyx_brick"],
                    blocks: {"{g}_brick": (break: 4)},
                    recipes: [(out: "{g}_brick", pattern: ["D"], key: {'D': "dirt"})],
                    fuel: {"{g}_brick": 50},
                )],
            )"#,
        )
        .unwrap();
        // the onyx has no brick, and no recipe of one
        assert_eq!(content.block_named("onyx_brick"), None);
        assert_eq!(content.item_named("onyx_brick"), None);
        // the jade's is the one written out, which the family's recipe makes
        let jade = content.block_named("jade_brick").unwrap();
        assert_eq!(content.block(jade).break_time, Some(9));
        // the amber's is the amber, which the alias names, and which it makes too
        let amber = content.block_named("amber").unwrap();
        assert_eq!(content.block_named("amber_brick"), Some(amber));
        let amber_item = content.item_named("amber").unwrap();
        assert_eq!(content.item_named("amber_brick"), Some(amber_item));
        assert_eq!(
            content.item(amber_item).default_type,
            ItemType::Block(amber)
        );
        assert_eq!(content.block_aliases["amber_brick"], amber);
        let dirt = Some(Ingredient::Item(ItemId::Dirt));
        let made: Vec<_> = content
            .crafting
            .recipes
            .iter()
            .filter(|recipe| recipe.cells == [dirt])
            .map(|recipe| recipe.output)
            .collect();
        assert_eq!(
            made,
            [amber_item, content.item_named("jade_brick").unwrap()]
        );
        assert_eq!(fuel_ticks_in(&content, amber_item), Some(50));
        // an alias is no block of its own
        assert!(!content.block_names.contains(&"amber_brick".to_string()));
    }

    #[test]
    fn a_bad_family_like_or_alias_is_refused_with_its_file_and_what_is_wrong() {
        let cases = [
            (
                r#"(families: [(each: {"g": ["a"], "h": ["b", "c"]})])"#,
                "family 1: each: the lists do not all have 1 words",
            ),
            (
                r#"(families: [(each: {})])"#,
                "family 1: each: a family has one",
            ),
            (r#"(families: [(each: {"g": []})])"#, "one member at least"),
            (
                r#"(families: [(each: {"G": ["amber"]})])"#,
                "\"G\" is not a name of lowercase letters",
            ),
            (
                r#"(families: [(each: {"g": ["amber"]}, blocks: {"{x}_brick": ()})])"#,
                "family 1: member amber: block \"{x}_brick\": the family has no word",
            ),
            (
                r#"(families: [(each: {"g": ["amber"]}, blocks: {"{g}_brick": (faces: (all: "{Gx}"))})])"#,
                "member amber: block \"amber_brick\": \"{Gx}\"",
            ),
            (
                r#"(families: [(each: {"g": ["amber"]}, except: ["amber_tile"], blocks: {"{g}_brick": ()})])"#,
                "except: the family makes nothing named \"amber_tile\"",
            ),
            (
                r#"(blocks: {"amber": (like: "ambre")})"#,
                "block \"amber\": like: no block is named \"ambre\"",
            ),
            (
                r#"(blocks: {"amber": (like: "jade"), "jade": (like: "amber")})"#,
                "like: amber is like jade is like amber again",
            ),
            (
                r#"(blocks: {"amber": (aliases: ["stone"])})"#,
                "block \"amber\": aliases: \"stone\" already names a block",
            ),
            (
                r#"(blocks: {"amber": (aliases: ["Amber"])})"#,
                "aliases: \"Amber\" is not words in lowercase",
            ),
            (
                r#"(items: {"apple": (aliases: ["stick"])})"#,
                "item \"apple\": aliases: \"stick\" already names an item",
            ),
        ];
        for (text, expected) in cases {
            let error = load(text).unwrap_err();
            assert_eq!(error.file.file_name().unwrap(), "extra.ron", "{text}");
            assert!(error.to_string().contains(expected), "{text}: {error}");
        }
    }

    /// To be removed with the matches of `palette.rs`: the families written out as the files
    /// were before them, and as the code pairs the doors, trapdoors, gates and powders.
    #[test]
    fn the_families_of_the_game_give_its_content_as_it_was_written_out_and_matched() {
        use crate::content::BlockProps;
        use crate::world::{BlockId, Form, Palette};
        use bevy::math::IVec3;

        let before = Content::load(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/content-before-families"),
        )
        .unwrap();
        let after = Content::load(&crate::content::files::tests::game_folder()).unwrap();
        assert_eq!(after.block_names, before.block_names);
        assert_eq!(after.block_idents, before.block_idents);
        assert_eq!(after.item_names, before.item_names);
        assert_eq!(after.item_idents, before.item_idents);
        for (name, (new, old)) in after
            .block_names
            .iter()
            .zip(after.blocks.iter().zip(&before.blocks))
        {
            let new = BlockProps {
                toggled: None,
                door_top: None,
                hardens: None,
                ..new.clone()
            };
            assert_eq!(new, *old, "{name}");
        }
        assert_eq!(after.items, before.items);
        assert_eq!(after.item_display, before.item_display);
        assert_eq!(after.crafting.tags, before.crafting.tags);
        assert_eq!(after.crafting.recipes, before.crafting.recipes);
        // tried for the first that matches, of which there is one
        let sorted = |mut list: Vec<(ItemId, ItemId)>| {
            list.sort();
            list
        };
        assert_eq!(
            sorted(after.crafting.smelting.clone()),
            sorted(before.crafting.smelting.clone())
        );
        let mut fuels = after.crafting.fuels.clone();
        let mut fuels_before = before.crafting.fuels.clone();
        fuels.sort();
        fuels_before.sort();
        assert_eq!(fuels, fuels_before);
        // the pairs the code matched
        for (index, name) in after.block_names.iter().enumerate() {
            let id = BlockId::from_index(index as u16);
            let props = after.block(id);
            let door = match props.palette {
                Some(Palette {
                    form: Form::Door { top, .. },
                    ..
                }) => props
                    .toggled
                    .map(|turned| (turned, if top { IVec3::NEG_Y } else { IVec3::Y })),
                _ => None,
            };
            assert_eq!(door, id.door(), "{name}");
            assert_eq!(props.door_top, id.door_top(), "{name}");
            let toggled = props.toggled.filter(|_| door.is_none());
            assert_eq!(toggled, id.toggled(), "{name}");
            assert_eq!(props.hardens, id.hardened(), "{name}");
        }
        let aliases: Vec<_> = after.block_aliases.keys().map(String::as_str).collect();
        assert_eq!(
            aliases,
            [
                "oak_door",
                "oak_door_top",
                "open_oak_door",
                "open_oak_door_top",
                "white_wool",
                "wooden_slab",
                "wooden_stairs"
            ]
        );
        let aliases: Vec<_> = after.item_aliases.keys().map(String::as_str).collect();
        assert_eq!(
            aliases,
            ["oak_door", "white_wool", "wooden_slab", "wooden_stairs"]
        );
    }
}
