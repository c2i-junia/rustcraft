//! Whether a dialog or a text input currently takes the player's inputs.

use bevy::{
    log::debug,
    prelude::{Component, Query, ResMut, Resource, Visibility, With},
};
use bevy_simple_text_input::TextInputInactive;

/// All UI dialogs toggling mouse visibility MUST use this in their bundle list\
/// They must also possess the `visibility` attribute\
/// Basically used to detect if multiple dialogs are open at once\
/// For example, mouse visibility : must stay visible as long as at least one dialog is active\
/// When the last active dialog is hidden, the mouse too\
#[derive(Component)]
pub struct UiDialog;

#[derive(PartialEq, Eq, Clone, Copy, Resource)]
pub enum UIMode {
    Opened,
    Closed,
    /// When the user is typing something in an input. All other inputs shall be ignored
    Typing,
}

pub fn set_ui_mode(
    mut ui_mode: ResMut<UIMode>,
    visibility: Query<&Visibility, With<UiDialog>>,
    inactive: Query<&TextInputInactive>,
) {
    for inac in inactive.iter() {
        if !inac.0 {
            debug!("Typing...");
            *ui_mode = UIMode::Typing;
            return;
        }
    }
    for vis in visibility.iter() {
        if vis == Visibility::Visible {
            *ui_mode = UIMode::Opened;
            return;
        }
    }
    *ui_mode = UIMode::Closed;
}
