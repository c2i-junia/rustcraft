//! The syntax of the chat commands, and what the chat suggests for a command being typed. The
//! server reads the commands in `server::network::commands`, whose tests check that it answers
//! the usages of this module.

use std::ops::Range;

use crate::world::ItemId;

/// An argument of a command.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Arg {
    /// One of these words.
    Word(&'static [&'static str]),
    /// The name of an item, such as `oak_log`.
    Item,
    /// A whole number above 0.
    Count,
}

impl Arg {
    /// How the usage shows the argument: its word when it has a single one, else `<day|night>`,
    /// `<item>` or `<count>`.
    pub fn placeholder(&self) -> String {
        match self {
            Arg::Word([word]) => word.to_string(),
            Arg::Word(words) => format!("<{}>", words.join("|")),
            Arg::Item => "<item>".into(),
            Arg::Count => "<count>".into(),
        }
    }

    /// The values to suggest, lowercase: none for a count.
    fn values(&self) -> Vec<String> {
        match self {
            Arg::Word(words) => words.iter().map(|word| word.to_string()).collect(),
            Arg::Item => ItemId::every().iter().map(ItemId::name).collect(),
            Arg::Count => Vec::new(),
        }
    }

    /// Whether the server reads `word` as a value of the argument.
    fn accepts(&self, word: &str) -> bool {
        match self {
            Arg::Word(words) => words.contains(&word),
            Arg::Item => ItemId::from_name(word).is_some(),
            Arg::Count => word.parse::<u32>().is_ok_and(|count| count > 0),
        }
    }
}

/// A command: its name, typed after the `/`, then its arguments.
#[derive(Debug, PartialEq)]
pub struct CommandSyntax {
    pub name: &'static str,
    pub args: &'static [Arg],
}

impl CommandSyntax {
    /// The command and the placeholders of its arguments, word by word: `/give`, `<item>`,
    /// `<count>`.
    pub fn usage_words(&self) -> Vec<String> {
        let name = format!("/{}", self.name);
        std::iter::once(name)
            .chain(self.args.iter().map(Arg::placeholder))
            .collect()
    }

    /// `/give <item> <count>`
    pub fn usage(&self) -> String {
        self.usage_words().join(" ")
    }

    /// The answer of the server to a wrong `word` for `arg`.
    fn error(&self, arg: &Arg, word: &str) -> String {
        match arg {
            Arg::Word(_) => format!("Usage: {}", self.usage()),
            Arg::Item => format!("Unknown item: {word}"),
            Arg::Count => format!("Invalid count: {word}"),
        }
    }
}

/// Every command, by name.
pub const COMMANDS: &[CommandSyntax] = &[
    CommandSyntax {
        name: "gamemode",
        args: &[Arg::Word(&["creative", "survival"])],
    },
    CommandSyntax {
        name: "give",
        args: &[Arg::Item, Arg::Count],
    },
    CommandSyntax {
        name: "time",
        args: &[Arg::Word(&["set"]), Arg::Word(&["day", "night"])],
    },
];

/// A value the word being typed may become.
#[derive(Debug, PartialEq)]
pub struct Suggestion {
    pub text: String,
    /// The bytes of `text` that the word typed matches.
    pub matched: Range<usize>,
    /// The arguments that follow a command name, such as `<item> <count>`; empty for the other
    /// words.
    pub detail: String,
}

/// What the chat shows for a command being typed.
#[derive(Debug, PartialEq, Default)]
pub struct Completion {
    /// Where the word being typed starts in the line, in bytes: a suggestion replaces the word.
    pub word_start: usize,
    /// The values that start with the word, then those that contain it, each in alphabetical
    /// order. None when the word is a value already, and the only one that matches.
    pub suggestions: Vec<Suggestion>,
    /// The usage of the command, word by word, once its name is known.
    pub usage: Vec<String>,
    /// The word of `usage` being typed, if any.
    pub current: Option<usize>,
    /// Whether an argument follows the word typed: a suggestion taken brings a space with it.
    pub more: bool,
    /// What the server would answer to the line, when it is already wrong.
    pub error: Option<String>,
}

