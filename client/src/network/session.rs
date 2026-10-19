//! A session, from the Solo or Multiplayer screen to the game and back: where it was started
//! from, the step it is at, and why it ended, as plain functions the systems call.

use std::net::SocketAddr;
use std::time::{Duration, Instant};

use bevy::prelude::*;
use bevy_renet::netcode::{NetcodeClientTransport, NetcodeDisconnectReason};
use bevy_renet::RenetClient;
use shared::messages::sanitize_username;
use shared::world::{world_position_to_chunk_position, ChunkMap};
use shared::BROADCAST_CHUNK_RADIUS;

use super::status::{InFlight, ServerStatus, StatusProbe};
use super::{
    authenticated, establish_authenticated_connection_to_server, open_connection,
    CurrentPlayerProfile, LocalServerHandle, TargetServer, TargetServerState,
};
use crate::player::CurrentPlayerMarker;
use crate::state::GameState;
use crate::world::ClientWorldMap;

/// Where the session being opened or played was started from.
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
    Incompatible,
    /// The server has no room for another player.
    Full,
    /// The server dropped the connection before the authentication: its `name` may be in use.
    /// `name` is the player's name as sent, once `sanitize_username` accepted it.
    Refused { name: String },
    /// The player's `name` is not one a server accepts. `name` is the player's name as given,
    /// which `sanitize_username` refuses.
    InvalidName { name: String },
    /// The connection dropped after the authentication, before the game.
    LostWhileLoading,
    /// The local server could not load the world, for this reason.
    WorldFailed(String),
    /// The local server of the last session still runs.
    WorldBusy,
    /// No socket could be opened, for this reason.
    NoConnection(String),
    /// The server disconnected the player in game.
    ServerStopped,
    /// The connection dropped in game.
    ConnectionLost,
}

/// The session being opened: its step and when it last changed. Exists in `PreGameLoading` only.
#[derive(Resource)]
pub struct ConnectWatch {
    pub step: ConnectStep,
    since: Instant,
    /// The status asked once a server gave no answer: whether it runs another version.
    verdict: Option<InFlight<ServerStatus>>,
}

/// How long a session may stay at `Connecting` or `LoadingChunks` without a new step.
#[derive(Resource)]
pub struct ConnectTimeout(pub Duration);

impl Default for ConnectTimeout {
    fn default() -> Self {
        Self(CONNECT_TIMEOUT)
    }
}

#[cfg(test)]
impl ConnectWatch {
    /// A watch at `step` since now.
    pub(crate) fn at(step: ConnectStep) -> Self {
        Self {
            step,
            since: Instant::now(),
            verdict: None,
        }
    }
}

/// Watches the session being opened and tells its step, then the session in the game.
pub fn session_plugin(app: &mut App) {
    app.init_resource::<ConnectTimeout>()
        .add_systems(OnEnter(GameState::PreGameLoading), start_watching)
        .add_systems(OnExit(GameState::PreGameLoading), stop_watching)
        // a session in the game has not ended: no menu may read an end left from before
        .add_systems(OnEnter(GameState::Game), |mut commands: Commands| {
            commands.remove_resource::<SessionEnd>();
        })
        .add_systems(
            Update,
            // the frame the server accepts the player shows `Loading chunks`, even when every
            // chunk comes in the next one
            (
                track_connection.after(establish_authenticated_connection_to_server),
                watch_connection.after(track_connection),
            )
                .run_if(in_state(GameState::PreGameLoading)),
        )
        .add_systems(Update, watch_session.run_if(in_state(GameState::Game)));
}

/// Ends the session as `error` (`None`: cancelled) unless it already ended this frame: inserts
/// `SessionEnd` and sets `GameState::Menu`. Returns whether it did.
pub fn end_session(
    commands: &mut Commands,
    game: &mut NextState<GameState>,
    origin: &SessionOrigin,
    error: Option<ConnectError>,
) -> bool {
    // whichever end comes first in the frame wins, in whatever order the systems run
    if !matches!(game, NextState::Unchanged) {
        return false;
    }
    commands.insert_resource(SessionEnd {
        origin: origin.clone(),
        error,
    });
    game.set(GameState::Menu);
    true
}

