//! Joining a hosted world: the hub is asked to start it (a world that runs already only says
//! where), then its status port is asked every `POLL_EVERY` until it answers, for
//! `START_TIMEOUT` at most, all on a thread while the connecting card shows "Starting the
//! world...". The session then connects as to any server.

use std::net::{Ipv4Addr, SocketAddr};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
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
use crate::account::hub::{hub_message, HubClient};
use crate::state::GameState;
use crate::ui::menus::account::end_revoked_session;
use crate::ui::menus::MenuState;

/// How long a hosted world has to answer on its status port once the hub started it.
pub const START_TIMEOUT: Duration = Duration::from_secs(60);
/// How often the status port of a starting world is asked.
pub const POLL_EVERY: Duration = Duration::from_millis(500);

/// A hosted world to join.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HostedJoin {
    pub realm: Uuid,
    pub name: String,
}

/// The start of the hosted world of the session, in `PreGameLoading` until its server answers:
/// its address as typed (`host:port`) and resolved, or why it did not start. Dropped (the
/// session was cancelled), it tells its thread to stop asking.
#[derive(Resource)]
pub struct HostedStart {
    result: InFlight<Result<(String, SocketAddr), ConnectError>>,
    /// The account the world is joined with: logged out if the hub no longer knows its session.
    account: Uuid,
    cancel: Arc<AtomicBool>,
}

impl Drop for HostedStart {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::SeqCst);
    }
}

/// The address of a world as a server list would have it typed: `host:port`.
pub fn typed_address(address: &RealmAddress) -> String {
    format!("{}:{}", address.host, address.port)
}

/// Asks the hub to start the world, then asks its status port every `POLL_EVERY` until it
/// answers, for `timeout` at most from the hub's answer. Gives up once `cancel` is set. Blocks:
/// call it from a thread.
pub fn start_and_wait(
    client: &HubClient,
    hub: &str,
    session: &str,
    join: &HostedJoin,
    (probe, resolver): (StatusProbe, AddressResolver),
    timeout: Duration,
    cancel: &AtomicBool,
) -> Result<(String, SocketAddr), ConnectError> {
    let started = client.start_realm(session, join.realm).map_err(|error| {
        if error.is_unauthorized() {
            ConnectError::HubSessionEnded
        } else {
            ConnectError::HostedWorldFailed(hub_message(&error, hub))
        }
    })?;
    let address = RealmAddress {
        host: started.host,
        port: started.port,
    };
    let typed = typed_address(&address);
    let not_found = || ConnectError::HostedWorldFailed(format!("Cannot find {}.", address.host));
    let parsed = parse_server_address(&typed).map_err(|_| not_found())?;
    let resolved = (resolver.0)(&parsed).map_err(|_| not_found())?;
    let deadline = Instant::now() + timeout;
    let cancelled = || ConnectError::NoConnection("the session was cancelled".into());
    loop {
        if cancel.load(Ordering::SeqCst) {
            return Err(cancelled());
        }
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
        if cancel.load(Ordering::SeqCst) {
            return Err(cancelled());
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
    let cancel = Arc::new(AtomicBool::new(false));
    let (client, hub, session, request, stop) = (
        HubClient::new(&file.hub),
        file.hub.clone(),
        file.session.clone(),
        join.clone(),
        Arc::clone(&cancel),
    );
    commands.insert_resource(HostedStart {
        result: InFlight::spawn(
            Err(ConnectError::NoConnection(
                "no thread to start the world on".into(),
            )),
            move || {
                start_and_wait(
                    &client,
                    &hub,
                    &session,
                    &request,
                    seams,
                    START_TIMEOUT,
                    &stop,
                )
            },
        ),
        account: file.account,
        cancel,
    });
    commands.insert_resource(SessionOrigin::Server {
        name: join.name.clone(),
        ip: String::new(),
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
    let Some(result) = start.result.take() else {
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
            if error == ConnectError::HubSessionEnded {
                end_revoked_session(&mut commands, start.account);
            }
            end_session(&mut commands, &mut game, &origin, Some(error));
        }
    }
}

#[cfg(test)]
impl HostedStart {
    /// A start whose result is already there.
    pub(crate) fn ready(result: Result<(String, SocketAddr), ConnectError>) -> Self {
        Self {
            result: InFlight::ready(result),
            account: Uuid::from_u128(1),
            cancel: Arc::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;
    use crate::account::test_hub::{Canned, TestHub};
    use crate::network::address::ServerAddress;

    fn join() -> HostedJoin {
        HostedJoin {
            realm: Uuid::from_u128(10),
            name: "Home".into(),
        }
    }

    fn running() -> Canned {
        Canned::ok(r#"{"state":"running","host":"play.example","port":25000}"#)
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
            &join(),
            (StatusProbe(online_third), AddressResolver(local)),
            Duration::from_secs(10),
            &AtomicBool::new(false),
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
    fn a_refusal_of_the_hub_or_a_silent_world_is_said() {
        let hub = TestHub::start(vec![
            Canned::error(
                429,
                "rate_limited",
                "This world was started too often. Try again later.",
            ),
            Canned::error(401, "unauthorized", "Log in again."),
            running(),
        ]);
        let seams = (StatusProbe(offline), AddressResolver(local));
        let ask = |timeout| {
            start_and_wait(
                &HubClient::new(&hub.url),
                &hub.url,
                "tok",
                &join(),
                seams,
                timeout,
                &AtomicBool::new(false),
            )
        };
        assert_eq!(
            ask(Duration::from_secs(10)),
            Err(ConnectError::HostedWorldFailed(
                "This world was started too often. Try again later.".into()
            ))
        );
        assert_eq!(
            ask(Duration::from_secs(10)),
            Err(ConnectError::HubSessionEnded)
        );
        // a running world is asked to start too: the hub only says where it runs
        assert_eq!(
            ask(Duration::from_millis(600)),
            Err(ConnectError::HostedWorldFailed(
                "The world did not start in time. Try again in a moment.".into()
            ))
        );
        assert_eq!(hub.requests().len(), 3);
    }

    /// How many times `counted` was asked.
    static COUNTED: AtomicUsize = AtomicUsize::new(0);

    fn counted(_: &str) -> ServerStatus {
        COUNTED.fetch_add(1, Ordering::SeqCst);
        ServerStatus::Offline
    }

    #[test]
    fn a_cancelled_start_stops_asking_the_world() {
        let hub = TestHub::start(vec![running()]);
        let start = HostedStart::ready(Err(ConnectError::NoAnswer));
        let cancel = Arc::clone(&start.cancel);
        // the session is cancelled: the start is dropped with it
        drop(start);
        let began = Instant::now();
        let result = start_and_wait(
            &HubClient::new(&hub.url),
            &hub.url,
            "tok",
            &join(),
            (StatusProbe(counted), AddressResolver(local)),
            Duration::from_secs(10),
            &cancel,
        );
        assert!(matches!(result, Err(ConnectError::NoConnection(_))));
        assert_eq!(COUNTED.load(Ordering::SeqCst), 0);
        assert!(began.elapsed() < POLL_EVERY, "{:?}", began.elapsed());
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
            &join(),
            (StatusProbe(offline), AddressResolver(local)),
            Duration::from_secs(1),
            &AtomicBool::new(false),
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
