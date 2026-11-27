//! The languages of the game. Each has one file of `lang/` in the assets, `<code>.ron`, a map
//! from a key to its text; English is complete, and gives the text of a key another language
//! lacks. A text may hold parameters, `{name}`, filled by `trf`.
//!
//! The language chosen is global: `tr` and `trf` read it, so that the names of items and the
//! labels of enums need no resource. The UI looks a text up when it spawns or changes, never
//! every frame. The files are read at start (`load`); a dev build reads them again when one is
//! saved (`watch`). Adding a language: its file and a variant of `Language`.

use serde::{Deserialize, Serialize};
use shared::players::{armour::ArmourPiece, game_mode::DeathCause, progress::Achievement};
use shared::world::{depths::DepthTier, Enchantment, ItemCategory, ItemId, ItemTag, MobKind};
use std::collections::HashMap;
use std::fmt::Display;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::RwLock;

#[cfg(feature = "dev")]
pub mod watch;

/// A language of the game.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Language {
    #[default]
    English,
    French,
}

impl Language {
    pub const ALL: [Language; 2] = [Language::English, Language::French];

    fn index(self) -> usize {
        self as usize
    }

    /// Its code, as in `LANG` and the name of its file: `en`.
    pub fn code(self) -> &'static str {
        match self {
            Language::English => "en",
            Language::French => "fr",
        }
    }

    /// Its name in itself, as the settings show it.
    pub fn native_name(self) -> &'static str {
        match self {
            Language::English => "English",
            Language::French => "Français",
        }
    }

    /// The language of a locale such as `fr-FR` or `fr_CA.UTF-8`: English unless it is one of ours.
    pub fn of_locale(locale: &str) -> Language {
        let code = locale.split(['-', '_', '.']).next().unwrap_or("");
        Language::ALL
            .into_iter()
            .find(|l| l.code().eq_ignore_ascii_case(code))
            .unwrap_or_default()
    }

    /// The language of the system, or English.
    pub fn of_system() -> Language {
        sys_locale::get_locale().map_or(Language::English, |l| Language::of_locale(&l))
    }
}

/// The folder of the language files in the assets folder.
pub fn lang_folder(assets_folder: &Path) -> PathBuf {
    assets_folder.join("lang")
}

/// The texts of a language, read from its file.
struct Table {
    texts: HashMap<String, String>,
}

/// The texts of every language, in the order of `Language::ALL`, and the names of the items.
/// Each reading of the files is leaked, so that the texts handed out stay valid when the files
/// are read again: a few hundred kilobytes for each save of a language file, in a dev build.
#[derive(Clone, Copy)]
struct Tables {
    languages: &'static [Table],
    /// The names of the engine items in each language, in the order of `ItemId::ENGINE`.
    item_names: &'static [Vec<&'static str>],
}

/// The tables in use, installed by `load` or `install`.
static TABLES: RwLock<Option<Tables>> = RwLock::new(None);

/// Reads the files of every language from `folder`, or says which one cannot be read.
fn read(folder: &Path) -> Result<Tables, String> {
    let mut languages = Vec::with_capacity(Language::ALL.len());
    for language in Language::ALL {
        let path = folder.join(format!("{}.ron", language.code()));
        let source = std::fs::read_to_string(&path)
            .map_err(|e| format!("{} cannot be read: {e}", path.display()))?;
        let texts = ron::from_str(&source)
            .map_err(|e| format!("{} cannot be read: {e}", path.display()))?;
        languages.push(Table { texts });
    }
    let languages: &'static [Table] = languages.leak();
    let item_names = Language::ALL
        .iter()
        .map(|&language| {
            ItemId::ENGINE
                .iter()
                .map(|item| {
                    lookup_in(languages, language, &format!("item.{item:?}")).unwrap_or("?")
                })
                .collect()
        })
        .collect::<Vec<_>>()
        .leak();
    Ok(Tables {
        languages,
        item_names,
    })
}

#[cfg(test)]
thread_local! {
    /// The tables a test installed, seen by its own thread only: the tests beside it keep
    /// those of the source tree.
    static INSTALLED_IN_TEST: std::cell::Cell<Option<Tables>> = const { std::cell::Cell::new(None) };
}

