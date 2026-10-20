//! A benchmark of the game's own systems in a world: the time `Update` and `PostUpdate` take in
//! a frame, without drawing it. `cargo test` skips it; run it in release:
//!
//! ```sh
//! cargo test -p client --release e2e::bench -- --ignored --nocapture --test-threads 1
//! ```

use std::time::Instant;

use accesskit::Role;
use bevy::{input::mouse::MouseMotion, prelude::*};

use super::game::enter_world;
use super::harness::{SCREEN, SERVER};
use super::helpers::{ctrl, is_in};
use super::locator::Locator as L;
use super::seed::GameFolder;
use crate::state::GameState;
use crate::ui::menus::pause::WORLD_SAVED;
use crate::ui::menus::MenuState;

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
/// of the render distance received and drawn, Save and quit to title; then the same world opened
/// again, Save now, and Save and quit. Times each step, the loads from the press of Ctrl+Enter,
/// and the longest frames while the chunks come in.
///
/// `BENCH_WORLD` is the folder of a saved world to copy and play, as the player `BENCH_PLAYER`
/// (default `PlayerOne`); without it, a new world is generated. The render distance is 8.
/// `BENCH_DISK` is the folder to put the game folder in: by default it is in the temporary
/// folder, often in memory, where syncing a file costs nothing.
#[test]
#[ignore = "benchmark: run it in release with --ignored"]
fn bench_world_lifecycle() {
    use super::harness::{Options, STARTUP};

    let folder = match std::env::var_os("BENCH_DISK") {
        Some(disk) => GameFolder::new_in(std::path::Path::new(&disk)),
        None => GameFolder::new(),
    }
    .with_file("video.ron", "(render_distance: 8)");
    let (folder, world) = match std::env::var("BENCH_WORLD") {
        Ok(source) => (
            folder.with_world_copy("Saved", std::path::Path::new(&source)),
            "Saved",
        ),
        Err(_) => (folder.with_world("New"), "New"),
    };
    // The copy is on the disk before the world opens, as a world saved long ago
    let _ = std::process::Command::new("sync").status();
    let player = std::env::var("BENCH_PLAYER").unwrap_or_else(|_| "PlayerOne".to_string());
    let mut h = super::harness::Harness::start(
        folder,
        Options {
            width: 1920,
            height: 1080,
            player_name: Some(player),
        },
    );
    h.wait_for(&L::text("Singleplayer"), STARTUP);
    h.frames(10);

    let first = load_with_ctrl_enter(&mut h);
    let first_quit = save_and_quit(&mut h);
    h.frames(10);
    let again = load_with_ctrl_enter(&mut h);
    let still = frame_times(&mut h, 300, |_| {});
    h.key_down(KeyCode::KeyW);
    let walking = frame_times(&mut h, 600, |h| {
        h.world_mut().write_message(MouseMotion {
            delta: Vec2::new(4.0, 0.0),
        });
    });
    h.key_up(KeyCode::KeyW);

    h.press(KeyCode::Escape);
    h.wait_for(&L::role(Role::Button, "Save now"), SCREEN);
    let save = Instant::now();
    h.click(&L::role(Role::Button, "Save now"));
    h.wait_for(&L::role(Role::Status, WORLD_SAVED), SERVER);
    let save_now = save.elapsed().as_secs_f64();
    let quit_after_save = save_and_quit(&mut h);

    let result = serde_json::json!({
        "world": world,
        "first": first,
        "save_and_quit_s": first_quit,
        "again": again,
        "save_now_s": save_now,
        "save_and_quit_after_save_now_s": quit_after_save,
        "still": still,
        "walking": walking,
    });
    println!("BENCH_RESULT {result}");
}

