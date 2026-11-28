//! What a creature drops when it dies, by its kind: a table, the `drops` of its kind in
//! `mobs.ron`, so that the server rolls it and the Codex and the wiki show it from the same lines.

use rand::Rng;
use serde::{Deserialize, Serialize};

use super::{ItemId, MobKind};

/// How many of an item a line of loot gives.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum LootCount {
    /// From `min` to `max`, each as likely.
    Range(u32, u32),
    /// One, `n` times in `d`.
    Odds(u32, u32),
    /// One, with this chance.
    Chance(f64),
}

/// When a line of loot is rolled at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum LootWhen {
    #[default]
    Always,
    /// Unless the sheep was sheared.
    NotSheared,
    /// If the mount wore a saddle.
    Saddled,
    /// If the line before gave nothing.
    Otherwise,
}

/// A line of loot.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Loot {
    pub item: ItemId,
    pub count: LootCount,
    pub when: LootWhen,
}

impl MobKind {
    /// What a grown creature of this kind drops: its meat, and its wool, leather or feathers. A
    /// young animal drops nothing.
    pub fn loot(self) -> &'static [Loot] {
        &self.props().drops
    }
}

/// What the state of a creature changes in its loot.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LootState {
    pub sheared: bool,
    pub saddled: bool,
}

/// Rolls `loot`: the items and their counts, those of none left out.
pub fn roll_loot(loot: &[Loot], state: LootState, rng: &mut impl Rng) -> Vec<(ItemId, u32)> {
    let mut drops = Vec::new();
    let mut last = 0;
    for line in loot {
        let rolled = match line.when {
            LootWhen::Always => true,
            LootWhen::NotSheared => !state.sheared,
            LootWhen::Saddled => state.saddled,
            LootWhen::Otherwise => last == 0,
        };
        last = if rolled { roll(line.count, rng) } else { 0 };
        if last > 0 {
            drops.push((line.item, last));
        }
    }
    drops
}

fn roll(count: LootCount, rng: &mut impl Rng) -> u32 {
    match count {
        LootCount::Range(min, max) if min == max => min,
        LootCount::Range(min, max) => rng.gen_range(min..=max),
        LootCount::Odds(n, d) => u32::from(rng.gen_ratio(n, d)),
        LootCount::Chance(p) => u32::from(rng.gen_bool(p)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::{rngs::StdRng, SeedableRng};

    #[test]
    fn a_sheared_sheep_keeps_its_wool_and_an_unsaddled_horse_has_no_saddle() {
        let mut rng = StdRng::seed_from_u64(3);
        let sheared = LootState {
            sheared: true,
            ..default()
        };
        let drops = roll_loot(MobKind::Sheep.loot(), sheared, &mut rng);
        assert!(drops.iter().all(|(item, _)| *item != ItemId::Wool));
        let drops = roll_loot(MobKind::Sheep.loot(), default(), &mut rng);
        assert!(drops.contains(&(ItemId::Wool, 1)));
        for _ in 0..50 {
            let drops = roll_loot(MobKind::Horse.loot(), default(), &mut rng);
            assert!(drops.iter().all(|(item, _)| *item != ItemId::Saddle));
        }
    }

    #[test]
    fn a_glintfin_gives_one_fish_or_the_other() {
        let mut rng = StdRng::seed_from_u64(5);
        let (mut glowfins, mut roaches) = (0, 0);
        for _ in 0..400 {
            let drops = roll_loot(MobKind::Glintfin.loot(), default(), &mut rng);
            assert_eq!(drops.len(), 1);
            match drops[0].0 {
                ItemId::RawGlowfin => glowfins += 1,
                ItemId::RawRoach => roaches += 1,
                other => panic!("{other:?}"),
            }
        }
        assert!(glowfins > 50 && roaches > 200, "{glowfins} {roaches}");
    }

    fn default() -> LootState {
        LootState::default()
    }
}
