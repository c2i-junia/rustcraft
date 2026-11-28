//! How the ores and the pockets of rock are scattered through the stone, as the generation of
//! the world places them (`server::world::generation::ores`) and the Codex shows them: their
//! heights, their attempts per chunk and their sizes, for each kind of underground.

use super::depths::Underground;
use super::BlockId;

/// How one ore is scattered.
pub struct Ore {
    pub block: BlockId,
    /// The heights an attempt may start from, both included.
    pub lowest: i32,
    pub highest: i32,
    /// Attempts per chunk.
    pub attempts: u64,
    /// Sizes of a cluster and lengths of a vein, both included.
    pub cluster: (u64, u64),
    pub vein: (u64, u64),
    /// Share of the attempts that are veins, the others are clusters.
    pub veins: f32,
    /// Whether it only takes plain rock, and never the cell of another ore.
    pub only_stone: bool,
    /// The share of the attempts kept, by the height they start at.
    pub chance: Chance,
}

/// The commonest first: a rarer ore is placed after, and wins a cell both claim.
pub const ORES: [Ore; 8] = [
    Ore {
        block: BlockId::CoalOre,
        lowest: 5,
        highest: 128,
        attempts: 6,
        cluster: (6, 14),
        vein: (8, 14),
        veins: 0.2,
        only_stone: false,
        chance: ALWAYS,
    },
    Ore {
        block: BlockId::IronOre,
        lowest: 5,
        highest: 64,
        attempts: 5,
        cluster: (4, 8),
        vein: (6, 12),
        veins: 0.4,
        only_stone: false,
        chance: ALWAYS,
    },
    Ore {
        block: BlockId::GoldOre,
        lowest: 5,
        highest: 32,
        attempts: 1,
        cluster: (3, 6),
        vein: (6, 10),
        veins: 0.5,
        only_stone: false,
        chance: ALWAYS,
    },
    // Deep down, in small clusters only
    Ore {
        block: BlockId::DiamondOre,
        lowest: 5,
        highest: 16,
        attempts: 1,
        cluster: (2, 6),
        vein: (0, 0),
        veins: 0.0,
        only_stone: false,
        chance: ALWAYS,
    },
    // Added after the others, whose ranks the worlds depend on: they leave their cells alone
    Ore {
        block: BlockId::CopperOre,
        lowest: 5,
        highest: 96,
        attempts: 5,
        cluster: (5, 10),
        vein: (6, 12),
        veins: 0.3,
        only_stone: true,
        chance: ALWAYS,
    },
    Ore {
        block: BlockId::TinOre,
        lowest: 5,
        highest: 64,
        attempts: 4,
        cluster: (4, 8),
        vein: (5, 10),
        veins: 0.3,
        only_stone: true,
        chance: ALWAYS,
    },
    // Pockets of rock rather than ores
    Ore {
        block: BlockId::Slate,
        lowest: 5,
        highest: 30,
        attempts: 12,
        cluster: (20, 27),
        vein: (0, 0),
        veins: 0.0,
        only_stone: true,
        chance: ALWAYS,
    },
    Ore {
        block: BlockId::Limestone,
        lowest: 40,
        highest: 100,
        attempts: 8,
        cluster: (20, 27),
        vein: (0, 0),
        veins: 0.0,
        only_stone: true,
        chance: ALWAYS,
    },
];

/// The ores of a deep world, over its whole height: coal, copper and tin high, iron in the
/// middle and high, gold deep, diamonds mostly around -50 and rare above. Same ranks as `ORES`.
pub const DEEP_ORES: [Ore; 8] = [
    Ore {
        block: BlockId::CoalOre,
        lowest: 0,
        highest: 128,
        attempts: 6,
        cluster: (6, 14),
        vein: (8, 14),
        veins: 0.2,
        only_stone: false,
        chance: ALWAYS,
    },
    Ore {
        block: BlockId::IronOre,
        lowest: -24,
        highest: 72,
        attempts: 4,
        cluster: (4, 8),
        vein: (6, 12),
        veins: 0.4,
        only_stone: false,
        chance: ALWAYS,
    },
    Ore {
        block: BlockId::GoldOre,
        lowest: -60,
        highest: -8,
        attempts: 1,
        cluster: (3, 6),
        vein: (6, 10),
        veins: 0.5,
        only_stone: false,
        chance: Chance {
            below: 0.8,
            rare_above: -32,
            rare: 0.4,
        },
    },
    Ore {
        block: BlockId::DiamondOre,
        lowest: -58,
        highest: 16,
        attempts: 1,
        cluster: (2, 6),
        vein: (0, 0),
        veins: 0.0,
        only_stone: false,
        chance: Chance {
            below: 0.8,
            rare_above: -42,
            rare: 0.06,
        },
    },
    Ore {
        block: BlockId::CopperOre,
        lowest: -16,
        highest: 96,
        attempts: 5,
        cluster: (5, 10),
        vein: (6, 12),
        veins: 0.3,
        only_stone: true,
        chance: ALWAYS,
    },
    Ore {
        block: BlockId::TinOre,
        lowest: -16,
        highest: 64,
        attempts: 4,
        cluster: (4, 8),
        vein: (5, 10),
        veins: 0.3,
        only_stone: true,
        chance: ALWAYS,
    },
    Ore {
        block: BlockId::Slate,
        lowest: 5,
        highest: 30,
        attempts: 12,
        cluster: (20, 27),
        vein: (0, 0),
        veins: 0.0,
        only_stone: true,
        chance: ALWAYS,
    },
    Ore {
        block: BlockId::Limestone,
        lowest: 40,
        highest: 100,
        attempts: 8,
        cluster: (20, 27),
        vein: (0, 0),
        veins: 0.0,
        only_stone: true,
        chance: ALWAYS,
    },
];

/// The share of the attempts of an ore that are kept: `below` up to `rare_above`, `rare` over
/// it.
pub struct Chance {
    pub below: f32,
    pub rare_above: i32,
    pub rare: f32,
}

pub const ALWAYS: Chance = Chance {
    below: 1.0,
    rare_above: i32::MAX,
    rare: 1.0,
};

impl Chance {
    pub fn at(&self, y: i32) -> f32 {
        if y > self.rare_above {
            self.rare
        } else {
            self.below
        }
    }
}

/// The ores of a world of `underground`.
pub fn ores(underground: Underground) -> &'static [Ore; 8] {
    match underground {
        Underground::Shallow => &ORES,
        Underground::Deep => &DEEP_ORES,
    }
}
