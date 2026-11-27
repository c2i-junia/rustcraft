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

/// The fish, by weight: the roach is common, the glowfin rare, and it bites at night only.
const FISH: [(ItemId, u32); 5] = [
    (ItemId::RawRoach, 40),
    (ItemId::RawPerch, 28),
    (ItemId::RawPike, 15),
    (ItemId::RawEel, 12),
    (ItemId::RawGlowfin, 5),
];
/// What the water holds that is not a fish, and how many of it.
const JUNK: [(ItemId, u32); 5] = [
    (ItemId::Stick, 2),
    (ItemId::Leather, 1),
    (ItemId::Rope, 1),
    (ItemId::Bowl, 1),
    (ItemId::Fiber, 3),
];
const TREASURE: [(ItemId, u32); 5] = [
    (ItemId::Coin, 4),
    (ItemId::GoldIngot, 1),
    (ItemId::Saddle, 1),
    (ItemId::Bow, 1),
    (ItemId::Diamond, 1),
];

/// What a catch is made of, drawn from `rng`: 84 % fish, 11 % junk, 5 % treasure.
pub fn catch_kind(rng: &mut impl Rng) -> CatchKind {
    match rng.gen_range(0..100) {
        0..84 => CatchKind::Fish,
        84..95 => CatchKind::Junk,
        _ => CatchKind::Treasure,
    }
}

/// What a fish that bites is: by night every fish bites, by day the glowfin hides.
pub fn fish(night: bool, rng: &mut impl Rng) -> ItemId {
    let biting: Vec<_> = FISH
        .iter()
        .filter(|(fish, _)| night || *fish != ItemId::RawGlowfin)
        .collect();
    let total: u32 = biting.iter().map(|(_, weight)| weight).sum();
    let mut roll = rng.gen_range(0..total);
    for &&(fish, weight) in &biting {
        if roll < weight {
            return fish;
        }
        roll -= weight;
    }
    ItemId::RawRoach
}

/// What the line brings back when drawn in during a bite, and how many.
pub fn catch(night: bool, rng: &mut impl Rng) -> (ItemId, u32) {
    match catch_kind(rng) {
        CatchKind::Fish => (fish(night, rng), 1),
        CatchKind::Junk => JUNK[rng.gen_range(0..JUNK.len())],
        CatchKind::Treasure => TREASURE[rng.gen_range(0..TREASURE.len())],
    }
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
        for (raw, _) in FISH {
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
