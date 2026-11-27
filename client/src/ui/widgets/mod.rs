//! The widgets of the UI. Each widget file has a `pub(super) fn plugin(app: &mut App)`, which
//! `widgets_plugin` adds.

use bevy::prelude::*;

use crate::ui::settings::InterfaceSettings;
use crate::ui::theme;

pub mod button;
pub mod dialog;
pub mod focus;
pub mod hints;
pub mod list;
pub mod progress;
pub mod scale;
pub mod scroll;
pub mod segmented;
pub mod slider;
pub mod slot;
pub mod tabs;
pub mod text_field;
pub mod toast;
pub mod tooltip;

/// The bevel of a raised surface: light at the top and left, dark at the bottom and right.
pub(super) fn raised_bevel() -> BorderColor {
    BorderColor {
        top: theme::bevel_hi(),
        left: theme::bevel_hi(),
        bottom: theme::bevel_lo(),
        right: theme::bevel_lo(),
    }
}

/// The bevel of a recessed surface: dark at the top and left, light at the bottom and right.
pub(super) fn sunken_bevel() -> BorderColor {
    BorderColor {
        top: theme::bevel_lo(),
        left: theme::bevel_lo(),
        bottom: theme::bevel_hi(),
        right: theme::bevel_hi(),
    }
}

pub fn widgets_plugin(app: &mut App) {
    // `add_client` inserts the saved settings; this is for apps that do not
    app.init_resource::<InterfaceSettings>();
    button::plugin(app);
    crate::ui::fonts::plugin(app);
    dialog::plugin(app);
    focus::plugin(app);
    list::plugin(app);
    progress::plugin(app);
    scale::plugin(app);
    segmented::plugin(app);
    slider::plugin(app);
    slot::plugin(app);
    tabs::plugin(app);
    text_field::plugin(app);
    toast::plugin(app);
    tooltip::plugin(app);
}
