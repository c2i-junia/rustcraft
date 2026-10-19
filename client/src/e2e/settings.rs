//! The settings screen: its tabs, the controls, the volumes and the UI scale.

use std::fs;

use accesskit::{NodeId, Role};
use bevy::a11y::AccessibilityNode;
use bevy::input::keyboard::{KeyCode, KeyboardFocusLost};
use bevy::prelude::*;
use bevy::ui::UiGlobalTransform;
use bevy::ui_widgets::{ScrollArea, Scrollbar, ScrollbarThumb};

use super::harness::{Harness, Options, SCREEN};
use super::locator::Locator as L;
use super::seed::GameFolder;
use crate::audio::settings::AudioSettings;
use crate::constants::{AUDIO_SETTINGS_PATH, BINDS_PATH, INTERFACE_SETTINGS_PATH};
use crate::input::data::GameAction;
use crate::state::KeyMap;
use crate::ui::button::MenuState;
use crate::ui::menus::settings::controls::PRESS_A_KEY;
use crate::ui::settings::{InterfaceSettings, UiScaleChoice};
use crate::ui::theme::{BEVEL_HI, FOCUS_RING_OFFSET, FOCUS_RING_WIDTH, MUTED, RAISE_HOVER};

/// A client on the settings, with the game folder `folder`.
#[track_caller]
fn settings_with(folder: GameFolder, options: Options) -> Harness {
    let mut h = Harness::start(folder, options);
    h.click_when_ready(&L::text("Settings"));
    h.wait_for(&L::role(Role::Tab, "Controls"), SCREEN);
    h
}

/// A client on the settings.
#[track_caller]
fn settings() -> Harness {
    settings_with(GameFolder::new(), Options::default())
}

/// A client on the controls tab, the tab the settings open on.
#[track_caller]
fn controls() -> Harness {
    let mut h = settings();
    h.wait_for(&L::text("Move forward"), SCREEN);
    h
}

impl Harness {
    /// Clicks the tab `label` of the settings.
    #[track_caller]
    fn open_tab(&mut self, label: &str) {
        self.click(&L::role(Role::Tab, label));
    }

    /// Clicks the tab `label` of the settings and waits for the content `shown`.
    #[track_caller]
    fn open_tab_showing(&mut self, label: &str, shown: &L) {
        self.open_tab(label);
        self.wait_for(shown, SCREEN);
    }

    /// Reads and parses a file of the game folder, which must exist.
    fn saved<T: serde::de::DeserializeOwned>(&self, file: &str) -> T {
        let text = fs::read_to_string(self.folder().join(file)).expect("a saved file");
        ron::from_str(&text).expect("a file in RON")
    }

    /// Clicks "Back" and waits for the main menu.
    #[track_caller]
    fn back_to_the_menu(&mut self) {
        self.click(&L::role(Role::Button, "Back"));
        self.wait_for(&L::text("Singleplayer"), SCREEN);
        assert_eq!(self.menu_state(), MenuState::Main);
    }

    /// The `UiScale` of the client.
    fn ui_scale(&self) -> f32 {
        self.world().resource::<UiScale>().0
    }

    /// Moves the cursor to the top left corner, over nothing, for a golden.
    fn hover_nothing(&mut self) {
        self.hover_at(Vec2::ZERO);
    }
}

/// The Master slider of the audio tab.
fn master() -> L {
    L::role(Role::Slider, "Master")
}

/// The UI scale control of the interface tab.
fn ui_scale_control() -> L {
    L::role(Role::RadioGroup, "UI scale")
}

/// A client on the audio tab.
#[track_caller]
fn audio() -> Harness {
    let mut h = settings();
    h.open_tab_showing("Audio", &master());
    h
}

/// A client on the interface tab, with the game folder `folder`.
#[track_caller]
fn interface_with(folder: GameFolder, options: Options) -> Harness {
    let mut h = settings_with(folder, options);
    h.open_tab_showing("Interface", &ui_scale_control());
    h
}

