//! The widgets of the UI. Each widget file has a `pub(super) fn plugin(app: &mut App)`, which
//! `widgets_plugin` adds.

use bevy::prelude::*;

pub mod button;
pub mod focus;
mod scale;
pub mod text_field;

pub fn widgets_plugin(app: &mut App) {
    button::plugin(app);
    focus::plugin(app);
    scale::plugin(app);
    text_field::plugin(app);
}
