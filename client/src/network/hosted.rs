//! Joining a hosted world: the hub starts it when it is stopped, then its status port is asked
//! every `POLL_EVERY` until it answers, for `START_TIMEOUT` at most, all on a thread while the
//! connecting card shows "Starting the world...". The session then connects as to any server.

use std::net::{Ipv4Addr, SocketAddr};
use std::thread;
use std::time::{Duration, Instant};

use bevy::prelude::*;
use rustcraft_protocol::api::RealmAddress;
use uuid::Uuid;

use super::address::{parse_server_address, AddressResolver};
use super::session::{end_session, ConnectError, SessionOrigin};
use super::status::{InFlight, ServerStatus, StatusProbe};
use super::{TargetServer, TargetServerState};
use crate::account::file::AccountFile;
use crate::account::hub::{HubClient, HubError};
use crate::state::GameState;
use crate::ui::menus::MenuState;

/// How long a hosted world has to answer on its status port once the hub started it.
pub const START_TIMEOUT: Duration = Duration::from_secs(60);
/// How often the status port of a starting world is asked.
pub const POLL_EVERY: Duration = Duration::from_millis(500);

/// A hosted world to join: which one, and where it runs when the list said so.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HostedJoin {
    pub realm: Uuid,
    pub name: String,
    /// The address of a world that is starting or running: no start is asked then.
    pub address: Option<RealmAddress>,
}

/// The start of the hosted world of the session, in `PreGameLoading` until its server answers:
/// its address as typed (`host:port`) and resolved, or why it did not start.
#[derive(Resource)]
pub struct HostedStart(InFlight<Result<(String, SocketAddr), ConnectError>>);

/// The address of a world as a server list would have it typed: `host:port`.
pub fn typed_address(address: &RealmAddress) -> String {
    format!("{}:{}", address.host, address.port)
}

/// What the player is told of a hub that refused or did not answer, on the error card.
fn hub_failure(error: &HubError, hub: &str) -> ConnectError {
    let message = match error {
        HubError::Api { message, .. } => message.clone(),
        HubError::Unreachable(_) => {
            let host = hub.split_once("://").map_or(hub, |(_, rest)| rest);
            format!("Cannot reach the hub at {}.", host.trim_end_matches('/'))
        }
        HubError::Malformed(_) => error.to_string(),
    };
    ConnectError::HostedWorldFailed(message)
}

/// Asks the hub to start the world unless its address is known, then asks its status port every
/// `POLL_EVERY` until it answers, for `timeout` at most. Blocks: call it from a thread.
pub fn start_and_wait(
    client: &HubClient,
    hub: &str,
    session: &str,
    join: &HostedJoin,
    (probe, resolver): (StatusProbe, AddressResolver),
    timeout: Duration,
) -> Result<(String, SocketAddr), ConnectError> {
    let address = match &join.address {
        Some(address) => address.clone(),
        None => {
            let started = client
                .start_realm(session, join.realm)
                .map_err(|error| hub_failure(&error, hub))?;
            RealmAddress {
                host: started.host,
                port: started.port,
            }
        }
    };
    let typed = typed_address(&address);
    let not_found = || ConnectError::HostedWorldFailed(format!("Cannot find {}.", address.host));
    let parsed = parse_server_address(&typed).map_err(|_| not_found())?;
    let resolved = (resolver.0)(&parsed).map_err(|_| not_found())?;
    let deadline = Instant::now() + timeout;
    loop {
        let asked = Instant::now();
        match (probe.0)(&typed) {
            ServerStatus::Online { .. } => return Ok((typed, resolved)),
            ServerStatus::Incompatible => return Err(ConnectError::Incompatible),
            _ => {}
        }
        if Instant::now() >= deadline {
            return Err(ConnectError::HostedWorldFailed(
                "The world did not start in time. Try again in a moment.".to_string(),
            ));
        }
        thread::sleep(POLL_EVERY.saturating_sub(asked.elapsed()));
    }
}

