//! The main menu: its buttons lead to the other screens.

use bevy::prelude::*;

use super::harness::{Harness, SCREEN};
use super::locator::Locator as L;
use crate::ui::assets::{
    DARK_BUTTON_BACKGROUND_IMAGE_PATH, DARK_BUTTON_BACKGROUND_LARGE_IMAGE_PATH,
};
use crate::ui::button::MenuState;

/// A client on the main menu.
fn main_menu() -> Harness {
    let mut h = Harness::new();
    h.wait_for(&L::text("Singleplayer"), SCREEN);
    h
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn singleplayer_opens_the_world_list() {
    let mut h = main_menu();
    h.click(&L::text("Singleplayer"));
    h.wait_for(&L::text("World list"), SCREEN);
    assert_eq!(h.menu_state(), MenuState::Solo);
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn multiplayer_opens_the_server_list() {
    let mut h = main_menu();
    h.click(&L::text("Multiplayer"));
    h.wait_for(&L::text("Server list"), SCREEN);
    assert_eq!(h.menu_state(), MenuState::Multi);
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn settings_opens_the_settings() {
    let mut h = main_menu();
    h.click(&L::text("Settings"));
    h.wait_for(&L::text("Controls"), SCREEN);
    assert_eq!(h.menu_state(), MenuState::Settings);
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn quit_exits_the_game() {
    let mut h = main_menu();
    assert!(!h.has_exited());
    h.click(&L::text("Quit"));
    assert!(h.has_exited());
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn hovering_a_button_darkens_it() {
    let mut h = main_menu();
    h.hover(&L::text("Singleplayer"));
    h.frames(1);

    let label = h.get(&L::text("Singleplayer"));
    let button = h.world().get::<ChildOf>(label).unwrap().parent();
    let image = h.world().get::<ImageNode>(button).unwrap().image.clone();
    let assets = h.world().resource::<AssetServer>();
    let dark = [
        DARK_BUTTON_BACKGROUND_IMAGE_PATH,
        DARK_BUTTON_BACKGROUND_LARGE_IMAGE_PATH,
    ]
    .map(|path| assets.load::<Image>(path));
    assert!(dark.contains(&image), "the button shows {image:?}");
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn two_harnesses_run_side_by_side() {
    let mut first = main_menu();
    let mut second = main_menu();
    first.click(&L::text("Settings"));
    second.click(&L::text("Multiplayer"));
    first.wait_for(&L::text("Controls"), SCREEN);
    second.wait_for(&L::text("Server list"), SCREEN);
    assert_eq!(first.menu_state(), MenuState::Settings);
    assert_eq!(second.menu_state(), MenuState::Multi);
    assert_ne!(first.folder(), second.folder());
}
