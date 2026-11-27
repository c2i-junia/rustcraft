//! The recipes, what a furnace smelts and burns, and the families of items (tags) they take, as
//! the content files define them (`docs/design/content-registry.md`, 2.2):
//!
//! ```ron
//! (
//!     tags: { "planks": ["oak_planks", "spruce_planks"] },
//!     recipes: [
//!         (out: "oak_planks", count: 4, pattern: ["L"], key: {'L': "oak_log"}),
//!         (out: "stick", count: 4, pattern: ["P", "P"], key: {'P': "#planks"}),
//!         (out: "flint_and_steel", shapeless: ["iron_ingot", "flint"]),
//!     ],
//!     smelting: [("sand", "glass"), ("#raw_fish", "glass")],
//!     fuel: { "#planks": 300, "coal": 1600 },
//! )
//! ```
//!
//! An ingredient, an input or a fuel is an item, or a family when its name starts with `#`. The
//! recipes of every file are tried in the order of the files, then of each file; the first that
//! matches the grid makes its output.

use std::collections::{BTreeMap, HashMap};
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::files::{present, Kind, Named, Names};
use super::{ContentError, ItemProps};
use crate::world::{Ingredient, ItemId, ItemTag, CRAFTING_GRID_SIZE};

/// The recipes, what a furnace smelts and burns, and the families of items they take.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Crafting {
    /// By `ItemTag::index`: the families of `ItemTag::ENGINE` first, in its order.
    pub tags: Vec<Tag>,
    /// In the order they are tried, which is the index of each (`RecipeView::index`).
    pub recipes: Vec<Recipe>,
    /// What a furnace makes of each item it smelts, in the order of the files.
    pub smelting: Vec<(ItemId, ItemId)>,
    /// How many ticks each fuel burns, in the order of the files.
    pub fuels: Vec<(ItemId, u16)>,
}

/// A family of items, which a cell of a recipe may take any of.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Tag {
    /// As the files name it, without its `#`: `planks`.
    pub name: String,
    /// In the order of its file: the item browser shows them in turn.
    pub members: Vec<ItemId>,
}

/// A shaped recipe: its cells, row after row, must match the filled part of the grid exactly,
/// wherever that part sits in the grid, or match it mirrored left to right. A shapeless one only
/// needs its ingredients in the grid, one per cell, anywhere; its cells are laid out in rows of
/// two, or of three past four ingredients, for the item browser.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Recipe {
    pub width: usize,
    /// Row after row, `width` by row.
    pub cells: Vec<Option<Ingredient>>,
    pub output: ItemId,
    pub nb: u32,
    pub shapeless: bool,
}

/// The cells of a shapeless recipe of `ingredients`, row after row: two by row up to four,
/// three by row past, the last row filled with empty cells. Returns the width and the cells.
fn shapeless_cells(ingredients: Vec<Ingredient>) -> (usize, Vec<Option<Ingredient>>) {
    let width = match ingredients.len() {
        0 | 1 => 1,
        2..=4 => 2,
        _ => 3,
    };
    let mut cells: Vec<Option<Ingredient>> = ingredients.into_iter().map(Some).collect();
    cells.resize(cells.len().div_ceil(width) * width, None);
    (width, cells)
}

/// A family of items in a file: its members, by name.
#[derive(Deserialize, Debug)]
#[serde(transparent)]
pub(super) struct TagDef(pub Vec<String>);

impl Kind for TagDef {
    const KIND: &'static str = "tag";
}

/// The ticks a fuel burns, in a file.
#[derive(Deserialize, Debug)]
#[serde(transparent)]
pub(super) struct FuelDef(pub u16);

impl Kind for FuelDef {
    const KIND: &'static str = "fuel";
}

/// A recipe in a file: `out` and `count`, then either a `pattern` of rows, each character of
/// which is a cell, a space an empty one, the others named in `key`; or the `shapeless` list of
/// its ingredients.
#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
pub(super) struct RecipeDef {
    pub out: String,
    /// How many `out` it makes, 1 by default.
    #[serde(default, deserialize_with = "present")]
    pub count: Option<u32>,
    #[serde(default, deserialize_with = "present")]
    pub pattern: Option<Vec<String>>,
    #[serde(default, deserialize_with = "present")]
    pub key: Option<BTreeMap<char, String>>,
    #[serde(default, deserialize_with = "present")]
    pub shapeless: Option<Vec<String>>,
}

