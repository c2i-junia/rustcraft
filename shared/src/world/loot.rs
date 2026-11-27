//! What a creature drops when it dies, by its kind: a table, so that the server rolls it and the
//! Codex and the wiki show it from the same lines.

use rand::Rng;

use super::{ItemId, MobKind};

/// How many of an item a line of loot gives.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LootCount {
    /// From `min` to `max`, each as likely.
    Range(u32, u32),
    /// One, `n` times in `d`.
    Odds(u32, u32),
    /// One, with this chance.
    Chance(f64),
}

/// When a line of loot is rolled at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LootWhen {
    Always,
    /// Unless the sheep was sheared.
    NotSheared,
    /// If the mount wore a saddle.
    Saddled,
    /// If the line before gave nothing.
    Otherwise,
}

/// A line of loot.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Loot {
    pub item: ItemId,
    pub count: LootCount,
    pub when: LootWhen,
}

const fn line(item: ItemId, count: LootCount) -> Loot {
    Loot {
        item,
        count,
        when: LootWhen::Always,
    }
}

const fn range(item: ItemId, min: u32, max: u32) -> Loot {
    line(item, LootCount::Range(min, max))
}

const fn odds(item: ItemId, n: u32, d: u32) -> Loot {
    line(item, LootCount::Odds(n, d))
}

/// A table of loot, made at compile time.
macro_rules! table {
    ($($line:expr),* $(,)?) => {{
        const TABLE: &[Loot] = &[$($line),*];
        TABLE
    }};
}

impl MobKind {
    /// What a grown creature of this kind drops: its meat, and its wool, leather or feathers. A
    /// young animal drops nothing.
    pub fn loot(self) -> &'static [Loot] {
        match self {
            MobKind::Sheep => table![
                range(ItemId::RawMutton, 1, 2),
                Loot {
                    item: ItemId::Wool,
                    count: LootCount::Range(1, 1),
                    when: LootWhen::NotSheared,
                },
            ],
            MobKind::Pig => table![range(ItemId::RawPork, 1, 3)],
            MobKind::Cow => table![range(ItemId::RawBeef, 1, 3), range(ItemId::Leather, 0, 2)],
            MobKind::Chicken => table![
                range(ItemId::RawChicken, 1, 1),
                range(ItemId::Feather, 0, 2)
            ],
            MobKind::Rabbit => table![range(ItemId::RawRabbit, 1, 1)],
            MobKind::Bramble => table![range(ItemId::Stick, 1, 3)],
            MobKind::Stumpling => table![
                range(ItemId::Stick, 2, 4),
                line(ItemId::Charcoal, LootCount::Chance(0.3)),
            ],
            MobKind::Roller => table![range(ItemId::Cobblestone, 1, 2), odds(ItemId::Flint, 1, 5)],
            MobKind::GustOwl => table![range(ItemId::Feather, 1, 3)],
            MobKind::Cinderling => table![range(ItemId::Coal, 1, 2), odds(ItemId::Flint, 1, 10)],
            MobKind::EmberMother => table![range(ItemId::EmberHeart, 1, 1)],
            // The fish of the seas give those of the rods (`world::fishing`)
            MobKind::Glintfin => table![
                odds(ItemId::RawGlowfin, 1, 4),
                Loot {
                    item: ItemId::RawRoach,
                    count: LootCount::Range(1, 1),
                    when: LootWhen::Otherwise,
                },
            ],
            MobKind::Shoalback => table![range(ItemId::RawPerch, 1, 1)],
            MobKind::Mossback => table![range(ItemId::Moss, 1, 2), odds(ItemId::Leather, 1, 3)],
            MobKind::Shardling => table![range(ItemId::CrystalShard, 1, 3)],
            MobKind::Slagcrawler => table![range(ItemId::Coal, 0, 2), odds(ItemId::Magma, 1, 4)],
            // It breaks into the item it was put down from
            MobKind::Boat => table![range(ItemId::Boat, 1, 1)],
            MobKind::Horse | MobKind::Donkey | MobKind::Mule => table![
                range(ItemId::Leather, 0, 2),
                Loot {
                    item: ItemId::Saddle,
                    count: LootCount::Range(1, 1),
                    when: LootWhen::Saddled,
                },
            ],
            // A part of the creature, which distils into strength (`alchemy`)
            MobKind::Gnawer => table![odds(ItemId::GnawerFang, 1, 2)],
            MobKind::Fox
            | MobKind::Wolf
            | MobKind::Goat
            | MobKind::Cat
            | MobKind::Drowner
            | MobKind::ThrownPotion
            | MobKind::Ember
            | MobKind::Arrow
            | MobKind::Bobber
            | MobKind::Farmer
            | MobKind::Smith
            | MobKind::Cartographer => &[],
        }
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
