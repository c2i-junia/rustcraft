//! The agent against the local Docker, with a stand-in for the game server's image that writes
//! its command line to its volume and exits on SIGTERM. Ignored by default:
//!   cargo test -p rustcraft-realm-agent --test docker -- --ignored
//! It creates only containers, volumes and an image named after it or its random world, and
//! removes them when it ends.

use std::io::Write;
use std::process::{Command, Stdio};
use std::sync::Arc;

use rustcraft_agent_protocol::{RealmState, Request, Response};
use rustcraft_protocol::api::RealmSettings;
use rustcraft_realm_agent::{agent::Agent, config::Config, socket};
use serde_json::json;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;
use uuid::Uuid;

const IMAGE: &str = "rustcraft-realm-agent-stand-in:test";
const STAND_IN: &str = r#"
FROM alpine:latest
RUN adduser -D -H -u 10001 rustcraft && mkdir /data && chown rustcraft /data
USER rustcraft
ENTRYPOINT ["sh", "-c", "echo \"$RUSTCRAFT_REALM_TOKEN $*\" > /data/args; trap 'exit 0' TERM; while true; do sleep 1 & wait $!; done", "stand-in"]
"#;
const TOKEN: &str = "c3RhbmQtaW4tdG9rZW4tb2YtdGhlLXRlc3Q";
const BASE_PORT: u16 = 47310;

fn docker(args: &[&str]) -> String {
    let output = Command::new("docker").args(args).output().unwrap();
    assert!(output.status.success(), "docker {args:?}: {output:?}");
    String::from_utf8(output.stdout).unwrap()
}

fn volumes_of(realm: Uuid) -> Vec<String> {
    let mut volumes: Vec<String> = docker(&[
        "volume",
        "ls",
        "--quiet",
        "--filter",
        &format!("name={realm}"),
    ])
    .lines()
    .map(String::from)
    .collect();
    volumes.sort();
    volumes
}

/// Removes what the test made, even when it fails.
struct Cleanup(Uuid);

impl Drop for Cleanup {
    fn drop(&mut self) {
        let containers = docker(&[
            "ps",
            "--all",
            "--quiet",
            "--filter",
            &format!("name={}", self.0),
        ]);
        for container in containers.lines() {
            docker(&["rm", "--force", container]);
        }
        for volume in volumes_of(self.0) {
            docker(&["volume", "rm", &volume]);
        }
        let _ = Command::new("docker").args(["image", "rm", IMAGE]).output();
    }
}

struct Client(BufReader<UnixStream>);

impl Client {
    async fn send(&mut self, request: serde_json::Value) -> Response {
        let mut line = request.to_string();
        line.push('\n');
        self.0.get_mut().write_all(line.as_bytes()).await.unwrap();
        let mut answer = String::new();
        self.0.read_line(&mut answer).await.unwrap();
        serde_json::from_str(&answer).unwrap()
    }

    async fn request(&mut self, request: Request) -> Response {
        self.send(serde_json::to_value(request).unwrap()).await
    }
}

