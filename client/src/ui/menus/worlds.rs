//! The saved worlds as the menus show them and change them on disk: the list, newest first,
//! with each world's day, seed, size and how long ago it was played; creating, renaming and
//! deleting a world.

use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::{self, Path};
use std::time::{Duration, SystemTime};

use bevy::prelude::*;
use server::{write_meta, WorldMeta, FORMAT_VERSION, WORLD_FILE};
use shared::world::WorldSeed;

use super::solo::SelectedWorld;
use super::MenuState;
use crate::constants::DAY_DURATION_IN_TICKS;
use crate::state::GameState;
use crate::ui::theme::RAISE;

/// What the menus show of a saved world.
#[derive(Clone, Debug)]
pub struct WorldSummary {
    /// The name of the world's folder, which is what playing it loads.
    pub name: String,
    pub seed: Option<u32>,
    /// The in-game day, from 1.
    pub day: Option<u64>,
    /// When the world was last saved.
    pub played: SystemTime,
    /// The bytes of every file of the world's folder.
    pub size: u64,
}

/// The folders of `saves` holding a `world.ron`, newest `world.ron` first, ties by name
/// ascending. `name` is the folder name (what Play loads); seed and day come from `world.ron`,
/// and are `None` when it cannot be read. Names that are not UTF-8 are skipped; a missing or
/// unreadable `saves` gives none. Creates nothing.
pub fn list_worlds(saves: &Path) -> Vec<WorldSummary> {
    let Ok(entries) = fs::read_dir(saves) else {
        return Vec::new();
    };
    let mut worlds: Vec<WorldSummary> = entries
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let folder = entry.path();
            let played = fs::metadata(folder.join(WORLD_FILE))
                .and_then(|meta| meta.modified())
                .ok()?;
            let name = entry.file_name().into_string().ok()?;
            let meta = read_meta(&folder);
            Some(WorldSummary {
                name,
                seed: meta.as_ref().map(|meta| meta.seed.0),
                day: meta.as_ref().map(|meta| day_of(meta.time)),
                played,
                size: folder_size(&folder),
            })
        })
        .collect();
    // the name breaks a tie, so that the same folder always lists the same way
    worlds.sort_by(|a, b| b.played.cmp(&a.played).then_with(|| a.name.cmp(&b.name)));
    worlds
}

/// The world whose `world.ron` was modified last: the first of `list_worlds`.
pub fn last_played_world(saves: &Path) -> Option<WorldSummary> {
    list_worlds(saves).into_iter().next()
}

/// The `world.ron` of the world stored in `folder`, if it can be read as `WorldMeta`.
fn read_meta(folder: &Path) -> Option<WorldMeta> {
    let contents = fs::read_to_string(folder.join(WORLD_FILE)).ok()?;
    ron::de::from_str(&contents).ok()
}

/// The bytes of every file under `folder`. Links are not followed, and what cannot be read
/// counts for nothing.
fn folder_size(folder: &Path) -> u64 {
    let Ok(entries) = fs::read_dir(folder) else {
        return 0;
    };
    entries
        .filter_map(Result::ok)
        .map(|entry| match entry.metadata() {
            Ok(meta) if meta.is_dir() => folder_size(&entry.path()),
            Ok(meta) if meta.is_file() => meta.len(),
            _ => 0,
        })
        .sum()
}

/// A size as a file manager says it: kilobytes rounded up, then megabytes and gigabytes with
/// one decimal. A unit takes over where the one below would say 1024.
pub fn format_size(bytes: u64) -> String {
    const KIB: u64 = 1024;
    let kb = bytes.div_ceil(KIB);
    if kb < KIB {
        return format!("{kb} KB");
    }
    // the size in tenths of `unit`, rounded to the nearest
    let tenths = |unit: u64| (u128::from(bytes) * 10 + u128::from(unit / 2)) / u128::from(unit);
    let mb = tenths(KIB * KIB);
    if mb < 10 * u128::from(KIB) {
        return format!("{}.{} MB", mb / 10, mb % 10);
    }
    let gb = tenths(KIB * KIB * KIB);
    format!("{}.{} GB", gb / 10, gb % 10)
}

/// The longest name a new world may have.
pub const MAX_WORLD_NAME_LENGTH: usize = 32;