/// Presses Ctrl+Enter on the main menu, and times the game, the first mesh shown, every chunk of
/// the render distance (8) held, and the last mesh shown before none is for 1.5 s.
fn load_with_ctrl_enter(h: &mut super::harness::Harness) -> serde_json::Value {
    use std::sync::atomic::Ordering;

    use crate::network::session::{ConnectStep, ConnectWatch};
    use crate::player::CurrentPlayerMarker;
    use crate::world::rendering::render::MESHES_SHOWN;
    use crate::world::ClientWorldMap;
    use shared::world::world_position_to_chunk_position;

    const RADIUS: i32 = 8;
    let shown_before = MESHES_SHOWN.load(Ordering::Relaxed);
    let start = Instant::now();
    ctrl(h, KeyCode::Enter);
    // The steps of the connecting card, as they change
    let mut steps: Vec<(String, f64)> = Vec::new();
    h.wait_until("the game", SERVER, |w| {
        let step = w
            .get_resource::<ConnectWatch>()
            .map(|watch| match watch.step {
                ConnectStep::LoadingChunks { .. } => "LoadingChunks".to_string(),
                step => format!("{step:?}"),
            });
        if let Some(step) = step {
            if steps.last().is_none_or(|(last, _)| *last != step) {
                steps.push((step, start.elapsed().as_secs_f64()));
            }
        }
        is_in(w, GameState::Game)
    });
    let in_game = start.elapsed().as_secs_f64();

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
        let held = world
            .resource::<ClientWorldMap>()
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
    let (held, shown) = last;
    frames.sort_by(f64::total_cmp);
    let percentile = |p: f64| frames[((frames.len() - 1) as f64 * p) as usize];
    serde_json::json!({
        "steps": steps,
        "in_game_s": in_game,
        "first_mesh_s": first_mesh,
        "all_chunks_s": all_chunks,
        "all_drawn_s": last_change,
        "chunks": held,
        "meshes_shown": shown,
        "frames": frames.len(),
        "frame_ms_p50": percentile(0.5),
        "frame_ms_p99": percentile(0.99),
        "frame_ms_max": percentile(1.0),
    })
}

/// Times `count` frames, each after `before` acted on the game: their median, 99th percentile
/// and longest, and how many there are per second.
fn frame_times(
    h: &mut super::harness::Harness,
    count: usize,
    mut before: impl FnMut(&mut super::harness::Harness),
) -> serde_json::Value {
    let start = Instant::now();
    let mut frames: Vec<f64> = (0..count)
        .map(|_| {
            before(h);
            let frame = Instant::now();
            h.frames(1);
            frame.elapsed().as_secs_f64() * 1e3
        })
        .collect();
    let fps = count as f64 / start.elapsed().as_secs_f64();
    frames.sort_by(f64::total_cmp);
    let percentile = |p: f64| frames[((frames.len() - 1) as f64 * p) as usize];
    serde_json::json!({
        "fps": fps,
        "frame_ms_p50": percentile(0.5),
        "frame_ms_p99": percentile(0.99),
        "frame_ms_max": percentile(1.0),
    })
}

/// Opens the pause menu unless it is open, clicks Save and quit to title, and times it until the
/// main menu.
fn save_and_quit(h: &mut super::harness::Harness) -> f64 {
    if !h.is_visible(&L::role(Role::Button, "Save and quit to title")) {
        h.press(KeyCode::Escape);
    }
    h.wait_for(&L::role(Role::Button, "Save and quit to title"), SCREEN);
    let quit = Instant::now();
    h.click(&L::role(Role::Button, "Save and quit to title"));
    h.wait_until("the main menu", SERVER, |w| {
        is_in(w, GameState::Menu) && *w.resource::<State<MenuState>>().get() == MenuState::Main
    });
    quit.elapsed().as_secs_f64()
}

