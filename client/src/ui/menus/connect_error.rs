//! The error card: why a session failed and what to do, over the dirt of the menus, with Try
//! again (focused) and the way back to the list the session came from.
//!
//! The session is already torn down when the card shows: it is a screen of the menus,
//! `MenuState::ConnectError`, which `first_menu` opens on a `SessionEnd` with an error. Back
//! keeps that end, so that the list focuses its card.

use crate::i18n::{tr, trf};
use bevy::{
    input_focus::{InputFocus, InputFocusVisible},
    prelude::*,
};

use super::connecting::CARD_WIDTH;
use super::multi::join_server;
use super::servers::ServerItem;
use super::solo::SelectedWorld;
use super::worlds::start_world;
use super::MenuState;
use crate::network::session::{ConnectError, SessionEnd, SessionOrigin};
use crate::network::TargetServer;
use crate::state::GameState;
use crate::ui::assets::FONT_PATH;
use crate::ui::menus::background::menu_background;
use crate::ui::widgets::button::ButtonVariant;
use crate::ui::widgets::dialog::{set_dialog_width, spawn_dialog_with_hints, Dialog, DialogClosed};
use crate::ui::widgets::hints::Hint;

/// The primary action of the card, which opens the session again.
pub const TRY_AGAIN: &str = "connect.try_again";

/// The index of Try again among the actions of the card: after Back.
const TRY_AGAIN_ACTION: usize = 1;

/// The key hints of the card; without `Tab Move` when the card has a single action.
pub const ERROR_HINTS: [Hint; 3] = [
    Hint {
        keys: "Tab",
        action: "Move",
    },
    Hint {
        keys: "Enter",
        action: "Select",
    },
    Hint {
        keys: "Esc",
        action: "Back",
    },
];

/// Whether trying again may succeed: not with a name no server accepts, nor on a server of
/// another version, nor with a world its server stopped on (an unreadable, corrupt or newer
/// world file), which stops it the same way again. A card that cannot succeed offers only the
/// way back.
pub fn can_try_again(error: &ConnectError) -> bool {
    !matches!(
        error,
        ConnectError::InvalidName { .. }
            | ConnectError::Incompatible
            | ConnectError::WorldFailed(_)
    )
}

/// The key hints of a card with Try again (`retry`), or with the way back only, where Tab has
/// nowhere to go.
fn card_hints(retry: bool) -> &'static [Hint] {
    if retry {
        &ERROR_HINTS
    } else {
        &ERROR_HINTS[1..]
    }
}

/// The action back to the list the session came from.
pub fn back_label(origin: &SessionOrigin) -> &'static str {
    if origin.is_world() {
        tr("connect.back_worlds")
    } else {
        tr("connect.back_servers")
    }
}

/// The title and the body of the error card (Decision 7).
pub fn error_text(error: &ConnectError, origin: &SessionOrigin) -> (String, String) {
    let name = origin.name();
    let world = origin.is_world();
    let (title, body) = match error {
        ConnectError::NoAnswer => match origin {
            SessionOrigin::Server { ip, .. } => (
                trf("connect.cannot_reach", &[("name", &name)]),
                format!(
                    "No answer from {ip}. Check that the server is running and that its port \
                     is open, then try again."
                ),
            ),
            SessionOrigin::World { .. } => (
                trf("connect.cannot_open", &[("name", &name)]),
                tr("connect.open_timeout").to_string(),
            ),
        },
        ConnectError::Incompatible => (
            trf("connect.incompatible", &[("name", &name)]),
            tr("connect.incompatible.body").to_string(),
        ),
        ConnectError::Full => (
            trf("connect.full", &[("name", &name)]),
            tr("connect.full.body").to_string(),
        ),
        ConnectError::Refused { name: player } => (
            trf("connect.refused", &[("name", &name)]),
            trf("connect.refused.body", &[("player", &player)]),
        ),
        ConnectError::InvalidName { .. } => (
            tr("connect.invalid_name").to_string(),
            tr("connect.invalid_name.body").to_string(),
        ),
        ConnectError::LostWhileLoading => (
            trf("connect.lost", &[("name", &name)]),
            if world {
                tr("connect.lost_loading.world")
            } else {
                tr("connect.lost_loading.server")
            }
            .to_string(),
        ),
        ConnectError::WorldFailed(reason) => (
            trf("connect.cannot_open", &[("name", &name)]),
            trf("connect.load_failed", &[("reason", &reason)]),
        ),
        ConnectError::WorldBusy => (
            trf("connect.cannot_open", &[("name", &name)]),
            tr("connect.still_closing").to_string(),
        ),
        ConnectError::NoConnection(reason) => (
            tr("connect.cannot_connect").to_string(),
            trf("connect.no_network", &[("reason", &reason)]),
        ),
        ConnectError::ServerStopped => (
            trf("connect.stopped", &[("name", &name)]),
            if world {
                tr("connect.stopped.world")
            } else {
                tr("connect.stopped.server")
            }
            .to_string(),
        ),
        ConnectError::Dismissed(reason) => {
            // the reason of the server, in the language of the game when it is known
            let reason =
                crate::i18n::translate_server_answer(reason).unwrap_or_else(|| reason.clone());
            (trf("connect.dismissed", &[("name", &name)]), reason)
        }
        ConnectError::ConnectionLost => (
            trf("connect.lost", &[("name", &name)]),
            if world {
                tr("connect.lost.world")
            } else {
                tr("connect.lost.server")
            }
            .to_string(),
        ),
    };
    (title, body)
}