/// Shown under the name field when it is empty.
pub const EMPTY_WORLD_NAME: &str = "Enter a name";
/// Shown under the name field when Windows keeps the typed name for a device, such as `CON`.
pub const RESERVED_WORLD_NAME: &str = "Windows reserves this name: choose another";
/// Shown under the name field when the typed name breaks the rule of `is_valid_world_name`.
pub const INVALID_WORLD_NAME: &str = "Use only A-Z, a-z, 0-9, spaces, _ and -";
/// Shown under the name field when another world already has the typed name, whatever its case.
pub const WORLD_EXISTS: &str = "A world with this name already exists";
/// Shown under the seed field when the typed seed is not a `u32`.
pub const INVALID_SEED: &str = "Use a number from 0 to 4294967295";

/// A world name becomes a folder name under the saves folder, so it must be a single plain
/// path component: letters, digits, spaces, `_` and `-`, 1 to 32 characters.
fn is_valid_world_name(name: &str) -> bool {
    (1..=MAX_WORLD_NAME_LENGTH).contains(&name.chars().count())
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, ' ' | '_' | '-'))
}

/// Worlds that already exist on disk may have any name, but changing one must stay inside the
/// saves folder: the name has to be a single normal path component.
fn is_single_path_component(name: &str) -> bool {
    let mut components = Path::new(name).components();
    matches!(
        (components.next(), components.next()),
        (Some(path::Component::Normal(component)), None) if component == name
    )
}

/// Whether Windows keeps `name` for a device (`CON`, `PRN`, `AUX`, `NUL`, `COM1` to `COM9`,
/// `LPT1` to `LPT9`), whatever its case and extension: no folder can have it there.
fn is_reserved_on_windows(name: &str) -> bool {
    let stem = name.split('.').next().unwrap_or(name);
    let numbered = |prefix: &str| {
        stem.len() == 4
            && stem
                .get(..3)
                .is_some_and(|start| start.eq_ignore_ascii_case(prefix))
            && matches!(stem.as_bytes()[3], b'1'..=b'9')
    };
    ["CON", "PRN", "AUX", "NUL"]
        .iter()
        .any(|device| stem.eq_ignore_ascii_case(device))
        || numbered("COM")
        || numbered("LPT")
}

/// The error of a name that is not a single path component.
fn invalid_name(name: &str) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidInput,
        format!("invalid world name {name:?}"),
    )
}

/// The `typed` name trimmed, or the message to show when it is empty, is kept by Windows for a
/// device, breaks the rule of `is_valid_world_name`, or is one of `others`, the names of the
/// other worlds, whatever its case (Windows and macOS folders ignore it).
pub fn check_world_name(typed: &str, others: &[String]) -> Result<String, &'static str> {
    let name = typed.trim();
    if name.is_empty() {
        return Err(EMPTY_WORLD_NAME);
    }
    if is_reserved_on_windows(name) {
        return Err(RESERVED_WORLD_NAME);
    }
    if !is_valid_world_name(name) {
        return Err(INVALID_WORLD_NAME);
    }
    if others.iter().any(|other| other.eq_ignore_ascii_case(name)) {
        return Err(WORLD_EXISTS);
    }
    Ok(name.to_string())
}

/// The first of "New world", "New world 2", "New world 3"… that none of `others` has, whatever
/// its case.
pub fn free_world_name(others: &[String]) -> String {
    let taken = |name: &str| others.iter().any(|other| other.eq_ignore_ascii_case(name));
    std::iter::once("New world".to_string())
        .chain((2..).map(|index| format!("New world {index}")))
        .find(|name| !taken(name))
        .unwrap_or_default()
}

/// The seed `typed` in the seed field, digits only: none when it is empty, which means a random
/// one.
pub fn parse_seed(typed: &str) -> Result<Option<u32>, &'static str> {
    if typed.is_empty() {
        return Ok(None);
    }
    if !typed.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(INVALID_SEED);
    }
    typed.parse().map(Some).map_err(|_| INVALID_SEED)
}

/// The error of a failed `write_meta`, of its own kind when it is an I/O error.
fn io_error(err: Box<dyn std::error::Error>) -> io::Error {
    match err.downcast::<io::Error>() {
        Ok(err) => *err,
        Err(err) => io::Error::other(err.to_string()),
    }
}