/// What a file holds of the crafting, `Crafting` by name.
#[derive(Default)]
pub(super) struct CraftingDefs {
    pub tags: Named<TagDef>,
    pub recipes: Vec<RecipeDef>,
    pub smelting: Vec<(String, String)>,
    pub fuel: Named<FuelDef>,
}

/// The names `Crafting` is built against: the items and blocks, the families, and which items
/// a file defines (not those only held as placeholders).
struct Resolver<'a> {
    names: &'a Names,
    items: &'a [ItemProps],
    tags: HashMap<String, ItemTag>,
}

impl Resolver<'_> {
    fn item(&self, field: &str, name: &str) -> Result<ItemId, String> {
        let item = self.names.item(field, name)?;
        if self.items[item.index()].placeholder {
            return Err(format!(
                "{field}: the item \"{name}\" is defined in no file"
            ));
        }
        Ok(item)
    }

    /// The item `name` names, or the family `#name` does.
    fn ingredient(&self, field: &str, name: &str) -> Result<Ingredient, String> {
        match name.strip_prefix('#') {
            Some(tag) => self
                .tags
                .get(tag)
                .map(|&tag| Ingredient::Tag(tag))
                .ok_or_else(|| format!("{field}: no tag is named \"{tag}\"")),
            None => self.item(field, name).map(Ingredient::Item),
        }
    }
}

impl Crafting {
    /// The crafting of `files`, the definitions of each file by its path, against the blocks
    /// and the items of `names`, whose properties are `items`. Every family of
    /// `ItemTag::ENGINE` must be defined, and every name must name something a file defines.
    pub(super) fn build(
        files: Vec<(&Path, CraftingDefs)>,
        folder: &Path,
        names: &Names,
        items: &[ItemProps],
    ) -> Result<Crafting, ContentError> {
        let fail = |file: &Path, reason: String| ContentError {
            file: file.to_path_buf(),
            reason,
        };
        // the families first, which the rest refer to
        let mut tag_names: Vec<String> = ItemTag::ENGINE_NAMES.map(String::from).to_vec();
        let mut others: Vec<String> = Vec::new();
        let mut defined: HashMap<&str, &Path> = HashMap::new();
        for (file, defs) in &files {
            for (name, _) in &defs.tags.0 {
                if let Some(other) = defined.insert(name, file) {
                    return Err(fail(
                        file,
                        format!("tag \"{name}\" is also defined in {}", other.display()),
                    ));
                }
                if !super::files::is_content_name(name) {
                    return Err(fail(
                        file,
                        format!("tag \"{name}\": a name is words in lowercase joined by _"),
                    ));
                }
                if !tag_names.contains(name) {
                    others.push(name.clone());
                }
            }
        }
        if let Some(name) = tag_names
            .iter()
            .find(|name| !defined.contains_key(name.as_str()))
        {
            return Err(fail(
                folder,
                format!("tag \"{name}\" is defined in no file"),
            ));
        }
        others.sort();
        tag_names.extend(others);
        let resolver = Resolver {
            names,
            items,
            tags: tag_names
                .iter()
                .enumerate()
                .map(|(index, name)| (name.clone(), ItemTag::from_index(index as u16)))
                .collect(),
        };

        let mut crafting = Crafting {
            tags: tag_names
                .into_iter()
                .map(|name| Tag {
                    name,
                    members: Vec::new(),
                })
                .collect(),
            ..Default::default()
        };
        for (file, defs) in &files {
            for (name, TagDef(members)) in &defs.tags.0 {
                let in_tag = |reason: String| fail(file, format!("tag \"{name}\": {reason}"));
                if members.is_empty() {
                    return Err(in_tag("a tag holds at least one item".to_string()));
                }
                let members = members
                    .iter()
                    .map(|member| resolver.item("members", member))
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(in_tag)?;
                crafting.tags[resolver.tags[name].index()].members = members;
            }
        }
        // the members of an ingredient, an input or a fuel
        let members = |ingredient: Ingredient| -> Vec<ItemId> {
            match ingredient {
                Ingredient::Item(item) => vec![item],
                Ingredient::Tag(tag) => crafting.tags[tag.index()].members.clone(),
            }
        };

        let mut recipes = Vec::new();
        // each with the file that makes it so
        let mut smelting: Vec<(ItemId, ItemId, &Path)> = Vec::new();
        let mut fuels: Vec<(ItemId, u16, &Path)> = Vec::new();
        for (file, defs) in &files {
            for (number, def) in defs.recipes.iter().enumerate() {
                let recipe = build_recipe(def, &resolver).map_err(|reason| {
                    fail(
                        file,
                        format!("recipe {} (out: \"{}\"): {reason}", number + 1, def.out),
                    )
                })?;
                recipes.push(recipe);
            }
            for (input, output) in &defs.smelting {
                let in_smelting =
                    |reason: String| fail(file, format!("smelting of \"{input}\": {reason}"));
                let output = resolver.item("output", output).map_err(in_smelting)?;
                let ingredient = resolver.ingredient("input", input).map_err(in_smelting)?;
                for item in members(ingredient) {
                    if let Some((.., other)) = smelting.iter().find(|(known, ..)| *known == item) {
                        return Err(in_smelting(format!(
                            "the item {} also smelts in {}",
                            name_of(names, item),
                            other.display()
                        )));
                    }
                    smelting.push((item, output, file));
                }
            }
            for (name, FuelDef(ticks)) in &defs.fuel.0 {
                let in_fuel = |reason: String| fail(file, format!("fuel \"{name}\": {reason}"));
                if *ticks == 0 {
                    return Err(in_fuel("0 ticks: a fuel burns 1 tick at least".to_string()));
                }
                let ingredient = resolver.ingredient("name", name).map_err(in_fuel)?;
                for item in members(ingredient) {
                    if let Some((.., other)) = fuels.iter().find(|(known, ..)| *known == item) {
                        return Err(in_fuel(format!(
                            "the item {} is also a fuel in {}",
                            name_of(names, item),
                            other.display()
                        )));
                    }
                    fuels.push((item, *ticks, file));
                }
            }
        }
        crafting.recipes = recipes;
        crafting.smelting = smelting
            .into_iter()
            .map(|(input, output, _)| (input, output))
            .collect();
        crafting.fuels = fuels
            .into_iter()
            .map(|(fuel, ticks, _)| (fuel, ticks))
            .collect();
        Ok(crafting)
    }
}

