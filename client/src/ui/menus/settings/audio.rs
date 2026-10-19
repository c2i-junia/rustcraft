//! The Audio tab of the settings: a slider per volume, saved as soon as it is set.

use bevy::{prelude::*, ui_widgets::ValueChange};
use shared::GameFolderPaths;

use crate::audio::settings::{write_audio_settings, AudioSettings, Channel};
use crate::ui::widgets::slider::{spawn_slider, SliderField, SliderScale};

/// The slider of the volume of a channel.
#[derive(Component)]
pub struct VolumeSlider(pub Channel);

/// Spawns a slider per channel under `panel`.
pub fn spawn_audio_tab(
    commands: &mut Commands,
    panel: Entity,
    settings: &AudioSettings,
    font: Handle<Font>,
) {
    for channel in Channel::ALL {
        let field = SliderField {
            label: channel.label(),
            value: settings.get(channel),
            scale: SliderScale::PERCENT,
        };
        let parts = spawn_slider(commands, &field, font.clone());
        commands.entity(parts.root).insert(ChildOf(panel));
        commands
            .entity(parts.slider)
            .insert(VolumeSlider(channel))
            .observe(set_volume)
            .observe(save_on_release);
    }
}

/// Sets the volume of the slider's channel, and saves it once the player is done with it.
fn set_volume(
    change: On<ValueChange<f32>>,
    sliders: Query<&VolumeSlider>,
    mut settings: ResMut<AudioSettings>,
    paths: Res<GameFolderPaths>,
) {
    let Ok(VolumeSlider(channel)) = sliders.get(change.source) else {
        return;
    };
    settings.set(*channel, change.value);
    if change.is_final {
        write_audio_settings(&settings, &paths);
    }
}

/// Saves the volumes when the button is released on a slider: a click on the track sets a value
/// that the widget never reports as final.
fn save_on_release(
    _: On<Pointer<Release>>,
    settings: Res<AudioSettings>,
    paths: Res<GameFolderPaths>,
) {
    write_audio_settings(&settings, &paths);
}