/// Starts watching the session at its first step.
fn start_watching(origin: Option<Res<SessionOrigin>>, mut commands: Commands) {
    let Some(origin) = origin else {
        return;
    };
    commands.insert_resource(ConnectWatch {
        step: connect_step(&origin, false, None),
        since: Instant::now(),
        verdict: None,
    });
}

fn stop_watching(mut commands: Commands) {
    commands.remove_resource::<ConnectWatch>();
}

/// Moves the watch to the step the session is at, and restarts its clock when it changed. The
/// chunks are counted around the local player once it has spawned.
pub(crate) fn track_connection(
    watch: Option<ResMut<ConnectWatch>>,
    origin: Option<Res<SessionOrigin>>,
    target: Res<TargetServer>,
    world: Res<ClientWorldMap>,
    player: Query<&Transform, With<CurrentPlayerMarker>>,
) {
    let (Some(mut watch), Some(origin)) = (watch, origin) else {
        return;
    };
    let chunks = player.single().ok().map(|player| {
        let center = world_position_to_chunk_position(player.translation);
        chunks_around(&world.blocks, center, BROADCAST_CHUNK_RADIUS)
    });
    let step = connect_step(&origin, authenticated(target), chunks);
    // only a new step marks the watch changed: the card redraws on a change
    if watch.step != step {
        watch.step = step;
        watch.since = Instant::now();
    }
}

/// Ends a session that failed: the local server of a world stopped (`WorldFailed`), the
/// transport dropped (`failure_of`), `CONNECT_TIMEOUT` without a new step while `Connecting` or
/// `LoadingChunks`, then for a server's `NoAnswer` the status verdict: a server of another
/// version says so. Stands down while the session already ends this frame, a cancel: the first
/// end of a session wins.
///
/// A world still starting has no timeout: its server thread is the watchdog. When its
/// connection gives up first (netcode's connect token, 15 s), a new one is opened.
///
/// The timeout counts from the last new step, not from the start: on a busy server the chunks
/// around the spawn may come after those of the other players, and each one that comes is a
/// new step.
#[allow(clippy::too_many_arguments)]
fn watch_connection(
    watch: Option<ResMut<ConnectWatch>>,
    origin: Option<Res<SessionOrigin>>,
    client: Option<Res<RenetClient>>,
    transport: Option<Res<NetcodeClientTransport>>,
    local_server: Option<ResMut<LocalServerHandle>>,
    mut target: ResMut<TargetServer>,
    profile: Res<CurrentPlayerProfile>,
    timeout: Res<ConnectTimeout>,
    probe: Res<StatusProbe>,
    mut game: ResMut<NextState<GameState>>,
    mut commands: Commands,
) {
    let (Some(mut watch), Some(origin)) = (watch, origin) else {
        return;
    };
    if !matches!(*game, NextState::Unchanged) {
        return;
    }
    // the handle is that of this session's world: no other one starts while it is held
    let mut local_server = local_server.filter(|_| origin.is_world());
    if let Some(error) = local_server.as_deref_mut().and_then(local_server_stopped) {
        end_session(&mut commands, &mut game, &origin, Some(error));
        return;
    }
    if let Some(verdict) = &watch.verdict {
        if let Some(status) = verdict.take() {
            let error = if status == ServerStatus::Incompatible {
                ConnectError::Incompatible
            } else {
                ConnectError::NoAnswer
            };
            end_session(&mut commands, &mut game, &origin, Some(error));
        }
        return;
    }
    let dropped = client.is_some_and(|client| client.is_disconnected());
    let stalled = matches!(
        watch.step,
        ConnectStep::Connecting | ConnectStep::LoadingChunks { .. }
    ) && watch.since.elapsed() >= timeout.0;
    if !dropped && !stalled {
        return;
    }
    let running = local_server.is_some_and(|local_server| local_server.0.is_some());
    if dropped && reconnect_to_world(&origin, watch.step, running) {
        if let Some(addr) = target.address {
            match open_connection(addr, profile.id) {
                Ok((client, transport)) => {
                    info!("The world is still starting, connecting to it again");
                    commands.insert_resource(client);
                    commands.insert_resource(transport);
                    // the request queued on the connection that gave up is lost: ask again
                    target.state = TargetServerState::Initial;
                    watch.since = Instant::now();
                }
                Err(reason) => {
                    let error = Some(ConnectError::NoConnection(reason));
                    end_session(&mut commands, &mut game, &origin, error);
                }
            }
            return;
        }
    }
    let reached = if authenticated(target.into()) {
        Reached::Authenticated
    } else {
        Reached::Connecting
    };
    let reason = transport
        .filter(|_| dropped)
        .and_then(|transport| transport.disconnect_reason());
    let error = failure_of(reached, reason, &profile.name);
    if let (ConnectError::NoAnswer, SessionOrigin::Server { ip, .. }) = (&error, &*origin) {
        info!("No answer from {ip}, asking its status");
        let (ip, probe) = (ip.clone(), probe.0);
        // not a new step: the card has nothing to redraw
        watch.bypass_change_detection().verdict =
            Some(InFlight::spawn(ServerStatus::Offline, move || probe(&ip)));
        return;
    }
    end_session(&mut commands, &mut game, &origin, Some(error));
}

