//! A session, from the Solo or Multiplayer screen to the game and back: where it was started
//! from, the step it is at, and why it ended, as plain functions the systems call.

use std::net::SocketAddr;
use std::time::Duration;

use bevy::prelude::*;
use bevy_renet::netcode::NetcodeDisconnectReason;
use shared::messages::sanitize_username;
use shared::world::ChunkMap;
use shared::BROADCAST_CHUNK_RADIUS;

/// Where the session being opened or played was started from.
#[allow(dead_code)] // start_world and join_server, Task 2
#[derive(Resource, Clone, Debug, PartialEq, Eq)]
pub enum SessionOrigin {
    World {
        name: String,
    },
    /// `ip` as typed in the server list; `address` what the join resolved it to.
    Server {
        name: String,
        ip: String,
        address: SocketAddr,
    },
}

impl SessionOrigin {
    /// The name of the world or of the server, as its list shows it.
    #[allow(dead_code)] // The connecting card, Task 2
    pub fn name(&self) -> &str {
        match self {
            SessionOrigin::World { name } | SessionOrigin::Server { name, .. } => name,
        }
    }

    /// Whether the session opens a world of this computer rather than joins a server.
    pub fn is_world(&self) -> bool {
        matches!(self, SessionOrigin::World { .. })
    }
}

/// How a session ended other than by the pause menu; read by the menus when they open.
/// `error: None` is a cancel.
#[allow(dead_code)] // end_session, Task 2
#[derive(Resource, Clone, Debug, PartialEq, Eq)]
pub struct SessionEnd {
    pub origin: SessionOrigin,
    pub error: Option<ConnectError>,
}

/// The step a session is at, from its start to the game.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConnectStep {
    /// The local server of a world loads it, then accepts the client.
    StartingWorld,
    /// The client connects to a server and authenticates.
    Connecting,
    /// `received` of the `total` chunks around the spawn are in.
    LoadingChunks { received: u32, total: u32 },
    /// The textures load and the local player spawns.
    Spawning,
}

/// Why a session failed, as the error card tells it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConnectError {
    /// The server never answered.
    NoAnswer,
    /// The server answers the status of another protocol.
    #[allow(dead_code)] // The status asked after no answer, Task 4
    Incompatible,
    /// The server has no room for another player.
    Full,
    /// The server dropped the connection before the authentication: its `name` may be in use.
    Refused { name: String },
    /// The player's `name` is not one a server accepts.
    InvalidName { name: String },
    /// The connection dropped after the authentication, before the game.
    LostWhileLoading,
    /// The local server could not load the world, for this reason.
    #[allow(dead_code)] // When a world cannot start, Task 5
    WorldFailed(String),
    /// The local server of the last session still runs.
    #[allow(dead_code)] // When a world cannot start, Task 5
    WorldBusy,
    /// No socket could be opened, for this reason.
    #[allow(dead_code)] // When a world cannot start, Task 5
    NoConnection(String),
    /// The server disconnected the player in game.
    ServerStopped,
    /// The connection dropped in game.
    ConnectionLost,
}

/// How far a session got before its connection dropped.
#[allow(dead_code)] // watch_connection, Task 4
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reached {
    /// Not authenticated yet.
    Connecting,
    /// Authenticated, not in the game yet.
    Authenticated,
    /// In the game.
    InGame,
}

