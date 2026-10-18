use bevy::audio::Volume;
use bevy::prelude::*;
use rand::{seq::SliceRandom, Rng};

use super::settings::AudioSettings;
use super::sounds::{Sound, SoundLibrary};

/// Asks for `sound` to be played once.
#[derive(Message, Clone, Copy, Debug, PartialEq)]
pub struct PlaySound {
    pub sound: Sound,
    /// Where in the world the sound comes from, heard from the camera. `None` plays it as is,
    /// as the interface and the local player's own steps are.
    pub at: Option<Vec3>,
}

impl PlaySound {
    pub fn here(sound: Sound) -> Self {
        Self { sound, at: None }
    }

    pub fn at(sound: Sound, position: Vec3) -> Self {
        Self {
            sound,
            at: Some(position),
        }
    }
}

/// Playback speeds a sound is picked from, which shift its pitch: the same sound played twice
/// in a row does not sound mechanical.
const SPEED_RANGE: std::ops::Range<f32> = 0.92..1.08;

/// Spawns one entity per requested sound, which despawns once the sound has played.
pub fn play_sounds(
    mut commands: Commands,
    mut requests: MessageReader<PlaySound>,
    library: Res<SoundLibrary>,
    settings: Res<AudioSettings>,
) {
    let mut rng = rand::thread_rng();
    for request in requests.read() {
        let volume = settings.volume_of(request.sound.channel());
        if volume <= 0.0 {
            continue;
        }
        let Some(source) = library
            .0
            .get(&request.sound)
            .and_then(|variants| variants.choose(&mut rng))
        else {
            continue;
        };

        let playback = PlaybackSettings::DESPAWN
            .with_volume(Volume::Linear(volume))
            .with_speed(rng.gen_range(SPEED_RANGE))
            .with_spatial(request.at.is_some());
        let mut entity = commands.spawn((AudioPlayer::new(source.clone()), playback));
        if let Some(position) = request.at {
            entity.insert(Transform::from_translation(position));
        }
    }
}
