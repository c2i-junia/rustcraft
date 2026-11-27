//! The markup of the texts of the Codex, a small part of Markdown that the game and the wiki
//! both draw:
//!
//! - `# Title` and `## Section`, `### Part`;
//! - paragraphs, apart by a blank line, their lines joined;
//! - `- item` and `1. item` lists, a line that starts with neither going on with the last item;
//! - `| a | b |` tables, a `|---|---:|` line under the first row making it the head (`:` at the
//!   end: a column of numbers, set right);
//! - `{{recipe Furnace}}`, the recipes of an item, and `{{table name}}`, a table the game works
//!   out (`codex::tables`), each alone on its line;
//! - in the text, `*emphasis*`, `**strong**`, and links: `[[Furnace]]`, `[[mob:Wolf]]`,
//!   `[[biome:plains]]`, `[[effect:Poison]]`, `[[guide:farming]]`, `[[page:smelting]]`, each
//!   with its own name unless `[[Furnace|a furnace]]` gives one.

use serde::Serialize;

use crate::players::effects::EffectKind;
use crate::world::{ItemId, MobKind};

/// What a link opens.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", content = "id", rename_all = "lowercase")]
pub enum Target {
    #[serde(serialize_with = "debug_name")]
    Item(ItemId),
    #[serde(serialize_with = "debug_name")]
    Mob(MobKind),
    /// A biome, by its key without `rustcraft:`.
    Biome(String),
    #[serde(serialize_with = "debug_name")]
    Effect(EffectKind),
    Guide(String),
    /// A page only the wiki has, by its path: `smelting`.
    Page(String),
}

fn debug_name<T: std::fmt::Debug, S: serde::Serializer>(v: &T, s: S) -> Result<S::Ok, S::Error> {
    s.serialize_str(&format!("{v:?}"))
}

/// The value of `list` whose name is `name`.
fn named<T: Copy + std::fmt::Debug>(list: &[T], name: &str) -> Option<T> {
    list.iter().copied().find(|v| format!("{v:?}") == name)
}

impl Target {
    /// The target `text` names, if it names one.
    pub fn parse(text: &str) -> Option<Target> {
        Some(match text.split_once(':') {
            None => Target::Item(named(&ItemId::ALL, text)?),
            Some(("mob", id)) => Target::Mob(named(&MobKind::ALL, id)?),
            Some(("effect", id)) => Target::Effect(named(&EffectKind::ALL, id)?),
            Some(("biome", key)) if !key.is_empty() => Target::Biome(key.into()),
            Some(("guide", slug)) if !slug.is_empty() => Target::Guide(slug.into()),
            Some(("page", path)) if !path.is_empty() => Target::Page(path.into()),
            _ => return None,
        })
    }
}

/// A run of text, all of one style.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Span {
    /// The words, `None` for a link drawn with the name of its target.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub em: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub strong: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub link: Option<Target>,
}

impl Span {
    pub fn text(text: impl Into<String>) -> Span {
        Span {
            text: Some(text.into()),
            em: false,
            strong: false,
            link: None,
        }
    }

    pub fn link(target: Target) -> Span {
        Span {
            text: None,
            em: false,
            strong: false,
            link: Some(target),
        }
    }
}

/// A line of text: its runs.
pub type Line = Vec<Span>;

/// A part of a page.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Block {
    Heading {
        level: u8,
        text: Line,
    },
    Paragraph {
        text: Line,
    },
    List {
        ordered: bool,
        items: Vec<Line>,
    },
    Table {
        head: Option<Vec<Line>>,
        numeric: Vec<bool>,
        rows: Vec<Vec<Line>>,
    },
    Recipe {
        item: ItemId,
    },
    Computed {
        name: String,
    },
}

/// A mistake of a text, with its line, counted from 1.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarkupError {
    pub line: usize,
    pub message: String,
}

impl std::fmt::Display for MarkupError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "line {}: {}", self.line, self.message)
    }
}

