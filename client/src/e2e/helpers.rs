//! Steps shared by the tests of the screens.

use accesskit::Role;
use bevy::{
    a11y::AccessibilityNode,
    input::keyboard::KeyCode,
    input_focus::InputFocus,
    math::Rect,
    prelude::{Component, State, With, World},
    text::EditableText,
    ui::{ComputedNode, UiGlobalTransform},
};

use super::harness::{Harness, Options, SCREEN, STARTUP};
use super::locator::Locator as L;
use super::seed::GameFolder;
use crate::state::GameState;
use crate::ui::menus::solo::dialogs::NEW_WORLD_TITLE;

/// A client on the main menu, with an empty game folder.
#[track_caller]
pub fn main_menu() -> Harness {
    let mut h = Harness::new();
    h.wait_for(&L::text("Singleplayer"), STARTUP);
    h
}

/// A client on the main menu, with the game folder `folder`.
#[track_caller]
pub fn main_menu_with(folder: GameFolder) -> Harness {
    let mut h = Harness::start(folder, Options::default());
    h.wait_for(&L::text("Singleplayer"), STARTUP);
    h
}

/// Presses `key` with Control held.
pub fn ctrl(h: &mut Harness, key: KeyCode) {
    h.key_down(KeyCode::ControlLeft);
    h.press(key);
    h.key_up(KeyCode::ControlLeft);
}

/// Presses Ctrl+N on the world list, and waits for the New world dialog.
#[track_caller]
pub fn ctrl_n(h: &mut Harness) {
    ctrl(h, KeyCode::KeyN);
    h.wait_for(&L::role(Role::Dialog, NEW_WORLD_TITLE), SCREEN);
}

/// Whether the game is in the state `state`.
pub fn is_in(world: &World, state: GameState) -> bool {
    world
        .get_resource::<State<GameState>>()
        .is_some_and(|current| *current.get() == state)
}

impl Harness {
    /// Waits for the locator to be visible, then clicks it: a screen takes a few frames to appear.
    #[track_caller]
    pub fn click_when_ready(&mut self, locator: &L) {
        self.wait_for(locator, SCREEN);
        self.click(locator);
    }

    /// Runs frames until the game is in the state `state`.
    #[track_caller]
    pub fn wait_for_game_state(&mut self, state: GameState, timeout: std::time::Duration) {
        self.wait_until(&format!("the game state {state:?}"), timeout, |w| {
            is_in(w, state)
        });
    }

    /// What the text field marked with `M` holds.
    #[track_caller]
    pub fn value_of<M: Component>(&mut self) -> String {
        self.world_mut()
            .query_filtered::<&EditableText, With<M>>()
            .single(self.world())
            .unwrap()
            .value()
            .to_string()
    }

    /// The rectangle of the element, in physical pixels.
    #[track_caller]
    pub fn rect_of(&mut self, locator: &L) -> Rect {
        let entity = self.get(locator);
        let world = self.world();
        let center = world.get::<UiGlobalTransform>(entity).unwrap().translation;
        let size = world.get::<ComputedNode>(entity).unwrap().size();
        Rect::from_center_size(center, size)
    }

    /// The accessible label of the focused entity.
    pub fn focus_label(&mut self) -> Option<String> {
        let focused = self.world().resource::<InputFocus>().get()?;
        let node = self.world().get::<AccessibilityNode>(focused)?;
        node.label().map(str::to_string)
    }

    /// How many pixels of a fresh screenshot inside the physical rectangle of the element are
    /// bright (a luminance above 128): the glyphs of a text on a dark ground.
    #[track_caller]
    pub fn bright_pixels_in(&mut self, locator: &L) -> usize {
        let entity = self.get(locator);
        let world = self.world();
        let center = world.get::<UiGlobalTransform>(entity).unwrap().translation;
        let half = world.get::<ComputedNode>(entity).unwrap().size() / 2.0;
        let shot = self.screenshot();
        let (min, max) = (center - half, center + half);
        shot.enumerate_pixels()
            .filter(|(x, y, _)| {
                (min.x..max.x).contains(&(*x as f32)) && (min.y..max.y).contains(&(*y as f32))
            })
            .filter(|(_, _, pixel)| {
                let [r, g, b] = pixel.0.map(f32::from);
                0.2126 * r + 0.7152 * g + 0.0722 * b > 128.0
            })
            .count()
    }
}
