//! The tables of the guide `farming`.

use super::{item, words, Table};
use crate::codex::markup::Block;
use crate::constants::{CHUNK_SIZE, TICKS_PER_SECOND};
use crate::world::growth::{next_stage, CROP_ODDS};
use crate::world::ticks::RANDOM_TICKS_PER_SECTION;
use crate::world::{BlockId, ItemId, ItemType};

/// The crops, by their seed.
const CROPS: [ItemId; 3] = [ItemId::Rye, ItemId::Carrot, ItemId::Potato];

/// The seconds between two random ticks of a cell, on average: a few cells of each chunk get
/// one every tick.
fn random_tick_seconds() -> f64 {
    let cells = CHUNK_SIZE.pow(3) as f64;
    cells / RANDOM_TICKS_PER_SECTION as f64 / TICKS_PER_SECOND as f64
}

/// The minutes a crop takes to pass a stage on average, not watered.
fn stage_minutes() -> f64 {
    f64::from(CROP_ODDS) * random_tick_seconds() / 60.0
}

/// The stages a crop planted from `seed` grows through, and the block it is once ripe.
fn growth(seed: ItemId) -> Result<(u32, BlockId), String> {
    let ItemType::Block(mut block) = seed.get_default_type() else {
        return Err(format!("{seed:?} is not planted"));
    };
    let mut stages = 0;
    while let Some(next) = next_stage(block) {
        block = next;
        stages += 1;
    }
    Ok((stages, block))
}

pub fn table(t: &Table) -> Result<Vec<Block>, String> {
    match t.name {
        // `| Tall grass drops | Chance`
        "grass" => {
            let drops = BlockId::TallGrass.get_drop_table();
            let total: u32 = drops.iter().map(|(chance, ..)| chance).sum();
            let rows = drops
                .iter()
                .map(|&(chance, id, count)| {
                    let share = f64::from(chance) / f64::from(total);
                    vec![item(id, count), words(t.percent(share))]
                })
                .collect();
            Ok(vec![super::table(t.head(2)?, &[false, true], rows)])
        }
        // `| Crop | Stages | Average time | Watered | Ripe harvest | {minutes} min`
        "crops" => {
            let minutes = t.arg(5, "the time, with {minutes}")?;
            let rows = CROPS
                .iter()
                .map(|&seed| {
                    let (stages, ripe) = growth(seed)?;
                    let counts = ripe.get_drop_table().into_iter().map(|(_, _, n)| n);
                    let (min, max) =
                        counts.fold((u32::MAX, 0), |(lo, hi), n| (lo.min(n), hi.max(n)));
                    let harvest = if min == max {
                        min.to_string()
                    } else {
                        format!("{min}-{max}")
                    };
                    let time = f64::from(stages) * stage_minutes();
                    Ok(vec![
                        item(seed, 1),
                        words(stages.to_string()),
                        t.fill(minutes, &[("minutes", t.number(time, 1))])?,
                        t.fill(minutes, &[("minutes", t.number(time / 2.0, 1))])?,
                        words(harvest),
                    ])
                })
                .collect::<Result<_, String>>()?;
            Ok(vec![super::table(
                t.head(5)?,
                &[false, true, true, true, true],
                rows,
            )])
        }
        // `| One stage takes about {minutes} minutes ...`: a paragraph.
        "stage" => {
            let text = t.arg(0, "the paragraph, with {minutes}")?;
            let minutes = t.number(stage_minutes(), 1);
            Ok(vec![Block::Paragraph {
                text: t.fill(text, &[("minutes", minutes)])?,
            }])
        }
        _ => t.unknown(),
    }
}
