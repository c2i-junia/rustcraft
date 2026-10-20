//! The settings screen: its tabs, the controls, the volumes, the UI scale and the FPS counter.

use std::fs;

use accesskit::{NodeId, Role};
use bevy::a11y::AccessibilityNode;
use bevy::input::keyboard::{KeyCode, KeyboardFocusLost};
use bevy::prelude::*;
use bevy::ui::UiGlobalTransform;
use bevy::ui_widgets::{ScrollArea, Scrollbar, ScrollbarThumb};

use super::game::enter_world;
use super::harness::{Harness, Options, SCREEN, STARTUP};
use super::locator::Locator as L;
use super::seed::GameFolder;
use crate::audio::settings::AudioSettings;
use crate::constants::{
    AUDIO_SETTINGS_PATH, BINDS_PATH, DEFAULT_CHUNK_RENDER_DISTANCE_RADIUS, INTERFACE_SETTINGS_PATH,
    MAX_CHUNK_RENDER_DISTANCE_RADIUS, VIDEO_SETTINGS_PATH,
};
use crate::input::data::GameAction;
use crate::input::keyboard::default_bindings;
use crate::state::KeyMap;
use crate::ui::hud::fps_counter::{FpsCounter, FpsGraph, FrameBar, GraphStat};
use crate::ui::menus::pause::PAUSE_TITLE;
use crate::ui::menus::settings::controls::{PRESS_A_KEY, SEARCH_LABEL};
use crate::ui::menus::MenuState;
use crate::ui::settings::{FpsCounterChoice, InterfaceSettings, UiScaleChoice};
use crate::ui::theme::{
    BEVEL_HI, BEVEL_LO, DANGER, FOCUS_RING_OFFSET, FOCUS_RING_WIDTH, MUTED, RAISE_HOVER,
};
use crate::ui::widgets::hints::HintBar;
use crate::world::{RenderDistance, ServerRenderDistanceLimit};

