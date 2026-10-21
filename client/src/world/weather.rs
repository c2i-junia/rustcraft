//! The weather as the client shows it: the kind the server tells, and the rain and thunder
//! levels that ease towards it, which the effects of the weather read.

use bevy::prelude::*;
use shared::world::WeatherKind;

use crate::world::celestial::smoothstep;

/// Seconds for a level to go all the way from 0 to 1, or back.
const TRANSITION_SECS: f32 = 20.0;

/// The weather of the world the player is in. Left clear outside of a world.
#[derive(Resource, Debug, Clone, Default, PartialEq)]
pub struct ClientWeather {
    /// The weather the server last told.
    pub kind: WeatherKind,
    /// How much it rains, from 0 to 1: towards 1 in rain and in a thunderstorm.
    pub rain: f32,
    /// How stormy it is, from 0 to 1: towards 1 in a thunderstorm only.
    pub thunder: f32,
    /// What `rain` and `thunder` follow, on a smoothstep: they move linearly, at
    /// 1 / `TRANSITION_SECS` a second, so that a change turned back midway does not jump.
    rain_ramp: f32,
    thunder_ramp: f32,
}

impl ClientWeather {
    /// `kind`, its levels reached already: the weather a player joins in.
    pub fn settled(kind: WeatherKind) -> Self {
        let (rain_ramp, thunder_ramp) = targets(kind);
        let mut weather = Self {
            kind,
            rain_ramp,
            thunder_ramp,
            ..default()
        };
        weather.advance(0.0);
        weather
    }

    /// Moves the levels `secs` seconds further towards those of `kind`.
    pub fn advance(&mut self, secs: f32) {
        let step = secs / TRANSITION_SECS;
        let (rain, thunder) = targets(self.kind);
        self.rain_ramp = approach(self.rain_ramp, rain, step);
        self.thunder_ramp = approach(self.thunder_ramp, thunder, step);
        self.rain = smoothstep(0.0, 1.0, self.rain_ramp);
        self.thunder = smoothstep(0.0, 1.0, self.thunder_ramp);
    }
}

/// The rain and thunder levels of `kind` once reached.
fn targets(kind: WeatherKind) -> (f32, f32) {
    match kind {
        WeatherKind::Clear => (0.0, 0.0),
        WeatherKind::Rain => (1.0, 0.0),
        WeatherKind::Thunder => (1.0, 1.0),
    }
}

/// `from` moved by `step` towards `to`, without passing it.
fn approach(from: f32, to: f32, step: f32) -> f32 {
    if from < to {
        (from + step).min(to)
    } else {
        (from - step).max(to)
    }
}

pub fn ease_weather_system(time: Res<Time>, mut weather: ResMut<ClientWeather>) {
    weather.advance(time.delta_secs());
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `weather` after `secs` seconds, in steps of a frame at 60 FPS.
    fn after(mut weather: ClientWeather, secs: f32) -> ClientWeather {
        let frames = (secs * 60.0).round() as usize;
        for _ in 0..frames {
            weather.advance(1.0 / 60.0);
        }
        weather
    }

    fn levels(weather: &ClientWeather) -> (f32, f32) {
        (weather.rain, weather.thunder)
    }

    #[test]
    fn a_settled_weather_has_its_levels_at_once() {
        assert_eq!(
            levels(&ClientWeather::settled(WeatherKind::Clear)),
            (0.0, 0.0)
        );
        assert_eq!(
            levels(&ClientWeather::settled(WeatherKind::Rain)),
            (1.0, 0.0)
        );
        assert_eq!(
            levels(&ClientWeather::settled(WeatherKind::Thunder)),
            (1.0, 1.0)
        );
        assert_eq!(
            ClientWeather::default(),
            ClientWeather::settled(WeatherKind::Clear)
        );
    }

    #[test]
    fn rain_rolls_in_over_20_seconds_starting_gently() {
        let weather = ClientWeather {
            kind: WeatherKind::Rain,
            ..default()
        };
        let early = after(weather.clone(), 1.0);
        assert!(early.rain > 0.0 && early.rain < 0.02, "{}", early.rain);
        let half = after(weather.clone(), 10.0);
        assert!((half.rain - 0.5).abs() < 0.01, "{}", half.rain);
        let almost = after(weather.clone(), 19.0);
        assert!(almost.rain > 0.98 && almost.rain < 1.0, "{}", almost.rain);
        let done = after(weather, 20.5);
        assert_eq!(levels(&done), (1.0, 0.0));
    }

    #[test]
    fn a_thunderstorm_brings_rain_and_thunder_and_clear_weather_takes_both_away() {
        let weather = ClientWeather {
            kind: WeatherKind::Thunder,
            ..default()
        };
        let mut weather = after(weather, 21.0);
        assert_eq!(levels(&weather), (1.0, 1.0));

        // rain without thunder: the thunder goes, the rain stays
        weather.kind = WeatherKind::Rain;
        let mut weather = after(weather, 21.0);
        assert_eq!(levels(&weather), (1.0, 0.0));

        weather.kind = WeatherKind::Clear;
        assert_eq!(levels(&after(weather, 21.0)), (0.0, 0.0));
    }

    #[test]
    fn a_change_turned_back_midway_goes_back_without_a_jump() {
        let weather = ClientWeather {
            kind: WeatherKind::Rain,
            ..default()
        };
        let mut weather = after(weather, 8.0);
        let before = weather.rain;
        weather.kind = WeatherKind::Clear;
        weather.advance(1.0 / 60.0);
        assert!(weather.rain < before && before - weather.rain < 0.01);
        assert_eq!(after(weather, 8.0).rain, 0.0);
    }
}
