//! The connecting card: its steps and progress, Cancel and Escape back to the list; the error
//! card of a session that failed, Try again and Back.

use accesskit::Role;
use std::fs;
use std::time::{Duration, Instant};

use bevy::input::keyboard::KeyCode;
use bevy::prelude::*;
use bevy::text::{FontSource, TextLayoutInfo};
use bevy::ui::InteractionDisabled;
use bevy_renet::netcode::NetcodeClientTransport;

use shared::players::Player;
use shared::world::world_position_to_chunk_position;

use super::harness::{Harness, Options, SCREEN, SERVER, STARTUP};
use super::helpers::is_in;
use super::locator::{is_inside, Locator as L};
use super::multi::selected_server;
use super::seed::GameFolder;
use super::solo::selected_world;
use crate::constants::SAVE_PATH;
use crate::network::session::{
    chunks_around, ConnectStep, ConnectTimeout, ConnectWatch, CONNECT_TIMEOUT,
};
use crate::network::status::{ServerStatus, StatusProbe};
use crate::player::CurrentPlayerMarker;
use crate::state::GameState;
use crate::ui::menus::connect_error::TRY_AGAIN;
use crate::ui::menus::connecting::ConnectingCard;
use crate::ui::menus::MenuState;
use crate::ui::widgets::dialog::DialogPanel;
use crate::world::ClientWorldMap;

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

/// The error card titled `title`.
fn error_card(title: &str) -> L {
    L::role(Role::Dialog, title)
}

/// The title `title` of the connecting card.
fn card_title(title: &str) -> L {
    L::text(title).within(L::marker::<ConnectingCard>())
}

/// The chunk of the local player.
#[track_caller]
fn player_chunk(h: &mut Harness) -> IVec3 {
    let world = h.world_mut();
    let player = world
        .query_filtered::<&Transform, With<CurrentPlayerMarker>>()
        .single(world)
        .unwrap();
    world_position_to_chunk_position(player.translation)
}

