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

/// Volumes from 0 (muted) to 1 (as the file is), in steps of `VOLUME_STEP`.
#[derive(Resource, Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(default)]
pub struct AudioSettings {
    pub master: f32,
    pub effects: f32,
    pub interface: f32,
}

pub const VOLUME_STEP: f32 = 0.1;

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

    /// Moves the volume of `channel` by `steps` steps, within 0 and 1.
    pub fn step(&mut self, channel: Channel, steps: i32) {
        let volume = match channel {
            Channel::Master => &mut self.master,
            Channel::Effects => &mut self.effects,
            Channel::Interface => &mut self.interface,
        };
        // Rounded to the step, so that repeated steps do not drift away from round values
        let stepped = (*volume / VOLUME_STEP).round() + steps as f32;
        *volume = (stepped * VOLUME_STEP).clamp(0.0, 1.0);
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

pub fn save_audio_settings(settings: Res<AudioSettings>, paths: Res<GameFolderPaths>) {
    let path = paths.game_folder_path.join(AUDIO_SETTINGS_PATH);
    let serialized = match ron::ser::to_string_pretty(settings.into_inner(), PrettyConfig::new()) {
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
    fn stepping_stays_within_zero_and_one_on_round_values() {
        let mut settings = AudioSettings::default();
        settings.step(Channel::Effects, 1);
        assert_eq!(settings.effects, 1.0);
        for _ in 0..3 {
            settings.step(Channel::Effects, -1);
        }
        assert!((settings.effects - 0.7).abs() < 1e-6);
        settings.step(Channel::Effects, -20);
        assert_eq!(settings.effects, 0.0);
    }

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
        let mut app = App::new();
        app.insert_resource(settings)
            .insert_resource(paths.clone())
            .add_systems(Update, save_audio_settings);
        app.update();

        let reloaded = load_audio_settings(&paths);
        fs::remove_dir_all(&dir).unwrap();
        assert_eq!(reloaded, settings);
    }
}
