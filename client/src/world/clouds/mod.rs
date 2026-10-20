//! Block clouds drifting east high above the terrain, the same for every player: a periodic
//! pattern of full and empty cells, slid along +X by the world clock.

// Nothing draws the clouds yet: their plugin comes with the material.
#![allow(dead_code)]

pub mod field;
pub mod mesh;

/// The width of a cell of the pattern along X and Z, in blocks.
pub const CELL: i32 = 12;

/// The height of the clouds, in blocks.
pub const THICKNESS: i32 = 4;

/// The height of the bottom of the clouds: the highest ridges stay under ~180.
pub const ALTITUDE: i32 = 192;

/// The pattern repeats every `PERIOD` cells along X and along Z.
pub const PERIOD: i32 = 256;

/// How fast the clouds drift towards +X, in blocks per second.
pub const SPEED: f32 = 0.5;

/// The share of the cells that are full.
pub const COVERAGE: f64 = 0.45;

/// The seed of the pattern: fixed, so that every world has the same sky.
pub const SEED: u64 = 0x00C1_0D5E_ED00_2026;

/// How bright each face of a cloud is, a fraction of the tint.
pub const SHADE_TOP: f32 = 1.0;
pub const SHADE_NORTH_SOUTH: f32 = 0.9;
pub const SHADE_EAST_WEST: f32 = 0.8;
pub const SHADE_BOTTOM: f32 = 0.7;
