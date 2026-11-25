//! The chat commands: a chat message that starts with `/` is run by the server for its author,
//! who alone gets the answer, instead of being relayed.

use bevy::math::Vec3;
use shared::players::game_mode::GameMode;
use shared::players::Player;
use shared::world::hostiles::Difficulty;
use shared::world::{ItemId, ItemStack, WeatherKind};
use shared::{DAY_DURATION_IN_TICKS, MIDNIGHT_IN_DAY, NOON_IN_DAY};

use crate::init::ServerTime;
use crate::network::permissions::{Admin, WhitelistAction};
use crate::world::generation::dungeon::DungeonKind;
use crate::world::weather::ServerWeather;

/// The author of the answers to a command.
pub const SERVER_AUTHOR: &str = "Server";

const TIME_USAGE: &str = "Usage: /time set <day|night>";
const GIVE_USAGE: &str = "Usage: /give <item> <count>";
const GAMEMODE_USAGE: &str = "Usage: /gamemode <creative|survival>";
const WEATHER_USAGE: &str = "Usage: /weather <clear|rain|thunder>";
const LOCATE_USAGE: &str = "Usage: /locate <biome|village|ruin|crypt|sanctuary> <biome>";
const DIFFICULTY_USAGE: &str = "Usage: /difficulty <peaceful|easy|normal|hard>";
const OP_USAGE: &str = "Usage: /op <player>";
const DEOP_USAGE: &str = "Usage: /deop <player>";
const KICK_USAGE: &str = "Usage: /kick <player> [reason]";
const BAN_USAGE: &str = "Usage: /ban <player> [reason]";
const PARDON_USAGE: &str = "Usage: /pardon <player>";
const WHITELIST_USAGE: &str = "Usage: /whitelist <on|off|add|remove|list> <player>";
const LIST_USAGE: &str = "Usage: /list";

/// The commands every player may run on a dedicated server; the others are the operators'.
/// `sign` is not typed by the player: the client sends it when a sign is written.
pub const OPEN_COMMANDS: &[&str] = &["list", "sign"];

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TimeOfDay {
    /// Noon.
    Day,
    /// Midnight.
    Night,
}

#[derive(Debug, PartialEq)]
pub enum Command {
    /// `/time set <day|night>`
    SetTime(TimeOfDay),
    /// `/give <item> <count>`: to the author of the command.
    Give { item: ItemId, count: u32 },
    /// `/gamemode <creative|survival>`: of the author of the command.
    SetGameMode(GameMode),
    /// `/weather <clear|rain|thunder>`: for as long as it would last on its own.
    SetWeather(WeatherKind),
    /// `/locate biome <biome>`, `/locate village` or `/locate` and a kind of dungeon: the nearest
    /// column of the biome, village or dungeon to the author of the command.
    Locate(Place),
    /// `/difficulty <peaceful|easy|normal|hard>`: of the world.
    SetDifficulty(Difficulty),
    /// `/list`: the players online.
    List,
    /// A command on who may do what (`permissions::Permissions::apply`).
    Admin(Admin),
}

/// The answer to a player who may not run the command `name`, or `None` when the player may.
/// `known` tells whether the command exists, the base game's or a mod's: an unknown command is
/// answered as unknown, whoever types it. Every command but those of `OPEN_COMMANDS` is an
/// operator's, the commands of the mods included: they change the world.
pub fn refusal(name: &str, known: bool, is_operator: bool) -> Option<String> {
    (known && !is_operator && !OPEN_COMMANDS.contains(&name))
        .then(|| format!("You must be an operator to use /{name}"))
}

/// The answer to `/list`: the names of the players online, sorted.
pub fn list_players(mut names: Vec<&str>) -> String {
    names.sort_unstable();
    match names.len() {
        1 => format!("1 player online: {}", names[0]),
        count => format!("{count} players online: {}", names.join(", ")),
    }
}

/// The name of the player that starts `text`, and what follows it: one word, or anything between
/// quotes, as a name may hold spaces (`"Bad Guy"`; a quote of the name is typed `\"`). `None`
/// when there is no name.
fn player_name(text: &str) -> Option<(String, &str)> {
    let text = text.trim_start();
    let Some(quoted) = text.strip_prefix('"') else {
        let end = text.find(char::is_whitespace).unwrap_or(text.len());
        let (name, rest) = text.split_at(end);
        return (!name.is_empty()).then(|| (name.to_string(), rest));
    };
    let mut name = String::new();
    let mut chars = quoted.char_indices();
    while let Some((at, c)) = chars.next() {
        match c {
            '"' => {
                let (name, rest) = (name.trim().to_string(), &quoted[at + 1..]);
                let ends = rest.is_empty() || rest.starts_with(char::is_whitespace);
                return (ends && !name.is_empty()).then_some((name, rest));
            }
            '\\' => name.push(chars.next()?.1),
            c => name.push(c),
        }
    }
    None
}

