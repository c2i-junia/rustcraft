//! The status effects: poison, burn, slowness, weakness and regeneration, each with a level and
//! a time left. The server gives them (the blows of some hostiles, an ember), counts them down
//! once a tick (`Player::tick_effects`) and sends them to the client, which shows them over the
//! hotbar; they are saved with the player, and a bucket of milk takes them all away.
//!
//! The system is generic: a new effect is a variant of `EffectKind` and its line in `on_tick`,
//! and whatever gives one calls `StatusEffects::give`.

use serde::{Deserialize, Serialize};

use super::game_mode::{DamageSource, MAX_HEALTH};
use super::Player;
use crate::TICKS_PER_SECOND;

/// What an effect does. Append only: it travels in `PlayerUpdateEvent` and is saved.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EffectKind {
    /// A point of health every `POISON_PERIOD` ticks, quicker by level, but never the last one.
    Poison,
    /// A point of health every `BURN_PERIOD` ticks, quicker by level: it may kill.
    Burn,
    /// The player walks slower: `SLOWNESS_PER_LEVEL` less per level.
    Slowness,
    /// The blows of the player hurt `WEAKNESS_PER_LEVEL` less per level.
    Weakness,
    /// A point of health back every `REGENERATION_PERIOD` ticks, quicker by level.
    Regeneration,
}

impl EffectKind {
    pub const ALL: [EffectKind; 5] = [
        Self::Poison,
        Self::Burn,
        Self::Slowness,
        Self::Weakness,
        Self::Regeneration,
    ];

    /// Whether the effect does the player good: the HUD frames it apart.
    pub fn is_beneficial(self) -> bool {
        self == Self::Regeneration
    }
}

/// The ticks between two points of health of each effect over time, at level 1.
pub const POISON_PERIOD: u32 = 25;
pub const BURN_PERIOD: u32 = 20;
pub const REGENERATION_PERIOD: u32 = 50;
/// What a level of slowness takes from the speed of the player, as a share.
pub const SLOWNESS_PER_LEVEL: f32 = 0.15;
/// The points a level of weakness takes from a blow.
pub const WEAKNESS_PER_LEVEL: u32 = 2;
/// The highest level an effect reaches.
pub const MAX_LEVEL: u8 = 4;

/// An effect on a player: its level, from 1, and the ticks it has left.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Effect {
    pub kind: EffectKind,
    pub level: u8,
    pub ticks_left: u32,
}

impl Effect {
    /// The seconds the effect has left, rounded up, as the HUD shows them.
    pub fn seconds_left(&self) -> u32 {
        self.ticks_left.div_ceil(TICKS_PER_SECOND as u32)
    }
}

/// The effects on a player, one at most of each kind, in the order they were given.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Default)]
pub struct StatusEffects(pub Vec<Effect>);

/// What a tick of the effects does to the health.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct EffectTick {
    /// The points of poison and of burn the player takes.
    pub poison: u32,
    pub burn: u32,
    /// The points of health the player gets back.
    pub healed: u32,
}

impl StatusEffects {
    /// The effect of `kind`, if the player is under it.
    pub fn get(&self, kind: EffectKind) -> Option<&Effect> {
        self.0.iter().find(|effect| effect.kind == kind)
    }

    /// The level of the effect of `kind`, 0 when the player is not under it.
    pub fn level(&self, kind: EffectKind) -> u8 {
        self.get(kind).map_or(0, |effect| effect.level)
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Gives the effect of `kind` at `level` for `ticks`. Over one of the same kind, the higher
    /// level takes its place, at the same level the longer time is kept, and a lower level is
    /// lost.
    pub fn give(&mut self, kind: EffectKind, level: u8, ticks: u32) {
        let level = level.clamp(1, MAX_LEVEL);
        if ticks == 0 {
            return;
        }
        let given = Effect {
            kind,
            level,
            ticks_left: ticks,
        };
        match self.0.iter_mut().find(|effect| effect.kind == kind) {
            None => self.0.push(given),
            Some(effect) if level > effect.level => *effect = given,
            Some(effect) if level == effect.level => {
                effect.ticks_left = effect.ticks_left.max(ticks)
            }
            Some(_) => {}
        }
    }

    /// Takes every effect away, as a bucket of milk does.
    pub fn clear(&mut self) {
        self.0.clear();
    }

    /// Counts one tick of every effect, and those run out are gone. Returns what the tick does
    /// to the health: the effects over time strike on the ticks their period divides, counted
    /// from their end, so that one given again keeps its rhythm.
    pub fn tick(&mut self) -> EffectTick {
        let mut done = EffectTick::default();
        for effect in &mut self.0 {
            let period = |base: u32| (base >> (effect.level - 1).min(4)).max(1);
            let strikes = |base| effect.ticks_left.is_multiple_of(period(base));
            match effect.kind {
                EffectKind::Poison if strikes(POISON_PERIOD) => done.poison += 1,
                EffectKind::Burn if strikes(BURN_PERIOD) => done.burn += 1,
                EffectKind::Regeneration if strikes(REGENERATION_PERIOD) => done.healed += 1,
                _ => {}
            }
            effect.ticks_left -= 1;
        }
        self.0.retain(|effect| effect.ticks_left > 0);
        done
    }

    /// What slowness leaves of the speed of the player, from 1.
    pub fn speed_factor(level: u8) -> f32 {
        (1.0 - SLOWNESS_PER_LEVEL * level as f32).max(0.2)
    }

    /// A blow of `damage` points by a player under weakness: never below 1.
    pub fn weakened(&self, damage: u32) -> u32 {
        let less = WEAKNESS_PER_LEVEL * self.level(EffectKind::Weakness) as u32;
        damage.saturating_sub(less).max(1)
    }
}

impl Player {
    /// A tick of the effects of the player: poison and burn hurt it, the poison never to death,
    /// regeneration heals it, and its slowness is set for the movement (`MovementState`). A
    /// dead player loses them.
    pub fn tick_effects(&mut self) {
        if self.is_dead() {
            self.effects.clear();
            self.slowness = 0;
            return;
        }
        let done = self.effects.tick();
        if done.poison > 0 && self.health > 1 {
            let amount = done.poison.min(self.health - 1);
            self.hurt(amount, DamageSource::Poison);
        }
        if done.burn > 0 {
            self.hurt(done.burn, DamageSource::Fire);
        }
        if done.healed > 0 && !self.is_dead() {
            self.health = (self.health + done.healed).min(MAX_HEALTH);
        }
        self.slowness = self.effects.level(EffectKind::Slowness);
    }