/// A new world played at the render distance `BENCH_RD` (default 8): Ctrl+Enter, then every
/// chunk received and drawn, then frames looking ahead and frames walking, which brings new
/// chunks in, then Save and quit to title. Says how long the world took to be drawn, how many
/// chunks, entities and triangles it holds, the memory of the process, the frame times, and
/// how long the quit took. `BENCH_DISK` is as in `bench_world_lifecycle`.
#[test]
#[ignore = "benchmark: run it in release with --ignored"]
fn bench_render_distance() {
    use std::sync::atomic::Ordering;

    use super::harness::{Options, STARTUP};
    use crate::world::rendering::render::{MESHES_SHOWN, MESHES_STARTED};
    use crate::world::ClientWorldMap;

    let radius: u32 = std::env::var("BENCH_RD")
        .ok()
        .and_then(|rd| rd.parse().ok())
        .unwrap_or(8);
    let folder = match std::env::var_os("BENCH_DISK") {
        Some(disk) => GameFolder::new_in(std::path::Path::new(&disk)),
        None => GameFolder::new(),
    }
    .with_file("video.ron", &format!("(render_distance: {radius})"))
    .with_world("Far");
    let mut h = super::harness::Harness::start(
        folder,
        Options {
            width: 1920,
            height: 1080,
            player_name: Some("PlayerOne".to_string()),
        },
    );
    h.wait_for(&L::text("Singleplayer"), STARTUP);
    h.frames(10);

    let start = Instant::now();
    ctrl(&mut h, KeyCode::Enter);
    h.wait_until("the game", SERVER, |w| is_in(w, GameState::Game));
    let in_game = start.elapsed().as_secs_f64();

    // Drawn once neither the chunks held nor the meshes shown changed for two seconds
    let shown_before = MESHES_SHOWN.load(Ordering::Relaxed);
    let started_before = MESHES_STARTED.load(Ordering::Relaxed);
    let mut last = (usize::MAX, usize::MAX);
    let mut last_change = start.elapsed().as_secs_f64();
    let mut loading = Vec::new();
    // The chunks held and the meshes shown every quarter of a second
    let mut curve = Vec::new();
    loop {
        let frame = Instant::now();
        h.frames(1);
        loading.push(frame.elapsed().as_secs_f64() * 1e3);
        let now = start.elapsed().as_secs_f64();
        let held = h
            .world()
            .resource::<ClientWorldMap>()
            .blocks
            .chunk_positions()
            .count();
        let shown = MESHES_SHOWN.load(Ordering::Relaxed) - shown_before;
        if (held, shown) != last {
            last = (held, shown);
            last_change = now;
        }
        if now >= curve.len() as f64 * 0.25 {
            curve.push(serde_json::json!([now, held, shown]));
        }
        if now - last_change > 2. || now > 600. {
            break;
        }
    }
    let mut drawn = scene_stats(&mut h);
    drawn["meshes_started"] = (MESHES_STARTED.load(Ordering::Relaxed) - started_before).into();
    // A picture of the world drawn, for a human to look at the horizon
    h.snapshot(&format!("rd{radius}"));
    phase("still");
    let still = frame_times(&mut h, 600, |_| {});
    phase("walking");
    let walking = walk_for(&mut h, 10.);
    phase("quit");
    let after_walk = scene_stats(&mut h);
    let memory = memory();
    let quit = save_and_quit(&mut h);

    let result = serde_json::json!({
        "radius": radius,
        "in_game_s": in_game,
        "all_drawn_s": last_change,
        "loading": percentiles(&mut loading),
        "loading_curve": curve,
        "drawn": drawn,
        "still": still,
        "walking": walking,
        "after_walk": after_walk,
        "memory": memory,
        "save_and_quit_s": quit,
    });
    println!("BENCH_RESULT {result}");
}

/// Says when a phase of a benchmark starts, in ms since the Unix epoch, to find it in a profile.
fn phase(name: &str) {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH);
    println!("BENCH_PHASE {name} {}", now.unwrap_or_default().as_millis());
}

/// The chunks the client holds, the chunk entities and the triangles of their meshes.
fn scene_stats(h: &mut super::harness::Harness) -> serde_json::Value {
    use crate::world::ClientWorldMap;

    let map = h.world().resource::<ClientWorldMap>();
    let renders = map.render.values();
    let entities = renders
        .clone()
        .flat_map(|render| [render.entity, render.liquid_entity])
        .flatten()
        .count();
    let liquid_entities = renders
        .clone()
        .filter(|render| render.liquid_entity.is_some())
        .count();
    let triangles: usize = renders
        .map(|render| render.triangles.iter().sum::<usize>())
        .sum();
    serde_json::json!({
        "chunks": map.blocks.chunk_positions().count(),
        "entities": entities,
        "liquid_entities": liquid_entities,
        "triangles": triangles,
    })
}

