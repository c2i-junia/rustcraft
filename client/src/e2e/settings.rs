//! The settings, controls and audio screens.

use std::fs;

use bevy::input::keyboard::KeyCode;

use super::harness::{Harness, SCREEN};
use super::locator::Locator as L;
use crate::audio::settings::AudioSettings;
use crate::constants::{AUDIO_SETTINGS_PATH, BINDS_PATH};
use crate::input::data::GameAction;
use crate::state::KeyMap;
use crate::ui::button::MenuState;

/// A client on the settings.
fn settings() -> Harness {
    let mut h = Harness::new();
    h.click_when_ready("Settings");
    h.wait_for(&L::text("Controls"), SCREEN);
    h
}

/// A client on the controls screen.
fn controls() -> Harness {
    let mut h = settings();
    h.click(&L::text("Controls"));
    h.wait_for(&L::text("Keyboard Controls"), SCREEN);
    h
}

/// A client on the audio screen.
fn audio() -> Harness {
    let mut h = settings();
    h.click(&L::text("Audio"));
    h.wait_for(&L::text("Master"), SCREEN);
    h
}

impl Harness {
    /// Clicks "Back" and waits for the settings.
    fn back_to_settings(&mut self) {
        self.click(&L::text("Back"));
        self.wait_for(&L::text("Controls"), SCREEN);
    }

    /// Reads and parses a file of the game folder, which must exist.
    fn saved<T: serde::de::DeserializeOwned>(&self, file: &str) -> T {
        let text = fs::read_to_string(self.folder().join(file)).expect("a saved file");
        ron::from_str(&text).expect("a file in RON")
    }
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn the_settings_match_their_golden() {
    let mut h = settings();
    h.assert_screenshot("menu_settings");
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn controls_audio_and_back_open_their_screen() {
    let mut h = settings();
    h.click(&L::text("Controls"));
    h.wait_for(&L::text("Keyboard Controls"), SCREEN);
    assert_eq!(h.menu_state(), MenuState::SettingsControls);
    h.back_to_settings();
    assert_eq!(h.menu_state(), MenuState::Settings);

    h.click(&L::text("Audio"));
    h.wait_for(&L::text("Master"), SCREEN);
    assert_eq!(h.menu_state(), MenuState::SettingsAudio);
    h.back_to_settings();
    assert_eq!(h.menu_state(), MenuState::Settings);

    h.click(&L::text("Back"));
    h.wait_for(&L::text("Singleplayer"), SCREEN);
    assert_eq!(h.menu_state(), MenuState::Main);
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn escape_leaves_the_settings_and_the_audio() {
    let mut h = audio();
    h.press(KeyCode::Escape);
    h.wait_for(&L::text("Controls"), SCREEN);
    assert_eq!(h.menu_state(), MenuState::Settings);

    h.press(KeyCode::Escape);
    h.wait_for(&L::text("Singleplayer"), SCREEN);
    assert_eq!(h.menu_state(), MenuState::Main);
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn the_controls_match_their_golden() {
    let mut h = controls();
    h.assert_screenshot("menu_controls");
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn each_action_lists_its_keys() {
    let mut h = controls();
    h.wait_for(&L::text("W").within(L::row("MoveForward")), SCREEN);
    h.wait_for(&L::text("Space").within(L::row("Jump")), SCREEN);
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn a_key_pressed_after_clicking_an_action_is_bound_and_saved() {
    let mut h = controls();
    h.click(&L::text("Jump"));
    h.wait_for(&L::text("Press any key..."), SCREEN);
    h.press(KeyCode::KeyJ);
    h.wait_for(&L::text("J").within(L::row("Jump")), SCREEN);

    h.back_to_settings();
    let saved: KeyMap = h.saved(BINDS_PATH);
    assert!(saved.map[&GameAction::Jump].contains(&KeyCode::KeyJ));
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn the_clear_button_removes_the_keys_of_an_action() {
    let mut h = controls();
    h.wait_for(&L::text("Space").within(L::row("Jump")), SCREEN);
    h.hover(&L::row("Jump"));
    h.wait_for(&L::name("Clear Jump"), SCREEN);
    h.click(&L::name("Clear Jump"));
    // The click frame handles the button: two more frames are plenty for a negative check
    h.frames(2);
    assert!(!h.is_visible(&L::text("Space").within(L::row("Jump"))));
    assert!(h.world().resource::<KeyMap>().map[&GameAction::Jump].is_empty());
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn the_audio_screen_matches_its_golden() {
    let mut h = audio();
    h.assert_screenshot("menu_audio");
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn the_steppers_change_the_volume_by_ten_percent() {
    let mut h = audio();
    let master = L::text("100%").within(L::row("Master"));
    h.wait_for(&master, SCREEN);
    h.click(&L::name("Lower Master"));
    h.wait_for(&L::text("90%").within(L::row("Master")), SCREEN);

    // Raised twice: the second step is clamped at 100%
    h.click(&L::name("Raise Master"));
    h.wait_for(&master, SCREEN);
    h.click(&L::name("Raise Master"));
    h.frames(2);
    assert!(h.is_visible(&master));
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn the_volumes_are_saved_when_leaving() {
    let mut h = audio();
    for _ in 0..3 {
        h.click(&L::name("Lower Effects"));
    }
    h.wait_for(&L::text("70%").within(L::row("Effects")), SCREEN);
    h.back_to_settings();
    let saved: AudioSettings = h.saved(AUDIO_SETTINGS_PATH);
    assert!((saved.effects - 0.7).abs() < 0.001);
}