/// The name of the player that is all of `text`.
fn only_player(text: &str) -> Option<String> {
    player_name(text)
        .filter(|(_, rest)| rest.trim().is_empty())
        .map(|(name, _)| name)
}

/// The name of the player that starts `text`, and the reason that is the rest of it, if any.
fn player_and_reason(text: &str) -> Option<(String, Option<String>)> {
    let (name, rest) = player_name(text)?;
    let reason = rest.trim();
    Some((name, (!reason.is_empty()).then(|| reason.to_string())))
}

/// `line` without its first `count` words.
fn after_words(line: &str, count: usize) -> &str {
    let mut rest = line.trim_start();
    for _ in 0..count {
        rest = rest
            .split_once(char::is_whitespace)
            .map_or("", |(_, rest)| rest)
            .trim_start();
    }
    rest
}

/// What `/locate` looks for.
#[derive(Debug, Clone, PartialEq)]
pub enum Place {
    /// A biome, by its key.
    Biome(String),
    Village,
    Dungeon(DungeonKind),
}

/// The command in `message`, without its `/`, or `None` when it is an ordinary message.
pub fn command_line(message: &str) -> Option<&str> {
    message.strip_prefix('/')
}

/// Reads a command line, without its `/`. The error is the answer for the player.
pub fn parse_command(line: &str) -> Result<Command, String> {
    let words: Vec<&str> = line.split_whitespace().collect();
    match words[..] {
        ["time", ..] => match words[1..] {
            ["set", "day"] => Ok(Command::SetTime(TimeOfDay::Day)),
            ["set", "night"] => Ok(Command::SetTime(TimeOfDay::Night)),
            _ => Err(TIME_USAGE.to_string()),
        },
        ["give", item, count] => {
            let item = ItemId::from_name(item).ok_or_else(|| format!("Unknown item: {item}"))?;
            let count = count
                .parse()
                .ok()
                .filter(|&count| count > 0)
                .ok_or_else(|| format!("Invalid count: {count}"))?;
            Ok(Command::Give { item, count })
        }
        ["give", ..] => Err(GIVE_USAGE.to_string()),
        ["gamemode", mode] => GameMode::from_name(mode)
            .map(Command::SetGameMode)
            .ok_or_else(|| GAMEMODE_USAGE.to_string()),
        ["gamemode", ..] => Err(GAMEMODE_USAGE.to_string()),
        ["weather", kind] => WeatherKind::from_name(kind)
            .map(Command::SetWeather)
            .ok_or_else(|| WEATHER_USAGE.to_string()),
        ["weather", ..] => Err(WEATHER_USAGE.to_string()),
        ["locate", "biome", biome] => Ok(Command::Locate(Place::Biome(biome.to_string()))),
        ["locate", "village"] => Ok(Command::Locate(Place::Village)),
        ["locate", place] => DungeonKind::from_name(place)
            .map(|kind| Command::Locate(Place::Dungeon(kind)))
            .ok_or_else(|| LOCATE_USAGE.to_string()),
        ["locate", ..] => Err(LOCATE_USAGE.to_string()),
        ["difficulty", difficulty] => Difficulty::from_name(difficulty)
            .map(Command::SetDifficulty)
            .ok_or_else(|| DIFFICULTY_USAGE.to_string()),
        ["difficulty", ..] => Err(DIFFICULTY_USAGE.to_string()),
        ["list"] => Ok(Command::List),
        ["list", ..] => Err(LIST_USAGE.to_string()),
        ["op", ..] => only_player(after_words(line, 1))
            .map(|name| Command::Admin(Admin::Op(name)))
            .ok_or_else(|| OP_USAGE.to_string()),
        ["deop", ..] => only_player(after_words(line, 1))
            .map(|name| Command::Admin(Admin::Deop(name)))
            .ok_or_else(|| DEOP_USAGE.to_string()),
        ["kick", ..] => player_and_reason(after_words(line, 1))
            .map(|(name, reason)| Command::Admin(Admin::Kick { name, reason }))
            .ok_or_else(|| KICK_USAGE.to_string()),
        ["ban", ..] => player_and_reason(after_words(line, 1))
            .map(|(name, reason)| Command::Admin(Admin::Ban { name, reason }))
            .ok_or_else(|| BAN_USAGE.to_string()),
        ["pardon", ..] => only_player(after_words(line, 1))
            .map(|name| Command::Admin(Admin::Pardon(name)))
            .ok_or_else(|| PARDON_USAGE.to_string()),
        ["whitelist", ..] => match words[1..] {
            ["on"] => Some(WhitelistAction::On),
            ["off"] => Some(WhitelistAction::Off),
            ["list"] => Some(WhitelistAction::List),
            ["add", ..] => only_player(after_words(line, 2)).map(WhitelistAction::Add),
            ["remove", ..] => only_player(after_words(line, 2)).map(WhitelistAction::Remove),
            _ => None,
        }
        .map(|action| Command::Admin(Admin::Whitelist(action)))
        .ok_or_else(|| WHITELIST_USAGE.to_string()),
        _ => Err(format!("Unknown command: /{line}")),
    }
}

