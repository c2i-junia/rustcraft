use bevy::math::{IVec2, Vec2};
use bevy::prelude::FloatExt;
use shared::{world::splitmix64, TICKS_PER_SECOND};

use super::{CELL, PERIOD, SEED, SPEED};
use crate::world::overcast::Overcast;

/// The wavelengths of the octaves of the noise, in cells, each with half the weight of the
/// previous one. Each divides `PERIOD`, so that the noise tiles.
const WAVELENGTHS: [i32; 3] = [8, 4, 2];

/// The density above which a cell is full under a clear sky, in rain and in a thunderstorm,
/// measured over one period so that a share `COVERAGE`, `RAIN_COVERAGE` and `STORM_COVERAGE`
/// of the cells are.
pub const CLEAR_THRESHOLD: f32 = 0.031;
pub const RAIN_THRESHOLD: f32 = -0.256;
pub const STORM_THRESHOLD: f32 = -0.41;

/// The densities the mesh gives to the cells full at any threshold of the weather, and to
/// those full at none.
pub const ALWAYS: f32 = 2.;
pub const NEVER: f32 = -2.;

/// Whether the cell `cell` of the pattern holds a cloud under a clear sky. The same on every
/// call and for every player, and periodic: `cell` and `cell + PERIOD` on either axis are
/// alike.
pub fn is_full(cell: IVec2) -> bool {
    density(cell) > CLEAR_THRESHOLD
}

/// The density above which a cell is full under `overcast`: it eases from the clear threshold
/// to the rain one with the overcast, and on to the storm one with the storm.
pub fn threshold(overcast: Overcast) -> f32 {
    CLEAR_THRESHOLD
        .lerp(RAIN_THRESHOLD, overcast.level)
        .lerp(STORM_THRESHOLD, overcast.storm)
}

/// The density of `cell` as the mesh holds it: `ALWAYS` above the clear threshold, `NEVER` at
/// or under the storm one, so that the cells alike at every threshold of the weather merge
/// into the same faces.
pub fn banded_density(cell: IVec2) -> f32 {
    match density(cell) {
        density if density > CLEAR_THRESHOLD => ALWAYS,
        density if density <= STORM_THRESHOLD => NEVER,
        density => density,
    }
}

/// Value noise in `[-1, 1]`: random values on a lattice that wraps around every period, smoothly
/// interpolated between its points.
pub fn density(cell: IVec2) -> f32 {
    let cell = IVec2::new(cell.x.rem_euclid(PERIOD), cell.y.rem_euclid(PERIOD));
    let (mut sum, mut total, mut weight) = (0.0, 0.0, 1.0);
    for (octave, wavelength) in WAVELENGTHS.into_iter().enumerate() {
        let points = PERIOD / wavelength;
        let lattice = |point: IVec2| {
            let [x, z] = [point.x, point.y].map(|c| c.rem_euclid(points) as u64);
            let hash = splitmix64(splitmix64(splitmix64(SEED ^ octave as u64) ^ x) ^ z);
            (hash >> 40) as f32 / (1u64 << 23) as f32 - 1.
        };
        // the middle of the cell, in lattice steps
        let position = (cell.as_vec2() + 0.5) / wavelength as f32;
        let corner = position.floor().as_ivec2();
        let t = position - position.floor();
        let t = t * t * (Vec2::splat(3.) - 2. * t);
        let row = |dz| {
            let a = lattice(corner + IVec2::new(0, dz));
            let b = lattice(corner + IVec2::new(1, dz));
            a + (b - a) * t.x
        };
        let (near, far) = (row(0), row(1));
        sum += weight * (near + (far - near) * t.y);
        total += weight;
        weight *= 0.5;
    }
    sum / total
}

/// How far the clouds have drifted along +X at the world clock `tick`, in blocks, wrapped to
/// one period of the pattern: `f32` would lose the fraction of a block after a few hours.
pub fn drift(tick: u64) -> f32 {
    let blocks = tick as f64 / TICKS_PER_SECOND as f64 * SPEED as f64;
    (blocks % (PERIOD * CELL) as f64) as f32
}

#[cfg(test)]
mod tests {
    use super::super::{COVERAGE, RAIN_COVERAGE, STORM_COVERAGE};
    use super::*;

