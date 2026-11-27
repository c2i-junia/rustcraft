//! What a page of the Codex says, and the pages of each section. Everything a page shows is read
//! from the tables of the game (`shared`) or from the `codex.*` texts of the languages.

use bevy::prelude::*;
use shared::codex::facts::{block_sources, mob_sources, trades_with};
use shared::codex::markup::{Block, Line, Span, Target};
use shared::players::effects::EffectKind;
use shared::world::trade::Goods;
use shared::world::{
    fuel_ticks, recipes_for, repair_material, smelting_inputs, smelting_result, uses_of, Biomes,
    Enchantment, Ingredient, ItemId, Loot, LootCount, LootWhen, MobKind,
};
use shared::TICKS_PER_SECOND;

use super::text::{blocks_of, line_of, plain, spawn_blocks, spawn_line, Look};
use super::{CodexPage, Section};
use crate::i18n::{category_name, enchantment_name, folded, tag_name, tr, trf};
use crate::ui::theme;
use crate::world::AtlasWrapper;

pub use shared::codex::guides::GUIDES;

/// The code of the language of the game, for the guides and their tables.
fn language_code() -> &'static str {
    match crate::i18n::language() {
        crate::i18n::Language::French => "fr",
        _ => "en",
    }
}

/// The text of a guide in the language of the game, English when it lacks one.
pub fn guide_source(slug: &str) -> Option<&'static str> {
    shared::codex::guides::source(slug, language_code())
}

/// The blocks of a guide.
pub fn guide_blocks(slug: &str) -> Vec<Block> {
    guide_source(slug)
        .and_then(|source| shared::codex::markup::parse(source).ok())
        .unwrap_or_default()
}

/// The title of a guide: its first heading.
pub fn guide_title(slug: &str) -> String {
    guide_blocks(slug)
        .iter()
        .find_map(|block| match block {
            Block::Heading { text, .. } => Some(plain(text)),
            _ => None,
        })
        .unwrap_or_else(|| slug.replace('-', " "))
}

/// The creatures the Codex has a page for: not the boat, which is an item, nor what is thrown.
pub fn creatures() -> impl Iterator<Item = MobKind> {
    MobKind::ALL
        .into_iter()
        .filter(|kind| !kind.is_projectile() && !kind.is_vehicle())
}

/// The keys of the biomes, without `rustcraft:`.
pub fn biome_keys() -> Vec<String> {
    Biomes::vanilla()
        .defs()
        .iter()
        .map(|def| def.key.trim_start_matches("rustcraft:").to_string())
        .collect()
}

/// The pages of a section, in their order.
pub fn entries(section: Section) -> Vec<CodexPage> {
    match section {
        Section::Items => ItemId::ALL.into_iter().map(CodexPage::Item).collect(),
        Section::Creatures => creatures().map(CodexPage::Mob).collect(),
        Section::Biomes => biome_keys().into_iter().map(CodexPage::Biome).collect(),
        Section::Effects => EffectKind::ALL.into_iter().map(CodexPage::Effect).collect(),
        Section::Guides => GUIDES
            .iter()
            .map(|(slug, ..)| CodexPage::Guide(slug.to_string()))
            .collect(),
    }
}

/// Whether the search `query` finds `page`: in its title, ignoring case and accents.
pub fn matches(query: &str, page: &CodexPage) -> bool {
    let query = folded(query.trim());
    query.is_empty() || folded(&page.title()).contains(&query)
}

/// What a page needs to be drawn.
pub struct PageContext<'a> {
    pub atlas: Option<&'a AtlasWrapper>,
    pub font: Handle<Font>,
    pub assets: &'a AssetServer,
    /// Whether the player has met a creature: one not met yet keeps its page closed.
    pub seen: &'a dyn Fn(MobKind) -> bool,
}

impl PageContext<'_> {
    fn look(&self, size: f32, color: Color) -> Look {
        Look::new(&self.font, size, color)
    }
}

fn text_span(text: impl Into<String>) -> Span {
    Span::text(text)
}

fn link(target: Target) -> Span {
    Span::link(target)
}

