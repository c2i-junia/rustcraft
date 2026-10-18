//! The widgets of the UI. Each widget file has a `pub(super) fn plugin(app: &mut App)`, which
//! `widgets_plugin` adds.

use bevy::prelude::*;

pub mod focus;
mod scale;

pub fn widgets_plugin(app: &mut App) {
    focus::plugin(app);
    scale::plugin(app);
}