#[tokio::test]
#[ignore = "needs Docker"]
async fn a_world_starts_stops_and_is_archived_then_purged() {
    let realm = Uuid::new_v4();
    let other = Uuid::new_v4();
    let _cleanup = (Cleanup(realm), Cleanup(other));
    let mut build = Command::new("docker")
        .args(["build", "--quiet", "--tag", IMAGE, "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .spawn()
        .unwrap();
    build
        .stdin
        .take()
        .unwrap()
        .write_all(STAND_IN.as_bytes())
        .unwrap();
    assert!(build.wait().unwrap().success());

    let folder = std::env::temp_dir().join(format!("realm-agent-{realm}"));
    std::fs::create_dir_all(&folder).unwrap();
    let path = folder.join("agent.sock");
    let config: Config = toml::from_str(&format!(
        r#"
        socket = "{}"
        image = "{IMAGE}"
        hub_url = "https://hub.example"
        public_host = "play.example"
        base_port = {BASE_PORT}
        slots = 2
        stop_grace_seconds = 5
        "#,
        path.display()
    ))
    .unwrap();
    let listener = socket::bind(&path).unwrap();
    let mode =
        std::os::unix::fs::PermissionsExt::mode(&std::fs::metadata(&path).unwrap().permissions());
    assert_eq!(mode & 0o777, 0o660);
    let docker_api = bollard::Docker::connect_with_local_defaults().unwrap();
    tokio::spawn(socket::serve(
        listener,
        Arc::new(Agent::new(config, docker_api)),
    ));
    let mut client = Client(BufReader::new(UnixStream::connect(&path).await.unwrap()));

    let status = Request::Status { realm };
    assert_eq!(
        client.request(status.clone()).await,
        Response::Status {
            state: RealmState::Absent
        }
    );
    let start = |slot| Request::Start {
        realm,
        slot,
        token: TOKEN.into(),
        settings: RealmSettings {
            seed: Some(42),
            difficulty: Some("hard".into()),
            ..Default::default()
        },
    };
    assert!(matches!(
        client.request(start(2)).await,
        Response::Refused { .. }
    ));
    assert!(matches!(
        client
            .send(json!({"request": "start", "realm": "nonsense"}))
            .await,
        Response::Refused { .. }
    ));
    assert_eq!(client.request(start(1)).await, Response::Done);
    assert_eq!(
        client.request(status.clone()).await,
        Response::Status {
            state: RealmState::Running
        }
    );
    assert!(matches!(
        client.request(start(1)).await,
        Response::Refused { .. }
    ));
    let inspected = docker(&[
        "inspect",
        "--format",
        "{{.HostConfig.ReadonlyRootfs}} {{.HostConfig.CapDrop}} {{.HostConfig.SecurityOpt}} {{.Config.User}} {{.HostConfig.Memory}} {{.HostConfig.PidsLimit}}",
        &format!("rustcraft-realm-{realm}"),
    ]);
    assert_eq!(
        inspected.trim(),
        "true [ALL] [no-new-privileges] 10001 1610612736 256"
    );
    let ports = docker(&["port", &format!("rustcraft-realm-{realm}")]);
    for expected in [
        "8000/udp -> 0.0.0.0:47312",
        "8000/udp -> [::]:47312",
        "8001/udp -> 0.0.0.0:47313",
        "8001/udp -> [::]:47313",
    ] {
        assert!(ports.contains(expected), "{ports}");
    }
    // Another world on the same slot cannot take its ports, and leaves nothing behind
    let taken = Request::Start {
        realm: other,
        slot: 1,
        token: TOKEN.into(),
        settings: RealmSettings::default(),
    };
    assert!(matches!(
        client.request(taken).await,
        Response::Failed { .. }
    ));
    assert!(volumes_of(other).is_empty());
    assert_eq!(
        client.request(Request::Status { realm: other }).await,
        Response::Status {
            state: RealmState::Absent
        }
    );

    // The stand-in leaves at once on SIGTERM: far within the grace
    let stopping = std::time::Instant::now();
    assert_eq!(
        client.request(Request::Stop { realm }).await,
        Response::Done
    );
    assert!(stopping.elapsed() < std::time::Duration::from_secs(4));
    assert_eq!(
        client.request(status.clone()).await,
        Response::Status {
            state: RealmState::Absent
        }
    );
    assert_eq!(volumes_of(realm), [format!("rustcraft-realm-{realm}")]);

    // An interrupted archive left its copy container and a partial archive, long ago
    let partial = format!("rustcraft-realm-{realm}-archived-20200102030405");
    docker(&["volume", "create", &partial]);
    docker(&[
        "create",
        "--name",
        &format!("rustcraft-realm-{realm}-archiving"),
        "--volume",
        &format!("{partial}:/data"),
        "--entrypoint",
        "touch",
        IMAGE,
        "/data/partial",
    ]);
    let in_volume = |volume: &str, command: &str, path: &str| {
        docker(&[
            "run",
            "--rm",
            "--network",
            "none",
            "--volume",
            &format!("{volume}:/data:ro"),
            "--entrypoint",
            command,
            IMAGE,
            path,
        ])
    };
    let archives = || {
        let volumes = volumes_of(realm);
        assert!(
            volumes.iter().all(|volume| volume.contains("-archived-")),
            "{volumes:?}"
        );
        volumes
    };

    // Archived while it runs: stopped first; the partial archive is left alone
    assert_eq!(client.request(start(0)).await, Response::Done);
    assert_eq!(
        client.request(Request::Archive { realm }).await,
        Response::Done
    );
    let first = archives();
    assert_eq!(first.len(), 2);
    assert_eq!(first[0], partial);
    let archive = first[1].clone();
    assert_eq!(in_volume(&archive, "ls", "/data").trim(), "args");
    assert!(docker(&[
        "ps",
        "--all",
        "--quiet",
        "--filter",
        &format!("name={realm}-archiving")
    ])
    .trim()
    .is_empty());
    assert_eq!(
        in_volume(&archive, "cat", "/data/args").trim(),
        format!(
            "{TOKEN} --world world --port 8000 --auth hub --hub https://hub.example \
             --public-address play.example:47310 --realm {realm} --max-render-distance 12 \
             --max-players 10 --seed 42 --difficulty hard"
        )
    );
    assert_eq!(
        client.request(Request::Archive { realm }).await,
        Response::Done
    );
    assert_eq!(archives(), first);

    // Started again and archived again the same day: both archives stay
    tokio::time::sleep(std::time::Duration::from_millis(1100)).await;
    assert_eq!(client.request(start(0)).await, Response::Done);
    assert_eq!(
        client.request(Request::Archive { realm }).await,
        Response::Done
    );
    let second = archives();
    assert_eq!(second.len(), 3);
    assert_eq!(second[..2], first[..]);
    assert_eq!(in_volume(&archive, "ls", "/data").trim(), "args");

    // The archives of 2020, the partial one among them, are purged; today's are kept
    let old = format!("rustcraft-realm-{realm}-archived-20200101000000");
    docker(&["volume", "create", &old]);
    assert_eq!(
        client.request(Request::Purge).await,
        Response::Purged {
            volumes: vec![old, partial]
        }
    );
    assert_eq!(archives(), second[1..]);
    std::fs::remove_dir_all(&folder).unwrap();
}