/// A client on the settings, with the game folder `folder`.
#[track_caller]
fn settings_with(folder: GameFolder, options: Options) -> Harness {
    let mut h = Harness::start(folder, options);
    h.wait_for(&L::text("Settings"), STARTUP);
    h.click(&L::text("Settings"));
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

/// The render distance slider of the video tab.
fn render_distance() -> L {
    L::role(Role::Slider, "Render distance")
}

/// The value shown after the render distance slider.
fn render_distance_readout(chunks: u32) -> L {
    L::text(&chunks.to_string()).within(L::row("Render distance"))
}

/// What the note under the render distance says when the server grants it.
const RENDER_DISTANCE_NOTE: &str = "Chunks loaded in every direction around you";

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

    // each tab has its own key hints: Enter does nothing on the Audio and Interface tabs
    let hint = |text: &str| L::text(text).within(L::marker::<HintBar>());
    assert!(h.is_visible(&hint("Change key")));
    assert!(h.is_visible(&hint("Enter")));

    h.open_tab_showing("Audio", &master());
    assert_eq!(h.menu_state(), MenuState::Settings);
    assert!(!h.is_visible(&L::text("Move forward")));
    for (keys, action) in [("Tab", "Move"), ("←→", "Change"), ("Esc", "Back")] {
        assert!(h.is_visible(&hint(keys)), "{keys}");
        assert!(h.is_visible(&hint(action)), "{action}");
    }
    assert!(!h.is_visible(&hint("Enter")));
    assert!(!h.is_visible(&hint("Select")));
    assert!(!h.is_visible(&hint("Change key")));
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
    assert!(h.is_visible(&hint("Tab")));
    assert!(!h.is_visible(&hint("Enter")));
    assert!(!h.is_visible(&hint("Select")));

    h.back_to_the_menu();
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn the_keyboard_alone_walks_the_tabs() {
    let mut h = controls();
    assert_eq!(h.focus_label().as_deref(), Some("Settings"));
    h.press(KeyCode::ArrowRight);
    h.wait_for(&render_distance(), SCREEN);
    h.press(KeyCode::Tab);
    assert_eq!(h.focus_label().as_deref(), Some("Render distance"));
    h.press(KeyCode::ArrowLeft);
    h.wait_for(
        &render_distance_readout(DEFAULT_CHUNK_RENDER_DISTANCE_RADIUS - 1),
        SCREEN,
    );
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
fn the_render_distance_moves_by_whole_chunks_within_its_bounds_and_is_kept() {
    let folder = GameFolder::new();
    let mut h = settings_with(folder.clone(), Options::default());
    h.open_tab_showing("Video", &render_distance());
    h.wait_for(
        &render_distance_readout(DEFAULT_CHUNK_RENDER_DISTANCE_RADIUS),
        SCREEN,
    );
    assert!(h.is_visible(&L::text(RENDER_DISTANCE_NOTE)));

    h.click(&render_distance());
    for _ in 0..MAX_CHUNK_RENDER_DISTANCE_RADIUS {
        h.press(KeyCode::ArrowLeft);
    }
    h.wait_for(&render_distance_readout(1), SCREEN);
    h.press(KeyCode::ArrowRight);
    h.wait_for(&render_distance_readout(2), SCREEN);
    h.scroll(1.);
    h.wait_for(&render_distance_readout(3), SCREEN);
    assert_eq!(h.world().resource::<RenderDistance>().distance, 3);
    for _ in 0..MAX_CHUNK_RENDER_DISTANCE_RADIUS {
        h.press(KeyCode::ArrowRight);
    }
    h.wait_for(
        &render_distance_readout(MAX_CHUNK_RENDER_DISTANCE_RADIUS),
        SCREEN,
    );
    let saved: RenderDistance = h.saved(VIDEO_SETTINGS_PATH);
    assert_eq!(saved.distance, MAX_CHUNK_RENDER_DISTANCE_RADIUS);
    // from the main menu, no server limits it
    assert!(h.is_visible(&L::text(RENDER_DISTANCE_NOTE)));
    drop(h);

    // the choice is kept for the next start
    let mut h = Harness::start(folder, Options::default());
    h.wait_for(&L::text("Singleplayer"), STARTUP);
    assert_eq!(
        h.world().resource::<RenderDistance>().distance,
        MAX_CHUNK_RENDER_DISTANCE_RADIUS
    );
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn in_game_the_render_distance_says_when_the_server_grants_less() {
    let mut h = super::game::enter_world(GameFolder::new().with_world("Far"), "Far");
    // as a server that grants 3 would have answered
    h.world_mut().insert_resource(ServerRenderDistanceLimit(3));
    h.press(KeyCode::Escape);
    h.wait_for(&L::role(Role::Dialog, PAUSE_TITLE), SCREEN);
    h.click(&L::role(Role::Button, "Settings"));
    h.wait_for(&L::role(Role::Tab, "Controls"), SCREEN);
    h.open_tab_showing("Video", &render_distance());

    h.click(&render_distance());
    for _ in 0..MAX_CHUNK_RENDER_DISTANCE_RADIUS {
        h.press(KeyCode::ArrowLeft);
    }
    for _ in 0..3 {
        h.press(KeyCode::ArrowRight);
    }
    h.wait_for(&render_distance_readout(4), SCREEN);
    h.wait_for(&L::text("Limited to 3 by this server"), SCREEN);

    h.press(KeyCode::ArrowLeft);
    h.wait_for(&L::text(RENDER_DISTANCE_NOTE), SCREEN);
    assert_eq!(h.world().resource::<RenderDistance>().distance, 3);
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn the_video_screen_matches_its_golden() {
    let mut h = settings();
    h.open_tab_showing("Video", &render_distance());
    h.hover_nothing();
    h.assert_screenshot("menu_video");
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
    h.wait_for(&L::text("Singleplayer"), STARTUP);
    assert_eq!(h.ui_scale(), 1.0);
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn the_fps_graph_is_chosen_in_the_menu_and_changed_in_game() {
    let folder = GameFolder::new().with_world("Fps");
    let mut h = interface_with(folder.clone(), Options::default());
    let counter = L::marker::<FpsCounter>();
    let graph = L::marker::<FpsGraph>();

    h.click(&L::role(Role::RadioButton, "Graph"));
    let saved: InterfaceSettings = h.saved(INTERFACE_SETTINGS_PATH);
    assert_eq!(saved.fps_counter, FpsCounterChoice::Graph);
    drop(h);

    // the choice is kept for the next start, and the graph fills with the frames of the game
    let mut h = enter_world(folder, "Fps");
    h.wait_for(&graph, SCREEN);
    h.wait_until("a bar and the stats of the graph", SCREEN, |w| {
        let mut bars = w.try_query::<(&FrameBar, &Node)>().unwrap();
        let mut stats = w.try_query::<(&GraphStat, &Text)>().unwrap();
        bars.iter(w).any(|(_, node)| node.height != px(0))
            && stats.iter(w).all(|(_, text)| text.0 != "N/A")
    });

    // changed from the pause menu, it applies at once
    h.press(KeyCode::Escape);
    h.click(&L::role(Role::Button, "Settings"));
    h.wait_for(&L::role(Role::Tab, "Controls"), SCREEN);
    h.open_tab_showing("Interface", &L::role(Role::RadioGroup, "FPS counter"));
    h.click(&L::role(Role::RadioButton, "Number"));
    h.frames(2);
    assert!(h.is_visible(&counter));
    assert!(!h.is_visible(&graph));
    h.click(&L::role(Role::RadioButton, "Off").within(L::role(Role::RadioGroup, "FPS counter")));
    h.frames(2);
    assert!(!h.is_visible(&counter));
    let saved: InterfaceSettings = h.saved(INTERFACE_SETTINGS_PATH);
    assert_eq!(saved.fps_counter, FpsCounterChoice::Off);
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn the_fps_counter_key_goes_round_its_choices_and_saves_them() {
    let folder = GameFolder::new().with_world("FpsKey");
    let mut h = enter_world(folder.clone(), "FpsKey");
    let counter = L::marker::<FpsCounter>();
    let graph = L::marker::<FpsGraph>();
    let saved = |h: &Harness| {
        h.saved::<InterfaceSettings>(INTERFACE_SETTINGS_PATH)
            .fps_counter
    };
    assert!(!h.is_visible(&counter));

    h.press(KeyCode::F8);
    h.frames(2);
    assert!(h.is_visible(&counter));
    assert!(!h.is_visible(&graph));
    assert_eq!(saved(&h), FpsCounterChoice::Number);

    h.press(KeyCode::F8);
    h.frames(2);
    assert!(h.is_visible(&graph));
    assert_eq!(saved(&h), FpsCounterChoice::Graph);

    h.press(KeyCode::F8);
    h.frames(2);
    assert!(!h.is_visible(&counter));
    assert_eq!(saved(&h), FpsCounterChoice::Off);
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
pub(super) fn key_slot(action: &str, slot: &str) -> L {
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
    // past the search field
    h.press(KeyCode::Tab);
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
fn a_key_slot_sinks_while_pressed_and_space_starts_its_capture_like_a_button() {
    let mut h = controls();
    let edges = |h: &mut Harness, slot: &L| {
        let entity = h.get(slot);
        let border = h.world().get::<BorderColor>(entity).unwrap();
        [border.top, border.left, border.bottom, border.right]
    };
    let secondary = key_slot("Move forward", "secondary");
    let raised = [BEVEL_HI, BEVEL_HI, BEVEL_LO, BEVEL_LO];
    assert_eq!(edges(&mut h, &secondary), raised);
    // the bevel inverts while the button is down, and the release starts the capture
    h.mouse_down(&secondary);
    h.frames(1);
    assert_eq!(
        edges(&mut h, &secondary),
        [BEVEL_LO, BEVEL_LO, BEVEL_HI, BEVEL_HI]
    );
    h.mouse_up();
    h.wait_for(&L::text(PRESS_A_KEY), SCREEN);
    h.press(KeyCode::Escape);
    h.frames(2);
    assert!(!h.is_visible(&L::text(PRESS_A_KEY)));
    assert_eq!(edges(&mut h, &secondary), raised);

    // Space starts the capture of the focused slot, and during it is bound like any key
    assert_eq!(
        h.focus_label().as_deref(),
        Some("Move forward, secondary key")
    );
    h.press(KeyCode::Space);
    h.wait_for(&L::text(PRESS_A_KEY), SCREEN);
    h.press(KeyCode::Space);
    h.wait_until("Space bound to Move forward", SCREEN, |w| {
        w.resource::<KeyMap>().map[&GameAction::MoveForward] == [KeyCode::KeyW, KeyCode::Space]
    });
    h.frames(2);
    assert!(!h.is_visible(&L::text(PRESS_A_KEY)));
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
fn a_capture_binds_the_key_pressed_with_a_modifier_and_never_a_modifier_it_did_not_see_pressed() {
    let mut h = controls();
    let jump = |w: &World| w.resource::<KeyMap>().map[&GameAction::Jump].clone();

    // Shift held then W: W is bound, not Shift
    h.click(&key_slot("Jump", "primary"));
    h.wait_for(&L::text(PRESS_A_KEY), SCREEN);
    h.key_down(KeyCode::ShiftLeft);
    h.press(KeyCode::KeyW);
    h.key_up(KeyCode::ShiftLeft);
    h.wait_for(&L::text("W").within(L::row("Jump")), SCREEN);
    h.frames(2);
    assert_eq!(jump(h.world()), [KeyCode::KeyW]);

    // a modifier then Escape: the capture is cancelled, and binds nothing
    h.click(&key_slot("Jump", "secondary"));
    h.wait_for(&L::text(PRESS_A_KEY), SCREEN);
    h.key_down(KeyCode::ControlLeft);
    h.press(KeyCode::Escape);
    h.key_up(KeyCode::ControlLeft);
    h.frames(2);
    assert!(!h.is_visible(&L::text(PRESS_A_KEY)));
    assert_eq!(h.menu_state(), MenuState::Settings);
    assert_eq!(jump(h.world()), [KeyCode::KeyW]);

    // a modifier held before the capture and released during it binds nothing
    h.key_down(KeyCode::AltLeft);
    h.click(&key_slot("Jump", "secondary"));
    h.wait_for(&L::text(PRESS_A_KEY), SCREEN);
    h.key_up(KeyCode::AltLeft);
    h.frames(2);
    assert!(h.is_visible(&L::text(PRESS_A_KEY)));
    assert_eq!(jump(h.world()), [KeyCode::KeyW]);
    let saved: KeyMap = h.saved(BINDS_PATH);
    assert_eq!(saved.map[&GameAction::Jump], [KeyCode::KeyW]);
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

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn a_key_bound_twice_is_shown_on_both_rows_and_reset_restores_the_defaults() {
    let mut h = controls();
    let any_conflict = L::text_containing("is also bound");
    h.frames(2);
    assert!(!h.is_visible(&any_conflict));

    h.click(&key_slot("Jump", "secondary"));
    h.wait_for(&L::text(PRESS_A_KEY), SCREEN);
    h.press(KeyCode::KeyW);
    h.wait_for(
        &L::text("W is also bound to Move forward").within(L::row("Jump")),
        SCREEN,
    );
    h.wait_for(
        &L::text("W is also bound to Jump").within(L::row("Move forward")),
        SCREEN,
    );
    // shown, never refused, and the slots holding the key are outlined in red
    assert_eq!(
        h.world().resource::<KeyMap>().map[&GameAction::Jump],
        [KeyCode::Space, KeyCode::KeyW]
    );
    let border = |h: &mut Harness, slot: &L| {
        let entity = h.get(slot);
        h.world().get::<BorderColor>(entity).unwrap().top
    };
    h.frames(2);
    assert_eq!(border(&mut h, &key_slot("Jump", "secondary")), DANGER);
    assert_eq!(border(&mut h, &key_slot("Move forward", "primary")), DANGER);
    assert_ne!(border(&mut h, &key_slot("Jump", "primary")), DANGER);
    // and shown again when the tab is; the reset button is only on the Controls tab
    let reset = L::role(Role::Button, "Reset to defaults");
    h.open_tab_showing("Audio", &master());
    assert!(!h.is_visible(&reset));
    h.open_tab_showing(
        "Controls",
        &L::text("W is also bound to Move forward").within(L::row("Jump")),
    );
    h.frames(2);
    assert_eq!(border(&mut h, &key_slot("Jump", "secondary")), DANGER);
    // beside Back, on the right
    let (reset_at, back_at) = (
        h.center_of(&reset),
        h.center_of(&L::role(Role::Button, "Back")),
    );
    assert_eq!(reset_at.y, back_at.y);
    assert!(reset_at.x > back_at.x);

    // the reset asks first, Cancel focused: Escape and Cancel change nothing
    let file = h.folder().join(BINDS_PATH);
    let before = fs::read_to_string(&file).unwrap();
    let dialog = || L::role(Role::Dialog, "Reset key bindings?");
    h.click(&reset);
    h.wait_for(&dialog(), SCREEN);
    assert!(h.is_visible(&L::text("Every action gets its default keys back.")));
    assert_eq!(h.focus_label().as_deref(), Some("Cancel"));
    h.press(KeyCode::Escape);
    h.frames(2);
    assert!(!h.is_visible(&dialog()));
    assert_eq!(h.menu_state(), MenuState::Settings);
    h.click(&reset);
    h.wait_for(&dialog(), SCREEN);
    h.click(&L::role(Role::Button, "Cancel").within(dialog()));
    h.frames(2);
    assert!(!h.is_visible(&dialog()));
    assert_eq!(h.focus_label().as_deref(), Some("Reset to defaults"));
    assert_eq!(fs::read_to_string(&file).unwrap(), before);
    assert_eq!(
        h.world().resource::<KeyMap>().map[&GameAction::Jump],
        [KeyCode::Space, KeyCode::KeyW]
    );

    h.click(&reset);
    h.wait_for(&dialog(), SCREEN);
    h.click(&L::role(Role::Button, "Reset").within(dialog()));
    h.wait_until("the default bindings", SCREEN, |w| {
        w.resource::<KeyMap>().map == default_bindings().map
    });
    h.frames(2);
    assert!(!h.is_visible(&dialog()));
    assert_eq!(h.focus_label().as_deref(), Some("Reset to defaults"));
    assert!(!h.is_visible(&any_conflict));
    assert_ne!(border(&mut h, &key_slot("Jump", "secondary")), DANGER);
    let saved: KeyMap = h.saved(BINDS_PATH);
    assert_eq!(saved.map, default_bindings().map);
}

impl Harness {
    /// Presses Ctrl+F.
    fn ctrl_f(&mut self) {
        self.key_down(KeyCode::ControlLeft);
        self.press(KeyCode::KeyF);
        self.key_up(KeyCode::ControlLeft);
    }
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn ctrl_f_searches_the_actions_by_name_or_key_and_escape_clears_then_leaves() {
    let mut h = controls();
    let hint = |text: &str| L::text(text).within(L::marker::<HintBar>());
    assert!(h.is_visible(&hint("Search")));
    h.ctrl_f();
    assert_eq!(h.focus_label().as_deref(), Some(SEARCH_LABEL));
    h.wait_for(&hint("Results"), SCREEN);

    // a key's name finds the actions it is bound to; the other rows and categories hide
    h.type_text("space");
    h.wait_for(&L::text("2 of 23"), SCREEN);
    assert!(h.is_visible(&L::text("Jump")));
    assert!(h.is_visible(&L::text("Fly up")));
    assert!(!h.is_visible(&L::text("Move forward")));
    assert!(!h.is_visible(&L::text("Debug")));
    assert!(h.is_visible(&hint("Clear")));
    h.snapshot("search_space");

    // Down goes to the first row found, which stays when its key changes
    h.press(KeyCode::ArrowDown);
    assert_eq!(h.focus_label().as_deref(), Some("Jump, primary key"));
    h.press(KeyCode::Enter);
    h.wait_for(&L::text(PRESS_A_KEY), SCREEN);
    h.press(KeyCode::KeyJ);
    h.wait_for(&L::text("J").within(L::row("Jump")), SCREEN);
    h.frames(2);
    assert!(h.is_visible(&L::text("Jump")));

    // Ctrl+F selects the search: what is typed replaces it
    h.ctrl_f();
    assert_eq!(h.focus_label().as_deref(), Some(SEARCH_LABEL));
    h.type_text("xyz");
    h.wait_for(
        &L::text("No action or key matches \u{201c}xyz\u{201d}"),
        SCREEN,
    );
    assert!(!h.is_visible(&L::text("Jump")));

    // every word, in any case
    h.ctrl_f();
    h.type_text("FLY tog");
    h.wait_for(&L::text("1 of 23"), SCREEN);
    assert!(h.is_visible(&L::text("Toggle flying")));

    // Escape clears the search, then leaves the settings
    h.press(KeyCode::Escape);
    h.wait_for(&L::text("Move forward"), SCREEN);
    assert!(!h.is_visible(&L::text("1 of 23")));
    assert_eq!(h.menu_state(), MenuState::Settings);
    assert_eq!(h.focus_label().as_deref(), Some(SEARCH_LABEL));
    h.wait_for(&hint("Back"), SCREEN);
    h.press(KeyCode::Escape);
    h.wait_for(&L::text("Singleplayer"), SCREEN);
    assert_eq!(h.menu_state(), MenuState::Main);
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn ctrl_f_comes_back_to_the_controls_and_the_search_outlives_the_tab() {
    let mut h = controls();
    h.ctrl_f();
    h.type_text("debug");
    h.wait_for(&L::text("5 of 23"), SCREEN);
    h.open_tab_showing("Audio", &master());
    h.ctrl_f();
    h.wait_for(&L::text("5 of 23"), SCREEN);
    assert_eq!(h.focus_label().as_deref(), Some(SEARCH_LABEL));
    assert!(h.is_visible(&L::text("Chunk borders")));
    assert!(!h.is_visible(&L::text("Jump")));
    // Up from the first row goes back to the field
    h.press(KeyCode::ArrowDown);
    assert_eq!(
        h.focus_label().as_deref(),
        Some("Debug overlay, primary key")
    );
    h.press(KeyCode::ArrowUp);
    assert_eq!(h.focus_label().as_deref(), Some(SEARCH_LABEL));
    // a slot waiting for a key takes Ctrl+F for itself
    h.press(KeyCode::ArrowDown);
    h.press(KeyCode::Enter);
    h.wait_for(&L::text(PRESS_A_KEY), SCREEN);
    h.ctrl_f();
    h.wait_for(&L::text("F").within(L::row("Debug overlay")), SCREEN);
    assert_eq!(
        h.focus_label().as_deref(),
        Some("Debug overlay, primary key")
    );
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
    // past the search field
    h.press(KeyCode::Tab);
    h.press(KeyCode::Tab);
    for _ in 0..9 {
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
    // a notch of the wheel moves the list by one row
    let row_step = h.center_of(&L::text("Move backward")).y - before.y;
    h.hover(&first);
    h.scroll(-1.);
    h.frames(2);
    assert_eq!(h.center_of(&first).y, before.y - row_step);
    h.scroll(-2.);
    h.frames(2);
    let scrolled = h.center_of(&first);
    assert_eq!(scrolled.y, before.y - 3.0 * row_step);
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
