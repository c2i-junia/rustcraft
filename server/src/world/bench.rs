//! Benchmarks of the server's work for the chunks around players. `cargo test` skips them; run
//! them in release, one at a time:
//!
//! ```sh
//! cargo test -p server --release --no-default-features bench_ -- --ignored --nocapture --test-threads 1
//! ```

use std::collections::{HashMap, HashSet};
use std::hint::black_box;
use std::time::Instant;

use bevy::prelude::*;
use shared::messages::{ServerToClientMessage, WorldUpdate};
use shared::world::{Chunk, ChunkMap};
use shared::{game_message_to_payload, CHUNK_SIZE, DEFAULT_MAX_RENDER_DISTANCE};

use super::broadcast_world::world_messages_around;
use super::generation::generate_chunk;

const SEED: u32 = 42;
/// The chunk of a player standing on the plains of `SEED`, around height 64.
const PLAYER_CHUNK: IVec3 = IVec3::new(0, 4, 0);

fn render_cube(radius: i32) -> Vec<IVec3> {
    let mut chunks = Vec::new();
    for x in -radius..=radius {
        for y in -radius..=radius {
            for z in -radius..=radius {
                chunks.push(PLAYER_CHUNK + IVec3::new(x, y, z));
            }
        }
    }
    chunks
}

fn micros(since: Instant) -> f64 {
    since.elapsed().as_secs_f64() * 1e6
}

fn median(mut samples: Vec<f64>) -> f64 {
    samples.sort_by(f64::total_cmp);
    samples[samples.len() / 2]
}

/// Generates every chunk a player sees at the default maximum render distance, as on joining,
/// and splits the time between the chunks of air, of the surface and of stone underground.
#[test]
#[ignore = "benchmark: run it in release with --ignored"]
fn bench_generation() {
    let radius = DEFAULT_MAX_RENDER_DISTANCE as i32;
    let mut by_kind: HashMap<&str, Vec<f64>> = HashMap::new();
    let start = Instant::now();
    for chunk_pos in render_cube(radius) {
        let one = Instant::now();
        let chunk = black_box(generate_chunk(chunk_pos, SEED));
        let time = micros(one);
        let full = chunk.iter().count();
        let kind = match full {
            0 => "air",
            n if n == (CHUNK_SIZE.pow(3)) as usize => "full",
            _ => "surface",
        };
        by_kind.entry(kind).or_default().push(time);
    }
    let total = start.elapsed().as_secs_f64() * 1e3;
    println!(
        "generation: {} chunks (radius {radius}) in {total:.1} ms on one thread",
        render_cube(radius).len()
    );
    for (kind, samples) in by_kind {
        println!(
            "  {kind:8} {:5} chunks, median {:7.1} µs, sum {:7.1} ms",
            samples.len(),
            median(samples.clone()),
            samples.iter().sum::<f64>() / 1e3
        );
    }
}

/// One tick of the broadcast for one player who already has every chunk: the work done for
/// each player on every tick while nobody moves.
#[test]
#[ignore = "benchmark: run it in release with --ignored"]
fn bench_broadcast_steady() {
    let radius = DEFAULT_MAX_RENDER_DISTANCE as i32;
    let mut chunks = ChunkMap::default();
    for chunk_pos in render_cube(radius) {
        chunks.insert_chunk(chunk_pos, Chunk::new());
    }
    let mut sent: HashSet<IVec3> = render_cube(radius).into_iter().collect();
    let mut samples = Vec::new();
    for _ in 0..2000 {
        let start = Instant::now();
        let messages = world_messages_around(&chunks, &[], PLAYER_CHUNK, radius, &mut sent);
        samples.push(micros(start));
        assert!(messages.2.is_empty());
        black_box(messages);
    }
    println!(
        "broadcast, nothing to send: median {:.1} µs per player per tick",
        median(samples)
    );
}

/// What sending the ten nearest chunks costs: bincode then lz4, the size on the wire, and the
/// time it takes at the cap of ten chunks per tick to send a whole render distance.
#[test]
#[ignore = "benchmark: run it in release with --ignored"]
fn bench_chunk_messages() {
    let radius = DEFAULT_MAX_RENDER_DISTANCE as i32;
    let mut cube = render_cube(radius);
    cube.sort_by_key(|c| (*c - PLAYER_CHUNK).length_squared());
    let generated: Vec<(IVec3, Chunk)> =
        cube.iter().map(|&c| (c, generate_chunk(c, SEED))).collect();

    let nearest: HashMap<IVec3, Chunk> = generated.iter().take(10).cloned().collect();
    let message = || {
        ServerToClientMessage::WorldUpdate(WorldUpdate {
            tick: 1,
            time: 1,
            new_map: nearest.clone(),
            mobs: HashMap::new(),
        })
    };
    let mut samples = Vec::new();
    let mut bytes = 0;
    for _ in 0..200 {
        let message = message();
        let start = Instant::now();
        bytes = black_box(game_message_to_payload(message)).len();
        samples.push(micros(start));
    }
    println!(
        "ten nearest chunks: {bytes} bytes on the wire, encoded in {:.1} µs (median)",
        median(samples)
    );

    let empty = generated
        .iter()
        .filter(|(_, c)| c.iter().next().is_none())
        .count();
    let full = generated
        .iter()
        .filter(|(_, c)| c.iter().count() == CHUNK_SIZE.pow(3) as usize)
        .count();
    let total_bytes: usize = generated
        .chunks(10)
        .map(|batch| {
            game_message_to_payload(ServerToClientMessage::WorldUpdate(WorldUpdate {
                tick: 1,
                time: 1,
                new_map: batch.iter().cloned().collect(),
                mobs: HashMap::new(),
            }))
            .len()
        })
        .sum();
    println!(
        "render distance {radius}: {} chunks ({empty} of air, {full} full), {} KiB in all, \
         {} ticks at 10 chunks per tick = {:.1} s at 20 Hz",
        generated.len(),
        total_bytes / 1024,
        generated.len().div_ceil(10),
        generated.len().div_ceil(10) as f64 / 20.
    );
}

/// `ChunkMap::get_block` on random positions of a loaded region, as collisions, raycasts and
/// the liquids read blocks.
#[test]
#[ignore = "benchmark: run it in release with --ignored"]
fn bench_block_lookups() {
    let mut chunks = ChunkMap::default();
    for chunk_pos in render_cube(3) {
        chunks.insert_chunk(chunk_pos, generate_chunk(chunk_pos, SEED));
    }
    let span = 7 * CHUNK_SIZE;
    let origin = (PLAYER_CHUNK - IVec3::splat(3)) * CHUNK_SIZE;
    let mut state = 0x2545_f491_u32;
    let positions: Vec<IVec3> = (0..1_000_000)
        .map(|_| {
            let mut next = || {
                state ^= state << 13;
                state ^= state >> 17;
                state ^= state << 5;
                (state % span as u32) as i32
            };
            origin + IVec3::new(next(), next(), next())
        })
        .collect();
    let mut samples = Vec::new();
    for _ in 0..5 {
        let start = Instant::now();
        let found = positions
            .iter()
            .filter(|p| chunks.get_block(**p).is_some())
            .count();
        black_box(found);
        samples.push(micros(start) * 1e3 / positions.len() as f64);
    }
    println!("get_block: median {:.1} ns per lookup", median(samples));
}