/// `template`, a text of the languages, with each `{name}` replaced by the runs of `args`.
pub fn fill_line(template: &str, args: &[(&str, Line)]) -> Line {
    let mut line = Vec::new();
    let mut rest = template;
    while let Some(start) = rest.find('{') {
        let Some(len) = rest[start..].find('}') else {
            break;
        };
        let name = &rest[start + 1..start + len];
        match args.iter().find(|(n, _)| *n == name) {
            Some((_, value)) => {
                if start > 0 {
                    line.push(text_span(&rest[..start]));
                }
                line.extend(value.iter().cloned());
            }
            None => line.push(text_span(&rest[..start + len + 1])),
        }
        rest = &rest[start + len + 1..];
    }
    if !rest.is_empty() {
        line.push(text_span(rest));
    }
    line
}

/// The runs of a list of things, joined by commas.
fn joined(parts: Vec<Line>) -> Line {
    let mut line = Vec::new();
    for (i, part) in parts.into_iter().enumerate() {
        if i > 0 {
            line.push(text_span(", "));
        }
        line.extend(part);
    }
    line
}

fn plain_line(text: impl Into<String>) -> Line {
    vec![text_span(text)]
}

fn number(value: f64) -> String {
    let text = if value.fract().abs() < 1e-9 {
        format!("{value:.0}")
    } else {
        format!("{value:.1}")
    };
    if crate::i18n::language() == crate::i18n::Language::French {
        text.replace('.', ",")
    } else {
        text
    }
}

fn percent(share: f64) -> String {
    format!("{} %", number((share * 100.0 * 10.0).round() / 10.0))
}

fn range(min: u32, max: u32) -> String {
    if min == max {
        min.to_string()
    } else {
        trf("codex.range", &[("min", &min), ("max", &max)])
    }
}

fn roman(level: u8) -> &'static str {
    ["", "I", "II", "III", "IV", "V"]
        .get(level as usize)
        .copied()
        .unwrap_or("?")
}

fn ingredient_link(ingredient: Ingredient) -> Line {
    match ingredient {
        Ingredient::Item(item) => vec![link(Target::Item(item))],
        Ingredient::Tag(tag) => plain_line(tag_name(tag)),
    }
}

fn counted(count: u32, what: Line) -> Line {
    let mut line = vec![text_span(format!("{count} "))];
    line.extend(what);
    line
}

/// A grid of facts: a label, and its value.
struct Facts {
    rows: Vec<(String, Line)>,
}

impl Facts {
    fn new() -> Self {
        Self { rows: Vec::new() }
    }

    fn add(&mut self, key: &str, value: Line) {
        self.rows.push((tr(key).to_string(), value));
    }

    fn spawn(self, commands: &mut Commands, parent: Entity, ctx: &PageContext) {
        if self.rows.is_empty() {
            return;
        }
        let grid = commands
            .spawn((
                Node {
                    display: Display::Grid,
                    grid_template_columns: vec![GridTrack::auto(), GridTrack::flex(1.0)],
                    column_gap: px(theme::space_m()),
                    row_gap: px(theme::space_xs()),
                    ..default()
                },
                ChildOf(parent),
            ))
            .id();
        for (label, value) in self.rows {
            commands.spawn((
                Text::new(label),
                TextFont {
                    font: ctx.font.clone().into(),
                    font_size: px(theme::font_s()).into(),
                    ..default()
                },
                TextColor(theme::muted()),
                ChildOf(grid),
            ));
            spawn_line(commands, grid, &value, &ctx.look(theme::font_s(), theme::text()));
        }
    }
}

fn heading(commands: &mut Commands, parent: Entity, key: &str, ctx: &PageContext) {
    commands.spawn((
        Text::new(tr(key)),
        TextFont {
            font: ctx.font.clone().into(),
            font_size: px(theme::font_m()).into(),
            ..default()
        },
        TextColor(theme::text()),
        Node {
            margin: UiRect::top(px(theme::space_s())),
            ..default()
        },
        ChildOf(parent),
    ));
}

fn lines(commands: &mut Commands, parent: Entity, lines: &[Line], ctx: &PageContext) {
    for line in lines {
        spawn_line(commands, parent, line, &ctx.look(theme::font_s(), theme::text()));
    }
}

