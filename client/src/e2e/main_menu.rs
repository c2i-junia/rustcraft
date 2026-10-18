//! The main menu: its buttons lead to the other screens, the Continue card plays the last world,
//! and quitting asks first.

use std::time::Duration;

use accesskit::Role;
use bevy::prelude::*;

use super::harness::{Harness, Options, SCREEN, SERVER};
use super::helpers::{is_in, main_menu};
use super::locator::Locator as L;
use super::seed::GameFolder;
use crate::constants::DAY_DURATION_IN_TICKS;
use crate::state::GameState;
use crate::ui::button::MenuState;
use crate::ui::menus::solo::SelectedWorld;
use crate::ui::menus::{ContinueCard, QUIT_TITLE};
use crate::ui::theme::{ACCENT, RAISE_HOVER};
use crate::ui::widgets::hints::HintBar;

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
    h.click(&L::text("Quit game"));
    h.wait_for(&L::text(QUIT_TITLE), SCREEN);
    assert!(!h.has_exited());
    h.click(&L::role(Role::Button, "Quit"));
    assert!(h.has_exited());
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn escape_asks_before_quitting_with_cancel_focused() {
    let mut h = main_menu();
    h.press(KeyCode::Escape);
    h.wait_for(&L::text(QUIT_TITLE), SCREEN);
    assert_eq!(h.focus_label().as_deref(), Some("Cancel"));
    assert!(!h.has_exited());

    // Escape closes the dialog, and the same key does not open it again
    h.press(KeyCode::Escape);
    assert!(!h.is_visible(&L::text(QUIT_TITLE)));
    h.frames(2);
    assert!(!h.is_visible(&L::text(QUIT_TITLE)), "the dialog reopened");
    assert!(!h.has_exited());
    assert_eq!(h.focus_label().as_deref(), Some("Singleplayer"));

    // Enter on Cancel closes it
    h.press(KeyCode::Escape);
    h.wait_for(&L::text(QUIT_TITLE), SCREEN);
    h.press(KeyCode::Enter);
    h.frames(2);
    assert!(!h.is_visible(&L::text(QUIT_TITLE)));
    assert!(!h.has_exited());

    // Tab then Enter quits
    h.press(KeyCode::Escape);
    h.wait_for(&L::text(QUIT_TITLE), SCREEN);
    h.press(KeyCode::Tab);
    h.press(KeyCode::Enter);
    h.frames(2);
    assert!(h.has_exited());
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn the_keyboard_alone_opens_a_screen() {
    let mut h = main_menu();
    h.frames(2);
    let ring = |h: &mut Harness, label: &str| {
        let button = h.get(&L::role(Role::Button, label));
        h.world().get::<Outline>(button).unwrap().color
    };
    assert_eq!(h.focus_label().as_deref(), Some("Singleplayer"));
    assert_eq!(ring(&mut h, "Singleplayer"), Color::NONE);

    h.press(KeyCode::ArrowDown);
    assert_eq!(h.focus_label().as_deref(), Some("Multiplayer"));
    h.press(KeyCode::Tab);
    assert_eq!(h.focus_label().as_deref(), Some("Settings"));
    assert_eq!(ring(&mut h, "Settings"), ACCENT);

    h.press(KeyCode::Enter);
    h.wait_for(&L::text("Controls"), SCREEN);
    assert_eq!(h.menu_state(), MenuState::Settings);
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn hovering_a_button_lightens_it() {
    let mut h = main_menu();
    let singleplayer = L::role(Role::Button, "Singleplayer");
    h.hover(&singleplayer);
    h.frames(1);
    let button = h.get(&singleplayer);
    assert_eq!(
        h.world().get::<BackgroundColor>(button).unwrap().0,
        RAISE_HOVER
    );
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn the_main_menu_shows_the_version_and_the_key_hints() {
    let mut h = main_menu();
    let version = L::text(&format!("v{}", env!("CARGO_PKG_VERSION")));
    assert!(h.is_visible(&version));
    let center = h.center_of(&version);
    assert!(center.x > 1280.0 * 3.0 / 4.0, "the version is at {center}");
    assert!(center.y < 100.0, "the version is at {center}");
    assert!(h.is_visible(&L::text("Esc").within(L::marker::<HintBar>())));
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

const HOUR: Duration = Duration::from_secs(60 * 60);

/// A client on the main menu, with the game folder `folder`.
#[track_caller]
fn main_menu_with(folder: GameFolder) -> Harness {
    let mut h = Harness::start(folder, Options::default());
    h.wait_for(&L::text("Singleplayer"), SCREEN);
    h
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn continue_shows_the_last_played_world() {
    let folder = GameFolder::new()
        .with_played_world("Old", 0, 5 * 24 * HOUR)
        .with_played_world("Recent", 3 * DAY_DURATION_IN_TICKS + 10, 2 * HOUR);
    let mut h = main_menu_with(folder);
    let card = || L::marker::<ContinueCard>();
    h.wait_for(&L::text("Recent").within(card()), SCREEN);
    assert!(h.is_visible(&L::text("Day 4").within(card())));
    assert!(h.is_visible(&L::text("played 2 hours ago").within(card())));
    assert!(!h.is_visible(&L::text("Old")));
    assert!(h.is_visible(&L::text("Ctrl+Enter")));
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn without_a_world_there_is_no_continue_card() {
    let mut h = main_menu();
    h.frames(2);
    assert!(!h.is_visible(&L::marker::<ContinueCard>()));
    assert!(!h.is_visible(&L::text("Ctrl+Enter")));
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn continue_plays_the_last_world_by_ctrl_enter_and_by_click() {
    let folder = GameFolder::new().with_played_world("Recent", 0, 2 * HOUR);
    let mut h = main_menu_with(folder);
    h.wait_for(&L::marker::<ContinueCard>(), SCREEN);
    assert_eq!(h.focus_label().as_deref(), Some("Singleplayer"));

    // Ctrl+Enter plays the last world, even with the focus on Singleplayer
    h.key_down(KeyCode::ControlLeft);
    h.press(KeyCode::Enter);
    h.key_up(KeyCode::ControlLeft);
    let selected = h.world().resource::<SelectedWorld>().name.clone();
    assert_eq!(selected.as_deref(), Some("Recent"));
    assert_ne!(h.menu_state(), MenuState::Solo);
    h.wait_for_game_state(GameState::Game, SERVER);

    // Back on the main menu, the world was just played, and Play plays it again
    h.press(KeyCode::Escape);
    h.click_when_ready(&L::text("Back to menu"));
    h.wait_until("the main menu", SERVER, |w| {
        is_in(w, GameState::Menu) && *w.resource::<State<MenuState>>().get() == MenuState::Main
    });
    h.wait_for(
        &L::text("played just now").within(L::marker::<ContinueCard>()),
        SCREEN,
    );
    h.click(&L::role(Role::Button, "Play"));
    h.wait_for_game_state(GameState::Game, SERVER);
}
