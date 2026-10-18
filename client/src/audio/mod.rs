//! The sound effects. The gameplay never names a file: it sends a `PlaySound`, or a message
//! that the triggers below turn into one.

use bevy::audio::{DefaultSpatialScale, SpatialScale};
use bevy::prelude::*;

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

/// A click for every button pressed, in the menus and in the game.
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
