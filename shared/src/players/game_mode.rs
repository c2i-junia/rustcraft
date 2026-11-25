//! The game modes, and the health of a player in survival.

use serde::{Deserialize, Serialize};

use crate::world::MobKind;

/// What a player may do. Append only: it travels in `PlayerUpdateEvent`.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GameMode {
    /// The player has health, can die and cannot fly.
    #[default]
    Survival,
    /// The player takes no damage and can fly.
    Creative,
}

impl GameMode {
    /// Every mode, in the order of the enum.
    pub const ALL: [GameMode; 2] = [GameMode::Survival, GameMode::Creative];

    /// The name the `/gamemode` command takes.
    pub fn name(self) -> &'static str {
        match self {
            GameMode::Survival => "survival",
            GameMode::Creative => "creative",
        }
    }

    pub fn from_name(name: &str) -> Option<GameMode> {
        Self::ALL
            .into_iter()
            .find(|mode| mode.name().eq_ignore_ascii_case(name))
    }

    pub fn can_fly(self) -> bool {
        self == GameMode::Creative
    }

    pub fn takes_damage(self) -> bool {
        self == GameMode::Survival
    }

    /// Whether items come out of the item browser's catalog.
    pub fn is_creative(self) -> bool {
        self == GameMode::Creative
    }
}

/// Health of a player in full health: 10 hearts of 2 points.
pub const MAX_HEALTH: u32 = 20;

/// A fall of up to this many blocks does no harm.
pub const SAFE_FALL_DISTANCE: f32 = 3.0;

/// Ticks between two points of health a living player gets back by itself: 4 seconds.
pub const REGENERATION_TICKS: u64 = 80;

/// The points of health a fall of `distance` blocks takes: one per block beyond
/// `SAFE_FALL_DISTANCE`, so that a fall of 23 blocks kills a player in full health.
pub fn fall_damage(distance: f32) -> u32 {
    (distance - SAFE_FALL_DISTANCE).round().max(0.0) as u32
}

/// Ticks after a hit during which a player ignores the next ones: half a second.
pub const INVULNERABILITY_TICKS: u32 = 10;

/// Ticks after a hit by a mob during which a deadly fall is put on the mob: 3 seconds.
pub const THROWN_OFF_TICKS: u32 = 60;

/// What hurts a player. It is neither sent nor saved: the death cause it leads to is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DamageSource {
    /// It landed after a fall of more than `SAFE_FALL_DISTANCE` blocks.
    Fall,
    /// It fell below the bottom of the world.
    Void,
    /// It ran out of air with its eyes in a liquid.
    Drowning,
    /// It is burning.
    Fire,
    /// It is in lava.
    Lava,
    /// It has no food left.
    Starvation,
    /// A mob of this kind hit it.
    Mob(MobKind),
}

impl DamageSource {
    /// Whether the source hurts a player who was just hit: the void and hunger do not wait.
    pub fn pierces_invulnerability(self) -> bool {
        matches!(self, DamageSource::Void | DamageSource::Starvation)
    }

    /// Whether armour absorbs a share of it: the blows and the throws of the mobs, fire and
    /// lava. Not what comes from within or leaves nothing to stop, drowning, hunger and the
    /// void, nor a fall: metal does not soften a landing.
    pub fn is_absorbed_by_armour(self) -> bool {
        matches!(
            self,
            DamageSource::Mob(_) | DamageSource::Fire | DamageSource::Lava
        )
    }
}

/// Why a player died, for the death screen. Append only: it travels in `PlayerUpdateEvent`.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeathCause {
    Fall,
    /// It fell below the bottom of the world.
    Void,
    Drowning,
    Fire,
    Lava,
    Starvation,
    /// A mob of this kind hit it last.
    Mob(MobKind),
    /// It fell to its death within `THROWN_OFF_TICKS` of a hit by a mob of this kind.
    ThrownOff(MobKind),
}

impl DeathCause {
    /// The cause of a death by `source`, `thrown_by` the mob that hit the player within the
    /// last `THROWN_OFF_TICKS`, if any.
    pub fn of(source: DamageSource, thrown_by: Option<MobKind>) -> DeathCause {
        match (source, thrown_by) {
            (DamageSource::Fall, Some(kind)) => DeathCause::ThrownOff(kind),
            (DamageSource::Fall, None) => DeathCause::Fall,
            (DamageSource::Void, _) => DeathCause::Void,
            (DamageSource::Drowning, _) => DeathCause::Drowning,
            (DamageSource::Fire, _) => DeathCause::Fire,
            (DamageSource::Lava, _) => DeathCause::Lava,
            (DamageSource::Starvation, _) => DeathCause::Starvation,
            (DamageSource::Mob(kind), _) => DeathCause::Mob(kind),
        }
    }

