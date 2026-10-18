//! The UI scale: it follows the height of the window, so that the UI keeps its size on screen.

use bevy::{prelude::*, window::PrimaryWindow};

/// The scale of the UI for a window of this logical height: 1 up to 1079 px, then one more half
/// every 360 px (1.5 at 1080, 2 at 1440, 3 at 2160).
pub fn auto_ui_scale(window_height: f32) -> f32 {
    ((window_height / 720. * 2.).floor() / 2.).max(1.)
}

pub(super) fn plugin(app: &mut App) {
    app.add_systems(Update, update_ui_scale);
}

/// Sets the `UiScale` from the logical height of the window, because `UiScale` multiplies the
/// scale factor of the window itself.
fn update_ui_scale(window: Single<&Window, With<PrimaryWindow>>, mut ui_scale: ResMut<UiScale>) {
    // `UiScale` has no `PartialEq`, so the value is compared, not the resource
    let scale = auto_ui_scale(window.height());
    if ui_scale.0 != scale {
        ui_scale.0 = scale;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_scale_follows_the_window_height() {
        for (height, scale) in [
            (400., 1.),
            (600., 1.),
            (720., 1.),
            (900., 1.),
            (1079., 1.),
            (1080., 1.5),
            (1200., 1.5),
            (1440., 2.),
            (2160., 3.),
        ] {
            assert_eq!(auto_ui_scale(height), scale, "{height}");
        }
    }

    #[test]
    fn auto_scale_keeps_the_logical_height_at_least_600() {
        for height in 600..=2400 {
            let height = height as f32;
            assert!(height / auto_ui_scale(height) >= 600., "{height}");
        }
    }
}
