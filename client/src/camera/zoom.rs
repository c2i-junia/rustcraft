//! The zoom, as OptiFine's: while its key is held, the field of view narrows to a quarter and
//! the mouse turns the camera as much slower, so that aiming stays as steady on screen.

use bevy::prelude::*;

use crate::input::{data::GameAction, keyboard::is_action_pressed};
use crate::state::KeyMap;
use crate::ui::hud::UIMode;

/// Share of the field of view, and of the mouse sensitivity, kept while zooming.
pub const ZOOM_SCALE: f32 = 0.25;

/// Whether the local player zooms.
#[derive(Resource, Default)]
pub struct Zoom {
    pub held: bool,
}

impl Zoom {
    /// What the field of view and the mouse sensitivity are multiplied by.
    pub fn scale(&self) -> f32 {
        if self.held {
            ZOOM_SCALE
        } else {
            1.0
        }
    }
}

/// Zooms while the zoom key is held, outside of the menus and the chat.
pub fn zoom_system(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    key_map: Res<KeyMap>,
    ui_mode: Res<UIMode>,
    mut zoom: ResMut<Zoom>,
) {
    let held = *ui_mode == UIMode::Closed
        && is_action_pressed(GameAction::Zoom, &keyboard_input, &key_map);
    if zoom.held != held {
        zoom.held = held;
    }
}
