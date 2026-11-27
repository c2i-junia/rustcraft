//! The one-shot dump of the recipes, the smelting, the fuels and the families written in the code
//! into the content file `data/content/rustcraft/recipes.ron`:
//! `cargo test -p shared dump_crafting_file -- --ignored`. The test beside it checks that the
//! files read as the code says.

use std::fmt::Write;

use super::*;
use crate::content::Content;
use crate::world::{fuel_ticks, smelting_result};

/// The name of `item`, an item of the engine, in the files.
fn name(item: ItemId) -> String {
    crate::utils::snake_case(&format!("{item:?}"))
}

/// An ingredient as a file writes it: the item's name, or `#` and the family's.
fn ingredient(ingredient: Ingredient) -> String {
    match ingredient {
        Ingredient::Item(item) => name(item),
        Ingredient::Tag(tag) => format!("#{}", ItemTag::ENGINE_NAMES[tag.index()]),
    }
}

/// The text of a list of names.
fn list(names: impl IntoIterator<Item = String>) -> String {
    let names: Vec<String> = names
        .into_iter()
        .map(|name| format!("\"{name}\""))
        .collect();
    format!("[{}]", names.join(", "))
}

/// The line of `recipe` in the file.
fn recipe_line(recipe: &Recipe) -> String {
    let mut line = format!("(out: \"{}\"", name(recipe.output));
    if recipe.nb != 1 {
        write!(line, ", count: {}", recipe.nb).unwrap();
    }
    if recipe.shapeless {
        let ingredients = recipe.cells.iter().flatten().map(|&cell| ingredient(cell));
        write!(line, ", shapeless: {})", list(ingredients)).unwrap();
        return line;
    }
    // a letter per ingredient: one of its name if it is free
    let mut key: Vec<(char, Ingredient)> = Vec::new();
    for &cell in recipe.cells.iter().flatten() {
        if key.iter().any(|(_, known)| *known == cell) {
            continue;
        }
        let written = ingredient(cell);
        let letter = written
            .chars()
            .filter(char::is_ascii_alphabetic)
            .map(|c| c.to_ascii_uppercase())
            .chain('A'..='Z')
            .find(|c| key.iter().all(|(used, _)| used != c))
            .unwrap();
        key.push((letter, cell));
    }
    let letter = |cell: Option<Ingredient>| match cell {
        None => ' ',
        Some(cell) => key.iter().find(|(_, known)| *known == cell).unwrap().0,
    };
    let rows = recipe
        .cells
        .chunks(recipe.width)
        .map(|row| row.iter().map(|&cell| letter(cell)).collect::<String>());
    let key: Vec<String> = key
        .iter()
        .map(|&(letter, cell)| format!("'{letter}': \"{}\"", ingredient(cell)))
        .collect();
    write!(
        line,
        ", pattern: {}, key: {{{}}})",
        list(rows),
        key.join(", ")
    )
    .unwrap();
    line
}

