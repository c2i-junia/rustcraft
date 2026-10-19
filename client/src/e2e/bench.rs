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

/// The life of a world as a player meets it: Ctrl+Enter on the main menu, the game, every chunk
/// of the render distance received and drawn, Save now, then Save and quit to title. Times each
/// step from the press of Ctrl+Enter, and the longest frames while the chunks come in.
///
/// `BENCH_WORLD` is the folder of a saved world to copy and play, as the player `BENCH_PLAYER`
/// (default `PlayerOne`); without it, a new world is generated. The render distance is 8.
#[test]
#[ignore = "benchmark: run it in release with --ignored"]
fn bench_world_lifecycle() {
    use std::sync::atomic::Ordering;

    use accesskit::Role;

    use super::harness::{Options, SERVER};
    use super::helpers::{ctrl, is_in};
    use super::locator::Locator as L;
    use crate::player::CurrentPlayerMarker;
    use crate::state::GameState;
    use crate::ui::menus::pause::WORLD_SAVED;
    use crate::ui::menus::MenuState;
    use crate::world::rendering::render::MESHES_SHOWN;
    use crate::world::ClientWorldMap;
    use shared::world::world_position_to_chunk_position;

    const RADIUS: i32 = 8;
    let folder = GameFolder::new().with_file("video.ron", "(render_distance: 8)");
    let (folder, world) = match std::env::var("BENCH_WORLD") {
        Ok(source) => (
            folder.with_world_copy("Saved", std::path::Path::new(&source)),
            "Saved",
        ),
        Err(_) => (folder.with_world("New"), "New"),
    };
    let player = std::env::var("BENCH_PLAYER").unwrap_or_else(|_| "PlayerOne".to_string());
    let mut h = super::harness::Harness::start(
        folder,
        Options {
            player_name: Some(player),
            ..Options::default()
        },
    );
    h.wait_for(&L::text("Singleplayer"), super::harness::STARTUP);
    h.frames(10);

    let shown_before = MESHES_SHOWN.load(Ordering::Relaxed);
    let start = Instant::now();
    ctrl(&mut h, KeyCode::Enter);
    h.wait_until("the game", SERVER, |w| is_in(w, GameState::Game));
    let in_game = start.elapsed().as_secs_f64();

    // Every chunk of the render distance in, then no mesh shown for 1.5 s
    let wanted = (2 * RADIUS + 1).pow(3) as usize;
    let mut all_chunks = None;
    let mut last = (usize::MAX, usize::MAX);
    let mut last_change = start.elapsed().as_secs_f64();
    let mut frames = Vec::new();
    let mut first_mesh = None;
    loop {
        let frame = Instant::now();
        h.frames(1);
        frames.push(frame.elapsed().as_secs_f64() * 1e3);
        let now = start.elapsed().as_secs_f64();
        let world = h.world_mut();
        let center = world
            .query_filtered::<&Transform, With<CurrentPlayerMarker>>()
            .single(world)
            .map(|t| world_position_to_chunk_position(t.translation))
            .unwrap_or_default();
        let map = world.resource::<ClientWorldMap>();
        let held = map
            .blocks
            .chunk_positions()
            .filter(|c| (*c - center).abs().max_element() <= RADIUS)
            .count();
        let shown = MESHES_SHOWN.load(Ordering::Relaxed) - shown_before;
        if shown > 0 && first_mesh.is_none() {
            first_mesh = Some(now);
        }
        if held >= wanted && all_chunks.is_none() {
            all_chunks = Some(now);
        }
        if (held, shown) != last {
            last = (held, shown);
            last_change = now;
        }
        if (all_chunks.is_some() && now - last_change > 1.5) || now > 120. {
            break;
        }
    }
    let drawn = last_change;
    let (held, shown) = last;
    frames.sort_by(f64::total_cmp);
    let percentile = |p: f64| frames[((frames.len() - 1) as f64 * p) as usize];

    h.press(KeyCode::Escape);
    h.wait_for(&L::role(Role::Button, "Save now"), super::harness::SCREEN);
    let save = Instant::now();
    h.click(&L::role(Role::Button, "Save now"));
    h.wait_for(&L::role(Role::Status, WORLD_SAVED), SERVER);
    let save_now = save.elapsed().as_secs_f64();

    let quit = Instant::now();
    h.click(&L::role(Role::Button, "Save and quit to title"));
    h.wait_until("the main menu", SERVER, |w| {
        is_in(w, GameState::Menu) && *w.resource::<State<MenuState>>().get() == MenuState::Main
    });
    let save_and_quit = quit.elapsed().as_secs_f64();

    let result = serde_json::json!({
        "world": world,
        "in_game_s": in_game,
        "first_mesh_s": first_mesh,
        "all_chunks_s": all_chunks,
        "all_drawn_s": drawn,
        "chunks": held,
        "meshes_shown": shown,
        "load_frames": frames.len(),
        "load_frame_ms_p50": percentile(0.5),
        "load_frame_ms_p99": percentile(0.99),
        "load_frame_ms_max": percentile(1.0),
        "save_now_s": save_now,
        "save_and_quit_s": save_and_quit,
    });
    println!("BENCH_RESULT {result}");
}
