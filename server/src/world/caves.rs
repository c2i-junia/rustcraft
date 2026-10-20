//! Caves: winding tunnels where a floor meets a wall, and wide chambers deep down where a noise
//! peaks. The floors are levels stacked every `LEVEL_SPACING / 2` blocks, raised and lowered by
//! a slow noise so that they tilt but never stand up; the walls are the zero surfaces of a noise
//! that changes slowly upwards, so they are steep. A tunnel therefore runs level or sloping,
//! never straight up, and it climbs to the surface where its level meets rising ground. A
//! third noise widens some tunnels into galleries and pinches others shut. The noises are
//! sampled on a coarse grid aligned on the world and interpolated in between, so a cell is
//! carved or not whichever chunk asks.

use bevy::prelude::*;
use noise::{NoiseFn, SuperSimplex};
use shared::{world::splitmix64, CHUNK_SIZE};

/// Nothing is carved below this height, so that the bottom of the world stays whole.
pub const CAVE_FLOOR: i32 = 6;

/// The floors: `sin(2π (y + LEVEL_SHIFT × noise) / LEVEL_SPACING)` is zero on them, the noise
/// having the wavelengths below. `LEVEL_SHIFT` sets how steep a floor gets; the noise changes
/// so slowly upwards that the levels never cross.
const LEVEL_SPACING: f64 = 48.0;
const LEVEL_SHIFT: f64 = 28.0;
const LEVEL_SHIFT_WAVELENGTH: f64 = 128.0;
const LEVEL_SHIFT_WAVELENGTH_Y: f64 = 384.0;
/// The walls: the zero surfaces of a noise of these wavelengths, across and up.
const WALLS_WAVELENGTH: f64 = 72.0;
const WALLS_WAVELENGTH_Y: f64 = 160.0;
/// The floors are scaled by this before they are squared: it sets how high a tunnel is against
/// how wide.
const FLOORS_SCALE: f64 = 0.4;
/// A cell is in a tunnel where the sum of the squares of the floors and the walls is below
/// this, times a factor from `TUNNEL_SHUT` (a negative one closes the tunnel) to `TUNNEL_WIDE`
/// that a slow noise sets.
const TUNNEL_WIDTH: f64 = 0.02;
const TUNNEL_SHUT: f64 = -0.4;
const TUNNEL_WIDE: f64 = 2.6;
const TUNNEL_SIZE_WAVELENGTH: f64 = 96.0;

/// Chambers open where their noise exceeds `CHAMBER_FROM` below `CHAMBERS_BELOW`, the
/// threshold rising over `CHAMBER_FADE` blocks under that height so that their roofs are not
/// flat.
const CHAMBER_WAVELENGTH: f64 = 56.0;
const CHAMBER_WAVELENGTH_Y: f64 = 32.0;
const CHAMBER_FROM: f64 = 0.6;
const CHAMBERS_BELOW: i32 = 48;
const CHAMBER_FADE: i32 = 16;

/// The noises are sampled every this many blocks, on each axis, and interpolated in between.
const STEP: i32 = 4;
/// Samples along each axis of a chunk, both of its faces included.
const SAMPLES: usize = (CHUNK_SIZE / STEP) as usize + 1;

// Salts: each noise of a world takes its seed from `(seed, salt)`. Apart from the terrain's.
const SALT_TUNNEL_A: u64 = 0x100;
const SALT_TUNNEL_B: u64 = 0x101;
const SALT_CHAMBERS: u64 = 0x102;
const SALT_TUNNEL_SIZE: u64 = 0x103;

/// One SuperSimplex noise, stretched to its wavelengths and offset by its seed: a noise is
/// zero at its origin whatever its seed.
pub(crate) struct Noise3 {
    noise: SuperSimplex,
    offset: [f64; 3],
    frequency: [f64; 3],
}