/// The header of a page: its icon or portrait, its title and a line under it.
fn header(
    commands: &mut Commands,
    parent: Entity,
    page: &CodexPage,
    under: Option<String>,
    ctx: &PageContext,
) {
    let row = commands
        .spawn((
            Node {
                align_items: AlignItems::Center,
                column_gap: px(theme::space_m()),
                ..default()
            },
            ChildOf(parent),
        ))
        .id();
    if let (CodexPage::Item(item), Some(atlas)) = (page, ctx.atlas) {
        commands.spawn((
            ImageNode::from_atlas_image(
                atlas.texture.clone(),
                atlas.sprite(&format!("{item:?}")).unwrap_or_default(),
            ),
            Node {
                width: px(48),
                height: px(48),
                flex_shrink: 0.0,
                ..default()
            },
            ChildOf(row),
        ));
    }
    let titles = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                ..default()
            },
            ChildOf(row),
        ))
        .id();
    commands.spawn((
        Text::new(page.title()),
        TextFont {
            font: ctx.font.clone().into(),
            font_size: px(theme::font_l()).into(),
            ..default()
        },
        TextColor(theme::text()),
        ChildOf(titles),
    ));
    if let Some(under) = under {
        commands.spawn((
            Text::new(under),
            TextFont {
                font: ctx.font.clone().into(),
                font_size: px(theme::font_s()).into(),
                ..default()
            },
            TextColor(theme::muted()),
            ChildOf(titles),
        ));
    }
}

/// Spawns the page under `parent`, a column. `with_header`: its title too, which the item
/// browser shows itself.
pub fn spawn_page(
    commands: &mut Commands,
    parent: Entity,
    page: &CodexPage,
    with_header: bool,
    ctx: &PageContext,
) {
    match page {
        CodexPage::Item(item) => spawn_item(commands, parent, *item, with_header, ctx),
        CodexPage::Mob(kind) if !(ctx.seen)(*kind) => {
            if with_header {
                commands.spawn((
                    Text::new(tr("codex.unknown")),
                    TextFont {
                        font: ctx.font.clone().into(),
                        font_size: px(theme::font_l()).into(),
                        ..default()
                    },
                    TextColor(theme::muted()),
                    ChildOf(parent),
                ));
            }
            lines(commands, parent, &[plain_line(tr("codex.not_met"))], ctx);
        }
        CodexPage::Mob(kind) => spawn_mob(commands, parent, *kind, with_header, ctx),
        CodexPage::Biome(key) => spawn_biome(commands, parent, key, ctx),
        CodexPage::Effect(kind) => spawn_effect(commands, parent, *kind, ctx),
        CodexPage::Guide(slug) => spawn_guide_blocks(commands, parent, &guide_blocks(slug), ctx),
    }
}

/// Spawns the blocks of a guide, its recipes and the tables the game works out among them.
fn spawn_guide_blocks(
    commands: &mut Commands,
    parent: Entity,
    blocks: &[Block],
    ctx: &PageContext,
) {
    spawn_blocks(
        commands,
        parent,
        blocks,
        &ctx.look(theme::font_s(), theme::text()),
        &mut |commands, parent, block| match block {
            Block::Recipe { item } => lines(commands, parent, &recipe_lines(*item), ctx),
            Block::Computed { name, args } => {
                match shared::codex::tables::expand(name, args, language_code()) {
                    Ok(blocks) => spawn_guide_blocks(commands, parent, &blocks, ctx),
                    Err(e) => warn!("the table {name} of a guide: {e}"),
                }
            }
            _ => {}
        },
    );
}

/// The lines of the recipes making `item`.
pub fn recipe_lines(item: ItemId) -> Vec<Line> {
    recipes_for(item)
        .map(|recipe| {
            let ingredients = joined(
                recipe
                    .ingredients()
                    .into_iter()
                    .map(|(ingredient, count)| counted(count, ingredient_link(ingredient)))
                    .collect(),
            );
            let key = if recipe.fits(2) {
                "codex.line.crafted_hand"
            } else {
                "codex.line.crafted_table"
            };
            let mut line = fill_line(tr(key), &[("ingredients", ingredients)]);
            if recipe.output.nb > 1 {
                line.push(text_span(format!(" (×{})", recipe.output.nb)));
            }
            line
        })
        .collect()
}

fn loot_count(loot: &Loot) -> String {
    let count = match loot.count {
        LootCount::Range(min, max) => range(min, max),
        LootCount::Odds(n, d) => percent(f64::from(n) / f64::from(d)),
        LootCount::Chance(p) => percent(p),
    };
    match loot.when {
        LootWhen::Always => count,
        LootWhen::NotSheared => format!("{count}, {}", tr("codex.loot.not_sheared")),
        LootWhen::Saddled => format!("{count}, {}", tr("codex.loot.saddled")),
        LootWhen::Otherwise => format!("{count}, {}", tr("codex.loot.otherwise")),
    }
}

