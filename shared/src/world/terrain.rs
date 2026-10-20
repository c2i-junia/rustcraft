use noise::{NoiseFn, SuperSimplex};

use super::{biome::OCEAN_HUMIDITY, determine_biome, splitmix64, BiomeType};

/// Water fills every column up to this height.
pub const SEA_LEVEL: i32 = 62;

/// Distance over which the temperature and the humidity go from one extreme to the other: it
/// sets the width of the biomes.
const CLIMATE_WAVELENGTH: f64 = 700.0;
/// How strongly the climate is pushed away from its middle, which the octaves crowd it into.
const CLIMATE_CONTRAST: f64 = 1.9;
/// The climate is read this many blocks away from the column, in a direction that wanders, so
/// that the borders of the biomes meander instead of tracing round blobs.
const CLIMATE_WARP: f64 = 60.0;
const CLIMATE_WARP_WAVELENGTH: f64 = 200.0;

/// Height of the land around which the hills rise and sink.
const LAND_HEIGHT: f64 = 67.0;
const HILLS_WAVELENGTH: f64 = 180.0;
/// How far the hills rise above and sink below `LAND_HEIGHT`, where the land is flattest and
/// where it is roughest. The roughness changes slowly, so that plains and hilly land alternate.
const HILLS_FLAT: f64 = 4.0;
const HILLS_ROUGH: f64 = 16.0;
const ROUGHNESS_WAVELENGTH: f64 = 700.0;

/// The sea floor sinks from just below the sea at the coast to this depth below it, as the
/// humidity rises from the coast to its maximum.
const OCEAN_DEPTH: f64 = 18.0;
/// Humidity over which the land slopes down into the sea, on each side of the coast.
const COAST_WIDTH: f64 = 0.025;

// Salts: each noise of a world takes its seed from `(seed, salt, octave)`.
const SALT_TEMPERATURE: u64 = 1;
const SALT_HUMIDITY: u64 = 2;
const SALT_WARP_X: u64 = 3;
const SALT_WARP_Z: u64 = 4;
const SALT_HILLS: u64 = 5;
const SALT_ROUGHNESS: u64 = 6;

/// What the noises decide for one world column, before it is filled with blocks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ColumnShape {
    /// Height of the surface block.
    pub height: i32,
    pub biome: BiomeType,
}

/// Octaves of SuperSimplex noise, each with twice the frequency and half the weight of the
/// previous one, averaged so that the sum stays within `[-1, 1]`.
struct Fractal {
    octaves: Vec<SuperSimplex>,
    frequency: f64,
}

impl Fractal {
    fn new(seed: u32, salt: u64, octaves: u64, wavelength: f64) -> Self {
        let octaves = (0..octaves)
            .map(|octave| {
                let hash = splitmix64(splitmix64(splitmix64(seed as u64) ^ salt) ^ octave);
                SuperSimplex::new((hash >> 32) as u32)
            })
            .collect();
        Self {
            octaves,
            frequency: 1.0 / wavelength,
        }
    }

    fn get(&self, x: f64, z: f64) -> f64 {
        let (mut sum, mut total) = (0.0, 0.0);
        let (mut weight, mut frequency) = (1.0, self.frequency);
        for octave in &self.octaves {
            sum += weight * octave.get([x * frequency, z * frequency]);
            total += weight;
            weight *= 0.5;
            frequency *= 2.0;
        }
        sum / total
    }
}