impl Noise3 {
    pub(crate) fn new(seed: u32, salt: u64, wavelength: f64, wavelength_y: f64) -> Self {
        let hash = splitmix64(splitmix64(seed as u64) ^ salt);
        let offset = |bits: u64| (bits & 0xffff) as f64 / 64.0;
        Self {
            noise: SuperSimplex::new((hash >> 32) as u32),
            offset: [offset(hash), offset(hash >> 16), offset(hash >> 48)],
            frequency: [1.0 / wavelength, 1.0 / wavelength_y, 1.0 / wavelength],
        }
    }

    pub(crate) fn get(&self, pos: IVec3) -> f64 {
        let [fx, fy, fz] = self.frequency;
        let [ox, oy, oz] = self.offset;
        self.noise.get([
            pos.x as f64 * fx + ox,
            pos.y as f64 * fy + oy,
            pos.z as f64 * fz + oz,
        ])
    }
}

/// The noises of the caves of a world, built once from its seed.
pub struct Caves {
    level_shift: Noise3,
    walls: Noise3,
    tunnel_size: Noise3,
    chambers: Noise3,
}

/// The cells of a chunk that the caves carve out.
pub struct CaveMask([u64; (CHUNK_SIZE * CHUNK_SIZE * CHUNK_SIZE / 64) as usize]);

impl CaveMask {
    fn index(local: IVec3) -> usize {
        ((local.y * CHUNK_SIZE + local.z) * CHUNK_SIZE + local.x) as usize
    }

    pub fn contains(&self, local: IVec3) -> bool {
        let index = Self::index(local);
        self.0[index / 64] & (1 << (index % 64)) != 0
    }

    fn insert(&mut self, local: IVec3) {
        let index = Self::index(local);
        self.0[index / 64] |= 1 << (index % 64);
    }
}

impl Caves {
    pub fn new(seed: u32) -> Self {
        Self {
            level_shift: Noise3::new(
                seed,
                SALT_TUNNEL_A,
                LEVEL_SHIFT_WAVELENGTH,
                LEVEL_SHIFT_WAVELENGTH_Y,
            ),
            walls: Noise3::new(seed, SALT_TUNNEL_B, WALLS_WAVELENGTH, WALLS_WAVELENGTH_Y),
            tunnel_size: Noise3::new(
                seed,
                SALT_TUNNEL_SIZE,
                TUNNEL_SIZE_WAVELENGTH,
                TUNNEL_SIZE_WAVELENGTH,
            ),
            chambers: Noise3::new(
                seed,
                SALT_CHAMBERS,
                CHAMBER_WAVELENGTH,
                CHAMBER_WAVELENGTH_Y,
            ),
        }
    }

    /// The cells the caves carve out of the chunk whose lowest corner is `chunk_origin`,
    /// whatever is there: the caller keeps what must stay whole.
    pub fn carve(&self, chunk_origin: IVec3) -> CaveMask {
        let mut mask = CaveMask([0; (CHUNK_SIZE * CHUNK_SIZE * CHUNK_SIZE / 64) as usize]);
        if chunk_origin.y + CHUNK_SIZE <= CAVE_FLOOR {
            return mask;
        }
        let deep = chunk_origin.y < CHAMBERS_BELOW;
        let sample_with = |f: &dyn Fn(IVec3) -> f64| {
            let mut samples = [[[0.0; SAMPLES]; SAMPLES]; SAMPLES];
            for (i, plane) in samples.iter_mut().enumerate() {
                for (j, row) in plane.iter_mut().enumerate() {
                    for (k, value) in row.iter_mut().enumerate() {
                        let step = IVec3::new(i as i32, j as i32, k as i32) * STEP;
                        *value = f(chunk_origin + step);
                    }
                }
            }
            samples
        };
        let sample = |noise: &Noise3| sample_with(&|pos| noise.get(pos));
        let floors = sample_with(&|pos| {
            let level = pos.y as f64 + LEVEL_SHIFT * self.level_shift.get(pos);
            FLOORS_SCALE * (std::f64::consts::TAU * level / LEVEL_SPACING).sin()
        });
        let walls = sample(&self.walls);
        let tunnel_size = sample(&self.tunnel_size);
        let chambers = deep.then(|| sample(&self.chambers));

        for y in 0..CHUNK_SIZE {
            let world_y = chunk_origin.y + y;
            if world_y < CAVE_FLOOR {
                continue;
            }
            let chamber_from = CHAMBER_FROM
                + (1.0 - CHAMBER_FROM)
                    * ((world_y - (CHAMBERS_BELOW - CHAMBER_FADE)).max(0) as f64
                        / CHAMBER_FADE as f64);
            for z in 0..CHUNK_SIZE {
                for x in 0..CHUNK_SIZE {
                    let local = IVec3::new(x, y, z);
                    let floor = interpolate(&floors, local);
                    let wall = interpolate(&walls, local);
                    let size = smoothstep(-0.4, 0.5, interpolate(&tunnel_size, local));
                    let width = TUNNEL_WIDTH * (TUNNEL_SHUT + (TUNNEL_WIDE - TUNNEL_SHUT) * size);
                    let tunnel = floor * floor + wall * wall < width;
                    let chamber = chambers.as_ref().is_some_and(|c| {
                        world_y < CHAMBERS_BELOW && interpolate(c, local) > chamber_from
                    });
                    if tunnel || chamber {
                        mask.insert(local);
                    }
                }
            }
        }
        mask
    }
}

