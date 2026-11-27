//! The one-shot dump of the content written in the code (`Content::builtin`) into the content
//! files of the game, `data/content/rustcraft/`, a file per kind of content:
//! `cargo test -p shared dump_content_files -- --ignored`. A definition leaves out each field
//! at its default, and an item that places the block of its name and has only defaults has no
//! entry. The test beside it checks that the files read as the code says.

use std::collections::BTreeMap;

use super::files::tests::game_folder;
use super::files::{block_name, BlockDef, ItemDef, Names, PaletteDef};
use super::{BlockProps, Content, ItemProps};
use crate::world::{
    BlockHitbox, BlockId, BlockTransparency, ItemCategory, ItemId, ItemType, ToolKind,
};

/// The definition that gives `props` to the block `id`, without the fields at their default.
fn block_def(id: BlockId, props: &BlockProps, names: &Names) -> BlockDef {
    let name = block_name(id);
    let BlockProps {
        palette,
        visibility,
        light_opacity,
        light_emission,
        break_time,
        faster_tools,
        required_tool,
        drops,
        hitbox,
        ray_hitbox,
        usable,
        replaceable,
        tall,
        glass,
        container,
        sapling,
        soil,
    } = props.clone();
    let defaults = BlockDef {
        look: Some(visibility),
        glass: Some(glass),
        collision: Some(hitbox),
        ..Default::default()
    }
    .build(&name, names)
    .unwrap();
    assert!(
        light_opacity.is_some() || defaults.light_opacity.is_none(),
        "{name}: a block no light goes through is solid"
    );
    let unless = |given: bool| given.then_some(true);
    BlockDef {
        palette: palette.map(|palette| PaletteDef {
            form: palette.form,
            base: block_name(palette.base),
            item: palette.item.name(),
        }),
        look: (visibility != BlockTransparency::Solid).then_some(visibility),
        light: (light_emission != 0).then_some(light_emission),
        opacity: light_opacity.filter(|_| light_opacity != defaults.light_opacity),
        break_hits: break_time,
        tools: (faster_tools != 0).then(|| {
            ToolKind::ALL
                .into_iter()
                .filter(|kind| faster_tools & super::tool_bit(*kind) != 0)
                .collect()
        }),
        needs_tool: required_tool,
        drops: (drops != defaults.drops).then(|| {
            drops
                .iter()
                .map(|&(chance, item, count)| (chance, item.name(), count))
                .collect()
        }),
        collision: (hitbox != BlockHitbox::FullBlock).then_some(hitbox),
        outline: (ray_hitbox != hitbox).then_some(ray_hitbox),
        usable: unless(usable),
        replaceable: unless(replaceable),
        tall: unless(tall),
        glass: unless(glass),
        container: unless(container),
        sapling: sapling.map(|item| item.name()),
        soil: soil.map(|soil| soil.into_iter().map(block_name).collect()),
    }
}

/// The definition that gives `props` to the item `id`, without the fields at their default.
fn item_def(id: ItemId, props: &ItemProps, names: &Names) -> ItemDef {
    let name = id.name();
    let defaults = ItemDef::default().build(&name, names).unwrap();
    let ItemProps {
        max_stack,
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
        default_type,
    } = props.clone();
    let typed = (default_type != defaults.default_type).then_some(default_type);
    ItemDef {
        stack: (max_stack != defaults.max_stack).then_some(max_stack),
        attack_damage: (attack_damage != defaults.attack_damage).then_some(attack_damage),
        food_value,
        food_effect,
        eaten_remains: eaten_remains.map(|item| item.name()),
        crafting_remains: crafting_remains.map(|item| item.name()),
        horse_armour,
        spawned_mob,
        tool,
        durability,
        category: (category != defaults.category).then_some(category),
        places: match typed {
            Some(ItemType::Block(block)) => Some(block_name(block)),
            _ => None,
        },
        default_type: typed.filter(|typed| !matches!(typed, ItemType::Block(_))),
    }
}

/// The file of the items of `category`, and of the blocks they place.
fn category_file(category: ItemCategory) -> &'static str {
    match category {
        ItemCategory::Blocks => "blocks",
        ItemCategory::Wood => "wood",
        ItemCategory::Nature => "nature",
        ItemCategory::Ores => "ores",
        ItemCategory::Materials => "materials",
        ItemCategory::Stations => "stations",
        ItemCategory::Tools => "tools",
        ItemCategory::Mobs => "mobs",
    }
}

