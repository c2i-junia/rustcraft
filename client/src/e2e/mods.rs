//! The Mods screen: the mods of the game folder, turned on and off one by one or all at once,
//! found by the search, and saved in `mods.ron` when the screen is left.

use accesskit::Role;
use bevy::prelude::*;
use shared::mods::{read_mods_settings, MODS_SETTINGS_PATH};
use shared::world::BASE_MOD;

use super::harness::{Harness, SCREEN, SERVER, STARTUP};
use super::helpers::main_menu_with;
use super::locator::Locator as L;
use super::seed::GameFolder;
use crate::ui::menus::mods::{ModList, ModSearch, ModSwitch, MODS_TITLE};
use crate::ui::menus::MenuState;

/// The texture of the torch, as the icon of Lanterns.
const TORCH: &[u8] = include_bytes!("../../../data/graphics/base_textures/blocks/Torch.png");

/// The warning of the footer while the base mod is off.
const NO_BASE: &str = "No world starts without Rustcraft, the base mod";

/// Six mods, beside the base mod that comes with the game: three with a manifest, Lantern Paths needing Lanterns, which has an icon, one without a manifest,
/// one whose manifest does not read and one whose script does not parse.
fn six_mods() -> GameFolder {
    GameFolder::new()
        .with_mod("lanterns", "Lanterns", "Adds the lantern.", &[])
        .with_mod_file("lanterns", "icon.png", TORCH)
        .with_mod(
            "lantern_paths",
            "Lantern Paths",
            "Lights the paths you walk.",
            &["lanterns"],
        )
        .with_mod("woolly", "Woolly Flocks", "Sheep spawn in herds.", &[])
        .with_mod_file(
            "tower",
            "init.luau",
            "rustcraft.register_command('tower', function() end)",
        )
        .with_mod_file("broken", "mod.toml", "this is not toml")
        .with_mod_file("typo", "init.luau", "local x = = 1")
}

/// The Mods screen, opened from the main menu, on the mods of `folder`.
#[track_caller]
fn open_mods(folder: GameFolder) -> Harness {
    let mut h = main_menu_with(folder);
    h.click(&L::role(Role::Button, MODS_TITLE));
    h.wait_for(&L::marker::<ModList>(), SCREEN);
    h
}

/// The card of the mod called `name`, on or off.
fn card(name: &str, on: bool) -> L {
    let state = if on { "on" } else { "off" };
    L::role(Role::ListItem, &format!("{name}, {state}"))
}

