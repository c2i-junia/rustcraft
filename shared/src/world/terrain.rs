use noise::{NoiseFn, SuperSimplex};

use super::{
    biome::{JUNGLE_HUMIDITY, JUNGLE_TEMPERATURE, OCEAN_HUMIDITY},
    determine_biome, splitmix64, BiomeType,
};

/// Water fills every column up to this height.
pub const SEA_LEVEL: i32 = 62;

/// From this height the mountains are snowy, give or take a few blocks from a column to the
/// next, and what falls from the sky is snow: the server's generation, the client's sounds and
/// its shaders (`chunk.wgsl`, `precipitation.wgsl`, held to it by a test) all go by it.
pub const SNOW_LINE: i32 = 122;

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

/// Where the mountain ranges stand: along the lines where a slow noise crosses zero, so that
/// they form long winding chains. `RANGES_FROM` and `RANGES_FULL` are values of
/// `1 - |noise|`: they set how wide a chain is. The ranges stay inland, fading out as the
/// humidity nears that of the coast.
const RANGES_WAVELENGTH: f64 = 1600.0;
const RANGES_FROM: f64 = 0.76;
const RANGES_FULL: f64 = 0.93;
const RANGES_INLAND_HUMIDITY: f64 = 0.3;
/// Height by which a whole range lifts the land, then height of its ridges above that.
const RANGES_LIFT: f64 = 14.0;
const RIDGES_HEIGHT: f64 = 80.0;
const RIDGES_WAVELENGTH: f64 = 320.0;

/// Rivers run along the lines where a noise crosses zero, `|noise|` measuring how far a
/// column is from one: below `RIVER_BED` the bed lies `RIVER_DEPTH` under the sea level, and
/// the banks rise to just above the sea at `RIVER_BANK`. Beyond, the land may rise no higher
/// than `VALLEY_SLOPE` per unit of that measure: high land is cut by a wide valley, low land
/// keeps its shape. Their water is the sea's: it stays at the sea level.
const RIVERS_WAVELENGTH: f64 = 500.0;
const RIVER_BED: f64 = 0.012;
const RIVER_BANK: f64 = 0.025;
const RIVER_DEPTH: f64 = 4.0;
const VALLEY_SLOPE: f64 = 300.0;

/// Lakes fill bowls around the peaks of a noise: `LAKE_DEPTH` under the sea level where it
/// reaches `LAKES_FULL`, the land around rising no higher than `LAKE_SLOPE` per unit of it
/// below, and the bed sinking `LAKE_DEEPENING` per unit of it above, so that the middle of a
/// big lake is deep. A bowl dug in high land stays dry, a smooth hollow.
const LAKES_WAVELENGTH: f64 = 200.0;
const LAKES_FULL: f64 = 0.55;
const LAKE_DEPTH: f64 = 5.0;
const LAKE_SLOPE: f64 = 150.0;
const LAKE_DEEPENING: f64 = 80.0;

/// Ground under the water rises and sinks by up to `BED_ROUGHNESS` with a noise, fully once it
/// lies `BED_ROUGHNESS_FROM` under the sea level and less above, so that no bed is flat and
/// no bump reaches the surface.
const BED_WAVELENGTH: f64 = 40.0;
const BED_ROUGHNESS: f64 = 5.0;
const BED_ROUGHNESS_FROM: f64 = 10.0;

/// The jungle is at its deepest from this temperature and this humidity on: its depth rises
/// from 0 at its border to 1 there.
const JUNGLE_FULL_TEMPERATURE: f64 = 0.75;
const JUNGLE_FULL_HUMIDITY: f64 = 0.62;

/// The mountain biomes are altitude bands: from these heights, whatever the climate.
pub const MOUNTAIN_FROM: i32 = 84;
pub const HIGH_MOUNTAIN_FROM: i32 = 100;

/// The sea floor sinks from just below the sea at the coast to this depth below it, as the
/// humidity rises from the coast to its maximum.
const OCEAN_DEPTH: f64 = 28.0;
/// Humidity over which the land slopes down into the sea, on each side of the coast.
const COAST_WIDTH: f64 = 0.025;

/// No surface lies lower than this: every noise is within `[-1, 1]`, and the deepest ground is
/// the middle of a lake at the peak of its noise, with its bed lowered by the whole roughness.
/// The sea floor and the river beds stop higher (`surface_bounds_hold` checks them all).
pub const LOWEST_SURFACE: i32 =
    (SEA_LEVEL as f64 - LAKE_DEPTH - LAKE_DEEPENING * (1.0 - LAKES_FULL) - BED_ROUGHNESS) as i32;