    /// Gives the player an effect, unless it takes no damage (creative) or is dead.
    pub fn give_effect(&mut self, kind: EffectKind, level: u8, seconds: u32) {
        if self.is_dead() || !self.game_mode.takes_damage() {
            return;
        }
        self.effects
            .give(kind, level, seconds * TICKS_PER_SECOND as u32);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::prelude::{Transform, Vec3};

    fn player() -> Player {
        Player::new(1, "patient".into(), Vec3::ZERO, Transform::default())
    }

    #[test]
    fn an_effect_lasts_its_time_then_is_gone() {
        let mut effects = StatusEffects::default();
        effects.give(EffectKind::Slowness, 1, 3);
        assert_eq!(effects.level(EffectKind::Slowness), 1);
        effects.tick();
        effects.tick();
        assert_eq!(effects.get(EffectKind::Slowness).unwrap().ticks_left, 1);
        effects.tick();
        assert!(effects.is_empty());
        assert_eq!(effects.level(EffectKind::Slowness), 0);
    }

    #[test]
    fn a_stronger_level_takes_the_place_and_the_same_level_keeps_the_longer_time() {
        let mut effects = StatusEffects::default();
        effects.give(EffectKind::Poison, 1, 100);
        effects.give(EffectKind::Poison, 1, 40);
        assert_eq!(effects.get(EffectKind::Poison).unwrap().ticks_left, 100);
        effects.give(EffectKind::Poison, 1, 200);
        assert_eq!(effects.get(EffectKind::Poison).unwrap().ticks_left, 200);
        effects.give(EffectKind::Poison, 2, 20);
        assert_eq!(effects.0.len(), 1);
        let poison = effects.get(EffectKind::Poison).unwrap();
        assert_eq!((poison.level, poison.ticks_left), (2, 20));
        // A weaker one is lost
        effects.give(EffectKind::Poison, 1, 500);
        assert_eq!(effects.level(EffectKind::Poison), 2);
        // Another kind adds up beside it
        effects.give(EffectKind::Weakness, 9, 10);
        assert_eq!(effects.level(EffectKind::Weakness), MAX_LEVEL);
        assert_eq!(effects.0.len(), 2);
    }

    #[test]
    fn poison_hurts_by_its_period_but_never_kills_and_burn_does() {
        let mut patient = player();
        patient.effects.give(EffectKind::Poison, 1, POISON_PERIOD * 4);
        for _ in 0..POISON_PERIOD * 4 {
            patient.invulnerable_ticks = 0;
            patient.tick_effects();
        }
        assert_eq!(patient.health, MAX_HEALTH - 4);
        assert!(patient.effects.is_empty());

        let mut dying = player();
        dying.health = 2;
        dying.effects.give(EffectKind::Poison, 4, 400);
        for _ in 0..400 {
            dying.invulnerable_ticks = 0;
            dying.tick_effects();
        }
        assert_eq!(dying.health, 1, "the poison leaves the last point");

        dying.effects.give(EffectKind::Burn, 1, BURN_PERIOD);
        for _ in 0..BURN_PERIOD {
            dying.tick_effects();
        }
        assert!(dying.is_dead());
        assert!(dying.effects.is_empty());
    }

    #[test]
    fn regeneration_heals_slowness_slows_and_weakness_weakens() {
        let mut patient = player();
        patient.health = 10;
        patient.give_effect(EffectKind::Regeneration, 2, 5);
        patient.give_effect(EffectKind::Slowness, 2, 5);
        patient.give_effect(EffectKind::Weakness, 1, 5);
        patient.tick_effects();
        assert_eq!(patient.slowness, 2);
        assert_eq!(patient.effects.weakened(6), 4);
        assert_eq!(patient.effects.weakened(1), 1);
        for _ in 1..5 * TICKS_PER_SECOND as u32 {
            patient.tick_effects();
        }
        // Every 25 ticks for 100 ticks
        assert_eq!(patient.health, 14);
        assert!(StatusEffects::speed_factor(2) < 1.0);
        assert_eq!(patient.slowness, 0);
        assert!(patient.effects.is_empty());
    }

    #[test]
    fn creative_players_are_given_nothing() {
        let mut builder = player();
        builder.game_mode = crate::players::game_mode::GameMode::Creative;
        builder.give_effect(EffectKind::Poison, 1, 10);
        assert!(builder.effects.is_empty());
    }
}
