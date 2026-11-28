//! The tables of the guide `combat`.

use super::{item, mob, table as grid, words, Table};
use crate::codex::markup::{Block, Line, Span, Target};
use crate::players::armour::{Armour, ArmourMaterial, ArmourPiece, ABSORBED_PER_POINT};
use crate::players::bow::BowShot;
use crate::players::melee::WeaponClass;
use crate::world::hostiles::Difficulty;
use crate::world::{ItemId, ItemStack, ItemType, MobKind};

/// The weapons the guide compares: every sword and axe, a pickaxe for the other tools, and the
/// weapons of their own.
const MELEE: [ItemId; 16] = [
    ItemId::WoodenSword,
    ItemId::GoldenSword,
    ItemId::StoneSword,
    ItemId::BronzeSword,
    ItemId::IronSword,
    ItemId::DiamondSword,
    ItemId::WoodenAxe,
    ItemId::GoldenAxe,
    ItemId::StoneAxe,
    ItemId::BronzeAxe,
    ItemId::IronAxe,
    ItemId::DiamondAxe,
    ItemId::IronPickaxe,
    ItemId::Spear,
    ItemId::ThrowingAxe,
    ItemId::Mace,
];

/// The materials of the suits, from the softest; the guide names them in this order.
const MATERIALS: [ArmourMaterial; 4] = [
    ArmourMaterial::Leather,
    ArmourMaterial::Bronze,
    ArmourMaterial::Iron,
    ArmourMaterial::Diamond,
];

/// The draws of a bow the guide shows, in ms: from the shortest that shoots to a full one.
const DRAWS: [u32; 5] = [150, 250, 500, 750, 1000];

pub fn table(t: &Table) -> Result<Vec<Block>, String> {
    match t.name {
        "melee" => melee(t),
        "armour" => armour(t),
        "durability" => durability(t),
        "arrows" => arrows(t),
        "strikes" => strikes(t),
        "experience" => experience(t),
        _ => t.unknown(),
    }
}

/// `| Weapon | Damage | Recharge | Per second | Wear | Durability | No wear | Never wears |`.
fn melee(t: &Table) -> Result<Vec<Block>, String> {
    let no_wear = t.arg(6, "the wear of a weapon that does not wear")?;
    let never = t.arg(7, "the durability of a weapon that never wears")?;
    let rows = MELEE
        .iter()
        .map(|&id| {
            let damage = id.attack_damage();
            let seconds = WeaponClass::of(Some(id)).recharge_ms() as f64 / 1000.0;
            let wear = match id.blow_wear() {
                0 => no_wear.to_string(),
                wear => wear.to_string(),
            };
            let durability = id
                .durability()
                .map_or_else(|| never.to_string(), |d| d.to_string());
            vec![
                item(id, 1),
                words(damage.to_string()),
                words(t.duration(seconds)),
                words(t.number(damage as f64 / seconds, 1)),
                words(wear),
                words(durability),
            ]
        })
        .collect();
    Ok(vec![grid(
        t.head(6)?,
        &[false, true, true, true, true, true],
        rows,
    )])
}

/// The armour item of `material` for `piece`.
fn piece(material: ArmourMaterial, piece: ArmourPiece) -> Result<ItemId, String> {
    ItemId::all()
        .find(|id| id.armour() == Some(Armour { piece, material }))
        .ok_or_else(|| format!("no {material:?} {piece:?}"))
}

/// The names of the materials, given after the `headings` headings.
fn material_names<'a>(t: &'a Table, headings: usize) -> Result<Vec<&'a str>, String> {
    (0..MATERIALS.len())
        .map(|i| t.arg(headings + i, "the name of a material"))
        .collect()
}

/// `| Material | Helmet | Chestplate | Leggings | Boots | Full suit | Absorbed |`, then the
/// names of the materials: the points of each piece, linked to it, and what a full suit absorbs.
fn armour(t: &Table) -> Result<Vec<Block>, String> {
    let names = material_names(t, 7)?;
    let mut rows = Vec::new();
    for (&material, name) in MATERIALS.iter().zip(names) {
        let mut row = vec![words(name)];
        let mut total = 0;
        for p in ArmourPiece::ALL {
            let points = Armour { piece: p, material }.points();
            total += points;
            let cell: Line = vec![Span {
                text: Some(points.to_string()),
                ..Span::link(Target::Item(piece(material, p)?))
            }];
            row.push(cell);
        }
        row.push(words(total.to_string()));
        row.push(words(
            t.percent((total * ABSORBED_PER_POINT) as f64 / 100.0),
        ));
        rows.push(row);
    }
    let numeric = [false, true, true, true, true, true, true];
    Ok(vec![grid(t.head(7)?, &numeric, rows)])
}

/// `| Material | Helmet | Chestplate | Leggings | Boots |`, then the names of the materials: the
/// hits each piece absorbs before it breaks.
fn durability(t: &Table) -> Result<Vec<Block>, String> {
    let names = material_names(t, 5)?;
    let rows = MATERIALS
        .iter()
        .zip(names)
        .map(|(&material, name)| {
            let mut row = vec![words(name)];
            row.extend(
                ArmourPiece::ALL
                    .map(|piece| words(Armour { piece, material }.durability().to_string())),
            );
            row
        })
        .collect();
    Ok(vec![grid(
        t.head(5)?,
        &[false, true, true, true, true],
        rows,
    )])
}

/// `| Draw | Arrow damage |`: what an arrow of a bow without enchantments does by its draw.
fn arrows(t: &Table) -> Result<Vec<Block>, String> {
    let bow = ItemStack {
        item_id: ItemId::Bow,
        item_type: ItemType::Generic,
        nb: 1,
    };
    let rows = DRAWS
        .iter()
        .map(|&ms| {
            let shot = BowShot::new(&bow, ms, false);
            vec![
                words(t.duration(ms as f64 / 1000.0)),
                words(shot.damage.to_string()),
            ]
        })
        .collect();
    Ok(vec![grid(t.head(2)?, &[true, true], rows)])
}

/// `| Creature | Easy | Normal | Hard |`: how hard each hostile strikes at the surface, the
/// Ember Mother last.
fn strikes(t: &Table) -> Result<Vec<Block>, String> {
    let mut kinds: Vec<_> = MobKind::ALL
        .into_iter()
        .filter_map(|kind| Some((kind, kind.strike()?.damage)))
        .collect();
    kinds.sort_by_key(|&(kind, _)| kind == MobKind::EmberMother);
    let rows = kinds
        .into_iter()
        .map(|(kind, damage)| {
            let mut row = vec![mob(kind)];
            row.extend(
                [Difficulty::Easy, Difficulty::Normal, Difficulty::Hard]
                    .map(|d| words(d.strike_damage(damage).to_string())),
            );
            row
        })
        .collect();
    Ok(vec![grid(t.head(4)?, &[false, true, true, true], rows)])
}

/// `| Creature | Experience |`: the orb of each creature that leaves one, the smallest first.
fn experience(t: &Table) -> Result<Vec<Block>, String> {
    let mut kinds: Vec<_> = MobKind::ALL
        .into_iter()
        .filter(|kind| kind.experience() > 0)
        .collect();
    kinds.sort_by_key(|kind| kind.experience());
    let rows = kinds
        .into_iter()
        .map(|kind| vec![mob(kind), words(kind.experience().to_string())])
        .collect();
    Ok(vec![grid(t.head(2)?, &[false, true], rows)])
}