/// What to suggest for `line`, the text of the chat before the caret, or `None` when it is not a
/// command. `mod_commands` are the commands the mods of the server add: their arguments are not
/// known, so any is accepted.
pub fn complete(line: &str, mod_commands: &[String]) -> Option<Completion> {
    let typed = line.strip_prefix('/')?;
    let word_start = typed
        .char_indices()
        .rev()
        .find(|(_, c)| c.is_whitespace())
        .map_or(0, |(at, c)| at + c.len_utf8());
    let word = &typed[word_start..];
    let before: Vec<&str> = typed[..word_start].split_whitespace().collect();
    let mut completion = Completion {
        word_start: word_start + 1,
        ..Completion::default()
    };

    let Some((&name, typed_args)) = before.split_first() else {
        let names = COMMANDS
            .iter()
            .map(|command| {
                let detail = command.usage_words()[1..].join(" ");
                (command.name.to_string(), detail)
            })
            .chain(
                mod_commands
                    .iter()
                    .map(|name| (name.clone(), String::new())),
            );
        completion.suggestions = suggest(word, names);
        // every command but one typed in full takes arguments
        completion.more = find(word).is_none_or(|command| !command.args.is_empty());
        if let Some(command) = find(word) {
            completion.usage = command.usage_words();
            completion.current = Some(0);
        } else if mod_commands.iter().any(|name| name == word) {
            completion.usage = vec![format!("/{word}")];
            completion.current = Some(0);
        } else if completion.suggestions.is_empty() && !word.is_empty() {
            completion.error = Some(format!("Unknown command: /{word}"));
        }
        return Some(completion);
    };
    let Some(command) = find(name) else {
        if mod_commands.iter().any(|mod_command| mod_command == name) {
            completion.usage = vec![format!("/{name}")];
        } else {
            completion.error = Some(format!("Unknown command: /{name}"));
        }
        return Some(completion);
    };
    completion.usage = command.usage_words();
    for (arg, typed) in command.args.iter().zip(typed_args) {
        if !arg.accepts(typed) {
            completion.error = Some(command.error(arg, typed));
            return Some(completion);
        }
    }
    let Some(arg) = command.args.get(typed_args.len()) else {
        if !word.is_empty() {
            completion.error = Some(format!("Usage: {}", command.usage()));
        }
        return Some(completion);
    };
    completion.current = Some(typed_args.len() + 1);
    completion.more = typed_args.len() + 1 < command.args.len();
    completion.suggestions = suggest(word, arg.values().into_iter().map(|v| (v, String::new())));
    if completion.suggestions.is_empty() && !word.is_empty() && !arg.accepts(word) {
        completion.error = Some(command.error(arg, word));
    }
    Some(completion)
}

/// The command called `name`.
fn find(name: &str) -> Option<&'static CommandSyntax> {
    COMMANDS.iter().find(|command| command.name == name)
}