    fn period() -> impl Iterator<Item = IVec2> {
        (0..PERIOD).flat_map(|z| (0..PERIOD).map(move |x| IVec2::new(x, z)))
    }

    #[test]
    fn the_field_is_the_same_on_every_call() {
        let first: Vec<bool> = period().map(is_full).collect();
        let second: Vec<bool> = period().map(is_full).collect();
        assert_eq!(first, second);
    }

    #[test]
    fn the_field_repeats_every_period() {
        for cell in period().step_by(7) {
            for shift in [
                IVec2::new(PERIOD, 0),
                IVec2::new(0, PERIOD),
                IVec2::new(-PERIOD, 3 * PERIOD),
            ] {
                assert_eq!(is_full(cell), is_full(cell + shift), "{cell} + {shift}");
            }
        }
    }

    #[test]
    fn about_the_target_share_of_cells_is_full() {
        let full = period().filter(|cell| is_full(*cell)).count();
        let share = full as f64 / (PERIOD * PERIOD) as f64;
        assert!((share - COVERAGE).abs() <= 0.05, "{share}");
    }

    #[test]
    fn rain_and_storms_cover_more_of_the_sky() {
        for (threshold, coverage) in [
            (RAIN_THRESHOLD, RAIN_COVERAGE),
            (STORM_THRESHOLD, STORM_COVERAGE),
        ] {
            let full = period().filter(|cell| density(*cell) > threshold).count();
            let share = full as f64 / (PERIOD * PERIOD) as f64;
            assert!((share - coverage).abs() <= 0.03, "{share}");
        }
    }

    #[test]
    fn the_threshold_follows_the_overcast_and_the_storm() {
        let at = |level, storm| threshold(Overcast { level, storm });
        assert_eq!(at(0., 0.), CLEAR_THRESHOLD);
        assert_eq!(at(1., 0.), RAIN_THRESHOLD);
        assert_eq!(at(1., 1.), STORM_THRESHOLD);
        assert!(at(0.5, 0.) < CLEAR_THRESHOLD && at(0.5, 0.) > RAIN_THRESHOLD);
    }

    #[test]
    fn a_banded_density_keeps_what_the_thresholds_of_the_weather_tell() {
        for cell in period().step_by(3) {
            let banded = banded_density(cell);
            for threshold in [CLEAR_THRESHOLD, -0.1, RAIN_THRESHOLD, STORM_THRESHOLD] {
                assert_eq!(banded > threshold, density(cell) > threshold, "{cell}");
            }
            assert_eq!(banded > CLEAR_THRESHOLD, is_full(cell));
        }
    }

    /// Salt and pepper would make half of the neighbours differ; blobs of tens of cells, a
    /// small share.
    #[test]
    fn full_cells_come_in_blobs() {
        let differ = period()
            .filter(|cell| is_full(*cell) != is_full(*cell + IVec2::X))
            .count();
        let share = differ as f64 / (PERIOD * PERIOD) as f64;
        assert!(share < 0.15, "{share}");
        assert!(share > 0.0, "{share}");
    }

    #[test]
    fn the_clouds_drift_half_a_block_a_second() {
        assert_eq!(drift(0), 0.);
        assert_eq!(drift(TICKS_PER_SECOND), 0.5);
        assert_eq!(drift(10 * TICKS_PER_SECOND + TICKS_PER_SECOND / 2), 5.25);
    }

    #[test]
    fn the_drift_wraps_after_one_period() {
        let period_in_ticks = (PERIOD * CELL) as u64 * 2 * TICKS_PER_SECOND;
        assert_eq!(drift(period_in_ticks), 0.);
        assert_eq!(drift(period_in_ticks + TICKS_PER_SECOND), 0.5);
        assert_eq!(
            drift(period_in_ticks - TICKS_PER_SECOND),
            (PERIOD * CELL) as f32 - 0.5
        );
        // a hundred years of ticks keep the exact fraction
        let years = 100 * 365 * 24 * 3600 * TICKS_PER_SECOND;
        let wrapped = years % period_in_ticks;
        assert_eq!(drift(years + 1), drift(wrapped + 1));
        assert!(drift(u64::MAX) < (PERIOD * CELL) as f32);
    }
}
