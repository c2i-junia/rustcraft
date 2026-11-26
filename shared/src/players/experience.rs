//! The experience of the players: the points they gather fill levels, which they spend. Only the
//! server changes it.

use serde::{Deserialize, Serialize};

use super::Player;
use crate::world::{BlockId, ItemId, MobKind};

/// How near a living player draws an orb to itself, in blocks.
pub const ORB_REACH: f32 = 6.0;
/// How fast an orb flies to a player, in blocks/s: from afar, then added as it nears.
pub const ORB_SPEED: (f32, f32) = (3.0, 7.0);
/// Two orbs closer than this merge, in the air as on the ground.
pub const ORB_MERGE_RADIUS: f32 = 1.0;
/// Most points the orb of a dead player is worth.
pub const DEATH_ORB_CAP: u32 = 400;

/// The points it takes to go from `level` to the next: 3 more at each level.
pub fn level_cost(level: u32) -> u32 {
    8 + level.saturating_mul(3)
}

/// The levels of a player, and the points it has toward the next.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[serde(default)]
pub struct Experience {
    pub level: u32,
    /// Below `level_cost(level)`.
    pub points: u32,
}

impl Experience {
    /// Adds `points`, going up the levels they fill. A level costs more than the one before, so
    /// that it takes few steps whatever `points` is.
    pub fn gain(&mut self, points: u32) {
        self.points = self.points.saturating_add(points);
        while self.points >= level_cost(self.level) {
            self.points -= level_cost(self.level);
            self.level += 1;
        }
    }

    /// The share of the next level already filled, below 1.
    pub fn progress(&self) -> f32 {
        self.points as f32 / level_cost(self.level) as f32
    }

    /// All the points gathered, those of the levels included.
    pub fn total(&self) -> u32 {
        let level = u64::from(self.level);
        // The sum of the costs of the levels below
        let levels = 8 * level + 3 * level * level.saturating_sub(1) / 2;
        u32::try_from(levels + u64::from(self.points)).unwrap_or(u32::MAX)
    }

    pub fn has_levels(&self, levels: u32) -> bool {
        self.level >= levels
    }

    /// Takes `levels` levels, if there are as many, and returns whether it did. The share of the
    /// next level already filled stays the same.
    pub fn spend_levels(&mut self, levels: u32) -> bool {
        if !self.has_levels(levels) {
            return false;
        }
        let cost = level_cost(self.level);
        self.level -= levels;
        self.points = self.points * level_cost(self.level) / cost;
        true
    }

    /// Takes everything from a dead player, and returns what the orb it leaves is worth: half,
    /// up to `DEATH_ORB_CAP`.
    pub fn lose_at_death(&mut self) -> u32 {
        let total = std::mem::take(self).total();
        (total / 2).min(DEATH_ORB_CAP)
    }
}

impl Player {
    pub fn has_levels(&self, levels: u32) -> bool {
        self.experience.has_levels(levels)
    }

    /// Takes `levels` levels of experience, if the player has them: see
    /// `Experience::spend_levels`.
    pub fn spend_levels(&mut self, levels: u32) -> bool {
        self.experience.spend_levels(levels)
    }
}

impl MobKind {
    /// The points of the orb of a mob of this kind killed by a player.
    pub fn experience(self) -> u32 {
        match self {
            MobKind::Gnawer => 3,
            MobKind::Bramble => 4,
            MobKind::Roller | MobKind::Stumpling => 5,
            MobKind::Drowner | MobKind::Cinderling | MobKind::GustOwl => 6,
            MobKind::EmberMother => 500,
            _ => 0,
        }
    }
}

impl BlockId {
    /// The points of the orb of an ore mined with a tool that gets its resource. The ores that
    /// drop themselves give none: placed and mined again they would give some for ever, so
    /// theirs comes out of the furnace.
    pub fn experience(self) -> u32 {
        match self {
            BlockId::CoalOre => 1,
            BlockId::DiamondOre => 5,
            _ => 0,
        }
    }
}

/// The hundredths of a point of experience each `result` taken out of a furnace gives.
pub fn smelting_experience(result: ItemId) -> u32 {
    match result {
        ItemId::Glass | ItemId::Stone | ItemId::Gloomrock => 10,
        ItemId::Charcoal => 15,
        ItemId::FiredBrick => 30,
        ItemId::CopperIngot | ItemId::TinIngot => 50,
        ItemId::IronIngot => 70,
        ItemId::GoldIngot | ItemId::Diamond => 100,
        ItemId::RoastMushroom | ItemId::BakedPotato => 25,
        ItemId::GreenDye => 20,
        ItemId::RoastMutton
        | ItemId::RoastPork
        | ItemId::RoastBeef
        | ItemId::RoastChicken
        | ItemId::RoastRabbit
        | ItemId::GrilledRoach
        | ItemId::GrilledPerch
        | ItemId::GrilledPike
        | ItemId::GrilledEel
        | ItemId::GrilledGlowfin => 35,
        ItemId::FriedEgg => 25,
        ItemId::CocoaCake | ItemId::ApplePie => 50,
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::furnace::smelting_result;

    #[test]
    fn levels_cost_more_and_more_and_are_spent_only_when_there_are_enough() {
        let mut experience = Experience::default();
        experience.gain(7);
        assert_eq!((experience.level, experience.points), (0, 7));
        experience.gain(1 + 11 + 7);
        assert_eq!((experience.level, experience.points), (2, 7));
        assert_eq!(experience.total(), 26);
        assert!(!experience.spend_levels(3));
        assert!(experience.has_levels(2) && experience.spend_levels(2));
        assert_eq!((experience.level, experience.points), (0, 4));

        // Dying leaves half, capped, and takes everything once
        experience.gain(16);
        assert_eq!(experience.lose_at_death(), 10);
        assert_eq!(experience.lose_at_death(), 0);
        experience.gain(u32::MAX);
        assert_eq!(experience.lose_at_death(), DEATH_ORB_CAP);
    }

    #[test]
    fn everything_smelted_gives_some_experience() {
        for item in ItemId::ALL {
            if let Some(result) = smelting_result(item) {
                assert!(smelting_experience(result) > 0, "{item:?} -> {result:?}");
            }
        }
    }
}
