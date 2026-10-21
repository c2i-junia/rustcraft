//! The weather of the world, held by the server: it counts down and changes on its own, and
//! `/weather` sets it. The clients only hear its kind.

use bevy::prelude::*;
use rand::Rng;
use shared::world::{next_kind, spell_ticks, WeatherKind};

use crate::world::format::SavedWeather;

/// The weather and the ticks it has left before the next one.
#[derive(Resource, Debug, Clone, PartialEq)]
pub struct ServerWeather {
    pub kind: WeatherKind,
    pub ticks_left: u32,
}

impl ServerWeather {
    /// The weather of a new world: clear, for a clear spell.
    pub fn new(rng: &mut impl Rng) -> Self {
        Self::from_saved(None, rng)
    }

    /// The weather `saved` in a world, or that of a new world when it saved none.
    pub fn from_saved(saved: Option<SavedWeather>, rng: &mut impl Rng) -> Self {
        let saved = saved.unwrap_or_else(|| SavedWeather {
            kind: WeatherKind::Clear,
            ticks_left: spell_ticks(WeatherKind::Clear, rng),
        });
        Self {
            kind: saved.kind,
            ticks_left: saved.ticks_left,
        }
    }

    /// What `world.ron` keeps of it.
    pub fn saved(&self) -> SavedWeather {
        SavedWeather {
            kind: self.kind,
            ticks_left: self.ticks_left,
        }
    }

    /// Puts `kind`, for a spell of the length it would naturally have.
    pub fn set(&mut self, kind: WeatherKind, rng: &mut impl Rng) {
        self.kind = kind;
        self.ticks_left = spell_ticks(kind, rng);
    }

    /// One tick: the weather counts down, and changes once its time is out. The new kind, when
    /// it changed.
    pub fn tick(&mut self, rng: &mut impl Rng) -> Option<WeatherKind> {
        self.ticks_left = self.ticks_left.saturating_sub(1);
        if self.ticks_left > 0 {
            return None;
        }
        self.set(next_kind(self.kind, rng), rng);
        Some(self.kind)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::rngs::StdRng;
    use rand::SeedableRng;
    use shared::world::{CLEAR_TICKS, STORM_TICKS};

    #[test]
    fn a_new_weather_is_clear_for_a_clear_spell() {
        let weather = ServerWeather::new(&mut StdRng::seed_from_u64(1));
        assert_eq!(weather.kind, WeatherKind::Clear);
        assert!(CLEAR_TICKS.contains(&weather.ticks_left));
    }

    #[test]
    fn the_weather_counts_down_then_changes_for_a_spell_of_the_next_kind() {
        let mut rng = StdRng::seed_from_u64(1);
        let mut weather = ServerWeather {
            kind: WeatherKind::Rain,
            ticks_left: 2,
        };
        assert_eq!(weather.tick(&mut rng), None);
        assert_eq!(weather.ticks_left, 1);
        assert_eq!(weather.tick(&mut rng), Some(WeatherKind::Clear));
        assert_eq!(weather.kind, WeatherKind::Clear);
        assert!(CLEAR_TICKS.contains(&weather.ticks_left));

        weather.ticks_left = 1;
        let next = weather.tick(&mut rng).unwrap();
        assert_ne!(next, WeatherKind::Clear);
        assert_eq!(weather.kind, next);
        assert!(STORM_TICKS.contains(&weather.ticks_left));
    }

    #[test]
    fn a_weather_set_lasts_a_natural_spell_of_its_kind() {
        let mut rng = StdRng::seed_from_u64(1);
        let mut weather = ServerWeather::new(&mut rng);
        weather.set(WeatherKind::Thunder, &mut rng);
        assert_eq!(weather.kind, WeatherKind::Thunder);
        assert!(STORM_TICKS.contains(&weather.ticks_left));
    }

    #[test]
    fn a_saved_weather_is_taken_back_and_a_world_without_one_starts_clear() {
        let mut rng = StdRng::seed_from_u64(1);
        let saved = SavedWeather {
            kind: WeatherKind::Rain,
            ticks_left: 345,
        };
        let weather = ServerWeather::from_saved(Some(saved), &mut rng);
        assert_eq!((weather.kind, weather.ticks_left), (WeatherKind::Rain, 345));
        assert_eq!(weather.saved(), saved);

        let weather = ServerWeather::from_saved(None, &mut rng);
        assert_eq!(weather.kind, WeatherKind::Clear);
        assert!(CLEAR_TICKS.contains(&weather.ticks_left));
    }
}
