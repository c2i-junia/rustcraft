//! The world list: the world cards and the detail pane, creating, deleting and playing the
//! saved worlds.

use std::fs;
use std::time::Duration;

use accesskit::Role;
use bevy::ecs::system::SystemState;
use bevy::input::keyboard::KeyCode;
use bevy::input_focus::{InputFocus, InputFocusVisible};
use bevy::math::Vec2;
use bevy::prelude::{BackgroundColor, Children, Entity, Query, With};
use bevy::ui::{InteractionDisabled, Selected};
use bevy::ui_widgets::Scrollbar;
use server::{WorldMeta, WORLD_FILE};

use super::harness::{Harness, Options, SCREEN, SERVER, STARTUP};
use super::helpers::{ctrl_n, main_menu_with};
use super::locator::Locator as L;
use super::seed::GameFolder;
use crate::constants::{DAY_DURATION_IN_TICKS, SAVE_PATH};
use crate::state::GameState;
use crate::ui::menus::solo::dialogs::DELETE_PERMISSION;
use crate::ui::menus::solo::dialogs::{WorldNameInput, WorldSeedInput, NEW_WORLD_TITLE};
use crate::ui::menus::solo::{SelectedWorld, WorldCard, WorldDetail, WorldList, WORLDS_TITLE};
use crate::ui::menus::worlds::{
    list_worlds, EMPTY_WORLD_NAME, INVALID_SEED, INVALID_WORLD_NAME, WORLD_EXISTS,
};
use crate::ui::menus::ContinueCard;
use crate::ui::menus::MenuState;
use crate::ui::theme::RAISE_HOVER;
use crate::ui::widgets::hints::HintBar;
use crate::ui::widgets::list::cards_of;

const HOUR: Duration = Duration::from_secs(60 * 60);
const DAY: Duration = Duration::from_secs(24 * 60 * 60);

/// The worlds `Survie` (seed 3, day 4, played 3 days ago), `Creative build` (seed 1, 2 hours
/// ago) and `Test world 3` (seed 5, 20 days ago).
fn three_worlds() -> GameFolder {
    GameFolder::new()
        .with_seeded_world("Survie", 3, 3 * DAY_DURATION_IN_TICKS, 3 * DAY)
        .with_seeded_world("Creative build", 1, 0, 2 * HOUR)
        .with_seeded_world("Test world 3", 5, 0, 20 * DAY)
}

/// A client on the world list, with the worlds of `three_worlds`.
fn world_list() -> Harness {
    open_world_list(three_worlds())
}

fn open_world_list(folder: GameFolder) -> Harness {
    open_world_list_with(folder, Options::default())
}

fn open_world_list_with(folder: GameFolder, options: Options) -> Harness {
    let mut h = Harness::start(folder, options);
    h.wait_for(&L::text("Singleplayer"), STARTUP);
    h.click(&L::text("Singleplayer"));
    h.wait_for(&L::text(WORLDS_TITLE), SCREEN);
    h
}

/// The card of the world `name`.
fn card(name: &str) -> L {
    L::role(Role::ListItem, name)
}

/// The names of the world cards, from the top of the list.
fn card_names(h: &mut Harness) -> Vec<String> {
    let world = h.world_mut();
    let list = world
        .query_filtered::<Entity, With<WorldList>>()
        .single(world)
        .unwrap();
    let mut state = SystemState::<Query<&Children>>::new(world);
    let children = state.get(world).unwrap();
    cards_of(list, &children)
        .into_iter()
        .filter_map(|card| world.get::<WorldCard>(card))
        .map(|card| card.0.name.clone())
        .collect()
}

/// The name of the world of the selected card.
pub(super) fn selected_world(h: &mut Harness) -> Option<String> {
    let world = h.world_mut();
    let mut selected = world.query_filtered::<&WorldCard, With<Selected>>();
    let names: Vec<String> = selected
        .iter(world)
        .map(|card| card.0.name.clone())
        .collect();
    assert!(names.len() <= 1, "several cards are selected: {names:?}");
    names.into_iter().next()
}

/// Whether the detail pane shows the text `text`.
fn pane_shows(h: &mut Harness, text: &str) -> bool {
    h.is_visible(&L::text(text).within(L::marker::<WorldDetail>()))
}

