//! The connecting card: its steps and progress, Cancel and Escape back to the list.

use accesskit::Role;
use std::time::Duration;

use bevy::input::keyboard::KeyCode;
use bevy::prelude::*;
use bevy::text::FontSource;

use super::harness::{Harness, Options, SCREEN, STARTUP};
use super::locator::{is_inside, Locator as L};
use super::multi::selected_server;
use super::seed::GameFolder;
use super::solo::selected_world;
use crate::state::GameState;
use crate::ui::menus::connecting::ConnectingCard;
use crate::ui::menus::MenuState;

/// A game folder with the server `Nowhere`, on the discard port, where nothing answers.
fn nowhere() -> GameFolder {
    GameFolder::new().with_servers(&[("Nowhere", "127.0.0.1:9")])
}

/// A client joining `Nowhere` from the server list, on the connecting card.
#[track_caller]
fn join_nowhere() -> Harness {
    let mut h = Harness::start(nowhere(), Options::default());
    h.wait_for(&L::text("Multiplayer"), STARTUP);
    h.click(&L::text("Multiplayer"));
    h.join_server_card("Nowhere");
    h.wait_for_game_state(GameState::PreGameLoading, SCREEN);
    h
}

/// The title `title` of the connecting card.
fn card_title(title: &str) -> L {
    L::text(title).within(L::marker::<ConnectingCard>())
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn the_connecting_card_is_drawn_on_its_first_frame() {
    let mut h = join_nowhere();
    h.frames(1);
    assert!(h.is_visible(&L::role(Role::Button, "Cancel")));
    assert!(h.is_visible(&card_title("Joining \"Nowhere\"")));
    // laid out once: no font still loading would move its text later, on the card or in the
    // hint bar
    let world = h.world_mut();
    let screens: Vec<Entity> = world
        .query::<(Entity, &DespawnOnExit<GameState>)>()
        .iter(world)
        .filter(|(_, exit)| exit.0 == GameState::PreGameLoading)
        .map(|(entity, _)| entity)
        .collect();
    let fonts: Vec<Handle<Font>> = world
        .query::<(Entity, &TextFont)>()
        .iter(world)
        .filter(|(entity, _)| {
            screens
                .iter()
                .any(|&screen| is_inside(world, *entity, screen))
        })
        .map(|(_, font)| match &font.font {
            FontSource::Handle(handle) => handle.clone(),
            other => panic!("a font that is not an asset: {other:?}"),
        })
        .collect();
    assert!(!fonts.is_empty());
    let assets = world.resource::<AssetServer>();
    for font in fonts {
        assert!(assets.is_loaded(&font), "{:?}", font.path());
    }
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn the_connecting_card_shows_its_steps_and_cancel_returns_to_the_server() {
    let mut h = join_nowhere();
    for text in ["127.0.0.1:9", "Connecting", "Loading chunks", "Spawning"] {
        h.wait_for(&L::text(text).within(L::marker::<ConnectingCard>()), SCREEN);
    }
    assert!(h.is_visible(&L::role(Role::ProgressIndicator, "Progress")));
    assert_eq!(h.focus_label().as_deref(), Some("Cancel"));

    h.click(&L::role(Role::Button, "Cancel"));
    h.wait_for_game_state(GameState::Menu, SCREEN);
    h.frames(2);
    assert_eq!(h.menu_state(), MenuState::Multi);
    assert_eq!(selected_server(&mut h).as_deref(), Some("Nowhere"));
    assert_eq!(h.focus_label().as_deref(), Some("Nowhere"));
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn escape_cancels_the_connection_and_returns_to_the_server() {
    let mut h = join_nowhere();
    h.wait_for(&card_title("Joining \"Nowhere\""), SCREEN);
    h.press(KeyCode::Escape);
    h.wait_for_game_state(GameState::Menu, SCREEN);
    h.frames(2);
    assert_eq!(h.menu_state(), MenuState::Multi);
    assert_eq!(selected_server(&mut h).as_deref(), Some("Nowhere"));
    // the Escape that cancelled does not also leave the list
    h.frames(3);
    assert_eq!(h.menu_state(), MenuState::Multi);
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn cancel_returns_to_the_world_it_was_opening() {
    // `Survie` is the second card: the list does not focus it by default
    let hour = Duration::from_secs(60 * 60);
    let folder = GameFolder::new()
        .with_played_world("Recent", 0, hour)
        .with_played_world("Survie", 0, 24 * hour);
    let mut h = Harness::start(folder, Options::default());
    h.wait_for(&L::text("Singleplayer"), STARTUP);
    h.click(&L::text("Singleplayer"));
    let survie = L::role(Role::ListItem, "Survie");
    h.wait_for(&survie, SCREEN);
    h.click(&survie);
    h.click(&L::role(Role::Button, "Play"));
    h.wait_for_game_state(GameState::PreGameLoading, SCREEN);
    assert!(h.is_visible(&card_title("Opening \"Survie\"")));
    assert!(h.is_visible(&L::text("Starting the world").within(L::marker::<ConnectingCard>())));

    h.click(&L::role(Role::Button, "Cancel"));
    h.wait_for_game_state(GameState::Menu, SCREEN);
    h.frames(2);
    assert_eq!(h.menu_state(), MenuState::Solo);
    assert_eq!(selected_world(&mut h).as_deref(), Some("Survie"));
    assert_eq!(h.focus_label().as_deref(), Some("Survie"));
}
