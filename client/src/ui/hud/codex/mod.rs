//! The Codex: the game's own wiki, over the whole screen. Its sections list the items, the
//! creatures, the biomes, the status effects and the guides; a page says what the tables of the
//! game and the `codex.*` texts of the languages say of one of them, in the language of the game.
//! The same texts make wiki.rustcraft.dev.
//!
//! F1 opens it during gameplay, and closes it, as Escape does; the pause menu has a button for it,
//! and the item browser opens it on the page of an item. A link of a page opens another, the page
//! left going on the history, which the back button and Backspace go back through.

use bevy::prelude::*;
use shared::players::effects::EffectKind;
use shared::world::{Biomes, ItemId, MobKind};

use crate::i18n::{effect_kind_name, item_name, mob_name, tr};
use crate::state::{GameSet, GameState};

#[cfg(test)]
mod goldens;
pub mod pages;
mod screen;
pub mod text;

pub use screen::*;

/// The title of the Codex.
pub const CODEX_TITLE: &str = "codex.title";

/// The address of the wiki on the web, whose pages have the same paths as these.
pub const WIKI_URL: &str = "https://wiki.rustcraft.dev";

/// A part of the Codex, with its list of pages.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Section {
    #[default]
    Items,
    Creatures,
    Biomes,
    Effects,
    Guides,
}

impl Section {
    pub const ALL: [Section; 5] = [
        Section::Items,
        Section::Creatures,
        Section::Biomes,
        Section::Effects,
        Section::Guides,
    ];

    pub fn label(self) -> &'static str {
        tr(match self {
            Section::Items => "codex.section.items",
            Section::Creatures => "codex.section.creatures",
            Section::Biomes => "codex.section.biomes",
            Section::Effects => "codex.section.effects",
            Section::Guides => "codex.section.guides",
        })
    }
}

/// A page of the Codex.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum CodexPage {
    Item(ItemId),
    Mob(MobKind),
    /// A biome, by its key without `rustcraft:`.
    Biome(String),
    Effect(EffectKind),
    /// A guide, by its slug: `farming`.
    Guide(String),
}

impl CodexPage {
    pub fn section(&self) -> Section {
        match self {
            CodexPage::Item(_) => Section::Items,
            CodexPage::Mob(_) => Section::Creatures,
            CodexPage::Biome(_) => Section::Biomes,
            CodexPage::Effect(_) => Section::Effects,
            CodexPage::Guide(_) => Section::Guides,
        }
    }

    /// Its title, in the language of the game.
    pub fn title(&self) -> String {
        match self {
            CodexPage::Item(item) => item_name(*item).to_string(),
            CodexPage::Mob(kind) => mob_name(*kind).to_string(),
            CodexPage::Biome(key) => biome_name(key),
            CodexPage::Effect(kind) => effect_kind_name(*kind).to_string(),
            CodexPage::Guide(slug) => pages::guide_title(slug),
        }
    }

    /// Its path on the wiki, after `WIKI_URL`.
    pub fn wiki_path(&self) -> String {
        match self {
            CodexPage::Item(item) => format!("/items/{}", slug_of(&format!("{item:?}"))),
            CodexPage::Mob(kind) => format!("/creatures/{}", slug_of(&format!("{kind:?}"))),
            CodexPage::Biome(key) => format!("/biomes/{}", key.replace('_', "-")),
            CodexPage::Effect(kind) => format!("/effects#{kind:?}"),
            CodexPage::Guide(slug) => format!("/guides/{slug}"),
        }
    }
}

/// The name of a biome, by its key without `rustcraft:`, in the language of the game.
pub fn biome_name(key: &str) -> String {
    let name_key = format!("biome.{key}");
    let translated = tr(&name_key);
    if translated != name_key {
        return translated.to_string();
    }
    let biomes = Biomes::vanilla();
    biomes
        .defs()
        .iter()
        .find(|def| def.key.trim_start_matches("rustcraft:") == key)
        .map_or_else(|| key.replace('_', " "), |def| spaced(&def.name))
}

/// `IcePlain` reads `Ice Plain`.
fn spaced(name: &str) -> String {
    let mut out = String::new();
    for (i, ch) in name.char_indices() {
        if i > 0 && ch.is_uppercase() {
            out.push(' ');
        }
        out.push(ch);
    }
    out
}

/// The part of a wiki path a name makes, as the wiki makes it: `OakLog` becomes `oak-log`.
pub fn slug_of(id: &str) -> String {
    let mut out = String::new();
    let mut previous: Option<char> = None;
    for ch in id.chars() {
        if ch.is_uppercase() && previous.is_some_and(|p| p.is_lowercase() || p.is_ascii_digit()) {
            out.push('-');
        }
        out.push(if ch == '_' {
            '-'
        } else {
            ch.to_ascii_lowercase()
        });
        previous = Some(ch);
    }
    out
}

/// What the Codex shows, kept for the session: it opens again where it was left.
#[derive(Resource, Default, Clone, PartialEq, Debug)]
pub struct CodexState {
    pub section: Section,
    /// What the search field holds.
    pub query: String,
    /// The page shown, if any.
    pub page: Option<CodexPage>,
    /// The pages the back button goes back to, the last one first.
    pub history: Vec<CodexPage>,
}

impl CodexState {
    /// Shows `page`, the open one going on the history, and its section.
    pub fn open(&mut self, page: CodexPage) {
        if self.page.as_ref() == Some(&page) {
            return;
        }
        if let Some(open) = self.page.replace(page) {
            self.history.push(open);
        }
        let section = self
            .page
            .as_ref()
            .map(CodexPage::section)
            .unwrap_or_default();
        if self.section != section {
            self.section = section;
            self.query.clear();
        }
    }

    /// Goes back to the last page of the history; returns whether there was one.
    pub fn back(&mut self) -> bool {
        match self.history.pop() {
            Some(page) => {
                self.section = page.section();
                self.page = Some(page);
                true
            }
            None => false,
        }
    }
}

pub fn codex_plugin(app: &mut App) {
    app.init_resource::<CodexState>()
        .add_systems(
            Update,
            (
                toggle_codex,
                (
                    read_codex_search,
                    show_codex_list,
                    show_codex_page,
                    codex_keys,
                )
                    .chain()
                    .run_if(codex_open),
            )
                .chain()
                .after(GameSet::UiMode)
                .run_if(in_state(GameState::Game)),
        )
        .add_systems(PostUpdate, close_codex)
        .add_observer(text::follow_link);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_page_opened_goes_on_the_history_and_back_comes_back_to_it() {
        let mut state = CodexState::default();
        state.open(CodexPage::Item(ItemId::Furnace));
        state.query = "fur".into();
        state.open(CodexPage::Mob(MobKind::Wolf));
        assert_eq!(state.section, Section::Creatures);
        assert!(state.query.is_empty(), "another section starts unfiltered");
        assert!(state.back());
        assert_eq!(state.page, Some(CodexPage::Item(ItemId::Furnace)));
        assert_eq!(state.section, Section::Items);
        assert!(!state.back());
    }

    #[test]
    fn the_paths_of_the_wiki_are_those_it_makes() {
        assert_eq!(
            CodexPage::Item(ItemId::OakLog).wiki_path(),
            "/items/oak-log"
        );
        assert_eq!(
            CodexPage::Mob(MobKind::GustOwl).wiki_path(),
            "/creatures/gust-owl"
        );
        assert_eq!(
            CodexPage::Biome("ice_plain".into()).wiki_path(),
            "/biomes/ice-plain"
        );
    }
}
