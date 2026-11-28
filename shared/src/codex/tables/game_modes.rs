//! The tables of the guide `game-modes`.

use super::{mob, words, Table};
use crate::codex::markup::Block;
use crate::world::hostiles::Difficulty;
use crate::world::MobKind;

/// The hostiles whose blows show what each difficulty makes of them: a weak one to the boss.
const SHOWN: [MobKind; 4] = [
    MobKind::Gnawer,
    MobKind::Bramble,
    MobKind::Roller,
    MobKind::EmberMother,
];

pub fn table(t: &Table) -> Result<Vec<Block>, String> {
    match t.name {
        // `Creature | Easy | Normal | Hard`: a blow of each hostile, in points of health
        "blows" => {
            let difficulties = [Difficulty::Easy, Difficulty::Normal, Difficulty::Hard];
            let rows = SHOWN
                .iter()
                .map(|&kind| {
                    let damage = kind.strike().map_or(0, |s| s.damage);
                    let mut row = vec![mob(kind)];
                    row.extend(
                        difficulties
                            .iter()
                            .map(|d| words(d.strike_damage(damage).to_string())),
                    );
                    row
                })
                .collect();
            Ok(vec![super::table(
                t.head(4)?,
                &[false, true, true, true],
                rows,
            )])
        }
        _ => t.unknown(),
    }
}