/// A client on the interface tab.
#[track_caller]
fn interface() -> Harness {
    interface_with(GameFolder::new(), Options::default())
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn the_settings_match_their_golden() {
    let mut h = interface();
    h.hover_nothing();
    h.assert_screenshot("menu_settings");
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn the_tabs_show_their_content_and_back_returns_to_the_menu() {
    let mut h = controls();
    assert_eq!(h.menu_state(), MenuState::Settings);
    assert!(h.is_visible(&L::role(Role::TabPanel, "Controls")));

    h.open_tab_showing("Audio", &master());
    assert_eq!(h.menu_state(), MenuState::Settings);
    assert!(!h.is_visible(&L::text("Move forward")));
    // the panel is labelled by its tab
    let panel = h.get(&L::role(Role::TabPanel, "Audio"));
    let tab = h.get(&L::role(Role::Tab, "Audio"));
    let labelled_by = h
        .world()
        .get::<AccessibilityNode>(panel)
        .unwrap()
        .labelled_by();
    assert_eq!(labelled_by, [NodeId(tab.to_bits())]);

    h.open_tab_showing("Interface", &ui_scale_control());
    assert!(!h.is_visible(&master()));
    assert!(h.is_visible(&L::role(Role::TabPanel, "Interface")));

    h.back_to_the_menu();
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn the_keyboard_alone_walks_the_tabs() {
    let mut h = controls();
    assert_eq!(h.focus_label().as_deref(), Some("Settings"));
    h.press(KeyCode::ArrowRight);
    h.wait_for(&master(), SCREEN);
    h.press(KeyCode::Tab);
    assert_eq!(h.focus_label().as_deref(), Some("Master"));
    h.press(KeyCode::ArrowLeft);
    h.wait_for(&L::text("95%").within(L::row("Master")), SCREEN);
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn escape_leaves_the_settings_from_any_tab() {
    let mut h = audio();
    h.press(KeyCode::Escape);
    h.wait_for(&L::text("Singleplayer"), SCREEN);
    assert_eq!(h.menu_state(), MenuState::Main);
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn a_click_the_arrows_and_the_wheel_change_a_volume_saved_at_once() {
    let mut h = audio();
    let percent = |value: &str| L::text(value).within(L::row("Master"));
    h.click(&master());
    h.wait_for(&percent("50%"), SCREEN);
    h.press(KeyCode::ArrowRight);
    h.wait_for(&percent("55%"), SCREEN);
    h.scroll(1.);
    h.wait_for(&percent("60%"), SCREEN);
    let saved: AudioSettings = h.saved(AUDIO_SETTINGS_PATH);
    assert!((saved.master - 0.6).abs() < 1e-4, "{}", saved.master);
    for _ in 0..10 {
        h.press(KeyCode::ArrowRight);
    }
    h.wait_for(&percent("100%"), SCREEN);
    let saved: AudioSettings = h.saved(AUDIO_SETTINGS_PATH);
    assert_eq!(saved.master, 1.0);
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn a_volume_is_saved_at_once() {
    let mut h = audio();
    h.click(&L::role(Role::Slider, "Effects"));
    let file = h.folder().join(AUDIO_SETTINGS_PATH);
    h.wait_until("Effects saved at 50%", SCREEN, |_| {
        fs::read_to_string(&file)
            .ok()
            .and_then(|text| ron::from_str::<AudioSettings>(&text).ok())
            .is_some_and(|saved| saved.effects == 0.5)
    });
    assert_eq!(h.menu_state(), MenuState::Settings);
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn the_audio_screen_matches_its_golden() {
    let mut h = audio();
    h.hover_nothing();
    h.assert_screenshot("menu_audio");
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn the_ui_scale_is_chosen_clamped_and_saved() {
    let full_hd = || Options {
        width: 1920,
        height: 1080,
        ..default()
    };
    let folder = GameFolder::new();
    let mut h = interface_with(folder.clone(), full_hd());
    let note = |scale: &str| L::text(&format!("Limited to {scale} by the window height"));

    // 3 does not fit 1080 px: the UI is at 1.5, and says so
    h.click(&L::role(Role::RadioButton, "3"));
    h.wait_for(&note("1.5"), SCREEN);
    assert_eq!(h.ui_scale(), 1.5);
    let saved: InterfaceSettings = h.saved(INTERFACE_SETTINGS_PATH);
    assert_eq!(saved.ui_scale, UiScaleChoice::X3);

    // a smaller window lowers it again
    h.resize(800, 600);
    h.wait_for(&note("1"), SCREEN);
    assert_eq!(h.ui_scale(), 1.0);

    h.resize(1920, 1080);
    h.wait_for(&note("1.5"), SCREEN);
    h.click(&L::role(Role::RadioButton, "1"));
    h.wait_until("the UI at scale 1", SCREEN, |w| {
        w.resource::<UiScale>().0 == 1.0
    });
    h.frames(2);
    assert!(!h.is_visible(&L::text_containing("Limited to")));
    drop(h);

    // the choice is kept for the next start
    let mut h = Harness::start(folder, full_hd());
    h.wait_for(&L::text("Singleplayer"), SCREEN);
    assert_eq!(h.ui_scale(), 1.0);
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn every_control_of_the_settings_lightens_on_hover() {
    let mut h = settings();
    let background = |h: &mut Harness, locator: &L| {
        let entity = h.get(locator);
        h.world().get::<BackgroundColor>(entity).unwrap().0
    };
    let audio = L::role(Role::Tab, "Audio");
    h.hover(&audio);
    h.frames(2);
    assert_eq!(background(&mut h, &audio), RAISE_HOVER);
    let back = L::role(Role::Button, "Back");
    h.hover(&back);
    h.frames(2);
    assert_eq!(background(&mut h, &back), RAISE_HOVER);
    let slot = L::role(Role::Button, "Jump, primary key");
    h.hover(&slot);
    h.frames(2);
    assert_eq!(background(&mut h, &slot), RAISE_HOVER);
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn starting_and_opening_the_settings_rewrite_no_settings_file() {
    // a file of three keys per action, which loading trims to two
    let keybindings = "(map: {Jump: [KeyJ, KeyK, KeyL]})";
    let audio = "(master: 0.7)";
    let interface = "(ui_scale: X1)";
    let folder = GameFolder::new()
        .with_file(BINDS_PATH, keybindings)
        .with_file(AUDIO_SETTINGS_PATH, audio)
        .with_file(INTERFACE_SETTINGS_PATH, interface);
    let mut h = settings_with(folder, Options::default());
    h.open_tab_showing("Audio", &master());
    h.open_tab_showing("Interface", &ui_scale_control());
    h.frames(2);
    let read = |file: &str| fs::read_to_string(h.folder().join(file)).unwrap();
    assert_eq!(read(BINDS_PATH), keybindings);
    assert_eq!(read(AUDIO_SETTINGS_PATH), audio);
    assert_eq!(read(INTERFACE_SETTINGS_PATH), interface);
    // nor does leaving them
    h.back_to_the_menu();
    h.frames(2);
    assert_eq!(
        fs::read_to_string(h.folder().join(BINDS_PATH)).unwrap(),
        keybindings
    );
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn the_controls_match_their_golden() {
    let mut h = controls();
    h.hover_nothing();
    h.assert_screenshot("menu_controls");
}

/// The key slot `slot` ("primary" or "secondary") of the action `action`.
fn key_slot(action: &str, slot: &str) -> L {
    L::role(Role::Button, &format!("{action}, {slot} key"))
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn each_action_lists_its_keys() {
    let mut h = controls();
    h.wait_for(&L::text("W").within(L::row("Move forward")), SCREEN);
    h.wait_for(&L::text("Space").within(L::row("Jump")), SCREEN);
    for category in ["Movement", "Interface", "View", "Debug"] {
        assert!(h.is_visible(&L::text(category)), "{category}");
    }
    assert!(!h.is_visible(&L::text("MoveForward")));
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn the_arrow_keys_show_their_glyph() {
    let mut h = controls();
    let arrow = L::text("↑").within(L::row("Move forward"));
    h.wait_for(&arrow, SCREEN);
    h.wait_for_ui_images();
    assert!(h.bright_pixels_in(&arrow) > 20);
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn a_key_pressed_after_clicking_a_slot_is_bound_and_saved_at_once() {
    let mut h = controls();
    h.click(&key_slot("Jump", "secondary"));
    h.wait_for(&L::text(PRESS_A_KEY), SCREEN);
    h.press(KeyCode::KeyJ);
    h.wait_for(&L::text("J").within(L::row("Jump")), SCREEN);
    assert!(!h.is_visible(&L::text(PRESS_A_KEY)));

    let saved: KeyMap = h.saved(BINDS_PATH);
    assert_eq!(
        saved.map[&GameAction::Jump],
        [KeyCode::Space, KeyCode::KeyJ]
    );
    assert_eq!(h.menu_state(), MenuState::Settings);
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn the_keyboard_alone_rebinds_cancels_and_clears() {
    let mut h = controls();
    let backward = || L::row("Move backward");
    h.press(KeyCode::Tab);
    assert_eq!(
        h.focus_label().as_deref(),
        Some("Move forward, primary key")
    );
    h.press(KeyCode::ArrowDown);
    assert_eq!(
        h.focus_label().as_deref(),
        Some("Move backward, primary key")
    );

    assert!(h.is_visible(&L::text("Change key")));

    // Escape cancels the capture, and only the capture
    h.press(KeyCode::Enter);
    h.wait_for(&L::text(PRESS_A_KEY), SCREEN);
    h.wait_for(&L::text("Any key"), SCREEN);
    assert!(h.is_visible(&L::text("Cancel")));
    assert!(!h.is_visible(&L::text("Change key")));
    h.press(KeyCode::Escape);
    assert_eq!(h.menu_state(), MenuState::Settings);
    h.frames(2);
    assert_eq!(h.menu_state(), MenuState::Settings);
    assert!(!h.is_visible(&L::text(PRESS_A_KEY)));
    assert!(h.is_visible(&L::text("Change key")));
    assert!(h.is_visible(&L::text("S").within(backward())));

    h.press(KeyCode::Enter);
    h.wait_for(&L::text(PRESS_A_KEY), SCREEN);
    h.press(KeyCode::KeyX);
    h.wait_for(&L::text("X").within(backward()), SCREEN);

    h.press(KeyCode::Delete);
    h.wait_until("Move backward cleared", SCREEN, |w| {
        w.resource::<KeyMap>().map[&GameAction::MoveBackward] == [KeyCode::ArrowDown]
    });
    let saved: KeyMap = h.saved(BINDS_PATH);
    assert_eq!(saved.map[&GameAction::MoveBackward], [KeyCode::ArrowDown]);

    h.press(KeyCode::Escape);
    h.wait_for(&L::text("Singleplayer"), SCREEN);
    assert_eq!(h.menu_state(), MenuState::Main);
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn a_capture_ends_when_the_window_loses_focus_and_a_modifier_binds_on_its_release() {
    let mut h = controls();
    let primary = key_slot("Jump", "primary");
    // Alt+Tab to another window: the Alt release never reaches the slot as a binding
    h.click(&primary);
    h.wait_for(&L::text(PRESS_A_KEY), SCREEN);
    h.key_down(KeyCode::AltLeft);
    assert!(h.is_visible(&L::text(PRESS_A_KEY)));
    h.world_mut().write_message(KeyboardFocusLost);
    h.frames(1);
    h.key_up(KeyCode::AltLeft);
    h.frames(2);
    assert!(!h.is_visible(&L::text(PRESS_A_KEY)));
    assert!(h.is_visible(&L::text("Space").within(L::row("Jump"))));
    assert_eq!(
        h.world().resource::<KeyMap>().map[&GameAction::Jump],
        [KeyCode::Space]
    );
    assert!(!h.folder().join(BINDS_PATH).exists());

    // a modifier alone binds when it is released
    h.click(&key_slot("Jump", "secondary"));
    h.wait_for(&L::text(PRESS_A_KEY), SCREEN);
    h.key_down(KeyCode::ShiftLeft);
    assert!(h.is_visible(&L::text(PRESS_A_KEY)));
    h.key_up(KeyCode::ShiftLeft);
    h.wait_for(&L::text("Left Shift").within(L::row("Jump")), SCREEN);
    let saved: KeyMap = h.saved(BINDS_PATH);
    assert_eq!(
        saved.map[&GameAction::Jump],
        [KeyCode::Space, KeyCode::ShiftLeft]
    );
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn a_right_click_clears_a_slot_and_a_click_elsewhere_cancels_a_capture() {
    let mut h = controls();
    let primary = key_slot("Jump", "primary");
    h.right_click(&primary);
    h.wait_until("Jump cleared", SCREEN, |w| {
        w.resource::<KeyMap>().map[&GameAction::Jump].is_empty()
    });
    let saved: KeyMap = h.saved(BINDS_PATH);
    assert!(saved.map[&GameAction::Jump].is_empty());

    h.click(&primary);
    h.wait_for(&L::text(PRESS_A_KEY), SCREEN);
    h.click(&L::text("Jump"));
    // The press handles the click: two more frames are plenty for a negative check
    h.frames(2);
    assert!(!h.is_visible(&L::text(PRESS_A_KEY)));
    assert!(h.world().resource::<KeyMap>().map[&GameAction::Jump].is_empty());
}

/// The rectangle of the element, in physical pixels.
#[track_caller]
fn rect_of(h: &mut Harness, locator: &L) -> Rect {
    let entity = h.get(locator);
    rect_of_entity(h, entity)
}

fn rect_of_entity(h: &Harness, entity: Entity) -> Rect {
    let center = h
        .world()
        .get::<UiGlobalTransform>(entity)
        .unwrap()
        .translation;
    let size = h.world().get::<ComputedNode>(entity).unwrap().size();
    Rect::from_center_size(center, size)
}

/// The fill of the thumb of the controls' scrollbar.
fn thumb_fill(h: &mut Harness) -> Color {
    h.world_mut()
        .query_filtered::<&BackgroundColor, With<ScrollbarThumb>>()
        .single(h.world())
        .unwrap()
        .0
}

/// The rectangle of the thumb of the controls' scrollbar, which the scrollbar lays out itself.
fn thumb_rect(h: &mut Harness) -> Rect {
    let thumb = h
        .world_mut()
        .query_filtered::<Entity, With<ScrollbarThumb>>()
        .single(h.world())
        .unwrap();
    rect_of_entity(h, thumb)
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn the_focused_slot_and_its_ring_stay_in_view() {
    let small = Options {
        width: 800,
        height: 600,
        ..default()
    };
    let mut h = settings_with(GameFolder::new(), small);
    h.wait_for(&L::text("Move forward"), SCREEN);
    h.press(KeyCode::Tab);
    for _ in 0..8 {
        h.press(KeyCode::ArrowDown);
    }
    h.frames(2);
    assert_eq!(h.focus_label().as_deref(), Some("Inventory, primary key"));
    let scale = h.world().resource::<UiScale>().0;
    let ring = (FOCUS_RING_WIDTH + FOCUS_RING_OFFSET) * scale;
    let slot = rect_of(&mut h, &key_slot("Inventory", "primary")).inflate(ring);
    let list = rect_of(&mut h, &L::marker::<ScrollArea>());
    let inside = |r: Rect| list.contains(r.min + 0.5) && list.contains(r.max - 0.5);
    assert!(inside(slot), "{slot:?} in {list:?}");
    // the label of its category too
    let label = rect_of(
        &mut h,
        &L::text("Interface").within(L::marker::<ScrollArea>()),
    );
    assert!(inside(label), "{label:?} in {list:?}");
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn the_wheel_scrolls_the_controls_list_under_the_cursor() {
    let mut h = controls();
    let first = L::text("Move forward");
    let before = h.center_of(&first);
    // the scrollbar shows how much of the list is in view
    let (thumb, track) = (
        thumb_rect(&mut h),
        rect_of(&mut h, &L::marker::<Scrollbar>()),
    );
    assert!(thumb.height() < track.height(), "{thumb:?} in {track:?}");
    h.hover(&first);
    h.scroll(-3.);
    h.frames(2);
    let scrolled = h.center_of(&first);
    assert!(scrolled.y < before.y, "{before} then {scrolled}");
    let moved = thumb_rect(&mut h);
    assert!(moved.min.y > thumb.min.y, "{thumb:?} then {moved:?}");
    // the thumb stands out from its track, and lightens under the cursor
    assert_eq!(thumb_fill(&mut h), BEVEL_HI);
    h.hover_at(moved.center());
    h.frames(2);
    assert_eq!(thumb_fill(&mut h), MUTED);

    // over the tabs, the wheel moves nothing
    h.hover(&L::role(Role::Tab, "Audio"));
    h.scroll(-3.);
    h.frames(2);
    assert_eq!(h.center_of(&first), scrolled);
}