/// The name of `item` in the files, for an error.
fn name_of(names: &Names, item: ItemId) -> String {
    names
        .items
        .iter()
        .find(|(_, known)| **known == item)
        .map_or_else(|| format!("{item:?}"), |(name, _)| format!("\"{name}\""))
}

/// The recipe `def` defines, its names resolved by `resolver`.
fn build_recipe(def: &RecipeDef, resolver: &Resolver) -> Result<Recipe, String> {
    let output = resolver.item("out", &def.out)?;
    let nb = def.count.unwrap_or(1);
    if nb == 0 {
        return Err("count 0: a recipe makes 1 item at least".to_string());
    }
    let size = CRAFTING_GRID_SIZE as usize;
    let (width, cells, shapeless) = match (&def.pattern, &def.key, &def.shapeless) {
        (Some(pattern), Some(key), None) => {
            let (width, cells) = pattern_cells(pattern, key, resolver)?;
            (width, cells, false)
        }
        (None, None, Some(ingredients)) => {
            if ingredients.is_empty() || ingredients.len() > size * size {
                return Err(format!(
                    "shapeless: 1 to {} ingredients, not {}",
                    size * size,
                    ingredients.len()
                ));
            }
            let ingredients = ingredients
                .iter()
                .map(|name| resolver.ingredient("shapeless", name))
                .collect::<Result<Vec<_>, _>>()?;
            let (width, cells) = shapeless_cells(ingredients);
            (width, cells, true)
        }
        (Some(_), None, None) => return Err("a pattern needs its key".to_string()),
        _ => {
            return Err(
                "a recipe has a pattern and its key, or a shapeless list, not both".to_string(),
            )
        }
    };
    Ok(Recipe {
        width,
        cells,
        output,
        nb,
        shapeless,
    })
}