/// Ends a session whose connection dropped in the game (`failure_of`): the server stopped or the
/// connection was lost. Stands down while the game is already left this frame: leaving on
/// purpose, from the pause menu, is never an error.
fn watch_session(
    client: Option<Res<RenetClient>>,
    transport: Option<Res<NetcodeClientTransport>>,
    origin: Option<Res<SessionOrigin>>,
    profile: Res<CurrentPlayerProfile>,
    mut game: ResMut<NextState<GameState>>,
    mut commands: Commands,
) {
    let Some(origin) = origin else {
        return;
    };
    if !client.is_some_and(|client| client.is_disconnected()) {
        return;
    }
    let reason = transport.and_then(|transport| transport.disconnect_reason());
    let error = failure_of(Reached::InGame, reason, &profile.name);
    if end_session(&mut commands, &mut game, &origin, Some(error.clone())) {
        warn!("The connection dropped in the game ({reason:?}): {error:?}");
    }
}

/// The error of the local server of a world whose thread finished, which it joins: the world
/// is no longer opening. `None` while it runs.
fn local_server_stopped(local_server: &mut LocalServerHandle) -> Option<ConnectError> {
    if !local_server.0.as_ref()?.is_finished() {
        return None;
    }
    let reason = match local_server.0.take()?.join() {
        Ok(Err(reason)) => reason,
        Ok(Ok(())) | Err(_) => "The world stopped while opening".to_string(),
    };
    warn!("The local server stopped while the world was opening: {reason}");
    Some(ConnectError::WorldFailed(reason))
}

/// Whether servers accept the player's name: `sanitize_username` keeps it.
pub fn player_name_accepted(profile: Res<CurrentPlayerProfile>) -> bool {
    sanitize_username(&profile.name).is_some()
}

/// Ends the session being opened when no server would accept the player's name, before a local
/// server starts or a packet leaves.
pub(crate) fn refuse_an_invalid_name(
    profile: Res<CurrentPlayerProfile>,
    origin: Option<Res<SessionOrigin>>,
    mut game: ResMut<NextState<GameState>>,
    mut commands: Commands,
) {
    let Some(origin) = origin else {
        return;
    };
    if sanitize_username(&profile.name).is_some() {
        return;
    }
    let error = ConnectError::InvalidName {
        name: profile.name.clone(),
    };
    end_session(&mut commands, &mut game, &origin, Some(error));
}

/// How far a session got before its connection dropped.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reached {
    /// Not authenticated yet.
    Connecting,
    /// Authenticated, not in the game yet.
    Authenticated,
    /// In the game.
    InGame,
}

