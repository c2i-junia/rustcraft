//! How the player looks around: the mouse sensitivity, the field of view and whether the game
//! fills the screen, saved in `view.ron` in the game folder.

use bevy::{
    prelude::*,
    window::{MonitorSelection, PrimaryWindow, WindowMode},
};
use ron::{from_str, ser::PrettyConfig};
use serde::{Deserialize, Serialize};
use shared::GameFolderPaths;
use std::fs;

use crate::constants::VIEW_SETTINGS_PATH;

/// The vertical field of view of the camera before the player sets one, in degrees.
pub const FOV_DEGREES: f32 = 60.0;
pub const MIN_FOV_DEGREES: f32 = 30.0;
pub const MAX_FOV_DEGREES: f32 = 110.0;

/// The least and the most mouse sensitivity, where 1 is the one before the player sets one.
pub const MIN_SENSITIVITY: f32 = 0.1;
pub const MAX_SENSITIVITY: f32 = 2.0;

/// How far the camera turns for a pixel of the mouse at a sensitivity of 1, in radians.
const RADIANS_PER_PIXEL: f32 = 0.003;

#[derive(Resource, Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(default)]
pub struct ViewSettings {
    /// Chosen on the Controls tab, like the rest on the Video tab.
    pub mouse_sensitivity: f32,
    /// The vertical field of view, in degrees: the sprint widens it, and the zoom narrows it.
    pub fov: f32,
    pub fullscreen: bool,
}

impl Default for ViewSettings {
    fn default() -> Self {
        Self {
            mouse_sensitivity: 1.0,
            fov: FOV_DEGREES,
            fullscreen: false,
        }
    }
}

/// `value` within the bounds, or `default` when it is not a number.
fn bounded(value: f32, min: f32, max: f32, default: f32) -> f32 {
    if value.is_nan() {
        default
    } else {
        value.clamp(min, max)
    }
}

impl ViewSettings {
    /// Sets the mouse sensitivity, kept within its bounds.
    pub fn set_mouse_sensitivity(&mut self, sensitivity: f32) {
        self.mouse_sensitivity = bounded(sensitivity, MIN_SENSITIVITY, MAX_SENSITIVITY, 1.0);
    }

    /// Sets the field of view in degrees, kept within its bounds.
    pub fn set_fov(&mut self, fov: f32) {
        self.fov = bounded(fov, MIN_FOV_DEGREES, MAX_FOV_DEGREES, FOV_DEGREES);
    }

    /// How far the camera turns for a pixel of the mouse, in radians.
    pub fn radians_per_pixel(&self) -> f32 {
        RADIANS_PER_PIXEL * self.mouse_sensitivity
    }

    /// The field of view, in radians.
    pub fn fov_radians(&self) -> f32 {
        self.fov.to_radians()
    }

    fn window_mode(&self) -> WindowMode {
        if self.fullscreen {
            WindowMode::BorderlessFullscreen(MonitorSelection::Current)
        } else {
            WindowMode::Windowed
        }
    }
}

/// The saved settings, brought back within their bounds, or the default ones when none were
/// saved or the file cannot be read.
pub fn load_view_settings(paths: &GameFolderPaths) -> ViewSettings {
    let path = paths.game_folder_path.join(VIEW_SETTINGS_PATH);
    let mut settings: ViewSettings = fs::read_to_string(&path)
        .ok()
        .and_then(|content| from_str(&content).ok())
        .unwrap_or_default();
    settings.set_mouse_sensitivity(settings.mouse_sensitivity);
    settings.set_fov(settings.fov);
    settings
}

/// Writes the settings to `view.ron` in the game folder.
pub fn write_view_settings(settings: &ViewSettings, paths: &GameFolderPaths) {
    let path = paths.game_folder_path.join(VIEW_SETTINGS_PATH);
    let serialized = match ron::ser::to_string_pretty(settings, PrettyConfig::new()) {
        Ok(serialized) => serialized,
        Err(e) => {
            error!("Failed to serialize the view settings: {e}");
            return;
        }
    };
    match fs::write(&path, serialized) {
        Ok(()) => info!("View settings saved to {path:?}"),
        Err(e) => error!("Failed to save the view settings to {path:?}: {e}"),
    }
}

/// Makes the window fill the screen, or not, as the settings say once that choice changes.
pub(super) fn apply_fullscreen_system(
    settings: Res<ViewSettings>,
    mut applied: Local<Option<bool>>,
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
) {
    if *applied == Some(settings.fullscreen) {
        return;
    }
    // The window the game opens is not fullscreen: nothing to leave
    if applied.is_none() && !settings.fullscreen {
        *applied = Some(false);
        return;
    }
    if let Ok(mut window) = windows.single_mut() {
        window.mode = settings.window_mode();
        *applied = Some(settings.fullscreen);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_paths(name: &str) -> GameFolderPaths {
        let dir = std::env::temp_dir().join(format!("rustcraft-{name}-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        GameFolderPaths {
            game_folder_path: dir.clone(),
            assets_folder_path: dir.join("assets"),
        }
    }

    #[test]
    fn the_view_settings_are_saved_and_loaded_back_within_their_bounds() {
        let paths = temp_paths("view");
        assert_eq!(load_view_settings(&paths), ViewSettings::default());
        let settings = ViewSettings {
            mouse_sensitivity: 0.55,
            fov: 90.0,
            fullscreen: true,
        };
        write_view_settings(&settings, &paths);
        assert_eq!(load_view_settings(&paths), settings);

        let default = ViewSettings::default();
        for (content, sensitivity, fov) in [
            ("garbage", 1.0, FOV_DEGREES),
            ("()", 1.0, FOV_DEGREES),
            ("(fov: 500.0)", 1.0, MAX_FOV_DEGREES),
            ("(fov: -5.0)", 1.0, MIN_FOV_DEGREES),
            ("(fov: NaN, mouse_sensitivity: NaN)", 1.0, FOV_DEGREES),
            ("(mouse_sensitivity: 0.0)", MIN_SENSITIVITY, FOV_DEGREES),
            ("(mouse_sensitivity: inf)", MAX_SENSITIVITY, FOV_DEGREES),
        ] {
            fs::write(paths.game_folder_path.join(VIEW_SETTINGS_PATH), content).unwrap();
            let expected = ViewSettings {
                mouse_sensitivity: sensitivity,
                fov,
                ..default
            };
            assert_eq!(load_view_settings(&paths), expected, "{content}");
        }
        fs::remove_dir_all(&paths.game_folder_path).unwrap();
    }

    #[test]
    fn the_window_fills_the_screen_as_the_settings_say() {
        let mut app = App::new();
        app.insert_resource(ViewSettings {
            fullscreen: true,
            ..default()
        })
        .add_systems(Update, apply_fullscreen_system);
        let window = app
            .world_mut()
            .spawn((Window::default(), PrimaryWindow))
            .id();
        let mode = |app: &App| app.world().get::<Window>(window).unwrap().mode;
        app.update();
        assert_eq!(
            mode(&app),
            WindowMode::BorderlessFullscreen(MonitorSelection::Current)
        );
        app.world_mut().resource_mut::<ViewSettings>().fullscreen = false;
        app.update();
        assert_eq!(mode(&app), WindowMode::Windowed);
    }
}