/// The `(value, detail)` of `values`, lowercase, that contain `word` in any case: those that start
/// with it first.
fn suggest(word: &str, values: impl IntoIterator<Item = (String, String)>) -> Vec<Suggestion> {
    let word = word.to_lowercase();
    let (mut starting, mut containing): (Vec<Suggestion>, Vec<Suggestion>) = values
        .into_iter()
        .filter_map(|(text, detail)| {
            let at = text.find(&word)?;
            Some(Suggestion {
                matched: at..at + word.len(),
                text,
                detail,
            })
        })
        .partition(|suggestion| suggestion.matched.start == 0);
    starting.sort_by(|a, b| a.text.cmp(&b.text));
    containing.sort_by(|a, b| a.text.cmp(&b.text));
    starting.append(&mut containing);
    if let [only] = &starting[..] {
        if only.text == word {
            starting.clear();
        }
    }
    starting
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts(completion: &Completion) -> Vec<&str> {
        completion
            .suggestions
            .iter()
            .map(|suggestion| suggestion.text.as_str())
            .collect()
    }

    #[test]
    fn only_a_line_starting_with_a_slash_is_completed() {
        assert_eq!(complete("hello", &[]), None);
        assert_eq!(complete(" /time", &[]), None);
        assert!(complete("/", &[]).is_some());
    }

    #[test]
    fn a_slash_alone_suggests_every_command_with_its_arguments() {
        let completion = complete("/", &[]).unwrap();
        assert_eq!(completion.word_start, 1);
        assert_eq!(texts(&completion), ["gamemode", "give", "time"]);
        assert_eq!(completion.suggestions[0].detail, "<creative|survival>");
        assert_eq!(completion.suggestions[1].detail, "<item> <count>");
        assert_eq!(completion.suggestions[2].detail, "set <day|night>");
        assert_eq!(completion.error, None);
        assert!(completion.usage.is_empty());
    }

    #[test]
    fn the_name_of_a_command_is_completed_from_its_start() {
        let completion = complete("/ti", &[]).unwrap();
        assert_eq!(texts(&completion), ["time"]);
        assert_eq!(completion.suggestions[0].matched, 0..2);
        // typed in full, it is not suggested again, and its usage shows
        let completion = complete("/time", &[]).unwrap();
        assert!(completion.suggestions.is_empty());
        assert_eq!(completion.usage, ["/time", "set", "<day|night>"]);
        assert_eq!(completion.current, Some(0));
        assert!(completion.more);
    }

    #[test]
    fn an_unknown_command_is_told_as_it_is_typed() {
        let completion = complete("/fly", &[]).unwrap();
        assert!(completion.suggestions.is_empty());
        assert_eq!(completion.error.as_deref(), Some("Unknown command: /fly"));
        let completion = complete("/fly high", &[]).unwrap();
        assert_eq!(completion.error.as_deref(), Some("Unknown command: /fly"));
    }

    #[test]
    fn the_commands_of_the_mods_are_suggested_and_take_any_argument() {
        let mods = ["tower".to_string()];
        assert!(texts(&complete("/", &mods).unwrap()).contains(&"tower"));
        assert_eq!(texts(&complete("/to", &mods).unwrap()), ["tower"]);
        let completion = complete("/tower", &mods).unwrap();
        assert_eq!(completion.error, None);
        assert_eq!(completion.usage, ["/tower"]);
        let completion = complete("/tower 12 high", &mods).unwrap();
        assert_eq!(completion.error, None);
        assert_eq!(completion.usage, ["/tower"]);
    }

    #[test]
    fn the_arguments_are_suggested_in_turn() {
        let completion = complete("/time ", &[]).unwrap();
        assert_eq!(completion.word_start, 6);
        assert_eq!(texts(&completion), ["set"]);
        assert_eq!(completion.current, Some(1));
        assert!(completion.more);

        let completion = complete("/time set n", &[]).unwrap();
        assert_eq!(completion.word_start, 10);
        assert_eq!(texts(&completion), ["night"]);
        assert_eq!(completion.current, Some(2));
        assert!(!completion.more);
    }

    #[test]
    fn items_that_start_with_the_word_come_before_those_that_contain_it() {
        let completion = complete("/give LOG", &[]).unwrap();
        assert_eq!(texts(&completion), ["oak_log", "spruce_log"]);
        assert_eq!(completion.suggestions[0].matched, 4..7);
        let completion = complete("/give s", &[]).unwrap();
        assert_eq!(
            texts(&completion)[..6],
            [
                "sand",
                "sheep_spawn_egg",
                "snow",
                "snowball",
                "spruce_log",
                "spruce_planks"
            ]
        );
        // `stone` is a value, and `cobblestone` contains it
        assert_eq!(
            texts(&complete("/give stone", &[]).unwrap()),
            ["stone", "cobblestone"]
        );
        assert_eq!(complete("/give", &[]).unwrap().suggestions, []);
        assert_eq!(
            complete("/give ", &[]).unwrap().suggestions.len(),
            ItemId::ALL.len()
        );
    }

    #[test]
    fn a_count_is_not_suggested_but_its_place_is_shown() {
        let completion = complete("/give dirt ", &[]).unwrap();
        assert!(completion.suggestions.is_empty());
        assert_eq!(completion.usage, ["/give", "<item>", "<count>"]);
        assert_eq!(completion.current, Some(2));
        assert_eq!(completion.error, None);
        assert_eq!(complete("/give dirt 64", &[]).unwrap().error, None);
    }

    #[test]
    fn a_wrong_argument_is_told_with_the_answer_of_the_server() {
        let error = |line| complete(line, &[]).unwrap().error;
        assert_eq!(
            error("/give diamond 3").as_deref(),
            Some("Unknown item: diamond")
        );
        assert_eq!(error("/give xyz").as_deref(), Some("Unknown item: xyz"));
        assert_eq!(error("/give dirt 0").as_deref(), Some("Invalid count: 0"));
        assert_eq!(error("/give dirt x").as_deref(), Some("Invalid count: x"));
        let usage = Some("Usage: /time set <day|night>");
        assert_eq!(error("/time add").as_deref(), usage);
        assert_eq!(error("/time set noon").as_deref(), usage);
        assert_eq!(error("/time set day 1").as_deref(), usage);
        // a word typed in full, with nothing after it, is fine
        assert_eq!(error("/time set day "), None);
        assert_eq!(error("/time set da"), None);
    }
}
