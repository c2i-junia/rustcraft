//! The game modes, and the health of a player in survival.

use serde::{Deserialize, Serialize};

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

/// Why a player died, for the death screen.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeathCause {
    Fall,
    /// It fell below the bottom of the world.
    Void,
}

impl DeathCause {
    pub fn message(self) -> &'static str {
        match self {
            DeathCause::Fall => "You hit the ground too hard",
            DeathCause::Void => "You fell out of the world",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