fn spawn_item(
    commands: &mut Commands,
    parent: Entity,
    item: ItemId,
    with_header: bool,
    ctx: &PageContext,
) {
    if with_header {
        let page = CodexPage::Item(item);
        header(
            commands,
            parent,
            &page,
            Some(category_name(item.category()).into()),
            ctx,
        );
    }
    let note = blocks_of(&format!("codex.item.{item:?}"));
    spawn_blocks(
        commands,
        parent,
        &note,
        &ctx.look(theme::font_s(), theme::text()),
        &mut |_, _, _| {},
    );

    let mut facts = Facts::new();
    let damage = item.attack_damage();
    if damage > 1 {
        facts.add("codex.fact.damage", plain_line(damage.to_string()));
    }
    if let Some(durability) = item.durability() {
        facts.add("codex.fact.durability", plain_line(durability.to_string()));
    }
    if let Some(tool) = item
        .tool()
        .filter(|t| t.kind != shared::world::ToolKind::Sword)
    {
        facts.add(
            "codex.fact.mining_level",
            plain_line(tool.tier.mining_level().to_string()),
        );
    }
    if let Some(armour) = item.armour() {
        facts.add("codex.fact.armour", plain_line(armour.points().to_string()));
    }
    if let Some(points) = item.horse_armour() {
        facts.add("codex.fact.horse_armour", plain_line(points.to_string()));
    }
    if let Some((hunger, saturation)) = item.food_value() {
        facts.add(
            "codex.fact.food",
            plain_line(trf(
                "codex.value.food",
                &[
                    ("hunger", &hunger),
                    ("saturation", &number(f64::from(saturation))),
                ],
            )),
        );
    }
    if let Some((effect, level, ticks)) = item.food_effect() {
        facts.add(
            "codex.fact.eaten",
            effect_line(effect, level, ticks / TICKS_PER_SECOND as u32),
        );
    }
    if let Some(potion) = item.potion() {
        facts.add(
            "codex.fact.potion",
            effect_line(potion.effect, 1, potion.seconds),
        );
    }
    if let Some(ticks) = fuel_ticks(item) {
        let seconds = f64::from(ticks) / f64::from(TICKS_PER_SECOND as u32);
        facts.add(
            "codex.fact.fuel",
            plain_line(trf("codex.value.seconds", &[("seconds", &number(seconds))])),
        );
    }
    if let Some(result) = smelting_result(item) {
        facts.add("codex.fact.smelts", vec![link(Target::Item(result))]);
    }
    if let Some(material) = repair_material(item) {
        facts.add("codex.fact.repair", ingredient_link(material));
    }
    let enchantments: Vec<Line> = Enchantment::ALL
        .iter()
        .filter(|e| e.fits(item))
        .map(|e| plain_line(enchantment_name(*e)))
        .collect();
    if !enchantments.is_empty() {
        facts.add("codex.fact.enchantments", joined(enchantments));
    }
    if let shared::world::ItemType::Block(block) = item.get_default_type() {
        let light = block.light_emission();
        if light > 0 {
            facts.add("codex.fact.light", plain_line(light.to_string()));
        }
    }
    if let Some(kind) = item.spawned_mob() {
        facts.add("codex.fact.spawns", vec![link(Target::Mob(kind))]);
    }
    facts.add(
        "codex.fact.stack",
        plain_line(item.get_max_stack().to_string()),
    );
    facts.spawn(commands, parent, ctx);

    // Where it comes from
    let mut sources = recipe_lines(item);
    let inputs: Vec<Line> = smelting_inputs(item)
        .map(|i| vec![link(Target::Item(i))])
        .collect();
    if !inputs.is_empty() {
        sources.push(fill_line(
            tr("codex.line.smelted"),
            &[("items", joined(inputs))],
        ));
    }
    for source in block_sources(item) {
        // a form of a block of its own, like a lit furnace, has no name to show
        let Some(block_item) = itemfor(source.block) else {
            continue;
        };
        let what = vec![link(Target::Item(block_item))];
        let mut count = range(source.count, source.count);
        if source.chance < 1.0 {
            count = format!("{count}, {}", percent(source.chance));
        }
        sources.push(fill_line(
            tr("codex.line.broken"),
            &[("block", what), ("count", plain_line(count))],
        ));
    }
    for (kind, loot) in mob_sources(item) {
        sources.push(fill_line(
            tr("codex.line.dropped"),
            &[
                ("mob", vec![link(Target::Mob(kind))]),
                ("count", plain_line(loot_count(&loot))),
            ],
        ));
    }
    let mut uses = Vec::new();
    for (kind, offer) in trades_with(item) {
        let price = counted(offer.cost.1, vec![link(Target::Item(offer.cost.0))]);
        match offer.goods {
            Goods::Items(goods, count) if goods == item => sources.push(fill_line(
                tr("codex.line.sells"),
                &[
                    ("mob", vec![link(Target::Mob(kind))]),
                    ("count", plain_line(count.to_string())),
                    ("price", price),
                ],
            )),
            Goods::Items(goods, count) => uses.push(fill_line(
                tr("codex.line.buys"),
                &[
                    ("mob", vec![link(Target::Mob(kind))]),
                    ("price", plain_line(offer.cost.1.to_string())),
                    ("goods", counted(count, vec![link(Target::Item(goods))])),
                ],
            )),
            Goods::Directions => {}
        }
    }
    if !sources.is_empty() {
        heading(commands, parent, "codex.heading.get", ctx);
        lines(commands, parent, &sources, ctx);
    }

    // What it is for
    let mut made: Vec<ItemId> = uses_of(item).map(|r| r.output.item_id).collect();
    made.dedup();
    if !made.is_empty() {
        let shown = made.len().min(12);
        let mut list = joined(
            made[..shown]
                .iter()
                .map(|i| vec![link(Target::Item(*i))])
                .collect(),
        );
        if made.len() > shown {
            list.push(text_span(format!(", … (+{})", made.len() - shown)));
        }
        uses.insert(
            0,
            fill_line(
                tr("codex.line.goes_into"),
                &[
                    ("count", plain_line(made.len().to_string())),
                    ("items", list),
                ],
            ),
        );
    }
    if !uses.is_empty() {
        heading(commands, parent, "codex.heading.uses", ctx);
        lines(commands, parent, &uses, ctx);
    }
}

