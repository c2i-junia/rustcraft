//! The tables of the guide `depths`.

use super::{mob, table as grid, Table};
use crate::codex::markup::Block;
use crate::world::depths::{strengthened, DepthTier};
use crate::world::MobKind;

/// The hostiles that appear in the caves, in the order of the guide.
const CAVE_HOSTILES: [MobKind; 5] = [
    MobKind::Gnawer,
    MobKind::Roller,
    MobKind::Cinderling,
    MobKind::Shardling,
    MobKind::Slagcrawler,
];

pub fn table(t: &Table) -> Result<Vec<Block>, String> {
    match t.name {
        "hostiles" => hostiles(t),
        _ => t.unknown(),
    }
}

/// The health, the damage of a blow at Normal difficulty and the experience of each hostile of
/// the caves, in each tier. Args: the heading of the hostiles, that of each tier, then the
/// template of a cell (`{health}`, `{damage}`, `{xp}`).
fn hostiles(t: &Table) -> Result<Vec<Block>, String> {
    let cell = t.arg(4, "the template of a cell")?;
    let rows = CAVE_HOSTILES
        .iter()
        .map(|&kind| {
            let damage = kind.strike().map_or(0, |s| s.damage);
            let mut row = vec![mob(kind)];
            for tier in DepthTier::ALL {
                let tier = Some(tier);
                row.push(t.fill(
                    cell,
                    &[
                        ("health", strengthened(kind.health(), tier).to_string()),
                        ("damage", strengthened(damage, tier).to_string()),
                        ("xp", strengthened(kind.experience(), tier).to_string()),
                    ],
                )?);
            }
            Ok(row)
        })
        .collect::<Result<_, String>>()?;
    Ok(vec![grid(t.head(4)?, &[false, true, true, true], rows)])
}
