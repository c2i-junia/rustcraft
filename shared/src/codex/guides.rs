//! The guides of the Codex, in `data/codex/guides/<slug>.<language>.md`: the game shows them,
//! and the wiki renders them from its export. A guide not yet translated is shown in English.

/// Each guide, in the order of the wiki: its slug, its English text and its French one, empty
/// while it has none.
pub const GUIDES: &[(&str, &str, &str)] = &[
    guide!("first-day"),
    guide!("controls"),
    guide!("game-modes"),
    guide!("progression"),
    guide!("depths"),
    guide!("ores"),
    guide!("structures"),
    guide!("weather"),
    guide!("animals"),
    guide!("trading"),
    guide!("ember-mother"),
    guide!("health"),
    guide!("combat"),
    guide!("farming"),
    guide!("fishing"),
    guide!("multiplayer"),
    guide!("commands"),
];

macro_rules! guide {
    ($slug:literal) => {
        (
            $slug,
            include_str!(concat!("../../../data/codex/guides/", $slug, ".en.md")),
            include_str!(concat!("../../../data/codex/guides/", $slug, ".fr.md")),
        )
    };
}
use guide;

/// The text of the guide `slug` in `language` (`en`, `fr`), English when it has none in it.
pub fn source(slug: &str, language: &str) -> Option<&'static str> {
    let &(_, en, fr) = GUIDES.iter().find(|(s, ..)| *s == slug)?;
    Some(if language == "fr" && !fr.trim().is_empty() {
        fr
    } else {
        en
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codex::markup::{parse, Block};
    use crate::codex::tables::expand;

    /// Every guide reads in each of its languages, starts with its title, and its tables are
    /// worked out; every file of the folder is a guide.
    #[test]
    fn every_guide_reads_and_works_out_its_tables() {
        for &(slug, en, fr) in GUIDES {
            for (language, text) in [("en", en), ("fr", fr)] {
                if text.trim().is_empty() {
                    assert_eq!(language, "fr", "{slug} has no English text");
                    continue;
                }
                let blocks = parse(text).unwrap_or_else(|e| panic!("{slug}.{language}: {e}"));
                assert!(
                    matches!(blocks.first(), Some(Block::Heading { level: 1, .. })),
                    "{slug}.{language} starts with its title"
                );
                for block in &blocks {
                    if let Block::Computed { name, args } = block {
                        if let Err(e) = expand(name, args, language) {
                            panic!("{slug}.{language}: {{{{table {name}}}}}: {e}");
                        }
                    }
                }
            }
        }
        let folder = concat!(env!("CARGO_MANIFEST_DIR"), "/../data/codex/guides");
        for file in std::fs::read_dir(folder).unwrap() {
            let name = file.unwrap().file_name().into_string().unwrap();
            let slug = name.split('.').next().unwrap();
            assert!(
                GUIDES.iter().any(|(s, ..)| *s == slug),
                "{name} is listed in GUIDES"
            );
        }
    }
}
