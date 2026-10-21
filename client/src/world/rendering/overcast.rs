//! The sky of the weather: how overcast it is, and what that does to the sky, the light and the
//! fog. Every curve gives its clear value exactly at an overcast level of 0, so that clear
//! weather draws as before.

use bevy::prelude::*;

use crate::world::celestial::smoothstep;
use crate::world::weather::ClientWeather;

/// How overcast the sky is, read from the rain and thunder levels: the sky leads the drops.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Overcast {
    /// From 0, a clear sky, to 1, a sky all grey: `smoothstep(0, 0.5, rain)`.
    pub level: f32,
    /// From 0 to 1, how much darker the storm makes it: `smoothstep(0, 0.5, thunder)`.
    pub storm: f32,
}

/// The values of the curves under a clear sky, a full overcast and a full storm.
const SUN: [f32; 3] = [1., 0.25, 0.12];
const EXPOSURE_SHIFT: [f32; 3] = [0., -1.3, -2.];
/// The part of the saturation kept: 1.12 becomes 0.9 in rain.
const SATURATION: [f32; 3] = [1., 0.9 / 1.12, 0.78];
/// Where the fog starts and ends, as parts of the distance to the edge of the chunks shown.
const FOG_START: [f32; 3] = [0.6, 0.3, 0.15];
const FOG_END: [f32; 3] = [1., 0.75, 0.6];
/// The closest the fog ends in rain, in blocks.
const MIN_FOG_END: f32 = 48.;

/// The overcast at the horizon by day, at sunset and by night, in linear RGB: with the
/// physical sky as seen on screen before the tonemapping, and without it as the colour behind
/// the world. The zenith is darker, and a storm darker and cooler.
const PHYSICAL_DAY: Vec3 = Vec3::new(0.62, 0.64, 0.68);
const PHYSICAL_GOLDEN: Vec3 = Vec3::new(0.22, 0.2, 0.19);
const PHYSICAL_NIGHT: Vec3 = Vec3::new(0.015, 0.016, 0.018);
const PLAIN_DAY: Vec3 = Vec3::new(0.3, 0.315, 0.34);
const PLAIN_GOLDEN: Vec3 = Vec3::new(0.085, 0.078, 0.076);
const PLAIN_NIGHT: Vec3 = Vec3::new(0.0045, 0.0048, 0.0055);
const ZENITH: Vec3 = Vec3::new(0.7, 0.71, 0.73);
const STORM_TINT: Vec3 = Vec3::new(0.62, 0.64, 0.66);

impl Overcast {
    pub fn of(weather: &ClientWeather) -> Self {
        Self {
            level: smoothstep(0., 0.5, weather.rain),
            storm: smoothstep(0., 0.5, weather.thunder),
        }
    }

    /// The overcast of `weather`, clear without one (outside of the game, in the tests).
    pub fn current(weather: Option<&ClientWeather>) -> Self {
        weather.map_or_else(Self::default, Self::of)
    }

    /// The value of a curve keyed at `[clear, rain, storm]`: exactly `clear` at a level of 0.
    fn blend(self, [clear, rain, storm]: [f32; 3]) -> f32 {
        clear.lerp(rain, self.level).lerp(storm, self.storm)
    }

    /// The part of the light of the sun and of the moon that gets through the clouds.
    pub fn sun(self) -> f32 {
        self.blend(SUN)
    }

    /// How much the exposure goes down, in EV, as the eye adapts to a darker day: less than
    /// the light, so that a rainy day is darker than a clear one, if not dim.
    pub fn exposure_shift(self) -> f32 {
        self.blend(EXPOSURE_SHIFT)
    }

    /// The saturation of the colour grading, `clear` under a clear sky.
    pub fn saturation(self, clear: f32) -> f32 {
        clear * self.blend(SATURATION)
    }

    /// Where the fog ends, as a part of where it ends under a clear sky.
    pub fn fog_end(self) -> f32 {
        self.blend(FOG_END)
    }

    /// Where the fog starts and ends, in blocks, with the chunks shown up to `distance`.
    pub fn fog_range(self, distance: f32) -> (f32, f32) {
        let end = (distance * self.fog_end()).max(distance.min(MIN_FOG_END));
        let start = (distance * self.blend(FOG_START)).min(end);
        (start, end)
    }

    /// How visible the sun, the moon and the stars are: gone by a level of 0.8.
    pub fn celestial_visibility(self) -> f32 {
        1. - smoothstep(0., 0.8, self.level)
    }

    /// The overcast sky at the horizon and at the zenith, in linear RGB, with the sun at
    /// `sun_height` (the sine of its height). With the physical sky, as seen on screen before
    /// the tonemapping; without it, as the colour behind the world. As bright as the clear sky
    /// is at that hour, not as the sky dimmed by the clouds.
    pub fn colors(self, sun_height: f32, physical_sky: bool) -> (Vec3, Vec3) {
        let [day, golden, night] = if physical_sky {
            [PHYSICAL_DAY, PHYSICAL_GOLDEN, PHYSICAL_NIGHT]
        } else {
            [PLAIN_DAY, PLAIN_GOLDEN, PLAIN_NIGHT]
        };
        let horizon = if sun_height >= 0. {
            golden.lerp(day, smoothstep(0., 0.3, sun_height))
        } else {
            golden.lerp(night, smoothstep(0., 0.2, -sun_height))
        };
        let horizon = horizon * Vec3::ONE.lerp(STORM_TINT, self.storm);
        (horizon, horizon * ZENITH)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::world::WeatherKind;

    const CLEAR: Overcast = Overcast {
        level: 0.,
        storm: 0.,
    };
    const RAIN: Overcast = Overcast {
        level: 1.,
        storm: 0.,
    };
    const STORM: Overcast = Overcast {
        level: 1.,
        storm: 1.,
    };

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-5
    }