/// The item a block is placed from, if one is.
fn itemfor(block: shared::world::BlockId) -> Option<ItemId> {
    ItemId::ALL
        .into_iter()
        .find(|i| i.get_default_type() == shared::world::ItemType::Block(block))
}

fn effect_line(effect: EffectKind, level: u8, seconds: u32) -> Line {
    fill_line(
        tr("codex.value.effect"),
        &[
            ("effect", vec![link(Target::Effect(effect))]),
            ("level", plain_line(roman(level))),
            ("seconds", plain_line(seconds.to_string())),
        ],
    )
}

fn spawn_mob(
    commands: &mut Commands,
    parent: Entity,
    kind: MobKind,
    with_header: bool,
    ctx: &PageContext,
) {
    let key = format!("{kind:?}");
    let summary = line_of(&format!("codex.mob.{key}.summary")).map(|l| plain(&l));
    if with_header {
        header(commands, parent, &CodexPage::Mob(kind), summary, ctx);
    }
    let body = commands
        .spawn((
            Node {
                column_gap: px(theme::space_m()),
                flex_wrap: FlexWrap::WrapReverse,
                row_gap: px(theme::space_m()),
                ..default()
            },
            ChildOf(parent),
        ))
        .id();
    let column = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                row_gap: px(theme::space_s()),
                flex_grow: 1.0,
                flex_basis: px(260),
                min_width: px(0),
                ..default()
            },
            ChildOf(body),
        ))
        .id();
    let side = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                row_gap: px(theme::space_s()),
                width: px(220),
                flex_shrink: 0.0,
                ..default()
            },
            ChildOf(body),
        ))
        .id();
    commands.spawn((
        ImageNode::new(ctx.assets.load(format!("graphics/creatures/{key}.png"))),
        Node {
            width: px(200),
            height: px(200),
            ..default()
        },
        ChildOf(side),
    ));
    let mut facts = Facts::new();
    let health = kind.health();
    facts.add(
        "codex.fact.health",
        plain_line(trf(
            "codex.value.health",
            &[
                ("health", &health),
                ("hearts", &number(f64::from(health) / 2.0)),
            ],
        )),
    );
    if let Some(strike) = kind.strike() {
        facts.add("codex.fact.damage", plain_line(strike.damage.to_string()));
    }
    if let Some((effect, level, seconds)) = kind.inflicted_effect() {
        facts.add("codex.fact.blow", effect_line(effect, level, seconds));
    }
    let xp = kind.experience();
    if xp > 0 {
        facts.add("codex.fact.xp", plain_line(xp.to_string()));
    }
    if let Some(speed) = line_of(&format!("codex.mob.{key}.speed")) {
        facts.add("codex.fact.speed", speed);
    }
    if let Some(hunt) = kind.hunt() {
        let sight = if hunt.sight == hunt.forget {
            number(f64::from(hunt.sight))
        } else {
            format!(
                "{} / {}",
                number(f64::from(hunt.sight)),
                number(f64::from(hunt.forget))
            )
        };
        facts.add(
            "codex.fact.sight",
            plain_line(trf("codex.value.blocks", &[("blocks", &sight)])),
        );
    }
    let size = kind.size();
    facts.add(
        "codex.fact.size",
        plain_line(trf(
            "codex.value.size",
            &[
                ("width", &number(f64::from((size.x * 10.0).round() / 10.0))),
                ("height", &number(f64::from((size.y * 10.0).round() / 10.0))),
            ],
        )),
    );
    if kind.is_tameable() {
        facts.add("codex.fact.tameable", plain_line(tr("codex.value.yes")));
    }
    facts.spawn(commands, side, ctx);

    let text_look = ctx.look(theme::font_s(), theme::text());
    let mut biomes = Vec::new();
    for def in Biomes::vanilla().defs() {
        for herd in def.herds.iter().filter(|h| h.kind == kind) {
            let key = def.key.trim_start_matches("rustcraft:").to_string();
            biomes.push(fill_line(
                tr("codex.line.herd"),
                &[
                    ("biome", vec![link(Target::Biome(key))]),
                    (
                        "count",
                        plain_line(range(herd.fewest as u32, herd.most as u32)),
                    ),
                ],
            ));
        }
    }
    if let Some(spawns) = line_of(&format!("codex.mob.{key}.spawns")) {
        heading(commands, column, "codex.heading.spawns", ctx);
        spawn_line(commands, column, &spawns, &text_look);
    }
    lines(commands, column, &biomes, ctx);
    if let Some(behaviour) = line_of(&format!("codex.mob.{key}.behaviour")) {
        heading(commands, column, "codex.heading.behaviour", ctx);
        spawn_line(commands, column, &behaviour, &text_look);
    }
    let eats: Vec<Line> = ItemId::ALL
        .into_iter()
        .filter(|i| kind.eats(*i))
        .map(|i| vec![link(Target::Item(i))])
        .collect();
    if !eats.is_empty() {
        heading(commands, column, "codex.heading.eats", ctx);
        spawn_line(commands, column, &joined(eats), &text_look);
    }
    let drops: Vec<Line> = kind
        .loot()
        .iter()
        .map(|loot| {
            fill_line(
                tr("codex.line.drop"),
                &[
                    ("item", vec![link(Target::Item(loot.item))]),
                    ("count", plain_line(loot_count(loot))),
                ],
            )
        })
        .collect();
    if !drops.is_empty() {
        heading(commands, column, "codex.heading.drops", ctx);
        lines(commands, column, &drops, ctx);
    }
    let trades: Vec<Line> = shared::world::trade::offers(kind)
        .iter()
        .map(|offer| {
            let price = counted(offer.cost.1, vec![link(Target::Item(offer.cost.0))]);
            let goods = match offer.goods {
                Goods::Items(item, count) => counted(count, vec![link(Target::Item(item))]),
                Goods::Directions => plain_line(tr("codex.value.directions")),
            };
            fill_line(
                tr("codex.line.trade"),
                &[("price", price), ("goods", goods)],
            )
        })
        .collect();
    if !trades.is_empty() {
        heading(commands, column, "codex.heading.trades", ctx);
        lines(commands, column, &trades, ctx);
    }
}

