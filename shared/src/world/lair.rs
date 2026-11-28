//! The lair of the Ember Mother, one per world that has dungeons: where it lies, and its shape
//! as the server builds it and its fight, and the depths compass, need it.

use bevy::math::{IVec2, IVec3, Vec3};

use crate::world::depths::{Underground, SHALLOW_LAVA_LEVEL};
use crate::world::{splitmix64, TerrainNoises};

/// How wide the lair is at its ground, and how high above it, before its walls waver.
pub const RADIUS: i32 = 30;
pub const HEIGHT: i32 = 16;
/// The walls waver by this much of the size of the lair, squared, either way.
pub const WAVER: f64 = 0.25;
/// Nothing of the lair is farther from its centre, across.
pub const REACH: i32 = 36;
/// The highest cell of the lair, above its ground. The ground lies at the lava level of its
/// world (`Underground::lava_level`), the surface of its lake too, so the centre's height.
pub const ROOF: i32 = HEIGHT + 2;
/// The radius of the lake of lava, and of the island in its middle, where the mother sleeps.
pub const LAKE: i32 = 12;
pub const ISLAND: i32 = 4;
/// The mother's health beyond that of her kind, for each player inside but the first.
pub const HEALTH_PER_PLAYER: u32 = 80;
/// She calls a Cinderling while fewer are out than one more than the players, and this many at
/// most.
pub const MOST_CALLED: usize = 5;

/// The lair is looked for in rings of regions of this side, this many regions from the origin:
/// a few hundred blocks.
const REGION: i32 = 256;
const RING: i32 = 2;
/// The centre is at least this far inside its region.
const INSET: i32 = 64;
/// How many places are tried, and the lowest surface wanted above the lair: 10 blocks over the
/// roof of a lair in a shallow world. A deep world draws the same place, its lair lower.
const ATTEMPTS: u64 = 400;
const COVER: i32 = SHALLOW_LAVA_LEVEL + ROOF + 10;
const SALT: u64 = 0x3500;

/// The centre of the ground of the lair of the world of `seed`: the first place drawn around
/// the origin that lies deep enough under the land, whose noises are `terrain`, at the lava
/// level of its `underground`.
pub fn centre(seed: u32, terrain: &TerrainNoises, underground: Underground) -> IVec3 {
    let draw = |attempt: u64, what: u64, count: i32| {
        let hash = splitmix64(splitmix64(splitmix64(seed as u64) ^ SALT) ^ (attempt * 8 + what));
        ((hash >> 32) % count as u64) as i32
    };
    let cover = |at: IVec2| {
        let mut lowest = i32::MAX;
        for x in -4..=4 {
            for z in -4..=4 {
                let column = at + IVec2::new(x, z) * (REACH / 4);
                lowest = lowest.min(terrain.column_shape(column.x, column.y).height);
                if lowest < COVER {
                    return lowest;
                }
            }
        }
        lowest
    };
    let mut best = (i32::MIN, IVec2::ZERO);
    for attempt in 0..ATTEMPTS {
        let offset = draw(attempt, 0, 2 * RING + 1) - RING;
        let region = match draw(attempt, 1, 4) {
            0 => IVec2::new(offset, RING),
            1 => IVec2::new(RING, offset),
            2 => IVec2::new(offset, -RING),
            _ => IVec2::new(-RING, offset),
        };
        let span = REGION - 2 * INSET;
        let at =
            region * REGION + INSET + IVec2::new(draw(attempt, 2, span), draw(attempt, 3, span));
        let lowest = cover(at);
        if lowest > best.0 {
            best = (lowest, at);
        }
        if lowest >= COVER {
            break;
        }
    }
    IVec3::new(best.1.x, underground.lava_level(), best.1.y)
}

/// The middle of the column of the block `centre`, at the height of its top.
pub fn heart(centre: IVec3) -> Vec3 {
    centre.as_vec3() + Vec3::new(0.5, 1.0, 0.5)
}

/// Whether `position` is in the lair around `centre`, its walls included.
pub fn holds(centre: IVec3, position: Vec3) -> bool {
    let from = position - heart(centre);
    from.x.hypot(from.z) <= REACH as f32 && (-6.0..=ROOF as f32).contains(&from.y)
}