pub fn connect_error_plugin(app: &mut App) {
    app.add_systems(OnEnter(MenuState::ConnectError), connect_error_setup);
}

/// Spawns the card of the session's error over the dirt, its last action focused with its ring
/// (Try again, else the way back): the card was not opened by a click, and Enter acts on it.
fn connect_error_setup(
    end: Option<Res<SessionEnd>>,
    assets: Res<AssetServer>,
    focus: Res<InputFocus>,
    mut focus_visible: ResMut<InputFocusVisible>,
    mut menu: ResMut<NextState<MenuState>>,
    mut commands: Commands,
) {
    let Some(SessionEnd {
        origin,
        error: Some(error),
    }) = end.as_deref()
    else {
        error!("No error to show, back to the main menu");
        menu.set(MenuState::Main);
        return;
    };
    let root = commands
        .spawn((
            menu_background(&assets),
            DespawnOnExit(MenuState::ConnectError),
        ))
        .id();
    let (title, body) = error_text(error, origin);
    let retry = can_try_again(error);
    let back = (back_label(origin), ButtonVariant::Default);
    let actions = [back, (crate::i18n::tr(TRY_AGAIN), ButtonVariant::Primary)];
    let actions = if retry { &actions[..] } else { &actions[..1] };
    let dialog = Dialog {
        title: &title,
        body: &body,
        actions,
        focused: actions.len() - 1,
    };
    let card = spawn_dialog_with_hints(
        &mut commands,
        &dialog,
        card_hints(retry),
        assets.load(FONT_PATH),
        &focus,
    );
    // as wide as the connecting card it takes the place of: the body wraps on a few lines
    set_dialog_width(&mut commands, card, CARD_WIDTH);
    commands
        .entity(card)
        .insert(ChildOf(root))
        .observe(act_on_error_card);
    focus_visible.0 = true;
}

