//! The widgets of the UI. Each widget file has a `pub(super) fn plugin(app: &mut App)`, which
//! `widgets_plugin` adds.

use bevy::prelude::*;

use crate::ui::settings::InterfaceSettings;

pub mod button;
pub mod dialog;
pub mod focus;
pub mod hints;
mod scale;
pub mod slot;
pub mod text_field;
pub mod tooltip;

pub fn widgets_plugin(app: &mut App) {
    // `add_client` inserts the saved settings; this is for apps that do not
    app.init_resource::<InterfaceSettings>();
    button::plugin(app);
    dialog::plugin(app);
    focus::plugin(app);
    scale::plugin(app);
    slot::plugin(app);
    text_field::plugin(app);
    tooltip::plugin(app);
}