/// Runs `command`, a command of the game (not `List` nor `Admin`, which the handler runs with
/// the lobby and the permissions), for `player`, its author, and returns the answer. `locate`
/// starts the search of a biome by its key from the position of the player, and returns the
/// answer to give at once (`Locator::start`). A world made peaceful loses its hostiles on the
/// next tick (`mob::spawning`).
pub fn run_command(
    command: Command,
    (time, weather, difficulty): (&mut ServerTime, &mut ServerWeather, &mut Difficulty),
    player: &mut Player,
    locate: impl FnOnce(Place, Vec3) -> String,
) -> String {
    match command {
        Command::SetTime(time_of_day) => {
            time.0 = next_tick_at(time.0, time_of_day);
            let name = match time_of_day {
                TimeOfDay::Day => "day",
                TimeOfDay::Night => "night",
            };
            format!("Time set to {name}")
        }
        Command::Give { item, count } => {
            let lost = player.inventory.add_item_to_inventory(ItemStack {
                item_id: item,
                item_type: item.get_default_type(),
                nb: count,
            });
            let given = count - lost;
            if lost == 0 {
                format!("Gave {given} {item:?}")
            } else {
                format!("Gave {given} {item:?}, {lost} did not fit in the inventory")
            }
        }
        Command::SetGameMode(mode) => {
            // The simulation lands a player that may no longer fly
            player.game_mode = mode;
            format!("Game mode set to {}", mode.name())
        }
        Command::SetWeather(kind) => {
            weather.set(kind, &mut rand::thread_rng());
            format!("Weather set to {}", kind.name())
        }
        Command::Locate(place) => locate(place, player.position),
        Command::SetDifficulty(new) => {
            *difficulty = new;
            format!("Difficulty set to {}", new.name())
        }
        // run by the handler
        Command::List | Command::Admin(_) => String::new(),
    }
}

