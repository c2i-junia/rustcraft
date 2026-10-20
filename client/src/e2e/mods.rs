//! The Mods screen: the mods of the game folder, turned on and off one by one or all at once,
//! found by the search, and saved in `mods.ron` when the screen is left.

use accesskit::Role;
use bevy::prelude::*;
use shared::mods::{read_mods_settings, MODS_SETTINGS_PATH};

use super::harness::{Harness, SCREEN};
use super::helpers::main_menu_with;
use super::locator::Locator as L;
use super::seed::GameFolder;
use crate::ui::menus::mods::{ModList, ModSearch, ModSwitch, MODS_TITLE};
use crate::ui::menus::MenuState;

/// The texture of the torch, as the icon of Lanterns.
const TORCH: &[u8] = include_bytes!("../../../data/graphics/base_textures/blocks/Torch.png");

/// Six mods: three with a manifest, Lantern Paths needing Lanterns, which has an icon, one without a manifest,
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
        "Lanterns",
        "Lantern Paths",
        "Woolly Flocks",
        "tower",
        "broken",
        "typo",
    ] {
        assert!(h.is_visible(&card(name, true)), "{name}");
    }
    assert!(h.is_visible(&L::text("6 of 6 on")));

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
    assert!(h.is_visible(&L::text("5 of 6 on")));
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
    assert!(h.is_visible(&L::text("5 of 6 on")));
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
    h.wait_for(&L::text("0 of 6 on"), SCREEN);
    h.click(&L::role(Role::Button, "Turn all on"));
    h.wait_for(&L::text("6 of 6 on"), SCREEN);
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
fn without_mods_the_screen_says_where_to_put_them() {
    let mut h = open_mods(GameFolder::new());
    assert!(h.is_visible(&L::text("No mods yet")));
    assert!(h.is_visible(&L::text("0 of 0 on")));
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn the_mods_screen_matches_its_golden() {
    let mut h = open_mods(six_mods().with_file(MODS_SETTINGS_PATH, "(disabled: [\"woolly\"])"));
    h.hover_at(Vec2::ZERO);
    h.assert_screenshot("menu_mods");
}