/// Try again opens the session again, as it was started; Back and Escape go to the list it came
/// from, which focuses its card.
fn act_on_error_card(
    closed: On<DialogClosed>,
    end: Option<Res<SessionEnd>>,
    mut selected: ResMut<SelectedWorld>,
    mut target: ResMut<TargetServer>,
    mut game: ResMut<NextState<GameState>>,
    mut menu: ResMut<NextState<MenuState>>,
    mut commands: Commands,
) {
    let Some(end) = end else {
        return;
    };
    if closed.action != Some(TRY_AGAIN_ACTION) {
        menu.set(if end.origin.is_world() {
            MenuState::Solo
        } else {
            MenuState::Multi
        });
        return;
    }
    // the game already leaves the menus this frame: the session is not opened again, and its
    // end stays for the screen that opens
    if !matches!(*game, NextState::Unchanged) {
        return;
    }
    commands.remove_resource::<SessionEnd>();
    match &end.origin {
        SessionOrigin::World { name } => {
            start_world(name, &mut selected, &mut commands, &mut game, &mut menu);
        }
        SessionOrigin::Server { name, ip, address } => {
            let server = ServerItem {
                name: name.clone(),
                ip: ip.clone(),
            };
            join_server(
                &server,
                *address,
                &mut target,
                &mut commands,
                &mut game,
                &mut menu,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn server() -> SessionOrigin {
        SessionOrigin::Server {
            name: "Nowhere".into(),
            ip: "127.0.0.1:9".into(),
            address: "127.0.0.1:9".parse().unwrap(),
        }
    }

    fn world() -> SessionOrigin {
        SessionOrigin::World {
            name: "Survie".into(),
        }
    }

    #[track_caller]
    fn says(error: ConnectError, origin: &SessionOrigin, title: &str, body: &str) {
        assert_eq!(
            error_text(&error, origin),
            (title.to_string(), body.to_string()),
            "{error:?}"
        );
    }

    #[test]
    fn only_a_failure_that_may_pass_offers_try_again() {
        for error in [
            ConnectError::InvalidName { name: "".into() },
            ConnectError::Incompatible,
            ConnectError::WorldFailed("bad ron".into()),
        ] {
            assert!(!can_try_again(&error), "{error:?}");
        }
        for error in [
            ConnectError::NoAnswer,
            ConnectError::Full,
            ConnectError::Refused {
                name: "Tester".into(),
            },
            ConnectError::LostWhileLoading,
            // the last session's world closes in a moment
            ConnectError::WorldBusy,
            ConnectError::NoConnection("no socket".into()),
            ConnectError::ServerStopped,
            ConnectError::ConnectionLost,
            // an operator may pardon the player, or put it on the whitelist
            ConnectError::Dismissed("You are banned from this server.".into()),
        ] {
            assert!(can_try_again(&error), "{error:?}");
        }
        let keys = |retry| {
            card_hints(retry)
                .iter()
                .map(|hint| hint.keys)
                .collect::<Vec<_>>()
        };
        assert_eq!(keys(true), ["Tab", "Enter", "Esc"]);
        assert_eq!(keys(false), ["Enter", "Esc"]);
    }

    #[test]
    fn a_try_again_that_stands_down_keeps_the_error() {
        let mut app = App::new();
        app.init_resource::<NextState<GameState>>()
            .init_resource::<NextState<MenuState>>()
            .init_resource::<SelectedWorld>()
            .insert_resource(TargetServer {
                address: None,
                username: None,
                session_token: None,
                state: crate::network::TargetServerState::Initial,
            })
            .insert_resource(SessionEnd {
                origin: server(),
                error: Some(ConnectError::NoAnswer),
            });
        // the game already leaves the menus this frame
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Menu);
        let card = app
            .world_mut()
            .spawn_empty()
            .observe(act_on_error_card)
            .id();
        app.world_mut().trigger(DialogClosed {
            entity: card,
            action: Some(TRY_AGAIN_ACTION),
        });
        app.world_mut().flush();
        assert!(app.world().contains_resource::<SessionEnd>());
        assert!(matches!(
            app.world().resource::<NextState<MenuState>>(),
            NextState::Unchanged
        ));
    }

    #[test]
    fn every_error_says_what_happened_and_what_to_do() {
        let server = server();
        says(
            ConnectError::NoAnswer,
            &server,
            "Cannot reach \"Nowhere\"",
            "No answer from 127.0.0.1:9. Check that the server is running and that its port is \
             open, then try again.",
        );
        says(
            ConnectError::Incompatible,
            &server,
            "\"Nowhere\" runs another version",
            "This server runs another version of Rustcraft. Join it with the same version as the \
             server.",
        );
        says(
            ConnectError::Full,
            &server,
            "\"Nowhere\" is full",
            "The server has no room for another player. Try again later.",
        );
        says(
            ConnectError::Refused {
                name: "Tester".into(),
            },
            &server,
            "\"Nowhere\" refused you",
            "The server closed the connection before letting you in. Another player may already \
             be using the name \"Tester\".",
        );
        let invalid_name = "Player names have 1 to 32 characters and no control characters. \
                            Change yours on the main menu.";
        for origin in [&server, &world()] {
            says(
                ConnectError::InvalidName { name: "".into() },
                origin,
                "Your name is not accepted",
                invalid_name,
            );
        }
        says(
            ConnectError::LostWhileLoading,
            &server,
            "Connection to \"Nowhere\" lost",
            "The server stopped answering while the world was loading.",
        );
        says(
            ConnectError::Dismissed("You were kicked from the server: too loud".into()),
            &server,
            "Disconnected from \"Nowhere\"",
            "You were kicked from the server: too loud",
        );
        says(
            ConnectError::NoConnection("no socket".into()),
            &server,
            "Cannot connect",
            "Could not open a network connection: no socket",
        );
        says(
            ConnectError::ServerStopped,
            &server,
            "\"Nowhere\" stopped",
            "The server shut down or closed your connection.",
        );
        says(
            ConnectError::ConnectionLost,
            &server,
            "Connection to \"Nowhere\" lost",
            "The server stopped answering. Check your network, then try again.",
        );

        let world = world();
        says(
            ConnectError::LostWhileLoading,
            &world,
            "Connection to \"Survie\" lost",
            "The world stopped while loading.",
        );
        says(
            ConnectError::WorldFailed("bad ron".into()),
            &world,
            "Cannot open \"Survie\"",
            "The world could not be loaded: bad ron",
        );
        says(
            ConnectError::WorldBusy,
            &world,
            "Cannot open \"Survie\"",
            "The last world is still closing. Try again in a moment.",
        );
        says(
            ConnectError::NoConnection("no socket".into()),
            &world,
            "Cannot connect",
            "Could not open a network connection: no socket",
        );
        says(
            ConnectError::ServerStopped,
            &world,
            "\"Survie\" stopped",
            "The world stopped unexpectedly.",
        );
        says(
            ConnectError::ConnectionLost,
            &world,
            "Connection to \"Survie\" lost",
            "The world stopped answering.",
        );

        assert_eq!(back_label(&world), "Back to Worlds");
        assert_eq!(back_label(&server), "Back to Servers");
    }
}
