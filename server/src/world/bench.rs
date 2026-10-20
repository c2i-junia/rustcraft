//! Benchmarks of the server's work for the chunks around players. `cargo test` skips them; run
//! them in release, one at a time:
//!
//! ```sh
//! cargo test -p server --release --no-default-features bench_ -- --ignored --nocapture --test-threads 1
//! ```

use std::collections::{HashMap, HashSet};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::hint::black_box;
use std::time::{Duration, Instant};

use bevy::prelude::*;
use shared::messages::{ServerToClientMessage, WorldUpdate};
use shared::players::Player;
use shared::world::{render_distance_offsets, Chunk, ChunkMap};
use shared::{game_message_to_payload, CHUNK_SIZE, TICKS_PER_SECOND};

use super::broadcast_world::world_messages_around;
use super::data::SentChunks;
use super::generation::{generate_chunk, WorldGenerator};
use super::simulation::{generate_missing_chunks, ChunkLoader};

const SEED: u32 = 42;
/// The chunk of a player standing on the plains of `SEED`, around height 64.
const PLAYER_CHUNK: IVec3 = IVec3::new(0, 4, 0);

fn render_cube(radius: i32) -> Vec<IVec3> {
    render_distance_offsets(radius)
        .iter()
        .map(|&offset| PLAYER_CHUNK + offset)
        .collect()
}

fn micros(since: Instant) -> f64 {
    since.elapsed().as_secs_f64() * 1e6
}

/// The render distance of the benchmarks, the default maximum of a server when they were
/// written: their measures stay comparable.
const BENCH_RADIUS: i32 = 6;

fn median(mut samples: Vec<f64>) -> f64 {
    samples.sort_by(f64::total_cmp);
    samples[samples.len() / 2]
}