/// No surface lies higher than this: the highest hill of the roughest land, under a mountain
/// range at its fullest, on its highest ridge. Rivers, lakes and the sea only lower the land.
pub const HIGHEST_SURFACE: i32 = (LAND_HEIGHT + HILLS_ROUGH + RANGES_LIFT + RIDGES_HEIGHT) as i32;

// Salts: each noise of a world takes its seed from `(seed, salt, octave)`.
const SALT_TEMPERATURE: u64 = 1;
const SALT_HUMIDITY: u64 = 2;
const SALT_WARP_X: u64 = 3;
const SALT_WARP_Z: u64 = 4;
const SALT_HILLS: u64 = 5;
const SALT_ROUGHNESS: u64 = 6;
const SALT_RANGES: u64 = 7;
const SALT_RIDGES: u64 = 8;
const SALT_RIVERS: u64 = 9;
const SALT_LAKES: u64 = 10;
const SALT_BEDS: u64 = 11;

/// What the noises decide for one world column, before it is filled with blocks.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ColumnShape {
    /// Height of the surface block.
    pub height: i32,
    pub biome: BiomeType,
    /// How far the column lies inside the jungle, in `[0, 1]`: 0 at its border and outside it,
    /// 1 at its heart.
    pub jungle_depth: f32,
}

/// Octaves of SuperSimplex noise, each with twice the frequency and half the weight of the
/// previous one, averaged so that the sum stays within `[-1, 1]`.
struct Fractal {
    /// Each octave and the offset of its coordinates: a noise is zero at its origin whatever
    /// its seed, which would make the origin of every world alike.
    octaves: Vec<(SuperSimplex, [f64; 2])>,
    frequency: f64,
}

