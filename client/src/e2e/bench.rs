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