/// The first tick from `now` on at `time_of_day`. The clock only moves forward: the autosave
/// and the spawning of the mobs count on it.
pub fn next_tick_at(now: u64, time_of_day: TimeOfDay) -> u64 {
    let in_day = match time_of_day {
        TimeOfDay::Day => NOON_IN_DAY,
        TimeOfDay::Night => MIDNIGHT_IN_DAY,
    };
    let start_of_day = now - now % DAY_DURATION_IN_TICKS;
    let tick = start_of_day + in_day;
    if tick < now {
        tick + DAY_DURATION_IN_TICKS
    } else {
        tick
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_messages_starting_with_a_slash_are_commands() {
        assert_eq!(command_line("/time set day"), Some("time set day"));
        assert_eq!(command_line("hello /time set day"), None);
        assert_eq!(command_line(" /time set day"), None);
    }

    #[test]
    fn time_set_takes_day_or_night() {
        assert_eq!(
            parse_command("time set day"),
            Ok(Command::SetTime(TimeOfDay::Day))
        );
        assert_eq!(
            parse_command("time  set   night "),
            Ok(Command::SetTime(TimeOfDay::Night))
        );
        for wrong in [
            "time",
            "time set",
            "time set noon",
            "time add day",
            "time set day 1",
        ] {
            assert_eq!(
                parse_command(wrong),
                Err("Usage: /time set <day|night>".to_string()),
                "{wrong}"
            );
        }
    }

    #[test]
    fn give_takes_an_item_and_a_count() {
        let give = |item, count| Ok(Command::Give { item, count });
        assert_eq!(parse_command("give dirt 3"), give(ItemId::Dirt, 3));
        assert_eq!(parse_command("give oak_log 64"), give(ItemId::OakLog, 64));
        assert_eq!(parse_command("give OAK_LOG 1"), give(ItemId::OakLog, 1));
        assert_eq!(parse_command("give OakLog 1"), give(ItemId::OakLog, 1));
        assert_eq!(
            parse_command("give crafting_table 2"),
            give(ItemId::CraftingTable, 2)
        );
        let usage = Err("Usage: /give <item> <count>".to_string());
        for wrong in ["give", "give dirt", "give dirt 3 4"] {
            assert_eq!(parse_command(wrong), usage, "{wrong}");
        }
    }

    #[test]
    fn give_refuses_unknown_items_and_counts_that_are_not_positive() {
        assert_eq!(
            parse_command("give emerald 1"),
            Err("Unknown item: emerald".to_string())
        );
        // Only the names of the items, not anything serde would accept
        assert_eq!(
            parse_command("give oak__log 1"),
            Err("Unknown item: oak__log".to_string())
        );
        for count in ["0", "-1", "two", "99999999999"] {
            assert_eq!(
                parse_command(&format!("give dirt {count}")),
                Err(format!("Invalid count: {count}")),
                "{count}"
            );
        }
    }

    #[test]
    fn the_usages_are_those_the_chat_shows() {
        let usage = |name| {
            let command = shared::commands::COMMANDS
                .iter()
                .find(|command| command.name == name)
                .unwrap();
            format!("Usage: {}", command.usage())
        };
        assert_eq!(shared::commands::COMMANDS.len(), 13);
        assert_eq!(OP_USAGE, usage("op"));
        assert_eq!(DEOP_USAGE, usage("deop"));
        assert_eq!(KICK_USAGE, usage("kick"));
        assert_eq!(BAN_USAGE, usage("ban"));
        assert_eq!(PARDON_USAGE, usage("pardon"));
        assert_eq!(WHITELIST_USAGE, usage("whitelist"));
        assert_eq!(LIST_USAGE, usage("list"));
        assert_eq!(DIFFICULTY_USAGE, usage("difficulty"));
        assert_eq!(LOCATE_USAGE, usage("locate"));
        assert_eq!(TIME_USAGE, usage("time"));
        assert_eq!(WEATHER_USAGE, usage("weather"));
        assert_eq!(GIVE_USAGE, usage("give"));
        assert_eq!(GAMEMODE_USAGE, usage("gamemode"));
    }

    #[test]
    fn gamemode_takes_creative_or_survival() {
        assert_eq!(
            parse_command("gamemode creative"),
            Ok(Command::SetGameMode(GameMode::Creative))
        );
        assert_eq!(
            parse_command("gamemode Survival"),
            Ok(Command::SetGameMode(GameMode::Survival))
        );
        for wrong in ["gamemode", "gamemode adventure", "gamemode creative now"] {
            assert_eq!(
                parse_command(wrong),
                Err(GAMEMODE_USAGE.to_string()),
                "{wrong}"
            );
        }
    }

    #[test]
    fn gamemode_changes_the_mode_of_its_author() {
        let mut player = Player::default();
        let answer = run_command(
            Command::SetGameMode(GameMode::Creative),
            (
                &mut ServerTime(0),
                &mut ServerWeather::new(&mut rand::thread_rng()),
                &mut Difficulty::Normal,
            ),
            &mut player,
            |_, _| unreachable!(),
        );
        assert_eq!(player.game_mode, GameMode::Creative);
        assert_eq!(answer, "Game mode set to creative");
    }

    #[test]
    fn difficulty_takes_a_name_and_sets_that_of_the_world() {
        assert_eq!(
            parse_command("difficulty peaceful"),
            Ok(Command::SetDifficulty(Difficulty::Peaceful))
        );
        for wrong in ["difficulty", "difficulty extreme", "difficulty normal now"] {
            assert_eq!(
                parse_command(wrong),
                Err(DIFFICULTY_USAGE.to_string()),
                "{wrong}"
            );
        }
        let mut difficulty = Difficulty::Normal;
        let answer = run_command(
            Command::SetDifficulty(Difficulty::Peaceful),
            (
                &mut ServerTime(0),
                &mut ServerWeather::new(&mut rand::thread_rng()),
                &mut difficulty,
            ),
            &mut Player::default(),
            |_, _| unreachable!(),
        );
        assert_eq!(difficulty, Difficulty::Peaceful);
        assert_eq!(answer, "Difficulty set to peaceful");
    }

    #[test]
    fn weather_takes_clear_rain_or_thunder() {
        assert_eq!(
            parse_command("weather rain"),
            Ok(Command::SetWeather(WeatherKind::Rain))
        );
        assert_eq!(
            parse_command(" weather  thunder"),
            Ok(Command::SetWeather(WeatherKind::Thunder))
        );
        for wrong in ["weather", "weather snow", "weather rain 600"] {
            assert_eq!(
                parse_command(wrong),
                Err(WEATHER_USAGE.to_string()),
                "{wrong}"
            );
        }
    }

    #[test]
    fn weather_sets_the_weather_for_a_natural_spell_and_says_so() {
        let mut weather = ServerWeather::new(&mut rand::thread_rng());
        let answer = run_command(
            Command::SetWeather(WeatherKind::Rain),
            (&mut ServerTime(0), &mut weather, &mut Difficulty::Normal),
            &mut Player::default(),
            |_, _| unreachable!(),
        );
        assert_eq!(answer, "Weather set to rain");
        assert_eq!(weather.kind, WeatherKind::Rain);
        assert!(shared::world::STORM_TICKS.contains(&weather.ticks_left));
    }

    #[test]
    fn locate_searches_a_biome_from_its_author() {
        assert_eq!(
            parse_command("locate biome jungle"),
            Ok(Command::Locate(Place::Biome("jungle".into())))
        );
        assert_eq!(
            parse_command("locate village"),
            Ok(Command::Locate(Place::Village))
        );
        for wrong in [
            "locate",
            "locate jungle",
            "locate biome",
            "locate biome a b",
            "locate village a",
        ] {
            assert_eq!(
                parse_command(wrong),
                Err(LOCATE_USAGE.to_string()),
                "{wrong}"
            );
        }
        let player = Player {
            position: Vec3::new(10.0, 70.0, -3.0),
            ..Player::default()
        };
        let answer = run_command(
            Command::Locate(Place::Biome("mymod:mangrove".into())),
            (
                &mut ServerTime(0),
                &mut ServerWeather::new(&mut rand::thread_rng()),
                &mut Difficulty::Normal,
            ),
            &mut player.clone(),
            |place, from| format!("{place:?} from {from}"),
        );
        assert_eq!(
            answer,
            format!("Biome(\"mymod:mangrove\") from {}", player.position)
        );
    }

    #[test]
    fn unknown_commands_are_named_in_the_answer() {
        assert_eq!(
            parse_command("fly"),
            Err("Unknown command: /fly".to_string())
        );
        assert_eq!(parse_command(""), Err("Unknown command: /".to_string()));
    }

    #[test]
    fn the_clock_moves_forward_to_the_next_noon_or_midnight() {
        let day = DAY_DURATION_IN_TICKS;
        let cases = [
            (0, TimeOfDay::Day, NOON_IN_DAY),
            (0, TimeOfDay::Night, MIDNIGHT_IN_DAY),
            // Past midnight, the next one is in the next cycle
            (
                3 * day + MIDNIGHT_IN_DAY + 1,
                TimeOfDay::Night,
                4 * day + MIDNIGHT_IN_DAY,
            ),
            (
                3 * day + MIDNIGHT_IN_DAY + 1,
                TimeOfDay::Day,
                3 * day + NOON_IN_DAY,
            ),
            // Already there: the clock stays
            (3 * day + NOON_IN_DAY, TimeOfDay::Day, 3 * day + NOON_IN_DAY),
        ];
        for (now, time_of_day, expected) in cases {
            assert_eq!(
                next_tick_at(now, time_of_day),
                expected,
                "{now} {time_of_day:?}"
            );
        }
    }

    #[test]
    fn every_command_but_list_is_an_operators() {
        for command in shared::commands::COMMANDS {
            let refused = refusal(command.name, true, false);
            assert_eq!(
                refused.is_some(),
                command.name != "list",
                "{}",
                command.name
            );
            assert_eq!(refusal(command.name, true, true), None);
        }
        assert_eq!(
            refusal("give", true, false).as_deref(),
            Some("You must be an operator to use /give")
        );
        // the commands of the mods too, but an unknown one is answered as unknown
        assert!(refusal("tower", true, false).is_some());
        assert_eq!(refusal("fly", false, false), None);
        assert_eq!(refusal("sign", true, false), None);
    }

    #[test]
    fn op_deop_pardon_take_a_player() {
        let admin = |admin| Ok(Command::Admin(admin));
        assert_eq!(parse_command("op Steve"), admin(Admin::Op("Steve".into())));
        assert_eq!(
            parse_command("deop steve"),
            admin(Admin::Deop("steve".into()))
        );
        assert_eq!(
            parse_command("pardon steve"),
            admin(Admin::Pardon("steve".into()))
        );
        assert_eq!(parse_command("op"), Err(OP_USAGE.to_string()));
        assert_eq!(parse_command("deop a b"), Err(DEOP_USAGE.to_string()));
        assert_eq!(parse_command("pardon"), Err(PARDON_USAGE.to_string()));
    }

    #[test]
    fn kick_and_ban_take_a_player_and_the_rest_of_the_line_as_reason() {
        assert_eq!(
            parse_command("kick steve"),
            Ok(Command::Admin(Admin::Kick {
                name: "steve".into(),
                reason: None
            }))
        );
        assert_eq!(
            parse_command(" ban  steve   griefed  my house "),
            Ok(Command::Admin(Admin::Ban {
                name: "steve".into(),
                reason: Some("griefed  my house".into())
            }))
        );
        assert_eq!(parse_command("kick"), Err(KICK_USAGE.to_string()));
        assert_eq!(parse_command("ban"), Err(BAN_USAGE.to_string()));
    }

    #[test]
    fn a_name_with_spaces_is_typed_between_quotes() {
        let name = |line: &str| match parse_command(line) {
            Ok(Command::Admin(Admin::Op(name))) => Some((name, None)),
            Ok(Command::Admin(Admin::Ban { name, reason })) => Some((name, reason)),
            Ok(Command::Admin(Admin::Whitelist(WhitelistAction::Add(name)))) => Some((name, None)),
            Ok(other) => panic!("{other:?}"),
            Err(_) => None,
        };
        let bad_guy = || "Bad  Guy".to_string();
        assert_eq!(name(r#"op "Bad  Guy""#), Some((bad_guy(), None)));
        assert_eq!(name(r#"whitelist add "Bad  Guy""#), Some((bad_guy(), None)));
        assert_eq!(
            name(r#"ban "Bad  Guy" for "that""#),
            Some((bad_guy(), Some(r#"for "that""#.into())))
        );
        assert_eq!(name(r#"op "a\"b""#), Some((r#"a"b"#.into(), None)));
        for wrong in [r#"op """#, r#"op "a b"#, r#"op "a"b"#, r#"op "a" b"#] {
            assert_eq!(name(wrong), None, "{wrong}");
        }
    }

    #[test]
    fn whitelist_takes_an_action() {
        let whitelist = |action| Ok(Command::Admin(Admin::Whitelist(action)));
        assert_eq!(
            parse_command("whitelist on"),
            whitelist(WhitelistAction::On)
        );
        assert_eq!(
            parse_command("whitelist off"),
            whitelist(WhitelistAction::Off)
        );
        assert_eq!(
            parse_command("whitelist list"),
            whitelist(WhitelistAction::List)
        );
        assert_eq!(
            parse_command("whitelist add alex"),
            whitelist(WhitelistAction::Add("alex".into()))
        );
        assert_eq!(
            parse_command("whitelist remove alex"),
            whitelist(WhitelistAction::Remove("alex".into()))
        );
        for wrong in [
            "whitelist",
            "whitelist add",
            "whitelist on now",
            "whitelist clear",
        ] {
            assert_eq!(
                parse_command(wrong),
                Err(WHITELIST_USAGE.to_string()),
                "{wrong}"
            );
        }
    }

    #[test]
    fn list_names_the_players_online() {
        assert_eq!(parse_command("list"), Ok(Command::List));
        assert_eq!(parse_command("list all"), Err(LIST_USAGE.to_string()));
        assert_eq!(list_players(vec!["steve"]), "1 player online: steve");
        assert_eq!(
            list_players(vec!["steve", "alex"]),
            "2 players online: alex, steve"
        );
    }
}
