//! The tables of the guide `ember-mother`.

use super::{words, Table};
use crate::codex::markup::{inline, Block};
use crate::world::lair::{HEALTH_PER_PLAYER, MOST_CALLED};
use crate::world::MobKind;

/// The players the table of her health goes up to: the last row stands for as many or more.
const MOST_PLAYERS: u32 = 5;

/// The values the words of her tables may name: her health alone, and what each player more
/// adds.
fn values() -> [(&'static str, String); 2] {
    [
        ("health", MobKind::EmberMother.health().to_string()),
        ("extra", HEALTH_PER_PLAYER.to_string()),
    ]
}

pub fn table(t: &Table) -> Result<Vec<Block>, String> {
    match t.name {
        // `| Health | {health}, +{extra} per extra player | Blow | 7, every 1.5 s ...`: her card,
        // each of its lines a name then its value.
        "facts" => {
            let rows = t
                .args
                .chunks(2)
                .map(|pair| {
                    let [name, value] = pair else {
                        return Err("facts: a name has no value".to_string());
                    };
                    Ok(vec![inline(name)?, t.fill(value, &values())?])
                })
                .collect::<Result<_, String>>()?;
            Ok(vec![Block::Table {
                head: None,
                numeric: vec![false, false],
                rows,
            }])
        }
        // `| She has {health} health, plus {extra} for each player ...`: a paragraph.
        "health" => {
            let text = t.arg(0, "the paragraph, with {health} and {extra}")?;
            Ok(vec![Block::Paragraph {
                text: t.fill(text, &values())?,
            }])
        }
        // `| Players | Health | Cinderlings at once | {players} or more | {health} +{extra} each`
        "scaling" => {
            let (more, each) = (
                t.arg(3, "{players} or more")?,
                t.arg(4, "{health} +{extra} each")?,
            );
            let base = MobKind::EmberMother.health();
            let rows = (1..=MOST_PLAYERS)
                .map(|players| {
                    let health = base + HEALTH_PER_PLAYER * (players - 1);
                    let called = (1 + players as usize).min(MOST_CALLED);
                    let called = words(called.to_string());
                    Ok(if players == MOST_PLAYERS {
                        vec![
                            t.fill(more, &[("players", players.to_string())])?,
                            t.fill(each, &[("health", health.to_string()), values()[1].clone()])?,
                            called,
                        ]
                    } else {
                        vec![
                            words(players.to_string()),
                            words(health.to_string()),
                            called,
                        ]
                    })
                })
                .collect::<Result<_, String>>()?;
            Ok(vec![super::table(t.head(3)?, &[true; 3], rows)])
        }
        _ => t.unknown(),
    }
}