impl Harness {
    /// Plays the world `world` from the world list: selects its card, then Play in the pane.
    #[track_caller]
    pub(super) fn play_world(&mut self, world: &str) {
        self.wait_for(&card(world), SCREEN);
        self.click(&card(world));
        self.click(&L::role(Role::Button, "Play"));
    }
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn the_world_list_matches_its_golden() {
    let mut h = world_list();
    h.wait_for(&card("Test world 3"), SCREEN);
    // The cursor in a corner, so that it hovers nothing
    h.hover_at(Vec2::ZERO);
    h.assert_screenshot("menu_solo");
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn the_empty_world_list_matches_its_golden() {
    let mut h = open_world_list(GameFolder::new());
    h.wait_for(&L::text("No worlds yet"), SCREEN);
    h.hover_at(Vec2::ZERO);
    h.assert_screenshot("menu_solo_empty");
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn the_saved_worlds_are_listed_by_last_played() {
    let mut h = world_list();
    h.wait_for(&card("Test world 3"), SCREEN);
    assert_eq!(
        card_names(&mut h),
        ["Creative build", "Survie", "Test world 3"]
    );
    for text in ["Survie", "Day 4 \u{b7} Seed 3", "played 3 days ago", "1 KB"] {
        assert!(
            h.is_visible(&L::text(text).within(card("Survie"))),
            "{text}"
        );
    }
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn the_first_world_is_selected_and_shown_in_the_detail_pane() {
    let mut h = world_list();
    h.wait_for(&card("Creative build"), SCREEN);
    h.frames(1);
    assert_eq!(selected_world(&mut h).as_deref(), Some("Creative build"));
    assert_eq!(h.focus_label().as_deref(), Some("Creative build"));
    for text in [
        "Creative build",
        "Seed 1",
        "played 2 hours ago",
        "1 KB on disk",
    ] {
        assert!(pane_shows(&mut h, text), "{text}");
    }
    h.click(&card("Survie"));
    h.frames(1);
    assert!(pane_shows(&mut h, "Day 4"));
    assert!(pane_shows(&mut h, "Survie"));
    assert_eq!(selected_world(&mut h).as_deref(), Some("Survie"));
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn the_keyboard_alone_walks_the_worlds_and_plays_one() {
    let mut h = main_menu_with(three_worlds());
    assert_eq!(h.focus_label().as_deref(), Some("Singleplayer"));
    h.press(KeyCode::Enter);
    h.wait_for(&L::text(WORLDS_TITLE), SCREEN);
    assert_eq!(h.focus_label().as_deref(), Some("Creative build"));
    // the Enter that opened the list did not play its first world
    h.frames(3);
    assert_eq!(h.game_state(), GameState::Menu);

    let step = |h: &mut Harness, key: KeyCode, world: &str| {
        h.press(key);
        assert_eq!(h.focus_label().as_deref(), Some(world), "{key:?}");
        assert_eq!(selected_world(h).as_deref(), Some(world), "{key:?}");
        assert!(pane_shows(h, world), "{key:?}: the pane shows {world}");
    };
    step(&mut h, KeyCode::ArrowDown, "Survie");
    step(&mut h, KeyCode::End, "Test world 3");
    step(&mut h, KeyCode::Home, "Creative build");

    // Tab crosses the list in one stop, to the pane
    h.press(KeyCode::Tab);
    assert_eq!(h.focus_label().as_deref(), Some("Play"));
    h.key_down(KeyCode::ShiftLeft);
    h.press(KeyCode::Tab);
    h.key_up(KeyCode::ShiftLeft);
    assert_eq!(h.focus_label().as_deref(), Some("Creative build"));

    step(&mut h, KeyCode::ArrowDown, "Survie");
    h.press(KeyCode::Enter);
    let selected = h.world().resource::<SelectedWorld>().name.clone();
    assert_eq!(selected.as_deref(), Some("Survie"));
    h.wait_for_game_state(GameState::Game, SERVER);
}

/// The Delete button of the detail pane.
fn pane_delete() -> L {
    L::role(Role::Button, "Delete").within(L::marker::<WorldDetail>())
}

/// Deletes the world `name` through the detail pane and the dialog.
#[track_caller]
fn delete_through_the_pane(h: &mut Harness, name: &str) {
    h.click(&card(name));
    h.click(&pane_delete());
    let dialog = L::role(Role::Dialog, &format!("Delete \"{name}\"?"));
    h.wait_for(&dialog, SCREEN);
    h.click(&L::role(Role::Button, "Delete").within(dialog));
    h.frames(2);
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn delete_asks_first_and_the_focus_moves_to_the_next_world() {
    let mut h = world_list();
    let survie = h.folder().join(SAVE_PATH).join("Survie");
    h.click_when_ready(&card("Survie"));
    h.press(KeyCode::Delete);
    let dialog = L::role(Role::Dialog, "Delete \"Survie\"?");
    h.wait_for(&dialog, SCREEN);
    assert!(h.is_visible(&L::text("Its 1 KB of saves will be deleted for good.")));
    assert_eq!(h.focus_label().as_deref(), Some("Cancel"));

    h.press(KeyCode::Escape);
    assert!(!h.is_visible(&dialog));
    h.frames(2);
    assert_eq!(h.menu_state(), MenuState::Solo);
    assert!(survie.exists());

    h.click(&pane_delete());
    h.wait_for(&dialog, SCREEN);
    h.click(&L::role(Role::Button, "Delete").within(dialog));
    h.frames(2);
    assert!(!survie.exists());
    assert_eq!(card_names(&mut h), ["Creative build", "Test world 3"]);
    assert_eq!(h.focus_label().as_deref(), Some("Test world 3"));
    assert_eq!(selected_world(&mut h).as_deref(), Some("Test world 3"));

    delete_through_the_pane(&mut h, "Test world 3");
    assert_eq!(h.focus_label().as_deref(), Some("Creative build"));
    delete_through_the_pane(&mut h, "Creative build");
    assert!(h.is_visible(&L::text("No worlds yet")));
    assert!(!h.is_visible(&L::marker::<WorldDetail>()));
    assert_eq!(h.focus_label().as_deref(), Some("New world"));
}

#[cfg(unix)]
#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn a_failed_delete_shows_what_is_left_on_disk_and_says_why() {
    use std::os::unix::fs::PermissionsExt;

    let mut h = world_list();
    let saves = h.folder().join(SAVE_PATH);
    let survie = saves.join("Survie");
    // a folder whose file cannot be removed
    let locked = survie.join("locked");
    fs::create_dir(&locked).unwrap();
    fs::write(locked.join("region"), "x").unwrap();
    h.click_when_ready(&card("Survie"));
    h.click(&pane_delete());
    let dialog = L::role(Role::Dialog, "Delete \"Survie\"?");
    h.wait_for(&dialog, SCREEN);
    let confirm = h.center_of(&L::role(Role::Button, "Delete").within(dialog));
    // world.ron goes before the delete fails: the world is half deleted, and no longer listed
    fs::remove_file(survie.join(WORLD_FILE)).unwrap();
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o555)).unwrap();
    h.click_at(confirm);
    h.frames(2);
    // put back before anything can panic: a failed test drops nothing
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).unwrap();

    let on_disk: Vec<String> = list_worlds(&saves).into_iter().map(|w| w.name).collect();
    assert_eq!(card_names(&mut h), on_disk);
    assert_eq!(card_names(&mut h), ["Creative build", "Test world 3"]);
    let failure = L::text(DELETE_PERMISSION).within(L::marker::<WorldDetail>());
    assert!(h.is_visible(&failure));
    // another world selected, the message goes
    h.click(&card("Test world 3"));
    h.frames(1);
    assert!(!h.is_visible(&failure));
}

#[cfg(unix)]
#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn a_failed_delete_of_the_last_world_says_why_in_the_empty_list() {
    use std::os::unix::fs::PermissionsExt;

    let folder = GameFolder::new().with_seeded_world("Survie", 3, 0, DAY);
    let mut h = open_world_list(folder);
    let survie = h.folder().join(SAVE_PATH).join("Survie");
    let locked = survie.join("locked");
    fs::create_dir(&locked).unwrap();
    fs::write(locked.join("region"), "x").unwrap();
    h.click_when_ready(&card("Survie"));
    h.click(&pane_delete());
    let dialog = L::role(Role::Dialog, "Delete \"Survie\"?");
    h.wait_for(&dialog, SCREEN);
    let confirm = h.center_of(&L::role(Role::Button, "Delete").within(dialog));
    fs::remove_file(survie.join(WORLD_FILE)).unwrap();
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o555)).unwrap();
    h.click_at(confirm);
    h.frames(2);
    // put back before anything can panic: a failed test drops nothing
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).unwrap();

    assert!(h.is_visible(&L::text("No worlds yet")));
    assert!(h.is_visible(&L::text(DELETE_PERMISSION)));
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn escape_from_delete_gives_the_focus_back_to_its_opener() {
    let mut h = world_list();
    let dialog = L::role(Role::Dialog, "Delete \"Survie\"?");
    h.click_when_ready(&card("Survie"));
    h.press(KeyCode::Delete);
    h.wait_for(&dialog, SCREEN);
    h.press(KeyCode::Escape);
    assert!(!h.is_visible(&dialog));
    assert_eq!(h.focus_label().as_deref(), Some("Survie"));

    h.click(&pane_delete());
    h.wait_for(&dialog, SCREEN);
    h.press(KeyCode::Escape);
    assert!(!h.is_visible(&dialog));
    let focused = h.world().resource::<InputFocus>().get();
    assert_eq!(focused, Some(h.get(&pane_delete())));
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn the_empty_screen_tabs_past_the_hidden_pane() {
    let mut h = open_world_list(GameFolder::new());
    h.wait_for(&L::text("No worlds yet"), SCREEN);
    assert_eq!(h.focus_label().as_deref(), Some("New world"));
    h.press(KeyCode::Tab);
    assert_eq!(h.focus_label().as_deref(), Some("Back"));
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn the_continue_card_is_the_first_world_of_the_list() {
    let mut h = main_menu_with(three_worlds());
    let continue_card = || L::marker::<ContinueCard>();
    h.wait_for(&L::text("Creative build").within(continue_card()), SCREEN);
    h.click(&L::text("Singleplayer"));
    h.wait_for(&card("Creative build"), SCREEN);
    assert_eq!(card_names(&mut h)[0], "Creative build");
    delete_through_the_pane(&mut h, "Creative build");
    h.press(KeyCode::Escape);
    h.wait_for(&L::text("Survie").within(continue_card()), SCREEN);
    assert!(h.is_visible(&L::text("played 3 days ago").within(continue_card())));
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn a_created_world_is_first_selected_and_focused() {
    let mut h = world_list();
    h.wait_for(&card("Survie"), SCREEN);
    ctrl_n(&mut h);
    h.type_text("Fresh");
    h.press(KeyCode::Enter);
    h.frames(2);
    assert_eq!(card_names(&mut h)[0], "Fresh");
    assert_eq!(h.focus_label().as_deref(), Some("Fresh"));
    assert_eq!(selected_world(&mut h).as_deref(), Some("Fresh"));
    assert!(h.is_visible(&L::text("played just now").within(card("Fresh"))));
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn closing_new_world_gives_the_focus_back_to_its_opener() {
    let mut h = world_list();
    h.click_when_ready(&new_world_button());
    h.wait_for(&L::role(Role::Dialog, NEW_WORLD_TITLE), SCREEN);
    h.press(KeyCode::Escape);
    assert!(!dialog_is_open(&mut h));
    assert_eq!(h.focus_label().as_deref(), Some("New world"));

    h.click(&card("Survie"));
    ctrl_n(&mut h);
    h.press(KeyCode::Escape);
    assert!(!dialog_is_open(&mut h));
    assert_eq!(h.focus_label().as_deref(), Some("Survie"));
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn the_key_hints_follow_the_focus() {
    let bar = || L::marker::<HintBar>();
    let mut h = world_list();
    h.wait_for(&L::text("Del").within(bar()), SCREEN);
    h.press(KeyCode::Tab);
    h.frames(1);
    assert!(!h.is_visible(&L::text("Del").within(bar())));
    assert!(h.is_visible(&L::text("Tab").within(bar())));
    ctrl_n(&mut h);
    h.frames(1);
    assert!(h.is_visible(&L::text("Create").within(bar())));
    drop(h);

    let mut h = open_world_list(GameFolder::new());
    h.wait_for(&L::text("Ctrl+N").within(bar()), SCREEN);
    assert!(!h.is_visible(&L::text("Del").within(bar())));
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn hovering_a_card_or_a_button_lightens_it() {
    let mut h = world_list();
    h.wait_for(&card("Survie"), SCREEN);
    let background = |h: &mut Harness, locator: &L| {
        let entity = h.get(locator);
        h.world().get::<BackgroundColor>(entity).unwrap().0
    };
    h.hover(&card("Survie"));
    h.frames(1);
    assert_eq!(background(&mut h, &card("Survie")), RAISE_HOVER);
    h.hover(&new_world_button());
    h.frames(1);
    assert_eq!(background(&mut h, &new_world_button()), RAISE_HOVER);
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn the_wheel_scrolls_only_the_list_under_the_cursor() {
    let mut folder = GameFolder::new();
    for i in 1..=15 {
        folder = folder.with_seeded_world(&format!("world {i:02}"), 42, 0, i * HOUR);
    }
    let mut h = open_world_list(folder);
    let first = card("world 01");
    h.wait_for(&first, SCREEN);
    let before = h.center_of(&first).y;
    // a notch of the wheel moves the list by one card
    let card_step = h.center_of(&card("world 02")).y - before;
    h.hover(&first);
    h.scroll(-1.);
    h.frames(2);
    assert_eq!(h.center_of(&first).y, before - card_step);
    h.scroll(-4.);
    h.frames(2);
    assert_eq!(h.center_of(&first).y, before - 5.0 * card_step);
    // a trackpad moves it by its pixels
    h.scroll_pixels(-30.);
    h.frames(2);
    let after = h.center_of(&first).y;
    assert_eq!(after, before - 5.0 * card_step - 30.0);

    h.hover(&L::marker::<WorldDetail>());
    h.scroll(-5.);
    h.frames(2);
    assert_eq!(h.center_of(&first).y, after, "the pane scrolled the list");
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn a_long_name_stays_inside_its_card_at_800x600() {
    let name = "W".repeat(32);
    let small = Options {
        width: 800,
        height: 600,
        ..Default::default()
    };
    let mut h = open_world_list_with(GameFolder::new().with_world(&name), small);
    h.wait_for(&card(&name), SCREEN);
    h.frames(2);
    let inside = |outer: bevy::math::Rect, inner: bevy::math::Rect| {
        outer.contains(inner.min) && outer.contains(inner.max)
    };
    let card_rect = h.rect_of(&card(&name));
    let card_name = h.rect_of(&L::text(&name).within(card(&name)));
    assert!(
        inside(card_rect, card_name),
        "{card_name:?} in {card_rect:?}"
    );
    let pane = h.rect_of(&L::marker::<WorldDetail>());
    let pane_name = h.rect_of(&L::text(&name).within(L::marker::<WorldDetail>()));
    assert!(inside(pane, pane_name), "{pane_name:?} in {pane:?}");
    // the name takes two lines of the pane: its buttons stay inside it
    for action in ["Play", "Rename", "Delete"] {
        let button = h.rect_of(&L::role(Role::Button, action).within(L::marker::<WorldDetail>()));
        assert!(inside(pane, button), "{action}: {button:?} in {pane:?}");
    }
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn a_ten_digit_seed_keeps_its_card_as_tall_as_the_others_at_800x600() {
    let small = Options {
        width: 800,
        height: 600,
        ..Default::default()
    };
    // enough worlds for the scrollbar, and the widest age
    let mut folder = GameFolder::new()
        .with_seeded_world("Short", 1, 0, HOUR)
        .with_seeded_world("Long", u32::MAX, 0, 10 * DAY);
    for i in 1..=4 {
        folder = folder.with_seeded_world(&format!("Older {i}"), 1, 0, (20 + i) * DAY);
    }
    let mut h = open_world_list_with(folder, small);
    h.wait_for(&card("Long"), SCREEN);
    h.frames(2);
    let short = h.rect_of(&card("Short")).height();
    let long = h.rect_of(&card("Long")).height();
    assert_eq!(
        long, short,
        "the card of the long seed is {long} px tall, not {short}"
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
    h.wait_for(&card("New world"), SCREEN);
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
    h.wait_for(&card("Survie"), SCREEN);
    ctrl_n(&mut h);
    h.type_text("Entered");
    h.press(KeyCode::Enter);
    h.frames(3);
    assert!(!dialog_is_open(&mut h));
    assert_eq!(
        card_names(&mut h),
        ["Entered", "Creative build", "Survie", "Test world 3"]
    );
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
fn a_world_created_by_the_keyboard_has_the_focus_and_its_ring() {
    let mut h = world_list();
    h.wait_for(&card("Survie"), SCREEN);
    ctrl_n(&mut h);
    h.press(KeyCode::Enter);
    h.wait_for(&card("New world"), SCREEN);
    h.frames(3);
    let world = h.world_mut();
    let focused = world.resource::<InputFocus>().get().unwrap();
    assert_eq!(
        world
            .get::<WorldCard>(focused)
            .map(|card| card.0.name.as_str()),
        Some("New world")
    );
    assert!(world.resource::<InputFocusVisible>().0);
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn a_typed_name_and_create_add_a_world() {
    let mut h = world_list();
    h.click_when_ready(&new_world_button());
    h.wait_for(&L::role(Role::Dialog, NEW_WORLD_TITLE), SCREEN);
    h.type_text("Nouveau");
    h.click(&create_button());
    h.wait_for(&card("Nouveau"), SCREEN);
    assert!(h.folder().join(SAVE_PATH).join("Nouveau").is_dir());
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn a_typed_seed_is_kept() {
    let mut h = world_list();
    h.wait_for(&card("Survie"), SCREEN);
    ctrl_n(&mut h);
    h.type_text("Seeded");
    h.press(KeyCode::Tab);
    h.type_text("12ab34");
    assert_eq!(h.value_of::<WorldSeedInput>(), "1234");
    h.press(KeyCode::Enter);
    h.wait_for(&card("Seeded"), SCREEN);
    assert_eq!(world_meta(&h, "Seeded").seed.0, 1234);
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn the_name_is_checked_as_you_type_and_nothing_moves() {
    let mut h = world_list();
    h.wait_for(&card("Survie"), SCREEN);
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
    assert_eq!(card_names(&mut h).len(), 3);

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
    h.wait_for(&card("Survie"), SCREEN);
    ctrl_n(&mut h);
    h.type_text("Taken");
    h.press(KeyCode::Enter);
    let failure = L::text("A folder with this name already exists");
    h.wait_for(&failure, SCREEN);
    assert!(dialog_is_open(&mut h));
    assert_eq!(card_names(&mut h), ["Survie"]);
    h.type_text("2");
    h.frames(2);
    assert!(!h.is_visible(&failure));
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn escape_cancels_the_dialog_and_stays_on_the_list() {
    let mut h = world_list();
    h.wait_for(&card("Survie"), SCREEN);
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
    h.wait_for(&card("One"), SCREEN);
    ctrl_n(&mut h);
    h.frames(2);
    assert_eq!(h.value_of::<WorldNameInput>(), "New world");
    h.type_text("Two");
    h.press(KeyCode::Enter);
    h.wait_for(&card("Two"), SCREEN);
    assert_eq!(h.find(&card("One")).len(), 1);
    assert_eq!(h.find(&card("Two")).len(), 1);
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn rename_renames_the_folder_and_keeps_the_place_of_the_world() {
    let mut h = world_list();
    let rename = L::role(Role::Button, "Rename");
    let dialog = || L::role(Role::Dialog, "Rename \"Survie\"");
    h.click_when_ready(&card("Survie"));
    h.click(&rename);
    h.wait_for(&dialog(), SCREEN);
    h.frames(2);
    assert_eq!(h.value_of::<WorldNameInput>(), "Survie");
    let confirm = L::role(Role::Button, "Rename").within(dialog());
    let disabled = |h: &mut Harness| {
        let button = h.get(&confirm);
        h.world().get::<InteractionDisabled>(button).is_some()
    };
    assert!(disabled(&mut h));
    assert!(!h.is_visible(&L::text(WORLD_EXISTS)));

    h.type_text("Creative build");
    h.frames(2);
    assert!(h.is_visible(&L::text(WORLD_EXISTS)));
    assert!(disabled(&mut h));
    for _ in 0..14 {
        h.press(KeyCode::Backspace);
    }
    h.type_text("Base");
    h.frames(2);
    assert!(!disabled(&mut h));
    h.press(KeyCode::Enter);
    h.frames(3);

    assert!(!h.is_visible(&dialog()));
    assert_eq!(world_meta(&h, "Base").name, "Base");
    assert!(!h.folder().join(SAVE_PATH).join("Survie").exists());
    assert_eq!(
        card_names(&mut h),
        ["Creative build", "Base", "Test world 3"]
    );
    assert_eq!(h.focus_label().as_deref(), Some("Base"));
    assert!(h.is_visible(&L::text("played 3 days ago").within(card("Base"))));

    h.click(&rename);
    h.wait_for(&L::role(Role::Dialog, "Rename \"Base\""), SCREEN);
    h.press(KeyCode::Escape);
    assert!(!h.is_visible(&L::role(Role::Dialog, "Rename \"Base\"")));
    let focused = h.world().resource::<InputFocus>().get();
    assert_eq!(focused, Some(h.get(&rename)));
    assert_eq!(selected_world(&mut h).as_deref(), Some("Base"));
    assert_eq!(
        card_names(&mut h),
        ["Creative build", "Base", "Test world 3"]
    );
    h.press(KeyCode::Escape);
    h.wait_for(&L::text("Singleplayer"), SCREEN);
    assert!(h.is_visible(&L::text("Creative build").within(L::marker::<ContinueCard>())));
}

/// Opens the Rename dialog of `world` from its card and the detail pane, its name ready to
/// replace.
#[track_caller]
fn open_rename(h: &mut Harness, world: &str) {
    h.click_when_ready(&card(world));
    h.click(&L::role(Role::Button, "Rename"));
    h.wait_for(
        &L::role(Role::Dialog, &format!("Rename \"{world}\"")),
        SCREEN,
    );
    h.frames(2);
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn rename_is_disabled_from_the_first_frame_of_its_dialog() {
    let mut h = world_list();
    h.click_when_ready(&card("Survie"));
    h.click(&L::role(Role::Button, "Rename"));
    let dialog = L::role(Role::Dialog, "Rename \"Survie\"");
    h.wait_for(&dialog, SCREEN);
    let confirm = h.get(&L::role(Role::Button, "Rename").within(dialog));
    assert!(h.world().get::<InteractionDisabled>(confirm).is_some());
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn a_rename_that_only_changes_the_case_renames_the_world() {
    let mut h = world_list();
    open_rename(&mut h, "Survie");
    h.type_text("survie");
    h.press(KeyCode::Enter);
    h.frames(3);
    assert!(!h.is_visible(&L::role(Role::Dialog, "Rename \"Survie\"")));
    assert_eq!(
        saved_folders(&h),
        ["Creative build", "Test world 3", "survie"]
    );
    assert_eq!(world_meta(&h, "survie").name, "survie");
    assert_eq!(
        card_names(&mut h),
        ["Creative build", "survie", "Test world 3"]
    );
    assert_eq!(h.focus_label().as_deref(), Some("survie"));
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn a_failed_rename_says_why_and_keeps_the_focus_in_its_dialog() {
    // A folder without a world.ron is not listed, yet its name is taken on disk
    let folder = three_worlds();
    fs::create_dir(folder.path().join(SAVE_PATH).join("Taken")).unwrap();
    let mut h = open_world_list(folder);
    open_rename(&mut h, "Survie");
    h.type_text("Taken");
    h.press(KeyCode::Enter);
    let failure = L::text("A folder with this name already exists");
    h.wait_for(&failure, SCREEN);
    assert!(h.is_visible(&L::role(Role::Dialog, "Rename \"Survie\"")));
    assert_eq!(h.focus_label().as_deref(), Some("Name"));
    assert_eq!(selected_world(&mut h).as_deref(), Some("Survie"));
    assert_eq!(
        card_names(&mut h),
        ["Creative build", "Survie", "Test world 3"]
    );
    // the field still takes what is typed
    h.type_text("2");
    h.frames(2);
    assert_eq!(h.value_of::<WorldNameInput>(), "Taken2");
    assert!(!h.is_visible(&failure));
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn back_and_escape_return_to_the_main_menu() {
    let mut h = world_list();
    h.click(&L::role(Role::Button, "Back"));
    h.wait_for(&L::text("Singleplayer"), SCREEN);
    assert_eq!(h.menu_state(), MenuState::Main);

    h.click(&L::text("Singleplayer"));
    h.wait_for(&card("Creative build"), SCREEN);
    assert_eq!(h.focus_label().as_deref(), Some("Creative build"));
    h.press(KeyCode::Escape);
    h.wait_for(&L::text("Singleplayer"), SCREEN);
    assert_eq!(h.menu_state(), MenuState::Main);
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn a_list_of_long_names_that_fits_shows_no_scrollbar() {
    let mut folder = GameFolder::new().with_seeded_world(&"W".repeat(32), u32::MAX, 0, DAY);
    for i in 1..=3 {
        folder = folder.with_seeded_world(&format!("World {i}"), 1, 0, (1 + i) * DAY);
    }
    let mut h = open_world_list(folder);
    h.wait_for(&card("World 3"), SCREEN);
    h.frames(3);
    assert!(!h.is_visible(&L::marker::<Scrollbar>()));
}
