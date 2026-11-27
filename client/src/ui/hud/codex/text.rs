//! The texts of the Codex drawn in the UI: the blocks and the lines of their markup
//! (`shared::codex::markup`). A link is a run of its line, underlined, that a click follows.

use bevy::prelude::*;
use bevy::text::Underline;
use shared::codex::markup::{self, Block, Line, Target};

use super::{biome_name, CodexPage, CodexState, OpenCodex, WIKI_URL};
use crate::i18n::{effect_kind_name, item_name, mob_name, tr};
use crate::ui::hud::item_browser::{ItemBrowserState, ItemView, RecipeTab};
use crate::ui::theme;

/// A run of a text that is a link, and what it opens.
#[derive(Component, Clone, Debug)]
pub struct CodexLink(pub Target);

/// What the item links of a text open: the page of the Codex, or the item's sheet in the item
/// browser, where the text is.
#[derive(Component, Clone, Copy, PartialEq, Eq, Debug)]
pub enum LinkHost {
    Codex,
    Browser,
}

/// The font and size of a text.
#[derive(Clone)]
pub struct Look {
    pub font: Handle<Font>,
    pub size: f32,
    pub color: Color,
}

impl Look {
    pub fn new(font: &Handle<Font>, size: f32, color: Color) -> Self {
        Self {
            font: font.clone(),
            size,
            color,
        }
    }

    fn font(&self, size: f32) -> TextFont {
        TextFont {
            font: self.font.clone().into(),
            font_size: px(size).into(),
            ..default()
        }
    }
}

/// The name a link without words of its own shows.
pub fn target_name(target: &Target) -> String {
    match target {
        Target::Item(item) => item_name(*item).to_string(),
        Target::Mob(kind) => mob_name(*kind).to_string(),
        Target::Biome(key) => biome_name(key),
        Target::Effect(kind) => effect_kind_name(*kind).to_string(),
        Target::Guide(slug) => super::pages::guide_title(slug),
        Target::Page(path) => path.replace('-', " "),
    }
}

/// The page of the Codex a link opens, if it is one: a page of the wiki only is not.
pub fn page_of(target: &Target) -> Option<CodexPage> {
    Some(match target {
        Target::Item(item) => CodexPage::Item(*item),
        Target::Mob(kind) => CodexPage::Mob(*kind),
        Target::Biome(key) => CodexPage::Biome(key.clone()),
        Target::Effect(kind) => CodexPage::Effect(*kind),
        Target::Guide(slug) => CodexPage::Guide(slug.clone()),
        Target::Page(_) => return None,
    })
}

/// The words of a markup text, in the language of the game, without their marks: for the
/// accessible label of a block, and for the search.
pub fn plain(line: &Line) -> String {
    markup::plain(line, target_name)
}

/// Spawns a line of text under `parent`: a `Text` and a span by run, the links underlined.
pub fn spawn_line(commands: &mut Commands, parent: Entity, line: &Line, look: &Look) -> Entity {
    let text = commands
        .spawn((
            Text::default(),
            look.font(look.size),
            TextColor(look.color),
            ChildOf(parent),
        ))
        .id();
    for span in line {
        let words = span
            .text
            .clone()
            .unwrap_or_else(|| span.link.as_ref().map(target_name).unwrap_or_default());
        let color = match (&span.link, span.strong, span.em) {
            (Some(_), _, _) => theme::accent(),
            (None, true, _) => theme::accent_hi(),
            (None, false, true) => theme::accent_hi(),
            (None, false, false) => look.color,
        };
        let mut run = commands.spawn((
            TextSpan::new(words),
            look.font(look.size),
            TextColor(color),
            ChildOf(text),
        ));
        if let Some(target) = &span.link {
            run.insert((CodexLink(target.clone()), Underline));
        }
    }
    text
}

