//! Quitting the game saves a world first: the window's close button and the main menu's Quit
//! ask to quit (`QuitRequested`). With a world being played or still starting, or a local server
//! still running, a card says the world is being saved, the game is left, and the app exits once
//! its server stopped, or after `STOP_TIMEOUT`. A second request quits at once.

use std::time::{Duration, Instant};

use accesskit::Role;
use bevy::{
    a11y::AccessibilityNode,
    input_focus::{tab_navigation::TabGroup, InputFocus},
    prelude::*,
    window::WindowCloseRequested,
};

use crate::network::leave::{LeaveGame, LeaveStep, QuitRequested, Quitting, STOP_TIMEOUT};
use crate::network::session::{end_session, SessionOrigin};
use crate::network::{finish_stopping_local_server, LocalServerHandle};
use crate::state::GameState;
use crate::ui::assets::FONT_PATH;
use crate::ui::hud::UiDialog;
use crate::ui::theme::{
    BEVEL_LO, BEVEL_WIDTH, FONT_L, FONT_S, MUTED, PANEL, SPACE_S, SPACE_XL, TEXT,
};
use crate::ui::widgets::dialog::DIALOG_Z;
use crate::ui::widgets::raised_bevel;

/// The title of the card shown while the world is saved before quitting.
pub const QUITTING_TITLE: &str = "Saving the world...";
/// The note under it.
pub const QUIT_NOW_NOTE: &str = "Close the window again to quit now.";

/// The card shown while the world is saved before quitting.
#[derive(Component)]
pub struct QuittingCard;

/// Whether the app may exit: no session left and no local server running, or STOP_TIMEOUT passed.
pub fn quit_step(in_session: bool, server_running: bool, elapsed: Duration) -> LeaveStep {
    if !in_session && !server_running {
        LeaveStep::Stopped
    } else if elapsed >= STOP_TIMEOUT {
        LeaveStep::GaveUp
    } else {
        LeaveStep::Wait
    }
}

/// The window's close button asks to quit, and so does the main menu's Quit; the app exits once
/// the world is saved.
pub fn quit_plugin(app: &mut App) {
    app.add_observer(on_quit_requested)
        .add_systems(
            Update,
            (
                forward_close_requests,
                // the menus join a local server that stopped before the app exits
                quit_once_stopped
                    .after(finish_stopping_local_server)
                    .run_if(resource_exists::<Quitting>),
            ),
        )
        .add_systems(Last, keep_the_focus_off.run_if(resource_exists::<Quitting>));
}

/// Takes back the focus that a screen opening beneath the card, or Tab, gave: no key reaches
/// through the card.
fn keep_the_focus_off(mut focus: ResMut<InputFocus>) {
    if focus.get().is_some() {
        focus.clear();
    }
}

/// Whether a world of this computer is being played or still starting.
fn in_world(state: GameState, origin: Option<&SessionOrigin>) -> bool {
    matches!(state, GameState::Game | GameState::PreGameLoading)
        && origin.is_some_and(SessionOrigin::is_world)
}

/// Whether a local server is left to join: its thread still runs, or it finished and the menus
/// have not joined it yet, which logs why it stopped when it failed.
fn server_to_join(local_server: Option<&LocalServerHandle>) -> bool {
    local_server.is_some_and(|local_server| local_server.0.is_some())
}

/// The window's close button asks to quit.
fn forward_close_requests(mut closes: MessageReader<WindowCloseRequested>, mut commands: Commands) {
    if closes.read().count() > 0 {
        commands.trigger(QuitRequested);
    }
}

/// Quits at once when nothing is left to save, or when the app already waits for the world to
/// be saved (a second close). Otherwise shows the card and leaves the world: the game saves it
/// as Save and quit does, and a world still starting is cancelled, then told to stop once it
/// accepts the connection. A server session with a local server still running is left for the
/// menus, where that server, which a world cancelled before it answered may have left untold,
/// is told to stop and joined.
fn on_quit_requested(
    _: On<QuitRequested>,
    quitting: Option<Res<Quitting>>,
    local_server: Option<Res<LocalServerHandle>>,
    origin: Option<Res<SessionOrigin>>,
    state: Res<State<GameState>>,
    assets: Res<AssetServer>,
    mut focus: ResMut<InputFocus>,
    mut game: ResMut<NextState<GameState>>,
    mut exit: MessageWriter<AppExit>,
    mut commands: Commands,
) {
    if quitting.is_some() {
        info!("Quitting now, without waiting for the world to be saved");
        exit.write(AppExit::Success);
        return;
    }
    let state = *state.get();
    let world = in_world(state, origin.as_deref());
    if !world && !server_to_join(local_server.as_deref()) {
        exit.write(AppExit::Success);
        return;
    }
    info!("Quitting once the world is saved");
    commands.insert_resource(Quitting {
        since: Instant::now(),
    });
    spawn_quitting_card(&mut commands, &assets);
    focus.clear();
    match (state, origin) {
        (GameState::Game, _) if world => commands.trigger(LeaveGame),
        (GameState::Game | GameState::PreGameLoading, Some(origin)) => {
            end_session(&mut commands, &mut game, &origin, None);
        }
        _ => {}
    }
}