/// The settings `mods.ron` holds, `None` while it is not written.
fn saved(h: &Harness) -> Option<Vec<String>> {
    h.folder().join(MODS_SETTINGS_PATH).exists().then(|| {
        read_mods_settings(h.folder())
            .unwrap()
            .disabled
            .into_iter()
            .collect()
    })
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn the_main_menu_opens_the_mods_and_esc_goes_back() {
    let mut h = open_mods(six_mods());
    assert_eq!(h.menu_state(), MenuState::Mods);
    for name in [
        "Rustcraft",
        "Lanterns",
        "Lantern Paths",
        "Woolly Flocks",
        "tower",
        "broken",
        "typo",
    ] {
        assert!(h.is_visible(&card(name, true)), "{name}");
    }
    assert!(h.is_visible(&L::text("7 of 7 on")));

    h.press(KeyCode::Escape);
    h.wait_for(&L::text("Singleplayer"), SCREEN);
    assert_eq!(h.menu_state(), MenuState::Main);
    assert_eq!(saved(&h), None, "nothing changed, nothing written");
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn a_mod_that_cannot_load_says_why() {
    let mut h = open_mods(six_mods());
    assert_eq!(h.find(&L::text("Cannot load")).len(), 2);

    h.click(&card("typo", true));
    h.wait_for(&L::text_containing("typo/init.luau:1"), SCREEN);
    h.click(&card("broken", true));
    h.wait_for(&L::text_containing("mod.toml:1"), SCREEN);
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn a_mod_turned_off_keeps_the_mods_that_need_it_waiting_and_done_saves_it() {
    let mut h = open_mods(six_mods());
    h.click(&L::marker::<ModSwitch>().within(card("Lanterns", true)));
    h.wait_for(&card("Lanterns", false), SCREEN);
    assert!(h.is_visible(&L::text("Needs lanterns")));
    assert!(h.is_visible(&L::text("6 of 7 on")));
    assert!(h.is_visible(&L::text("1 change, applied when a world starts")));

    h.click(&card("Lantern Paths", true));
    h.wait_for(&L::text("Needs lanterns, which is off"), SCREEN);

    h.click(&L::role(Role::Button, "Done"));
    h.wait_for(&L::text("Singleplayer"), SCREEN);
    assert_eq!(saved(&h), Some(vec!["lanterns".to_string()]));
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn the_mods_turned_off_in_mods_ron_show_off() {
    let folder = six_mods().with_file(MODS_SETTINGS_PATH, "(disabled: [\"woolly\"])");
    let mut h = open_mods(folder);
    assert!(h.is_visible(&card("Woolly Flocks", false)));
    assert!(h.is_visible(&L::text("6 of 7 on")));
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn turn_all_acts_on_the_mods_the_search_shows() {
    let mut h = open_mods(six_mods());
    h.click(&L::marker::<ModSearch>());
    h.type_text("lantern");
    h.wait_for(&L::text("Turn 2 off"), SCREEN);
    assert!(!h.is_visible(&card("Woolly Flocks", true)));

    h.click(&L::role(Role::Button, "Turn 2 off"));
    h.wait_for(&card("Lanterns", false), SCREEN);
    assert!(h.is_visible(&card("Lantern Paths", false)));

    // Escape clears the search, which shows every mod again
    h.click(&L::marker::<ModSearch>());
    h.press(KeyCode::Escape);
    h.wait_for(&card("Woolly Flocks", true), SCREEN);
    h.click(&L::role(Role::Button, "Turn all off"));
    h.wait_for(&L::text("0 of 7 on"), SCREEN);
    assert!(h.is_visible(&L::text(NO_BASE)));
    h.click(&L::role(Role::Button, "Turn all on"));
    h.wait_for(&L::text("7 of 7 on"), SCREEN);
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn a_search_that_matches_nothing_says_so() {
    let mut h = open_mods(six_mods());
    h.type_text("/");
    h.type_text("castle");
    h.wait_for(&L::text("No mod matches \u{201c}castle\u{201d}"), SCREEN);
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn space_turns_the_focused_mod_off_and_revert_puts_it_back() {
    let mut h = open_mods(six_mods());
    // the cards by id: broken, lantern_paths, lanterns
    h.press(KeyCode::ArrowDown);
    h.press(KeyCode::Space);
    h.wait_for(&card("Lantern Paths", false), SCREEN);

    h.click(&L::role(Role::Button, "Revert"));
    h.wait_for(&card("Lantern Paths", true), SCREEN);
    assert!(!h.is_visible(&L::role(Role::Button, "Revert")));
    h.press(KeyCode::Escape);
    h.wait_for(&L::text("Singleplayer"), SCREEN);
    assert_eq!(saved(&h), None);
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn without_mods_the_screen_shows_the_base_mod_alone() {
    let mut h = open_mods(GameFolder::new());
    assert!(h.is_visible(&card("Rustcraft", true)));
    assert!(h.is_visible(&L::text("1 of 1 on")));
    assert!(h.is_visible(&L::text("Comes with the game: it holds every block")));
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn with_the_base_mod_off_no_world_starts_and_says_why() {
    let folder = GameFolder::new().with_world("Alpha").with_mod(
        "castle",
        "Castle",
        "Builds a castle.",
        &[BASE_MOD],
    );
    let mut h = open_mods(folder);
    h.click(&L::marker::<ModSwitch>().within(card("Rustcraft", true)));
    h.wait_for(&card("Rustcraft", false), SCREEN);
    assert!(h.is_visible(&L::text(NO_BASE)));
    assert!(h.is_visible(&L::text("Needs rustcraft")));

    // turned on again, the warning goes
    h.click(&L::marker::<ModSwitch>().within(card("Rustcraft", false)));
    h.wait_for(&card("Rustcraft", true), SCREEN);
    assert!(!h.is_visible(&L::text(NO_BASE)));
    h.click(&L::marker::<ModSwitch>().within(card("Rustcraft", true)));
    h.wait_for(&card("Rustcraft", false), SCREEN);

    h.click(&L::role(Role::Button, "Done"));
    h.wait_for(&L::text("Singleplayer"), STARTUP);
    assert_eq!(saved(&h), Some(vec![BASE_MOD.to_string()]));
    h.click(&L::text("Singleplayer"));
    h.play_world("Alpha");
    let title = L::role(Role::Dialog, "Cannot open \"Alpha\"");
    h.wait_for(&title, SERVER);
    let reason = "The world could not be loaded: its base mod, Rustcraft, is off. Turn it on in \
        the Mods screen";
    assert!(h.is_visible(&L::text(reason).within(title)));
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn the_example_mods_show_what_they_need() {
    let examples = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../examples/mods");
    let mut folder = GameFolder::new();
    for id in ["shapes", "watchtower", "beacon"] {
        for file in ["mod.toml", "init.luau"] {
            let text = std::fs::read(examples.join(id).join(file)).unwrap();
            folder = folder.with_mod_file(id, file, text);
        }
    }
    let mut h = open_mods(folder);
    assert!(h.is_visible(&L::text("4 of 4 on")));
    assert!(h.find(&L::text("Cannot load")).is_empty());

    h.click(&L::marker::<ModSwitch>().within(card("Shapes", true)));
    h.wait_for(&card("Shapes", false), SCREEN);
    assert!(h.is_visible(&L::text("Needs shapes")));
    // only Watchtower waits: Beacon works without Shapes
    assert_eq!(h.find(&L::text("Needs shapes")).len(), 1);
    h.click(&card("Watchtower", true));
    h.wait_for(&L::text("Needs shapes, which is off"), SCREEN);
    h.click(&card("Beacon", true));
    h.wait_for(&L::text("Works with: shapes"), SCREEN);
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn the_mods_screen_matches_its_golden() {
    let mut h = open_mods(six_mods().with_file(MODS_SETTINGS_PATH, "(disabled: [\"woolly\"])"));
    h.hover_at(Vec2::ZERO);
    h.assert_screenshot("menu_mods");
}
