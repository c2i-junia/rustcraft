use noise::{NoiseFn, Perlin};

use super::BiomeType;

/// How fast temperature and humidity change from one column to the next.
pub const BIOME_SCALE: f64 = 0.01;

pub fn determine_biome(temperature: f64, humidity: f64) -> BiomeType {
    let ocean_percentage: f64 = 0.33;
    if humidity > (1.0 - (ocean_percentage / 3.0)) {
        return BiomeType::DeepOcean;
    }
    if humidity > (1.0 - 2.0 * (ocean_percentage / 3.0)) {
        return BiomeType::Ocean;
    }
    if humidity > (1.0 - ocean_percentage) {
        return BiomeType::ShallowOcean;
    }
    if temperature > 0.6 {
        if humidity > (1.0 - ocean_percentage) / 2.0 {
            BiomeType::Forest
        } else {
            BiomeType::Desert
        }
    } else if temperature > 0.3 {
        if humidity > 2.0 * (1.0 - ocean_percentage) / 3.0 {
            BiomeType::FlowerPlains
        } else if humidity > (1.0 - ocean_percentage) / 3.0 {
            BiomeType::Plains
        } else {
            BiomeType::MediumMountain
        }
    } else if humidity > (1.0 - ocean_percentage) / 2.0 {
        BiomeType::IcePlain
    } else {
        BiomeType::HighMountainGrass
    }
}

/// The temperature and humidity noises of a world, which pick the biome of each column. They
/// are part of the [`TerrainNoises`](super::TerrainNoises), through which the server and the
/// client read them.
pub(super) struct BiomeNoises {
    temperature: Perlin,
    humidity: Perlin,
}

impl BiomeNoises {
    pub(super) fn new(seed: u32) -> Self {
        Self {
            temperature: Perlin::new(seed.wrapping_add(1)),
            humidity: Perlin::new(seed.wrapping_add(2)),
        }
    }

    /// The biome of the world column `(x, z)`.
    pub(super) fn biome_at(&self, x: i32, z: i32) -> BiomeType {
        let point = [x as f64 * BIOME_SCALE, z as f64 * BIOME_SCALE];
        let temperature = (self.temperature.get(point) + 1.0) / 2.0;
        let humidity = (self.humidity.get(point) + 1.0) / 2.0;
        determine_biome(temperature, humidity)
    }
}
