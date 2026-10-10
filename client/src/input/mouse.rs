use crate::ui::hud::UIMode;
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, PrimaryWindow};

pub fn handle_mouse_system(
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
    ui_mode: Res<UIMode>,
) {
    // The window is already gone on the frame the app is closing
    let Ok(mut window) = windows.single_mut() else {
        return;
    };

    let is_playing = *ui_mode == UIMode::Closed;

    window.cursor_options.grab_mode = if is_playing {
        CursorGrabMode::Locked
    } else {
        CursorGrabMode::None
    };

    window.cursor_options.visible = !is_playing;
}