/// A picture of the horizon at the render distance `BENCH_RD` (default 8), for a human to look
/// at: the player of a new world flies up to y = 150 and looks ahead and a little down, once
/// every chunk is drawn. Saved with `UI_SNAPSHOTS=1`, as `horizon_rd{distance}`.
#[test]
#[ignore = "a picture for a human: run it in release with --ignored and UI_SNAPSHOTS=1"]
fn snapshot_the_horizon() {
    use super::harness::{Options, STARTUP};
    use crate::player::CurrentPlayerMarker;
    use crate::world::ClientWorldMap;

    let radius: u32 = std::env::var("BENCH_RD")
        .ok()
        .and_then(|rd| rd.parse().ok())
        .unwrap_or(8);
    let folder = GameFolder::new()
        .with_file("video.ron", &format!("(render_distance: {radius})"))
        .with_world("Far");
    let mut h = super::harness::Harness::start(
        folder,
        Options {
            width: 1920,
            height: 1080,
            player_name: Some("PlayerOne".to_string()),
        },
    );
    h.wait_for(&L::text("Singleplayer"), STARTUP);
    h.frames(10);
    ctrl(&mut h, KeyCode::Enter);
    h.wait_until("the game", SERVER, |w| is_in(w, GameState::Game));

    h.key_down(KeyCode::KeyF);
    h.frames(2);
    h.key_up(KeyCode::KeyF);
    h.key_down(KeyCode::Space);
    h.wait_until("the player up in the air", SERVER, |w| {
        let mut players = w
            .try_query_filtered::<&Transform, With<CurrentPlayerMarker>>()
            .unwrap();
        players.iter(w).any(|player| player.translation.y > 150.)
    });
    h.key_up(KeyCode::Space);
    h.world_mut().write_message(MouseMotion {
        delta: Vec2::new(0., 40.),
    });

    // Drawn once the chunks held did not change for two seconds
    let mut last = (usize::MAX, Instant::now());
    while last.1.elapsed().as_secs_f64() < 2. {
        h.frames(1);
        let held = h
            .world()
            .resource::<ClientWorldMap>()
            .blocks
            .chunk_positions()
            .count();
        if held != last.0 {
            last = (held, Instant::now());
        }
    }
    h.frames(30);
    h.snapshot(&format!("horizon_rd{radius}"));
}

/// Walks ahead for `seconds`, turning slowly, and times the frames.
fn walk_for(h: &mut super::harness::Harness, seconds: f64) -> serde_json::Value {
    h.key_down(KeyCode::KeyW);
    let start = Instant::now();
    let mut frames = Vec::new();
    while start.elapsed().as_secs_f64() < seconds {
        h.world_mut().write_message(MouseMotion {
            delta: Vec2::new(0.5, 0.0),
        });
        let frame = Instant::now();
        h.frames(1);
        frames.push(frame.elapsed().as_secs_f64() * 1e3);
    }
    h.key_up(KeyCode::KeyW);
    let fps = frames.len() as f64 / start.elapsed().as_secs_f64();
    let mut result = percentiles(&mut frames);
    result["fps"] = fps.into();
    result
}

/// The median, 99th percentile and longest of `frames`, in ms, and how many there are.
fn percentiles(frames: &mut [f64]) -> serde_json::Value {
    frames.sort_by(f64::total_cmp);
    let percentile = |p: f64| frames[((frames.len() - 1) as f64 * p) as usize];
    serde_json::json!({
        "frames": frames.len(),
        "frame_ms_p50": percentile(0.5),
        "frame_ms_p99": percentile(0.99),
        "frame_ms_max": percentile(1.0),
    })
}

/// The resident memory of the process and its peak, in MiB.
fn memory() -> serde_json::Value {
    let status = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
    let field = |name: &str| {
        status
            .lines()
            .find_map(|line| line.strip_prefix(name))
            .and_then(|rest| rest.trim().trim_end_matches(" kB").parse::<f64>().ok())
            .map(|kib| kib / 1024.)
    };
    serde_json::json!({ "rss_mib": field("VmRSS:"), "peak_mib": field("VmHWM:") })
}
