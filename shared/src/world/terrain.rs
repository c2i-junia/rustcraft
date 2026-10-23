use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use bevy::math::IVec2;
use noise::{NoiseFn, SuperSimplex};
use serde::{Deserialize, Serialize};

pub use super::biome::vanilla::{COAST, HIGH_MOUNTAIN_FROM, MOUNTAIN_FROM};
use super::{splitmix64, BiomeId, Biomes, ClimatePoint};

/// Water fills every column up to this height.
pub const SEA_LEVEL: i32 = 62;

/// From this height the mountains are snowy, give or take a few blocks from a column to the
/// next, and what falls from the sky is snow: the server's generation, the client's sounds and
/// its shaders (`chunk.wgsl`, `precipitation.wgsl`, held to it by a test) all go by it.
pub const SNOW_LINE: i32 = 122;

/// The climate parameters: their distance from one extreme to the other, which sets the size of
/// the biomes, and their octaves. Each is spread evenly over `[-1, 1]` (`Remap`).
const TEMPERATURE_WAVELENGTH: f64 = 3600.0;
const HUMIDITY_WAVELENGTH: f64 = 2800.0;
const CONTINENTALNESS_WAVELENGTH: f64 = 3600.0;
const EROSION_WAVELENGTH: f64 = 1400.0;
const WEIRDNESS_WAVELENGTH: f64 = 1000.0;
/// The weight of each octave of a parameter against the previous one. The temperature and the
/// humidity keep to the usual half, so that the climate changes over large regions; the others
/// take more, so that their fine octaves fray the borders of the biomes they set.
const CLIMATE_PERSISTENCE: f64 = 0.5;
const SHAPE_PERSISTENCE: f64 = 0.6;

/// Shares the climates several biomes claim between them (`Biomes::pick`), in patches this wide.
const SELECTOR_WAVELENGTH: f64 = 600.0;
/// Picks the sub-biomes, in patches this wide, deep enough inside their parent.
const PATCHES_WAVELENGTH: f64 = 90.0;
/// How deep inside its parent a column must be for a sub-biome to take it.
const SUB_BIOME_DEPTH: f32 = 0.6;
/// How far inside a biome its heart starts, in units of the parameters: `depth_in_biome` grows
/// from 0 on its border to 1 there.
const HEART_MARGIN: f64 = 0.15;

/// The climate is read away from the column, moved by three layers of warp, the coarsest
/// first, each read where the previous one led: (wavelength, amplitude in blocks, octaves). The
/// coarse layers bend whole biomes into arms and bays, the fine one frays their borders.
const WARPS: [(f64, f64, u64); 3] = [(1600.0, 320.0, 3), (250.0, 60.0, 3), (48.0, 8.0, 2)];
/// The climate is computed on the corners of cells of this many blocks, and interpolated
/// within them: it changes over tens of blocks at least, and the warps and the parameters are
/// most of the cost of a column.
const CLIMATE_CELL: i32 = 4;
/// The climate a biome is picked by is moved by up to this much, by a white noise, on each
/// parameter but the continentalness: across a border, columns of either biome mingle.
const DITHER: f64 = 0.02;

/// Height of the land around which the hills rise and sink, and how far it rises from there
/// far inland.
const LAND_HEIGHT: f64 = 67.0;
const INLAND_RISE: f64 = 6.0;
const HILLS_WAVELENGTH: f64 = 180.0;
/// How far the hills rise above and sink below the land, where the erosion is highest and where
/// it is lowest.
const HILLS_FLAT: f64 = 4.0;
const HILLS_ROUGH: f64 = 16.0;

/// Where the mountain ranges stand: along the lines where a slow noise crosses zero, so that
/// they form long winding chains. `RANGES_FROM` and `RANGES_FULL` are values of
/// `1 - |noise|`: they set how wide a chain is. The ranges stay inland, fading out toward the
/// coast, and sink where the land is eroded.
const RANGES_WAVELENGTH: f64 = 1600.0;
const RANGES_FROM: f64 = 0.76;
const RANGES_FULL: f64 = 0.93;
/// The ranges fade in over this much continentalness from the coast inland.
const RANGES_INLAND: (f64, f64) = (0.1, 0.35);
/// The ranges are whole up to the first erosion and gone from the second.
const RANGES_EROSION: (f64, f64) = (0.1, 0.75);
/// Height by which a whole range lifts the land, then height of its ridges above that.
const RANGES_LIFT: f64 = 14.0;
const RIDGES_HEIGHT: f64 = 80.0;
const RIDGES_WAVELENGTH: f64 = 320.0;

/// Rivers run along the lines where a noise crosses zero, `|noise|` measuring how far a
/// column is from one: below `RIVER_BED` the bed lies `RIVER_DEPTH` under the sea level, and
/// the banks rise evenly to just above the sea at `RIVER_BANK`. Beyond, the land may rise no higher
/// than `VALLEY_SLOPE` per unit of that measure: high land is cut by a wide valley, low land
/// keeps its shape. Their water is the sea's: it stays at the sea level.
const RIVERS_WAVELENGTH: f64 = 500.0;
const RIVER_BED: f64 = 0.010;
const RIVER_BANK: f64 = 0.030;
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

