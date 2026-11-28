//! Writes what the wiki (`wiki/`) shows of the game as JSON, read from the game's own tables so
//! that the wiki says what the game does: every item and block, the recipes, the smelting, the
//! alchemy, the creatures, the biomes, the enchantments, the effects and the trades.
//!
//! ```sh
//! cargo run -p shared --example wiki_export -- wiki/src/lib/data/game.json
//! ```

use std::collections::BTreeMap;
use std::path::Path;

use serde_json::{json, Value};
use shared::codex::markup;
use shared::players::effects::EffectKind;
use shared::world::alchemy;
use shared::world::fishing::CatchKind;
use shared::world::trade::{offers, Goods};
use shared::world::{
    fuel_ticks, recipes, repair_material, smelting_result, Biomes, BlockId, Enchantment,
    Ingredient, ItemCategory, ItemId, ItemType, LootCount, MobKind, ToolKind, COOK_TICKS,
    MAX_ENCHANTMENT_LEVEL,
};
use shared::TICKS_PER_SECOND;

const TOOL_KINDS: [ToolKind; 5] = [
    ToolKind::Pickaxe,
    ToolKind::Axe,
    ToolKind::Shovel,
    ToolKind::Hoe,
    ToolKind::Sword,
];

fn main() {
    let out = std::env::args()
        .nth(1)
        .expect("usage: wiki_export <out.json>");
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let en = lang(&root.join("data/lang/en.ron"));
    let fr = lang(&root.join("data/lang/fr.ron"));
    let textures = root.join("data/graphics/base_textures");
    let has = |folder: &str, name: &str| textures.join(format!("{folder}/{name}.png")).is_file();
    let tr = |key: String| -> Value { json!({ "en": en.get(&key), "fr": fr.get(&key) }) };
    let seconds = |ticks: u32| ticks as f64 / TICKS_PER_SECOND as f64;
    // The texts of the Codex, in markup: a note of blocks, a line of runs
    let note = |key: String| -> Value {
        en.get(&key).map_or(Value::Null, |text| {
            json!(markup::parse(text).unwrap_or_else(|e| panic!("{key}: {e}")))
        })
    };
    let line = |key: String| -> Value {
        en.get(&key).map_or(Value::Null, |text| {
            json!(markup::inline(text).unwrap_or_else(|e| panic!("{key}: {e}")))
        })
    };

    let items: Vec<Value> = ItemId::all()
        .map(|id| {
            let key = format!("{id:?}");
            let block = match id.get_default_type() {
                ItemType::Block(block) => Some(format!("{block:?}")),
                _ => None,
            };
            let icon = if has("items", &key) {
                Some(format!("items/{key}.png"))
            } else if has("blocks", &key) {
                Some(format!("blocks/{key}.png"))
            } else {
                None
            };
            let top = format!("{key}Top");
            json!({
                "id": key,
                "slug": id.name(),
                "name": tr(format!("item.{key}")),
                "category": category(id.category()),
                "icon": icon,
                // A block with its own top, drawn on the cube of the icon
                "iconTop": (icon.as_deref().is_some_and(|i| i.starts_with("blocks/")) && has("blocks", &top))
                    .then(|| format!("blocks/{top}.png")),
                "block": block,
                "maxStack": id.get_max_stack(),
                "attackDamage": id.attack_damage(),
                "durability": id.durability(),
                "food": id.food_value().map(|(hunger, saturation)| json!({ "hunger": hunger, "saturation": saturation })),
                "foodEffect": id.food_effect().map(|(kind, level, ticks)| json!({ "effect": format!("{kind:?}"), "level": level, "seconds": seconds(ticks) })),
                "eatenRemains": id.eaten_remains().map(|i| format!("{i:?}")),
                "tool": id.tool().map(|t| json!({
                    "kind": format!("{:?}", t.kind),
                    "tier": format!("{:?}", t.tier),
                    "speed": t.speed,
                    "miningLevel": t.tier.mining_level(),
                })),
                "armour": id.armour().map(|a| json!({
                    "piece": format!("{:?}", a.piece),
                    "material": format!("{:?}", a.material),
                    "points": a.points(),
                    "durability": a.durability(),
                })),
                "horseArmour": id.horse_armour(),
                "spawns": id.spawned_mob().map(|m| format!("{m:?}")),
                "fuelSeconds": fuel_ticks(id).map(|t| seconds(t as u32)),
                "smeltsInto": smelting_result(id).map(|i| format!("{i:?}")),
                "potion": id.potion().map(|p| json!({
                    "effect": format!("{:?}", p.effect),
                    "seconds": p.seconds,
                    "splash": p.splash,
                    "stronger": p.strengthens(),
                    "longer": p.lengthens(),
                })),
                "potionColor": id.potion_color(),
                "repair": repair_material(id).map(ingredient),
                "enchantments": Enchantment::ALL.iter().filter(|e| e.fits(id)).map(|e| format!("{e:?}")).collect::<Vec<_>>(),
                "note": note(format!("codex.item.{key}")),
            })
        })
        .collect();

    let blocks: Vec<Value> = BlockId::all()
        .map(|id| {
            let key = format!("{id:?}");
            let drops: Vec<Value> = id
                .get_drop_table()
                .into_iter()
                .map(|(chance, item, nb)| json!({ "chance": chance, "item": format!("{item:?}"), "count": nb }))
                .collect();
            json!({
                "id": key,
                "breakTime": id.get_break_time(),
                "requiredTool": id.get_required_tool().map(|t| json!({ "kind": format!("{:?}", t.kind), "level": t.level })),
                "fasterWith": TOOL_KINDS.iter().filter(|k| id.is_mined_faster_by(**k)).map(|k| format!("{k:?}")).collect::<Vec<_>>(),
                "drops": drops,
                "light": id.light_emission(),
                "container": id.is_container(),
                "usable": id.is_usable(),
                "texture": has("blocks", &key).then(|| format!("blocks/{key}.png")),
            })
        })
        .collect();

    let recipes: Vec<Value> = recipes()
        .map(|r| {
            json!({
                "output": format!("{:?}", r.output.item_id),
                "count": r.output.nb,
                "width": r.width,
                "height": r.height,
                "cells": r.cells.iter().map(|c| c.map(ingredient)).collect::<Vec<_>>(),
                "table": !r.fits(2),
            })
        })
        .collect();

    let tags: BTreeMap<String, Vec<String>> = [
        shared::world::ItemTag::Planks,
        shared::world::ItemTag::Cobble,
        shared::world::ItemTag::Wool,
        shared::world::ItemTag::RawFish,
        shared::world::ItemTag::RoastMeat,
    ]
    .into_iter()
    .map(|tag| {
        (
            // the Rust name, `Planks`, as the wiki knows them
            format!("{tag:?}"),
            tag.members().iter().map(|i| format!("{i:?}")).collect(),
        )
    })
    .collect();

    let alchemy: Vec<Value> = alchemy::RECIPES
        .iter()
        .map(|(vial, ingredient, result)| {
            json!({ "vial": format!("{vial:?}"), "ingredient": format!("{ingredient:?}"), "result": format!("{result:?}") })
        })
        .collect();

    let mobs: Vec<Value> = MobKind::ALL
        .iter()
        .filter(|kind| !kind.is_projectile())
        .map(|&kind| {
            let key = format!("{kind:?}");
            let size = kind.size();
            json!({
                "id": key,
                "name": tr(format!("mob.{key}")),
                "health": kind.health(),
                "hostile": kind.is_hostile(),
                "aquatic": kind.is_aquatic(),
                "boss": kind.is_boss(),
                "villager": kind.is_villager(),
                "vehicle": kind.is_vehicle(),
                "animal": kind.is_animal(),
                "tameable": kind.is_tameable(),
                "mount": kind.is_mount(),
                "leashable": kind.is_leashable(),
                "carriesChest": kind.carries_chest(),
                "weakToAxes": kind.weak_to_axes(),
                "size": [size.x, size.y, size.z],
                "eats": ItemId::all().filter(|i| kind.eats(*i)).map(|i| format!("{i:?}")).collect::<Vec<_>>(),
                "egg": ItemId::all().find(|i| i.spawned_mob() == Some(kind)).map(|i| format!("{i:?}")),
                "codex": {
                    "summary": line(format!("codex.mob.{key}.summary")),
                    "spawns": line(format!("codex.mob.{key}.spawns")),
                    "behaviour": line(format!("codex.mob.{key}.behaviour")),
                    "speed": line(format!("codex.mob.{key}.speed")),
                },
                "loot": kind.loot().iter().map(|l| json!({
                    "item": format!("{:?}", l.item),
                    "count": match l.count {
                        LootCount::Range(min, max) => json!({ "min": min, "max": max }),
                        LootCount::Odds(n, d) => json!({ "chance": n as f64 / d as f64 }),
                        LootCount::Chance(p) => json!({ "chance": p }),
                    },
                    "when": format!("{:?}", l.when),
                })).collect::<Vec<_>>(),
                "damage": kind.strike().map(|s| s.damage),
                "effect": kind.inflicted_effect().map(|(effect, level, seconds)| json!({ "effect": format!("{effect:?}"), "level": level, "seconds": seconds })),
                "xp": kind.experience(),
                "sight": kind.hunt().map(|h| json!({ "sight": tenths(h.sight), "forget": tenths(h.forget), "speed": tenths(h.speed) })),
                "walk": tenths(kind.walk_speed()),
                "trades": offers(kind).iter().map(|o| json!({
                    "cost": { "item": format!("{:?}", o.cost.0), "count": o.cost.1 },
                    "goods": match o.goods {
                        Goods::Items(item, nb) => json!({ "item": format!("{item:?}"), "count": nb }),
                        Goods::Directions => json!({ "directions": true }),
                    },
                    "stock": o.stock,
                })).collect::<Vec<_>>(),
            })
        })
        .collect();

    let biomes: Vec<Value> = Biomes::vanilla()
        .defs()
        .iter()
        .map(|def| serde_json::to_value(def).expect("a biome serializes"))
        .collect();

    let biome_notes: BTreeMap<String, Value> = Biomes::vanilla()
        .defs()
        .iter()
        .map(|def| {
            let key = def.key.trim_start_matches("rustcraft:").to_string();
            let text = line(format!("codex.biome.{key}"));
            (key, text)
        })
        .collect();

    let enchantments: Vec<Value> = Enchantment::ALL
        .iter()
        .map(|&e| {
            json!({
                "id": format!("{e:?}"),
                "name": e.name(),
                "levels": (1..=MAX_ENCHANTMENT_LEVEL).map(|l| e.effect(l)).collect::<Vec<_>>(),
                "items": ItemId::all().filter(|i| e.fits(*i)).map(|i| format!("{i:?}")).collect::<Vec<_>>(),
            })
        })
        .collect();

    let effects: Vec<Value> = EffectKind::ALL
        .iter()
        .map(|&kind| {
            let key = format!("{kind:?}");
            json!({
                "id": key,
                "name": tr(format!("effect.{key}")),
                "beneficial": kind.is_beneficial(),
                "instant": kind.is_instant(),
                "does": tr(format!("codex.effect.{key}")),
            })
        })
        .collect();

    let achievements: Vec<Value> = en
        .keys()
        .filter_map(|k| k.strip_prefix("achievement.")?.strip_suffix(".title"))
        .map(|id| {
            json!({
                "id": id,
                "title": tr(format!("achievement.{id}.title")),
                "goal": tr(format!("achievement.{id}.goal")),
            })
        })
        .collect();

    // What a fishing rod brings back: each kind of catch, its share and its pool
    let fishing: Vec<Value> = CatchKind::ALL
        .iter()
        .map(|&kind| {
            json!({
                "name": format!("{kind:?}"),
                "share": kind.percent() as f64 / 100.0,
                "entries": kind.pool().iter().map(|c| json!({
                    "item": format!("{:?}", c.item),
                    "weight": c.weight,
                    "count": c.count,
                    "night": c.night_only,
                })).collect::<Vec<_>>(),
            })
        })
        .collect();

    // The guides, their tables worked out: the wiki only draws them
    let guide = |slug: &str, language: &str| -> Value {
        let source = shared::codex::guides::source(slug, language).unwrap();
        let blocks = markup::parse(source).unwrap_or_else(|e| panic!("{slug}.{language}: {e}"));
        let expanded: Vec<markup::Block> = blocks
            .into_iter()
            .flat_map(|block| match block {
                markup::Block::Computed { name, args } => {
                    shared::codex::tables::expand(&name, &args, language)
                        .unwrap_or_else(|e| panic!("{slug}.{language}: {name}: {e}"))
                }
                block => vec![block],
            })
            .collect();
        json!(expanded)
    };
    let guides: Vec<Value> = shared::codex::guides::GUIDES
        .iter()
        .map(|&(slug, _, fr)| {
            json!({
                "slug": slug,
                "en": guide(slug, "en"),
                "fr": (!fr.trim().is_empty()).then(|| guide(slug, "fr")),
            })
        })
        .collect();

    let game = json!({
        "ticksPerSecond": TICKS_PER_SECOND,
        "cookSeconds": seconds(COOK_TICKS as u32),
        "distillSeconds": seconds(alchemy::DISTILL_TICKS as u32),
        "alchemyLonger": format!("{:?}", alchemy::LONGER),
        "alchemyStronger": format!("{:?}", alchemy::STRONGER),
        "items": items,
        "blocks": blocks,
        "recipes": recipes,
        "tags": tags,
        "alchemy": alchemy,
        "mobs": mobs,
        "biomes": biomes,
        "biomeNotes": biome_notes,
        "enchantments": enchantments,
        "effects": effects,
        "achievements": achievements,
        "fishing": fishing,
        "guides": guides,
    });
    let mut game = game;
    name_links(&mut game, "en");
    std::fs::write(&out, serde_json::to_string_pretty(&game).unwrap() + "\n")
        .unwrap_or_else(|e| panic!("cannot write {out}: {e}"));
    println!("wrote {out}");
}

