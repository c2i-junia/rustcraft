//! The world list: creating, deleting and playing the saved worlds.

use bevy::{
    input::keyboard::KeyCode,
    prelude::{Entity, State},
    prelude::{Name, With},
    ui::UiGlobalTransform,
};

use super::harness::{Harness, SCREEN, SERVER};
use super::locator::Locator as L;
use super::seed::GameFolder;
use crate::state::GameState;
use crate::ui::button::MenuState;
use crate::ui::menus::solo::WorldNameInput;
use bevy_simple_text_input::TextInputValue;

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
    h.click_when_ready("Singleplayer");
    h.wait_for(&L::text("World list"), SCREEN);
    h
}

impl Harness {
    pub(super) fn click_when_ready(&mut self, text: &str) {
        self.wait_for(&L::text(text), SCREEN);
        self.click(&L::text(text));
    }

    /// Waits until the icons of a row are laid out side by side: they are only borders until
    /// their images are loaded, and a click would then hit the wrong one.
    pub(super) fn wait_for_icons(&mut self, world: &str) {
        let play = self.wait_for_named(&format!("Play {world}"));
        let delete = self.wait_for_named(&format!("Delete {world}"));
        self.wait_until("the icons of the row", SCREEN, |w| {
            let x = |e| w.get::<UiGlobalTransform>(e).unwrap().translation.x;
            x(delete) - x(play) > 20.0
        });
    }

    pub(super) fn wait_for_named(&mut self, name: &str) -> Entity {
        self.wait_for(&L::name(name), SCREEN);
        self.get(&L::name(name))
    }
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn the_world_list_matches_its_golden() {
    let mut h = world_list();
    h.wait_for_icons("Test world 3");
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
fn an_invalid_name_adds_no_world() {
    let mut h = world_list();
    h.wait_for(&L::name("Play Survie"), SCREEN);
    h.type_text("bad/name");
    let typed = h
        .world_mut()
        .query_filtered::<&TextInputValue, With<WorldNameInput>>()
        .single(h.world())
        .unwrap()
        .0
        .clone();
    assert_eq!(typed, "bad/name");
    h.click(&L::text("Create world"));
    // The click frame handles the button: two more frames are plenty for a negative check
    h.frames(2);
    assert_eq!(h.find(&L::name("Play bad/name")).len(), 0);
    let rows = h
        .world_mut()
        .query::<&Name>()
        .iter(h.world())
        .filter(|n| n.starts_with("Play "))
        .count();
    assert_eq!(rows, 3);
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
    h.wait_until("the game", SERVER, |w| {
        *w.resource::<State<GameState>>().get() == GameState::Game
    });
}
