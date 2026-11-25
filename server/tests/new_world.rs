//! Runs the built `server` binary with the options of a new world and reads the `world.ron`
//! it writes when it is stopped.

#![cfg(unix)]

use shared::status::{StatusReply, StatusRequest};
use std::net::UdpSocket;
use std::path::Path;
use std::process::{Child, Command};
use std::time::{Duration, Instant};

const TIMEOUT: Duration = Duration::from_secs(10);
const WORLD_NAME: &str = "new-world-test";

/// Kills the child at the end of the test, whatever happened.
struct KillOnDrop(Child);

impl Drop for KillOnDrop {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// Whether the status port at `port` answers a request.
fn answers_status(port: u16) -> bool {
    let socket = UdpSocket::bind("127.0.0.1:0").unwrap();
    socket.connect(("127.0.0.1", port)).unwrap();
    socket
        .set_read_timeout(Some(Duration::from_millis(200)))
        .unwrap();
    socket.send(&StatusRequest { nonce: 1 }.encode()).unwrap();
    let mut buffer = [0; 64];
    socket
        .recv(&mut buffer)
        .ok()
        .and_then(|len| StatusReply::decode(&buffer[..len]))
        .is_some()
}

/// Runs the server in `folder` with `options`, until its status answers, and returns what the
/// status says of its places, after stopping it with SIGTERM.
fn run_server(folder: &Path, options: &[&str]) -> u32 {
    let (game, status) = server::acquire_local_socket_pair("127.0.0.1".parse().unwrap()).unwrap();
    let port = game.local_addr().unwrap().port();
    let status_port = status.local_addr().unwrap().port();
    drop((game, status));

    let child = Command::new(env!("CARGO_BIN_EXE_server"))
        .args(["--port", &port.to_string(), "--world", WORLD_NAME])
        .arg("--game-folder-path")
        .arg(folder)
        .args(options)
        .spawn()
        .unwrap();
    let mut child = KillOnDrop(child);

    let start = Instant::now();
    while !answers_status(status_port) {
        assert!(start.elapsed() < TIMEOUT, "the server did not start");
        assert!(child.0.try_wait().unwrap().is_none(), "the server exited");
    }
    let max_players = status_max_players(status_port);

    assert_eq!(unsafe { libc::kill(child.0.id() as i32, libc::SIGTERM) }, 0);
    let start = Instant::now();
    while child.0.try_wait().unwrap().is_none() {
        assert!(start.elapsed() < TIMEOUT, "the server did not exit");
        std::thread::sleep(Duration::from_millis(20));
    }
    max_players
}

fn status_max_players(port: u16) -> u32 {
    let socket = UdpSocket::bind("127.0.0.1:0").unwrap();
    socket.connect(("127.0.0.1", port)).unwrap();
    socket.set_read_timeout(Some(TIMEOUT)).unwrap();
    socket.send(&StatusRequest { nonce: 1 }.encode()).unwrap();
    let mut buffer = [0; 64];
    let len = socket.recv(&mut buffer).unwrap();
    StatusReply::decode(&buffer[..len]).unwrap().max_players
}

fn world_file(folder: &Path) -> String {
    std::fs::read_to_string(folder.join("saves").join(WORLD_NAME).join("world.ron")).unwrap()
}

#[test]
fn a_new_world_gets_the_options_of_the_command_line() {
    let folder = tempfile::tempdir().unwrap();

    let max_players = run_server(
        folder.path(),
        &[
            "--seed",
            "2024",
            "--world-type",
            "single-biome",
            "--biome",
            "plains",
            "--game-mode",
            "creative",
            "--difficulty",
            "hard",
            "--max-players",
            "7",
        ],
    );

    let world = world_file(folder.path());
    assert!(world.contains("seed: (2024)"), "{world}");
    assert!(world.contains("SingleBiome(\"plains\")"), "{world}");
    assert!(world.contains("game_mode: Creative"), "{world}");
    assert!(world.contains("difficulty: Hard"), "{world}");
    assert_eq!(max_players, 7);
}

#[test]
fn a_world_that_exists_keeps_its_own_options() {
    let folder = tempfile::tempdir().unwrap();
    run_server(folder.path(), &["--seed", "11", "--difficulty", "easy"]);

    run_server(
        folder.path(),
        &[
            "--seed",
            "22",
            "--game-mode",
            "creative",
            "--difficulty",
            "hard",
        ],
    );

    let after = world_file(folder.path());
    assert!(after.contains("seed: (11)"), "{after}");
    assert!(after.contains("difficulty: Easy"), "{after}");
    assert!(after.contains("game_mode: Survival"), "{after}");
}