/// The width and the cells of the `pattern` of a shaped recipe, its characters read by `key`.
fn pattern_cells(
    pattern: &[String],
    key: &BTreeMap<char, String>,
    resolver: &Resolver,
) -> Result<(usize, Vec<Option<Ingredient>>), String> {
    let size = CRAFTING_GRID_SIZE as usize;
    let rows: Vec<Vec<char>> = pattern.iter().map(|row| row.chars().collect()).collect();
    let width = rows.first().map_or(0, Vec::len);
    if rows.is_empty() || rows.len() > size || width == 0 || width > size {
        return Err(format!("pattern: 1 to {size} rows of 1 to {size} cells"));
    }
    if rows.iter().any(|row| row.len() != width) {
        return Err("pattern: its rows are not all as long".to_string());
    }
    let empty_row = |row: &[char]| row.iter().all(|&c| c == ' ');
    let empty_column = |col: usize| rows.iter().all(|row| row[col] == ' ');
    if empty_row(&rows[0])
        || empty_row(&rows[rows.len() - 1])
        || empty_column(0)
        || empty_column(width - 1)
    {
        // the filled part of a grid has no empty edge
        return Err(
            "pattern: a row or a column at its edge is empty, so it never matches".to_string(),
        );
    }
    if key.contains_key(&' ') {
        return Err("key: ' ' is the empty cell".to_string());
    }
    let mut cells = Vec::with_capacity(width * rows.len());
    for c in rows.iter().flatten() {
        cells.push(match c {
            ' ' => None,
            c => {
                let name = key
                    .get(c)
                    .ok_or_else(|| format!("pattern: '{c}' is not in the key"))?;
                Some(resolver.ingredient("key", name)?)
            }
        });
    }
    if let Some(unused) = key
        .keys()
        .find(|c| !rows.iter().flatten().any(|cell| cell == *c))
    {
        return Err(format!("key: '{unused}' is in no row of the pattern"));
    }
    Ok((width, cells))
}

#[cfg(test)]
mod tests {
    use super::super::files::tests::{game_folder, game_folder_with};
    use super::*;
    use crate::content::Content;

