//! Dumps generated terrain for `scripts/worldgen_preview.py`, which draws it. Run it in release:
//!
//! ```text
//! cargo run -p server --release --example worldgen_dump -- map <seed> <x0> <z0> <size> <out>
//! cargo run -p server --release --example worldgen_dump -- slice <seed> <z> <x0> <width> <out>
//! cargo run -p server --release --example worldgen_dump -- layer <seed> <y> <x0> <z0> <size> <out>
//! ```
//!
//! Every file starts with its width and height as two little-endian `u32`. A `map` then holds,
//! per column, row by row: the height of its top block (`i16`), that block (`u8`), the height of
//! its highest ground block, not water nor anything growing (`i16`) and its biome (`u8`). A `slice` (the
//! blocks of the plane `z`, from y 0 up) and a `layer` (the blocks at height `y`) hold one `u8`
//! per cell: the `BlockId`, or 255 for air.

use std::collections::HashMap;
use std::io::Write;

use bevy::prelude::*;
use server::WorldGenerator;
use shared::world::{BlockId, Chunk, TerrainNoises};
use shared::CHUNK_SIZE;

const AIR: u8 = 255;
/// Slices go from y 0 to this height.
const SLICE_TOP: i32 = 256;

/// Whether `block` is part of the ground: not water nor the ice on it, and not something
/// growing on it.
fn is_ground(block: BlockId) -> bool {
    !matches!(
        block,
        BlockId::Water
            | BlockId::Ice
            | BlockId::OakLeaves
            | BlockId::OakLog
            | BlockId::SpruceLeaves
            | BlockId::SpruceLog
            | BlockId::JungleLeaves
            | BlockId::JungleLog
            | BlockId::Cactus
            | BlockId::Dandelion
            | BlockId::Poppy
            | BlockId::TallGrass
    )
}

fn block_at(chunk: &Chunk, local: IVec3) -> Option<BlockId> {
    chunk.get(local).map(|b| b.id)
}

fn arg<T: std::str::FromStr>(args: &[String], i: usize) -> T {
    args.get(i)
        .and_then(|a| a.parse().ok())
        .unwrap_or_else(|| panic!("argument {i} is missing or invalid, see the doc comment"))
}

fn write(path: &str, width: usize, height: usize, data: &[u8]) {
    let mut file = std::fs::File::create(path).unwrap();
    file.write_all(&(width as u32).to_le_bytes()).unwrap();
    file.write_all(&(height as u32).to_le_bytes()).unwrap();
    file.write_all(data).unwrap();
}

/// Runs `f` on every item, on every core, and gathers the results in order.
fn parallel<T: Sync, R: Send>(items: &[T], f: impl Fn(&T) -> R + Sync) -> Vec<R> {
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
    std::thread::scope(|scope| {
        let f = &f;
        let workers: Vec<_> = items
            .chunks(items.len().div_ceil(threads).max(1))
            .map(|part| scope.spawn(move || part.iter().map(f).collect::<Vec<_>>()))
            .collect();
        workers
            .into_iter()
            .flat_map(|w| w.join().unwrap())
            .collect()
    })
}

/// The top block and the top ground block of every column of the chunk column `(cx, cz)`,
/// generating the chunks from above the highest ground down.
fn chunk_column_tops(
    generator: &WorldGenerator,
    terrain: &TerrainNoises,
    cx: i32,
    cz: i32,
) -> Vec<(i16, u8, i16, u8)> {
    let size = CHUNK_SIZE as usize;
    let mut highest = 0;
    for dx in 0..CHUNK_SIZE {
        for dz in 0..CHUNK_SIZE {
            let shape = terrain.column_shape(cx * CHUNK_SIZE + dx, cz * CHUNK_SIZE + dz);
            highest = highest.max(shape.height);
        }
    }
    // trees grow above the ground, and water fills up to the sea
    let top_chunk = (highest.max(64) + 16).div_euclid(CHUNK_SIZE);

    let mut tops: Vec<Option<(i16, u8)>> = vec![None; size * size];
    let mut floors: Vec<Option<i16>> = vec![None; size * size];
    for cy in (0..=top_chunk).rev() {
        let chunk = generator.generate_chunk(IVec3::new(cx, cy, cz));
        for dx in 0..CHUNK_SIZE {
            for dz in 0..CHUNK_SIZE {
                let i = dz as usize * size + dx as usize;
                for dy in (0..CHUNK_SIZE).rev() {
                    let Some(block) = block_at(&chunk, IVec3::new(dx, dy, dz)) else {
                        continue;
                    };
                    let y = (cy * CHUNK_SIZE + dy) as i16;
                    if tops[i].is_none() {
                        tops[i] = Some((y, block as u8));
                    }
                    if floors[i].is_none() && is_ground(block) {
                        floors[i] = Some(y);
                    }
                }
            }
        }
        if floors.iter().all(Option::is_some) {
            break;
        }
    }
    (0..size * size)
        .map(|i| {
            let x = cx * CHUNK_SIZE + (i % size) as i32;
            let z = cz * CHUNK_SIZE + (i / size) as i32;
            let (top, block) = tops[i].unwrap_or((-1, AIR));
            let biome = terrain.column_shape(x, z).biome as u8;
            (top, block, floors[i].unwrap_or(-1), biome)
        })
        .collect()
}

