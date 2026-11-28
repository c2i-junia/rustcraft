//! The tables of the guide `progression`.

use super::{item, words, Table};
use crate::codex::markup::{Block, Line, Target};
use crate::players::armour::{Armour, ArmourMaterial, ArmourPiece, ABSORBED_PER_POINT};
use crate::players::progress::Achievement;
use crate::world::{recipes_for, Ingredient, ItemId, ItemTag, ToolKind, ToolTier};

/// The tool tiers from the first to the last one reached, gold, the odd one, at the end.
const TIERS: [ToolTier; 6] = [
    ToolTier::Wood,
    ToolTier::Stone,
    ToolTier::Bronze,
    ToolTier::Iron,
    ToolTier::Diamond,
    ToolTier::Gold,
];

const MATERIALS: [ArmourMaterial; 4] = [
    ArmourMaterial::Leather,
    ArmourMaterial::Bronze,
    ArmourMaterial::Iron,
    ArmourMaterial::Diamond,
];

pub fn table(t: &Table) -> Result<Vec<Block>, String> {
    match t.name {
        // `Tier | Made of | Pickaxe mines | Speed | Uses | Sword damage`, then what a pickaxe of
        // each mining level mines, 0 to 3, then the words for any planks and any cobblestone
        "tools" => {
            let rows = TIERS
                .iter()
                .map(|&tier| {
                    let pickaxe = tool(tier, ToolKind::Pickaxe)?;
                    let sword = tool(tier, ToolKind::Sword)?;
                    let mines = t.arg(6 + tier.mining_level() as usize, "what a level mines")?;
                    Ok(vec![
                        item(pickaxe, 1),
                        material(t, pickaxe)?,
                        t.fill(mines, &[])?,
                        words(format!("x{}", tier.speed())),
                        words(tier.durability().to_string()),
                        words(sword.attack_damage().to_string()),
                    ])
                })
                .collect::<Result<_, String>>()?;
            let numeric = [false, false, false, true, true, true];
            Ok(vec![super::table(t.head(6)?, &numeric, rows)])
        }
        // `Material | Helmet | Chestplate | Leggings | Boots | Full suit | Blows absorbed`, the
        // names of leather, bronze, iron and diamond, then the cell of a piece of one point and
        // of more, `{points}` and `{durability}`
        "armour" => {
            let rows = MATERIALS
                .iter()
                .enumerate()
                .map(|(i, &material)| {
                    let mut row = vec![words(t.arg(7 + i, "a material")?)];
                    let mut points = 0;
                    for piece in ArmourPiece::ALL {
                        let armour = Armour { piece, material };
                        let id = ItemId::ALL
                            .into_iter()
                            .find(|id| id.armour() == Some(armour))
                            .ok_or_else(|| format!("no {material:?} {piece:?}"))?;
                        points += armour.points();
                        let template = t.arg(
                            if armour.points() > 1 { 12 } else { 11 },
                            "the cell of a piece",
                        )?;
                        let mut cell = t.fill(
                            template,
                            &[
                                ("points", armour.points().to_string()),
                                ("durability", armour.durability().to_string()),
                            ],
                        )?;
                        for span in &mut cell {
                            span.link = Some(Target::Item(id));
                        }
                        row.push(cell);
                    }
                    row.push(words(points.to_string()));
                    row.push(words(
                        t.percent((points * ABSORBED_PER_POINT) as f64 / 100.0),
                    ));
                    Ok(row)
                })
                .collect::<Result<_, String>>()?;
            let numeric = [false, false, false, false, false, true, true];
            Ok(vec![super::table(t.head(7)?, &numeric, rows)])
        }
        // `<an achievement, {title} and {goal}>`: every achievement, in the order of the game
        "achievements" => {
            let template = t.arg(0, "an achievement")?;
            let items = Achievement::ALL
                .iter()
                .map(|a| {
                    let id = format!("{a:?}");
                    t.fill(
                        template,
                        &[
                            (
                                "title",
                                text(t.language, &format!("achievement.{id}.title"))?,
                            ),
                            ("goal", text(t.language, &format!("achievement.{id}.goal"))?),
                        ],
                    )
                })
                .collect::<Result<_, String>>()?;
            Ok(vec![Block::List {
                ordered: true,
                items,
            }])
        }
        _ => t.unknown(),
    }
}

/// The tool of `kind` made of `tier`.
fn tool(tier: ToolTier, kind: ToolKind) -> Result<ItemId, String> {
    ItemId::ALL
        .into_iter()
        .find(|id| {
            id.tool()
                .is_some_and(|tool| tool.tier == tier && tool.kind == kind)
        })
        .ok_or_else(|| format!("no {kind:?} of {tier:?}"))
}

/// What `pickaxe` is made of: the ingredient of its recipe that is not a stick, a family of
/// items named by the args.
fn material(t: &Table, pickaxe: ItemId) -> Result<Line, String> {
    let recipe = recipes_for(pickaxe)
        .next()
        .ok_or_else(|| format!("no recipe of {pickaxe:?}"))?;
    let ingredient = recipe
        .ingredients()
        .into_iter()
        .map(|(ingredient, _)| ingredient)
        .find(|&i| i != Ingredient::Item(ItemId::Stick))
        .ok_or_else(|| format!("{pickaxe:?} is made of sticks only"))?;
    let (first, index) = match ingredient {
        Ingredient::Item(id) => return Ok(item(id, 1)),
        Ingredient::Tag(ItemTag::Planks) => (ItemId::OakPlanks, 10),
        Ingredient::Tag(ItemTag::Cobble) => (ItemId::Cobblestone, 11),
        Ingredient::Tag(tag) => return Err(format!("no words for the tag {tag:?}")),
    };
    let mut line = words(t.arg(index, "the words of a family of items")?);
    line[0].link = Some(Target::Item(first));
    Ok(line)
}

/// The text of `key` in the game's translation into `language`, English when it has none.
fn text(language: &str, key: &str) -> Result<String, String> {
    super::lang_text(language, key)
        .map(str::to_string)
        .ok_or_else(|| format!("no text for {key}"))
}