/// Spawns the blocks of a text under `parent`, a column. `extra` spawns the blocks the game
/// works out: a recipe, a table.
pub fn spawn_blocks(
    commands: &mut Commands,
    parent: Entity,
    blocks: &[Block],
    look: &Look,
    extra: &mut dyn FnMut(&mut Commands, Entity, &Block),
) {
    for block in blocks {
        match block {
            Block::Heading { level, text } => {
                let size = match level {
                    1 => theme::font_l(),
                    2 => theme::font_m(),
                    _ => look.size,
                };
                let heading = Look {
                    size,
                    color: if *level == 3 { theme::accent_hi() } else { theme::text() },
                    ..look.clone()
                };
                let line = spawn_line(commands, parent, text, &heading);
                commands.entity(line).insert(Node {
                    margin: UiRect::top(px(if *level <= 2 { theme::space_m() } else { theme::space_s() })),
                    ..default()
                });
            }
            Block::Paragraph { text } => {
                spawn_line(commands, parent, text, look);
            }
            Block::List { ordered, items } => {
                let list = commands
                    .spawn((
                        Node {
                            flex_direction: FlexDirection::Column,
                            row_gap: px(theme::space_xs()),
                            ..default()
                        },
                        ChildOf(parent),
                    ))
                    .id();
                for (index, item) in items.iter().enumerate() {
                    let row = commands
                        .spawn((
                            Node {
                                width: percent(100),
                                column_gap: px(theme::space_s()),
                                ..default()
                            },
                            ChildOf(list),
                        ))
                        .id();
                    let marker = if *ordered {
                        format!("{}.", index + 1)
                    } else {
                        "•".to_string()
                    };
                    commands.spawn((
                        Text::new(marker),
                        look.font(look.size),
                        TextColor(theme::muted()),
                        Node {
                            flex_shrink: 0.0,
                            ..default()
                        },
                        ChildOf(row),
                    ));
                    let line = spawn_line(commands, row, item, look);
                    // the rest of the row, its words wrapping in it
                    commands.entity(line).insert(Node {
                        flex_grow: 1.0,
                        flex_basis: px(0),
                        min_width: px(0),
                        ..default()
                    });
                }
            }
            Block::Table {
                head,
                numeric,
                rows,
            } => {
                let columns = head
                    .as_ref()
                    .map_or_else(|| rows.first().map_or(1, Vec::len), Vec::len);
                let table = commands
                    .spawn((
                        Node {
                            display: Display::Grid,
                            grid_template_columns: vec![RepeatedGridTrack::auto(columns as u16)],
                            column_gap: px(theme::space_m()),
                            row_gap: px(theme::space_xs()),
                            ..default()
                        },
                        ChildOf(parent),
                    ))
                    .id();
                let mut cells = |row: &[Line], look: &Look| {
                    for (column, cell) in row.iter().enumerate() {
                        let text = spawn_line(commands, table, cell, look);
                        if numeric.get(column).copied().unwrap_or(false) {
                            commands.entity(text).insert(Node {
                                justify_self: JustifySelf::End,
                                ..default()
                            });
                        }
                    }
                };
                if let Some(head) = head {
                    cells(
                        head,
                        &Look {
                            color: theme::muted(),
                            ..look.clone()
                        },
                    );
                }
                for row in rows {
                    cells(row, look);
                }
            }
            Block::Recipe { .. } | Block::Computed { .. } => extra(commands, parent, block),
        }
    }
}

/// The note of a text of the languages, read as blocks; nothing when the language has no such
/// key or the text is wrong (a test reads every one).
pub fn blocks_of(key: &str) -> Vec<Block> {
    let text = tr(key);
    if text == key {
        return Vec::new();
    }
    markup::parse(text).unwrap_or_default()
}

/// A text of the languages read as one line; `None` when the language has no such key.
pub fn line_of(key: &str) -> Option<Line> {
    let text = tr(key);
    (text != key).then(|| markup::inline(text).ok()).flatten()
}

/// Follows a link clicked: an item of the item browser's sheet opens its sheet there, any other
/// link opens its page in the Codex, a page of the wiki only opens in the web browser.
pub fn follow_link(
    click: On<Pointer<Click>>,
    links: Query<&CodexLink>,
    hosts: Query<&LinkHost>,
    parents: Query<&ChildOf>,
    mut browser: ResMut<ItemBrowserState>,
    mut codex: ResMut<CodexState>,
    mut commands: Commands,
) {
    if click.button != PointerButton::Primary {
        return;
    }
    let Ok(CodexLink(target)) = links.get(click.entity) else {
        return;
    };
    let host = parents
        .iter_ancestors(click.entity)
        .find_map(|e| hosts.get(e).ok().copied())
        .unwrap_or(LinkHost::Codex);
    match (page_of(target), host) {
        (Some(CodexPage::Item(item)), LinkHost::Browser) => browser.open(
            ItemView {
                item,
                tab: RecipeTab::Sheet,
            },
            false,
        ),
        (Some(page), LinkHost::Codex) => codex.open(page),
        (Some(page), LinkHost::Browser) => commands.queue(OpenCodex {
            page: Some(page),
            opener: None,
        }),
        (None, _) => {
            if let Target::Page(path) = target {
                open_on_the_web(&format!("{WIKI_URL}/{path}"));
            }
        }
    }
}

/// Opens `url` in the web browser of the system.
pub fn open_on_the_web(url: &str) {
    if let Err(e) = opener::open(url) {
        warn!("cannot open {url}: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::Language;

    /// Every `codex.*` text of every language reads as markup, its links to things that exist.
    #[test]
    fn every_text_of_the_codex_reads() {
        for language in Language::ALL {
            crate::i18n::in_language(language, || {
                for key in crate::i18n::keys_with_prefix("codex.") {
                    let text = tr(key);
                    if key.starts_with("codex.item.") {
                        if let Err(e) = markup::parse(text) {
                            panic!("{} {key}: {e}", language.code());
                        }
                    } else if let Err(e) = markup::inline(text) {
                        panic!("{} {key}: {e}", language.code());
                    }
                }
            });
        }
    }
}