/// Exits once no world is left to save, or after `STOP_TIMEOUT`, when a world still loading or
/// saving is stopped with the process.
fn quit_once_stopped(
    quitting: Res<Quitting>,
    local_server: Option<Res<LocalServerHandle>>,
    origin: Option<Res<SessionOrigin>>,
    state: Res<State<GameState>>,
    mut exit: MessageWriter<AppExit>,
) {
    let in_session = in_world(*state.get(), origin.as_deref());
    let running = server_to_join(local_server.as_deref());
    match quit_step(in_session, running, quitting.since.elapsed()) {
        LeaveStep::Wait => {}
        LeaveStep::Stopped => {
            exit.write(AppExit::Success);
        }
        LeaveStep::GaveUp => {
            warn!("The world was not saved within {STOP_TIMEOUT:?}, quitting anyway");
            exit.write(AppExit::Success);
        }
    }
}

/// Spawns the card shown while the world is saved before quitting: over everything, dialogs
/// included, modal, nothing on it to focus. It is not despawned with a state: the app exits
/// with it.
pub fn spawn_quitting_card(commands: &mut Commands, assets: &AssetServer) -> Entity {
    let font: Handle<Font> = assets.load(FONT_PATH);
    let text = |content: &str, size: f32, color: Color| {
        (
            Text::new(content),
            TextFont {
                font: font.clone().into(),
                font_size: px(size).into(),
                ..default()
            },
            TextColor(color),
            TextLayout::linebreak(LineBreak::WordOrCharacter),
        )
    };
    let root = commands
        .spawn((
            QuittingCard,
            // the game keys wait while it shows
            UiDialog,
            Visibility::Visible,
            // the role before the label: the label's default node must not win
            AccessibilityNode(accesskit::Node::new(Role::Dialog)),
            AccessibleLabel::new(QUITTING_TITLE),
            // Tab stays in it, where there is nothing to focus
            TabGroup::modal(),
            Node {
                position_type: PositionType::Absolute,
                width: percent(100),
                height: percent(100),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(BEVEL_LO.with_alpha(0.7)),
            GlobalZIndex(DIALOG_Z + 10),
        ))
        .id();
    commands.spawn((
        Node {
            flex_direction: FlexDirection::Column,
            row_gap: px(SPACE_S),
            padding: UiRect::all(px(SPACE_XL)),
            border: UiRect::all(px(BEVEL_WIDTH)),
            max_width: percent(90),
            ..default()
        },
        BackgroundColor(PANEL),
        raised_bevel(),
        ChildOf(root),
        children![
            text(QUITTING_TITLE, FONT_L, TEXT),
            text(QUIT_NOW_NOTE, FONT_S, MUTED),
        ],
    ));
    root
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::network::finish_stopping_local_server;
    use crate::network::session::SessionEnd;
    use bevy::input_focus::FocusCause;
    use bevy::state::app::StatesPlugin;
    use bevy::window::WindowCloseRequested;
    use std::sync::mpsc::{channel, Sender};
    use std::thread::JoinHandle;

    #[test]
    fn the_app_quits_once_no_world_is_left_to_save() {
        let s = Duration::from_secs;
        assert_eq!(quit_step(false, false, s(0)), LeaveStep::Stopped);
        assert_eq!(quit_step(true, false, s(1)), LeaveStep::Wait);
        assert_eq!(quit_step(false, true, s(1)), LeaveStep::Wait);
        assert_eq!(quit_step(false, true, s(10)), LeaveStep::GaveUp);
        assert_eq!(quit_step(true, true, s(11)), LeaveStep::GaveUp);
    }

    /// A thread parked until the sender returned is used, as a local server still loading.
    fn parked() -> (Sender<()>, JoinHandle<Result<(), String>>) {
        let (release, wait) = channel::<()>();
        let server = std::thread::spawn(move || {
            let _ = wait.recv();
            Ok(())
        });
        (release, server)
    }

    /// A bare app in the menus with the quit and the stop of a local server left running, which
    /// runs in the menus only, as in the game, whose local server is `server`.
    fn menu_app(server: JoinHandle<Result<(), String>>) -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), StatesPlugin))
            .init_asset::<Font>()
            .init_resource::<InputFocus>()
            .add_message::<WindowCloseRequested>()
            .insert_state(GameState::Menu)
            .insert_resource(LocalServerHandle(Some(server)))
            .add_plugins(quit_plugin)
            .add_systems(
                Update,
                finish_stopping_local_server.run_if(in_state(GameState::Menu)),
            );
        app
    }

    fn cards(app: &mut App) -> usize {
        let world = app.world_mut();
        world
            .query_filtered::<(), With<QuittingCard>>()
            .iter(world)
            .count()
    }

    /// Runs frames of `app` until it asks to exit. Fails the test after 10 s.
    fn update_until_exit(app: &mut App) {
        let start = Instant::now();
        loop {
            app.update();
            if app.should_exit().is_some() {
                return;
            }
            assert!(start.elapsed() < Duration::from_secs(10), "never quit");
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    /// A world cancelled or left earlier still saves in the menus: the app waits for its server,
    /// and exits once it is joined.
    #[test]
    fn quitting_in_the_menus_waits_for_a_local_server_still_saving() {
        let (release, server) = parked();
        let mut app = menu_app(server);
        // a screen focused beneath the card, then a screen focusing its card as it opens
        let button = app.world_mut().spawn_empty().id();
        app.world_mut()
            .resource_mut::<InputFocus>()
            .set(button, FocusCause::Navigated);
        app.world_mut().trigger(QuitRequested);
        assert_eq!(app.world().resource::<InputFocus>().get(), None);
        for _ in 0..3 {
            app.world_mut()
                .resource_mut::<InputFocus>()
                .set(button, FocusCause::Navigated);
            app.update();
            assert!(app.should_exit().is_none());
            assert_eq!(app.world().resource::<InputFocus>().get(), None);
        }
        assert_eq!(cards(&mut app), 1);

        release.send(()).unwrap();
        update_until_exit(&mut app);
        assert!(app.world().resource::<LocalServerHandle>().0.is_none());
    }

    /// A world still starting is cancelled, back to the menus, where its server is waited for.
    #[test]
    fn quitting_while_a_world_starts_cancels_it_then_waits_for_its_server() {
        let (release, server) = parked();
        let mut app = menu_app(server);
        let origin = SessionOrigin::World {
            name: "Slow".to_string(),
        };
        app.insert_resource(origin.clone())
            .world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::PreGameLoading);
        app.update();

        app.world_mut().trigger(QuitRequested);
        for _ in 0..3 {
            app.update();
            assert!(app.should_exit().is_none());
        }
        assert_eq!(
            *app.world().resource::<State<GameState>>().get(),
            GameState::Menu
        );
        let end = app.world().resource::<SessionEnd>();
        assert_eq!((&end.origin, &end.error), (&origin, &None));
        assert_eq!(cards(&mut app), 1);

        release.send(()).unwrap();
        update_until_exit(&mut app);
        assert!(app.world().resource::<LocalServerHandle>().0.is_none());
    }

    /// A server session joined after a world cancelled before its server answered: that
    /// server, never told to stop, is told from the menus the server session is left for, and
    /// the app exits once it is joined.
    #[test]
    fn quitting_a_server_session_tells_a_local_server_left_running_to_stop() {
        let (release, server) = parked();
        let mut app = menu_app(server);
        let origin = SessionOrigin::Server {
            name: "Friends".to_string(),
            ip: "127.0.0.1".to_string(),
            address: "127.0.0.1:8000".parse().unwrap(),
        };
        app.insert_resource(origin.clone())
            .world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Game);
        app.update();

        app.world_mut().trigger(QuitRequested);
        for _ in 0..3 {
            app.update();
            assert!(app.should_exit().is_none());
        }
        assert_eq!(
            *app.world().resource::<State<GameState>>().get(),
            GameState::Menu
        );
        let end = app.world().resource::<SessionEnd>();
        assert_eq!((&end.origin, &end.error), (&origin, &None));
        assert_eq!(cards(&mut app), 1);

        release.send(()).unwrap();
        update_until_exit(&mut app);
        assert!(app.world().resource::<LocalServerHandle>().0.is_none());
    }

    /// A server whose thread finished is joined before the app exits, which logs why it stopped
    /// when it failed: the app waits for the menus to join it.
    #[test]
    fn the_app_exits_only_once_the_local_server_is_joined() {
        let server = std::thread::spawn(|| Ok(()));
        while !server.is_finished() {
            std::thread::sleep(Duration::from_millis(1));
        }
        let mut app = menu_app(server);
        // out of the menus, where nothing joins it
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Game);
        app.update();
        app.insert_resource(Quitting {
            since: Instant::now(),
        });
        app.update();
        assert!(app.should_exit().is_none());

        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Menu);
        update_until_exit(&mut app);
        assert!(app.world().resource::<LocalServerHandle>().0.is_none());
    }

    /// With no local server running, a server session has nothing to save: the app exits at once.
    #[test]
    fn quitting_a_server_session_without_a_local_server_quits_at_once() {
        let mut app = menu_app(std::thread::spawn(|| Ok(())));
        app.insert_resource(LocalServerHandle(None))
            .insert_resource(SessionOrigin::Server {
                name: "Friends".to_string(),
                ip: "127.0.0.1".to_string(),
                address: "127.0.0.1:8000".parse().unwrap(),
            })
            .world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Game);
        app.update();

        app.world_mut().trigger(QuitRequested);
        app.update();
        assert!(app.should_exit().is_some());
        assert_eq!(cards(&mut app), 0);
    }

    #[test]
    fn a_second_close_quits_at_once() {
        let (release, server) = parked();
        let mut app = menu_app(server);
        app.world_mut().insert_resource(Quitting {
            since: Instant::now(),
        });
        app.world_mut().trigger(QuitRequested);
        app.update();
        assert!(app.should_exit().is_some());
        assert_eq!(cards(&mut app), 0);
        release.send(()).unwrap();
    }
}
