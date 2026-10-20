use super::BiomeType;

/// Share of the humidity range taken by the three ocean biomes, at its wet end.
const OCEAN_SHARE: f64 = 0.27;
/// Humidity from which a column is in the sea: the ocean biomes start here.
pub const OCEAN_HUMIDITY: f64 = 1.0 - OCEAN_SHARE;
/// Land hotter and wetter than these is jungle: the wet half of the hot forests, along the
/// coasts.
pub const JUNGLE_TEMPERATURE: f64 = 0.65;
pub const JUNGLE_HUMIDITY: f64 = 0.55;

/// The biome of a column from its climate, both in `[0, 1]`. The mountain biomes come from the
/// altitude instead (see `TerrainNoises::column_shape`).
pub fn determine_biome(temperature: f64, humidity: f64) -> BiomeType {
    if humidity > (1.0 - (OCEAN_SHARE / 3.0)) {
        return BiomeType::DeepOcean;
    }
    if humidity > (1.0 - 2.0 * (OCEAN_SHARE / 3.0)) {
        return BiomeType::Ocean;
    }
    if humidity > OCEAN_HUMIDITY {
        return BiomeType::ShallowOcean;
    }
    if temperature > JUNGLE_TEMPERATURE && humidity > JUNGLE_HUMIDITY {
        return BiomeType::Jungle;
    }
    if temperature > 0.6 {
        if humidity > OCEAN_HUMIDITY / 2.0 {
            BiomeType::Forest
        } else if humidity > OCEAN_HUMIDITY / 3.0 {
            BiomeType::Savanna
        } else {
            BiomeType::Desert
        }
    } else if temperature > 0.3 {
        if humidity > 2.0 * OCEAN_HUMIDITY / 3.0 {
            BiomeType::FlowerPlains
        } else {
            BiomeType::Plains
        }
    } else if humidity > OCEAN_HUMIDITY / 2.0 {
        BiomeType::IcePlain
    } else {
        BiomeType::Plains
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hot_and_wet_land_is_jungle_and_the_rest_of_the_hot_and_wet_land_stays_forest() {
        assert_eq!(determine_biome(0.7, 0.6), BiomeType::Jungle);
        assert_eq!(determine_biome(0.99, OCEAN_HUMIDITY), BiomeType::Jungle);
        assert_eq!(determine_biome(JUNGLE_TEMPERATURE, 0.6), BiomeType::Forest);
        assert_eq!(determine_biome(0.7, JUNGLE_HUMIDITY), BiomeType::Forest);
        assert_eq!(determine_biome(0.7, 0.74), BiomeType::ShallowOcean);
        assert_eq!(determine_biome(0.7, 0.3), BiomeType::Savanna);
        assert_eq!(determine_biome(0.7, 0.2), BiomeType::Desert);
    }
}