/// Puts `tables` in use: the UI spawned from now on shows their texts.
fn install(tables: Tables) {
    #[cfg(test)]
    INSTALLED_IN_TEST.set(Some(tables));
    #[cfg(not(test))]
    {
        *TABLES.write().unwrap_or_else(|poison| poison.into_inner()) = Some(tables);
    }
}

/// Reads the language files of the assets folder and puts them in use. The game cannot show a
/// word without them: a file missing or wrong stops it, naming the file.
///
/// The clients of the tests, whose assets folder is the source tree, leave the tables to
/// `tables`, read once for the whole process.
pub fn load(assets_folder: &Path) {
    if cfg!(test) {
        return;
    }
    let folder = lang_folder(assets_folder);
    install(read(&folder).unwrap_or_else(|e| panic!("{e}")));
}

/// The tables in use. The tests, for which `load` reads nothing, read those of the source tree.
fn tables() -> Tables {
    #[cfg(test)]
    if let Some(tables) = INSTALLED_IN_TEST.get() {
        return tables;
    }
    if let Some(tables) = *TABLES.read().unwrap_or_else(|poison| poison.into_inner()) {
        return tables;
    }
    #[cfg(test)]
    {
        let mut installed = TABLES.write().unwrap_or_else(|poison| poison.into_inner());
        *installed.get_or_insert_with(|| {
            let data = Path::new(env!("CARGO_MANIFEST_DIR")).join("../data");
            read(&lang_folder(&data)).unwrap_or_else(|e| panic!("{e}"))
        })
    }
    #[cfg(not(test))]
    panic!("a text was looked up before the language files were loaded (`i18n::load`)")
}

static CURRENT: AtomicUsize = AtomicUsize::new(0);

#[cfg(test)]
thread_local! {
    static OVERRIDE: std::cell::Cell<Option<Language>> = const { std::cell::Cell::new(None) };
}

/// The language the game is in.
pub fn language() -> Language {
    #[cfg(test)]
    if let Some(language) = OVERRIDE.with(|o| o.get()) {
        return language;
    }
    Language::ALL[CURRENT.load(Ordering::Relaxed)]
}

/// Puts the game in `language`. The UI shown already changes when it is spawned again.
pub fn set_language(language: Language) {
    CURRENT.store(language.index(), Ordering::Relaxed);
}

/// Runs `f` with the texts of `language`, on this thread only: the tests of another language,
/// beside those in English.
#[cfg(test)]
pub fn in_language<T>(language: Language, f: impl FnOnce() -> T) -> T {
    let before = OVERRIDE.with(|o| o.replace(Some(language)));
    let result = f();
    OVERRIDE.with(|o| o.set(before));
    result
}

fn lookup(language: Language, key: &str) -> Option<&'static str> {
    lookup_in(tables().languages, language, key)
}

/// The text of `key` in `language` among `languages`, or in English when it lacks it.
fn lookup_in(languages: &'static [Table], language: Language, key: &str) -> Option<&'static str> {
    languages[language.index()]
        .texts
        .get(key)
        .or_else(|| languages[Language::English.index()].texts.get(key))
        .map(String::as_str)
}

/// The keys of English starting with `prefix`, in order.
#[cfg(test)]
pub fn keys_with_prefix(prefix: &str) -> Vec<&'static str> {
    let mut keys: Vec<&'static str> = tables()[Language::English.index()]
        .texts
        .keys()
        .filter(|k| k.starts_with(prefix))
        .map(String::as_str)
        .collect();
    keys.sort_unstable();
    keys
}

/// The text of `key` in the language of the game, the English one when it lacks it, or the key
/// itself when English lacks it too.
pub fn tr(key: &str) -> &str {
    lookup(language(), key).unwrap_or(key)
}

/// The text of `english` in the group `group`, keyed by its English text: `hint.Move`. For the
/// short words of the key hints, written in many constants; `english` itself when there is none.
pub fn tr_as<'a>(group: &str, english: &'a str) -> &'a str {
    lookup(language(), &format!("{group}.{english}")).unwrap_or(english)
}

/// The text of `key` with its parameters `{name}` replaced by their values.
pub fn trf(key: &str, args: &[(&str, &dyn Display)]) -> String {
    fill(tr(key), args)
}

