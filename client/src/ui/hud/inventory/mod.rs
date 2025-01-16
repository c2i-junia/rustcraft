use bevy::{
    log::debug,
    prelude::{Component, Query, ResMut, Resource, Visibility, With},
};

/// All UI dialogs toggling mouse visibility MUST use this in their bundle list\
/// They must also possess the `visibility` attribute\
/// Basically used to detect if multiple dialogs are open at once\
/// For example, mouse visibility : must stay visible as long as at least one dialog is active\
/// When the last active dialog is hidden, the mouse too\
#[derive(Component)]
pub struct UiDialog;

// Marker for Inventory root
#[derive(Component)]
pub struct InventoryRoot;

/// Main inventory dialog
#[derive(Component)]
pub struct InventoryDialog;

#[derive(Component)]
pub struct InventoryCell {
    pub id: u32,
}

/// The current selected stack, not considered in the player's inventory
#[derive(Component)]
pub struct FloatingStack {
    pub items: Option<ItemStack>,
}

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

mod display;
pub mod items;
mod setup;

use bevy_simple_text_input::TextInputInactive;
pub use display::*;
use items::*;
pub use setup::*;
use shared::world::ItemStack;
