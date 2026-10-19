//! The sound effects. The gameplay never names a file: it sends a `PlaySound`, or a message
//! that the triggers below turn into one.

use bevy::audio::{DefaultSpatialScale, SpatialScale};
use bevy::prelude::*;
use bevy::ui_widgets::Activate;

use crate::state::{GameSet, GameState};

mod blocks;
mod footsteps;
mod playback;
pub mod settings;
mod sounds;

use playback::PlaySound;
use sounds::Sound;

/// Distances in blocks are multiplied by this before the spatial attenuation, which divides a
/// sound's volume by the square of its distance: full volume up to 4 blocks away, a sixteenth
/// at 16 blocks.
const SPATIAL_SCALE: f32 = 0.25;

pub fn audio_plugin(app: &mut App) {
    app.insert_resource(DefaultSpatialScale(SpatialScale::new(SPATIAL_SCALE)))
        .add_message::<PlaySound>()
        .add_observer(click_on_activate)
        .add_systems(Startup, sounds::load_sound_library)
        .add_systems(
            Update,
            (
                click_sounds,
                (
                    blocks::block_sounds,
                    footsteps::local_footsteps,
                    footsteps::other_players_footsteps,
                )
                    .after(GameSet::Reconciliation)
                    .run_if(in_state(GameState::Game)),
                playback::play_sounds,
            )
                .chain(),
        );
}

/// A click for every widget button activated, as `click_sounds` gives the legacy buttons.
fn click_on_activate(_: On<Activate>, mut sounds: MessageWriter<PlaySound>) {
    sounds.write(PlaySound::here(Sound::Click));
}

/// A click for every legacy `Button` pressed. The pause menu's are the last ones (Lot 6): every
/// other screen uses the widget buttons, which `click_on_activate` gives a click.
fn click_sounds(
    buttons: Query<&Interaction, (Changed<Interaction>, With<Button>)>,
    mut sounds: MessageWriter<PlaySound>,
) {
    for interaction in &buttons {
        if *interaction == Interaction::Pressed {
            sounds.write(PlaySound::here(Sound::Click));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn activating_a_widget_button_clicks() {
        let mut app = App::new();
        app.add_message::<PlaySound>()
            .add_observer(click_on_activate);
        let entity = app.world_mut().spawn_empty().id();
        app.world_mut().trigger(Activate { entity });
        let heard: Vec<_> = app
            .world_mut()
            .resource_mut::<Messages<PlaySound>>()
            .drain()
            .collect();
        assert_eq!(heard, [PlaySound::here(Sound::Click)]);
    }
}