/// Generates every chunk a player sees within `BENCH_RADIUS`, as on joining,
/// and splits the time between the chunks of air, of the surface and of stone underground.
#[test]
#[ignore = "benchmark: run it in release with --ignored"]
fn bench_generation() {
    let radius = BENCH_RADIUS;
    let mut by_kind: HashMap<&str, Vec<f64>> = HashMap::new();
    let mut checksum = DefaultHasher::new();
    let start = Instant::now();
    for chunk_pos in render_cube(radius) {
        let one = Instant::now();
        let chunk = black_box(generate_chunk(chunk_pos, SEED));
        let time = micros(one);
        for (pos, block) in chunk.iter() {
            (chunk_pos, pos, block.id.number() as u32).hash(&mut checksum);
        }
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
        "generation: {} chunks (radius {radius}) in {total:.1} ms on one thread, checksum {:016x}",
        render_cube(radius).len(),
        checksum.finish()
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
    let radius = BENCH_RADIUS;
    let mut chunks = ChunkMap::default();
    for chunk_pos in render_cube(radius) {
        chunks.insert_chunk(chunk_pos, Chunk::new());
    }
    let mut sent = SentChunks {
        chunks: render_cube(radius).into_iter().collect(),
        progress: None,
    };
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
    let radius = BENCH_RADIUS;
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

/// A player joins an empty world: the ticks of the server generate its render distance. The
/// longest tick is the freeze every player sees.
#[test]
#[ignore = "benchmark: run it in release with --ignored"]
fn bench_join_generation() {
    // BENCH_RD sets the render distance
    let radius = std::env::var("BENCH_RD")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(BENCH_RADIUS);
    let mut players = HashMap::new();
    let position = (PLAYER_CHUNK * CHUNK_SIZE).as_vec3() + Vec3::splat(8.);
    players.insert(
        1,
        Player::new(1, "p".into(), position, Transform::default()),
    );
    let mut loader = ChunkLoader::new(SEED);
    let mut chunks = ChunkMap::default();
    let mut dirty = HashSet::new();
    let mut asked = 0;
    // the work of each tick, the ticks coming every 50 ms as in the game
    let mut ticks = Vec::new();
    let tick = Duration::from_millis(1000 / TICKS_PER_SECOND);
    let start = Instant::now();
    loop {
        let tick_start = Instant::now();
        loader.insert_made(&mut chunks, &mut dirty, &mut Vec::new());
        let complete = loader.ask(&players, |_| radius, &chunks, None, &mut asked);
        ticks.push(tick_start.elapsed().as_secs_f64() * 1e3);
        if complete || ticks.len() == 10000 {
            break;
        }
        std::thread::sleep(tick.saturating_sub(tick_start.elapsed()));
    }
    let wall = start.elapsed().as_secs_f64() * 1e3;
    // what was generated, to check that a faster generation makes the same chunks
    let mut positions: Vec<IVec3> = chunks.chunk_positions().collect();
    positions.sort_by_key(|c| (c.x, c.y, c.z));
    let mut checksum = DefaultHasher::new();
    let mut uniform = 0;
    for &chunk_pos in &positions {
        let chunk = chunks.chunk(chunk_pos).unwrap();
        uniform += chunk.uniform().is_some() as usize;
        for (pos, block) in chunk.iter() {
            (chunk_pos, pos, block.id.number() as u32).hash(&mut checksum);
        }
    }
    println!(
        "join generation (radius {radius}): {} chunks, {uniform} of one kind of cell, in {wall:.0} \
         ms, {} ticks, longest {:.1} ms, checksum {:016x}",
        positions.len(),
        ticks.len(),
        ticks.iter().copied().fold(0., f64::max),
        checksum.finish()
    );
}

/// A player joins a generated world: the ticks of the broadcast send its render distance.
#[test]
#[ignore = "benchmark: run it in release with --ignored"]
fn bench_join_sending() {
    let radius = BENCH_RADIUS;
    let mut chunks = ChunkMap::default();
    for chunk_pos in render_cube(radius) {
        chunks.insert_chunk(chunk_pos, generate_chunk(chunk_pos, SEED));
    }
    let mut sent = SentChunks::default();
    let mut ticks = 0;
    let mut bytes = 0;
    let mut largest = 0;
    while sent.chunks.len() < render_cube(radius).len() && ticks < 10_000 {
        let (_, _, new_chunks) =
            world_messages_around(&chunks, &[], PLAYER_CHUNK, radius, &mut sent);
        let message = game_message_to_payload(ServerToClientMessage::WorldUpdate(WorldUpdate {
            tick: ticks,
            time: ticks,
            new_map: new_chunks
                .into_iter()
                .map(|(pos, chunk)| (pos, chunk.clone()))
                .collect(),
            mobs: HashMap::new(),
        }));
        bytes += message.len();
        largest = largest.max(message.len());
        ticks += 1;
    }
    println!(
        "join sending: {ticks} ticks = {:.1} s at 20 Hz, {} KiB in all, largest message {} KiB",
        ticks as f64 / 20.,
        bytes / 1024,
        largest / 1024
    );
}

/// The chunks around each player that the simulation needs, checked after each batch of inputs
/// of twenty players who send three batches per tick: the work of one tick.
#[test]
#[ignore = "benchmark: run it in release with --ignored"]
fn bench_inputs_generation() {
    let mut players = HashMap::new();
    let mut chunks = ChunkMap::default();
    for id in 0..20u64 {
        let chunk = PLAYER_CHUNK + IVec3::new(id as i32 * 4, 0, 0);
        let position = (chunk * CHUNK_SIZE).as_vec3() + Vec3::splat(8.);
        players.insert(
            id,
            Player::new(id, "p".into(), position, Transform::default()),
        );
    }
    let mut dirty = HashSet::new();
    let generator = WorldGenerator::new(SEED);
    generate_missing_chunks(
        &players,
        |_| 1,
        &mut chunks,
        &mut dirty,
        &mut Vec::new(),
        None,
        &generator,
    );
    let mut samples = Vec::new();
    for _ in 0..200 {
        let start = Instant::now();
        for _ in 0..3 {
            for player in &players {
                generate_missing_chunks(
                    [player],
                    |_| 1,
                    &mut chunks,
                    &mut dirty,
                    &mut Vec::new(),
                    None,
                    &generator,
                );
            }
        }
        samples.push(micros(start));
    }
    println!(
        "inputs generation, 20 players: median {:.1} µs per tick",
        median(samples)
    );
}

/// The bytes the updates of twenty players with a full inventory take on the wire each tick,
/// every client being sent the update of every player.
#[test]
#[ignore = "benchmark: run it in release with --ignored"]
fn bench_player_updates() {
    use shared::messages::PlayerUpdateEvent;
    use shared::world::{ItemId, ItemStack, ItemType};
    const PLAYERS: usize = 20;
    let mut player = Player::new(1, "p".into(), Vec3::ZERO, Transform::default());
    for slot in 0..shared::MAX_INVENTORY_SLOTS {
        player.inventory.inner.insert(
            slot,
            ItemStack {
                item_id: ItemId::Dirt,
                item_type: ItemType::Block(shared::world::BlockId::Dirt),
                nb: 64,
            },
        );
    }
    let size = |inventory| {
        game_message_to_payload(ServerToClientMessage::PlayerUpdate(PlayerUpdateEvent {
            id: player.id,
            movement: player.movement_state(),
            orientation: player.camera_transform.rotation,
            last_ack_time: 1234,
            inventory,
            open_chest: None,
            health: shared::players::game_mode::MAX_HEALTH,
            game_mode: Default::default(),
            death_cause: None,
        }))
        .len()
    };
    let with = size(Some(player.inventory.clone()));
    let without = size(None);
    println!(
        "player updates, {PLAYERS} players: {with} bytes with the inventory, {without} without; \
         {} KiB a tick if all carry it, {} KiB if only its owner's does",
        PLAYERS * PLAYERS * with / 1024,
        (PLAYERS * with + PLAYERS * (PLAYERS - 1) * without) / 1024
    );
}
