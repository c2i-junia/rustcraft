//! The sound effects. The gameplay never names a file: it sends a `PlaySound`, or a message
//! that the triggers below turn into one.

use bevy::audio::{DefaultSpatialScale, SpatialScale};
use bevy::prelude::*;
use bevy::ui_widgets::Activate;

use crate::dropped_items::ItemPickedUp;
use crate::mob::MobHurt;
use crate::player::LocalPlayerHurt;
use crate::portal::PortalOpened;
use crate::state::{GameSet, GameState};

mod blocks;
mod footsteps;
mod inventory;
mod playback;
pub mod settings;
pub(crate) mod sounds;
pub(crate) mod weather;

use playback::PlaySound;
use sounds::Sound;

/// Distances in blocks are multiplied by this before the spatial attenuation, which divides a
/// sound's volume by the square of its distance: full volume up to 4 blocks away, a sixteenth
/// at 16 blocks.
const SPATIAL_SCALE: f32 = 0.25;

pub fn audio_plugin(app: &mut App) {
    app.insert_resource(DefaultSpatialScale(SpatialScale::new(SPATIAL_SCALE)))
        .add_message::<PlaySound>()
        .add_plugins(weather::weather_sound_plugin)
        .add_observer(click_on_activate)
        .add_systems(Startup, sounds::load_sound_library)
        .add_systems(
            Update,
            (
                (
                    blocks::block_sounds,
                    footsteps::local_footsteps,
                    footsteps::other_players_footsteps,
                    pickup_sounds,
                    hurt_sounds,
                    mob_hurt_sounds,
                    inventory::inventory_sounds,
                    portal_sounds,
                )
                    .after(GameSet::Reconciliation)
                    .run_if(in_state(GameState::Game)),
                playback::play_sounds,
            )
                .chain(),
        );
}

/// A pop for every item the local player picks up.
fn pickup_sounds(mut pickups: MessageReader<ItemPickedUp>, mut sounds: MessageWriter<PlaySound>) {
    for _ in pickups.read() {
        sounds.write(PlaySound::here(Sound::Pickup));
    }
}

/// A grunt for every hurt of the local player.
fn hurt_sounds(mut hurts: MessageReader<LocalPlayerHurt>, mut sounds: MessageWriter<PlaySound>) {
    for _ in hurts.read() {
        sounds.write(PlaySound::here(Sound::Hurt));
    }
}

/// A blow on a mob, heard where it is.
fn mob_hurt_sounds(mut hurts: MessageReader<MobHurt>, mut sounds: MessageWriter<PlaySound>) {
    for MobHurt { at } in hurts.read() {
        sounds.write(PlaySound::at(Sound::MobHurt, *at));
    }
}

/// The opening of a portal, heard where it is.
fn portal_sounds(mut opened: MessageReader<PortalOpened>, mut sounds: MessageWriter<PlaySound>) {
    for PortalOpened { at } in opened.read() {
        sounds.write(PlaySound::at(Sound::PortalOpen, *at));
    }
}

/// A click for every widget button activated.
fn click_on_activate(_: On<Activate>, mut sounds: MessageWriter<PlaySound>) {
    sounds.write(PlaySound::here(Sound::Click));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_hurt_of_the_local_player_plays_the_hurt_sound() {
        let mut app = App::new();
        app.add_message::<PlaySound>()
            .add_message::<LocalPlayerHurt>()
            .add_systems(Update, hurt_sounds);
        app.world_mut().write_message(LocalPlayerHurt);
        app.update();
        let heard: Vec<_> = app
            .world_mut()
            .resource_mut::<Messages<PlaySound>>()
            .drain()
            .collect();
        assert_eq!(heard, [PlaySound::here(Sound::Hurt)]);
    }

    #[test]
    fn a_hurt_mob_is_heard_where_it_is() {
        let mut app = App::new();
        app.add_message::<PlaySound>()
            .add_message::<MobHurt>()
            .add_systems(Update, mob_hurt_sounds);
        let at = Vec3::new(1.0, 2.0, 3.0);
        app.world_mut().write_message(MobHurt { at });
        app.update();
        let heard: Vec<_> = app
            .world_mut()
            .resource_mut::<Messages<PlaySound>>()
            .drain()
            .collect();
        assert_eq!(heard, [PlaySound::at(Sound::MobHurt, at)]);
    }

    #[test]
    fn a_portal_that_opens_is_heard_where_it_is() {
        let mut app = App::new();
        app.add_message::<PlaySound>()
            .add_message::<PortalOpened>()
            .add_systems(Update, portal_sounds);
        let at = Vec3::new(1.0, 2.0, 3.0);
        app.world_mut().write_message(PortalOpened { at });
        app.update();
        let heard: Vec<_> = app
            .world_mut()
            .resource_mut::<Messages<PlaySound>>()
            .drain()
            .collect();
        assert_eq!(heard, [PlaySound::at(Sound::PortalOpen, at)]);
    }

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