/// The sea floor sinks from just below the sea at the coast to this depth below it, as the
/// continentalness falls from the coast to its lowest.
const OCEAN_DEPTH: f64 = 28.0;
/// Continentalness over which the land slopes down into the sea, on each side of the coast.
const COAST_WIDTH: f64 = 0.05;

/// How far the archipelago shifts the continentalness down, toward the sea.
const ARCHIPELAGO_SHIFT: f64 = 0.35;
/// How much larger the biomes of the large biomes are.
const LARGE_BIOMES_SCALE: f64 = 2.0;

/// No surface lies lower than this: every noise is within `[-1, 1]`, and the deepest ground is
/// the middle of a lake at the peak of its noise, with its bed lowered by the whole roughness.
/// The sea floor and the river beds stop higher (`surface_bounds_hold` checks them all).
pub const LOWEST_SURFACE: i32 =
    (SEA_LEVEL as f64 - LAKE_DEPTH - LAKE_DEEPENING * (1.0 - LAKES_FULL) - BED_ROUGHNESS) as i32;
/// No surface lies higher than this: the highest hill of the roughest land far inland, under a
/// mountain range at its fullest, on its highest ridge. Rivers, lakes and the sea only lower
/// the land.
pub const HIGHEST_SURFACE: i32 =
    (LAND_HEIGHT + INLAND_RISE + HILLS_ROUGH + RANGES_LIFT + RIDGES_HEIGHT) as i32;

// Salts: each noise of a world takes its seed from `(seed, salt, octave)`.
const SALT_TEMPERATURE: u64 = 1;
const SALT_HUMIDITY: u64 = 2;
const SALT_HILLS: u64 = 5;
const SALT_RANGES: u64 = 7;
const SALT_RIDGES: u64 = 8;
const SALT_RIVERS: u64 = 9;
const SALT_LAKES: u64 = 10;
const SALT_BEDS: u64 = 11;
const SALT_CONTINENTALNESS: u64 = 12;
const SALT_EROSION: u64 = 13;
const SALT_WEIRDNESS: u64 = 14;
const SALT_SELECTOR: u64 = 15;
const SALT_PATCHES: u64 = 16;
/// Each layer of warp takes two salts from here, one per axis.
const SALT_WARP: u64 = 20;
const SALT_DITHER: u64 = 30;

/// How the world of a seed is shaped beyond its seed: chosen when it is created, saved with it,
/// and sent to the clients that join it.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct WorldGenSettings {
    pub preset: WorldPreset,
}

/// The kinds of worlds.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub enum WorldPreset {
    #[default]
    Default,
    /// Biomes twice as large, and continents with them.
    LargeBiomes,
    /// Islands scattered over a wide sea.
    Archipelago,
    /// One biome everywhere, by its key.
    SingleBiome(String),
}

impl WorldPreset {
    /// The names a world type is typed as: `single_biome` is followed by `:<biome>`.
    pub const NAMES: [&'static str; 4] = ["default", "large_biomes", "archipelago", "single_biome"];

    /// `default`, `large_biomes`, `archipelago` or `single_biome:<biome>`.
    pub fn name(&self) -> String {
        match self {
            WorldPreset::Default => "default".into(),
            WorldPreset::LargeBiomes => "large_biomes".into(),
            WorldPreset::Archipelago => "archipelago".into(),
            WorldPreset::SingleBiome(key) => format!("single_biome:{key}"),
        }
    }

    /// The preset named `name`, as `name` writes it; the biome of a single biome is checked
    /// against the biomes of the world when it is built.
    pub fn from_name(name: &str) -> Option<Self> {
        match name.trim() {
            "" | "default" => Some(WorldPreset::Default),
            "large_biomes" => Some(WorldPreset::LargeBiomes),
            "archipelago" => Some(WorldPreset::Archipelago),
            other => other
                .strip_prefix("single_biome:")
                .filter(|key| !key.is_empty())
                .map(|key| WorldPreset::SingleBiome(key.to_string())),
        }
    }
}

/// What the noises decide for one world column, before it is filled with blocks.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ColumnShape {
    /// Height of the surface block.
    pub height: i32,
    pub biome: BiomeId,
    /// How far the column lies inside its biome, in `[0, 1]`: 0 at its border, 1 at its heart.
    pub depth_in_biome: f32,
}

/// Octaves of SuperSimplex noise, each with twice the frequency of the previous one and
/// `persistence` times its weight, averaged so that the sum stays within `[-1, 1]`.
struct Fractal {
    /// Each octave and the offset of its coordinates: a noise is zero at its origin whatever
    /// its seed, which would make the origin of every world alike.
    octaves: Vec<(SuperSimplex, [f64; 2])>,
    frequency: f64,
    persistence: f64,
}

impl Fractal {
    fn new(seed: u32, salt: u64, octaves: u64, wavelength: f64) -> Self {
        Self::with_persistence(seed, salt, octaves, wavelength, 0.5)
    }

    fn with_persistence(
        seed: u32,
        salt: u64,
        octaves: u64,
        wavelength: f64,
        persistence: f64,
    ) -> Self {
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
            persistence,
        }
    }

    fn get(&self, x: f64, z: f64) -> f64 {
        let (mut sum, mut total) = (0.0, 0.0);
        let (mut weight, mut frequency) = (1.0, self.frequency);
        for (octave, [dx, dz]) in &self.octaves {
            sum += weight * octave.get([x * frequency + dx, z * frequency + dz]);
            total += weight;
            weight *= self.persistence;
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
            weight *= self.persistence;
            frequency *= 2.0;
        }
        sum / total
    }
}