    #[test]
    fn the_sky_closes_over_the_first_half_of_the_rain() {
        let weather = |rain, thunder| {
            let mut weather = ClientWeather::settled(WeatherKind::Thunder);
            (weather.rain, weather.thunder) = (rain, thunder);
            weather
        };
        assert_eq!(Overcast::of(&weather(0., 0.)), CLEAR);
        assert_eq!(Overcast::of(&weather(0.5, 0.)), RAIN);
        assert_eq!(Overcast::of(&weather(1., 1.)), STORM);
        assert_eq!(Overcast::of(&weather(0.25, 0.25)).level, 0.5);
        assert_eq!(Overcast::of(&weather(0.25, 0.25)).storm, 0.5);
        assert_eq!(Overcast::current(None), CLEAR);
        assert_eq!(
            Overcast::current(Some(&ClientWeather::settled(WeatherKind::Rain))),
            RAIN
        );
    }

    #[test]
    fn a_clear_sky_changes_nothing() {
        assert_eq!(CLEAR.sun(), 1.);
        assert_eq!(CLEAR.exposure_shift(), 0.);
        assert_eq!(CLEAR.saturation(1.12), 1.12);
        assert_eq!(CLEAR.celestial_visibility(), 1.);
        assert_eq!(CLEAR.fog_range(256.), (0.6 * 256., 256.));
    }

    #[test]
    fn rain_dims_the_sun_and_a_storm_more() {
        assert!(close(RAIN.sun(), 0.25));
        assert!(close(STORM.sun(), 0.12));
        assert!(close(RAIN.exposure_shift(), -1.3));
        assert!(close(STORM.exposure_shift(), -2.));
        // the exposure makes up for some of the light lost, not all of it
        for overcast in [RAIN, STORM] {
            assert!(overcast.sun() * 2f32.powf(-overcast.exposure_shift()) < 0.75);
        }
        assert!(close(RAIN.saturation(1.12), 0.9));
        assert!(STORM.saturation(1.12) <= RAIN.saturation(1.12));
    }

    #[test]
    fn the_sun_the_moon_and_the_stars_are_gone_by_four_fifths_of_the_overcast() {
        let at = |level| Overcast { level, storm: 0. }.celestial_visibility();
        assert_eq!(at(0.8), 0.);
        assert_eq!(at(1.), 0.);
        assert!(at(0.4) > 0. && at(0.4) < 1.);
        assert!(at(0.2) > at(0.4));
    }

    #[test]
    fn the_fog_closes_in_under_rain_and_more_in_a_storm() {
        let (start, end) = RAIN.fog_range(256.);
        assert!(close(start, 0.3 * 256.) && close(end, 0.75 * 256.));
        let (start, end) = STORM.fog_range(256.);
        assert!(close(start, 0.15 * 256.) && close(end, 0.6 * 256.));
        // halfway, in between
        let (start, end) = Overcast {
            level: 0.5,
            storm: 0.,
        }
        .fog_range(256.);
        assert!(start > 0.3 * 256. && start < 0.6 * 256., "{start}");
        assert!(end > 0.75 * 256. && end < 256., "{end}");
    }

    #[test]
    fn the_fog_never_ends_closer_than_48_blocks_nor_past_the_chunks() {
        let (start, end) = STORM.fog_range(64.);
        assert_eq!(end, 48.);
        assert!(start < end);
        // fewer chunks shown than that: the fog ends where they do, as in clear weather
        assert_eq!(STORM.fog_range(32.).1, 32.);
        assert!(STORM.fog_range(32.).0 <= 32.);
    }

    fn brightness(color: Vec3) -> f32 {
        color.element_sum() / 3.
    }

    #[test]
    fn the_overcast_is_grey_lighter_at_the_horizon() {
        for physical in [true, false] {
            for height in [-1., -0.05, 0., 0.1, 0.3, 0.9] {
                for overcast in [RAIN, STORM] {
                    let (horizon, zenith) = overcast.colors(height, physical);
                    for color in [horizon, zenith] {
                        let spread = color.max_element() - color.min_element();
                        assert!(spread <= 0.3 * color.max_element(), "{color}");
                    }
                    assert!(brightness(horizon) >= brightness(zenith), "{height}");
                }
            }
        }
    }

    #[test]
    fn the_overcast_follows_the_day_and_darkens_in_a_storm() {
        for physical in [true, false] {
            let noon = brightness(RAIN.colors(0.9, physical).0);
            let dusk = brightness(RAIN.colors(0., physical).0);
            let night = brightness(RAIN.colors(-0.9, physical).0);
            assert!(
                noon > 2. * dusk && dusk > 2. * night,
                "{noon} {dusk} {night}"
            );
            assert!(brightness(STORM.colors(0.9, physical).0) < 0.8 * noon);
            let mut before = RAIN.colors(-1., physical).0;
            for step in -99..=100 {
                let now = RAIN.colors(step as f32 / 100., physical).0;
                assert!(now.distance(before) < 0.05, "{step}: {before} then {now}");
                before = now;
            }
        }
    }
}
