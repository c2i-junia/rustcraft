//! The world list: creating, deleting and playing the saved worlds.

use accesskit::Role;
use bevy::input::keyboard::KeyCode;

use super::harness::{Harness, SCREEN, SERVER};
use super::locator::Locator as L;
use super::seed::GameFolder;
use crate::state::GameState;
use crate::ui::button::MenuState;
use crate::ui::menus::solo::{WorldNameInput, INVALID_WORLD_NAME, WORLD_EXISTS};

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
    h.click_when_ready(&L::text("Singleplayer"));
    h.wait_for(&L::text("World list"), SCREEN);
    h
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn the_world_list_matches_its_golden() {
    let mut h = world_list();
    h.wait_for_icons("Test world 3");
    // The caret of the focused name field blinks on the wall clock: a click elsewhere clears the
    // focus, so that the screenshot shows no caret
    h.click(&L::text("World list"));
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

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn a_typed_name_and_create_add_a_world() {
    let mut h = world_list();
    h.type_text("Nouveau");
    h.click(&L::text("Create world"));
    h.wait_for(&L::name("Play Nouveau"), SCREEN);
    assert!(h.is_visible(&L::text("Nouveau")));
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn enter_in_the_world_name_creates_the_world() {
    let mut h = world_list();
    h.type_text("Entered");
    h.press(KeyCode::Enter);
    h.wait_for(&L::name("Play Entered"), SCREEN);
    assert_eq!(h.value_of::<WorldNameInput>(), "");
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn after_create_a_click_on_the_field_types_again() {
    let mut h = world_list();
    h.type_text("One");
    h.click(&L::text("Create world"));
    h.wait_for(&L::name("Play One"), SCREEN);
    h.click(&L::role(Role::TextInput, "World name"));
    h.type_text("Two");
    h.press(KeyCode::Enter);
    h.wait_for(&L::name("Play Two"), SCREEN);
    assert_eq!(h.find(&L::name("Play One")).len(), 1);
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn an_invalid_name_adds_no_world() {
    let mut h = world_list();
    h.wait_for(&L::name("Play Survie"), SCREEN);
    h.type_text("bad/name");
    assert_eq!(h.value_of::<WorldNameInput>(), "bad/name");
    h.click(&L::text("Create world"));
    // The click frame handles the button: two more frames are plenty for a negative check
    h.frames(2);
    assert_eq!(h.find(&L::name("Play bad/name")).len(), 0);
    assert_eq!(h.play_rows(), 3);
    assert!(h.is_visible(&L::text(INVALID_WORLD_NAME)));
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn a_duplicate_name_adds_no_world() {
    let mut h = world_list();
    h.wait_for(&L::name("Play Survie"), SCREEN);
    h.type_text("Survie");
    h.click(&L::text("Create world"));
    // The click frame handles the button: two more frames are plenty for a negative check
    h.frames(2);
    assert_eq!(h.find(&L::name("Play Survie")).len(), 1);
    assert!(h.is_visible(&L::text(WORLD_EXISTS)));
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