/// How long a session may stay at `Connecting` or `LoadingChunks` without a new step before it
/// gives up. Netcode itself gives up on a silent server only after 15 s (the `timeout_seconds` of
/// renetcode's unsecure connect token).
pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// What a dropped connection means (Decision 5); `name` is the player's.
///
/// Before the authentication, a server denies a client when it is full and drops one whose
/// name is in use or invalid; anything else is no answer. After it, the connection is lost
/// while loading. In the game, a dedicated server only disconnects a player when it shuts down.
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
        // the client's own disconnect is a cancel, which ends the session before its connection
        // is watched: it never reaches the card
        (Reached::Connecting, Some(NetcodeDisconnectReason::DisconnectedByClient)) => {
            ConnectError::NoAnswer
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
    use bevy::ecs::system::RunSystemOnce;
    use bevy_renet::netcode::NetcodeDisconnectReason as Reason;
    use shared::get_shared_renet_config;
    use shared::world::Chunk;

    use crate::network::status::ServerStatus;
    use std::thread::JoinHandle;

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
            Some(Reason::ConnectionTimedOut),
            Some(Reason::DisconnectedByClient),
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
            Some(Reason::ConnectionDenied),
            Some(Reason::DisconnectedByClient),
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
    fn the_first_end_of_a_session_wins() {
        let mut world = World::new();
        world.init_resource::<NextState<GameState>>();
        let ended = world
            .run_system_once(
                |mut commands: Commands, mut game: ResMut<NextState<GameState>>| {
                    let origin = server();
                    let first = end_session(
                        &mut commands,
                        &mut game,
                        &origin,
                        Some(ConnectError::NoAnswer),
                    );
                    let second = end_session(&mut commands, &mut game, &origin, None);
                    (first, second)
                },
            )
            .unwrap();
        assert_eq!(ended, (true, false));
        assert_eq!(
            world.resource::<SessionEnd>().error,
            Some(ConnectError::NoAnswer)
        );
        assert!(matches!(
            world.resource::<NextState<GameState>>(),
            NextState::Pending(GameState::Menu)
        ));
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
        // no chunk to wait for is all of them; more than expected is no more than all
        let all = 0.2 + 0.7;
        let empty = progress(ConnectStep::LoadingChunks {
            received: 0,
            total: 0,
        });
        assert!((empty - all).abs() < 1e-6, "{empty}");
        let over = progress(ConnectStep::LoadingChunks {
            received: 30,
            total: 27,
        });
        assert!((over - all).abs() < 1e-6, "{over}");
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
    fn the_step_counts_the_chunks_around_the_local_player() {
        let mut app = App::new();
        app.insert_resource(world())
            .insert_resource(TargetServer {
                address: None,
                username: None,
                session_token: None,
                state: TargetServerState::FullyReady,
            })
            .insert_resource(ConnectWatch::at(ConnectStep::StartingWorld))
            .init_resource::<ClientWorldMap>()
            .add_systems(Update, track_connection);
        // in the chunk (0, 2, 0)
        app.world_mut()
            .spawn((CurrentPlayerMarker {}, Transform::from_xyz(8., 40., 8.)));
        let mut cube = Vec::new();
        for x in -1..=1 {
            for y in 1..=3 {
                for z in -1..=1 {
                    cube.push(IVec3::new(x, y, z));
                }
            }
        }
        let load = |app: &mut App, chunks: &[IVec3]| {
            let mut map = app.world_mut().resource_mut::<ClientWorldMap>();
            for &pos in chunks {
                map.blocks.insert_chunk(pos, Chunk::new());
            }
        };

        load(&mut app, &cube[..12]);
        app.update();
        assert_eq!(
            app.world().resource::<ConnectWatch>().step,
            ConnectStep::LoadingChunks {
                received: 12,
                total: 27
            }
        );

        load(&mut app, &cube[12..]);
        app.update();
        assert_eq!(
            app.world().resource::<ConnectWatch>().step,
            ConnectStep::Spawning
        );
    }

    /// A bare app watching a session to the server `Nowhere` at `step`, without a new step for
    /// `stalled`, the client still connecting (no transport), the status asked with `probe`.
    fn watched(step: ConnectStep, stalled: Duration, probe: fn(&str) -> ServerStatus) -> App {
        let authenticated = !matches!(step, ConnectStep::Connecting);
        let mut app = App::new();
        app.init_resource::<NextState<GameState>>()
            .insert_resource(SessionOrigin::Server {
                name: "Nowhere".into(),
                ip: "127.0.0.1:9".into(),
                address: "127.0.0.1:9".parse().unwrap(),
            })
            .insert_resource(TargetServer {
                address: None,
                username: None,
                session_token: None,
                state: if authenticated {
                    TargetServerState::ConnectionEstablished
                } else {
                    TargetServerState::Establishing
                },
            })
            .insert_resource(RenetClient::new(get_shared_renet_config()))
            .insert_resource(CurrentPlayerProfile {
                id: 1,
                name: "Tester".into(),
            })
            .insert_resource(StatusProbe(probe))
            .init_resource::<ConnectTimeout>()
            .insert_resource(ConnectWatch {
                step,
                since: Instant::now() - stalled,
                verdict: None,
            })
            .add_systems(Update, watch_connection);
        app
    }

    /// Runs frames of `app` until its session ends, and returns its error. Fails after 5 s.
    fn ended(app: &mut App) -> Option<ConnectError> {
        let start = Instant::now();
        while !app.world().contains_resource::<SessionEnd>() {
            assert!(
                start.elapsed() < Duration::from_secs(5),
                "the session never ended"
            );
            app.update();
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(matches!(
            app.world().resource::<NextState<GameState>>(),
            NextState::Pending(GameState::Menu)
        ));
        app.world().resource::<SessionEnd>().error.clone()
    }

    /// Whether `app` ended its session after a few frames.
    fn ends_soon(app: &mut App) -> bool {
        for _ in 0..3 {
            app.update();
        }
        app.world().contains_resource::<SessionEnd>()
            || !matches!(
                app.world().resource::<NextState<GameState>>(),
                NextState::Unchanged
            )
    }

    fn never_asked(_: &str) -> ServerStatus {
        panic!("the status was asked")
    }

    const ELEVEN: Duration = Duration::from_secs(11);

    #[test]
    fn a_server_silent_for_too_long_times_out() {
        let mut app = watched(ConnectStep::Connecting, ELEVEN, |_| ServerStatus::Offline);
        assert_eq!(ended(&mut app), Some(ConnectError::NoAnswer));

        let mut app = watched(ConnectStep::Connecting, Duration::from_secs(9), |_| {
            ServerStatus::Offline
        });
        assert!(!ends_soon(&mut app));
        assert!(app.world().resource::<ConnectWatch>().verdict.is_none());
    }

    #[test]
    fn a_silent_server_of_another_version_says_so() {
        let mut app = watched(ConnectStep::Connecting, ELEVEN, |_| {
            ServerStatus::Incompatible
        });
        assert_eq!(ended(&mut app), Some(ConnectError::Incompatible));
    }

    #[test]
    fn a_stall_while_loading_chunks_is_a_lost_connection() {
        let loading = ConnectStep::LoadingChunks {
            received: 3,
            total: 27,
        };
        let mut app = watched(loading, ELEVEN, never_asked);
        assert_eq!(ended(&mut app), Some(ConnectError::LostWhileLoading));

        // authenticated, the local player never spawns: the step stays at no chunk, and the
        // tracking that runs before the watch never restarts its clock
        let mut app = watched(
            ConnectStep::LoadingChunks {
                received: 0,
                total: 27,
            },
            ELEVEN,
            never_asked,
        );
        app.init_resource::<ClientWorldMap>()
            .add_systems(Update, track_connection.before(watch_connection));
        assert_eq!(ended(&mut app), Some(ConnectError::LostWhileLoading));
    }

    #[test]
    fn the_spawning_step_never_times_out() {
        let mut app = watched(ConnectStep::Spawning, Duration::from_secs(60), never_asked);
        assert!(!ends_soon(&mut app));
    }

    #[test]
    fn a_cancel_in_the_frame_of_a_failure_shows_no_error() {
        let mut app = watched(ConnectStep::Connecting, ELEVEN, |_| ServerStatus::Offline);
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Menu);
        for _ in 0..3 {
            app.update();
        }
        assert!(!app.world().contains_resource::<SessionEnd>());
        assert!(app.world().resource::<ConnectWatch>().verdict.is_none());
    }

    /// A bare app watching the world `Broken` starting since long ago, its local server
    /// `server`, the client `client`.
    fn world_watched(server: JoinHandle<Result<(), String>>, client: RenetClient) -> App {
        let mut app = App::new();
        app.init_resource::<NextState<GameState>>()
            .insert_resource(SessionOrigin::World {
                name: "Broken".into(),
            })
            .insert_resource(TargetServer {
                address: Some("127.0.0.1:9".parse().unwrap()),
                username: None,
                session_token: None,
                state: TargetServerState::Establishing,
            })
            .insert_resource(client)
            .insert_resource(CurrentPlayerProfile {
                id: 1,
                name: "Tester".into(),
            })
            .insert_resource(StatusProbe(never_asked))
            .init_resource::<ConnectTimeout>()
            .insert_resource(LocalServerHandle(Some(server)))
            .insert_resource(ConnectWatch {
                step: ConnectStep::StartingWorld,
                since: Instant::now() - ELEVEN,
                verdict: None,
            })
            .add_systems(Update, watch_connection);
        app
    }

    #[test]
    fn a_world_whose_server_stopped_says_why() {
        let server = std::thread::spawn(|| Err("Failed to load world Broken: bad".to_string()));
        let client = RenetClient::new(get_shared_renet_config());
        let mut app = world_watched(server, client);
        assert_eq!(
            ended(&mut app),
            Some(ConnectError::WorldFailed(
                "Failed to load world Broken: bad".into()
            ))
        );
        assert!(app.world().resource::<LocalServerHandle>().0.is_none());

        // a panicking server reads the same, but the dev profile (Cranelift) cannot unwind one
        let server = std::thread::spawn(|| Ok(()));
        let client = RenetClient::new(get_shared_renet_config());
        let mut app = world_watched(server, client);
        assert_eq!(
            ended(&mut app),
            Some(ConnectError::WorldFailed(
                "The world stopped while opening".into()
            ))
        );
    }

    #[test]
    fn a_world_still_starting_gets_a_new_connection() {
        let (release, wait) = std::sync::mpsc::channel::<()>();
        let server = std::thread::spawn(move || {
            let _ = wait.recv();
            Ok(())
        });
        let mut client = RenetClient::new(get_shared_renet_config());
        client.disconnect();
        let mut app = world_watched(server, client);
        assert!(!ends_soon(&mut app));
        assert!(app.world().contains_resource::<NetcodeClientTransport>());
        assert!(!app.world().resource::<RenetClient>().is_disconnected());
        // the authentication is asked again on the new connection
        assert_eq!(
            app.world().resource::<TargetServer>().state,
            TargetServerState::Initial
        );
        let watch = app.world().resource::<ConnectWatch>();
        assert!(
            watch.since.elapsed() < ELEVEN,
            "{:?}",
            watch.since.elapsed()
        );

        release.send(()).unwrap();
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

    /// A bare app in the game on the server `Friends`, with the client `client` and no transport.
    fn in_game(client: RenetClient) -> App {
        let mut app = App::new();
        app.init_resource::<NextState<GameState>>()
            .insert_resource(server())
            .insert_resource(client)
            .insert_resource(CurrentPlayerProfile {
                id: 1,
                name: "Tester".into(),
            })
            .add_systems(Update, watch_session);
        app
    }

    #[test]
    fn leaving_on_purpose_is_never_an_error() {
        let mut client = RenetClient::new(get_shared_renet_config());
        client.disconnect();
        let mut app = in_game(client);
        // the pause menu leaves the game in the frame the connection drops
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Menu);
        app.update();
        assert!(!app.world().contains_resource::<SessionEnd>());
    }

    #[test]
    fn a_connection_dropped_in_game_ends_the_session() {
        let mut client = RenetClient::new(get_shared_renet_config());
        client.disconnect();
        let mut app = in_game(client);
        app.update();
        assert_eq!(
            app.world()
                .get_resource::<SessionEnd>()
                .map(|end| end.error.clone()),
            Some(Some(ConnectError::ConnectionLost))
        );
        assert!(matches!(
            app.world().resource::<NextState<GameState>>(),
            NextState::Pending(GameState::Menu)
        ));
    }
}
