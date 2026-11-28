//! The tables of the guide `health`.

use super::{table as grid, words, Table};
use crate::codex::markup::Block;
use crate::players::game_mode::{fall_damage, MAX_HEALTH};

/// The heights of the falls the guide shows, from harmless to deadly.
const FALLS: [u32; 10] = [3, 4, 5, 8, 10, 13, 15, 18, 20, 23];

pub fn table(t: &Table) -> Result<Vec<Block>, String> {
    match t.name {
        "falls" => falls(t),
        _ => t.unknown(),
    }
}

/// What each fall takes, in points and hearts: `| Fall | Damage | Hearts | All of them |`.
fn falls(t: &Table) -> Result<Vec<Block>, String> {
    let all = t.arg(3, "the hearts of a deadly fall")?;
    let rows = FALLS
        .iter()
        .map(|&height| {
            let damage = fall_damage(height as f32);
            let hearts = if damage >= MAX_HEALTH {
                all.to_string()
            } else {
                t.number(damage as f64 / 2.0, 1)
            };
            vec![
                words(height.to_string()),
                words(damage.to_string()),
                words(hearts),
            ]
        })
        .collect();
    Ok(vec![grid(t.head(3)?, &[true, true, true], rows)])
}
