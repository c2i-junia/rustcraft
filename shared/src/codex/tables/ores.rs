//! The tables of the guide `ores`.

use super::{item, table as grid, words, Table};
use crate::codex::markup::{Block, Line};
use crate::world::depths::Underground;
use crate::world::ores::{ores, Ore};
use crate::world::{smelting_result, BlockId, ItemId, ItemType};

/// The ores, in the order of the guide: the commonest first.
const ORES: [ItemId; 6] = [
    ItemId::CoalOre,
    ItemId::CopperOre,
    ItemId::TinOre,
    ItemId::IronOre,
    ItemId::GoldOre,
    ItemId::DiamondOre,
];

pub fn table(t: &Table) -> Result<Vec<Block>, String> {
    match t.name {
        "spread" => spread(t),
        "mining" => mining(t),
        _ => t.unknown(),
    }
}

/// The block an ore item places.
fn block(ore: ItemId) -> BlockId {
    match ore.get_default_type() {
        ItemType::Block(block) => block,
        _ => unreachable!("{ore:?} is a block"),
    }
}

/// How `ore` is scattered in a world of `underground`.
fn scattering(ore: ItemId, underground: Underground) -> Result<&'static Ore, String> {
    ores(underground)
        .iter()
        .find(|o| o.block == block(ore))
        .ok_or_else(|| format!("{ore:?} is not scattered"))
}

/// `1-4`, or `1` when both ends are the same.
fn range((least, most): (u64, u64)) -> String {
    if least == most {
        least.to_string()
    } else {
        format!("{least}-{most}")
    }
}

/// The heights of each ore in a new and in an old world, its tries per chunk, the sizes of its
/// clusters and veins and its share of veins. Args: the 8 headings, the template of a band of
/// heights (`{from}`, `{to}`), then what stands for no vein.
fn spread(t: &Table) -> Result<Vec<Block>, String> {
    let band = t.arg(8, "the template of a band of heights")?;
    let none = t.arg(9, "no vein")?;
    let rows = ORES
        .iter()
        .map(|&ore| {
            let deep = scattering(ore, Underground::Deep)?;
            let shallow = scattering(ore, Underground::Shallow)?;
            let heights = |o: &Ore| -> Result<Line, String> {
                t.fill(
                    band,
                    &[
                        ("from", o.lowest.to_string()),
                        ("to", o.highest.to_string()),
                    ],
                )
            };
            Ok(vec![
                item(ore, 1),
                heights(deep)?,
                words(deep.attempts.to_string()),
                heights(shallow)?,
                words(shallow.attempts.to_string()),
                words(range(deep.cluster)),
                words(if deep.veins > 0.0 {
                    range(deep.vein)
                } else {
                    none.to_string()
                }),
                words(t.percent(f64::from(deep.veins))),
            ])
        })
        .collect::<Result<_, String>>()?;
    let numeric = [false, false, true, false, true, true, true, true];
    Ok(vec![grid(t.head(8)?, &numeric, rows)])
}

/// The pickaxe each ore needs, what it drops and what that smelts into. Args: the 4 headings,
/// the word for any tool, the name of each mining level (wood or gold, stone or bronze, iron,
/// diamond), then what stands for nothing.
fn mining(t: &Table) -> Result<Vec<Block>, String> {
    let none = t.arg(9, "nothing")?;
    let rows = ORES
        .iter()
        .map(|&ore| {
            let block = block(ore);
            let pickaxe = match block.get_required_tool() {
                None => t.arg(4, "any tool")?,
                Some(tool) => t.arg(5 + tool.level as usize, "a mining level")?,
            };
            let (_, drop, _) = block
                .get_drop_table()
                .into_iter()
                .next()
                .ok_or_else(|| format!("{ore:?} drops nothing"))?;
            Ok(vec![
                item(ore, 1),
                words(pickaxe),
                item(drop, 1),
                smelting_result(drop).map_or_else(|| words(none), |smelted| item(smelted, 1)),
            ])
        })
        .collect::<Result<_, String>>()?;
    Ok(vec![grid(t.head(4)?, &[false; 4], rows)])
}