/// The chunks within one chunk of `center`.
fn cube(center: IVec3) -> impl Iterator<Item = IVec3> {
    (-1..=1).flat_map(move |x| {
        (-1..=1).flat_map(move |y| (-1..=1).map(move |z| center + IVec3::new(x, y, z)))
    })
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

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn a_world_opens_once_the_chunks_around_the_player_are_loaded() {
    let mut h = Harness::start(GameFolder::new().with_world("Survie"), Options::default());
    h.wait_for(&L::text("Singleplayer"), STARTUP);
    h.click(&L::text("Singleplayer"));
    h.play_world("Survie");
    h.wait_for_game_state(GameState::PreGameLoading, SCREEN);
    assert!(h.is_visible(&card_title("Opening \"Survie\"")));
    assert!(h.is_visible(&L::text("Starting the world").within(L::marker::<ConnectingCard>())));

    // the first frame of the game
    h.wait_until("the game", SERVER, |w| is_in(w, GameState::Game));
    let center = player_chunk(&mut h);
    let blocks = &h.world().resource::<ClientWorldMap>().blocks;
    assert_eq!(chunks_around(blocks, center, 1), (27, 27));
    h.wait_until("a mesh around the player", SCREEN, |w| {
        let map = w.resource::<ClientWorldMap>();
        cube(center).any(|pos| map.render.get(&pos).is_some_and(|r| r.entity.is_some()))
    });
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn a_world_cancelled_while_loading_chunks_opens_again_from_nothing() {
    let mut h = Harness::start(GameFolder::new().with_world("Survie"), Options::default());
    h.wait_for(&L::text("Singleplayer"), STARTUP);
    h.click(&L::text("Singleplayer"));
    h.play_world("Survie");
    h.wait_until("Loading chunks", SERVER, |w| {
        w.get_resource::<ConnectWatch>()
            .is_some_and(|watch| matches!(watch.step, ConnectStep::LoadingChunks { .. }))
    });
    // Escape cancels on the frame it is pressed, before the game can be entered: a click takes
    // a frame more
    h.press(KeyCode::Escape);
    h.wait_for_game_state(GameState::Menu, SCREEN);
    let map = h.world().resource::<ClientWorldMap>();
    assert_eq!(map.blocks.chunk_positions().count(), 0);
    assert!(map.render.is_empty());
    let world = h.world_mut();
    assert_eq!(
        world.query::<&Player>().iter(world).count(),
        0,
        "a player left over"
    );
    h.frames(2);
    assert_eq!(h.menu_state(), MenuState::Solo);

    h.play_world("Survie");
    h.wait_for_game_state(GameState::PreGameLoading, SCREEN);
    h.wait_until("the game", SERVER, |w| is_in(w, GameState::Game));
    let center = player_chunk(&mut h);
    let blocks = &h.world().resource::<ClientWorldMap>().blocks;
    assert_eq!(chunks_around(blocks, center, 1), (27, 27));
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn a_server_that_never_answers_times_out_then_says_it_runs_another_version() {
    let mut h = Harness::start(nowhere(), Options::default());
    h.wait_for(&L::text("Multiplayer"), STARTUP);
    h.click(&L::text("Multiplayer"));
    // offline, as an old server without a status port reads: it may still be joined
    h.wait_for(
        &L::text("Offline").within(L::role(Role::ListItem, "Nowhere")),
        SERVER,
    );
    let join = h.get(&L::role(Role::Button, "Join"));
    assert!(h.world().get::<InteractionDisabled>(join).is_none());

    let t0 = Instant::now();
    h.join_server_card("Nowhere");
    let no_answer = error_card("Cannot reach \"Nowhere\"");
    h.wait_for(&no_answer, SERVER);
    assert!(t0.elapsed() >= CONNECT_TIMEOUT, "{:?}", t0.elapsed());
    assert!(h.is_visible(&L::text_containing("127.0.0.1:9").within(no_answer)));
    assert_eq!(h.focus_label().as_deref(), Some(TRY_AGAIN));
    assert_eq!(h.menu_state(), MenuState::ConnectError);
    assert_eq!(h.game_state(), GameState::Menu);

    // the server answers its status now, in another version; the real timeout was waited once
    h.world_mut()
        .insert_resource(StatusProbe(|_| ServerStatus::Incompatible));
    h.world_mut()
        .insert_resource(ConnectTimeout(Duration::from_millis(500)));
    h.press(KeyCode::Enter);
    h.wait_for(&card_title("Joining \"Nowhere\""), SCREEN);
    let incompatible = error_card("\"Nowhere\" runs another version");
    h.wait_for(&incompatible, SERVER);
    // trying again cannot help: only the way back is offered
    h.frames(1);
    assert!(!h.is_visible(&L::text(TRY_AGAIN).within(incompatible)));
    assert_eq!(h.focus_label().as_deref(), Some("Back to Servers"));

    h.press(KeyCode::Escape);
    h.wait_for_game_state(GameState::Menu, SCREEN);
    h.wait_until("the server list", SCREEN, |w| {
        *w.resource::<State<MenuState>>().get() == MenuState::Multi
    });
    h.frames(2);
    assert_eq!(h.menu_state(), MenuState::Multi);
    assert_eq!(selected_server(&mut h).as_deref(), Some("Nowhere"));
    assert_eq!(h.focus_label().as_deref(), Some("Nowhere"));
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn an_invalid_player_name_is_said_before_connecting() {
    let options = Options {
        player_name: "".into(),
        ..default()
    };
    let mut h = Harness::start(nowhere(), options);
    h.wait_for(&L::text("Multiplayer"), STARTUP);
    h.click(&L::text("Multiplayer"));
    h.join_server_card("Nowhere");
    let invalid_name = error_card("Your name is not accepted");
    h.wait_for(&invalid_name, SCREEN);
    // no packet left: no connection was opened
    assert!(!h.world().contains_resource::<NetcodeClientTransport>());
    // trying again cannot help: only the way back is offered
    h.frames(1);
    assert!(!h.is_visible(&L::text(TRY_AGAIN).within(invalid_name)));
    assert_eq!(h.focus_label().as_deref(), Some("Back to Servers"));

    h.hover_at(Vec2::ZERO);
    h.assert_screenshot("connect_error");

    h.click(&L::role(Role::Button, "Back to Servers"));
    h.wait_until("the server list", SCREEN, |w| {
        *w.resource::<State<MenuState>>().get() == MenuState::Multi
    });
    h.frames(2);
    assert_eq!(h.menu_state(), MenuState::Multi);
    assert_eq!(selected_server(&mut h).as_deref(), Some("Nowhere"));
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn a_world_that_cannot_load_says_why_and_returns_to_its_card() {
    let folder = GameFolder::new()
        .with_world("Alpha")
        .with_broken_world("Broken");
    let mut h = Harness::start(folder, Options::default());
    h.wait_for(&L::text("Singleplayer"), STARTUP);
    h.click(&L::text("Singleplayer"));
    h.play_world("Broken");
    let title = "Cannot open \"Broken\"";
    h.wait_for(&error_card(title), SERVER);
    assert!(h.is_visible(&L::text_containing("could not be loaded").within(error_card(title))));
    // opening the world again would stop the same way: only the way back is offered
    h.frames(1);
    assert!(!h.is_visible(&L::text(TRY_AGAIN).within(error_card(title))));
    assert_eq!(h.focus_label().as_deref(), Some("Back to Worlds"));

    h.press(KeyCode::Escape);
    h.wait_until("the world list", SCREEN, |w| {
        *w.resource::<State<MenuState>>().get() == MenuState::Solo
    });
    h.frames(2);
    assert_eq!(selected_world(&mut h).as_deref(), Some("Broken"));

    // the server that stopped was waited for: the world is not said to be still closing
    h.play_world("Broken");
    h.wait_for(&error_card(title), SERVER);
    assert!(h.is_visible(&L::text_containing("could not be loaded").within(error_card(title))));
    assert!(!h.is_visible(&L::text_containing("still closing")));

    // the world is deleted while its card shows: the list focuses another one
    fs::remove_dir_all(h.folder().join(SAVE_PATH).join("Broken")).unwrap();
    h.press(KeyCode::Enter);
    h.wait_until("the world list", SCREEN, |w| {
        *w.resource::<State<MenuState>>().get() == MenuState::Solo
    });
    h.frames(2);
    assert_eq!(selected_world(&mut h).as_deref(), Some("Alpha"));
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn a_long_error_stays_inside_its_card_at_800x600() {
    let name = "W".repeat(32);
    let options = Options {
        width: 800,
        height: 600,
        ..default()
    };
    let mut h = Harness::start(GameFolder::new().with_broken_world(&name), options);
    h.wait_for(&L::text("Singleplayer"), STARTUP);
    h.click(&L::text("Singleplayer"));
    h.play_world(&name);
    let title = format!("Cannot open \"{name}\"");
    h.wait_for(&error_card(&title), SERVER);
    h.frames(1);
    let panel = L::marker::<DialogPanel>().within(error_card(&title));
    let panel_rect = h.rect_of(&panel);
    let panel = h.get(&panel);
    let room = h
        .world()
        .get::<ComputedNode>(panel)
        .unwrap()
        .content_box()
        .width();
    for text in [
        L::text(&title).within(error_card(&title)),
        L::text_containing("could not be loaded").within(error_card(&title)),
    ] {
        let rect = h.rect_of(&text);
        assert!(
            panel_rect.contains(rect.min) && panel_rect.contains(rect.max),
            "{text:?} at {rect:?} leaves the card at {panel_rect:?}"
        );
        // the glyphs, not the node, which the dialog keeps inside itself whatever its text
        let entity = h.get(&text);
        let glyphs = h.world().get::<TextLayoutInfo>(entity).unwrap().size.x;
        assert!(
            glyphs <= room,
            "{text:?} is {glyphs} px wide in a card of {room} px"
        );
    }
}