/// Creates the world `name` in `saves` with this `seed`: its folder, never an existing one, and
/// its `world.ron`, so that it is listed at once. On a failed write the folder is removed.
pub fn create_world(saves: &Path, name: &str, seed: u32) -> io::Result<()> {
    if !is_single_path_component(name) {
        return Err(invalid_name(name));
    }
    fs::create_dir_all(saves)?;
    let folder = saves.join(name);
    fs::create_dir(&folder)?;
    let meta = WorldMeta::new(name.to_string(), WorldSeed(seed), 0, HashMap::new());
    write_meta(&folder, &meta).map_err(|err| {
        let _ = fs::remove_dir_all(&folder);
        io_error(err)
    })
}

/// Renames the world `from` of `saves` to `to`: its folder, then the name in its `world.ron`
/// when that can be read, keeping its format version and its modification time (renaming is
/// not playing). A `world.ron` of a newer format version is left as it is, since it may hold
/// fields this version does not know. Another world named `to` is never replaced, but `to` may
/// be `from` in another case. On an error nothing is changed.
#[allow(dead_code)] // called by the rename dialog, Task 6
pub fn rename_world(saves: &Path, from: &str, to: &str) -> io::Result<()> {
    for name in [from, to] {
        if !is_single_path_component(name) {
            return Err(invalid_name(name));
        }
    }
    let target = saves.join(to);
    if !to.eq_ignore_ascii_case(from) && fs::symlink_metadata(&target).is_ok() {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!("a world named {to:?} already exists"),
        ));
    }
    fs::rename(saves.join(from), &target)?;

    let Some(mut meta) = read_meta(&target).filter(|meta| meta.format_version <= FORMAT_VERSION)
    else {
        return Ok(());
    };
    let world_ron = target.join(WORLD_FILE);
    let played = fs::metadata(&world_ron).and_then(|meta| meta.modified());
    meta.name = to.to_string();
    if let Err(err) = write_meta(&target, &meta) {
        if let Err(back) = fs::rename(&target, saves.join(from)) {
            error!(
                "Could not rename {} back to {from:?}: {back}",
                target.display()
            );
        }
        return Err(io_error(err));
    }
    let restored = played.and_then(|played| {
        fs::File::options()
            .write(true)
            .open(&world_ron)?
            .set_modified(played)
    });
    if let Err(err) = restored {
        warn!("Renamed {from:?} to {to:?}, but could not restore the date of its world.ron: {err}");
    }
    Ok(())
}

/// Deletes the world `name` of `saves` with every file of its folder. A name that is not a
/// single path component is refused, so that nothing outside `saves` is deleted; a world that
/// was never saved has nothing to delete.
pub fn delete_world(saves: &Path, name: &str) -> io::Result<()> {
    if !is_single_path_component(name) {
        return Err(invalid_name(name));
    }

    // Delete the entire world directory
    let world_dir = saves.join(name);

    match fs::remove_dir_all(&world_dir) {
        Ok(()) => info!("Deleted world directory: {}", world_dir.display()),
        // Nothing was ever saved for this world: there is nothing to delete
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(e) => {
            return Err(io::Error::new(
                e.kind(),
                format!("could not delete {}: {e}", world_dir.display()),
            ))
        }
    }
    Ok(())
}

/// The in-game day at the game time `time`, counted from 1.
pub fn day_of(time: u64) -> u64 {
    time / DAY_DURATION_IN_TICKS + 1
}

