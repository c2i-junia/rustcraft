//! Leaving a world for the title without blocking a frame: `Exit` is sent at once, and the game
//! goes on behind the pause menu's "Saving..." until the local server has saved and stopped, or
//! for `STOP_TIMEOUT`, when the title opens anyway and the server finishes in the background.

use std::time::{Duration, Instant};

use bevy::prelude::*;
use bevy_renet::netcode::NetcodeClientTransport;
use bevy_renet::RenetClient;

use super::cleanup::{send_exit, ExitSent};
use super::LocalServerHandle;
use crate::state::GameState;

/// How long a world may take to save and stop: the game waits that long for the local server
/// before the title opens anyway, and so do the quit, the app's last frame and a world opened
/// while the last one closes.
pub const STOP_TIMEOUT: Duration = Duration::from_secs(10);

/// The game is being left on purpose: Exit was sent, the game waits for the local server to stop.
#[derive(Resource, Debug)]
pub struct Leaving {
    pub since: Instant,
}

/// Where leaving the game is at.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LeaveStep {
    /// The local server still saves.
    Wait,
    /// The local server stopped: the world is saved.
    Stopped,
    /// The local server did not stop within `STOP_TIMEOUT`: it finishes in the background.
    GaveUp,
}

/// The step of leaving the game `elapsed` after `Exit` was sent. A server that stopped is
/// `Stopped`, however late.
pub fn leave_step(server_running: bool, elapsed: Duration) -> LeaveStep {
    if !server_running {
        LeaveStep::Stopped
    } else if elapsed >= STOP_TIMEOUT {
        LeaveStep::GaveUp
    } else {
        LeaveStep::Wait
    }
}

/// Asks to leave the game for the title once the world is saved.
#[derive(Event)]
pub struct LeaveGame;

/// Asked by the window's close button and the main menu's Quit (`ui/quit.rs`).
#[derive(Event)]
pub struct QuitRequested;

/// The app quits once the world is saved: asked since `since`.
#[derive(Resource)]
pub struct Quitting {
    pub since: Instant,
}

/// Whether the game is being left on purpose.
pub fn leaving(leaving: Option<Res<Leaving>>) -> bool {
    leaving.is_some()
}

/// Leaving the game on purpose: `LeaveGame` starts it, the title opens once the local server
/// stopped or `STOP_TIMEOUT` passed.
pub fn leave_plugin(app: &mut App) {
    app.add_observer(start_leaving)
        .add_systems(
            Update,
            finish_leaving
                .run_if(in_state(GameState::Game))
                .run_if(resource_exists::<Leaving>),
        )
        .add_systems(OnExit(GameState::Game), |mut commands: Commands| {
            commands.remove_resource::<Leaving>();
        });
}

/// Tells the local server to save and stop, and waits for it in the game. Without a local server,
/// or a connection to tell it, the title opens at once, and the end of the session tells it. Does
/// nothing while the game is already being left: `Exit` is sent once.
fn start_leaving(
    _: On<LeaveGame>,
    leaving: Option<Res<Leaving>>,
    client: Option<ResMut<RenetClient>>,
    transport: Option<ResMut<NetcodeClientTransport>>,
    local_server: Option<Res<LocalServerHandle>>,
    mut game: ResMut<NextState<GameState>>,
    mut commands: Commands,
) {
    if leaving.is_some() {
        return;
    }
    let running = local_server.is_some_and(|local_server| local_server.0.is_some());
    let told = running
        && match (client, transport) {
            (Some(mut client), Some(mut transport)) => send_exit(&mut client, &mut transport),
            _ => false,
        };
    if !told {
        game.set(GameState::Menu);
        return;
    }
    info!("Leaving the game once the local server has saved the world");
    commands.insert_resource(Leaving {
        since: Instant::now(),
    });
    commands.insert_resource(ExitSent);
}

/// Opens the title once the local server stopped, or after `STOP_TIMEOUT`.
fn finish_leaving(
    leaving: Res<Leaving>,
    local_server: Option<Res<LocalServerHandle>>,
    mut game: ResMut<NextState<GameState>>,
) {
    let running = local_server
        .is_some_and(|local_server| local_server.0.as_ref().is_some_and(|h| !h.is_finished()));
    match leave_step(running, leaving.since.elapsed()) {
        LeaveStep::Wait => {}
        LeaveStep::Stopped => game.set(GameState::Menu),
        LeaveStep::GaveUp => {
            warn!("The local server did not stop within {STOP_TIMEOUT:?}, it finishes in the background");
            game.set(GameState::Menu);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::state::app::StatesPlugin;
    use std::sync::mpsc::{channel, Sender};
    use std::thread::JoinHandle;

    #[test]
    fn leaving_waits_for_the_server_then_gives_up() {
        let s = Duration::from_secs;
        assert_eq!(leave_step(true, s(1)), LeaveStep::Wait);
        assert_eq!(leave_step(false, s(1)), LeaveStep::Stopped);
        assert_eq!(leave_step(true, s(10)), LeaveStep::GaveUp);
        assert_eq!(leave_step(false, s(11)), LeaveStep::Stopped);
    }

    /// A thread parked until the sender returned is used, as a local server still saving.
    fn parked() -> (Sender<()>, JoinHandle<Result<(), String>>) {
        let (release, wait) = channel::<()>();
        let server = std::thread::spawn(move || {
            let _ = wait.recv();
            Ok(())
        });
        (release, server)
    }

    /// A bare app in the game, leaving it since `since`, its local server `server`.
    fn leaving_app(since: Instant, server: JoinHandle<Result<(), String>>) -> App {
        let mut app = App::new();
        app.add_plugins(StatesPlugin)
            .insert_state(GameState::Game)
            .add_plugins(leave_plugin)
            .insert_resource(LocalServerHandle(Some(server)))
            .insert_resource(Leaving { since });
        app
    }

    fn state(app: &App) -> GameState {
        *app.world().resource::<State<GameState>>().get()
    }

    /// Runs frames of `app` until it is in the menu. Fails the test after 10 s.
    fn update_until_the_menu(app: &mut App) {
        let start = Instant::now();
        while state(app) != GameState::Menu {
            assert!(
                start.elapsed() < Duration::from_secs(10),
                "still in the game"
            );
            app.update();
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    #[test]
    fn leaving_never_blocks_a_frame() {
        let (release, server) = parked();
        let mut app = leaving_app(Instant::now(), server);
        for _ in 0..3 {
            let start = Instant::now();
            app.update();
            assert!(
                start.elapsed() < Duration::from_millis(50),
                "a frame took {:?}",
                start.elapsed()
            );
            assert_eq!(state(&app), GameState::Game);
        }
        release.send(()).unwrap();
        update_until_the_menu(&mut app);
        assert!(!app.world().contains_resource::<Leaving>());

        // a server that does not stop in time finishes in the background
        let (release, server) = parked();
        let mut app = leaving_app(Instant::now() - Duration::from_secs(11), server);
        update_until_the_menu(&mut app);
        let handle = app.world().resource::<LocalServerHandle>().0.as_ref();
        assert!(handle.is_some_and(|handle| !handle.is_finished()));
        release.send(()).unwrap();
    }

    #[test]
    fn a_second_leave_does_nothing() {
        let (release, server) = parked();
        let since = Instant::now();
        let mut app = leaving_app(since, server);
        // with no connection, a first leave would go to the title at once
        app.world_mut().trigger(LeaveGame);
        app.update();
        assert_eq!(state(&app), GameState::Game);
        assert_eq!(app.world().resource::<Leaving>().since, since);
        release.send(()).unwrap();
    }
}
