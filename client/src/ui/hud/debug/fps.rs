use bevy::diagnostic::DiagnosticsStore;
use bevy::diagnostic::FrameTimeDiagnosticsPlugin;
use bevy::prelude::*;

/// Marker to find the text entity so we can update it
#[derive(Component)]
pub struct FpsText;

pub fn fps_text_update_system(
    diagnostics: Res<DiagnosticsStore>,
    query: Query<Entity, With<FpsText>>,
    mut writer: TextUiWriter,
    mut text_colors: Query<&mut TextColor>,
) {
    for entity in query.iter() {
        // Try to get a "smoothed" FPS value from Bevy
        if let Some(value) = diagnostics
            .get(&FrameTimeDiagnosticsPlugin::FPS)
            .and_then(|fps| fps.smoothed())
        {
            // Format the number as to leave space for 4 digits, just in case,
            // right-aligned and rounded. This helps readability when the
            // number changes rapidly.
            *writer.text(entity, 1) = format!("{value:>4.0}");

            // Adjust text color based on FPS value
            if let Ok(mut color) = text_colors.get_mut(entity) {
                color.0 = if value >= 120.0 {
                    // Above 120 FPS, use green color
                    Color::srgb(0.0, 1.0, 0.0)
                } else if value >= 60.0 {
                    // Between 60-120 FPS, gradually transition from yellow to green
                    Color::srgb((1.0 - (value - 60.0) / (120.0 - 60.0)) as f32, 1.0, 0.0)
                } else if value >= 30.0 {
                    // Between 30-60 FPS, gradually transition from red to yellow
                    Color::srgb(1.0, ((value - 30.0) / (60.0 - 30.0)) as f32, 0.0)
                } else {
                    // Below 30 FPS, use red color
                    Color::srgb(1.0, 0.0, 0.0)
                };
            }
        } else {
            // Display "N/A" if we can't get a FPS measurement
            *writer.text(entity, 1) = " N/A".into();

            // Reset text color to white if no FPS is available
            if let Ok(mut color) = text_colors.get_mut(entity) {
                color.0 = Color::WHITE;
            }
        }
    }
}
