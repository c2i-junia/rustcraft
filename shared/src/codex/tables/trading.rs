//! The tables of the guide `trading`.

use super::{item, mob, words, Table};
use crate::codex::markup::{Block, Line};
use crate::world::trade::{offers, Goods};
use crate::world::MobKind;

pub fn table(t: &Table) -> Result<Vec<Block>, String> {
    match t.name {
        // `You give | You get | Per day | <the goods of the directions>`: a heading and the
        // offers of each profession
        "offers" => {
            let head = t.head(3)?;
            let directions = t.arg(3, "the words for the directions to a dungeon")?;
            let mut blocks = Vec::new();
            for kind in MobKind::ALL.into_iter().filter(|&k| !offers(k).is_empty()) {
                blocks.push(Block::Heading {
                    level: 3,
                    text: mob(kind),
                });
                let rows = offers(kind)
                    .iter()
                    .map(|offer| {
                        let goods: Line = match offer.goods {
                            Goods::Items(id, count) => item(id, count),
                            Goods::Directions => words(directions),
                        };
                        vec![
                            item(offer.cost.0, offer.cost.1),
                            goods,
                            words(offer.stock.to_string()),
                        ]
                    })
                    .collect();
                blocks.push(super::table(head.clone(), &[false, false, true], rows));
            }
            Ok(blocks)
        }
        _ => t.unknown(),
    }
}
