//! The UI scale: it follows the height of the window, so that the UI keeps its size on screen.

use bevy::{prelude::*, window::PrimaryWindow};

use crate::ui::settings::{InterfaceSettings, UiScaleChoice};

/// The scales the UI can have.
pub const UI_SCALE_STEPS: [f32; 4] = [1.0, 1.5, 2.0, 3.0];

/// The smallest logical height of the window a forced scale leaves to the UI.
pub const MIN_LOGICAL_HEIGHT: f32 = 600.0;

/// The scale of the UI for a window of this logical height: 1 up to 1079 px, then one more half
/// every 360 px (1.5 at 1080, 2 at 1440, 3 at 2160).
pub fn auto_ui_scale(window_height: f32) -> f32 {
    ((window_height / 720. * 2.).floor() / 2.).max(1.)
}

/// The scale of the UI for the player's `choice` and a window of this logical height. A forced
/// scale is lowered to the largest step that keeps the logical height at `MIN_LOGICAL_HEIGHT`,
/// and never goes under 1.
pub fn ui_scale_for(choice: UiScaleChoice, window_height: f32) -> f32 {
    let Some(factor) = choice.factor() else {
        return auto_ui_scale(window_height);
    };
    UI_SCALE_STEPS
        .into_iter()
        .rev()
        .find(|&step| step <= factor && window_height / step >= MIN_LOGICAL_HEIGHT)
        .unwrap_or(1.)
}

pub(super) fn plugin(app: &mut App) {
    app.add_systems(Update, update_ui_scale);
}

/// Sets the `UiScale` from the logical height of the window and the player's choice, because
/// `UiScale` multiplies the scale factor of the window itself.
fn update_ui_scale(
    window: Single<&Window, With<PrimaryWindow>>,
    settings: Res<InterfaceSettings>,
    mut ui_scale: ResMut<UiScale>,
) {
    // `UiScale` has no `PartialEq`, so the value is compared, not the resource
    let scale = ui_scale_for(settings.ui_scale, window.height());
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

    #[test]
    fn a_forced_scale_keeps_the_logical_height_at_least_600() {
        use UiScaleChoice::*;
        for (choice, height, scale) in [
            (X3, 2160., 3.),
            (X3, 1440., 2.),
            (X3, 1080., 1.5),
            (X2, 1080., 1.5),
            (X2, 1200., 2.),
            (X1_5, 900., 1.5),
            (X1_5, 720., 1.),
            (X1, 2160., 1.),
            (X3, 500., 1.),
            (Auto, 1080., 1.5),
        ] {
            assert_eq!(
                ui_scale_for(choice, height),
                scale,
                "{choice:?} at {height}"
            );
        }
        for choice in UiScaleChoice::ALL {
            for height in 600..=2400 {
                let height = height as f32;
                let scale = ui_scale_for(choice, height);
                // `Auto` is not held to the steps: it gives 2.5 from 1800 to 2159
                if choice != UiScaleChoice::Auto {
                    assert!(UI_SCALE_STEPS.contains(&scale), "{choice:?} at {height}");
                }
                assert!(
                    height / scale >= MIN_LOGICAL_HEIGHT,
                    "{choice:?} at {height}"
                );
            }
        }
    }
}