/// The blocks of `source`, or its first mistake: a link to nothing, an unclosed mark.
pub fn parse(source: &str) -> Result<Vec<Block>, MarkupError> {
    let mut blocks = Vec::new();
    let mut lines = source.lines().enumerate().peekable();
    while let Some((n, raw)) = lines.next() {
        let line = raw.trim();
        let err = |message: String| MarkupError {
            line: n + 1,
            message,
        };
        if line.is_empty() {
            continue;
        }
        if let Some(rest) = line.strip_prefix("{{").and_then(|l| l.strip_suffix("}}")) {
            blocks.push(match rest.trim().split_once(' ') {
                Some(("recipe", id)) => Block::Recipe {
                    item: match Target::parse(id.trim()) {
                        Some(Target::Item(item)) => item,
                        _ => return Err(err(format!("no item {id}"))),
                    },
                },
                Some(("table", name)) => Block::Computed {
                    name: name.trim().into(),
                },
                _ => return Err(err(format!("unknown block {{{{{rest}}}}}"))),
            });
            continue;
        }
        let hashes = line.bytes().take_while(|&b| b == b'#').count();
        if (1..=3).contains(&hashes) && line[hashes..].starts_with(' ') {
            blocks.push(Block::Heading {
                level: hashes as u8,
                text: inline(line[hashes..].trim()).map_err(err)?,
            });
            continue;
        }
        if line.starts_with('|') {
            let mut rows = vec![cells(line, n)?];
            let mut head = None;
            let mut numeric = Vec::new();
            while let Some((m, next)) = lines.peek().map(|&(m, l)| (m, l.trim())) {
                if !next.starts_with('|') {
                    break;
                }
                lines.next();
                if head.is_none() && rows.len() == 1 && is_rule(next) {
                    numeric = split_cells(next).map(|c| c.ends_with(':')).collect();
                    head = Some(rows.pop().unwrap_or_default());
                } else {
                    rows.push(cells(next, m)?);
                }
            }
            blocks.push(Block::Table {
                head,
                numeric,
                rows,
            });
            continue;
        }
        if let Some(ordered) = item_marker(line).map(|(o, _)| o) {
            let mut items = Vec::new();
            let mut current = item_marker(line)
                .map(|(_, t)| t.to_string())
                .unwrap_or_default();
            let mut start = n;
            while let Some(&(m, next)) = lines.peek() {
                let next = next.trim();
                let other_list = item_marker(next).is_some_and(|(o, _)| o != ordered);
                if next.is_empty() || next.starts_with('|') || next.starts_with('#') || other_list {
                    break;
                }
                lines.next();
                match item_marker(next) {
                    Some((o, text)) if o == ordered => {
                        items.push(inline(&current).map_err(|message| MarkupError {
                            line: start + 1,
                            message,
                        })?);
                        current = text.into();
                        start = m;
                    }
                    _ => {
                        current.push(' ');
                        current.push_str(next);
                    }
                }
            }
            items.push(inline(&current).map_err(|message| MarkupError {
                line: start + 1,
                message,
            })?);
            blocks.push(Block::List { ordered, items });
            continue;
        }
        let mut text = line.to_string();
        while let Some(&(_, next)) = lines.peek() {
            let next = next.trim();
            if next.is_empty()
                || next.starts_with('|')
                || next.starts_with("{{")
                || item_marker(next).is_some()
                || next.starts_with("# ")
                || next.starts_with("## ")
                || next.starts_with("### ")
            {
                break;
            }
            lines.next();
            text.push(' ');
            text.push_str(next);
        }
        blocks.push(Block::Paragraph {
            text: inline(&text).map_err(err)?,
        });
    }
    Ok(blocks)
}

/// Whether a line of a list starts, ordered or not, and its text.
fn item_marker(line: &str) -> Option<(bool, &str)> {
    if let Some(text) = line.strip_prefix("- ") {
        return Some((false, text.trim()));
    }
    let digits = line.bytes().take_while(u8::is_ascii_digit).count();
    (digits > 0)
        .then(|| line[digits..].strip_prefix(". "))
        .flatten()
        .map(|text| (true, text.trim()))
}

fn split_cells(line: &str) -> impl Iterator<Item = &str> {
    let inner = line.trim().trim_start_matches('|');
    let inner = inner.strip_suffix('|').unwrap_or(inner);
    inner.split('|').map(str::trim)
}

fn is_rule(line: &str) -> bool {
    split_cells(line).all(|c| !c.is_empty() && c.chars().all(|ch| ch == '-' || ch == ':'))
}

fn cells(line: &str, n: usize) -> Result<Vec<Line>, MarkupError> {
    split_cells(line)
        .map(|c| {
            inline(c).map_err(|message| MarkupError {
                line: n + 1,
                message,
            })
        })
        .collect()
}

