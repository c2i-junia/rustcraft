//! The saved worlds as the menus show them: the last one played, its day, and how long ago.

use std::fs;
use std::path::Path;
use std::time::{Duration, SystemTime};

use bevy::prelude::*;
use server::{WorldMeta, WORLD_FILE};

use super::solo::SelectedWorld;
use super::MenuState;
use crate::constants::DAY_DURATION_IN_TICKS;
use crate::state::GameState;
use crate::ui::theme::RAISE;

/// What the menus show of a saved world.
pub struct WorldSummary {
    /// The name of the world's folder, which is what playing it loads.
    pub name: String,
    pub seed: Option<u32>,
    /// The in-game day, from 1.
    pub day: Option<u64>,
    /// When the world was last saved.
    pub played: SystemTime,
}

/// The world whose `world.ron` was modified last. `name` is the folder name (what Play loads);
/// seed and day come from `world.ron`, and are `None` when it cannot be read.
pub fn last_played_world(saves: &Path) -> Option<WorldSummary> {
    let (played, name) = fs::read_dir(saves)
        .ok()?
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let played = fs::metadata(entry.path().join(WORLD_FILE))
                .and_then(|meta| meta.modified())
                .ok()?;
            Some((played, entry.file_name().into_string().ok()?))
        })
        // the name breaks a tie, so that the same folder always gives the same world
        .max()?;
    let meta = fs::read_to_string(saves.join(&name).join(WORLD_FILE))
        .ok()
        .and_then(|contents| ron::de::from_str::<WorldMeta>(&contents).ok());
    Some(WorldSummary {
        name,
        seed: meta.as_ref().map(|meta| meta.seed.0),
        day: meta.as_ref().map(|meta| day_of(meta.time)),
        played,
    })
}

/// The in-game day at the game time `time`, counted from 1.
pub fn day_of(time: u64) -> u64 {
    time / DAY_DURATION_IN_TICKS + 1
}

/// How long ago a world was played, in the largest whole unit: "played 3 days ago".
pub fn played_ago(elapsed: Duration) -> String {
    const MINUTE: u64 = 60;
    const HOUR: u64 = 60 * MINUTE;
    const DAY: u64 = 24 * HOUR;
    let secs = elapsed.as_secs();
    let (count, unit) = match secs {
        0..MINUTE => return "played just now".to_string(),
        MINUTE..HOUR => (secs / MINUTE, "minute"),
        HOUR..DAY => (secs / HOUR, "hour"),
        _ => (secs / DAY, "day"),
    };
    let plural = if count == 1 { "" } else { "s" };
    format!("played {count} {unit}{plural} ago")
}

/// The earthy colours a world's thumbnail takes: grass, dirt, sand, stone, clay and pine.
const THUMBNAIL_COLORS: [Color; 6] = [
    Color::srgb_u8(0x5D, 0x8A, 0x3A),
    Color::srgb_u8(0x86, 0x60, 0x3A),
    Color::srgb_u8(0xC9, 0xB2, 0x7A),
    Color::srgb_u8(0x7D, 0x7F, 0x82),
    Color::srgb_u8(0xA0, 0x58, 0x3A),
    Color::srgb_u8(0x2F, 0x5D, 0x50),
];

/// The colour of a world's thumbnail, which its seed picks; a world of unknown seed is grey.
pub fn thumbnail_color(seed: Option<u32>) -> Color {
    seed.map_or(RAISE, |seed| {
        THUMBNAIL_COLORS[seed as usize % THUMBNAIL_COLORS.len()]
    })
}

/// Leaves the menus to play the world `name`.
pub fn start_world(
    name: &str,
    selected: &mut SelectedWorld,
    game: &mut NextState<GameState>,
    menu: &mut NextState<MenuState>,
) {
    selected.name = Some(name.to_string());
    game.set(GameState::PreGameLoading);
    menu.set(MenuState::Disabled);
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::world::WorldSeed;
    use std::collections::HashMap;

    const HOUR: Duration = Duration::from_secs(60 * 60);
    const DAY: Duration = Duration::from_secs(24 * 60 * 60);

    /// Saves the world `name` in `saves` with this `seed` and `time`, its `world.ron` last
    /// modified `ago`.
    fn save_world(saves: &Path, name: &str, seed: u32, time: u64, ago: Duration) {
        let meta = WorldMeta::new(name.to_string(), WorldSeed(seed), time, HashMap::new());
        let contents = ron::ser::to_string_pretty(&meta, Default::default()).unwrap();
        write_world_ron(saves, name, &contents, ago);
    }

    fn write_world_ron(saves: &Path, name: &str, contents: &str, ago: Duration) {
        let world = saves.join(name);
        fs::create_dir_all(&world).unwrap();
        let path = world.join(WORLD_FILE);
        fs::write(&path, contents).unwrap();
        fs::File::options()
            .write(true)
            .open(&path)
            .unwrap()
            .set_modified(SystemTime::now() - ago)
            .unwrap();
    }

    #[test]
    fn the_last_played_world_is_the_newest_world_ron() {
        let saves = tempfile::tempdir().unwrap();
        save_world(saves.path(), "Old", 1, 0, 5 * DAY);
        save_world(saves.path(), "Recent", 2, 0, 2 * HOUR);
        // a newer folder, but not a world
        fs::create_dir(saves.path().join("Newer")).unwrap();

        let world = last_played_world(saves.path()).unwrap();
        assert_eq!(world.name, "Recent");
    }

    #[test]
    fn the_day_and_seed_come_from_world_ron() {
        let saves = tempfile::tempdir().unwrap();
        save_world(
            saves.path(),
            "Survie",
            7,
            3 * DAY_DURATION_IN_TICKS + 5,
            HOUR,
        );

        let world = last_played_world(saves.path()).unwrap();
        assert_eq!(world.seed, Some(7));
        assert_eq!(world.day, Some(4));
    }

    #[test]
    fn an_unreadable_world_ron_still_counts_without_a_day() {
        let saves = tempfile::tempdir().unwrap();
        write_world_ron(saves.path(), "Broken", "", HOUR);

        let world = last_played_world(saves.path()).unwrap();
        assert_eq!(world.name, "Broken");
        assert_eq!(world.day, None);
        assert_eq!(world.seed, None);
    }

    #[test]
    fn no_saves_folder_means_no_world() {
        let folder = tempfile::tempdir().unwrap();
        assert!(last_played_world(&folder.path().join("saves")).is_none());
    }

    #[test]
    fn played_ago_reads_like_a_person_would_say_it() {
        let secs = Duration::from_secs;
        let cases = [
            (secs(0), "played just now"),
            (secs(59), "played just now"),
            (secs(60), "played 1 minute ago"),
            (secs(5 * 60), "played 5 minutes ago"),
            (HOUR, "played 1 hour ago"),
            (25 * HOUR, "played 1 day ago"),
            (3 * DAY + HOUR, "played 3 days ago"),
        ];
        for (elapsed, said) in cases {
            assert_eq!(played_ago(elapsed), said, "after {elapsed:?}");
        }
    }

    #[test]
    fn the_thumbnail_colour_follows_the_seed() {
        assert_eq!(thumbnail_color(Some(9)), thumbnail_color(Some(9)));
        let colors: Vec<Color> = (0..6).map(|seed| thumbnail_color(Some(seed))).collect();
        for (i, a) in colors.iter().enumerate() {
            for b in &colors[i + 1..] {
                assert_ne!(a, b, "two seeds share a colour");
            }
        }
        assert_eq!(thumbnail_color(None), RAISE);
    }
}
