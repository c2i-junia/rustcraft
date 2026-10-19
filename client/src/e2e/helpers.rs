//! Steps shared by the tests of the screens.

use bevy::{
    a11y::AccessibilityNode,
    input_focus::InputFocus,
    prelude::{Component, Entity, Name, State, With, World},
    text::EditableText,
    ui::{ComputedNode, UiGlobalTransform},
};

use super::harness::{Harness, SCREEN, STARTUP};
use super::locator::Locator as L;
use crate::state::GameState;

/// A client on the main menu, with an empty game folder.
#[track_caller]
pub fn main_menu() -> Harness {
    let mut h = Harness::new();
    h.wait_for(&L::text("Singleplayer"), STARTUP);
    h
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

    /// Waits until the icons of the row `title` are laid out side by side: they are only borders
    /// until their images are loaded, and a click would then hit the wrong one. `title` is the
    /// name of a world or of a server.
    #[track_caller]
    pub fn wait_for_icons(&mut self, title: &str) {
        let play = self.wait_for_named(&format!("Play {title}"));
        let delete = self.wait_for_named(&format!("Delete {title}"));
        self.wait_until("the icons of the row", SCREEN, |w| {
            let x = |e| w.get::<UiGlobalTransform>(e).unwrap().translation.x;
            x(delete) - x(play) > 20.0
        });
    }

    /// Waits for the element with this `Name`, and returns it.
    #[track_caller]
    pub fn wait_for_named(&mut self, name: &str) -> Entity {
        self.wait_for(&L::name(name), SCREEN);
        self.get(&L::name(name))
    }

    /// Runs frames until the game is in the state `state`.
    #[track_caller]
    pub fn wait_for_game_state(&mut self, state: GameState, timeout: std::time::Duration) {
        self.wait_until(&format!("the game state {state:?}"), timeout, |w| {
            is_in(w, state)
        });
    }

    /// How many rows of the world list or of the server list there are: the "Play ..." buttons,
    /// shown or not.
    pub fn play_rows(&mut self) -> usize {
        self.world_mut()
            .query::<&Name>()
            .iter(self.world())
            .filter(|name| name.starts_with("Play "))
            .count()
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