/// `template` with its parameters `{name}` replaced by their values; an unknown one stays.
pub fn fill(template: &str, args: &[(&str, &dyn Display)]) -> String {
    let mut out = String::with_capacity(template.len() + 16);
    let mut rest = template;
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let after = &rest[open + 1..];
        match after.find('}') {
            Some(close) => {
                let name = &after[..close];
                match args.iter().find(|(n, _)| *n == name) {
                    Some((_, value)) => out.push_str(&value.to_string()),
                    None => out.push_str(&rest[open..open + close + 2]),
                }
                rest = &after[close + 1..];
            }
            None => {
                out.push_str(&rest[open..]);
                rest = "";
            }
        }
    }
    out.push_str(rest);
    out
}

/// The name of `item` in the language of the game.
pub fn item_name(item: ItemId) -> &'static str {
    match tables().item_names[language().index()].get(item.index()) {
        Some(name) => name,
        // an item only the content files define
        None => lookup(language(), &format!("item.{item:?}")).unwrap_or("?"),
    }
}

/// The text of a key built at run time, such as `mob.Fox`: "?" when no language has it.
fn tr_built(key: String) -> &'static str {
    lookup(language(), &key).unwrap_or("?")
}

/// The name of a status effect.
pub fn effect_kind_name(kind: shared::players::effects::EffectKind) -> &'static str {
    tr_built(format!("effect.{kind:?}"))
}

/// The name of a mob, an animal or a villager.
pub fn mob_name(kind: MobKind) -> &'static str {
    tr_built(format!("mob.{kind:?}"))
}

pub fn achievement_title(achievement: Achievement) -> &'static str {
    tr_built(format!("achievement.{achievement:?}.title"))
}

pub fn achievement_goal(achievement: Achievement) -> &'static str {
    tr_built(format!("achievement.{achievement:?}.goal"))
}

/// The title shown on reaching a depth tier: "The Caves".
pub fn depth_title(tier: DepthTier) -> &'static str {
    tr_built(format!("depth.{tier:?}"))
}

pub fn enchantment_name(enchantment: Enchantment) -> &'static str {
    tr_built(format!("enchantment.{enchantment:?}"))
}

/// Its name and its level: "Efficiency II".
pub fn enchantment_title(enchantment: Enchantment, level: u32) -> String {
    let numeral = ["", " I", " II", " III"];
    let numeral = numeral.get(level as usize).copied().unwrap_or_default();
    format!("{}{numeral}", enchantment_name(enchantment))
}

/// What it does at `level`, in a few words: "+100 % mining speed".
pub fn enchantment_effect(enchantment: Enchantment, level: u32) -> String {
    fill(
        tr_built(format!("enchantment.{enchantment:?}.effect")),
        &[("n", &enchantment.amount(level))],
    )
}

pub fn armour_piece_name(piece: ArmourPiece) -> &'static str {
    tr_built(format!("armour.{piece:?}"))
}

pub fn category_name(category: ItemCategory) -> &'static str {
    tr_built(format!("category.{category:?}"))
}

pub fn tag_name(tag: ItemTag) -> &'static str {
    tr_built(format!("tag.{tag:?}"))
}

/// The line of the death screen, naming the player who shot when it is known.
pub fn death_message(cause: DeathCause, shooter: Option<&str>) -> String {
    match (cause, shooter) {
        (DeathCause::Shot(_), Some(name)) => trf("death.shot_by", &[("name", &name)]),
        (DeathCause::Mob(kind), _) => tr_built(format!("death.mob.{kind:?}")).to_string(),
        (DeathCause::ThrownOff(kind), _) => tr_built(format!("death.thrown.{kind:?}")).to_string(),
        (DeathCause::Shot(_), None) => tr("death.Shot").to_string(),
        (cause, _) => tr_built(format!("death.{cause:?}")).to_string(),
    }
}

