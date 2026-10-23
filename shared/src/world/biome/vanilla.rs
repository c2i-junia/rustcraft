//! The biomes of the base game, registered through the same calls a mod uses. Their ids are
//! fixed: they are registered first, in the order of the constants below, before any other.

use super::{
    tags, BiomeDef, BiomeId, BiomeLook, BiomeRegistry, BorderRule, Chance, ClimateBox, Flora,
    Ground, Herd, Precipitation, Span, Tree,
};
use crate::world::{BlockId, MobKind};

pub const PLAINS: BiomeId = BiomeId(0);
pub const FOREST: BiomeId = BiomeId(1);
pub const MEDIUM_MOUNTAIN: BiomeId = BiomeId(2);
pub const HIGH_MOUNTAIN_GRASS: BiomeId = BiomeId(3);
pub const DESERT: BiomeId = BiomeId(4);
pub const ICE_PLAIN: BiomeId = BiomeId(5);
pub const FLOWER_PLAINS: BiomeId = BiomeId(6);
pub const SHALLOW_OCEAN: BiomeId = BiomeId(7);
pub const OCEAN: BiomeId = BiomeId(8);
pub const DEEP_OCEAN: BiomeId = BiomeId(9);
/// Hot, drier than a forest and wetter than a desert: grass with a few trees.
pub const SAVANNA: BiomeId = BiomeId(10);
/// Hot and wet, along the warm coasts.
pub const JUNGLE: BiomeId = BiomeId(11);
/// The fringe of the jungle where it meets another land biome: sparse jungle trees among oaks.
pub const JUNGLE_EDGE: BiomeId = BiomeId(12);
/// A rare variant of the temperate forest, carpeted with flowers.
pub const FLOWER_FOREST: BiomeId = BiomeId(13);
/// Open patches of grass and flowers inside a forest.
pub const FOREST_CLEARING: BiomeId = BiomeId(14);
/// Clumps of trees inside the plains.
pub const GROVE: BiomeId = BiomeId(15);
/// A rare cold biome where spikes of ice rise from the snow.
pub const ICE_SPIKES: BiomeId = BiomeId(16);
/// The water of the rivers that cross the land.
pub const RIVER: BiomeId = BiomeId(17);

/// The keys of the biomes above, by id.
pub const KEYS: [&str; 18] = [
    "rustcraft:plains",
    "rustcraft:forest",
    "rustcraft:medium_mountain",
    "rustcraft:high_mountain_grass",
    "rustcraft:desert",
    "rustcraft:ice_plain",
    "rustcraft:flower_plains",
    "rustcraft:shallow_ocean",
    "rustcraft:ocean",
    "rustcraft:deep_ocean",
    "rustcraft:savanna",
    "rustcraft:jungle",
    "rustcraft:jungle_edge",
    "rustcraft:flower_forest",
    "rustcraft:forest_clearing",
    "rustcraft:grove",
    "rustcraft:ice_spikes",
    "rustcraft:river",
];

/// Continentalness from which a column is land: the coast lies there.
pub const COAST: f64 = -0.4;
/// The land, from the coast inland.
const LAND: Span = Span::new(COAST, 1.0);
/// The three bands of temperature.
const COLD: Span = Span::new(-1.0, -0.45);
const TEMPERATE: Span = Span::new(-0.45, 0.2);
const HOT: Span = Span::new(0.2, 1.0);

/// The mountains rise from these heights over every land biome, whatever its climate.
pub const MOUNTAIN_FROM: i32 = 84;
pub const HIGH_MOUNTAIN_FROM: i32 = 100;

fn key(id: BiomeId) -> String {
    KEYS[id.0 as usize].to_string()
}

/// A land box of `temperature` and `humidity`.
fn land(temperature: Span, humidity: Span) -> ClimateBox {
    ClimateBox {
        temperature,
        humidity,
        continentalness: LAND,
        ..ClimateBox::default()
    }
}

/// A land biome: the mountains rise over it.
fn land_biome(id: BiomeId, name: &str) -> BiomeDef {
    BiomeDef {
        altitude_variants: vec![
            (HIGH_MOUNTAIN_FROM, key(HIGH_MOUNTAIN_GRASS)),
            (MOUNTAIN_FROM, key(MEDIUM_MOUNTAIN)),
        ],
        ..BiomeDef::new(&key(id), name)
    }
}

fn sea(id: BiomeId, name: &str, continentalness: Span, colour: [u8; 3]) -> BiomeDef {
    BiomeDef {
        climate: vec![ClimateBox {
            continentalness,
            ..ClimateBox::default()
        }],
        dither: false,
        tags: vec![tags::OCEAN.into()],
        ground: Ground::of(BlockId::Sand, BlockId::Sand),
        look: BiomeLook::new([0.5, 0.5], colour),
        ..BiomeDef::new(&key(id), name)
    }
}

/// What grows on grass in most temperate biomes: some flowers and tall grass.
fn meadow(flowers: f32) -> Flora {
    Flora {
        flowers,
        tall_grass: Chance::flat(0.10),
        ..Flora::default()
    }
}

