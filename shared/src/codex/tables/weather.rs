//! The tables of the guide `weather`.

use super::Table;
use crate::codex::markup::Block;
use crate::world::{Biomes, Precipitation};

pub fn table(t: &Table) -> Result<Vec<Block>, String> {
    match t.name {
        "precipitation" => precipitation(t),
        _ => t.unknown(),
    }
}

/// What falls where while it rains, as a list: the biomes where it snows, those where nothing
/// falls, and the rest. Args: the item of the snowy biomes and that of the dry ones (`{biomes}`),
/// the item of everywhere else, then the word that joins the last two biomes of a list.
fn precipitation(t: &Table) -> Result<Vec<Block>, String> {
    let and = t.arg(3, "the word and")?;
    let biomes = |kind: Precipitation| -> String {
        let links: Vec<String> = Biomes::vanilla()
            .defs()
            .iter()
            .filter(|def| def.look.precipitation == kind)
            .map(|def| format!("[[biome:{}]]", def.key.trim_start_matches("rustcraft:")))
            .collect();
        match links.split_last() {
            Some((last, [])) => last.clone(),
            Some((last, rest)) => format!("{} {and} {last}", rest.join(", ")),
            None => String::new(),
        }
    };
    let items = vec![
        t.fill(
            t.arg(0, "the snowy biomes")?,
            &[("biomes", biomes(Precipitation::Snow))],
        )?,
        t.fill(
            t.arg(1, "the dry biomes")?,
            &[("biomes", biomes(Precipitation::None))],
        )?,
        t.fill(t.arg(2, "everywhere else")?, &[])?,
    ];
    Ok(vec![Block::List {
        ordered: false,
        items,
    }])
}