fn spawn_biome(commands: &mut Commands, parent: Entity, key: &str, ctx: &PageContext) {
    header(commands, parent, &CodexPage::Biome(key.into()), None, ctx);
    if let Some(note) = line_of(&format!("codex.biome.{key}")) {
        spawn_line(commands, parent, &note, &ctx.look(theme::font_s(), theme::text()));
    }
    let biomes = Biomes::vanilla();
    let Some(def) = biomes
        .defs()
        .iter()
        .find(|d| d.key.trim_start_matches("rustcraft:") == key)
    else {
        return;
    };
    let mut facts = Facts::new();
    let ground = |block| match itemfor(block) {
        Some(item) => vec![link(Target::Item(item))],
        None => plain_line(format!("{block:?}")),
    };
    facts.add("codex.fact.surface", ground(def.ground.surface));
    facts.add("codex.fact.under", ground(def.ground.sub_surface));
    if def.village.is_some() {
        facts.add("codex.fact.villages", plain_line(tr("codex.value.yes")));
    }
    facts.spawn(commands, parent, ctx);
    let herds: Vec<Line> = def
        .herds
        .iter()
        .map(|herd| {
            fill_line(
                tr("codex.line.herd_of"),
                &[
                    ("mob", vec![link(Target::Mob(herd.kind))]),
                    (
                        "count",
                        plain_line(range(herd.fewest as u32, herd.most as u32)),
                    ),
                ],
            )
        })
        .collect();
    if !herds.is_empty() {
        heading(commands, parent, "codex.heading.herds", ctx);
        lines(commands, parent, &herds, ctx);
    }
}