/// How long a connected session may stay at `Connecting` or `LoadingChunks` without a new step
/// before it gives up.
#[allow(dead_code)] // watch_connection, Task 4
pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// What a dropped connection means (Decision 5); `name` is the player's.
///
/// Before the authentication, a server denies a client when it is full and drops one whose
/// name is in use or invalid; anything else is no answer. After it, the connection is lost
/// while loading. In the game, a dedicated server only disconnects a player when it shuts down.
#[allow(dead_code)] // watch_connection, Task 4
pub fn failure_of(
    reached: Reached,
    reason: Option<NetcodeDisconnectReason>,
    name: &str,
) -> ConnectError {
    match (reached, reason) {
        (Reached::Connecting, Some(NetcodeDisconnectReason::ConnectionDenied)) => {
            ConnectError::Full
        }
        (Reached::Connecting, Some(NetcodeDisconnectReason::DisconnectedByServer)) => {
            match sanitize_username(name) {
                Some(accepted) => ConnectError::Refused {
                    name: accepted.to_string(),
                },
                None => ConnectError::InvalidName {
                    name: name.to_string(),
                },
            }
        }
        (Reached::Connecting, _) => ConnectError::NoAnswer,
        (Reached::Authenticated, _) => ConnectError::LostWhileLoading,
        (Reached::InGame, Some(NetcodeDisconnectReason::DisconnectedByServer)) => {
            ConnectError::ServerStopped
        }
        (Reached::InGame, _) => ConnectError::ConnectionLost,
    }
}

/// The step of a session; `chunks` is `None` while the local player has not spawned.
#[allow(dead_code)] // track_connection, Task 2
pub fn connect_step(
    origin: &SessionOrigin,
    authenticated: bool,
    chunks: Option<(u32, u32)>,
) -> ConnectStep {
    if !authenticated {
        return if origin.is_world() {
            ConnectStep::StartingWorld
        } else {
            ConnectStep::Connecting
        };
    }
    let side = (2 * BROADCAST_CHUNK_RADIUS + 1) as u32;
    match chunks.unwrap_or((0, side.pow(3))) {
        (received, total) if received < total => ConnectStep::LoadingChunks { received, total },
        _ => ConnectStep::Spawning,
    }
}

/// How far the progress bar is filled at `step` (Decision 3), from 0 to 1.
#[allow(dead_code)] // The connecting card, Task 2
pub fn progress(step: ConnectStep) -> f32 {
    match step {
        ConnectStep::StartingWorld | ConnectStep::Connecting => 0.1,
        ConnectStep::LoadingChunks { received, total } => {
            let loaded = if total == 0 {
                1.0
            } else {
                received.min(total) as f32 / total as f32
            };
            0.2 + 0.7 * loaded
        }
        ConnectStep::Spawning => 0.95,
    }
}

/// How many of the chunks within `radius` of the chunk `center` `map` holds, and how many
/// there are.
#[allow(dead_code)] // Chunks before the game, Task 3
pub fn chunks_around(map: &ChunkMap, center: IVec3, radius: i32) -> (u32, u32) {
    let range = -radius..=radius;
    let mut received = 0;
    let mut total = 0;
    for x in range.clone() {
        for y in range.clone() {
            for z in range.clone() {
                total += 1;
                if map.is_chunk_loaded(center + IVec3::new(x, y, z)) {
                    received += 1;
                }
            }
        }
    }
    (received, total)
}