/// `text` in lower case without its accents, to compare what a player types with a name.
pub fn folded(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        let plain = match c {
            'à' | 'â' | 'ä' | 'á' | 'å' | 'À' | 'Â' | 'Ä' | 'Á' | 'Å' => "a",
            'ç' | 'Ç' => "c",
            'é' | 'è' | 'ê' | 'ë' | 'É' | 'È' | 'Ê' | 'Ë' => "e",
            'î' | 'ï' | 'í' | 'Î' | 'Ï' | 'Í' => "i",
            'ô' | 'ö' | 'ó' | 'Ô' | 'Ö' | 'Ó' => "o",
            'ù' | 'û' | 'ü' | 'ú' | 'Ù' | 'Û' | 'Ü' | 'Ú' => "u",
            'ÿ' | 'Ÿ' => "y",
            'ñ' | 'Ñ' => "n",
            'œ' | 'Œ' => "oe",
            'æ' | 'Æ' => "ae",
            '’' => "'",
            _ => {
                out.extend(c.to_lowercase());
                continue;
            }
        };
        out.push_str(plain);
    }
    out
}

/// The answer of the server `content` in the language of the game, when it is one of the
/// answers `server.*` of the English file; `None` for another, such as a mod's, or that of a
/// server of another version, which stay as they came.
///
/// A parameter named `item` holds an item as the server writes it, `OakLog` or `oak_log`, and
/// is given its name; the value of another is replaced by `server.word.<value>` when there is one.
pub fn translate_server_answer(content: &str) -> Option<String> {
    let english = &tables().languages[Language::English.index()].texts;
    let mut templates: Vec<(&String, &String)> = english
        .iter()
        .filter(|(key, _)| key.starts_with("server.") && !key.starts_with("server.word."))
        .collect();
    // the longest first: "Gave {n} {item}, {lost} did not fit" before "Gave {n} {item}"
    templates.sort_by_key(|(key, template)| (std::cmp::Reverse(literal_length(template)), *key));
    for (key, template) in templates {
        let Some(values) = parse(template, content) else {
            continue;
        };
        let translated: Vec<(&str, String)> = values
            .iter()
            .map(|(name, value)| (*name, translate_value(name, value)))
            .collect();
        let args: Vec<(&str, &dyn Display)> = translated
            .iter()
            .map(|(name, value)| (*name, value as &dyn Display))
            .collect();
        return Some(trf(key, &args));
    }
    None
}

fn translate_value(name: &str, value: &str) -> String {
    if name == "item" {
        if let Some(item) = ItemId::from_name(value) {
            return item_name(item).to_string();
        }
    }
    let key = format!("server.word.{value}");
    match lookup(language(), &key) {
        Some(word) => word.to_string(),
        None => value.to_string(),
    }
}

fn literal_length(template: &str) -> usize {
    let mut length = 0;
    let mut inside = false;
    for c in template.chars() {
        match c {
            '{' => inside = true,
            '}' => inside = false,
            _ if !inside => length += 1,
            _ => {}
        }
    }
    length
}

