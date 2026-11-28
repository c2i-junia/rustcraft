//! The tables of the guide `structures`.

use super::{item, table as grid, words, Table};
use crate::codex::markup::{inline, Block, Line, Span, Target};
use crate::world::structure_loot::{self, LootEntry};
use crate::world::{Biomes, VillageStyle};

/// The styles of the villages, in the order of the guide.
const STYLES: [VillageStyle; 4] = [
    VillageStyle::Plains,
    VillageStyle::Savanna,
    VillageStyle::Desert,
    VillageStyle::Snowy,
];

pub fn table(t: &Table) -> Result<Vec<Block>, String> {
    let loot = match t.name {
        "styles" => return styles(t),
        "forge" => structure_loot::FORGE,
        "granary" => structure_loot::GRANARY,
        "house" => structure_loot::HOUSE,
        "ruin" => structure_loot::RUIN,
        "crypt" => structure_loot::CRYPT,
        "sanctuary" => structure_loot::SANCTUARY,
        "shipwreck" => structure_loot::SHIPWRECK,
        "treasure" => structure_loot::BURIED_TREASURE,
        _ => return t.unknown(),
    };
    loot_table(t, loot)
}

/// Each village style, the biomes built in it and what its houses are made of. Args: the 3
/// headings, then the name and the materials of each style: plains, savanna, desert, snowy.
fn styles(t: &Table) -> Result<Vec<Block>, String> {
    let biomes = Biomes::vanilla();
    let rows = STYLES
        .iter()
        .enumerate()
        .map(|(i, &style)| {
            let mut built: Line = Vec::new();
            for def in biomes.defs().iter().filter(|d| d.village == Some(style)) {
                if !built.is_empty() {
                    built.extend(words(", "));
                }
                let key = def.key.trim_start_matches("rustcraft:").to_string();
                built.push(Span::link(Target::Biome(key)));
            }
            Ok(vec![
                inline(t.arg(3 + 2 * i, "the name of a style")?)?,
                built,
                inline(t.arg(4 + 2 * i, "the materials of a style")?)?,
            ])
        })
        .collect::<Result<_, String>>()?;
    Ok(vec![grid(t.head(3)?, &[false; 3], rows)])
}

/// What a container may hold: each item with its chance and its count. Args: the 3 headings.
fn loot_table(t: &Table, loot: &[LootEntry]) -> Result<Vec<Block>, String> {
    let rows = loot
        .iter()
        .map(|&(id, chance, fewest, most)| {
            let count = if fewest == most {
                fewest.to_string()
            } else {
                format!("{fewest}-{most}")
            };
            vec![
                item(id, 1),
                words(t.percent(f64::from(chance))),
                words(count),
            ]
        })
        .collect();
    Ok(vec![grid(t.head(3)?, &[false, true, true], rows)])
}