    #[test]
    fn a_file_adds_recipes_smelting_fuels_and_families_after_those_of_the_game() {
        let game = Content::load(&game_folder()).unwrap().crafting;
        let folder = game_folder_with(
            r##"(
                tags: {"grains": ["gravel", "sand"]},
                recipes: [
                    (out: "lantern", count: 2, pattern: ["CTC", " K "], key: {'C': "#cobble", 'T': "torch", 'K': "coal"}),
                    (out: "glass", shapeless: ["sand", "sand", "#grains", "coal", "coal"]),
                ],
                smelting: [("dirt", "gravel")],
                fuel: {"#grains": 50, "dirt": 7},
            )"##,
        );
        let content = Content::load(folder.path()).unwrap();
        let crafting = &content.crafting;
        // extra.ron is read before recipes.ron
        assert_eq!(crafting.recipes.len(), game.recipes.len() + 2);
        assert_eq!(crafting.recipes[2..], game.recipes[..]);
        let grains = ItemTag::from_index(ItemTag::ENGINE.len() as u16);
        assert_eq!(crafting.tags[grains.index()].name, "grains");
        assert_eq!(
            crafting.tags[grains.index()].members,
            [ItemId::Gravel, ItemId::Sand]
        );
        let (c, t, k) = (
            Some(Ingredient::Tag(ItemTag::Cobble)),
            Some(Ingredient::Item(ItemId::Torch)),
            Some(Ingredient::Item(ItemId::Coal)),
        );
        assert_eq!(
            crafting.recipes[0],
            Recipe {
                width: 3,
                cells: vec![c, t, c, None, k, None],
                output: ItemId::Lantern,
                nb: 2,
                shapeless: false,
            }
        );
        let glass = &crafting.recipes[1];
        assert!(glass.shapeless);
        // five ingredients: rows of three
        assert_eq!(glass.width, 3);
        assert_eq!(glass.cells.len(), 6);
        assert_eq!(glass.cells[2], Some(Ingredient::Tag(grains)));
        assert_eq!(glass.cells[5], None);
        assert_eq!(crafting.smelting[0], (ItemId::Dirt, ItemId::Gravel));
        for fuel in [(ItemId::Gravel, 50), (ItemId::Sand, 50), (ItemId::Dirt, 7)] {
            assert!(crafting.fuels.contains(&fuel), "{fuel:?}");
        }
        assert!(!content.same_properties(&Content::load(&game_folder()).unwrap()));
    }

    /// The error of a bad recipe, smelting, fuel or family names the file and the entry, and
    /// nothing is loaded.
    #[test]
    fn a_bad_recipe_smelting_fuel_or_tag_is_refused_with_its_file_and_its_entry() {
        let cases = [
            (
                r#"(recipes: [(out: "torch", pattern: ["C"], key: {'C': "cole"})])"#,
                "recipe 1 (out: \"torch\"): key: no item is named \"cole\"",
            ),
            (
                r##"(recipes: [(out: "torch", shapeless: ["#sticks"])])"##,
                "recipe 1 (out: \"torch\"): shapeless: no tag is named \"sticks\"",
            ),
            (
                r#"(recipes: [(out: "tourch", shapeless: ["coal"])])"#,
                "out: no item is named \"tourch\"",
            ),
            (
                r#"(recipes: [(out: "torch", shapeless: ["coal"]), (out: "torch", count: 0, shapeless: ["coal"])])"#,
                "recipe 2 (out: \"torch\"): count 0",
            ),
            (
                r#"(recipes: [(out: "torch", pattern: ["CC", "C"], key: {'C': "coal"})])"#,
                "rows are not all as long",
            ),
            (
                r#"(recipes: [(out: "torch", pattern: ["CCCC"], key: {'C': "coal"})])"#,
                "pattern: 1 to 3 rows",
            ),
            (
                r#"(recipes: [(out: "torch", pattern: ["CX"], key: {'C': "coal"})])"#,
                "'X' is not in the key",
            ),
            (
                r#"(recipes: [(out: "torch", pattern: ["C"], key: {'C': "coal", 'S': "stick"})])"#,
                "'S' is in no row",
            ),
            (
                r#"(recipes: [(out: "torch", pattern: ["C ", "C "], key: {'C': "coal"})])"#,
                "at its edge is empty",
            ),
            (
                r#"(recipes: [(out: "torch", pattern: ["C"])])"#,
                "a pattern needs its key",
            ),
            (
                r#"(recipes: [(out: "torch", pattern: ["C"], key: {'C': "coal"}, shapeless: ["coal"])])"#,
                "not both",
            ),
            (
                r#"(recipes: [(out: "torch", shapeless: ["coal", "coal", "coal", "coal", "coal", "coal", "coal", "coal", "coal", "coal"])])"#,
                "1 to 9 ingredients, not 10",
            ),
            (
                r#"(recipes: [(out: "torch", shapless: ["coal"])])"#,
                "shapless",
            ),
            (r#"(smelting: [("dirt", "glas")])"#, "smelting of \"dirt\""),
            (
                r##"(smelting: [("dirt", "#cobble")])"##,
                "smelting of \"dirt\": output: no item is named \"#cobble\"",
            ),
            (
                r#"(smelting: [("sand", "stone")])"#,
                "smelting of \"sand\": the item \"sand\" also smelts in",
            ),
            (r#"(fuel: {"dirt": 0})"#, "fuel \"dirt\": 0 ticks"),
            (
                r#"(fuel: {"coal": 10})"#,
                "fuel \"coal\": the item \"coal\" is also a fuel in",
            ),
            (
                r##"(fuel: {"#wood": 10})"##,
                "fuel \"#wood\": name: no tag is named \"wood\"",
            ),
            (
                r#"(tags: {"logs": ["oak_log", "oak_lgo"]})"#,
                "tag \"logs\": members: no item",
            ),
            (
                r#"(tags: {"logs": []})"#,
                "tag \"logs\": a tag holds at least one item",
            ),
            (
                r#"(tags: {"planks": ["oak_planks"]})"#,
                "tag \"planks\" is also defined in",
            ),
            (
                r#"(tags: {"Logs": ["oak_log"]})"#,
                "tag \"Logs\": a name is",
            ),
        ];
        for (text, key) in cases {
            let error = Content::load(game_folder_with(text).path()).unwrap_err();
            // the file at fault, or the other one of an entry written twice
            assert!(error.to_string().contains("extra.ron"), "{text}: {error}");
            assert!(error.to_string().contains(key), "{text}: {error}");
        }
    }

    #[test]
    fn the_families_of_the_engine_cannot_leave_the_files() {
        let folder = game_folder_with("()");
        std::fs::remove_file(folder.path().join("recipes.ron")).unwrap();
        let error = Content::load(folder.path()).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("tag \"planks\" is defined in no file"),
            "{error}"
        );
    }

    #[test]
    fn a_recipe_cannot_take_an_item_the_files_no_longer_define() {
        let before = Content::load(game_folder_with(r#"(items: {"ruby": ()})"#).path()).unwrap();
        // the ruby is gone, its number held by a placeholder
        let error = Content::reload(
            game_folder_with(r#"(recipes: [(out: "torch", shapeless: ["ruby"])])"#).path(),
            &before,
        )
        .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("shapeless: the item \"ruby\" is defined in no file"),
            "{error}"
        );
    }
}