/// Quantiles of the values of a `Fractal` of some octaves and persistence: a sum of octaves
/// crowds around 0, this spreads it evenly over `[-1, 1]`. The quantiles are those of any seed.
struct Remap {
    quantiles: Vec<f64>,
}

/// Steps of `Remap::quantiles`.
const QUANTILES: usize = 128;

impl Remap {
    /// The remap of the fractals of `octaves` and `persistence`, measured once per process.
    fn of(octaves: u64, persistence: f64) -> Arc<Remap> {
        type Cache = Mutex<HashMap<(u64, u64), Arc<Remap>>>;
        static CACHE: OnceLock<Cache> = OnceLock::new();
        let cache = CACHE.get_or_init(Mutex::default);
        let key = (octaves, persistence.to_bits());
        if let Some(remap) = cache.lock().unwrap().get(&key) {
            return remap.clone();
        }
        let remap = Arc::new(Remap::measure(octaves, persistence));
        cache.lock().unwrap().insert(key, remap.clone());
        remap
    }

    fn measure(octaves: u64, persistence: f64) -> Remap {
        let fractal = Fractal::with_persistence(0, 0xCDF, octaves, 1.0, persistence);
        // points far enough apart, in wavelengths, for their values to be unrelated
        let mut values: Vec<f64> = (0..256 * 256)
            .map(|i| fractal.get((i % 256) as f64 * 3.71, (i / 256) as f64 * 2.93))
            .collect();
        values.sort_by(f64::total_cmp);
        let last = values.len() - 1;
        let quantiles = (0..=QUANTILES)
            .map(|k| values[k * last / QUANTILES])
            .collect();
        Remap { quantiles }
    }

    /// `value` of a fractal, spread over `[-1, 1]`.
    fn apply(&self, value: f64) -> f64 {
        let q = &self.quantiles;
        let above = q.partition_point(|&quantile| quantile < value);
        let t = if above == 0 {
            0.0
        } else if above > QUANTILES {
            QUANTILES as f64
        } else {
            let (low, high) = (q[above - 1], q[above]);
            let within = if high > low {
                (value - low) / (high - low)
            } else {
                0.0
            };
            (above - 1) as f64 + within
        };
        -1.0 + 2.0 * t / QUANTILES as f64
    }
}

/// A fractal spread evenly over `[-1, 1]`.
struct Parameter {
    fractal: Fractal,
    remap: Arc<Remap>,
}

impl Parameter {
    fn new(seed: u32, salt: u64, octaves: u64, wavelength: f64, persistence: f64) -> Self {
        Self {
            fractal: Fractal::with_persistence(seed, salt, octaves, wavelength, persistence),
            remap: Remap::of(octaves, persistence),
        }
    }

    fn get(&self, x: f64, z: f64) -> f64 {
        self.remap.apply(self.fractal.get(x, z))
    }

    /// The value in `[0, 1]`.
    fn unit(&self, x: f64, z: f64) -> f64 {
        (0.5 + 0.5 * self.get(x, z)).clamp(0.0, 0.999_999)
    }
}