fn spawn_effect(commands: &mut Commands, parent: Entity, kind: EffectKind, ctx: &PageContext) {
    let under = if kind.is_beneficial() {
        tr("codex.value.beneficial")
    } else {
        tr("codex.value.harmful")
    };
    header(
        commands,
        parent,
        &CodexPage::Effect(kind),
        Some(under.into()),
        ctx,
    );
    if let Some(note) = line_of(&format!("codex.effect.{kind:?}")) {
        spawn_line(commands, parent, &note, &ctx.look(theme::font_s(), theme::text()));
    }
    let mut given = Vec::new();
    let potions: Vec<Line> = ItemId::ALL
        .into_iter()
        .filter(|i| i.potion().is_some_and(|p| p.effect == kind))
        .map(|i| vec![link(Target::Item(i))])
        .collect();
    if !potions.is_empty() {
        given.push(joined(potions));
    }
    for item in ItemId::ALL {
        if let Some((effect, level, ticks)) = item.food_effect().filter(|f| f.0 == kind) {
            let mut line = vec![link(Target::Item(item)), text_span(": ")];
            line.extend(effect_line(effect, level, ticks / TICKS_PER_SECOND as u32));
            given.push(line);
        }
    }
    for mob in creatures() {
        if let Some((effect, level, seconds)) = mob.inflicted_effect().filter(|f| f.0 == kind) {
            let mut line = vec![link(Target::Mob(mob)), text_span(": ")];
            line.extend(effect_line(effect, level, seconds));
            given.push(line);
        }
    }
    if !given.is_empty() {
        heading(commands, parent, "codex.heading.given_by", ctx);
        lines(commands, parent, &given, ctx);
    }
}

/// The name a list shows for a page: `???` for a creature not met yet.
pub fn entry_name(page: &CodexPage, seen: &dyn Fn(MobKind) -> bool) -> String {
    match page {
        CodexPage::Mob(kind) if !seen(*kind) => tr("codex.unknown").to_string(),
        _ => page.title(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_template_takes_the_runs_of_its_arguments() {
        let line = fill_line(
            "{mob} sells it: {count}.",
            &[
                ("mob", vec![link(Target::Mob(MobKind::Smith))]),
                ("count", plain_line("5")),
            ],
        );
        assert_eq!(line[0].link, Some(Target::Mob(MobKind::Smith)));
        assert_eq!(line[1].text.as_deref(), Some(" sells it: "));
        assert_eq!(line[2].text.as_deref(), Some("5"));
        assert_eq!(line[3].text.as_deref(), Some("."));
    }

    #[test]
    fn the_search_ignores_case_and_accents() {
        crate::i18n::in_language(crate::i18n::Language::French, || {
            assert!(matches("epee", &CodexPage::Item(ItemId::IronSword)));
            assert!(!matches("loup", &CodexPage::Item(ItemId::IronSword)));
            assert!(matches("LOUP", &CodexPage::Mob(MobKind::Wolf)));
        });
    }

    #[test]
    fn every_section_has_pages_but_the_guides_until_they_are_written() {
        for section in Section::ALL {
            assert!(
                !entries(section).is_empty() || section == Section::Guides,
                "{section:?}"
            );
        }
        assert!(!entries(Section::Creatures).contains(&CodexPage::Mob(MobKind::Boat)));
    }
}
