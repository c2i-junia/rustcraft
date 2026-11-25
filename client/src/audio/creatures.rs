//! The voices of the creatures: a cry now and then from each animal and from each hostile at
//! rest, the approach of a hostile hunting a player, again and again while it hunts, and its
//! attack as it strikes. They follow what the server says a mob does (`Mob::action`): a hostile
//! that starts to chase is heard approaching, one that starts to strike attacks.

use std::ops::Range;

use bevy::prelude::*;
use rand::Rng;
use shared::world::{MobAction, MobKind};

use super::playback::PlaySound;
use super::sounds::{CreatureCall, Sound, SoundMaterial};
use crate::mob::Mob;
use crate::portal::PortalCamera;

/// Seconds between two cries of an animal.
const ANIMAL_CRY_DELAY: Range<f32> = 10.0..30.0;
/// Seconds between two cries of a hostile not hunting: they are fewer, and should be heard.
const HOSTILE_CRY_DELAY: Range<f32> = 6.0..16.0;
/// Seconds between two approach sounds of a hostile hunting.
const APPROACH_DELAY: Range<f32> = 4.0..8.0;
/// A creature farther than this from the ears, in blocks, is not heard: the spatial attenuation
/// would leave a 64th of it.
pub const HEARING_DISTANCE: f32 = 32.0;

/// What a creature says now and then while it does `action`: a hostile that hunts approaches,
/// one that strikes or burrows and an ember in flight say nothing more, the others cry.
pub fn periodic_call(kind: MobKind, action: MobAction) -> Option<CreatureCall> {
    if kind.is_projectile() {
        return None;
    }
    match action {
        MobAction::Chase if kind.is_hostile() => Some(CreatureCall::Approach),
        MobAction::Attack | MobAction::Burrow => None,
        _ => Some(CreatureCall::Cry),
    }
}

/// The seconds a creature of `kind` doing `action` waits before its next call, if it calls.
fn call_delay(kind: MobKind, action: MobAction) -> Option<Range<f32>> {
    Some(match periodic_call(kind, action)? {
        CreatureCall::Approach => APPROACH_DELAY,
        _ if kind.is_hostile() => HOSTILE_CRY_DELAY,
        _ => ANIMAL_CRY_DELAY,
    })
}

/// What a creature of `kind` says at once as it goes from doing `from` to doing `to`: a hostile
/// approaches as it starts to hunt (not when it hunts on after a strike, but the Roller, which
/// winds up before it rolls, rumbles as it starts to roll), and attacks as it strikes.
pub fn call_on_change(kind: MobKind, from: MobAction, to: MobAction) -> Option<CreatureCall> {
    if !kind.is_hostile() || from == to {
        return None;
    }
    match to {
        MobAction::Attack => Some(CreatureCall::Attack),
        MobAction::Chase if from != MobAction::Attack || kind == MobKind::Roller => {
            Some(CreatureCall::Approach)
        }
        _ => None,
    }
}

/// When a creature calls next, and what it did when last seen.
#[derive(Component, Debug)]
pub struct CreatureVoice {
    action: MobAction,
    /// Seconds before its next call, while it does `action`.
    next_call: Option<f32>,
}

impl CreatureVoice {
    /// The voice of a creature first seen doing `action`, and what it says as it appears: an
    /// ember is heard as it is thrown. Its first cry comes at any time within the usual wait,
    /// so that a herd seen at once does not cry at once.
    pub fn new(
        kind: MobKind,
        action: MobAction,
        rng: &mut impl Rng,
    ) -> (Self, Option<CreatureCall>) {
        let next_call = call_delay(kind, action).map(|delay| rng.gen_range(0.0..delay.end));
        let first = (kind == MobKind::Ember).then_some(CreatureCall::Cry);
        (Self { action, next_call }, first)
    }