/// Opens the session of the hosted world `join` with the account `file`: the connecting card
/// shows while the world starts, and the session connects once it answers. Stands down when the
/// game already leaves the menus this frame.
pub fn join_hosted_world(
    join: &HostedJoin,
    file: &AccountFile,
    seams: (StatusProbe, AddressResolver),
    target: &mut TargetServer,
    commands: &mut Commands,
    (game, menu): (&mut NextState<GameState>, &mut NextState<MenuState>),
) {
    if !matches!(game, NextState::Unchanged) {
        return;
    }
    info!("Joining the hosted world {:?}", join.name);
    let (client, hub, session, request) = (
        HubClient::new(&file.hub),
        file.hub.clone(),
        file.session.clone(),
        join.clone(),
    );
    commands.insert_resource(HostedStart(InFlight::spawn(
        Err(ConnectError::NoConnection(
            "no thread to start the world on".into(),
        )),
        move || start_and_wait(&client, &hub, &session, &request, seams, START_TIMEOUT),
    )));
    commands.insert_resource(SessionOrigin::Server {
        name: join.name.clone(),
        ip: join.address.as_ref().map(typed_address).unwrap_or_default(),
        address: SocketAddr::from((Ipv4Addr::UNSPECIFIED, 0)),
        realm: Some(join.realm),
    });
    target.address = None;
    target.state = TargetServerState::Initial;
    game.set(GameState::PreGameLoading);
    menu.set(MenuState::Disabled);
}

/// Hands the address of the hosted world over to the connection once it answers, or ends the
/// session with why it did not start. Stands down while the session already ends this frame.
pub fn wait_for_hosted_world(
    start: Res<HostedStart>,
    origin: Option<ResMut<SessionOrigin>>,
    mut target: ResMut<TargetServer>,
    mut game: ResMut<NextState<GameState>>,
    mut commands: Commands,
) {
    let Some(mut origin) = origin else {
        return;
    };
    if !matches!(*game, NextState::Unchanged) {
        return;
    }
    let Some(result) = start.0.take() else {
        return;
    };
    commands.remove_resource::<HostedStart>();
    match result {
        Ok((typed, resolved)) => {
            info!("The hosted world answers at {typed}, joining it at {resolved}");
            if let SessionOrigin::Server { ip, address, .. } = &mut *origin {
                *ip = typed;
                *address = resolved;
            }
            target.address = Some(resolved);
            target.state = TargetServerState::Initial;
        }
        Err(error) => {
            warn!("The hosted world did not start: {error:?}");
            end_session(&mut commands, &mut game, &origin, Some(error));
        }
    }
}

