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
use shared::players::effects::EffectKind;
use shared::world::alchemy;
use shared::world::trade::{offers, Goods};
use shared::world::{
    fuel_ticks, recipes, repair_material, smelting_result, BlockId, Biomes, Enchantment, Ingredient,
    ItemCategory, ItemId, ItemType, MobKind, ToolKind, COOK_TICKS, MAX_ENCHANTMENT_LEVEL,
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
    let tr = |key: String| -> Value {
        json!({ "en": en.get(&key), "fr": fr.get(&key) })
    };
    let seconds = |ticks: u32| ticks as f64 / TICKS_PER_SECOND as f64;

    let items: Vec<Value> = ItemId::ALL
        .iter()
        .map(|&id| {
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
                "foodEffect": id.food_effect().map(|(kind, level, s)| json!({ "effect": format!("{kind:?}"), "level": level, "seconds": s })),
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
            })
        })
        .collect();

    let blocks: Vec<Value> = BlockId::ALL
        .iter()
        .map(|&id| {
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
            tag.name().to_string(),
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
                "eats": ItemId::ALL.iter().filter(|i| kind.eats(**i)).map(|i| format!("{i:?}")).collect::<Vec<_>>(),
                "egg": ItemId::ALL.iter().find(|i| i.spawned_mob() == Some(kind)).map(|i| format!("{i:?}")),
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

    let enchantments: Vec<Value> = Enchantment::ALL
        .iter()
        .map(|&e| {
            json!({
                "id": format!("{e:?}"),
                "name": e.name(),
                "levels": (1..=MAX_ENCHANTMENT_LEVEL).map(|l| e.effect(l)).collect::<Vec<_>>(),
                "items": ItemId::ALL.iter().filter(|i| e.fits(**i)).map(|i| format!("{i:?}")).collect::<Vec<_>>(),
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
        "enchantments": enchantments,
        "effects": effects,
        "achievements": achievements,
    });
    std::fs::write(&out, serde_json::to_string_pretty(&game).unwrap() + "\n")
        .unwrap_or_else(|e| panic!("cannot write {out}: {e}"));
    println!("wrote {out}");
}

fn ingredient(ingredient: Ingredient) -> String {
    match ingredient {
        Ingredient::Item(item) => format!("{item:?}"),
        Ingredient::Tag(tag) => format!("#{}", tag.name()),
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