/// The file a block goes in: the palette has its own; another block goes with the item of its
/// name, or else with the item it drops.
fn block_file(id: BlockId, content: &Content, names: &Names) -> &'static str {
    let props = content.block(id);
    if props.palette.is_some() {
        return "palette";
    }
    let item = names
        .items
        .get(&block_name(id))
        .copied()
        .or_else(|| props.drops.first().map(|&(_, item, _)| item));
    item.map_or("blocks", |item| item_file(item, content))
}

/// The file an item goes in: with the block of the palette it places, or else by its category.
fn item_file(id: ItemId, content: &Content) -> &'static str {
    match content.item(id).default_type {
        ItemType::Block(block) if content.block(block).palette.is_some() => "palette",
        _ => category_file(content.item(id).category),
    }
}

/// `value` written on one line, as RON, with a space after each comma and colon.
fn one_line<T: serde::Serialize>(value: &T) -> String {
    let compact = ron::ser::to_string(value).unwrap();
    // the names and the numbers of the definitions hold neither
    compact.replace(',', ", ").replace(':', ": ")
}

/// The text of each content file of `content`, by file name.
fn dump(content: &Content) -> BTreeMap<String, String> {
    let names = Names::new();
    let mut files: BTreeMap<&str, (Vec<String>, Vec<String>)> = BTreeMap::new();
    for id in BlockId::ALL {
        let def = block_def(id, content.block(id), &names);
        let line = format!("        \"{}\": {},", block_name(id), one_line(&def));
        files
            .entry(block_file(id, content, &names))
            .or_default()
            .0
            .push(line);
    }
    for id in ItemId::ALL {
        let def = item_def(id, content.item(id), &names);
        if def == ItemDef::default() && names.blocks.contains_key(&id.name()) {
            continue;
        }
        let line = format!("        \"{}\": {},", id.name(), one_line(&def));
        files
            .entry(item_file(id, content))
            .or_default()
            .1
            .push(line);
    }
    files
        .into_iter()
        .map(|(name, (blocks, items))| {
            let mut text = String::from(HEADER);
            text.push_str("(\n");
            for (section, lines) in [("blocks", blocks), ("items", items)] {
                if lines.is_empty() {
                    continue;
                }
                text.push_str(&format!("    {section}: {{\n"));
                for line in lines {
                    text.push_str(&line);
                    text.push('\n');
                }
                text.push_str("    },\n");
            }
            text.push_str(")\n");
            (format!("{name}.ron"), text)
        })
        .collect()
}

/// The comment at the top of each file.
const HEADER: &str = "\
// Blocks and items of the base game, named as the commands name them. Every `*.ron` of this
// folder is read at start, and again once one is saved (or on `/reload`), by the server, which
// sends the content to its players; a set of files with an error changes nothing, and the error
// goes to the log and to the chat. The fields, and their defaults, are those of `BlockDef` and
// `ItemDef` in `shared/src/content/files.rs`. A block or an item is defined in one file only.
";

/// Writes the content files of the game from the content written in the code.
#[test]
#[ignore = "writes data/content/rustcraft: run it once with --ignored"]
fn dump_content_files() {
    let folder = game_folder();
    for entry in std::fs::read_dir(&folder).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|ext| ext == "ron") {
            std::fs::remove_file(path).unwrap();
        }
    }
    for (name, text) in dump(&Content::builtin()) {
        std::fs::write(folder.join(name), text).unwrap();
    }
}

/// The content files of the game say what the code says, block by block and item by item:
/// a block or item added to the code and not to the files fails here, naming it.
#[test]
fn the_content_files_are_the_builtin_content() {
    let files = Content::load(&game_folder()).unwrap_or_else(|err| panic!("{err}"));
    let builtin = Content::builtin();
    for id in BlockId::ALL {
        assert_eq!(
            files.block(id),
            builtin.block(id),
            "block {}",
            block_name(id)
        );
    }
    for id in ItemId::ALL {
        assert_eq!(files.item(id), builtin.item(id), "item {}", id.name());
    }
    assert!(files.same_properties(&builtin));
}
