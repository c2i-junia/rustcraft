//! The tables of the guide `commands`.

use super::{item, words, Table};
use crate::codex::markup::{Block, Span, Target};
use crate::world::{Biomes, ItemId, BASE_NAMESPACE};

/// The English name of `id`, as the players see it, if it has one.
fn english_name(id: ItemId) -> Option<&'static str> {
    super::lang_text("en", &format!("item.{id:?}")).filter(|name| !name.is_empty())
}

pub fn table(t: &Table) -> Result<Vec<Block>, String> {
    match t.name {
        // `| Item | Command name`: the items `/give` names otherwise than their English name in
        // lower case with underscores.
        "renamed" => {
            let rows = ItemId::all()
                .filter_map(|id| {
                    let shown = english_name(id)?.to_lowercase().replace(' ', "_");
                    (id.name() != shown).then(|| vec![item(id, 1), words(id.name())])
                })
                .collect();
            Ok(vec![super::table(t.head(2)?, &[false, false], rows)])
        }
        // `| Biome | Key`: what `/locate biome` takes.
        "biomes" => {
            let rows = Biomes::vanilla()
                .defs()
                .iter()
                .map(|def| {
                    let key = def.key.strip_prefix(BASE_NAMESPACE).unwrap_or(&def.key);
                    vec![vec![Span::link(Target::Biome(key.to_string()))], words(key)]
                })
                .collect();
            Ok(vec![super::table(t.head(2)?, &[false, false], rows)])
        }
        _ => t.unknown(),
    }
}
