//! A benchmark of the game's own systems in a world: the time `Update` and `PostUpdate` take in
//! a frame, without drawing it. `cargo test` skips it; run it in release:
//!
//! ```sh
//! cargo test -p client --release e2e::bench -- --ignored --nocapture --test-threads 1
//! ```

use std::time::Instant;

use bevy::prelude::*;

use super::game::enter_world;
use super::seed::GameFolder;

#[test]
#[ignore = "benchmark: run it in release with --ignored"]
fn bench_game_systems() {
    let mut h = enter_world(GameFolder::new().with_world("Bench"), "Bench");
    // The render distance arrives and is meshed
    h.frames(200);
    let mut samples = Vec::new();
    for _ in 0..2000 {
        let start = Instant::now();
        h.world_mut().run_schedule(Update);
        h.world_mut().run_schedule(PostUpdate);
        samples.push(start.elapsed().as_secs_f64() * 1e6);
    }
    samples.sort_by(f64::total_cmp);
    println!(
        "game systems (Update + PostUpdate): median {:.1} µs, p95 {:.1} µs per frame",
        samples[samples.len() / 2],
        samples[samples.len() * 95 / 100]
    );
}

/// A player enters a world: the chunks of its render distance arrive from the integrated server
/// and are meshed. Counts the meshes started until none has been for two seconds.
#[test]
#[ignore = "benchmark: run it in release with --ignored"]
fn bench_join_meshing() {
    use crate::world::rendering::render::MESHES_STARTED;
    use crate::world::ClientWorldMap;
    use std::sync::atomic::Ordering;

    let mut h = enter_world(GameFolder::new().with_world("Join"), "Join");
    let start = Instant::now();
    let mut last_change = Instant::now();
    let mut last = (usize::MAX, usize::MAX);
    while last_change.elapsed().as_secs_f64() < 2. && start.elapsed().as_secs() < 120 {
        h.frames(1);
        let now = (
            MESHES_STARTED.load(Ordering::Relaxed),
            h.world()
                .resource::<ClientWorldMap>()
                .blocks
                .chunk_positions()
                .count(),
        );
        if now != last {
            last = now;
            last_change = Instant::now();
        }
    }
    let (meshes, chunks) = last;
    let drawn = h
        .world()
        .resource::<ClientWorldMap>()
        .render
        .values()
        .filter(|r| r.entity.is_some() || r.liquid_entity.is_some())
        .count();
    println!(
        "join meshing: {chunks} chunks, {drawn} drawn, {meshes} meshes started ({:.2} a chunk)",
        meshes as f64 / chunks as f64
    );
}