impl Fractal {
    fn new(seed: u32, salt: u64, octaves: u64, wavelength: f64) -> Self {
        let octaves = (0..octaves)
            .map(|octave| {
                let hash = splitmix64(splitmix64(splitmix64(seed as u64) ^ salt) ^ octave);
                // two offsets in [0, 1024) from the low bits, which the noise seed leaves aside
                let offset = |bits: u64| (bits & 0xffff) as f64 / 64.0;
                let offset = [offset(hash), offset(hash >> 16)];
                (SuperSimplex::new((hash >> 32) as u32), offset)
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
        for (octave, [dx, dz]) in &self.octaves {
            sum += weight * octave.get([x * frequency + dx, z * frequency + dz]);
            total += weight;
            weight *= 0.5;
            frequency *= 2.0;
        }
        sum / total
    }

    /// Like `get`, but each octave folded into ridges: `(1 - |noise|)²`, highest where the
    /// noise crosses zero. In `[0, 1]`.
    fn ridged(&self, x: f64, z: f64) -> f64 {
        let (mut sum, mut total) = (0.0, 0.0);
        let (mut weight, mut frequency) = (1.0, self.frequency);
        for (octave, [dx, dz]) in &self.octaves {
            let ridge = 1.0 - octave.get([x * frequency + dx, z * frequency + dz]).abs();
            sum += weight * ridge * ridge;
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

/// How far a column of the jungle lies inside it, from its climate: 0 where the temperature or
/// the humidity is at the jungle's threshold, 1 once both are well past it.
fn jungle_depth(climate: &Climate) -> f32 {
    let hot = smoothstep(
        JUNGLE_TEMPERATURE,
        JUNGLE_FULL_TEMPERATURE,
        climate.temperature,
    );
    let wet = smoothstep(JUNGLE_HUMIDITY, JUNGLE_FULL_HUMIDITY, climate.humidity);
    hot.min(wet) as f32
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
    ranges: Fractal,
    ridges: Fractal,
    rivers: Fractal,
    lakes: Fractal,
    beds: Fractal,
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
            ranges: Fractal::new(seed, SALT_RANGES, 2, RANGES_WAVELENGTH),
            ridges: Fractal::new(seed, SALT_RIDGES, 4, RIDGES_WAVELENGTH),
            rivers: Fractal::new(seed, SALT_RIVERS, 3, RIVERS_WAVELENGTH),
            lakes: Fractal::new(seed, SALT_LAKES, 2, LAKES_WAVELENGTH),
            beds: Fractal::new(seed, SALT_BEDS, 3, BED_WAVELENGTH),
        }
    }

    /// The shape of the world column `(x, z)`: a pure function of the seed and the coordinates.
    pub fn column_shape(&self, x: i32, z: i32) -> ColumnShape {
        let (x, z) = (x as f64, z as f64);
        let climate = self.climate(x, z);
        let height = self.height(x, z, &climate).round() as i32;
        let biome = if height >= HIGH_MOUNTAIN_FROM {
            BiomeType::HighMountainGrass
        } else if height >= MOUNTAIN_FROM {
            BiomeType::MediumMountain
        } else {
            determine_biome(climate.temperature, climate.humidity)
        };
        let jungle_depth = if biome == BiomeType::Jungle {
            jungle_depth(&climate)
        } else {
            0.0
        };
        ColumnShape {
            height,
            biome,
            jungle_depth,
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
        let mut land = LAND_HEIGHT + hills * lerp(HILLS_FLAT, HILLS_ROUGH, roughness);

        let humidity = climate.humidity;
        let inland = 1.0
            - smoothstep(
                OCEAN_HUMIDITY - RANGES_INLAND_HUMIDITY,
                OCEAN_HUMIDITY - COAST_WIDTH,
                humidity,
            );
        let chain = 1.0 - self.ranges.get(x, z).abs();
        let range = smoothstep(RANGES_FROM, RANGES_FULL, chain) * inland;
        if range > 0.0 {
            land += range * (RANGES_LIFT + RIDGES_HEIGHT * self.ridges.ridged(x, z));
        }

        // Rivers and lakes sink the land, fading out as it rises into the mountains
        let lowland = 1.0 - range;
        let sea = SEA_LEVEL as f64;
        let river = self.rivers.get(x, z).abs();
        let valley = if river < RIVER_BANK {
            lerp(
                sea - RIVER_DEPTH,
                sea + 1.0,
                smoothstep(RIVER_BED, RIVER_BANK, river),
            )
        } else {
            sea + 1.0 + VALLEY_SLOPE * (river - RIVER_BANK)
        };
        land = lerp(land, land.min(valley), lowland);
        let lake = LAKES_FULL - self.lakes.get(x, z);
        let bowl = sea - LAKE_DEPTH
            + lake
                * if lake > 0.0 {
                    LAKE_SLOPE
                } else {
                    LAKE_DEEPENING
                };
        land = lerp(land, land.min(bowl), lowland);

        let depth = smoothstep(OCEAN_HUMIDITY, 1.0, humidity);
        let sea_floor = SEA_LEVEL as f64 - 2.0 - OCEAN_DEPTH * depth + 3.0 * hills;

        let sea = smoothstep(
            OCEAN_HUMIDITY - COAST_WIDTH,
            OCEAN_HUMIDITY + COAST_WIDTH,
            humidity,
        );
        let ground = lerp(land, sea_floor, sea);

        let depth = SEA_LEVEL as f64 - ground;
        if depth > 0.0 {
            let roughness = BED_ROUGHNESS * smoothstep(0.0, BED_ROUGHNESS_FROM, depth);
            ground + roughness * self.beds.get(x, z)
        } else {
            ground
        }
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
        // only the mountains may be steeper
        let terrain = TerrainNoises::new(42);
        for z in (-1024..1024).step_by(64) {
            let heights: Vec<i32> = (-1024..1024)
                .map(|x| terrain.column_shape(x, z).height)
                .collect();
            for (x, pair) in (-1024..).zip(heights.windows(2)) {
                if pair[0].max(pair[1]) < MOUNTAIN_FROM {
                    assert!((pair[0] - pair[1]).abs() <= 4, "({x}, {z}): {pair:?}");
                }
            }
        }
    }

    #[test]
    fn mountains_rise_far_above_the_plains_and_are_named_by_their_altitude() {
        let terrain = TerrainNoises::new(42);
        let mut highest = i32::MIN;
        for z in (-1024..1024).step_by(4) {
            for x in (-1024..1024).step_by(4) {
                let shape = terrain.column_shape(x, z);
                highest = highest.max(shape.height);
                let mountain = matches!(
                    shape.biome,
                    BiomeType::MediumMountain | BiomeType::HighMountainGrass
                );
                assert_eq!(
                    mountain,
                    shape.height >= MOUNTAIN_FROM,
                    "({x}, {z}): {shape:?}"
                );
                if shape.height >= HIGH_MOUNTAIN_FROM {
                    assert_eq!(shape.biome, BiomeType::HighMountainGrass);
                }
            }
        }
        assert!(highest >= 140, "the highest column is at {highest}");
    }

    #[test]
    fn surface_bounds_hold() {
        let sea = SEA_LEVEL as f64;
        let deepest_sea_floor = sea - 2.0 - OCEAN_DEPTH - 3.0 - BED_ROUGHNESS;
        let deepest_river = sea - RIVER_DEPTH - BED_ROUGHNESS;
        let lowest_hill = LAND_HEIGHT - HILLS_ROUGH;
        for lowest in [deepest_sea_floor, deepest_river, lowest_hill] {
            assert!(LOWEST_SURFACE as f64 <= lowest, "{lowest}");
        }
        let terrain = TerrainNoises::new(42);
        let (mut lowest, mut highest) = (i32::MAX, i32::MIN);
        for z in (-2048..2048).step_by(8) {
            for x in (-2048..2048).step_by(8) {
                let height = terrain.column_shape(x, z).height;
                lowest = lowest.min(height);
                highest = highest.max(height);
            }
        }
        assert!(
            (LOWEST_SURFACE..=HIGHEST_SURFACE).contains(&lowest)
                && (LOWEST_SURFACE..=HIGHEST_SURFACE).contains(&highest),
            "{lowest} to {highest}"
        );
    }

    /// Columns of the seed 42 every `step` blocks over a square of `side` blocks around the
    /// origin, with their climate and shape.
    fn sample(step: usize, side: i32) -> Vec<(Climate, ColumnShape)> {
        let terrain = TerrainNoises::new(42);
        let mut columns = Vec::new();
        for z in (-side / 2..side / 2).step_by(step) {
            for x in (-side / 2..side / 2).step_by(step) {
                let climate = terrain.climate(x as f64, z as f64);
                columns.push((climate, terrain.column_shape(x, z)));
            }
        }
        columns
    }

    #[test]
    fn the_jungle_lies_only_where_hot_and_wet_on_low_land() {
        let mut jungle = 0;
        for (climate, shape) in sample(16, 4096) {
            let Climate {
                temperature,
                humidity,
            } = climate;
            let expected = temperature > JUNGLE_TEMPERATURE
                && humidity > JUNGLE_HUMIDITY
                && humidity <= OCEAN_HUMIDITY
                && shape.height < MOUNTAIN_FROM;
            assert_eq!(
                shape.biome == BiomeType::Jungle,
                expected,
                "{temperature:.3}, {humidity:.3}: {shape:?}"
            );
            jungle += expected as usize;
        }
        assert!(jungle > 100, "{jungle} jungle columns");
    }

    #[test]
    fn the_jungle_covers_about_six_percent_of_the_world() {
        let columns = sample(32, 6144);
        let jungle = columns
            .iter()
            .filter(|(_, shape)| shape.biome == BiomeType::Jungle)
            .count();
        let share = jungle as f64 / columns.len() as f64;
        assert!((0.05..0.07).contains(&share), "{:.1} %", 100.0 * share);
    }

    #[test]
    fn the_jungle_deepens_from_its_border_to_its_heart() {
        let terrain = TerrainNoises::new(42);
        let (mut borders, mut deepest) = (0, 0.0f32);
        for z in (-256..2816).step_by(32) {
            let shapes: Vec<ColumnShape> =
                (-1536..1536).map(|x| terrain.column_shape(x, z)).collect();
            for (x, pair) in (-1536..).zip(shapes.windows(2)) {
                for (shape, other) in [(pair[0], pair[1]), (pair[1], pair[0])] {
                    if shape.biome != BiomeType::Jungle {
                        assert_eq!(shape.jungle_depth, 0.0, "({x}, {z}): {shape:?}");
                        continue;
                    }
                    assert!((0.0..=1.0).contains(&shape.jungle_depth), "{shape:?}");
                    deepest = deepest.max(shape.jungle_depth);
                    // the border of the climate, not that of the sea or of the mountains
                    let land = [
                        BiomeType::Forest,
                        BiomeType::Desert,
                        BiomeType::Plains,
                        BiomeType::FlowerPlains,
                    ];
                    if land.contains(&other.biome) {
                        borders += 1;
                        assert!(shape.jungle_depth < 0.1, "({x}, {z}): {shape:?}");
                    }
                }
            }
        }
        assert!(borders > 20, "{borders} columns on a border");
        assert_eq!(deepest, 1.0);
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
