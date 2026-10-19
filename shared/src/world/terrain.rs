use noise::{NoiseFn, Perlin};

use super::{biome::BiomeNoises, get_biome_data, BiomeType};

/// How fast the height changes from one column to the next, within a biome.
const TERRAIN_SCALE: f64 = 0.1;

/// What the noises decide for one world column, before it is filled with blocks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ColumnShape {
    /// Height of the surface block.
    pub height: i32,
    pub biome: BiomeType,
}

/// Every noise the shape of a world depends on, built once from its seed. The server
/// generates the terrain with them; the client, given the seed, finds the biome the player is
/// in.
pub struct TerrainNoises {
    height: Perlin,
    biome: BiomeNoises,
}

impl TerrainNoises {
    pub fn new(seed: u32) -> Self {
        Self {
            height: Perlin::new(seed),
            biome: BiomeNoises::new(seed),
        }
    }

    /// The shape of the world column `(x, z)`: a pure function of the seed and the coordinates.
    pub fn column_shape(&self, x: i32, z: i32) -> ColumnShape {
        ColumnShape {
            height: self.height_at(x, z),
            biome: self.biome.biome_at(x, z),
        }
    }

    /// The base height and the variation of the biome of the column, blended with those of
    /// the biomes 4 blocks around so that the ground does not step at a border, plus the
    /// variation times the height noise.
    fn height_at(&self, x: i32, z: i32) -> i32 {
        let biome = get_biome_data(self.biome.biome_at(x, z));

        let mut weighted_base_height = biome.base_height as f64;
        let mut weighted_variation = biome.height_variation as f64;
        let mut total_weight = 1.0;

        for offset_x in [-4, 0, 4] {
            for offset_z in [-4, 0, 4] {
                if offset_x == 0 && offset_z == 0 {
                    continue;
                }
                let neighbour = get_biome_data(self.biome.biome_at(x + offset_x, z + offset_z));

                // the farther a neighbour is, the less influence it has
                let distance = ((offset_x.pow(2) + offset_z.pow(2)) as f64).sqrt();
                let weight = 1.0 / (distance + 1.0);

                weighted_base_height += neighbour.base_height as f64 * weight;
                weighted_variation += neighbour.height_variation as f64 * weight;
                total_weight += weight;
            }
        }

        weighted_base_height /= total_weight;
        weighted_variation /= total_weight;

        let noise = self
            .height
            .get([x as f64 * TERRAIN_SCALE, z as f64 * TERRAIN_SCALE]);
        (weighted_base_height + weighted_variation * noise).round() as i32
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
}