/// Whether a dropped connection is opened again: a world still starting whose server runs.
#[allow(dead_code)] // When a world cannot start, Task 5
pub fn reconnect_to_world(
    origin: &SessionOrigin,
    step: ConnectStep,
    local_server_running: bool,
) -> bool {
    origin.is_world() && step == ConnectStep::StartingWorld && local_server_running
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_renet::netcode::NetcodeDisconnectReason as Reason;
    use shared::world::Chunk;

    fn world() -> SessionOrigin {
        SessionOrigin::World {
            name: "Survie".into(),
        }
    }

    fn server() -> SessionOrigin {
        SessionOrigin::Server {
            name: "Friends".into(),
            ip: "127.0.0.1:8000".into(),
            address: "127.0.0.1:8000".parse().unwrap(),
        }
    }

    #[test]
    fn a_dropped_connection_reads_as_what_the_player_can_do() {
        for reason in [
            Some(Reason::ConnectionRequestTimedOut),
            Some(Reason::ConnectionResponseTimedOut),
            Some(Reason::ConnectTokenExpired),
            None,
        ] {
            assert_eq!(
                failure_of(Reached::Connecting, reason, "Tester"),
                ConnectError::NoAnswer,
                "{reason:?}"
            );
        }
        assert_eq!(
            failure_of(
                Reached::Connecting,
                Some(Reason::ConnectionDenied),
                "Tester"
            ),
            ConnectError::Full
        );
        assert_eq!(
            failure_of(
                Reached::Connecting,
                Some(Reason::DisconnectedByServer),
                "Tester"
            ),
            ConnectError::Refused {
                name: "Tester".into()
            }
        );
        assert_eq!(
            failure_of(Reached::Connecting, Some(Reason::DisconnectedByServer), ""),
            ConnectError::InvalidName { name: "".into() }
        );
        for reason in [
            Some(Reason::DisconnectedByServer),
            Some(Reason::ConnectionTimedOut),
            None,
        ] {
            assert_eq!(
                failure_of(Reached::Authenticated, reason, "Tester"),
                ConnectError::LostWhileLoading,
                "{reason:?}"
            );
        }
        assert_eq!(
            failure_of(
                Reached::InGame,
                Some(Reason::DisconnectedByServer),
                "Tester"
            ),
            ConnectError::ServerStopped
        );
        for reason in [Some(Reason::ConnectionTimedOut), None] {
            assert_eq!(
                failure_of(Reached::InGame, reason, "Tester"),
                ConnectError::ConnectionLost,
                "{reason:?}"
            );
        }
    }

    #[test]
    fn the_steps_follow_the_session() {
        assert_eq!(
            connect_step(&world(), false, None),
            ConnectStep::StartingWorld
        );
        assert_eq!(
            connect_step(&server(), false, None),
            ConnectStep::Connecting
        );
        assert_eq!(
            connect_step(&world(), true, None),
            ConnectStep::LoadingChunks {
                received: 0,
                total: 27
            }
        );
        assert_eq!(
            connect_step(&server(), true, Some((12, 27))),
            ConnectStep::LoadingChunks {
                received: 12,
                total: 27
            }
        );
        assert_eq!(
            connect_step(&server(), true, Some((27, 27))),
            ConnectStep::Spawning
        );
    }

    #[test]
    fn progress_only_moves_forward() {
        let loading = |received| ConnectStep::LoadingChunks {
            received,
            total: 27,
        };
        let steps = [
            ConnectStep::Connecting,
            loading(0),
            loading(13),
            loading(27),
            ConnectStep::Spawning,
        ];
        let values = steps.map(progress);
        assert_eq!(values[0], 0.1);
        assert_eq!(values[1], 0.2);
        assert!((values[3] - 0.9).abs() < 1e-6);
        assert_eq!(values[4], 0.95);
        assert!(
            values.windows(2).all(|pair| pair[0] < pair[1]),
            "{values:?}"
        );
        assert!(values[4] <= 1.0);
        assert_eq!(progress(ConnectStep::StartingWorld), 0.1);
    }

    #[test]
    fn the_chunks_around_the_spawn_are_counted() {
        let mut map = ChunkMap::default();
        for pos in [IVec3::ZERO, IVec3::new(1, 0, -1), IVec3::new(5, 5, 5)] {
            map.insert_chunk(pos, Chunk::new());
        }
        assert_eq!(chunks_around(&map, IVec3::ZERO, 1), (2, 27));
    }

    #[test]
    fn a_world_still_starting_is_reconnected() {
        let loading = ConnectStep::LoadingChunks {
            received: 3,
            total: 27,
        };
        assert!(reconnect_to_world(
            &world(),
            ConnectStep::StartingWorld,
            true
        ));
        assert!(!reconnect_to_world(
            &world(),
            ConnectStep::StartingWorld,
            false
        ));
        assert!(!reconnect_to_world(&world(), loading, true));
        assert!(!reconnect_to_world(
            &server(),
            ConnectStep::Connecting,
            true
        ));
    }
}