#[cfg(test)]
impl HostedStart {
    /// A start whose result is already there.
    pub(crate) fn ready(result: Result<(String, SocketAddr), ConnectError>) -> Self {
        Self(InFlight::ready(result))
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;
    use crate::account::test_hub::{Canned, TestHub};
    use crate::network::address::ServerAddress;

    fn join(address: Option<RealmAddress>) -> HostedJoin {
        HostedJoin {
            realm: Uuid::from_u128(10),
            name: "Home".into(),
            address,
        }
    }

    fn local(address: &ServerAddress) -> std::io::Result<SocketAddr> {
        Ok(SocketAddr::from(([127, 0, 0, 1], address.port)))
    }

    /// How many times `online_third` was asked.
    static ASKED: AtomicUsize = AtomicUsize::new(0);

    /// Offline twice, then online.
    fn online_third(_: &str) -> ServerStatus {
        if ASKED.fetch_add(1, Ordering::SeqCst) < 2 {
            ServerStatus::Offline
        } else {
            ServerStatus::Online {
                players: 0,
                max_players: 10,
                latency: Duration::from_millis(5),
            }
        }
    }

    fn offline(_: &str) -> ServerStatus {
        ServerStatus::Offline
    }

    #[test]
    fn a_stopped_world_is_started_then_asked_until_it_answers() {
        let hub = TestHub::start(vec![Canned::ok(
            r#"{"state":"starting","host":"play.example","port":25002}"#,
        )]);
        let client = HubClient::new(&hub.url);
        let started = Instant::now();

        let result = start_and_wait(
            &client,
            &hub.url,
            "tok",
            &join(None),
            (StatusProbe(online_third), AddressResolver(local)),
            Duration::from_secs(10),
        );

        assert_eq!(
            result,
            Ok((
                "play.example:25002".to_string(),
                "127.0.0.1:25002".parse().unwrap()
            ))
        );
        assert_eq!(ASKED.load(Ordering::SeqCst), 3);
        // asked every POLL_EVERY
        assert!(
            started.elapsed() >= 2 * POLL_EVERY,
            "{:?}",
            started.elapsed()
        );
        let request = &hub.requests()[0];
        assert_eq!(
            request.path,
            format!("/v1/realms/{}/start", Uuid::from_u128(10))
        );
        assert_eq!(request.header("authorization"), Some("Bearer tok"));
    }

    #[test]
    fn a_running_world_is_not_started_again() {
        let hub = TestHub::start(Vec::new());
        let address = RealmAddress {
            host: "play.example".into(),
            port: 25000,
        };
        let result = start_and_wait(
            &HubClient::new(&hub.url),
            &hub.url,
            "tok",
            &join(Some(address)),
            (
                StatusProbe(|_| ServerStatus::Incompatible),
                AddressResolver(local),
            ),
            Duration::from_secs(10),
        );
        assert_eq!(result, Err(ConnectError::Incompatible));
        assert!(hub.requests().is_empty());
    }

    #[test]
    fn a_refusal_of_the_hub_or_a_silent_world_is_said() {
        let hub = TestHub::start(vec![Canned::error(
            429,
            "rate_limited",
            "This world was started too often. Try again later.",
        )]);
        let seams = (StatusProbe(offline), AddressResolver(local));
        let refused = start_and_wait(
            &HubClient::new(&hub.url),
            &hub.url,
            "tok",
            &join(None),
            seams,
            Duration::from_secs(10),
        );
        assert_eq!(
            refused,
            Err(ConnectError::HostedWorldFailed(
                "This world was started too often. Try again later.".into()
            ))
        );

        let address = RealmAddress {
            host: "play.example".into(),
            port: 25000,
        };
        let silent = start_and_wait(
            &HubClient::new(&hub.url),
            &hub.url,
            "tok",
            &join(Some(address)),
            seams,
            Duration::from_millis(600),
        );
        assert_eq!(
            silent,
            Err(ConnectError::HostedWorldFailed(
                "The world did not start in time. Try again in a moment.".into()
            ))
        );
    }

    #[test]
    fn a_hub_that_cannot_be_reached_is_named() {
        let port = {
            let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            listener.local_addr().unwrap().port()
        };
        let hub = format!("http://127.0.0.1:{port}");
        let result = start_and_wait(
            &HubClient::new(&hub),
            &hub,
            "tok",
            &join(None),
            (StatusProbe(offline), AddressResolver(local)),
            Duration::from_secs(1),
        );
        assert_eq!(
            result,
            Err(ConnectError::HostedWorldFailed(format!(
                "Cannot reach the hub at 127.0.0.1:{port}."
            )))
        );
    }

    fn app_waiting(result: Result<(String, SocketAddr), ConnectError>) -> App {
        use bevy::state::app::StatesPlugin;
        let mut app = App::new();
        app.add_plugins(StatesPlugin)
            .init_state::<GameState>()
            .insert_resource(TargetServer {
                address: None,
                username: None,
                session_token: None,
                state: TargetServerState::Initial,
            })
            .insert_resource(SessionOrigin::Server {
                name: "Home".into(),
                ip: String::new(),
                address: SocketAddr::from((Ipv4Addr::UNSPECIFIED, 0)),
                realm: Some(Uuid::from_u128(10)),
            })
            .insert_resource(HostedStart::ready(result))
            .add_systems(
                Update,
                wait_for_hosted_world.run_if(resource_exists::<HostedStart>),
            );
        app
    }

    #[test]
    fn the_answering_world_becomes_the_server_of_the_session() {
        let address: SocketAddr = "127.0.0.1:25002".parse().unwrap();
        let mut app = app_waiting(Ok(("play.example:25002".into(), address)));
        app.update();
        assert_eq!(
            app.world().resource::<TargetServer>().address,
            Some(address)
        );
        assert!(!app.world().contains_resource::<HostedStart>());
        let SessionOrigin::Server {
            ip, address: at, ..
        } = app.world().resource::<SessionOrigin>()
        else {
            panic!("not a server");
        };
        assert_eq!((ip.as_str(), *at), ("play.example:25002", address));
    }

    #[test]
    fn a_world_that_did_not_start_ends_the_session_with_why() {
        let error = ConnectError::HostedWorldFailed("No.".into());
        let mut app = app_waiting(Err(error.clone()));
        app.update();
        let end = app
            .world()
            .resource::<crate::network::session::SessionEnd>();
        assert_eq!(end.error, Some(error));
        assert_eq!(app.world().resource::<TargetServer>().address, None);
    }
}