/// `0` up to `edge0`, `1` from `edge1`, and a smooth S-curve in between; the other way round
/// when `edge1` is below `edge0`.
fn smoothstep(edge0: f64, edge1: f64, value: f64) -> f64 {
    let t = ((value - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// The climate of the column `(x, z)`, interpolated between the corners of its cell, which
/// `corner` gives by their place on the grid of cells. A corner that weighs nothing is not
/// read, so a column on a corner reads only that one.
fn interpolated(x: i32, z: i32, corner: impl Fn(i32, i32) -> ClimatePoint) -> ClimatePoint {
    let (gx, gz) = (x.div_euclid(CLIMATE_CELL), z.div_euclid(CLIMATE_CELL));
    let cell = f64::from(CLIMATE_CELL);
    let (tx, tz) = (
        f64::from(x.rem_euclid(CLIMATE_CELL)) / cell,
        f64::from(z.rem_euclid(CLIMATE_CELL)) / cell,
    );
    let row = |gz: i32| {
        let near = corner(gx, gz);
        if tx == 0.0 {
            near
        } else {
            mix(&near, &corner(gx + 1, gz), tx)
        }
    };
    let near = row(gz);
    if tz == 0.0 {
        near
    } else {
        mix(&near, &row(gz + 1), tz)
    }
}

/// `a` moved toward `b` by `t`, parameter by parameter.
fn mix(a: &ClimatePoint, b: &ClimatePoint, t: f64) -> ClimatePoint {
    ClimatePoint {
        temperature: lerp(a.temperature, b.temperature, t),
        humidity: lerp(a.humidity, b.humidity, t),
        continentalness: lerp(a.continentalness, b.continentalness, t),
        erosion: lerp(a.erosion, b.erosion, t),
        weirdness: lerp(a.weirdness, b.weirdness, t),
        depth: lerp(a.depth, b.depth, t),
    }
}

fn lerp(from: f64, to: f64, t: f64) -> f64 {
    from + (to - from) * t
}

/// The height of a column, and whether a river runs over it.
struct Ground {
    height: f64,
    river: bool,
}

/// Every noise the shape of a world depends on, built once from its seed, its settings and its
/// biomes. The server generates the terrain with them; the client, given the same, finds the
/// biome the player is in and the colour of the grass.
pub struct TerrainNoises {
    seed: u32,
    temperature: Parameter,
    humidity: Parameter,
    continentalness: Parameter,
    erosion: Parameter,
    weirdness: Parameter,
    selector: Parameter,
    patches: Parameter,
    warps: Vec<(Fractal, Fractal, f64)>,
    hills: Fractal,
    ranges: Fractal,
    ridges: Fractal,
    rivers: Fractal,
    lakes: Fractal,
    beds: Fractal,
    biomes: Arc<Biomes>,
    /// The climate is read at the coordinates times this.
    climate_scale: f64,
    continentalness_shift: f64,
    /// The one biome of a single biome world.
    single: Option<BiomeId>,
    river: Option<BiomeId>,
}

impl TerrainNoises {
    /// The noises of a default world of the base game.
    pub fn new(seed: u32) -> Self {
        Self::with(seed, &WorldGenSettings::default(), Biomes::vanilla())
    }

    /// The noises of the world of `seed` and `settings`, whose biomes are `biomes`. A single
    /// biome of a key not among them falls back to the default world.
    pub fn with(seed: u32, settings: &WorldGenSettings, biomes: Arc<Biomes>) -> Self {
        let warps = WARPS
            .iter()
            .enumerate()
            .map(|(i, &(wavelength, amplitude, octaves))| {
                let salt = SALT_WARP + 2 * i as u64;
                (
                    Fractal::new(seed, salt, octaves, wavelength),
                    Fractal::new(seed, salt + 1, octaves, wavelength),
                    amplitude,
                )
            })
            .collect();
        let single = match &settings.preset {
            WorldPreset::SingleBiome(key) => biomes.id(key),
            _ => None,
        };
        Self {
            seed,
            temperature: Parameter::new(
                seed,
                SALT_TEMPERATURE,
                4,
                TEMPERATURE_WAVELENGTH,
                CLIMATE_PERSISTENCE,
            ),
            humidity: Parameter::new(
                seed,
                SALT_HUMIDITY,
                4,
                HUMIDITY_WAVELENGTH,
                CLIMATE_PERSISTENCE,
            ),
            continentalness: Parameter::new(
                seed,
                SALT_CONTINENTALNESS,
                5,
                CONTINENTALNESS_WAVELENGTH,
                SHAPE_PERSISTENCE,
            ),
            erosion: Parameter::new(seed, SALT_EROSION, 4, EROSION_WAVELENGTH, SHAPE_PERSISTENCE),
            weirdness: Parameter::new(
                seed,
                SALT_WEIRDNESS,
                4,
                WEIRDNESS_WAVELENGTH,
                SHAPE_PERSISTENCE,
            ),
            selector: Parameter::new(seed, SALT_SELECTOR, 3, SELECTOR_WAVELENGTH, 0.5),
            patches: Parameter::new(seed, SALT_PATCHES, 3, PATCHES_WAVELENGTH, 0.5),
            warps,
            hills: Fractal::new(seed, SALT_HILLS, 4, HILLS_WAVELENGTH),
            ranges: Fractal::new(seed, SALT_RANGES, 2, RANGES_WAVELENGTH),
            ridges: Fractal::new(seed, SALT_RIDGES, 4, RIDGES_WAVELENGTH),
            rivers: Fractal::new(seed, SALT_RIVERS, 3, RIVERS_WAVELENGTH),
            lakes: Fractal::new(seed, SALT_LAKES, 2, LAKES_WAVELENGTH),
            beds: Fractal::new(seed, SALT_BEDS, 3, BED_WAVELENGTH),
            river: biomes.id("rustcraft:river"),
            biomes,
            climate_scale: if settings.preset == WorldPreset::LargeBiomes {
                1.0 / LARGE_BIOMES_SCALE
            } else {
                1.0
            },
            continentalness_shift: if settings.preset == WorldPreset::Archipelago {
                ARCHIPELAGO_SHIFT
            } else {
                0.0
            },
            single,
        }
    }

    /// The biomes of the world.
    pub fn biomes(&self) -> &Arc<Biomes> {
        &self.biomes
    }

    /// The shape of the world column `(x, z)`: a pure function of the seed, the settings, the
    /// biomes and the coordinates. `column_shapes` gives the same for many columns at once,
    /// faster.
    pub fn column_shape(&self, x: i32, z: i32) -> ColumnShape {
        let climate = interpolated(x, z, |gx, gz| self.corner_climate(gx, gz));
        self.shape(x, z, &climate)
    }

    /// The shapes of the `width` by `depth` columns from `(x0, z0)`, row by row along x: those
    /// `column_shape` gives, the climate of the corners of their cells computed once.
    pub fn column_shapes(&self, (x0, z0): (i32, i32), width: i32, depth: i32) -> Vec<ColumnShape> {
        if width <= 0 || depth <= 0 {
            return Vec::new();
        }
        let first = IVec2::new(x0, z0).div_euclid(IVec2::splat(CLIMATE_CELL));
        let last =
            IVec2::new(x0 + width - 1, z0 + depth - 1).div_euclid(IVec2::splat(CLIMATE_CELL));
        // one more corner past the last cell, which the columns of its far edge read
        let side = last - first + 2;
        let corners: Vec<ClimatePoint> = (0..side.x * side.y)
            .map(|i| self.corner_climate(first.x + i % side.x, first.y + i / side.x))
            .collect();
        let corner = |gx: i32, gz: i32| corners[((gz - first.y) * side.x + gx - first.x) as usize];
        (z0..z0 + depth)
            .flat_map(|z| (x0..x0 + width).map(move |x| (x, z)))
            .map(|(x, z)| self.shape(x, z, &interpolated(x, z, corner)))
            .collect()
    }

    /// The climate of the column `(x, z)`, as its biome is picked by.
    pub fn climate_at(&self, x: i32, z: i32) -> ClimatePoint {
        interpolated(x, z, |gx, gz| self.corner_climate(gx, gz))
    }

    /// The climate of the corner `(gx, gz)` of the cells of `CLIMATE_CELL` blocks.
    fn corner_climate(&self, gx: i32, gz: i32) -> ClimatePoint {
        let cell = f64::from(CLIMATE_CELL);
        self.climate(f64::from(gx) * cell, f64::from(gz) * cell)
    }

    fn shape(&self, x: i32, z: i32, climate: &ClimatePoint) -> ColumnShape {
        let ground = self.ground(x as f64, z as f64, climate);
        let height = ground.height.round() as i32;
        let (biome, depth_in_biome) = self.biome(x, z, climate, height, ground.river);
        ColumnShape {
            height,
            biome,
            depth_in_biome,
        }
    }

    fn climate(&self, x: f64, z: f64) -> ClimatePoint {
        let (mut x, mut z) = (x * self.climate_scale, z * self.climate_scale);
        for (warp_x, warp_z, amplitude) in &self.warps {
            let (dx, dz) = (warp_x.get(x, z), warp_z.get(x, z));
            x += amplitude * dx;
            z += amplitude * dz;
        }
        ClimatePoint {
            temperature: self.temperature.get(x, z),
            humidity: self.humidity.get(x, z),
            continentalness: (self.continentalness.get(x, z) - self.continentalness_shift)
                .clamp(-1.0, 1.0),
            erosion: self.erosion.get(x, z),
            weirdness: self.weirdness.get(x, z),
            depth: 0.0,
        }
    }

    /// The biome of the column `(x, z)` and how deep inside it the column is: the biome its
    /// climate falls in, its border or one of its sub-biomes, then the river over it or the
    /// variant of its altitude.
    fn biome(
        &self,
        x: i32,
        z: i32,
        climate: &ClimatePoint,
        height: i32,
        river: bool,
    ) -> (BiomeId, f32) {
        if let Some(single) = self.single {
            return (single, 1.0);
        }
        let biomes = &self.biomes;
        let (fx, fz) = (x as f64 * self.climate_scale, z as f64 * self.climate_scale);
        let selector = self.selector.unit(fx, fz);
        let plain = biomes.pick(climate, selector);
        let dithered = biomes.pick(&self.dithered(x, z, climate), selector);
        let dither = |pick: &super::Pick| biomes.get(pick.biome).dither;
        let pick = if dither(&plain) && dither(&dithered) {
            dithered
        } else {
            plain
        };
        let depth = smoothstep(0.0, HEART_MARGIN, pick.margin) as f32;
        let mut biome = pick.biome;
        if let Some((border, _)) = biomes
            .border(biome)
            .filter(|(_, w)| pick.border_margin < *w)
        {
            biome = border;
        } else if depth >= SUB_BIOME_DEPTH && !biomes.sub_biomes(biome).is_empty() {
            let patch = self.patches.unit(x as f64, z as f64);
            let mut start = 0.0;
            for &(sub_biome, share) in biomes.sub_biomes(biome) {
                start += share;
                if patch < start {
                    biome = sub_biome;
                    break;
                }
            }
        }
        if river && !biomes.get(biome).ground.frozen {
            if let Some(river) = self.river {
                return (river, depth);
            }
        }
        while let Some(&(_, variant)) = biomes
            .altitude_variants(biome)
            .iter()
            .find(|(from, _)| height >= *from)
        {
            biome = variant;
        }
        (biome, depth)
    }

    /// `climate`, moved on each parameter but the continentalness by a white noise of the
    /// column, up to `DITHER`.
    fn dithered(&self, x: i32, z: i32, climate: &ClimatePoint) -> ClimatePoint {
        let hash = splitmix64(
            splitmix64(splitmix64(self.seed as u64 ^ SALT_DITHER) ^ x as u32 as u64)
                ^ z as u32 as u64,
        );
        let jitter = |shift: u32| DITHER * ((hash >> shift & 0xffff) as f64 / 32767.5 - 1.0);
        ClimatePoint {
            temperature: climate.temperature + jitter(0),
            humidity: climate.humidity + jitter(16),
            erosion: climate.erosion + jitter(32),
            weirdness: climate.weirdness + jitter(48),
            ..*climate
        }
    }

    /// The land, made of hills over the continents, sinks smoothly into the sea floor across the
    /// coast, so that the ground never steps at the border of two biomes.
    fn ground(&self, x: f64, z: f64, climate: &ClimatePoint) -> Ground {
        let continentalness = climate.continentalness;
        let rough = smoothstep(0.55, -0.55, climate.erosion);
        let hills = self.hills.get(x, z);
        let mut land = LAND_HEIGHT
            + INLAND_RISE * smoothstep(0.1, 1.0, continentalness)
            + hills * lerp(HILLS_FLAT, HILLS_ROUGH, rough);

        let inland = smoothstep(
            COAST + RANGES_INLAND.0,
            COAST + RANGES_INLAND.1,
            continentalness,
        );
        let uneroded = smoothstep(RANGES_EROSION.1, RANGES_EROSION.0, climate.erosion);
        let chain = 1.0 - self.ranges.get(x, z).abs();
        let range = smoothstep(RANGES_FROM, RANGES_FULL, chain) * inland * uneroded;
        if range > 0.0 {
            land += range * (RANGES_LIFT + RIDGES_HEIGHT * self.ridges.ridged(x, z));
        }

        // Rivers and lakes sink the land, fading out as it rises into the mountains
        let lowland = 1.0 - range;
        let sea = SEA_LEVEL as f64;
        let river = self.rivers.get(x, z).abs();
        let valley = if river < RIVER_BANK {
            let bank = ((river - RIVER_BED) / (RIVER_BANK - RIVER_BED)).clamp(0.0, 1.0);
            lerp(sea - RIVER_DEPTH, sea + 1.0, bank)
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

        let depth = smoothstep(COAST, -1.0, continentalness);
        let sea_floor = sea - 2.0 - OCEAN_DEPTH * depth + 3.0 * hills;
        let coast = smoothstep(COAST + COAST_WIDTH, COAST - COAST_WIDTH, continentalness);
        let ground = lerp(land, sea_floor, coast);

        let depth = sea - ground;
        let height = if depth > 0.0 {
            let roughness = BED_ROUGHNESS * smoothstep(0.0, BED_ROUGHNESS_FROM, depth);
            ground + roughness * self.beds.get(x, z)
        } else {
            ground
        };
        Ground {
            height,
            river: river < RIVER_BANK && coast < 0.5 && height.round() < sea,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::{tags, vanilla::*};
    use super::*;

    #[test]
    fn the_shapes_of_an_area_are_those_of_its_columns() {
        let terrain = TerrainNoises::new(42);
        // across the origin, over cells cut by the edges of the area
        let (x0, z0, width, depth) = (-7, -10, 13, 9);
        let shapes = terrain.column_shapes((x0, z0), width, depth);
        assert_eq!(shapes.len(), (width * depth) as usize);
        for (i, shape) in shapes.iter().enumerate() {
            let (x, z) = (x0 + i as i32 % width, z0 + i as i32 / width);
            assert_eq!(*shape, terrain.column_shape(x, z), "({x}, {z})");
        }
        assert!(terrain.column_shapes((0, 0), 0, 5).is_empty());
    }

    #[test]
    fn the_climate_of_a_corner_is_its_own_and_moves_smoothly_between() {
        let terrain = TerrainNoises::new(42);
        let cell = CLIMATE_CELL as f64;
        assert_eq!(terrain.climate_at(-8, 12), terrain.climate(-8.0, 12.0));
        assert_eq!(terrain.climate_at(-8, 12), terrain.corner_climate(-2, 3));
        let (a, b) = (terrain.climate_at(-8, 12), terrain.climate_at(-4, 12));
        let half = terrain.climate_at(-6, 12);
        assert!((half.temperature - (a.temperature + b.temperature) / 2.0).abs() < 1e-12);
        // the interpolated climate stays near the one of the column itself
        for x in -40..40 {
            let (between, exact) = (terrain.climate_at(x, 7), terrain.climate(x as f64, 7.0));
            let error = (between.temperature - exact.temperature).abs();
            assert!(error < 0.02 * cell, "{x}: {error}");
        }
    }

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
    fn every_climate_parameter_is_spread_evenly() {
        let terrain = TerrainNoises::new(42);
        let mut counts = [[0usize; 4]; 5];
        let mut samples = 0;
        for z in (-20_000..20_000).step_by(397) {
            for x in (-20_000..20_000).step_by(401) {
                let axes = terrain.climate_at(x, z).axes();
                for (parameter, count) in counts.iter_mut().enumerate() {
                    let quarter = (((axes[parameter] + 1.0) * 2.0) as usize).min(3);
                    count[quarter] += 1;
                }
                samples += 1;
            }
        }
        for (parameter, count) in counts.iter().enumerate() {
            for quarter in count {
                let share = *quarter as f64 / samples as f64;
                assert!(
                    (0.17..0.33).contains(&share),
                    "parameter {parameter}: {count:?}"
                );
            }
        }
    }

    /// The biome `id` belongs to: its parent for a sub-biome or a border, itself otherwise.
    fn parent(biomes: &Biomes, id: BiomeId) -> BiomeId {
        biomes
            .ids()
            .find(|&parent| {
                biomes.sub_biomes(parent).iter().any(|(sub, _)| *sub == id)
                    || biomes
                        .border(parent)
                        .is_some_and(|(border, _)| border == id)
            })
            .unwrap_or(id)
    }

    /// The width of the biome each column stands in, along lines of `length` blocks every
    /// `spacing`, averaged over the columns: the thin bands where two biomes meet weigh as
    /// little as they cover. Sub-biomes and borders count as their parent; rivers and mountains,
    /// which cross the biomes, as nothing.
    fn biome_width(terrain: &TerrainNoises, length: i32, spacing: i32) -> f64 {
        let biomes = terrain.biomes();
        let (mut squares, mut total) = (0.0, 0.0);
        for line in 0..4 {
            let z = line * spacing;
            let row: Vec<BiomeId> = (-length / 2..length / 2)
                .map(|x| parent(biomes, terrain.column_shape(x, z).biome))
                .filter(|&biome| {
                    let def = biomes.get(biome);
                    !def.has_tag(tags::RIVER) && !def.has_tag(tags::MOUNTAIN)
                })
                .collect();
            for run in row.chunk_by(|a, b| a == b) {
                squares += (run.len() as f64).powi(2);
                total += run.len() as f64;
            }
        }
        squares / total
    }

    #[test]
    fn a_biome_spans_several_hundred_blocks() {
        let width = biome_width(&TerrainNoises::new(42), 12_000, 1500);
        assert!((350.0..900.0).contains(&width), "{width:.0} blocks");
    }

    #[test]
    fn the_large_biomes_are_larger() {
        let settings = WorldGenSettings {
            preset: WorldPreset::LargeBiomes,
        };
        let large = TerrainNoises::with(42, &settings, Biomes::vanilla());
        let (large, default) = (
            biome_width(&large, 12_000, 1500),
            biome_width(&TerrainNoises::new(42), 12_000, 1500),
        );
        // the mountains and the rivers, which are not scaled, cut the biomes as much
        assert!(large > 1.3 * default, "{large:.0} against {default:.0}");
    }

    #[test]
    fn the_borders_of_the_biomes_are_ragged() {
        // Along each border met on lines across the world, the length of the border within a
        // window of 64 blocks, against the side of the window: a straight one is 1
        let terrain = TerrainNoises::new(42);
        let biome = |x, z| terrain.column_shape(x, z).biome;
        let (mut length, mut windows) = (0, 0);
        for z0 in (-6000..6000).step_by(1000) {
            let row: Vec<BiomeId> = (-6000..6000).map(|x| biome(x, z0)).collect();
            for (i, pair) in row.windows(2).enumerate().step_by(7) {
                if pair[0] == pair[1] || windows >= 60 {
                    continue;
                }
                let x0 = -6000 + i as i32;
                // the border cells of the window: a cell whose right or lower neighbour differs
                let mut cells = 0;
                for dz in -32..32 {
                    for dx in -32..32 {
                        let (x, z) = (x0 + dx, z0 + dz);
                        let here = biome(x, z);
                        if here != biome(x + 1, z) || here != biome(x, z + 1) {
                            cells += 1;
                        }
                    }
                }
                length += cells;
                windows += 1;
            }
        }
        assert!(windows >= 20, "{windows} borders");
        let ratio = length as f64 / (64.0 * windows as f64);
        assert!(ratio > 1.5, "{ratio:.2}");
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
        let biomes = terrain.biomes().clone();
        let mut highest = i32::MIN;
        for z in (-2048..2048).step_by(8) {
            for x in (-2048..2048).step_by(8) {
                let shape = terrain.column_shape(x, z);
                highest = highest.max(shape.height);
                let mountain = biomes.get(shape.biome).has_tag(tags::MOUNTAIN);
                assert_eq!(
                    mountain,
                    shape.height >= MOUNTAIN_FROM,
                    "({x}, {z}): {shape:?}"
                );
                if shape.height >= HIGH_MOUNTAIN_FROM {
                    assert_eq!(shape.biome, HIGH_MOUNTAIN_GRASS);
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
        for z in (-4096..4096).step_by(8) {
            for x in (-4096..4096).step_by(8) {
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
    fn sample(step: usize, side: i32) -> Vec<(ClimatePoint, ColumnShape)> {
        let terrain = TerrainNoises::new(42);
        let mut columns = Vec::new();
        for z in (-side / 2..side / 2).step_by(step) {
            for x in (-side / 2..side / 2).step_by(step) {
                columns.push((terrain.climate_at(x, z), terrain.column_shape(x, z)));
            }
        }
        columns
    }

    #[test]
    fn the_sea_covers_about_a_third_of_the_world_and_its_biomes_lie_under_water() {
        let biomes = Biomes::vanilla();
        let columns = sample(64, 24_000);
        let mut sea = 0;
        for (climate, shape) in &columns {
            if biomes.get(shape.biome).has_tag(tags::OCEAN) {
                sea += 1;
                assert!(climate.continentalness < COAST, "{climate:?}");
                if climate.continentalness < COAST - 2.0 * COAST_WIDTH {
                    assert!(shape.height < SEA_LEVEL, "{climate:?}: {shape:?}");
                }
            }
        }
        let share = sea as f64 / columns.len() as f64;
        assert!((0.2..0.4).contains(&share), "{:.1} %", 100.0 * share);
    }

    #[test]
    fn the_jungle_lies_only_where_hot_and_wet_on_low_coastal_land() {
        let mut jungle = 0;
        for (climate, shape) in sample(32, 8192) {
            if [JUNGLE, JUNGLE_EDGE].contains(&shape.biome) {
                jungle += 1;
                // the dithering moves the climate a little
                assert!(climate.temperature > 0.2 - DITHER, "{climate:?}");
                assert!(climate.humidity > 0.2 - DITHER, "{climate:?}");
                assert!(
                    (COAST..0.3).contains(&climate.continentalness),
                    "{climate:?}"
                );
                assert!(shape.height < MOUNTAIN_FROM);
            }
        }
        assert!(jungle > 100, "{jungle} jungle columns");
    }

    #[test]
    fn the_jungle_covers_a_few_percent_of_the_world() {
        let columns = sample(64, 24_000);
        let jungle = columns
            .iter()
            .filter(|(_, shape)| [JUNGLE, JUNGLE_EDGE].contains(&shape.biome))
            .count();
        let share = jungle as f64 / columns.len() as f64;
        assert!((0.03..0.08).contains(&share), "{:.1} %", 100.0 * share);
    }

    #[test]
    fn every_vanilla_biome_is_somewhere() {
        let mut seen: Vec<BiomeId> = sample(32, 24_000).iter().map(|(_, s)| s.biome).collect();
        seen.sort();
        seen.dedup();
        let missing: Vec<&str> = Biomes::vanilla()
            .ids()
            .filter(|id| !seen.contains(id))
            .map(|id| KEYS[id.0 as usize])
            .collect();
        assert!(missing.is_empty(), "{missing:?}");
    }

    #[test]
    fn a_biome_deepens_from_its_border_to_its_heart() {
        let terrain = TerrainNoises::new(42);
        let (mut borders, mut deepest) = (0, 0.0f32);
        for z in (-4096..4096).step_by(256) {
            let shapes: Vec<ColumnShape> =
                (-4096..4096).map(|x| terrain.column_shape(x, z)).collect();
            for pair in shapes.windows(2) {
                for shape in pair {
                    assert!((0.0..=1.0).contains(&shape.depth_in_biome), "{shape:?}");
                    deepest = deepest.max(shape.depth_in_biome);
                }
                let land = |shape: &ColumnShape| {
                    let def = terrain.biomes().get(shape.biome);
                    !def.has_tag(tags::OCEAN)
                        && !def.has_tag(tags::RIVER)
                        && !def.has_tag(tags::MOUNTAIN)
                };
                let biomes = terrain.biomes();
                let (a, b) = (parent(biomes, pair[0].biome), parent(biomes, pair[1].biome));
                // the border of a sub-biome lies inside its parent
                if a != b && land(&pair[0]) && land(&pair[1]) {
                    borders += 1;
                    let shallow = pair[0].depth_in_biome.min(pair[1].depth_in_biome);
                    assert!(shallow < 0.3, "{pair:?}");
                }
            }
        }
        assert!(borders > 20, "{borders} borders");
        assert_eq!(deepest, 1.0);
    }

    #[test]
    fn rivers_are_a_biome_of_their_own_but_freeze_in_the_cold() {
        let mut rivers = 0;
        for (_, shape) in sample(4, 2048) {
            if shape.biome == RIVER {
                rivers += 1;
                assert!(shape.height < SEA_LEVEL, "{shape:?}");
            }
        }
        assert!(rivers > 20, "{rivers} river columns");
    }

    #[test]
    fn a_single_biome_world_has_that_biome_everywhere() {
        let settings = WorldGenSettings {
            preset: WorldPreset::SingleBiome("desert".into()),
        };
        let terrain = TerrainNoises::with(42, &settings, Biomes::vanilla());
        for (x, z) in [(0, 0), (500, -300), (-4000, 2500)] {
            assert_eq!(terrain.column_shape(x, z).biome, DESERT);
        }
        // the land keeps its shape
        assert_eq!(
            terrain.column_shape(123, 456).height,
            TerrainNoises::new(42).column_shape(123, 456).height
        );
    }

    #[test]
    fn the_archipelago_has_more_sea() {
        let settings = WorldGenSettings {
            preset: WorldPreset::Archipelago,
        };
        let terrain = TerrainNoises::with(42, &settings, Biomes::vanilla());
        let default = TerrainNoises::new(42);
        let wet = |terrain: &TerrainNoises| {
            (0..1600)
                .filter(|i| terrain.column_shape(i % 40 * 300, i / 40 * 300).height < SEA_LEVEL)
                .count() as f64
        };
        let (archipelago, default) = (wet(&terrain), wet(&default));
        assert!(
            archipelago > 1.25 * default,
            "{archipelago} against {default}"
        );
    }

    #[test]
    fn presets_are_named_and_read_back() {
        for preset in [
            WorldPreset::Default,
            WorldPreset::LargeBiomes,
            WorldPreset::Archipelago,
            WorldPreset::SingleBiome("rustcraft:jungle".into()),
        ] {
            assert_eq!(WorldPreset::from_name(&preset.name()), Some(preset));
        }
        assert_eq!(WorldPreset::from_name(""), Some(WorldPreset::Default));
        assert_eq!(WorldPreset::from_name("single_biome:"), None);
        assert_eq!(WorldPreset::from_name("flat"), None);
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

    #[test]
    fn the_remap_is_monotonic_and_spans_the_range() {
        let remap = Remap::of(5, SHAPE_PERSISTENCE);
        let mut last = -1.0;
        for i in -100..=100 {
            let value = remap.apply(i as f64 / 100.0);
            assert!(
                value >= last && (-1.0..=1.0).contains(&value),
                "{i}: {value}"
            );
            last = value;
        }
        assert_eq!(remap.apply(-1.0), -1.0);
        assert_eq!(remap.apply(1.0), 1.0);
    }
}