/// The runs of a line of text.
pub fn inline(text: &str) -> Result<Line, String> {
    let mut spans: Line = Vec::new();
    let (mut em, mut strong) = (false, false);
    let mut word = String::new();
    let flush = |word: &mut String, spans: &mut Line, em: bool, strong: bool| {
        if !word.is_empty() {
            spans.push(Span {
                text: Some(std::mem::take(word)),
                em,
                strong,
                link: None,
            });
        }
    };
    let mut rest = text;
    while let Some(ch) = rest.chars().next() {
        if let Some(after) = rest.strip_prefix("[[") {
            let end = after
                .find("]]")
                .ok_or_else(|| format!("unclosed link in {text:?}"))?;
            let inside = &after[..end];
            let (target, label) = match inside.split_once('|') {
                Some((t, l)) => (t.trim(), Some(l.trim().to_string())),
                None => (inside.trim(), None),
            };
            let target =
                Target::parse(target).ok_or_else(|| format!("a link to nothing: [[{inside}]]"))?;
            flush(&mut word, &mut spans, em, strong);
            spans.push(Span {
                text: label,
                em,
                strong,
                link: Some(target),
            });
            rest = &after[end + 2..];
        } else if let Some(after) = rest.strip_prefix("**") {
            flush(&mut word, &mut spans, em, strong);
            strong = !strong;
            rest = after;
        } else if let Some(after) = rest.strip_prefix('*') {
            flush(&mut word, &mut spans, em, strong);
            em = !em;
            rest = after;
        } else {
            word.push(ch);
            rest = &rest[ch.len_utf8()..];
        }
    }
    if em || strong {
        return Err(format!("unclosed * in {text:?}"));
    }
    flush(&mut word, &mut spans, em, strong);
    Ok(spans)
}

/// The words of `line` without their marks, links named by `name`.
pub fn plain(line: &[Span], name: impl Fn(&Target) -> String) -> String {
    line.iter()
        .map(|s| match (&s.text, &s.link) {
            (Some(text), _) => text.clone(),
            (None, Some(target)) => name(target),
            (None, None) => String::new(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_text_reads_into_its_blocks() {
        let source = "# Farming\n\nThree crops grow on [[Dirt|tilled soil]]: *rye*,\nand **more**.\n\n## Seeds\n- Break [[TallGrass]].\n- Or trade with the [[mob:Farmer]]\n  in a village.\n1. One\n2. Two\n\n| Crop | Minutes |\n|---|---:|\n| [[Rye]] | 4 |\n\n{{recipe Furnace}}\n{{table crops}}\n";
        let blocks = parse(source).unwrap();
        assert_eq!(blocks.len(), 8, "{blocks:#?}");
        assert!(matches!(&blocks[0], Block::Heading { level: 1, .. }));
        let Block::Paragraph { text } = &blocks[1] else {
            panic!()
        };
        assert_eq!(text[1].link, Some(Target::Item(ItemId::Dirt)));
        assert_eq!(text[1].text.as_deref(), Some("tilled soil"));
        assert!(text[3].em && !text[3].strong);
        assert!(text
            .iter()
            .any(|s| s.strong && s.text.as_deref() == Some("more")));
        let Block::List {
            ordered: false,
            items,
        } = &blocks[3]
        else {
            panic!()
        };
        assert_eq!(items.len(), 2);
        assert_eq!(
            plain(&items[1], |_| "Farmer".into()),
            "Or trade with the Farmer in a village."
        );
        assert!(matches!(&blocks[4], Block::List { ordered: true, items } if items.len() == 2));
        let Block::Table {
            head,
            numeric,
            rows,
        } = &blocks[5]
        else {
            panic!()
        };
        assert!(head.is_some());
        assert_eq!(numeric, &[false, true]);
        assert_eq!(rows.len(), 1);
        assert_eq!(
            blocks[6],
            Block::Recipe {
                item: ItemId::Furnace
            }
        );
        assert_eq!(
            blocks[7],
            Block::Computed {
                name: "crops".into()
            }
        );
    }

    #[test]
    fn a_link_to_nothing_and_an_unclosed_mark_are_mistakes() {
        assert_eq!(parse("ok\n\nsee [[Nothing]]").unwrap_err().line, 3);
        assert!(parse("*open").is_err());
        assert!(parse("[[Furnace").is_err());
        assert!(parse("{{recipe Nothing}}").is_err());
    }
}
