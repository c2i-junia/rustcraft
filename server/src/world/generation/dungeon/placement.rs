//! Where dungeons are: at most one of each kind per region, where the land or the caves suit it.

use std::ops::RangeInclusive;

use bevy::prelude::*;
use shared::world::{HIGHEST_SURFACE, MOUNTAIN_FROM, SEA_LEVEL};
use shared::CHUNK_SIZE;

use super::super::structure::{rotate, DIRECTIONS};
use super::super::{village, PositionRandom};
use super::blueprints::Blueprint;
use super::{DungeonKind, Site, World};
use crate::world::caves::LAVA_LEVEL;

/// Side of a region, in blocks: each may hold one dungeon of each kind. A village region holds
/// whole ones.
pub const REGION: i32 = 256;
const _: () = assert!(village::REGION % REGION == 0);
/// The door of a dungeon lies at least this far inside its region: more than a dungeon is long
/// and the margin of the chunks, so that it never leaves its region.
const INSET: i32 = 32;

/// How many places are drawn in a region, until one suits.
const ATTEMPTS: u64 = 12;
/// The salts of a kind: its roll, then three for each attempt.
const SALT_DUNGEON: u64 = 0x3100;
const SALTS_PER_KIND: u64 = 0x100;

/// Most blocks between the lowest and the highest column under a ruin.
const RUIN_SPREAD: i32 = 3;
/// Rock kept between the top of an underground dungeon and the surface around it.
const ROOF: i32 = 10;

/// The chance of a region to hold a dungeon of `kind`, before its places are looked at.
fn chance(kind: DungeonKind) -> f32 {
    match kind {
        DungeonKind::Ruin => 0.5,
        DungeonKind::Crypt => 0.75,
        DungeonKind::Sanctuary => 0.6,
    }
}

/// The heights the cells of a dungeon of `kind` lie at.
pub fn heights(kind: DungeonKind) -> RangeInclusive<i32> {
    match kind {
        DungeonKind::Ruin => SEA_LEVEL - 8..=HIGHEST_SURFACE,
        DungeonKind::Crypt => 20..=50,
        DungeonKind::Sanctuary => LAVA_LEVEL - 1..=17,
    }
}

/// The region of the column `(x, z)`.
pub fn region_of(x: i32, z: i32) -> IVec2 {
    IVec2::new(x.div_euclid(REGION), z.div_euclid(REGION))
}

/// The dungeon of `kind` in `region`, if it holds one: the first of the places drawn that suits
/// it.
pub fn site(world: &World, kind: DungeonKind, region: IVec2) -> Option<Site> {
    let random = PositionRandom {
        seed: world.seed,
        coords: [region.x, region.y],
    };
    let salts = SALT_DUNGEON + SALTS_PER_KIND * kind as u64;
    if random.unit(salts) >= chance(kind)
        || (kind != DungeonKind::Ruin
            && world.lair.is_some_and(|at| region == region_of(at.x, at.y)))
    {
        return None;
    }
    let span = (REGION - 2 * INSET) as u64;
    (0..ATTEMPTS).find_map(|attempt| {
        let salt = salts + 1 + 3 * attempt;
        let at = region * REGION
            + INSET
            + IVec2::new(random.below(salt, span), random.below(salt + 1, span));
        let draw = random.below(salt + 2, 4096);
        match kind {
            DungeonKind::Ruin => on_the_surface(world, at, draw),
            DungeonKind::Crypt | DungeonKind::Sanctuary => off_a_cave(world, kind, at, draw),
        }
    })
}