/// `0` up to `edge0`, `1` from `edge1`, and a smooth S-curve in between.
fn smoothstep(edge0: f64, edge1: f64, value: f64) -> f64 {
    let t = ((value - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn lerp(from: f64, to: f64, t: f64) -> f64 {
    from + (to - from) * t
}

/// Temperature and humidity of a column, both in `[0, 1]`.
struct Climate {
    temperature: f64,
    humidity: f64,
}

/// Every noise the shape of a world depends on, built once from its seed. The server
/// generates the terrain with them; the client, given the seed, finds the biome the player is
/// in.
pub struct TerrainNoises {
    temperature: Fractal,
    humidity: Fractal,
    warp_x: Fractal,
    warp_z: Fractal,
    hills: Fractal,
    roughness: Fractal,
}

impl TerrainNoises {
    pub fn new(seed: u32) -> Self {
        Self {
            temperature: Fractal::new(seed, SALT_TEMPERATURE, 3, CLIMATE_WAVELENGTH),
            humidity: Fractal::new(seed, SALT_HUMIDITY, 3, CLIMATE_WAVELENGTH),
            warp_x: Fractal::new(seed, SALT_WARP_X, 2, CLIMATE_WARP_WAVELENGTH),
            warp_z: Fractal::new(seed, SALT_WARP_Z, 2, CLIMATE_WARP_WAVELENGTH),
            hills: Fractal::new(seed, SALT_HILLS, 4, HILLS_WAVELENGTH),
            roughness: Fractal::new(seed, SALT_ROUGHNESS, 2, ROUGHNESS_WAVELENGTH),
        }
    }

    /// The shape of the world column `(x, z)`: a pure function of the seed and the coordinates.
    pub fn column_shape(&self, x: i32, z: i32) -> ColumnShape {
        let (x, z) = (x as f64, z as f64);
        let climate = self.climate(x, z);
        ColumnShape {
            height: self.height(x, z, &climate).round() as i32,
            biome: determine_biome(climate.temperature, climate.humidity),
        }
    }

    fn climate(&self, x: f64, z: f64) -> Climate {
        let warped_x = x + CLIMATE_WARP * self.warp_x.get(x, z);
        let warped_z = z + CLIMATE_WARP * self.warp_z.get(x, z);
        let unit = |noise: f64| (0.5 + 0.5 * CLIMATE_CONTRAST * noise).clamp(0.0, 1.0);
        Climate {
            temperature: unit(self.temperature.get(warped_x, warped_z)),
            humidity: unit(self.humidity.get(warped_x, warped_z)),
        }
    }

    /// The land, made of hills, sinks smoothly into the sea floor across the coast, so that
    /// the ground never steps at the border of two biomes.
    fn height(&self, x: f64, z: f64, climate: &Climate) -> f64 {
        let roughness = smoothstep(-0.35, 0.35, self.roughness.get(x, z));
        let hills = self.hills.get(x, z);
        let land = LAND_HEIGHT + hills * lerp(HILLS_FLAT, HILLS_ROUGH, roughness);

        let humidity = climate.humidity;
        let depth = smoothstep(OCEAN_HUMIDITY, 1.0, humidity);
        let sea_floor = SEA_LEVEL as f64 - 2.0 - OCEAN_DEPTH * depth + 3.0 * hills;

        let sea = smoothstep(
            OCEAN_HUMIDITY - COAST_WIDTH,
            OCEAN_HUMIDITY + COAST_WIDTH,
            humidity,
        );
        lerp(land, sea_floor, sea)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_shape_of_a_column_depends_only_on_the_seed_and_the_coordinates() {
        let first = TerrainNoises::new(42);
        let second = TerrainNoises::new(42);
        for (x, z) in [(0, 0), (37, -91), (-1500, -1056), (1_000_000, -1_000_000)] {
            assert_eq!(first.column_shape(x, z), second.column_shape(x, z));
        }
    }

    #[test]
    fn another_seed_gives_another_shape() {
        let first = TerrainNoises::new(42);
        let second = TerrainNoises::new(43);
        let differs =
            (0..64).any(|x| first.column_shape(x * 16, 0) != second.column_shape(x * 16, 0));
        assert!(differs);
    }

    #[test]
    fn a_biome_spans_about_a_hundred_and_fifty_blocks() {
        // Along a long line, the width of the biome each column stands in, averaged over the
        // columns: the thin bands where two biomes meet weigh as little as they cover.
        let terrain = TerrainNoises::new(42);
        let biomes: Vec<BiomeType> = (-4096..4096)
            .map(|x| terrain.column_shape(x, 300).biome)
            .collect();
        let runs: Vec<f64> = biomes
            .chunk_by(|a, b| a == b)
            .map(|run| run.len() as f64)
            .collect();
        let width = runs.iter().map(|r| r * r).sum::<f64>() / runs.iter().sum::<f64>();
        assert!(width > 120.0, "{width:.0} blocks");
    }

    #[test]
    fn the_ground_never_steps_at_the_border_of_two_biomes() {
        let terrain = TerrainNoises::new(42);
        for z in (-1024..1024).step_by(64) {
            let heights: Vec<i32> = (-1024..1024)
                .map(|x| terrain.column_shape(x, z).height)
                .collect();
            for (x, pair) in (-1024..).zip(heights.windows(2)) {
                assert!((pair[0] - pair[1]).abs() <= 4, "({x}, {z}): {pair:?}");
            }
        }
    }

    #[test]
    fn neighbouring_seeds_give_unrelated_worlds() {
        // the noises of seed n + 1 must not be those of seed n with another salt or octave
        let first = TerrainNoises::new(42);
        let second = TerrainNoises::new(43);
        let same = (0..1000)
            .filter(|i| {
                let (x, z) = (i * 37 % 2000, i * 91 % 2000);
                first.column_shape(x, z).height == second.column_shape(x, z).height
            })
            .count();
        assert!(same < 300, "{same} of 1000 columns have the same height");
    }
}