#[test]
#[ignore = "writes data/content/rustcraft/recipes.ron, once"]
fn dump_crafting_file() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../data/content/rustcraft/recipes.ron");
    // the families first, alone, which the content of the process needs to load
    let tags: String = ItemTag::ENGINE
        .into_iter()
        .zip(ItemTag::ENGINE_NAMES)
        .map(|(tag, tag_name)| {
            let members = tag.members().iter().map(|&item| name(item));
            format!("        \"{tag_name}\": {},\n", list(members))
        })
        .collect();
    std::fs::write(&path, format!("(\n    tags: {{\n{tags}    }},\n)\n")).unwrap();
    let mut text = String::from(
        "// The recipes, what a furnace smelts and burns, and the families of items (tags) they \
         take, named\n// as the commands name the items, a family as `#planks`. The recipes are \
         tried in the order of\n// the files, then of each file. The fields are those of \
         `RecipeDef` in\n// `shared/src/content/recipes.rs`.\n(\n    tags: {\n",
    );
    text.push_str(&tags);
    text.push_str("    },\n    recipes: [\n");
    let sections = [
        (RECIPES.len(), None),
        (
            palette_recipes().len(),
            Some("The palette: the slabs, stairs, doors, trapdoors and gates of each wood, the slabs, stairs, low walls, polished and chiseled blocks of each rock, the dyes and what they colour"),
        ),
        (
            MAP_RECIPES.len(),
            Some("The maps: paper from reeds, the empty map, its copy (the map drawn stays in the grid), the compass and the frame"),
        ),
        (
            decor_recipes().len(),
            Some("The furniture: bedrolls, carpets, banners and lanterns of each colour, tables and chairs of each wood, the book, the bookshelf, the flower pot, the item frame and the paintings"),
        ),
        (ALCHEMY_RECIPES.len(), Some("Alchemy: the alembic and the glass vials")),
    ];
    let mut recipes = all_recipes().iter();
    for (count, comment) in sections {
        if let Some(comment) = comment {
            writeln!(text, "        // {comment}").unwrap();
        }
        for recipe in recipes.by_ref().take(count) {
            writeln!(text, "        {},", recipe_line(recipe)).unwrap();
        }
    }
    assert!(recipes.next().is_none(), "every recipe is in a section");
    text.push_str("    ],\n    smelting: [\n");
    for item in ItemId::all() {
        if let Some(result) = smelting_result(item) {
            writeln!(text, "        (\"{}\", \"{}\"),", name(item), name(result)).unwrap();
        }
    }
    text.push_str("    ],\n    fuel: {\n");
    let planks = ItemTag::Planks.members();
    let planks_ticks = fuel_ticks(planks[0]).unwrap();
    assert!(planks
        .iter()
        .all(|&item| fuel_ticks(item) == Some(planks_ticks)));
    writeln!(text, "        \"#planks\": {planks_ticks},").unwrap();
    for item in ItemId::all().filter(|item| !planks.contains(item)) {
        if let Some(ticks) = fuel_ticks(item) {
            writeln!(text, "        \"{}\": {ticks},", name(item)).unwrap();
        }
    }
    text.push_str("    },\n)\n");
    std::fs::write(path, text).unwrap();
}

/// The files say what the code says: the same recipes in the same order, with the same cells,
/// outputs and counts; the same smelting; the same fuels; the same families.
#[test]
fn the_crafting_of_the_files_is_that_of_the_code() {
    let folder = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../data/content/rustcraft");
    let crafting = Content::load(&folder).unwrap().crafting;

    let code: Vec<crate::content::Recipe> = all_recipes()
        .iter()
        .map(|recipe| crate::content::Recipe {
            width: recipe.width,
            cells: recipe.cells.to_vec(),
            output: recipe.output,
            nb: recipe.nb,
            shapeless: recipe.shapeless,
        })
        .collect();
    assert_eq!(crafting.recipes.len(), code.len());
    for (index, (files, code)) in crafting.recipes.iter().zip(&code).enumerate() {
        assert_eq!(files, code, "recipe {index}");
    }

    let tags: Vec<(String, Vec<ItemId>)> = ItemTag::ENGINE
        .into_iter()
        .zip(ItemTag::ENGINE_NAMES)
        .map(|(tag, name)| (name.to_string(), tag.members().to_vec()))
        .collect();
    let file_tags: Vec<(String, Vec<ItemId>)> = crafting
        .tags
        .iter()
        .map(|tag| (tag.name.clone(), tag.members.clone()))
        .collect();
    assert_eq!(file_tags, tags);

    for item in ItemId::all() {
        let smelts = crafting
            .smelting
            .iter()
            .find(|(input, _)| *input == item)
            .map(|(_, output)| *output);
        assert_eq!(smelts, smelting_result(item), "the smelting of {item:?}");
        let burns = crafting
            .fuels
            .iter()
            .find(|(fuel, _)| *fuel == item)
            .map(|(_, ticks)| *ticks);
        assert_eq!(burns, fuel_ticks(item), "the fuel {item:?}");
    }
    // no item twice, and only those of the code
    assert_eq!(
        crafting.smelting.len(),
        ItemId::all()
            .filter(|&item| smelting_result(item).is_some())
            .count()
    );
    assert_eq!(
        crafting.fuels.len(),
        ItemId::all()
            .filter(|&item| fuel_ticks(item).is_some())
            .count()
    );
}
