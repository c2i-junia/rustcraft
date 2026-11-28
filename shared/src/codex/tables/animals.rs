//! The tables of the guide `animals`.

use super::{item, mob, words, Table};
use crate::codex::markup::{Block, Span};
use crate::players::armour::ABSORBED_PER_POINT;
use crate::world::{Coat, ItemId, MobKind};

pub fn table(t: &Table) -> Result<Vec<Block>, String> {
    match t.name {
        // `| Animal | Food | Tameable | Yes | No`
        "food" => {
            let (yes, no) = (t.arg(3, "yes")?, t.arg(4, "no")?);
            let rows = MobKind::ALL
                .iter()
                .filter(|kind| !kind.is_vehicle())
                .filter_map(|&kind| {
                    let foods: Vec<ItemId> = ItemId::all().filter(|&i| kind.eats(i)).collect();
                    (!foods.is_empty()).then(|| {
                        let mut cell = Vec::new();
                        for (n, food) in foods.into_iter().enumerate() {
                            if n > 0 {
                                cell.push(Span::text(", "));
                            }
                            cell.extend(item(food, 1));
                        }
                        let tameable = if kind.is_tameable() { yes } else { no };
                        vec![mob(kind), cell, words(tameable)]
                    })
                })
                .collect();
            Ok(vec![super::table(t.head(3)?, &[false; 3], rows)])
        }
        // `| Coat | Speed | Endurance | Bay | <its name> | Chestnut | <its name> ...`: each coat
        // by its name in the code, then by its name in the language of the guide.
        "coats" => {
            let names = &t.args[3.min(t.args.len())..];
            let rows = Coat::ALL
                .iter()
                .map(|&coat| {
                    let key = format!("{coat:?}");
                    let name = names
                        .chunks(2)
                        .find(|pair| pair[0] == key)
                        .and_then(|pair| pair.get(1))
                        .ok_or_else(|| format!("coats: the name of the coat {key} is missing"))?;
                    let (speed, endurance) = coat.base();
                    Ok(vec![
                        words(name.as_str()),
                        words(t.number(f64::from(speed), 1)),
                        words(t.number(f64::from(endurance), 1)),
                    ])
                })
                .collect::<Result<_, String>>()?;
            Ok(vec![super::table(t.head(3)?, &[false, true, true], rows)])
        }
        // `| Armour | Points | Damage taken off`
        "armour" => {
            let rows = ItemId::all()
                .filter_map(|id| {
                    let points = id.horse_armour()?;
                    let share = f64::from(points * ABSORBED_PER_POINT) / 100.0;
                    Some(vec![
                        item(id, 1),
                        words(points.to_string()),
                        words(t.percent(share)),
                    ])
                })
                .collect();
            Ok(vec![super::table(t.head(3)?, &[false, true, true], rows)])
        }
        _ => t.unknown(),
    }
}
