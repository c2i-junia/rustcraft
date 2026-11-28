//! The tables of the guide `fishing`.

use super::{item, table as grid, words, Table};
use crate::codex::markup::Block;
use crate::world::fishing::CatchKind;

pub fn table(t: &Table) -> Result<Vec<Block>, String> {
    match t.name {
        "catches" => catches(t),
        _ => t.unknown(),
    }
}

/// For each kind of catch, a heading with its share, then the odds of each of its catches by
/// day and by night. Args: the 3 headings, the word for a catch that never bites, the heading
/// template (`{name}`, `{share}`), then the name of each kind: fish, junk, treasure.
fn catches(t: &Table) -> Result<Vec<Block>, String> {
    let never = t.arg(3, "never")?;
    let title = t.arg(4, "the heading of a kind")?;
    let mut blocks = Vec::new();
    for (i, kind) in CatchKind::ALL.into_iter().enumerate() {
        let share = kind.percent() as f64 / 100.0;
        let pool = kind.pool();
        let total = |night: bool| -> f64 {
            pool.iter()
                .filter(|c| night || !c.night_only)
                .map(|c| c.weight as f64)
                .sum()
        };
        let (day, night) = (total(false), total(true));
        blocks.push(Block::Heading {
            level: 3,
            text: t.fill(
                title,
                &[
                    ("name", t.arg(5 + i, "the name of a kind")?.to_string()),
                    ("share", t.percent(share)),
                ],
            )?,
        });
        let rows = pool
            .iter()
            .map(|c| {
                let odds = |total: f64| t.percent(share * c.weight as f64 / total);
                vec![
                    item(c.item, c.count),
                    words(if c.night_only {
                        never.to_string()
                    } else {
                        odds(day)
                    }),
                    words(odds(night)),
                ]
            })
            .collect();
        blocks.push(grid(t.head(3)?, &[false, true, true], rows));
    }
    Ok(blocks)
}