/// A ruin whose door is at the column `at`, if the land under it is dry, rather flat, below the
/// mountains and away from the giant trees and the villages.
fn on_the_surface(world: &World, at: IVec2, draw: i32) -> Option<Site> {
    let turns = (draw % 4) as usize;
    let blueprint = Blueprint::of(DungeonKind::Ruin);
    let (width, depth) = blueprint.size();
    let biomes = world.terrain.biomes();
    let (mut low, mut high) = (i32::MAX, i32::MIN);
    for i in 0..=4 {
        for j in 0..=4 {
            let local = IVec2::new((width - 1) * i / 4 - blueprint.door, (depth - 1) * j / 4);
            let column = at + rotate(local, turns);
            let shape = world.terrain.column_shape(column.x, column.y);
            low = low.min(shape.height);
            high = high.max(shape.height);
            if !village::dry(low)
                || high >= MOUNTAIN_FROM - 8
                || high - low > RUIN_SPREAD
                || biomes.get(shape.biome).flora.giant_trees
            {
                return None;
            }
        }
    }
    if world.villages {
        let region = village::region_of(at.x, at.y);
        if let Some(site) = village::site(world.seed, world.terrain, region) {
            if (site.centre - at).abs().max_element() <= village::REACH + INSET {
                return None;
            }
        }
    }
    Some(Site {
        door: IVec3::new(at.x, (low + high + 1) / 2, at.y),
        turns,
    })
}

/// The place of a dungeon of `kind` dug behind the wall of a cave, in the chunk of the column
/// `at`: its door opens on a cell of the cave with a floor, and, for a sanctuary, lava is near.
fn off_a_cave(world: &World, kind: DungeonKind, at: IVec2, draw: i32) -> Option<Site> {
    let heights = heights(kind);
    let blueprint = Blueprint::of(kind);
    let height = blueprint.layers.len() as i32 + blueprint.lowest;
    // the floors that keep the dungeon between its heights
    let floors = heights.start() - blueprint.lowest..=heights.end() - height + 1;
    let layers = floors.start().div_euclid(CHUNK_SIZE)..=floors.end().div_euclid(CHUNK_SIZE);
    let chunk_y = layers.start() + draw % (layers.end() - layers.start() + 1);
    let chunk = at.div_euclid(IVec2::splat(CHUNK_SIZE));
    let origin = IVec3::new(chunk.x, chunk_y, chunk.y) * CHUNK_SIZE;
    let caves = world.caves.carve(origin);
    let carved = |local: IVec3| caves.contains(local);
    if kind == DungeonKind::Sanctuary {
        let lava = (0..CHUNK_SIZE)
            .flat_map(|x| (0..CHUNK_SIZE).map(move |z| (x, z)))
            .filter(|&(x, z)| carved(IVec3::new(x, LAVA_LEVEL - origin.y, z)))
            .count();
        if lava < 8 {
            return None;
        }
    }
    // the cells of the chunk, from the one drawn on
    let cells = CHUNK_SIZE * CHUNK_SIZE * CHUNK_SIZE;
    let found = (0..cells).find_map(|i| {
        let i = (i + draw) % cells;
        let feet = IVec3::new(
            i % CHUNK_SIZE,
            i / (CHUNK_SIZE * CHUNK_SIZE),
            i / CHUNK_SIZE % CHUNK_SIZE,
        );
        let inner = |d: i32| (1..CHUNK_SIZE - 1).contains(&d);
        let open = inner(feet.x)
            && inner(feet.y)
            && inner(feet.z)
            && floors.contains(&(origin.y + feet.y - 1))
            && carved(feet)
            && carved(feet + IVec3::Y)
            && !carved(feet - IVec3::Y);
        if !open {
            return None;
        }
        (0..4).map(|k| (k + draw as usize) % 4).find_map(|turns| {
            let out = DIRECTIONS[turns];
            let door = feet - IVec3::new(out.x, 0, out.y);
            (!carved(door) && !carved(door + IVec3::Y)).then_some(Site {
                door: origin + door - IVec3::Y,
                turns,
            })
        })
    })?;
    // deep under dry land
    let (width, depth) = blueprint.size();
    let reach = width.max(depth);
    let top = found.door.y + height;
    for dx in [-reach, 0, reach] {
        for dz in [-reach, 0, reach] {
            let shape = world
                .terrain
                .column_shape(found.door.x + dx, found.door.z + dz);
            if shape.height < (top + ROOF).max(SEA_LEVEL - 8) {
                return None;
            }
        }
    }
    Some(found)
}
