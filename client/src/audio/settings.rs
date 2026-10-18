//! The volumes the player chose, saved in `audio.ron` in the game folder.

use bevy::prelude::*;
use ron::{from_str, ser::PrettyConfig};
use serde::{Deserialize, Serialize};
use shared::GameFolderPaths;
use std::fs;

use crate::constants::AUDIO_SETTINGS_PATH;

/// A volume the player sets. Every sound plays on `Effects` or `Interface`, scaled by `Master`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Channel {
    Master,
    Effects,
    Interface,
}

impl Channel {
    pub const ALL: [Channel; 3] = [Channel::Master, Channel::Effects, Channel::Interface];

    pub fn label(self) -> &'static str {
        match self {
            Channel::Master => "Master",
            Channel::Effects => "Effects",
            Channel::Interface => "Interface",
        }
    }
}

/// Volumes from 0 (muted) to 1 (as the file is).
#[derive(Resource, Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(default)]
pub struct AudioSettings {
    pub master: f32,
    pub effects: f32,
    pub interface: f32,
}

impl Default for AudioSettings {
    fn default() -> Self {
        Self {
            master: 1.0,
            effects: 1.0,
            interface: 1.0,
        }
    }
}

impl AudioSettings {
    pub fn get(&self, channel: Channel) -> f32 {
        match channel {
            Channel::Master => self.master,
            Channel::Effects => self.effects,
            Channel::Interface => self.interface,
        }
    }

    /// The volume a sound of `channel` plays at.
    pub fn volume_of(&self, channel: Channel) -> f32 {
        self.master * self.get(channel)
    }

    /// Sets the volume of `channel`, within 0 and 1.
    pub fn set(&mut self, channel: Channel, volume: f32) {
        let slot = match channel {
            Channel::Master => &mut self.master,
            Channel::Effects => &mut self.effects,
            Channel::Interface => &mut self.interface,
        };
        *slot = volume.clamp(0.0, 1.0);
    }
}

/// The saved settings, or the default ones when none were saved or the file cannot be read.
pub fn load_audio_settings(paths: &GameFolderPaths) -> AudioSettings {
    let path = paths.game_folder_path.join(AUDIO_SETTINGS_PATH);
    fs::read_to_string(&path)
        .ok()
        .and_then(|content| from_str(&content).ok())
        .unwrap_or_default()
}

/// Writes the settings to `audio.ron` in the game folder.
pub fn write_audio_settings(settings: &AudioSettings, paths: &GameFolderPaths) {
    let path = paths.game_folder_path.join(AUDIO_SETTINGS_PATH);
    let serialized = match ron::ser::to_string_pretty(settings, PrettyConfig::new()) {
        Ok(serialized) => serialized,
        Err(e) => {
            error!("Failed to serialize the audio settings: {e}");
            return;
        }
    };
    match fs::write(&path, serialized) {
        Ok(()) => info!("Audio settings saved to {path:?}"),
        Err(e) => error!("Failed to save the audio settings to {path:?}: {e}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_channel_is_scaled_by_the_master_volume() {
        let settings = AudioSettings {
            master: 0.5,
            effects: 0.4,
            interface: 1.0,
        };
        assert!((settings.volume_of(Channel::Effects) - 0.2).abs() < 1e-6);
        assert_eq!(settings.volume_of(Channel::Interface), 0.5);
    }

    #[test]
    fn saved_settings_are_loaded_back_from_the_game_folder() {
        let dir = std::env::temp_dir().join(format!("rustcraft-audio-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let paths = GameFolderPaths {
            game_folder_path: dir.clone(),
            assets_folder_path: dir.join("assets"),
        };
        assert_eq!(load_audio_settings(&paths), AudioSettings::default());

        let settings = AudioSettings {
            master: 0.3,
            effects: 0.6,
            interface: 0.0,
        };
        write_audio_settings(&settings, &paths);

        let reloaded = load_audio_settings(&paths);
        fs::remove_dir_all(&dir).unwrap();
        assert_eq!(reloaded, settings);
    }

    #[test]
    fn setting_a_volume_clamps_it() {
        let mut settings = AudioSettings::default();
        for (volume, expected) in [(1.4, 1.0), (-0.1, 0.0), (0.55, 0.55)] {
            settings.set(Channel::Effects, volume);
            assert_eq!(settings.effects, expected);
        }
    }

    #[test]
    fn an_old_audio_file_still_loads() {
        let dir = std::env::temp_dir().join(format!("rustcraft-old-audio-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let paths = GameFolderPaths {
            game_folder_path: dir.clone(),
            assets_folder_path: dir.join("assets"),
        };
        fs::write(dir.join(AUDIO_SETTINGS_PATH), "(master: 0.7)").unwrap();
        let loaded = load_audio_settings(&paths);
        fs::remove_dir_all(&dir).unwrap();
        assert_eq!(
            loaded,
            AudioSettings {
                master: 0.7,
                ..default()
            }
        );
    }
}
