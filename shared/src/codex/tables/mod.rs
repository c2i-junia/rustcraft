//! The tables of the guides that the game works out from its data, so that they never go stale:
//! `{{table fishing.catches | Catch | By day | By night}}` in a guide. A guide's tables are in
//! its module, named after it; the words a table needs, its headings first, come from the guide,
//! in its language, as `args`.

mod animals;
mod combat;
mod commands;
mod controls;
mod depths;
mod ember_mother;
mod farming;
mod first_day;
mod fishing;
mod game_modes;
mod health;
mod multiplayer;
mod ores;
mod progression;
mod structures;
mod trading;
mod weather;

use super::markup::{inline, Block, Line, Span, Target};
use crate::world::{ItemId, MobKind};

/// The blocks of the table `name` (`<guide>.<table>`) with the words `args`, its numbers
/// written the way of `language` (`en`, `fr`).
pub fn expand(name: &str, args: &[String], language: &str) -> Result<Vec<Block>, String> {
    let (guide, table) = name
        .split_once('.')
        .ok_or_else(|| format!("{name} is not <guide>.<table>"))?;
    let t = Table {
        name: table,
        args,
        language,
    };
    match guide {
        "animals" => animals::table(&t),
        "combat" => combat::table(&t),
        "commands" => commands::table(&t),
        "controls" => controls::table(&t),
        "depths" => depths::table(&t),
        "ember-mother" => ember_mother::table(&t),
        "farming" => farming::table(&t),
        "first-day" => first_day::table(&t),
        "fishing" => fishing::table(&t),
        "game-modes" => game_modes::table(&t),
        "health" => health::table(&t),
        "multiplayer" => multiplayer::table(&t),
        "ores" => ores::table(&t),
        "progression" => progression::table(&t),
        "structures" => structures::table(&t),
        "trading" => trading::table(&t),
        "weather" => weather::table(&t),
        _ => Err(format!("no tables for the guide {guide}")),
    }
}

/// What a table is worked out with.
pub struct Table<'a> {
    /// Its name within its guide.
    pub name: &'a str,
    pub args: &'a [String],
    pub language: &'a str,
}

impl Table<'_> {
    /// The word `index` of the args, or an error naming what it is for.
    pub fn arg(&self, index: usize, what: &str) -> Result<&str, String> {
        self.args
            .get(index)
            .map(String::as_str)
            .ok_or_else(|| format!("{}: arg {index} ({what}) is missing", self.name))
    }

    /// The first `count` args, read as markup lines: the headings.
    pub fn head(&self, count: usize) -> Result<Vec<Line>, String> {
        (0..count)
            .map(|i| inline(self.arg(i, "a heading")?))
            .collect()
    }

    /// `template` (an arg, with `{name}` parameters) filled with `values`, read as markup.
    pub fn fill(&self, template: &str, values: &[(&str, String)]) -> Result<Line, String> {
        let mut text = template.to_string();
        for (name, value) in values {
            text = text.replace(&format!("{{{name}}}"), value);
        }
        inline(&text)
    }

    /// `value` with at most `decimals` decimals, without trailing zeros, the French way with a
    /// comma.
    pub fn number(&self, value: f64, decimals: usize) -> String {
        number(value, decimals, self.language)
    }

    /// `share` (0 to 1) as a percentage, as the wiki writes them: `25 %`, `3.3 %`, `0.25 %`.
    pub fn percent(&self, share: f64) -> String {
        let p = share * 100.0;
        let decimals = if p >= 10.0 || p.fract() == 0.0 {
            0
        } else if p < 1.0 {
            2
        } else {
            1
        };
        format!("{} %", self.number(p, decimals))
    }

    /// Seconds as a player reads them: `0.5 s`, `15 s`, `3 min`, `1 min 30 s`.
    pub fn duration(&self, seconds: f64) -> String {
        if seconds < 60.0 {
            return format!("{} s", self.number(seconds, 2));
        }
        let minutes = (seconds / 60.0).floor();
        let rest = (seconds - minutes * 60.0).round();
        if rest > 0.0 {
            format!("{minutes} min {rest} s")
        } else {
            format!("{minutes} min")
        }
    }

    /// An error for a table of the guide that does not exist.
    pub fn unknown(&self) -> Result<Vec<Block>, String> {
        Err(format!("no table {}", self.name))
    }
}

/// `value` with at most `decimals` decimals, without trailing zeros; a comma in French.
pub fn number(value: f64, decimals: usize, language: &str) -> String {
    let mut text = format!("{value:.decimals$}");
    if text.contains('.') {
        text = text.trim_end_matches('0').trim_end_matches('.').to_string();
    }
    if language == "fr" {
        text = text.replace('.', ",");
    }
    text
}

/// The text of `key` in the game's translation into `language` (`data/lang`), English when it
/// has none.
pub fn lang_text(language: &str, key: &str) -> Option<&'static str> {
    use std::collections::HashMap;
    use std::sync::OnceLock;
    type Texts = HashMap<String, String>;
    static TEXTS: OnceLock<(Texts, Texts)> = OnceLock::new();
    let (en, fr) = TEXTS.get_or_init(|| {
        let read = |source: &str| ron::from_str::<Texts>(source).expect("a language file reads");
        (
            read(include_str!("../../../../data/lang/en.ron")),
            read(include_str!("../../../../data/lang/fr.ron")),
        )
    });
    let fr = (language == "fr").then(|| fr.get(key)).flatten();
    fr.or_else(|| en.get(key)).map(String::as_str)
}

/// A cell of plain words.
pub fn words(text: impl Into<String>) -> Line {
    vec![Span::text(text)]
}

/// A cell linking to `item`, with `count` before it when more than one.
pub fn item(item: ItemId, count: u32) -> Line {
    let link = vec![Span::link(Target::Item(item))];
    if count > 1 {
        let mut line = words(format!("{count} "));
        line.extend(link);
        line
    } else {
        link
    }
}

/// A cell linking to the creature `kind`.
pub fn mob(kind: MobKind) -> Line {
    vec![Span::link(Target::Mob(kind))]
}

/// A table of `rows` under `head`, the columns `numeric` set right.
pub fn table(head: Vec<Line>, numeric: &[bool], rows: Vec<Vec<Line>>) -> Block {
    Block::Table {
        head: Some(head),
        numeric: numeric.to_vec(),
        rows,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_are_written_the_way_of_the_language() {
        assert_eq!(number(1.25, 2, "en"), "1.25");
        assert_eq!(number(1.25, 2, "fr"), "1,25");
        assert_eq!(number(4.0, 1, "en"), "4");
        assert_eq!(number(0.333, 1, "fr"), "0,3");
        let t = Table {
            name: "t",
            args: &[],
            language: "en",
        };
        assert_eq!(t.percent(0.05), "5 %");
        assert_eq!(t.percent(0.853), "85 %");
        assert_eq!(t.percent(0.0333), "3.3 %");
        assert_eq!(t.percent(0.0025), "0.25 %");
        assert_eq!(t.duration(90.0), "1 min 30 s");
        assert_eq!(t.duration(0.5), "0.5 s");
        assert!(t.arg(0, "a heading").is_err());
    }
}
