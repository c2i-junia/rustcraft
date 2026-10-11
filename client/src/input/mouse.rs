use crate::ui::hud::UIMode;
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};

pub fn handle_mouse_system(
    mut cursors: Query<&mut CursorOptions, With<PrimaryWindow>>,
    ui_mode: Res<UIMode>,
) {
    // The window can be missing while the app is closing
    let Ok(mut cursor) = cursors.single_mut() else {
        return;
    };

    let is_playing = *ui_mode == UIMode::Closed;

    cursor.grab_mode = if is_playing {
        CursorGrabMode::Locked
    } else {
        CursorGrabMode::None
    };

    cursor.visible = !is_playing;
}
