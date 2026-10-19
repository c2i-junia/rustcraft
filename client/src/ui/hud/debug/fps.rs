use bevy::diagnostic::DiagnosticsStore;
use bevy::diagnostic::FrameTimeDiagnosticsPlugin;
use bevy::prelude::*;

use crate::ui::theme::{DANGER, OK, TEXT};

/// Marker to find the text entity so we can update it
#[derive(Component)]
pub struct FpsText;

/// The colour of the frame rate `fps`: `DANGER` at 30 and below, `OK` at 120 and above, and a
/// blend of the two between.
pub(crate) fn fps_color(fps: f64) -> Color {
    let ok = ((fps - 30.0) / (120.0 - 30.0)).clamp(0.0, 1.0) as f32;
    DANGER.mix(&OK, ok)
}

pub fn fps_text_update_system(
    diagnostics: Res<DiagnosticsStore>,
    mut query: Query<(&mut TextSpan, &mut TextColor), With<FpsText>>,
) {
    for (mut span, mut color) in query.iter_mut() {
        // Try to get a "smoothed" FPS value from Bevy
        if let Some(value) = diagnostics
            .get(&FrameTimeDiagnosticsPlugin::FPS)
            .and_then(|fps| fps.smoothed())
        {
            // Format the number as to leave space for 4 digits, just in case,
            // right-aligned and rounded. This helps readability when the
            // number changes rapidly.
            **span = format!("{value:>4.0}");

            color.0 = fps_color(value);
        } else {
            // Display "N/A" if we can't get a FPS measurement
            **span = " N/A".into();

            color.0 = TEXT;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::fps_color;
    use crate::ui::theme::{DANGER, OK};

    #[test]
    fn the_fps_go_from_the_danger_colour_to_the_ok_one() {
        assert_eq!(fps_color(120.0), OK);
        assert_eq!(fps_color(200.0), OK);
        assert_eq!(fps_color(30.0), DANGER);
        assert_eq!(fps_color(10.0), DANGER);
        assert_ne!(fps_color(60.0), OK);
        assert_ne!(fps_color(60.0), DANGER);
    }
}
