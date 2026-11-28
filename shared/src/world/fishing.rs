//! Fishing: a rod casts a float (`MobKind::Bobber`, flown and watched by the server in
//! `server::mob::fishing`) into the water, where after a random wait a fish bites: the float
//! plunges for a moment (`MobAction::Attack`), and the player who draws the rod in then catches
//! what `catch` draws. Drawn in too late or too soon, the line comes back empty.

use rand::Rng;

use crate::messages::PlayerId;
use crate::world::ItemId;
use crate::TICKS_PER_SECOND;

/// The shortest and the longest a float waits in the water for a bite, in ticks: 5 to 25 s.
pub const BITE_WAIT_TICKS: std::ops::Range<u32> =
    5 * TICKS_PER_SECOND as u32..25 * TICKS_PER_SECOND as u32;
/// How long a bite lasts, in ticks: the time to draw the rod in, under a second.
pub const BITE_TICKS: std::ops::Range<u32> = 14..22;
/// How far below the surface a float plunges when a fish bites, in blocks.
pub const BITE_DIP: f32 = 0.25;
/// How fast a float leaves the rod, in blocks per second.
pub const CAST_SPEED: f32 = 11.0;
/// How long the line is at most: a float farther than this from its angler is drawn in.
pub const LINE_LENGTH: f32 = 32.0;
/// The wear of a rod each catch adds, and that of a float drawn out of the ground.
pub const CATCH_WEAR: u32 = 1;
pub const SNAG_WEAR: u32 = 2;

/// What the server knows of a float, beside its place: whose rod it hangs from, and whether it
/// floats on water, where the fish bite, rather than lying on the ground.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bobber {
    pub angler: PlayerId,
    pub in_water: bool,
    /// Whether it is night where it floats, which brings the glowfin.
    pub night: bool,
}

/// What a catch is made of: mostly fish, now and then rubbish, rarely a treasure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CatchKind {
    Fish,
    Junk,
    Treasure,
}

/// A line of the pool of a kind of catch: the item, how many of it, its weight among the others,
/// and whether it bites only at night.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Catch {
    pub item: ItemId,
    pub count: u32,
    pub weight: u32,
    pub night_only: bool,
}

const fn line(item: ItemId, count: u32, weight: u32) -> Catch {
    Catch {
        item,
        count,
        weight,
        night_only: false,
    }
}

/// The fish, by weight: the roach is common, the glowfin rare, and it bites at night only.
const FISH: [Catch; 5] = [
    line(ItemId::RawRoach, 1, 40),
    line(ItemId::RawPerch, 1, 28),
    line(ItemId::RawPike, 1, 15),
    line(ItemId::RawEel, 1, 12),
    Catch {
        night_only: true,
        ..line(ItemId::RawGlowfin, 1, 5)
    },
];
/// What the water holds that is not a fish, and how many of it, each as likely.
const JUNK: [Catch; 5] = [
    line(ItemId::Stick, 2, 1),
    line(ItemId::Leather, 1, 1),
    line(ItemId::Rope, 1, 1),
    line(ItemId::Bowl, 1, 1),
    line(ItemId::Fiber, 3, 1),
];
const TREASURE: [Catch; 5] = [
    line(ItemId::Coin, 4, 1),
    line(ItemId::GoldIngot, 1, 1),
    line(ItemId::Saddle, 1, 1),
    line(ItemId::Bow, 1, 1),
    line(ItemId::Diamond, 1, 1),
];

impl CatchKind {
    pub const ALL: [CatchKind; 3] = [CatchKind::Fish, CatchKind::Junk, CatchKind::Treasure];

    /// Its share of the catches, in percent.
    pub fn percent(self) -> u32 {
        match self {
            CatchKind::Fish => 84,
            CatchKind::Junk => 11,
            CatchKind::Treasure => 5,
        }
    }

    /// What a catch of this kind may be.
    pub fn pool(self) -> &'static [Catch] {
        match self {
            CatchKind::Fish => &FISH,
            CatchKind::Junk => &JUNK,
            CatchKind::Treasure => &TREASURE,
        }
    }
}

/// What a catch is made of, drawn from `rng`, each kind as often as its `percent`.
pub fn catch_kind(rng: &mut impl Rng) -> CatchKind {
    let mut roll = rng.gen_range(0..100);
    for kind in CatchKind::ALL {
        if roll < kind.percent() {
            return kind;
        }
        roll -= kind.percent();
    }
    CatchKind::Treasure
}

/// A line of `pool` by weight, among those that bite at this time of day.
fn draw(pool: &'static [Catch], night: bool, rng: &mut impl Rng) -> Catch {
    let biting: Vec<_> = pool.iter().filter(|c| night || !c.night_only).collect();
    let total: u32 = biting.iter().map(|c| c.weight).sum();
    let mut roll = rng.gen_range(0..total);
    for &&c in &biting {
        if roll < c.weight {
            return c;
        }
        roll -= c.weight;
    }
    pool[0]
}

/// What a fish that bites is: by night every fish bites, by day the glowfin hides.
pub fn fish(night: bool, rng: &mut impl Rng) -> ItemId {
    draw(&FISH, night, rng).item
}

/// What the line brings back when drawn in during a bite, and how many.
pub fn catch(night: bool, rng: &mut impl Rng) -> (ItemId, u32) {
    let line = draw(catch_kind(rng).pool(), night, rng);
    (line.item, line.count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::{rngs::StdRng, SeedableRng};

    #[test]
    fn most_catches_are_fish_some_rubbish_and_a_few_treasures() {
        let mut rng = StdRng::seed_from_u64(3);
        let mut counts = [0; 3];
        for _ in 0..10_000 {
            counts[catch_kind(&mut rng) as usize] += 1;
        }
        assert!((8_100..8_700).contains(&counts[0]), "{counts:?}");
        assert!((900..1_300).contains(&counts[1]), "{counts:?}");
        assert!((350..650).contains(&counts[2]), "{counts:?}");
    }

    #[test]
    fn every_fish_is_eaten_raw_and_grilled_and_the_glowfin_bites_at_night_only() {
        let mut rng = StdRng::seed_from_u64(5);
        let by_day: std::collections::HashSet<_> =
            (0..2_000).map(|_| fish(false, &mut rng)).collect();
        let by_night: std::collections::HashSet<_> =
            (0..2_000).map(|_| fish(true, &mut rng)).collect();
        assert_eq!(by_day.len(), 4);
        assert!(!by_day.contains(&ItemId::RawGlowfin));
        assert_eq!(by_night.len(), 5);
        for Catch { item: raw, .. } in FISH {
            let grilled = crate::world::furnace::smelting_result(raw).unwrap();
            let (raw_food, _) = raw.food_value().unwrap();
            let (grilled_food, _) = grilled.food_value().unwrap();
            assert!(grilled_food > raw_food, "{raw:?}");
        }
    }

    #[test]
    fn the_wait_for_a_bite_is_longer_than_the_bite() {
        const { assert!(BITE_TICKS.end < BITE_WAIT_TICKS.start) };
    }
}
