//! The world list: creating, deleting and playing the saved worlds.

use std::fs;

use accesskit::Role;
use bevy::input::keyboard::KeyCode;
use bevy::math::Vec2;
use bevy::ui::InteractionDisabled;
use server::{WorldMeta, WORLD_FILE};

use super::harness::{Harness, SCREEN, SERVER, STARTUP};
use super::helpers::ctrl_n;
use super::locator::Locator as L;
use super::seed::GameFolder;
use crate::constants::SAVE_PATH;
use crate::state::GameState;
use crate::ui::button::MenuState;
use crate::ui::menus::solo::dialogs::{WorldNameInput, WorldSeedInput, NEW_WORLD_TITLE};
use crate::ui::menus::worlds::{EMPTY_WORLD_NAME, INVALID_SEED, INVALID_WORLD_NAME, WORLD_EXISTS};

/// A client on the world list, with the worlds `Survie`, `Creative build` and `Test world 3`.
fn world_list() -> Harness {
    let folder = GameFolder::new()
        .with_world("Survie")
        .with_world("Creative build")
        .with_world("Test world 3");
    open_world_list(folder)
}

fn open_world_list(folder: GameFolder) -> Harness {
    let mut h = Harness::start(folder, Default::default());
    h.wait_for(&L::text("Singleplayer"), STARTUP);
    h.click(&L::text("Singleplayer"));
    h.wait_for(&L::text("World list"), SCREEN);
    h
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn the_world_list_matches_its_golden() {
    let mut h = world_list();
    h.wait_for_icons("Test world 3");
    // The cursor in a corner, so that it hovers nothing
    h.hover_at(Vec2::ZERO);
    h.assert_screenshot("menu_solo");
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn the_saved_worlds_are_listed_in_order() {
    let mut h = world_list();
    h.wait_for(&L::text("Test world 3"), SCREEN);
    let y = |h: &mut Harness, name| h.center_of(&L::text(name)).y;
    let (creative, survie, test) = (
        y(&mut h, "Creative build"),
        y(&mut h, "Survie"),
        y(&mut h, "Test world 3"),
    );
    assert!(
        creative < survie && survie < test,
        "{creative} {survie} {test}"
    );
}

/// The New world button of the world list.
fn new_world_button() -> L {
    L::role(Role::Button, "New world")
}

/// The Create button of the New world dialog.
fn create_button() -> L {
    L::role(Role::Button, "Create")
}

/// Whether the New world dialog is open.
fn dialog_is_open(h: &mut Harness) -> bool {
    h.is_visible(&L::role(Role::Dialog, NEW_WORLD_TITLE))
}

/// The names of the folders in the saves folder, sorted.
fn saved_folders(h: &Harness) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(h.folder().join(SAVE_PATH))
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect();
    names.sort();
    names
}

/// The `world.ron` of the world `name`.
fn world_meta(h: &Harness, name: &str) -> WorldMeta {
    let path = h.folder().join(SAVE_PATH).join(name).join(WORLD_FILE);
    ron::de::from_str(&fs::read_to_string(path).unwrap()).unwrap()
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn new_world_opens_a_dialog_with_a_free_name_ready_to_replace() {
    let mut h = open_world_list(GameFolder::new().with_world("New world"));
    h.wait_for(&L::name("Play New world"), SCREEN);
    h.click(&new_world_button());
    h.wait_for(&L::role(Role::Dialog, NEW_WORLD_TITLE), SCREEN);
    h.frames(2);
    assert_eq!(h.focus_label().as_deref(), Some("Name"));
    assert_eq!(h.value_of::<WorldNameInput>(), "New world 2");
    h.type_text("Base");
    assert_eq!(h.value_of::<WorldNameInput>(), "Base");
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn ctrl_n_then_enter_creates_one_world_on_disk() {
    let mut h = world_list();
    h.wait_for(&L::name("Play Survie"), SCREEN);
    ctrl_n(&mut h);
    h.type_text("Entered");
    h.press(KeyCode::Enter);
    h.frames(3);
    assert!(!dialog_is_open(&mut h));
    assert_eq!(h.find(&L::name("Play Entered")).len(), 1);
    assert_eq!(
        saved_folders(&h),
        ["Creative build", "Entered", "Survie", "Test world 3"]
    );
    let meta = world_meta(&h, "Entered");
    assert_eq!(meta.name, "Entered");
    assert_eq!(meta.time, 0);
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn a_typed_name_and_create_add_a_world() {
    let mut h = world_list();
    h.click_when_ready(&new_world_button());
    h.wait_for(&L::role(Role::Dialog, NEW_WORLD_TITLE), SCREEN);
    h.type_text("Nouveau");
    h.click(&create_button());
    h.wait_for(&L::name("Play Nouveau"), SCREEN);
    assert!(h.is_visible(&L::text("Nouveau")));
    assert!(h.folder().join(SAVE_PATH).join("Nouveau").is_dir());
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn a_typed_seed_is_kept() {
    let mut h = world_list();
    h.wait_for(&L::name("Play Survie"), SCREEN);
    ctrl_n(&mut h);
    h.type_text("Seeded");
    h.press(KeyCode::Tab);
    h.type_text("12ab34");
    assert_eq!(h.value_of::<WorldSeedInput>(), "1234");
    h.press(KeyCode::Enter);
    h.wait_for(&L::name("Play Seeded"), SCREEN);
    assert_eq!(world_meta(&h, "Seeded").seed.0, 1234);
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn the_name_is_checked_as_you_type_and_nothing_moves() {
    let mut h = world_list();
    h.wait_for(&L::name("Play Survie"), SCREEN);
    ctrl_n(&mut h);
    let y = h.center_of(&create_button()).y;
    let create_disabled = |h: &mut Harness| {
        let create = h.get(&create_button());
        h.world().get::<InteractionDisabled>(create).is_some()
    };
    let messages = [
        EMPTY_WORLD_NAME,
        INVALID_WORLD_NAME,
        WORLD_EXISTS,
        INVALID_SEED,
    ];

    h.type_text("bad/name");
    h.wait_for(&L::text(INVALID_WORLD_NAME), SCREEN);
    assert!(create_disabled(&mut h));
    assert_eq!(h.center_of(&create_button()).y, y);
    h.press(KeyCode::Enter);
    h.frames(2);
    assert!(dialog_is_open(&mut h));
    assert_eq!(h.play_rows(), 3);

    for _ in 0..8 {
        h.press(KeyCode::Backspace);
    }
    h.wait_for(&L::text(EMPTY_WORLD_NAME), SCREEN);
    assert!(create_disabled(&mut h));
    h.type_text("survie");
    h.wait_for(&L::text(WORLD_EXISTS), SCREEN);
    assert!(create_disabled(&mut h));
    assert_eq!(h.center_of(&create_button()).y, y);
    h.type_text("2");
    h.frames(2);
    for message in messages {
        assert!(!h.is_visible(&L::text(message)), "{message}");
    }
    assert!(!create_disabled(&mut h));

    h.press(KeyCode::Tab);
    h.type_text("4294967296");
    h.wait_for(&L::text(INVALID_SEED), SCREEN);
    assert!(create_disabled(&mut h));
    assert_eq!(h.center_of(&create_button()).y, y);
    h.press(KeyCode::Backspace);
    h.frames(2);
    assert!(!h.is_visible(&L::text(INVALID_SEED)));
    assert!(!create_disabled(&mut h));
    assert_eq!(h.center_of(&create_button()).y, y);
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn a_failed_create_says_why_and_keeps_the_dialog_open() {
    // A folder without a world.ron is not listed, yet its name is taken on disk
    let folder = GameFolder::new().with_world("Survie");
    fs::create_dir(folder.path().join(SAVE_PATH).join("Taken")).unwrap();
    let mut h = open_world_list(folder);
    h.wait_for(&L::name("Play Survie"), SCREEN);
    ctrl_n(&mut h);
    h.type_text("Taken");
    h.press(KeyCode::Enter);
    let failure = L::text("A folder with this name already exists");
    h.wait_for(&failure, SCREEN);
    assert!(dialog_is_open(&mut h));
    assert_eq!(h.play_rows(), 1);
    h.type_text("2");
    h.frames(2);
    assert!(!h.is_visible(&failure));
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn escape_cancels_the_dialog_and_stays_on_the_list() {
    let mut h = world_list();
    h.wait_for(&L::name("Play Survie"), SCREEN);
    ctrl_n(&mut h);
    h.press(KeyCode::Escape);
    assert!(!dialog_is_open(&mut h));
    assert_eq!(h.menu_state(), MenuState::Solo);
    h.frames(2);
    assert_eq!(h.menu_state(), MenuState::Solo);
    assert_eq!(
        saved_folders(&h),
        ["Creative build", "Survie", "Test world 3"]
    );
    h.press(KeyCode::Escape);
    h.wait_for(&L::text("Singleplayer"), SCREEN);
    assert_eq!(h.menu_state(), MenuState::Main);
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn after_a_create_new_world_opens_with_a_fresh_name() {
    let mut h = world_list();
    h.click_when_ready(&new_world_button());
    h.wait_for(&L::role(Role::Dialog, NEW_WORLD_TITLE), SCREEN);
    h.type_text("One");
    h.click(&create_button());
    h.wait_for(&L::name("Play One"), SCREEN);
    ctrl_n(&mut h);
    h.frames(2);
    assert_eq!(h.value_of::<WorldNameInput>(), "New world");
    h.type_text("Two");
    h.press(KeyCode::Enter);
    h.wait_for(&L::name("Play Two"), SCREEN);
    assert_eq!(h.find(&L::name("Play One")).len(), 1);
    assert_eq!(h.find(&L::name("Play Two")).len(), 1);
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn deleting_a_world_removes_its_row_and_folder() {
    let mut h = world_list();
    h.wait_for_icons("Survie");
    assert!(h.folder().join("saves/Survie").exists());
    h.click(&L::name("Delete Survie"));
    h.frames(2);
    assert!(!h.is_visible(&L::text("Survie")));
    assert!(!h.folder().join("saves/Survie").exists());
    assert!(h.is_visible(&L::text("Creative build")));
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn back_and_escape_return_to_the_main_menu() {
    let mut h = world_list();
    h.click(&L::text("Back to menu"));
    h.wait_for(&L::text("Singleplayer"), SCREEN);
    assert_eq!(h.menu_state(), MenuState::Main);

    h.click(&L::text("Singleplayer"));
    h.wait_for(&L::text("World list"), SCREEN);
    h.press(KeyCode::Escape);
    h.wait_for(&L::text("Singleplayer"), SCREEN);
    assert_eq!(h.menu_state(), MenuState::Main);
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn the_wheel_scrolls_a_long_list() {
    let mut folder = GameFolder::new();
    for i in 1..=15 {
        folder = folder.with_world(&format!("world {i:02}"));
    }
    let mut h = open_world_list(folder);
    h.wait_for(&L::text("world 01"), SCREEN);
    let before = h.center_of(&L::text("world 01")).y;
    h.hover(&L::text("world 01"));
    h.scroll(-5.);
    h.frames(2);
    let after = h.center_of(&L::text("world 01")).y;
    assert!(after < before, "the list stayed at {before}, then {after}");
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn playing_a_world_connects_then_enters_the_game() {
    let mut h = world_list();
    h.wait_for_icons("Survie");
    h.click(&L::name("Play Survie"));
    assert_eq!(h.game_state(), GameState::PreGameLoading);
    h.wait_for_game_state(GameState::Game, SERVER);
}