/// The values of the parameters of `template` in `text`, when `text` is `template` filled. A
/// parameter takes the shortest text before the literal that follows it, the last one all
/// that is left; none is empty.
fn parse<'t>(template: &'t str, text: &str) -> Option<Vec<(&'t str, String)>> {
    let mut values = Vec::new();
    let mut rest = text;
    let mut pattern = template;
    let mut pending: Option<&str> = None;
    loop {
        let (literal, next) = match pattern.find('{') {
            Some(open) => (&pattern[..open], Some(open)),
            None => (pattern, None),
        };
        match pending.take() {
            None => rest = rest.strip_prefix(literal)?,
            Some(name) if next.is_none() && literal.is_empty() => {
                if rest.is_empty() {
                    return None;
                }
                values.push((name, rest.to_string()));
                rest = "";
            }
            Some(name) => {
                let at = if next.is_none() {
                    // the end of the text
                    rest.strip_suffix(literal).map(|v| v.len())?
                } else {
                    rest.find(literal)?
                };
                if at == 0 || literal.is_empty() {
                    return None;
                }
                values.push((name, rest[..at].to_string()));
                rest = &rest[at + literal.len()..];
            }
        }
        let Some(open) = next else {
            return rest.is_empty().then_some(values);
        };
        let close = pattern[open..].find('}')? + open;
        pending = Some(&pattern[open + 1..close]);
        pattern = &pattern[close + 1..];
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    fn keys(language: Language) -> BTreeSet<&'static str> {
        tables().languages[language.index()]
            .texts
            .keys()
            .map(String::as_str)
            .collect()
    }

    fn parameters(text: &str) -> BTreeSet<&str> {
        text.split('{')
            .skip(1)
            .filter_map(|part| part.split_once('}').map(|(name, _)| name))
            .collect()
    }

    #[test]
    fn every_language_has_the_keys_of_english_and_no_other() {
        let english = keys(Language::English);
        for language in Language::ALL {
            let theirs = keys(language);
            let unknown: Vec<_> = theirs.difference(&english).collect();
            assert!(
                unknown.is_empty(),
                "{language:?} has keys English lacks: {unknown:?}"
            );
            let missing: Vec<_> = english.difference(&theirs).collect();
            assert!(missing.is_empty(), "{language:?} lacks {missing:?}");
            for key in theirs {
                let text = &tables().languages[language.index()].texts[key];
                let english = &tables().languages[0].texts[key];
                assert_eq!(
                    parameters(text),
                    parameters(english),
                    "{language:?}: the parameters of {key}"
                );
            }
        }
    }

    #[test]
    fn every_item_has_a_name_in_every_language() {
        for language in Language::ALL {
            for item in ItemId::ENGINE {
                assert!(
                    tables().languages[language.index()]
                        .texts
                        .contains_key(&format!("item.{item:?}")),
                    "{language:?} has no name for {item:?}"
                );
            }
        }
    }

    #[test]
    fn a_missing_key_falls_back_on_english_then_on_itself() {
        assert_eq!(
            lookup(Language::French, "home.quit_confirm"),
            Some("Quitter")
        );
        // a key French would lack is looked up in English
        let english = &tables().languages[0].texts;
        let french = &tables().languages[1].texts;
        let key = english.keys().next().unwrap();
        assert!(french.contains_key(key));
        in_language(Language::French, || {
            assert_eq!(tr("no.such.key"), "no.such.key");
        });
        assert_eq!(fill("{a} and {b}", &[("a", &1)]), "1 and {b}");
    }

    #[test]
    fn the_language_changes_the_texts_and_the_names() {
        assert_eq!(item_name(ItemId::OakLog), "Oak Log");
        in_language(Language::French, || {
            assert_eq!(language(), Language::French);
            assert_eq!(tr("home.quit_confirm"), "Quitter");
            assert_eq!(item_name(ItemId::OakLog), "Bûche de chêne");
        });
        assert_eq!(tr("home.quit_confirm"), "Quit");
    }

    #[test]
    fn a_locale_gives_its_language_or_english() {
        assert_eq!(Language::of_locale("fr_FR.UTF-8"), Language::French);
        assert_eq!(Language::of_locale("fr-CA"), Language::French);
        assert_eq!(Language::of_locale("en-GB"), Language::English);
        assert_eq!(Language::of_locale("de-DE"), Language::English);
        assert_eq!(Language::of_locale(""), Language::English);
    }

    #[test]
    fn names_are_compared_without_case_or_accents() {
        assert_eq!(folded("Bûche de Chêne"), "buche de chene");
        assert_eq!(folded("Œuf"), "oeuf");
    }

    #[test]
    fn the_known_answers_of_the_server_are_translated() {
        in_language(Language::French, || {
            assert_eq!(
                translate_server_answer("Gave 3 OakLog").as_deref(),
                Some("3 Bûche de chêne donnés")
            );
            assert_eq!(
                translate_server_answer("Gave 3 OakLog, 2 did not fit in the inventory").as_deref(),
                Some("3 Bûche de chêne donnés, 2 ne tenaient pas dans l’inventaire")
            );
            assert_eq!(
                translate_server_answer("Time set to night").as_deref(),
                Some("Heure réglée sur la nuit")
            );
            assert_eq!(
                translate_server_answer("Unknown command: /fly").as_deref(),
                Some("Commande inconnue : /fly")
            );
            // a mod's answer stays as it came
            assert_eq!(translate_server_answer("Hello from my mod"), None);
        });
        assert_eq!(
            translate_server_answer("Time set to night").as_deref(),
            Some("Time set to night")
        );
    }
}