    /// The line of the death screen.
    pub fn message(self) -> String {
        match self {
            DeathCause::Fall => "You hit the ground too hard".into(),
            DeathCause::Void => "You fell out of the world".into(),
            DeathCause::Drowning => "You ran out of air".into(),
            DeathCause::Fire => "You went up in flames".into(),
            DeathCause::Lava => "You tried to swim in lava".into(),
            DeathCause::Starvation => "You starved to death".into(),
            DeathCause::Mob(kind) => format!("A {} got you", kind.display_name()),
            DeathCause::ThrownOff(kind) => {
                format!("A {} threw you off a ledge", kind.display_name())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::MobKind;

    #[test]
    fn a_fall_hurts_one_point_per_block_beyond_three() {
        assert_eq!(fall_damage(0.0), 0);
        assert_eq!(fall_damage(1.3), 0);
        assert_eq!(fall_damage(3.0), 0);
        assert_eq!(fall_damage(3.4), 0);
        // A drop of 4 blocks is measured a little over 4
        assert_eq!(fall_damage(4.001), 1);
        assert_eq!(fall_damage(10.0), 7);
        assert_eq!(
            fall_damage(SAFE_FALL_DISTANCE + MAX_HEALTH as f32),
            MAX_HEALTH
        );
    }

    #[test]
    fn modes_are_found_by_their_names() {
        for mode in GameMode::ALL {
            assert_eq!(GameMode::from_name(mode.name()), Some(mode));
        }
        assert_eq!(GameMode::from_name("CREATIVE"), Some(GameMode::Creative));
        assert_eq!(GameMode::from_name("adventure"), None);
        assert!(GameMode::Creative.can_fly() && !GameMode::Survival.can_fly());
    }

    #[test]
    fn each_source_kills_with_its_own_cause() {
        for (source, cause) in [
            (DamageSource::Fall, DeathCause::Fall),
            (DamageSource::Void, DeathCause::Void),
            (DamageSource::Drowning, DeathCause::Drowning),
            (DamageSource::Fire, DeathCause::Fire),
            (DamageSource::Lava, DeathCause::Lava),
            (DamageSource::Starvation, DeathCause::Starvation),
            (
                DamageSource::Mob(MobKind::Wolf),
                DeathCause::Mob(MobKind::Wolf),
            ),
        ] {
            assert_eq!(DeathCause::of(source, None), cause, "{source:?}");
        }
    }

    #[test]
    fn a_fall_after_a_mob_hit_is_put_on_the_mob() {
        assert_eq!(
            DeathCause::of(DamageSource::Fall, Some(MobKind::Goat)),
            DeathCause::ThrownOff(MobKind::Goat)
        );
        // The mob only takes the fall
        assert_eq!(
            DeathCause::of(DamageSource::Void, Some(MobKind::Goat)),
            DeathCause::Void
        );
        assert_eq!(
            DeathCause::of(DamageSource::Mob(MobKind::Fox), Some(MobKind::Goat)),
            DeathCause::Mob(MobKind::Fox)
        );
    }

    #[test]
    fn only_the_void_and_starvation_pierce_the_invulnerability() {
        assert!(DamageSource::Void.pierces_invulnerability());
        assert!(DamageSource::Starvation.pierces_invulnerability());
        for source in [
            DamageSource::Fall,
            DamageSource::Drowning,
            DamageSource::Fire,
            DamageSource::Lava,
            DamageSource::Mob(MobKind::Fox),
        ] {
            assert!(!source.pierces_invulnerability(), "{source:?}");
        }
    }

    #[test]
    fn the_death_messages_name_the_mob() {
        assert_eq!(DeathCause::Fall.message(), "You hit the ground too hard");
        assert_eq!(DeathCause::Drowning.message(), "You ran out of air");
        assert_eq!(DeathCause::Fire.message(), "You went up in flames");
        assert_eq!(DeathCause::Lava.message(), "You tried to swim in lava");
        assert_eq!(DeathCause::Starvation.message(), "You starved to death");
        assert_eq!(DeathCause::Mob(MobKind::Fox).message(), "A Fox got you");
        assert_eq!(
            DeathCause::ThrownOff(MobKind::Goat).message(),
            "A Goat threw you off a ledge"
        );
        assert_eq!(
            DeathCause::ThrownOff(MobKind::GustOwl).message(),
            "A Gust Owl threw you off a ledge"
        );
    }
}