    /// Lets `seconds` pass for a creature of `kind`, now doing `action`; returns what it says.
    pub fn hear(
        &mut self,
        kind: MobKind,
        action: MobAction,
        seconds: f32,
        rng: &mut impl Rng,
    ) -> Option<CreatureCall> {
        if action != self.action {
            let from = std::mem::replace(&mut self.action, action);
            let now = call_on_change(kind, from, action);
            self.next_call = call_delay(kind, action).map(|delay| rng.gen_range(delay));
            if now.is_some() {
                return now;
            }
        }
        let left = self.next_call.as_mut()?;
        *left -= seconds;
        if *left > 0.0 {
            return None;
        }
        self.next_call = call_delay(kind, action).map(|delay| rng.gen_range(delay));
        periodic_call(kind, action)
    }
}

/// Gives each creature drawn its voice, and plays what it says where it is, if it is near
/// enough to be heard.
pub fn creature_calls(
    mut commands: Commands,
    time: Res<Time>,
    mut mobs: Query<(Entity, &Mob, &Transform, Option<&mut CreatureVoice>)>,
    cameras: Query<&GlobalTransform, (With<Camera3d>, Without<PortalCamera>)>,
    mut sounds: MessageWriter<PlaySound>,
) {
    let ears = cameras.iter().next().map(GlobalTransform::translation);
    let mut rng = rand::thread_rng();
    for (entity, mob, transform, voice) in &mut mobs {
        // An arrow is heard as it is let fly and as it sticks in a block
        if mob.kind == MobKind::Arrow {
            let sound = match &voice {
                None if mob.action != MobAction::Idle => Some(Sound::BowLoose),
                Some(voice) if mob.action == MobAction::Idle && voice.action != mob.action => {
                    Some(Sound::Dig(SoundMaterial::Wood))
                }
                _ => None,
            };
            let at = transform.translation;
            if ears.is_some_and(|ears| ears.distance(at) <= HEARING_DISTANCE) {
                sounds.write_batch(sound.map(|sound| PlaySound::at(sound, at)));
            }
        }
        let call = match voice {
            Some(mut voice) => voice.hear(mob.kind, mob.action, time.delta_secs(), &mut rng),
            None => {
                let (voice, first) = CreatureVoice::new(mob.kind, mob.action, &mut rng);
                commands.entity(entity).insert(voice);
                first
            }
        };
        let at = transform.translation;
        let heard = ears.is_some_and(|ears| ears.distance(at) <= HEARING_DISTANCE);
        if let (Some(call), true) = (call, heard) {
            sounds.write(PlaySound::at(Sound::Creature(mob.kind, call), at));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::{rngs::StdRng, SeedableRng};

    /// The calls of a creature of `kind` doing `actions` one after the other, each for
    /// `seconds`, in steps of a 20th of a second, with the time of each.
    fn calls(kind: MobKind, actions: &[(MobAction, f32)]) -> Vec<(f32, CreatureCall)> {
        let mut rng = StdRng::seed_from_u64(7);
        let (mut voice, first) = CreatureVoice::new(kind, actions[0].0, &mut rng);
        let mut heard: Vec<_> = first.map(|call| (0.0, call)).into_iter().collect();
        let mut now = 0.0;
        for &(action, seconds) in actions {
            let end = now + seconds;
            while now < end {
                now += 0.05;
                if let Some(call) = voice.hear(kind, action, 0.05, &mut rng) {
                    heard.push((now, call));
                }
            }
        }
        heard
    }

    #[test]
    fn an_animal_cries_now_and_then_within_its_delays() {
        let heard = calls(MobKind::Sheep, &[(MobAction::Graze, 600.0)]);
        assert!(heard.iter().all(|(_, call)| *call == CreatureCall::Cry));
        // At most one every 10 s, at least one every 30 s
        assert!((20..=61).contains(&heard.len()), "{}", heard.len());
        for pair in heard.windows(2) {
            let gap = pair[1].0 - pair[0].0;
            assert!(
                (ANIMAL_CRY_DELAY.start - 0.06..=ANIMAL_CRY_DELAY.end + 0.06).contains(&gap),
                "{gap}"
            );
        }
        assert!(heard[0].0 <= ANIMAL_CRY_DELAY.end);
    }

    #[test]
    fn a_herd_seen_at_once_does_not_cry_at_once() {
        let mut rng = StdRng::seed_from_u64(3);
        let firsts: Vec<f32> = (0..10)
            .map(|_| {
                CreatureVoice::new(MobKind::Cow, MobAction::Idle, &mut rng)
                    .0
                    .next_call
                    .unwrap()
            })
            .collect();
        let earliest = firsts.iter().copied().fold(f32::MAX, f32::min);
        let latest = firsts.iter().copied().fold(0.0, f32::max);
        assert!(latest - earliest > 5.0, "{firsts:?}");
    }

    #[test]
    fn a_hostile_is_heard_approaching_as_it_starts_to_hunt_and_then_again_and_again() {
        let heard = calls(
            MobKind::Gnawer,
            &[(MobAction::Idle, 0.5), (MobAction::Chase, 30.0)],
        );
        let approaches: Vec<f32> = heard
            .iter()
            .filter(|(_, call)| *call == CreatureCall::Approach)
            .map(|(at, _)| *at)
            .collect();
        assert!((approaches[0] - 0.55).abs() < 0.01, "{heard:?}");
        assert!(approaches.len() >= 4, "{approaches:?}");
        for pair in approaches.windows(2) {
            assert!(pair[1] - pair[0] >= APPROACH_DELAY.start - 0.06);
        }
        // No cry while it hunts
        assert!(heard
            .iter()
            .all(|(at, call)| *call != CreatureCall::Cry || *at < 0.55));
    }

    #[test]
    fn a_hostile_attacks_as_it_strikes_and_hunts_on_without_announcing_it_again() {
        let heard = calls(
            MobKind::Stumpling,
            &[
                (MobAction::Chase, 1.0),
                (MobAction::Attack, 0.5),
                (MobAction::Chase, 1.0),
            ],
        );
        // From the strike on: the attack, and no approach as it hunts on
        let after: Vec<CreatureCall> = heard
            .iter()
            .filter(|(at, _)| *at > 1.0)
            .map(|(_, call)| *call)
            .collect();
        assert_eq!(after, [CreatureCall::Attack], "{heard:?}");
    }

    #[test]
    fn the_roller_rumbles_as_it_rolls_after_its_wind_up() {
        assert_eq!(
            call_on_change(MobKind::Roller, MobAction::Attack, MobAction::Chase),
            Some(CreatureCall::Approach)
        );
        assert_eq!(
            call_on_change(MobKind::Roller, MobAction::Idle, MobAction::Attack),
            Some(CreatureCall::Attack)
        );
    }

    #[test]
    fn an_animal_says_nothing_more_when_it_flees() {
        for (from, to) in [
            (MobAction::Idle, MobAction::Flee),
            (MobAction::Walk, MobAction::Chase),
        ] {
            assert_eq!(call_on_change(MobKind::Pig, from, to), None);
        }
    }

    #[test]
    fn an_ember_is_heard_once_as_it_is_thrown() {
        let heard = calls(MobKind::Ember, &[(MobAction::Walk, 5.0)]);
        assert_eq!(heard, [(0.0, CreatureCall::Cry)]);
    }

    #[test]
    fn a_hostile_at_rest_cries_more_often_than_an_animal() {
        assert_eq!(
            call_delay(MobKind::GustOwl, MobAction::Idle),
            Some(HOSTILE_CRY_DELAY)
        );
        const { assert!(HOSTILE_CRY_DELAY.end < ANIMAL_CRY_DELAY.end) };
        assert_eq!(periodic_call(MobKind::Stumpling, MobAction::Burrow), None);
    }
}