/// How long ago `played` was: none when it is in the future (a clock set back since).
pub fn elapsed_since(played: SystemTime) -> Duration {
    SystemTime::now().duration_since(played).unwrap_or_default()
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
        write_world_ron_at(saves, name, contents, SystemTime::now() - ago);
    }

    /// Writes the `world.ron` of `name`, last modified at `modified`.
    fn write_world_ron_at(saves: &Path, name: &str, contents: &str, modified: SystemTime) {
        let world = saves.join(name);
        fs::create_dir_all(&world).unwrap();
        let path = world.join(WORLD_FILE);
        fs::write(&path, contents).unwrap();
        fs::File::options()
            .write(true)
            .open(&path)
            .unwrap()
            .set_modified(modified)
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
    fn two_worlds_played_at_the_same_time_are_told_apart_by_name() {
        let saves = tempfile::tempdir().unwrap();
        let same = SystemTime::now() - HOUR;
        for name in ["Bravo", "Charlie", "Alpha"] {
            write_world_ron_at(saves.path(), name, "", same);
        }

        assert_eq!(last_played_world(saves.path()).unwrap().name, "Alpha");
    }

    #[test]
    fn a_world_saved_in_the_future_was_played_just_now() {
        let saves = tempfile::tempdir().unwrap();
        write_world_ron_at(saves.path(), "Ahead", "", SystemTime::now() + DAY);

        let world = last_played_world(saves.path()).unwrap();
        assert_eq!(played_ago(elapsed_since(world.played)), "played just now");
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

    /// The names of the worlds `list_worlds` gives, in its order.
    fn listed_names(saves: &Path) -> Vec<String> {
        list_worlds(saves)
            .into_iter()
            .map(|world| world.name)
            .collect()
    }

    #[test]
    fn the_worlds_are_listed_by_last_played_then_by_name() {
        let saves = tempfile::tempdir().unwrap();
        save_world(saves.path(), "Old", 1, 0, 5 * DAY);
        save_world(saves.path(), "Recent", 2, 0, 2 * HOUR);
        let same = SystemTime::now() - DAY;
        write_world_ron_at(saves.path(), "Bravo", "", same);
        write_world_ron_at(saves.path(), "Alpha", "", same);
        fs::create_dir(saves.path().join("Not a world")).unwrap();
        fs::write(saves.path().join("file"), "").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::ffi::OsStrExt;
            let bad = saves.path().join(std::ffi::OsStr::from_bytes(b"bad\xff"));
            fs::create_dir(&bad).unwrap();
            fs::write(bad.join(WORLD_FILE), "").unwrap();
        }

        assert_eq!(
            listed_names(saves.path()),
            ["Recent", "Alpha", "Bravo", "Old"]
        );
    }

    #[test]
    fn a_missing_or_unreadable_saves_folder_lists_no_world_and_creates_nothing() {
        let folder = tempfile::tempdir().unwrap();
        let saves = folder.path().join("saves");

        assert!(list_worlds(&saves).is_empty());
        assert!(!saves.exists());

        fs::write(&saves, "").unwrap();
        assert!(list_worlds(&saves).is_empty());
    }

    #[test]
    fn the_size_counts_every_file_of_the_world() {
        let saves = tempfile::tempdir().unwrap();
        write_world_ron(saves.path(), "Survie", &"x".repeat(10), HOUR);
        let world = saves.path().join("Survie");
        fs::create_dir_all(world.join("chunks")).unwrap();
        fs::write(world.join("chunks/a.bin"), vec![0u8; 2000]).unwrap();
        fs::create_dir_all(world.join("players")).unwrap();
        fs::write(world.join("players/1.ron"), "x".repeat(30)).unwrap();

        assert_eq!(list_worlds(saves.path())[0].size, 2040);
    }

    #[test]
    fn sizes_read_like_a_file_manager_says_them() {
        let cases = [
            (0, "0 KB"),
            (1, "1 KB"),
            (1024, "1 KB"),
            (1025, "2 KB"),
            (1_048_576, "1.0 MB"),
            (1_572_864, "1.5 MB"),
            (3 << 30, "3.0 GB"),
            // never 1024 of a unit: the next unit takes over
            (1_047_552, "1023 KB"),
            (1_047_553, "1.0 MB"),
            (1_048_575, "1.0 MB"),
            (1_073_689_395, "1023.9 MB"),
            (1_073_689_396, "1.0 GB"),
            ((1 << 30) - 1, "1.0 GB"),
        ];
        for (bytes, said) in cases {
            assert_eq!(format_size(bytes), said, "for {bytes} bytes");
        }
    }

    #[test]
    fn world_names_are_checked_and_trimmed() {
        let others = ["Survie".to_string()];
        assert_eq!(
            check_world_name("  Base  ", &others),
            Ok("Base".to_string())
        );
        for empty in ["", "   "] {
            assert_eq!(check_world_name(empty, &others), Err(EMPTY_WORLD_NAME));
        }
        for invalid in ["bad/name", "é", "a.b", &"x".repeat(33)] {
            assert_eq!(
                check_world_name(invalid, &others),
                Err(INVALID_WORLD_NAME),
                "{invalid:?}"
            );
        }
        for taken in ["survie", "SURVIE "] {
            assert_eq!(
                check_world_name(taken, &others),
                Err(WORLD_EXISTS),
                "{taken:?}"
            );
        }
        assert_eq!(
            check_world_name("Survie2", &others),
            Ok("Survie2".to_string())
        );
    }

    #[test]
    fn names_windows_reserves_are_refused() {
        for reserved in [
            "CON",
            "con",
            "Prn",
            "AUX",
            "nul",
            "COM1",
            "com9",
            "LPT1",
            "lpt9",
            "aux.txt",
            "Con.tar.gz",
        ] {
            assert_eq!(
                check_world_name(reserved, &[]),
                Err(RESERVED_WORLD_NAME),
                "{reserved:?}"
            );
        }
        for allowed in ["COM0", "COM10", "LPT", "Console", "nul2", "My CON"] {
            assert_eq!(
                check_world_name(allowed, &[]),
                Ok(allowed.to_string()),
                "{allowed:?}"
            );
        }
    }

    #[test]
    fn a_free_name_is_offered() {
        let names = |names: &[&str]| {
            names
                .iter()
                .map(|name| name.to_string())
                .collect::<Vec<_>>()
        };
        assert_eq!(free_world_name(&[]), "New world");
        assert_eq!(free_world_name(&names(&["new world"])), "New world 2");
        assert_eq!(
            free_world_name(&names(&["New world", "New world 2"])),
            "New world 3"
        );
    }

    #[test]
    fn a_seed_is_a_number_or_nothing() {
        assert_eq!(parse_seed(""), Ok(None));
        assert_eq!(parse_seed("0"), Ok(Some(0)));
        assert_eq!(parse_seed("4294967295"), Ok(Some(u32::MAX)));
        for invalid in ["4294967296", "12a", "+5", "-1", " 5"] {
            assert_eq!(parse_seed(invalid), Err(INVALID_SEED), "{invalid:?}");
        }
    }

    /// The `world.ron` of the world `name`, read as `WorldMeta`.
    fn meta_of(saves: &Path, name: &str) -> WorldMeta {
        let contents = fs::read_to_string(saves.join(name).join(WORLD_FILE)).unwrap();
        ron::de::from_str(&contents).unwrap()
    }

    #[test]
    fn creating_a_world_writes_its_world_ron_at_once() {
        let folder = tempfile::tempdir().unwrap();
        let saves = folder.path().join("saves");

        create_world(&saves, "Nouveau", 7).unwrap();

        let meta = meta_of(&saves, "Nouveau");
        assert_eq!(meta.format_version, 2);
        assert_eq!(meta.name, "Nouveau");
        assert_eq!(meta.seed.0, 7);
        assert_eq!(meta.time, 0);
        assert!(meta.mobs.is_empty());
        let worlds = list_worlds(&saves);
        assert_eq!(worlds.len(), 1);
        assert_eq!(worlds[0].name, "Nouveau");
        assert_eq!(worlds[0].day, Some(1));
        assert_eq!(worlds[0].seed, Some(7));
    }

    /// A folder under `base` whose path is `length` bytes long, made of short components.
    #[cfg(target_os = "linux")]
    fn path_of_length(base: &Path, length: usize) -> std::path::PathBuf {
        let mut path = base.to_path_buf();
        while path.as_os_str().len() < length {
            let left = length - path.as_os_str().len() - 1;
            path.push("x".repeat(left.min(200)));
        }
        assert_eq!(path.as_os_str().len(), length);
        path
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn a_world_whose_world_ron_cannot_be_written_is_not_left_behind() {
        let folder = tempfile::tempdir().unwrap();
        // The world's folder fits in PATH_MAX (4096 with its terminating NUL), its
        // world.ron.tmp does not: the folder is made, then the write fails
        let saves = path_of_length(folder.path(), 4090 - "/Nouveau".len());

        assert!(create_world(&saves, "Nouveau", 1).is_err());

        assert!(saves.is_dir());
        assert!(!saves.join("Nouveau").exists());
    }

    #[test]
    fn creating_never_writes_into_an_existing_folder() {
        let saves = tempfile::tempdir().unwrap();
        write_world_ron(saves.path(), "Survie", "precious", HOUR);

        let err = create_world(saves.path(), "Survie", 1).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::AlreadyExists);
        let contents = fs::read_to_string(saves.path().join("Survie").join(WORLD_FILE)).unwrap();
        assert_eq!(contents, "precious");

        for outside in ["..", "a/b"] {
            let err = create_world(saves.path(), outside, 1).unwrap_err();
            assert_eq!(err.kind(), io::ErrorKind::InvalidInput, "{outside:?}");
        }
    }

    #[test]
    fn renaming_moves_the_folder_and_the_name_and_keeps_the_played_time() {
        let saves = tempfile::tempdir().unwrap();
        save_world(saves.path(), "Survie", 7, 0, 3 * DAY);
        let chunks = saves.path().join("Survie/chunks");
        fs::create_dir(&chunks).unwrap();
        fs::write(chunks.join("0_0_0.bin"), "x").unwrap();
        let played = list_worlds(saves.path())[0].played;

        rename_world(saves.path(), "Survie", "Base").unwrap();

        assert!(!saves.path().join("Survie").exists());
        assert!(saves.path().join("Base/chunks/0_0_0.bin").is_file());
        assert_eq!(meta_of(saves.path(), "Base").name, "Base");
        assert_eq!(list_worlds(saves.path())[0].played, played);
    }

    #[test]
    fn renaming_keeps_the_format_version_and_leaves_an_unreadable_world_ron_alone() {
        let saves = tempfile::tempdir().unwrap();
        let version_1 = "(format_version: 1, name: \"Old\", seed: (3), time: 0, mobs: {})";
        write_world_ron(saves.path(), "Old", version_1, HOUR);
        write_world_ron(saves.path(), "Broken", "garbage", HOUR);

        rename_world(saves.path(), "Old", "Older").unwrap();
        rename_world(saves.path(), "Broken", "Mended").unwrap();

        let meta = meta_of(saves.path(), "Older");
        assert_eq!(meta.format_version, 1);
        assert_eq!(meta.name, "Older");
        let contents = fs::read_to_string(saves.path().join("Mended").join(WORLD_FILE)).unwrap();
        assert_eq!(contents, "garbage");
        assert!(!saves.path().join("Broken").exists());
    }

    #[test]
    fn renaming_onto_another_world_is_refused_but_a_change_of_case_is_not() {
        let saves = tempfile::tempdir().unwrap();
        save_world(saves.path(), "A", 1, 0, HOUR);
        save_world(saves.path(), "B", 2, 0, DAY);

        let err = rename_world(saves.path(), "A", "B").unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::AlreadyExists);
        assert_eq!(meta_of(saves.path(), "A").seed.0, 1);
        assert_eq!(meta_of(saves.path(), "B").seed.0, 2);

        rename_world(saves.path(), "A", "a").unwrap();
        assert_eq!(listed_names(saves.path()), ["a", "B"]);
    }

    #[test]
    fn a_rename_whose_world_ron_cannot_be_written_changes_nothing() {
        let saves = tempfile::tempdir().unwrap();
        save_world(saves.path(), "Survie", 7, 0, HOUR);
        let world_ron = saves.path().join("Survie").join(WORLD_FILE);
        let before = fs::read_to_string(&world_ron).unwrap();
        // A directory squatting on the temporary name makes the write of world.ron fail
        fs::create_dir(saves.path().join("Survie/world.ron.tmp")).unwrap();

        let err = rename_world(saves.path(), "Survie", "Base").unwrap_err();

        assert_ne!(err.kind(), io::ErrorKind::Other, "the kind is kept: {err}");
        assert!(!saves.path().join("Base").exists());
        assert_eq!(fs::read_to_string(&world_ron).unwrap(), before);
    }

    #[test]
    fn renaming_leaves_the_world_ron_of_a_newer_version_alone() {
        let saves = tempfile::tempdir().unwrap();
        let newer = format!(
            "(format_version: {}, name: \"Next\", seed: (3), time: 0, mobs: {{}})",
            FORMAT_VERSION + 1
        );
        write_world_ron(saves.path(), "Next", &newer, HOUR);

        rename_world(saves.path(), "Next", "Later").unwrap();

        let contents = fs::read_to_string(saves.path().join("Later").join(WORLD_FILE)).unwrap();
        assert_eq!(contents, newer);
    }

    #[test]
    fn renaming_refuses_names_that_leave_the_saves_folder() {
        let saves = tempfile::tempdir().unwrap();
        save_world(saves.path(), "Survie", 1, 0, HOUR);

        for (from, to) in [("Survie", "../x"), ("Survie", ""), ("../x", "Survie")] {
            let err = rename_world(saves.path(), from, to).unwrap_err();
            assert_eq!(
                err.kind(),
                io::ErrorKind::InvalidInput,
                "{from:?} to {to:?}"
            );
        }
        assert!(saves.path().join("Survie").is_dir());
    }

    #[test]
    fn world_names_are_a_single_plain_component() {
        for valid in ["a", "My World", "new_world_1", "a-b", &"x".repeat(32)] {
            assert!(is_valid_world_name(valid), "{valid:?} should be valid");
        }
        for invalid in [
            "",
            ".",
            "..",
            "/",
            "a/b",
            "..\\x",
            "/etc",
            "C:\\x",
            "a.b",
            "é",
            &"x".repeat(33),
        ] {
            assert!(
                !is_valid_world_name(invalid),
                "{invalid:?} should be invalid"
            );
        }
    }

    #[test]
    fn existing_worlds_can_be_deleted_whatever_their_name_if_it_is_one_component() {
        for name in ["a", "world.v2", "é", "a b", "..x", "x.."] {
            assert!(
                is_single_path_component(name),
                "{name:?} should be accepted"
            );
        }
        for name in ["", ".", "..", "/", "a/b", "a/", "/etc", "../x"] {
            assert!(
                !is_single_path_component(name),
                "{name:?} should be refused"
            );
        }
    }

    #[test]
    fn a_world_with_a_name_outside_the_creation_rule_is_deleted() {
        let saves = tempfile::tempdir().unwrap();
        write_world_ron(saves.path(), "world.v2", "", HOUR);

        delete_world(saves.path(), "world.v2").unwrap();
        assert!(!saves.path().join("world.v2").exists());
    }

    #[test]
    fn deleting_a_world_removes_its_chunk_files_and_players_too() {
        let saves = tempfile::tempdir().unwrap();
        let saves = saves.path();
        write_world_ron(saves, "full", "", HOUR);
        fs::create_dir_all(saves.join("full/chunks")).unwrap();
        fs::write(saves.join("full/chunks/-1_4_0.bin"), "x").unwrap();
        fs::create_dir_all(saves.join("full/players")).unwrap();
        fs::write(saves.join("full/players/7.ron"), "x").unwrap();
        write_world_ron(saves, "other", "", HOUR);

        delete_world(saves, "full").unwrap();

        assert!(!saves.join("full").exists());
        assert!(saves.join("other").join(WORLD_FILE).is_file());
    }

    #[test]
    fn deleting_refuses_names_that_leave_the_saves_folder() {
        let folder = tempfile::tempdir().unwrap();
        let game = folder.path().join("game");
        let saves = game.join("saves");
        write_world_ron(&saves, "keep", "", HOUR);
        write_world_ron(&saves, "gone", "", HOUR);
        let outside = game.join("outside");
        fs::create_dir_all(&outside).unwrap();

        for name in ["..", "../outside", "../x", "/", ""] {
            let err = delete_world(&saves, name).unwrap_err();
            assert_eq!(err.kind(), io::ErrorKind::InvalidInput, "{name:?}");
        }
        assert!(outside.is_dir() && saves.join("keep").is_dir());

        delete_world(&saves, "gone").unwrap();
        assert!(!saves.join("gone").exists() && saves.join("keep").is_dir());
    }

    #[test]
    fn deleting_a_world_that_is_already_gone_succeeds() {
        let folder = tempfile::tempdir().unwrap();
        delete_world(&folder.path().join("saves"), "never-saved").unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn a_world_that_cannot_be_deleted_is_an_error() {
        use std::os::unix::fs::PermissionsExt;

        let saves = tempfile::tempdir().unwrap();
        let saves = saves.path();
        write_world_ron(saves, "locked", "", HOUR);
        // Nothing can be removed from a folder that is not writable
        let read_only = fs::Permissions::from_mode(0o555);
        fs::set_permissions(saves.join("locked"), read_only).unwrap();

        let result = delete_world(saves, "locked");

        fs::set_permissions(saves.join("locked"), fs::Permissions::from_mode(0o755)).unwrap();
        assert!(result.is_err());
        assert!(saves.join("locked").join(WORLD_FILE).is_file());
    }
}
