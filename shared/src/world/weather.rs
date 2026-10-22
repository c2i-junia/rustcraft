//! The weather and its cycle. The server holds the weather and changes it on its own; the
//! clients only hear its kind, and ease their rain and thunder towards it.

use std::ops::RangeInclusive;

use rand::Rng;
use serde::{Deserialize, Serialize};

/// The weather of a world. Append only: it travels in messages and is saved in `world.ron`.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WeatherKind {
    #[default]
    Clear,
    Rain,
    /// Rain with lightning and thunder.
    Thunder,
}

impl WeatherKind {
    pub const ALL: [WeatherKind; 3] = [WeatherKind::Clear, WeatherKind::Rain, WeatherKind::Thunder];

    /// The name `/weather` takes.
    pub fn name(self) -> &'static str {
        match self {
            WeatherKind::Clear => "clear",
            WeatherKind::Rain => "rain",
            WeatherKind::Thunder => "thunder",
        }
    }

    pub fn from_name(name: &str) -> Option<WeatherKind> {
        Self::ALL.into_iter().find(|kind| kind.name() == name)
    }
}

/// How long a clear spell lasts, in ticks: 10 to 100 minutes, half a day to five days.
pub const CLEAR_TICKS: RangeInclusive<u32> = 12_000..=120_000;
/// How long rain or a thunderstorm lasts, in ticks: 5 to 15 minutes.
pub const STORM_TICKS: RangeInclusive<u32> = 6_000..=18_000;

/// How far a lightning strike is seen and heard, in blocks along the ground: the thunder is
/// not heard beyond it, the flash fades out at it, and the server tells a strike only to the
/// players within it.
pub const LIGHTNING_REACH: f32 = 1_500.;

/// How long a spell of `kind` lasts, in ticks, drawn uniformly.
pub fn spell_ticks(kind: WeatherKind, rng: &mut impl Rng) -> u32 {
    match kind {
        WeatherKind::Clear => rng.gen_range(CLEAR_TICKS),
        WeatherKind::Rain | WeatherKind::Thunder => rng.gen_range(STORM_TICKS),
    }
}

/// The weather that follows `kind`: rain (2 in 3) or a thunderstorm (1 in 3) after a clear
/// spell, a clear spell after either.
pub fn next_kind(kind: WeatherKind, rng: &mut impl Rng) -> WeatherKind {
    match kind {
        WeatherKind::Clear if rng.gen_ratio(1, 3) => WeatherKind::Thunder,
        WeatherKind::Clear => WeatherKind::Rain,
        WeatherKind::Rain | WeatherKind::Thunder => WeatherKind::Clear,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::rngs::StdRng;
    use rand::SeedableRng;

    #[test]
    fn a_weather_is_named_and_found_by_its_name() {
        for kind in WeatherKind::ALL {
            assert_eq!(WeatherKind::from_name(kind.name()), Some(kind));
        }
        assert_eq!(WeatherKind::Thunder.name(), "thunder");
        assert_eq!(WeatherKind::from_name("snow"), None);
        assert_eq!(WeatherKind::default(), WeatherKind::Clear);
    }

    #[test]
    fn a_spell_lasts_within_the_range_of_its_kind_and_spreads_over_it() {
        let mut rng = StdRng::seed_from_u64(1);
        for (kind, range) in [
            (WeatherKind::Clear, CLEAR_TICKS),
            (WeatherKind::Rain, STORM_TICKS),
            (WeatherKind::Thunder, STORM_TICKS),
        ] {
            let spells: Vec<u32> = (0..1000).map(|_| spell_ticks(kind, &mut rng)).collect();
            assert!(spells.iter().all(|ticks| range.contains(ticks)), "{kind:?}");
            let span = range.end() - range.start();
            let (low, high) = (range.start() + span / 10, range.end() - span / 10);
            assert!(spells.iter().any(|&ticks| ticks < low), "{kind:?}");
            assert!(spells.iter().any(|&ticks| ticks > high), "{kind:?}");
        }
        assert_eq!(CLEAR_TICKS, 12_000..=120_000);
        assert_eq!(STORM_TICKS, 6_000..=18_000);
    }

    #[test]
    fn rain_comes_two_clear_spells_in_three_and_a_thunderstorm_one_in_three() {
        let mut rng = StdRng::seed_from_u64(1);
        let draws = 3000;
        let thunder = (0..draws)
            .map(|_| next_kind(WeatherKind::Clear, &mut rng))
            .inspect(|kind| assert_ne!(*kind, WeatherKind::Clear))
            .filter(|kind| *kind == WeatherKind::Thunder)
            .count();
        assert!((900..1100).contains(&thunder), "{thunder} of {draws}");
    }

    #[test]
    fn rain_and_thunderstorms_clear_up() {
        let mut rng = StdRng::seed_from_u64(1);
        for kind in [WeatherKind::Rain, WeatherKind::Thunder] {
            for _ in 0..100 {
                assert_eq!(next_kind(kind, &mut rng), WeatherKind::Clear);
            }
        }
    }
}