/// `0` up to `edge0`, `1` from `edge1`, and a smooth S-curve in between.
fn smoothstep(edge0: f64, edge1: f64, value: f64) -> f64 {
    let t = ((value - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// The value at `local` of noise samples taken every `STEP` blocks, trilinearly interpolated.
fn interpolate(samples: &[[[f64; SAMPLES]; SAMPLES]; SAMPLES], local: IVec3) -> f64 {
    let (i, j, k) = (
        (local.x / STEP) as usize,
        (local.y / STEP) as usize,
        (local.z / STEP) as usize,
    );
    let t = |v: i32| (v % STEP) as f64 / STEP as f64;
    let (tx, ty, tz) = (t(local.x), t(local.y), t(local.z));
    let lerp = |a: f64, b: f64, t: f64| a + (b - a) * t;
    let along_z = |i: usize, j: usize| lerp(samples[i][j][k], samples[i][j][k + 1], tz);
    let along_y = |i: usize| lerp(along_z(i, j), along_z(i, j + 1), ty);
    lerp(along_y(i), along_y(i + 1), tx)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn carved(caves: &Caves, chunk_pos: IVec3) -> Vec<IVec3> {
        let mask = caves.carve(chunk_pos * CHUNK_SIZE);
        let range = 0..CHUNK_SIZE;
        range
            .clone()
            .flat_map(|x| {
                range
                    .clone()
                    .flat_map(move |y| (0..CHUNK_SIZE).map(move |z| IVec3::new(x, y, z)))
            })
            .filter(|local| mask.contains(*local))
            .collect()
    }

    #[test]
    fn caves_are_carved_underground_but_never_at_the_bottom_of_the_world() {
        let caves = Caves::new(42);
        let mut total = 0;
        for x in -3..3 {
            for z in -3..3 {
                for y in 0..4 {
                    let cells = carved(&caves, IVec3::new(x, y, z));
                    for local in &cells {
                        assert!(y * CHUNK_SIZE + local.y >= CAVE_FLOOR);
                    }
                    total += cells.len();
                }
            }
        }
        // between one cell in a hundred and one in ten
        let cells = 6 * 6 * 4 * CHUNK_SIZE.pow(3) as usize;
        assert!(
            (cells / 100..cells / 10).contains(&total),
            "{total} of {cells}"
        );
    }

    #[test]
    fn the_caves_depend_only_on_the_seed() {
        // a single chunk may hold no cave for either seed: compare a row of them
        let row = |seed| -> Vec<Vec<IVec3>> {
            let caves = Caves::new(seed);
            (-4..4)
                .map(|x| carved(&caves, IVec3::new(x, 1, 3)))
                .collect()
        };
        let first = row(7);
        assert!(first.iter().any(|cells| !cells.is_empty()));
        assert_eq!(first, row(7));
        assert_ne!(first, row(8));
    }
}
