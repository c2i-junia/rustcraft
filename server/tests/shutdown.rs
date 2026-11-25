//! Runs the built `server` binary and stops it with SIGTERM, as `docker stop` does.

#![cfg(unix)]

use shared::status::{StatusReply, StatusRequest};
use std::net::UdpSocket;
use std::process::{Child, Command};
use std::time::{Duration, Instant};

const TIMEOUT: Duration = Duration::from_secs(10);
const WORLD_NAME: &str = "shutdown-test";

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

#[test]
fn a_dedicated_server_saves_the_world_and_exits_on_sigterm() {
    let folder = tempfile::tempdir().unwrap();
    let world_dir = folder.path().join("saves").join(WORLD_NAME);
    std::fs::create_dir_all(&world_dir).unwrap();
    let world_file = world_dir.join("world.ron");
    std::fs::write(
        &world_file,
        format!(r#"(format_version: 2, name: "{WORLD_NAME}", seed: (147), time: 0, mobs: {{}})"#),
    )
    .unwrap();
    let before = std::fs::read(&world_file).unwrap();

    let (game, status) = server::acquire_local_socket_pair("127.0.0.1".parse().unwrap()).unwrap();
    let port = game.local_addr().unwrap().port();
    let status_port = status.local_addr().unwrap().port();
    drop((game, status));

    let child = Command::new(env!("CARGO_BIN_EXE_server"))
        .args(["--port", &port.to_string(), "--world", WORLD_NAME])
        .arg("--game-folder-path")
        .arg(folder.path())
        .spawn()
        .unwrap();
    let mut child = KillOnDrop(child);

    let start = Instant::now();
    while !answers_status(status_port) {
        assert!(start.elapsed() < TIMEOUT, "the server did not start");
        assert!(child.0.try_wait().unwrap().is_none(), "the server exited");
    }

    // Well within the 60 s of the autosave: only the signal can have the world saved
    assert_eq!(std::fs::read(&world_file).unwrap(), before);
    assert_eq!(unsafe { libc::kill(child.0.id() as i32, libc::SIGTERM) }, 0);

    let start = Instant::now();
    let status = loop {
        if let Some(status) = child.0.try_wait().unwrap() {
            break status;
        }
        assert!(
            start.elapsed() < TIMEOUT,
            "the server did not exit on SIGTERM"
        );
        std::thread::sleep(Duration::from_millis(20));
    };
    assert!(status.success(), "the server exited with {status}");
    assert_ne!(
        std::fs::read(&world_file).unwrap(),
        before,
        "the world was not saved"
    );
}