fn map(seed: u32, x0: i32, z0: i32, size: i32, out: &str) {
    let generator = WorldGenerator::new(seed);
    let terrain = TerrainNoises::new(seed);
    let chunks = size.div_euclid(CHUNK_SIZE);
    let (cx0, cz0) = (x0.div_euclid(CHUNK_SIZE), z0.div_euclid(CHUNK_SIZE));
    let positions: Vec<(i32, i32)> = (0..chunks)
        .flat_map(|j| (0..chunks).map(move |i| (cx0 + i, cz0 + j)))
        .collect();
    let columns: HashMap<(i32, i32), Vec<_>> = positions
        .iter()
        .copied()
        .zip(parallel(&positions, |&(cx, cz)| {
            chunk_column_tops(&generator, &terrain, cx, cz)
        }))
        .collect();

    let width = (chunks * CHUNK_SIZE) as usize;
    let mut data = Vec::with_capacity(width * width * 6);
    for z in 0..width as i32 {
        for x in 0..width as i32 {
            let column = &columns[&(cx0 + x / CHUNK_SIZE, cz0 + z / CHUNK_SIZE)];
            let (top, block, floor, biome) =
                column[(z % CHUNK_SIZE * CHUNK_SIZE + x % CHUNK_SIZE) as usize];
            data.extend(top.to_le_bytes());
            data.push(block);
            data.extend(floor.to_le_bytes());
            data.push(biome);
        }
    }
    write(out, width, width, &data);
}

fn slice(seed: u32, z: i32, x0: i32, width: i32, out: &str) {
    let generator = WorldGenerator::new(seed);
    let cz = z.div_euclid(CHUNK_SIZE);
    let positions: Vec<IVec3> = (x0.div_euclid(CHUNK_SIZE)
        ..=(x0 + width - 1).div_euclid(CHUNK_SIZE))
        .flat_map(|cx| (0..SLICE_TOP / CHUNK_SIZE).map(move |cy| IVec3::new(cx, cy, cz)))
        .collect();
    let chunks: HashMap<IVec3, Chunk> = positions
        .iter()
        .copied()
        .zip(parallel(&positions, |&pos| generator.generate_chunk(pos)))
        .collect();
    let mut data = Vec::with_capacity((width * SLICE_TOP) as usize);
    for y in (0..SLICE_TOP).rev() {
        for x in x0..x0 + width {
            let pos = IVec3::new(x, y, z);
            let chunk = &chunks[&pos.div_euclid(IVec3::splat(CHUNK_SIZE))];
            let local = pos.rem_euclid(IVec3::splat(CHUNK_SIZE));
            data.push(block_at(chunk, local).map_or(AIR, |b| b as u8));
        }
    }
    write(out, width as usize, SLICE_TOP as usize, &data);
}

fn layer(seed: u32, y: i32, x0: i32, z0: i32, size: i32, out: &str) {
    let generator = WorldGenerator::new(seed);
    let cy = y.div_euclid(CHUNK_SIZE);
    let range =
        |start: i32| start.div_euclid(CHUNK_SIZE)..=(start + size - 1).div_euclid(CHUNK_SIZE);
    let positions: Vec<IVec3> = range(z0)
        .flat_map(|cz| range(x0).map(move |cx| IVec3::new(cx, cy, cz)))
        .collect();
    let chunks: HashMap<IVec3, Chunk> = positions
        .iter()
        .copied()
        .zip(parallel(&positions, |&pos| generator.generate_chunk(pos)))
        .collect();
    let mut data = Vec::with_capacity((size * size) as usize);
    for z in z0..z0 + size {
        for x in x0..x0 + size {
            let pos = IVec3::new(x, y, z);
            let chunk = &chunks[&pos.div_euclid(IVec3::splat(CHUNK_SIZE))];
            let local = pos.rem_euclid(IVec3::splat(CHUNK_SIZE));
            data.push(block_at(chunk, local).map_or(AIR, |b| b as u8));
        }
    }
    write(out, size as usize, size as usize, &data);
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let start = std::time::Instant::now();
    match args.first().map(String::as_str) {
        Some("map") => map(
            arg(&args, 1),
            arg(&args, 2),
            arg(&args, 3),
            arg(&args, 4),
            &args[5],
        ),
        Some("slice") => slice(
            arg(&args, 1),
            arg(&args, 2),
            arg(&args, 3),
            arg(&args, 4),
            &args[5],
        ),
        Some("layer") => layer(
            arg(&args, 1),
            arg(&args, 2),
            arg(&args, 3),
            arg(&args, 4),
            arg(&args, 5),
            &args[6],
        ),
        _ => panic!("expected map, slice or layer, see the doc comment"),
    }
    eprintln!("dumped in {:?}", start.elapsed());
}