/// Gives the links to a biome or a guide without words of their own the name the game shows,
/// in `language`: the wiki has no names of its own for them. The French texts of the guides
/// get the French names.
fn name_links(value: &mut Value, language: &str) {
    match value {
        Value::Object(map) => {
            let is_span = map.contains_key("link") && !map.contains_key("type");
            if is_span && map.get("text").is_none_or(Value::is_null) {
                if let Some(name) = map.get("link").and_then(|link| link_name(link, language)) {
                    map.insert("text".into(), Value::String(name));
                }
            }
            for (key, child) in map.iter_mut() {
                let language = if key == "fr" { "fr" } else { language };
                name_links(child, language);
            }
        }
        Value::Array(items) => items.iter_mut().for_each(|item| name_links(item, language)),
        _ => {}
    }
}

/// The name of a biome or a guide a link opens, in `language`.
fn link_name(link: &Value, language: &str) -> Option<String> {
    let id = link.get("id")?.as_str()?;
    match link.get("kind")?.as_str()? {
        "biome" => {
            shared::codex::tables::lang_text(language, &format!("biome.{id}")).map(str::to_string)
        }
        "guide" => {
            let source = shared::codex::guides::source(id, language)?;
            let title = source.lines().next()?.strip_prefix("# ")?;
            let line = markup::inline(title).ok()?;
            Some(markup::plain(&line, |target| format!("{target:?}")))
        }
        _ => None,
    }
}

/// A speed or a distance of the game, an `f32`, as it is written in its code: `2.8`, not
/// `2.799999952316284`.
fn tenths(v: f32) -> f64 {
    (f64::from(v) * 100.0).round() / 100.0
}

fn ingredient(ingredient: Ingredient) -> String {
    match ingredient {
        Ingredient::Item(item) => format!("{item:?}"),
        Ingredient::Tag(tag) => format!("#{tag:?}"),
    }
}

fn category(category: ItemCategory) -> &'static str {
    category.name()
}

/// The texts of a language file: a map of keys to texts, in RON.
fn lang(path: &Path) -> BTreeMap<String, String> {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    ron::de::from_str(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}
