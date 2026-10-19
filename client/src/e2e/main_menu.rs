//! The main menu: its buttons lead to the other screens, the Continue card plays the last world,
//! and quitting asks first.

use std::time::{Duration, Instant};

use accesskit::Role;
use bevy::prelude::*;
use bevy::ui::UiGlobalTransform;

use super::harness::{Harness, Options, SCREEN, SERVER, STARTUP};
use super::helpers::{is_in, main_menu, main_menu_with};
use super::locator::Locator as L;
use super::seed::GameFolder;
use crate::constants::DAY_DURATION_IN_TICKS;
use crate::state::GameState;
use crate::ui::assets::TITLE_IMAGE_PATH;
use crate::ui::menus::multi::SERVERS_TITLE;
use crate::ui::menus::solo::{SelectedWorld, WORLDS_TITLE};
use crate::ui::menus::MenuState;
use crate::ui::menus::{ContinueCard, QUIT_TITLE};
use crate::ui::theme::{ACCENT, RAISE_HOVER};
use crate::ui::widgets::hints::HintBar;

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn singleplayer_opens_the_world_list() {
    let mut h = main_menu();
    h.click(&L::text("Singleplayer"));
    h.wait_for(&L::text(WORLDS_TITLE), SCREEN);
    assert_eq!(h.menu_state(), MenuState::Solo);
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn multiplayer_opens_the_server_list() {
    let mut h = main_menu();
    h.click(&L::text("Multiplayer"));
    h.wait_for(&L::text(SERVERS_TITLE), SCREEN);
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
    // opened from the keyboard, the dialog shows where Enter goes
    h.frames(1);
    let cancel = h.get(&L::role(Role::Button, "Cancel"));
    assert_eq!(h.world().get::<Outline>(cancel).unwrap().color, ACCENT);
    // and its own key hints, in place of the menu's
    let hints = L::marker::<HintBar>();
    assert_eq!(h.text_of(&hints), "Tab Move Enter Select Esc Cancel");

    // Escape closes the dialog, and the same key does not open it again
    h.press(KeyCode::Escape);
    assert!(!h.is_visible(&L::text(QUIT_TITLE)));
    h.frames(2);
    assert!(!h.is_visible(&L::text(QUIT_TITLE)), "the dialog reopened");
    assert!(!h.has_exited());
    assert_eq!(h.focus_label().as_deref(), Some("Singleplayer"));
    assert!(
        h.text_of(&hints).ends_with("Esc Quit"),
        "{}",
        h.text_of(&hints)
    );

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
    second.wait_for(&L::text(SERVERS_TITLE), SCREEN);
    assert_eq!(first.menu_state(), MenuState::Settings);
    assert_eq!(second.menu_state(), MenuState::Multi);
    assert_ne!(first.folder(), second.folder());
}

const HOUR: Duration = Duration::from_secs(60 * 60);

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
    // the world list never opens on the way into the game
    let start = Instant::now();
    while h.game_state() != GameState::Game {
        assert_ne!(h.menu_state(), MenuState::Solo);
        assert!(
            !h.is_visible(&L::text(WORLDS_TITLE)),
            "the world list opened"
        );
        assert!(start.elapsed() < SERVER, "timed out waiting for the game");
        h.frames(1);
    }

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

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn the_logo_starts_where_the_buttons_start() {
    let full_hd = Options {
        width: 1920,
        height: 1080,
        ..default()
    };
    let mut h = Harness::start(GameFolder::new(), full_hd);
    h.wait_for(&L::text("Singleplayer"), STARTUP);
    h.wait_for_ui_images();
    h.frames(1);
    let left_of = |world: &World, entity: Entity| {
        let size = world.get::<ComputedNode>(entity).unwrap().size();
        world
            .get::<UiGlobalTransform>(entity)
            .unwrap()
            .translation
            .x
            - size.x / 2.
    };
    let button = h.get(&L::role(Role::Button, "Singleplayer"));
    let world = h.world_mut();
    let title: Handle<Image> = world.resource::<AssetServer>().load(TITLE_IMAGE_PATH);
    let logo = world
        .query::<(Entity, &ImageNode)>()
        .iter(world)
        .find(|(_, node)| node.image.id() == title.id())
        .map(|(entity, _)| entity)
        .expect("the logo");
    // the first column of the image where the letters start: its edges are transparent
    let image = world.resource::<Assets<Image>>().get(&title).unwrap();
    let ink = (0..image.width())
        .find(|&x| (0..image.height()).any(|y| image.get_color_at(x, y).unwrap().alpha() > 0.5))
        .unwrap();
    let logo_width = world.get::<ComputedNode>(logo).unwrap().size().x;
    let letters = left_of(world, logo) + ink as f32 * logo_width / image.width() as f32;
    let buttons = left_of(world, button);
    assert!((letters - buttons).abs() < 1., "{letters} and {buttons}");
}
