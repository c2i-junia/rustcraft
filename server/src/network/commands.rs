//! The chat commands: a chat message that starts with `/` is run by the server for its author,
//! who alone gets the answer, instead of being relayed.

use bevy::math::Vec3;
use shared::players::game_mode::GameMode;
use shared::players::Player;
use shared::world::{ItemId, ItemStack, WeatherKind};
use shared::{DAY_DURATION_IN_TICKS, MIDNIGHT_IN_DAY, NOON_IN_DAY};

use crate::init::ServerTime;
use crate::world::weather::ServerWeather;

/// The author of the answers to a command.
pub const SERVER_AUTHOR: &str = "Server";

const TIME_USAGE: &str = "Usage: /time set <day|night>";
const GIVE_USAGE: &str = "Usage: /give <item> <count>";
const GAMEMODE_USAGE: &str = "Usage: /gamemode <creative|survival>";
const WEATHER_USAGE: &str = "Usage: /weather <clear|rain|thunder>";
const LOCATE_USAGE: &str = "Usage: /locate biome <biome>";

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
    /// `/locate biome <biome>`: the nearest column of the biome to the author of the command.
    LocateBiome(String),
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
        ["locate", "biome", biome] => Ok(Command::LocateBiome(biome.to_string())),
        ["locate", ..] => Err(LOCATE_USAGE.to_string()),
        _ => Err(format!("Unknown command: /{line}")),
    }
}

/// Runs `command` for `player`, its author, and returns the answer. `locate` starts the search
/// of a biome by its key from the position of the player, and returns the answer to give at once
/// (`Locator::start`).
pub fn run_command(
    command: Command,
    time: &mut ServerTime,
    weather: &mut ServerWeather,
    player: &mut Player,
    locate: impl FnOnce(&str, Vec3) -> String,
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
        Command::LocateBiome(key) => locate(&key, player.position),
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
        assert_eq!(shared::commands::COMMANDS.len(), 5);
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
            &mut ServerTime(0),
            &mut ServerWeather::new(&mut rand::thread_rng()),
            &mut player,
            |_, _| unreachable!(),
        );
        assert_eq!(player.game_mode, GameMode::Creative);
        assert_eq!(answer, "Game mode set to creative");
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
            &mut ServerTime(0),
            &mut weather,
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
            Ok(Command::LocateBiome("jungle".into()))
        );
        for wrong in [
            "locate",
            "locate jungle",
            "locate biome",
            "locate biome a b",
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
            Command::LocateBiome("mymod:mangrove".into()),
            &mut ServerTime(0),
            &mut ServerWeather::new(&mut rand::thread_rng()),
            &mut player.clone(),
            |key, from| format!("{key} from {from}"),
        );
        assert_eq!(answer, format!("mymod:mangrove from {}", player.position));
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
}
