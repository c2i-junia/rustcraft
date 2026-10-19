//! The interface settings the player chose, saved in `interface.ron` in the game folder.

use bevy::prelude::*;
use ron::{from_str, ser::PrettyConfig};
use serde::{Deserialize, Serialize};
use shared::GameFolderPaths;
use std::fs;

use crate::constants::INTERFACE_SETTINGS_PATH;

/// The UI scale the player asked for.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum UiScaleChoice {
    /// Follows the height of the window.
    #[default]
    Auto,
    X1,
    X1_5,
    X2,
    X3,
}

impl UiScaleChoice {
    pub const ALL: [UiScaleChoice; 5] = [
        UiScaleChoice::Auto,
        UiScaleChoice::X1,
        UiScaleChoice::X1_5,
        UiScaleChoice::X2,
        UiScaleChoice::X3,
    ];

    pub fn label(self) -> &'static str {
        match self {
            UiScaleChoice::Auto => "Auto",
            UiScaleChoice::X1 => "1",
            UiScaleChoice::X1_5 => "1.5",
            UiScaleChoice::X2 => "2",
            UiScaleChoice::X3 => "3",
        }
    }

    /// The scale asked for, or `None` when the window height decides.
    pub fn factor(self) -> Option<f32> {
        match self {
            UiScaleChoice::Auto => None,
            UiScaleChoice::X1 => Some(1.0),
            UiScaleChoice::X1_5 => Some(1.5),
            UiScaleChoice::X2 => Some(2.0),
            UiScaleChoice::X3 => Some(3.0),
        }
    }
}

/// What the FPS counter in the top-right corner of the game shows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum FpsCounterChoice {
    #[default]
    Off,
    /// The frame rate alone.
    Number,
    /// The frame rate, over a graph of the frame times of the last seconds.
    Graph,
}

impl FpsCounterChoice {
    pub const ALL: [FpsCounterChoice; 3] = [
        FpsCounterChoice::Off,
        FpsCounterChoice::Number,
        FpsCounterChoice::Graph,
    ];

    pub fn label(self) -> &'static str {
        match self {
            FpsCounterChoice::Off => "Off",
            FpsCounterChoice::Number => "Number",
            FpsCounterChoice::Graph => "Graph",
        }
    }

    /// The choice after this one, back to `Off` after the last.
    pub fn next(self) -> FpsCounterChoice {
        let index = FpsCounterChoice::ALL.iter().position(|&c| c == self);
        FpsCounterChoice::ALL[index.map_or(0, |i| (i + 1) % FpsCounterChoice::ALL.len())]
    }
}

/// Whether the minimap shows in the top-right corner of the game.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum MinimapChoice {
    Off,
    #[default]
    On,
}

impl MinimapChoice {
    pub const ALL: [MinimapChoice; 2] = [MinimapChoice::Off, MinimapChoice::On];

    pub fn label(self) -> &'static str {
        match self {
            MinimapChoice::Off => "Off",
            MinimapChoice::On => "On",
        }
    }
}

/// The interface settings, saved in `interface.ron`.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct InterfaceSettings {
    pub ui_scale: UiScaleChoice,
    pub fps_counter: FpsCounterChoice,
    pub minimap: MinimapChoice,
}

/// The saved settings, or the default ones when none were saved or the file cannot be read.
pub fn load_interface_settings(paths: &GameFolderPaths) -> InterfaceSettings {
    let path = paths.game_folder_path.join(INTERFACE_SETTINGS_PATH);
    fs::read_to_string(&path)
        .ok()
        .and_then(|content| from_str(&content).ok())
        .unwrap_or_default()
}

/// Writes the settings to `interface.ron` in the game folder.
pub fn write_interface_settings(settings: &InterfaceSettings, paths: &GameFolderPaths) {
    let path = paths.game_folder_path.join(INTERFACE_SETTINGS_PATH);
    let serialized = match ron::ser::to_string_pretty(settings, PrettyConfig::new()) {
        Ok(serialized) => serialized,
        Err(e) => {
            error!("Failed to serialize the interface settings: {e}");
            return;
        }
    };
    match fs::write(&path, serialized) {
        Ok(()) => info!("Interface settings saved to {path:?}"),
        Err(e) => error!("Failed to save the interface settings to {path:?}: {e}"),
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
    fn the_interface_settings_are_saved_and_loaded_back() {
        let paths = temp_paths("interface-roundtrip");
        let settings = InterfaceSettings {
            ui_scale: UiScaleChoice::X1_5,
            fps_counter: FpsCounterChoice::Graph,
            minimap: MinimapChoice::Off,
        };
        write_interface_settings(&settings, &paths);
        let reloaded = load_interface_settings(&paths);
        fs::remove_dir_all(&paths.game_folder_path).unwrap();
        assert_eq!(reloaded, settings);
    }

    #[test]
    fn a_missing_or_unreadable_interface_file_gives_auto() {
        let paths = temp_paths("interface-unreadable");
        let file = paths.game_folder_path.join(INTERFACE_SETTINGS_PATH);
        assert_eq!(
            load_interface_settings(&paths).ui_scale,
            UiScaleChoice::Auto
        );
        for (content, expected) in [
            ("garbage", UiScaleChoice::Auto),
            ("()", UiScaleChoice::Auto),
            ("(ui_scale: X2)", UiScaleChoice::X2),
        ] {
            fs::write(&file, content).unwrap();
            assert_eq!(
                load_interface_settings(&paths).ui_scale,
                expected,
                "{content}"
            );
        }
        fs::remove_dir_all(&paths.game_folder_path).unwrap();
    }

    #[test]
    fn the_minimap_shows_unless_saved_off() {
        let paths = temp_paths("interface-minimap");
        let file = paths.game_folder_path.join(INTERFACE_SETTINGS_PATH);
        assert_eq!(load_interface_settings(&paths).minimap, MinimapChoice::On);
        for (content, expected) in [
            // a file saved before the minimap existed
            ("(ui_scale: X2, fps_counter: Graph)", MinimapChoice::On),
            ("(minimap: Off)", MinimapChoice::Off),
        ] {
            fs::write(&file, content).unwrap();
            assert_eq!(
                load_interface_settings(&paths).minimap,
                expected,
                "{content}"
            );
        }
        fs::remove_dir_all(&paths.game_folder_path).unwrap();
    }

    #[test]
    fn the_fps_counter_choices_go_round() {
        use FpsCounterChoice::*;
        assert_eq!(Off.next(), Number);
        assert_eq!(Number.next(), Graph);
        assert_eq!(Graph.next(), Off);
    }

    #[test]
    fn the_fps_counter_is_off_unless_saved_on() {
        let paths = temp_paths("interface-fps");
        let file = paths.game_folder_path.join(INTERFACE_SETTINGS_PATH);
        for (content, expected) in [
            ("(ui_scale: X2)", FpsCounterChoice::Off),
            ("(fps_counter: Number)", FpsCounterChoice::Number),
            ("(fps_counter: Graph)", FpsCounterChoice::Graph),
        ] {
            fs::write(&file, content).unwrap();
            assert_eq!(
                load_interface_settings(&paths).fps_counter,
                expected,
                "{content}"
            );
        }
        fs::remove_dir_all(&paths.game_folder_path).unwrap();
    }
}
