//! The sound effects. The gameplay never names a file: it sends a `PlaySound`, or a message
//! that the triggers below turn into one.

use bevy::audio::{DefaultSpatialScale, SpatialScale};
use bevy::prelude::*;
use bevy::ui_widgets::Activate;

use crate::mob::MobHurt;
use crate::player::LocalPlayerHurt;
use crate::portal::PortalOpened;
use crate::state::{GameSet, GameState};
use crate::ui::hud::{depths, progress};

mod blocks;
mod creatures;
mod cues;
mod footsteps;
mod inventory;
pub mod music;
mod playback;
pub mod settings;
pub(crate) mod sounds;
pub(crate) mod weather;

pub(crate) use playback::PlaySound;
use sounds::CreatureCall;
pub(crate) use sounds::Sound;

/// Distances in blocks are multiplied by this before the spatial attenuation, which divides a
/// sound's volume by the square of its distance: full volume up to 4 blocks away, a sixteenth
/// at 16 blocks.
const SPATIAL_SCALE: f32 = 0.25;

pub fn audio_plugin(app: &mut App) {
    app.insert_resource(DefaultSpatialScale(SpatialScale::new(SPATIAL_SCALE)))
        .add_message::<PlaySound>()
        .add_plugins((weather::weather_sound_plugin, music::music_plugin))
        .add_observer(click_on_activate)
        .add_observer(cues::refused_click)
        .add_systems(Startup, sounds::load_sound_library)
        .add_systems(
            Update,
            (
                (
                    blocks::block_sounds,
                    footsteps::local_footsteps,
                    footsteps::other_players_footsteps,
                    cues::pickup_sounds,
                    hurt_sounds,
                    mob_hurt_sounds,
                    creatures::creature_calls,
                    inventory::inventory_sounds,
                    portal_sounds,
                    (
                        cues::player_cues,
                        cues::arrow_cues,
                        cues::mob_cues,
                        cues::level_cues,
                        cues::trade_cues,
                        cues::depth_cue.before(depths::announce_depth),
                        cues::achievement_cue.before(progress::show_earned),
                        cues::boss_cue,
                    ),
                )
                    .after(GameSet::Reconciliation)
                    .run_if(in_state(GameState::Game)),
                // The credits roll from the menu too
                cues::credits_cue,
                playback::play_sounds,
            )
                .chain(),
        );
}

/// A grunt for every hurt of the local player.
fn hurt_sounds(mut hurts: MessageReader<LocalPlayerHurt>, mut sounds: MessageWriter<PlaySound>) {
    for _ in hurts.read() {
        sounds.write(PlaySound::here(Sound::Hurt));
    }
}

/// A blow on a mob, heard where it is, in the voice of its kind.
fn mob_hurt_sounds(mut hurts: MessageReader<MobHurt>, mut sounds: MessageWriter<PlaySound>) {
    for MobHurt { kind, at } in hurts.read() {
        sounds.write(PlaySound::at(
            Sound::Creature(*kind, CreatureCall::Hurt),
            *at,
        ));
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
    use shared::world::MobKind;

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
    fn a_hurt_mob_is_heard_where_it_is_in_its_own_voice() {
        let mut app = App::new();
        app.add_message::<PlaySound>()
            .add_message::<MobHurt>()
            .add_systems(Update, mob_hurt_sounds);
        let at = Vec3::new(1.0, 2.0, 3.0);
        app.world_mut().write_message(MobHurt {
            kind: MobKind::Gnawer,
            at,
        });
        app.update();
        let heard: Vec<_> = app
            .world_mut()
            .resource_mut::<Messages<PlaySound>>()
            .drain()
            .collect();
        assert_eq!(
            heard,
            [PlaySound::at(
                Sound::Creature(MobKind::Gnawer, CreatureCall::Hurt),
                at
            )]
        );
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