fn herd(kind: MobKind, chance: f32, fewest: u64, most: u64) -> Herd {
    Herd {
        kind,
        chance,
        fewest,
        most,
    }
}

/// The definitions of every biome of the base game, by id.
pub fn definitions() -> Vec<BiomeDef> {
    let sheep = |chance, fewest, most| herd(MobKind::Sheep, chance, fewest, most);
    let fox = |chance, fewest, most| herd(MobKind::Fox, chance, fewest, most);
    vec![
        BiomeDef {
            climate: vec![
                land(COLD, Span::new(-1.0, -0.3)),
                land(TEMPERATE, Span::new(-1.0, -0.1)),
            ],
            sub_biomes: vec![(key(GROVE), 0.12)],
            flora: meadow(0.02),
            herds: vec![sheep(0.12, 2, 4)],
            look: BiomeLook::new([0.8, 0.4], [140, 190, 90]),
            ..land_biome(PLAINS, "Plains")
        },
        BiomeDef {
            climate: vec![
                land(TEMPERATE, Span::new(-0.1, 1.0)),
                land(HOT, Span::new(-0.1, 0.2)),
                ClimateBox {
                    continentalness: Span::new(0.3, 1.0),
                    ..land(HOT, Span::new(0.2, 1.0))
                },
            ],
            sub_biomes: vec![(key(FOREST_CLEARING), 0.12)],
            tags: vec![tags::FOREST.into()],
            flora: Flora {
                trees: vec![
                    (Tree::BigOak, Chance::flat(0.01)),
                    (Tree::Oak, Chance::flat(0.05)),
                ],
                ..meadow(0.02)
            },
            herds: vec![fox(0.10, 1, 2), sheep(0.03, 2, 3)],
            look: BiomeLook::new([0.7, 0.8], [40, 120, 50]),
            ..land_biome(FOREST, "Forest")
        },
        BiomeDef {
            altitude_variants: vec![(HIGH_MOUNTAIN_FROM, key(HIGH_MOUNTAIN_GRASS))],
            tags: vec![tags::MOUNTAIN.into()],
            flora: Flora {
                trees: vec![(Tree::Oak, Chance::flat(0.02))],
                ..meadow(0.02)
            },
            herds: vec![sheep(0.06, 2, 4)],
            // As the windswept hills
            look: BiomeLook::new([0.2, 0.3], [150, 140, 120]),
            ..BiomeDef::new(&key(MEDIUM_MOUNTAIN), "MediumMountain")
        },
        BiomeDef {
            tags: vec![tags::MOUNTAIN.into()],
            herds: vec![sheep(0.04, 2, 3)],
            // As the snowy slopes
            look: BiomeLook::new([0.0, 0.3], [200, 200, 200]),
            ..BiomeDef::new(&key(HIGH_MOUNTAIN_GRASS), "HighMountainGrass")
        },
        BiomeDef {
            climate: vec![land(HOT, Span::new(-1.0, -0.5))],
            tags: vec![tags::HOT.into(), tags::DRY.into()],
            ground: Ground::of(BlockId::Sand, BlockId::Sand),
            flora: Flora {
                cacti: 0.01,
                ..Flora::default()
            },
            look: BiomeLook {
                precipitation: Precipitation::None,
                ..BiomeLook::new([1.0, 0.0], [230, 210, 140])
            },
            ..land_biome(DESERT, "Desert")
        },
        BiomeDef {
            climate: vec![land(COLD, Span::new(-0.3, 1.0))],
            tags: vec![tags::SNOWY.into()],
            ground: Ground {
                frozen: true,
                ..Ground::of(BlockId::Snow, BlockId::Ice)
            },
            flora: meadow(0.0),
            herds: vec![fox(0.06, 1, 2)],
            look: BiomeLook {
                precipitation: Precipitation::Snow,
                ..BiomeLook::new([0.0, 0.5], [200, 230, 250])
            },
            ..land_biome(ICE_PLAIN, "IcePlain")
        },
        BiomeDef {
            // Shares the temperate plains, in patches
            climate: vec![land(TEMPERATE, Span::new(-1.0, -0.1))],
            weight: 0.35,
            flora: Flora {
                trees: vec![(Tree::Oak, Chance::flat(0.02))],
                ..meadow(0.1)
            },
            herds: vec![sheep(0.12, 2, 4)],
            look: BiomeLook::new([0.7, 0.8], [200, 180, 90]),
            ..land_biome(FLOWER_PLAINS, "FlowerPlains")
        },
        sea(
            SHALLOW_OCEAN,
            "ShallowOcean",
            Span::new(-0.6, COAST),
            [90, 160, 220],
        ),
        sea(OCEAN, "Ocean", Span::new(-0.8, -0.6), [50, 100, 200]),
        sea(
            DEEP_OCEAN,
            "DeepOcean",
            Span::new(-1.0, -0.8),
            [20, 50, 140],
        ),
        BiomeDef {
            climate: vec![land(HOT, Span::new(-0.5, -0.1))],
            tags: vec![tags::HOT.into(), tags::DRY.into()],
            flora: Flora {
                tall_grass: Chance::flat(0.25),
                trees: vec![(Tree::Oak, Chance::flat(0.004))],
                ..Flora::default()
            },
            look: BiomeLook {
                precipitation: Precipitation::None,
                ..BiomeLook::new([1.0, 0.0], [210, 140, 60])
            },
            ..land_biome(SAVANNA, "Savanna")
        },
        BiomeDef {
            climate: vec![ClimateBox {
                continentalness: Span::new(COAST, 0.3),
                ..land(HOT, Span::new(0.2, 1.0))
            }],
            border: Some(BorderRule {
                biome: key(JUNGLE_EDGE),
                width: 0.05,
            }),
            tags: vec![tags::HOT.into(), tags::FOREST.into()],
            // the jungle grows denser from its border to its heart
            flora: Flora {
                tall_grass: Chance::new(0.10, 0.22),
                ferns: Chance::new(0.04, 0.12),
                giant_trees: true,
                trees: vec![
                    (Tree::Jungle, Chance::new(0.0, 0.03)),
                    (Tree::Bush, Chance::new(0.01, 0.06)),
                ],
                ..Flora::default()
            },
            look: BiomeLook::new([0.95, 0.9], [150, 200, 0]),
            ..land_biome(JUNGLE, "Jungle")
        },
        BiomeDef {
            tags: vec![tags::HOT.into(), tags::FOREST.into()],
            flora: Flora {
                tall_grass: Chance::flat(0.12),
                ferns: Chance::flat(0.04),
                trees: vec![
                    (Tree::Jungle, Chance::flat(0.008)),
                    (Tree::Oak, Chance::flat(0.02)),
                    (Tree::Bush, Chance::flat(0.02)),
                ],
                ..Flora::default()
            },
            look: BiomeLook::new([0.85, 0.85], [120, 170, 30]),
            ..land_biome(JUNGLE_EDGE, "JungleEdge")
        },
        BiomeDef {
            // A rare variant: it shares the temperate forest, with a small weight
            climate: vec![land(TEMPERATE, Span::new(-0.1, 1.0))],
            weight: 0.08,
            tags: vec![tags::FOREST.into()],
            flora: Flora {
                trees: vec![
                    (Tree::BigOak, Chance::flat(0.005)),
                    (Tree::Oak, Chance::flat(0.03)),
                ],
                tall_grass: Chance::flat(0.08),
                ..meadow(0.15)
            },
            herds: vec![fox(0.10, 1, 2), sheep(0.03, 2, 3)],
            look: BiomeLook::new([0.7, 0.8], [230, 110, 170]),
            ..land_biome(FLOWER_FOREST, "FlowerForest")
        },
        BiomeDef {
            flora: Flora {
                tall_grass: Chance::flat(0.2),
                ..meadow(0.06)
            },
            herds: vec![sheep(0.12, 2, 4)],
            look: BiomeLook::new([0.7, 0.8], [110, 170, 80]),
            ..land_biome(FOREST_CLEARING, "ForestClearing")
        },
        BiomeDef {
            tags: vec![tags::FOREST.into()],
            flora: Flora {
                trees: vec![
                    (Tree::BigOak, Chance::flat(0.01)),
                    (Tree::Oak, Chance::flat(0.06)),
                ],
                ..meadow(0.02)
            },
            herds: vec![fox(0.05, 1, 2), sheep(0.06, 2, 3)],
            look: BiomeLook::new([0.75, 0.7], [70, 140, 60]),
            ..land_biome(GROVE, "Grove")
        },
        BiomeDef {
            // At the far end of the weirdness, in the cold: it wins over the ice plains
            climate: vec![ClimateBox {
                weirdness: Span::new(0.82, 1.0),
                ..land(Span::new(-1.0, -0.55), Span::new(-0.2, 1.0))
            }],
            priority: 1,
            tags: vec![tags::SNOWY.into()],
            ground: Ground {
                frozen: true,
                ..Ground::of(BlockId::Snow, BlockId::Ice)
            },
            flora: Flora {
                ice_spikes: 0.012,
                ..Flora::default()
            },
            look: BiomeLook {
                precipitation: Precipitation::Snow,
                ..BiomeLook::new([0.0, 0.5], [170, 220, 255])
            },
            ..land_biome(ICE_SPIKES, "IceSpikes")
        },
        BiomeDef {
            dither: false,
            tags: vec![tags::RIVER.into()],
            look: BiomeLook::new([0.5, 0.5], [60, 120, 230]),
            ..BiomeDef::new(&key(RIVER), "River")
        },
    ]
}

/// A registry holding the biomes of the base game, to which others may be added.
pub fn registry() -> BiomeRegistry {
    let mut registry = BiomeRegistry::default();
    for def in definitions() {
        registry.register(def);
    }
    registry
}
