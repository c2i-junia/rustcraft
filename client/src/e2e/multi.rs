//! The server list: the server cards with their status and the detail pane, adding, removing
//! and joining servers.

use std::collections::HashSet;
use std::fs;
use std::io;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Mutex;
use std::thread;
use std::time::{Duration, Instant};

use accesskit::Role;
use bevy::{
    ecs::system::SystemState,
    input::keyboard::KeyCode,
    input_focus::InputFocus,
    math::{Rect, Vec2},
    prelude::{BackgroundColor, Children, Component, Entity, Query, With},
    ui::{InteractionDisabled, Selected},
};
use shared::{GameFolderPaths, GameServerConfig};

use super::harness::{Harness, Options, SCREEN, SERVER, STARTUP};
use super::helpers::{ctrl, is_in, main_menu_with};
use super::locator::Locator as L;
use super::seed::{fixed_status, slow_status, GameFolder};
use crate::constants::SERVER_LIST_SAVE_NAME;
use crate::network::address::{
    AddressResolver, ServerAddress, EMPTY_ADDRESS, INVALID_ADDRESS, INVALID_PORT,
};
use crate::network::status::{ServerStatus, StatusProbe};
use crate::network::TargetServer;
use crate::state::GameState;
use crate::ui::menus::multi::dialogs::{ServerAddressInput, ServerNameInput, ADD_SERVER_TITLE};
use crate::ui::menus::multi::status::LatencyBars;
use crate::ui::menus::multi::{ServerCard, ServerDetail, ServerList, SERVERS_TITLE};
use crate::ui::menus::servers::{
    read_servers, ServerItem, EMPTY_SERVER_NAME, SERVER_EXISTS, SERVER_NAME_EXISTS,
};
use crate::ui::menus::MenuState;
use crate::ui::theme::RAISE_HOVER;
use crate::ui::widgets::dialog::DialogRoot;
use crate::ui::widgets::hints::HintBar;
use crate::ui::widgets::list::cards_of;

/// A client on the server list, with the saved servers `localhost` and `Friends`, which answer
/// as `fixed_status` says.
fn server_list() -> Harness {
    open_server_list_with(
        GameFolder::new().with_servers(&[
            ("localhost", "127.0.0.1:8000"),
            ("Friends", "192.168.1.20:8000"),
        ]),
        StatusProbe(fixed_status),
    )
}

/// A client on the server list of `with_status_servers`, every server answered.
fn status_list() -> Harness {
    let mut h = open_server_list_with(
        GameFolder::new().with_status_servers(),
        StatusProbe(fixed_status),
    );
    h.wait_for(&card("Typo"), SCREEN);
    wait_for_answers(&mut h);
    h
}

/// A client on the server list, with the game folder `folder`, which asks the servers for
/// their status with `probe`.
fn open_server_list_with(folder: GameFolder, probe: StatusProbe) -> Harness {
    let mut h = Harness::start(folder, Default::default());
    h.world_mut().insert_resource(probe);
    h.wait_for(&L::text("Multiplayer"), STARTUP);
    h.click(&L::text("Multiplayer"));
    h.wait_for(&L::text(SERVERS_TITLE), SCREEN);
    h
}

/// Runs frames until no card says it is pinging its server.
#[track_caller]
fn wait_for_answers(h: &mut Harness) {
    let start = Instant::now();
    while h.is_visible(&L::text("Pinging...")) {
        assert!(start.elapsed() < SCREEN, "the servers never answered");
        h.frames(1);
    }
}

/// The card of the server `name`.
fn card(name: &str) -> L {
    L::role(Role::ListItem, name)
}

/// The names of the server cards, from the top of the list.
fn card_names(h: &mut Harness) -> Vec<String> {
    let world = h.world_mut();
    let list = world
        .query_filtered::<Entity, With<ServerList>>()
        .single(world)
        .unwrap();
    let mut state = SystemState::<Query<&Children>>::new(world);
    let children = state.get(world).unwrap();
    cards_of(list, &children)
        .into_iter()
        .filter_map(|card| world.get::<ServerCard>(card))
        .map(|card| card.0.name.clone())
        .collect()
}

/// The name of the server of the selected card.
fn selected_server(h: &mut Harness) -> Option<String> {
    let world = h.world_mut();
    let mut selected = world.query_filtered::<&ServerCard, With<Selected>>();
    let names: Vec<String> = selected
        .iter(world)
        .map(|card| card.0.name.clone())
        .collect();
    assert!(names.len() <= 1, "several cards are selected: {names:?}");
    names.into_iter().next()
}

/// The detail pane.
fn pane() -> L {
    L::marker::<ServerDetail>()
}

/// The button `label` of the detail pane.
fn pane_button(label: &str) -> L {
    L::role(Role::Button, label).within(pane())
}

/// Whether the element has `InteractionDisabled`.
fn is_disabled(h: &mut Harness, locator: &L) -> bool {
    let entity = h.get(locator);
    h.world().get::<InteractionDisabled>(entity).is_some()
}

/// The Add server dialog.
fn dialog() -> L {
    L::role(Role::Dialog, ADD_SERVER_TITLE)
}

/// The Remove dialog of the server `name`.
fn remove_dialog(name: &str) -> L {
    L::role(Role::Dialog, &format!("Remove \"{name}\"?"))
}

/// The Add button of the Add server dialog.
fn add_button() -> L {
    L::role(Role::Button, "Add")
}

/// Presses Ctrl+N on the server list, and waits for the Add server dialog.
#[track_caller]
fn add_dialog(h: &mut Harness) {
    ctrl(h, KeyCode::KeyN);
    h.wait_for(&dialog(), SCREEN);
}

/// Whether the Add server dialog is open.
fn dialog_is_open(h: &mut Harness) -> bool {
    h.is_visible(&dialog())
}

/// The saved servers, in file order.
fn saved_servers(h: &Harness) -> Vec<ServerItem> {
    read_servers(&h.folder().join(SERVER_LIST_SAVE_NAME)).expect("a readable server list")
}

/// The names of the saved servers, in file order.
fn saved_names(h: &Harness) -> Vec<String> {
    saved_servers(h).into_iter().map(|s| s.name).collect()
}

/// What `servers.ron` holds, byte for byte.
fn saved_file(h: &Harness) -> String {
    fs::read_to_string(h.folder().join(SERVER_LIST_SAVE_NAME)).expect("a saved server list")
}

/// Presses Backspace `n` times.
fn backspace(h: &mut Harness, n: usize) {
    for _ in 0..n {
        h.press(KeyCode::Backspace);
    }
}

/// Presses Shift+Tab.
fn shift_tab(h: &mut Harness) {
    h.key_down(KeyCode::ShiftLeft);
    h.press(KeyCode::Tab);
    h.key_up(KeyCode::ShiftLeft);
}

/// Removes the server `name` through the detail pane and the dialog.
#[track_caller]
fn remove_through_the_pane(h: &mut Harness, name: &str) {
    h.click(&card(name));
    h.click(&pane_button("Remove"));
    h.wait_for(&remove_dialog(name), SCREEN);
    h.click(&L::role(Role::Button, "Remove").within(remove_dialog(name)));
    h.frames(2);
}

/// The address the client was handed to connect to.
fn target_address(h: &Harness) -> Option<SocketAddr> {
    h.world().resource::<TargetServer>().address
}

impl Harness {
    /// Joins the server `name` from the server list: selects its card, then Join in the pane.
    #[track_caller]
    pub(super) fn join_server_card(&mut self, name: &str) {
        self.wait_for(&card(name), SCREEN);
        self.click(&card(name));
        self.click(&L::role(Role::Button, "Join"));
    }

    /// Whether the focus is on the text field marked with `M`.
    #[track_caller]
    fn has_focus<M: Component>(&mut self) -> bool {
        let field = self
            .world_mut()
            .query_filtered::<Entity, With<M>>()
            .single(self.world())
            .unwrap();
        self.world().resource::<InputFocus>().get() == Some(field)
    }
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn the_server_list_matches_its_golden() {
    let mut h = status_list();
    // The cursor in a corner, so that it hovers nothing
    h.hover_at(Vec2::ZERO);
    h.assert_screenshot("menu_multi");
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn the_empty_server_list_matches_its_golden() {
    let folder = GameFolder::new().with_file(SERVER_LIST_SAVE_NAME, "[]");
    let mut h = open_server_list_with(folder, StatusProbe(fixed_status));
    h.wait_for(&L::text("No servers yet"), SCREEN);
    h.hover_at(Vec2::ZERO);
    h.assert_screenshot("menu_multi_empty");
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn the_servers_are_listed_with_their_status() {
    let mut h = status_list();
    assert_eq!(
        card_names(&mut h),
        ["Survival", "Creative", "Old friends", "Friends", "Typo"]
    );
    let bars = |h: &mut Harness, name: &str| {
        let bars = h.get(&L::marker::<LatencyBars>().within(card(name)));
        h.world().get::<LatencyBars>(bars).unwrap().0
    };
    for text in ["10.0.0.1:8000", "3 / 20"] {
        assert!(
            h.is_visible(&L::text(text).within(card("Survival"))),
            "{text}"
        );
    }
    assert_eq!(bars(&mut h, "Survival"), 4);
    assert!(h.is_visible(&L::text("18 / 20").within(card("Creative"))));
    assert_eq!(bars(&mut h, "Creative"), 2);
    for (name, word) in [
        ("Old friends", "Incompatible"),
        ("Friends", "Offline"),
        ("Typo", "Unknown host"),
    ] {
        assert!(h.is_visible(&L::text(word).within(card(name))), "{name}");
    }
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn the_first_server_is_selected_and_shown_in_the_detail_pane() {
    let mut h = status_list();
    assert_eq!(h.focus_label().as_deref(), Some("Survival"));
    assert_eq!(selected_server(&mut h).as_deref(), Some("Survival"));
    assert!(h.is_visible(&L::text("Survival").within(pane())));
    assert!(h.is_visible(&L::text_containing("3 / 20 players").within(pane())));
    assert!(h.is_visible(&L::text("42 ms").within(pane())));

    h.click(&card("Old friends"));
    h.frames(1);
    assert_eq!(selected_server(&mut h).as_deref(), Some("Old friends"));
    assert!(h.is_visible(&L::text("Incompatible version").within(pane())));
    assert!(is_disabled(&mut h, &pane_button("Join")));
    // Enter on the card of a server of another version joins nothing
    h.press(KeyCode::Enter);
    h.frames(3);
    assert_eq!(h.menu_state(), MenuState::Multi);
    assert_eq!(target_address(&h), None);
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn refresh_pings_every_server_again() {
    let mut h = open_server_list_with(
        GameFolder::new().with_status_servers(),
        StatusProbe(slow_status),
    );
    h.wait_for(&L::text("3 / 20").within(card("Survival")), SCREEN);
    let pinging = |name| L::text("Pinging...").within(card(name));

    // looked at in the frame of the key or the click: an answer comes in a later one at best,
    // however slow the frames are
    h.key_down(KeyCode::F5);
    assert!(h.is_visible(&pinging("Survival")));
    assert!(h.is_visible(&pinging("Friends")));
    h.key_up(KeyCode::F5);
    h.wait_for(&L::text("3 / 20").within(card("Survival")), SCREEN);

    h.click(&L::role(Role::Button, "Refresh"));
    assert!(h.is_visible(&pinging("Survival")));
    assert!(h.is_visible(&pinging("Friends")));
    h.wait_for(&L::text("3 / 20").within(card("Survival")), SCREEN);
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn nothing_listening_shows_offline() {
    let folder = GameFolder::new().with_servers(&[("Nowhere", "127.0.0.1:9")]);
    let mut h = open_server_list_with(folder, StatusProbe::default());
    h.wait_for(
        &L::text("Offline").within(card("Nowhere")),
        Duration::from_secs(5),
    );
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn the_keyboard_alone_adds_a_server_sees_it_online_and_joins() {
    // The server runs on its own thread and is left running when the test ends: a dedicated
    // server has no way to stop. Its folder belongs to the test, which deletes it at the end.
    // The server may then fail to save into it, and only writes an error; it cannot fail the
    // test, which has stopped listening to the server, or keep the process from exiting.
    let server_folder = GameFolder::new();
    let (socket, status) = server::acquire_local_socket_pair(IpAddr::V4(Ipv4Addr::LOCALHOST))
        .expect("the sockets of the server");
    let port = socket.local_addr().unwrap().port();
    let paths = GameFolderPaths {
        game_folder_path: server_folder.path().to_path_buf(),
        assets_folder_path: server_folder.path().join("assets"),
    };
    let config = GameServerConfig {
        world_name: "dedicated".to_string(),
        is_solo: false,
    };
    let (stopped, server_error) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        if let Err(error) = server::init_with_status(socket, status, config, paths) {
            let _ = stopped.send(error);
        }
    });

    let mut h = main_menu_with(GameFolder::new());
    h.press(KeyCode::ArrowDown);
    assert_eq!(h.focus_label().as_deref(), Some("Multiplayer"));
    h.press(KeyCode::Enter);
    h.wait_for(&L::text(SERVERS_TITLE), SCREEN);
    assert_eq!(h.focus_label().as_deref(), Some("localhost"));
    // the Enter that opened the list did not join its first server
    h.frames(3);
    assert_eq!(h.game_state(), GameState::Menu);

    ctrl(&mut h, KeyCode::KeyN);
    h.wait_for(&dialog(), SCREEN);
    h.type_text("Local test");
    h.press(KeyCode::Tab);
    h.type_text(&format!("localhost:{port}"));
    h.press(KeyCode::Enter);
    h.frames(2);
    assert_eq!(h.focus_label().as_deref(), Some("Local test"));
    assert_eq!(saved_names(&h), ["localhost", "Local test"]);
    h.wait_for(
        &L::text("0 / 64").within(card("Local test")),
        Duration::from_secs(5),
    );

    h.press(KeyCode::Enter);
    h.wait_until("the game", SERVER, |w| {
        if let Ok(error) = server_error.try_recv() {
            panic!("the server stopped: {error}");
        }
        is_in(w, GameState::Game)
    });
    let joined = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port);
    assert_eq!(target_address(&h), Some(joined));
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn join_hands_a_host_name_over_with_the_default_port() {
    let folder = GameFolder::new().with_servers(&[("Home", "localhost")]);
    let mut h = open_server_list_with(folder, StatusProbe(fixed_status));
    h.wait_for(&card("Home"), SCREEN);
    h.click(&pane_button("Join"));
    h.wait_for_game_state(GameState::PreGameLoading, SCREEN);
    let default = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 8000);
    assert_eq!(target_address(&h), Some(default));
    // its font loaded, the connecting screen lays out again: [Cancel] moves until then
    h.wait_for(&L::text_containing("Connecting to server"), SCREEN);
    h.click(&L::text("[Cancel]"));
    h.wait_for_game_state(GameState::Menu, SCREEN);
}

/// Whether `gated_resolve` may answer.
static LOOKUP_OPEN: AtomicBool = AtomicBool::new(false);
/// How many times `gated_resolve` has answered.
static LOOKUPS_DONE: AtomicUsize = AtomicUsize::new(0);

/// Resolves every host to this computer, once the test opens `LOOKUP_OPEN`.
fn gated_resolve(address: &ServerAddress) -> io::Result<SocketAddr> {
    while !LOOKUP_OPEN.load(Ordering::SeqCst) {
        thread::sleep(Duration::from_millis(1));
    }
    LOOKUPS_DONE.fetch_add(1, Ordering::SeqCst);
    Ok(SocketAddr::new(
        IpAddr::V4(Ipv4Addr::LOCALHOST),
        address.port,
    ))
}

/// Lets `gated_resolve` answer, waits until it answered for the `done`th time, closes it again,
/// and runs a few frames, in which a join still pending would be handed over.
#[track_caller]
fn finish_the_lookup(h: &mut Harness, done: usize) {
    LOOKUP_OPEN.store(true, Ordering::SeqCst);
    let start = Instant::now();
    while LOOKUPS_DONE.load(Ordering::SeqCst) < done {
        assert!(start.elapsed() < SCREEN, "the lookup never ended");
        thread::sleep(Duration::from_millis(1));
    }
    LOOKUP_OPEN.store(false, Ordering::SeqCst);
    h.frames(5);
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn a_join_still_looking_up_is_dropped_by_another_card_or_a_dialog() {
    let folder =
        GameFolder::new().with_servers(&[("Home", "home.example"), ("Survival", "10.0.0.1:8000")]);
    let mut h = open_server_list_with(folder, StatusProbe(fixed_status));
    h.world_mut()
        .insert_resource(AddressResolver(gated_resolve));
    h.wait_for(&card("Survival"), SCREEN);
    wait_for_answers(&mut h);
    let looking_up = || L::text("Looking up home.example...").within(pane());

    h.click(&pane_button("Join"));
    assert!(h.is_visible(&looking_up()));
    h.click(&card("Survival"));
    assert!(!h.is_visible(&looking_up()));
    finish_the_lookup(&mut h, 1);
    assert_eq!(target_address(&h), None);
    assert_eq!(h.game_state(), GameState::Menu);
    assert_eq!(h.menu_state(), MenuState::Multi);

    h.click(&card("Home"));
    h.click(&pane_button("Join"));
    assert!(h.is_visible(&looking_up()));
    add_dialog(&mut h);
    finish_the_lookup(&mut h, 2);
    h.press(KeyCode::Escape);
    h.frames(2);
    assert!(!h.is_visible(&looking_up()));
    assert_eq!(target_address(&h), None);
    assert_eq!(h.game_state(), GameState::Menu);
    assert_eq!(h.menu_state(), MenuState::Multi);
}

/// Finds no host.
fn no_such_host(address: &ServerAddress) -> io::Result<SocketAddr> {
    Err(io::Error::new(
        io::ErrorKind::NotFound,
        format!("no {}", address.host),
    ))
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn a_host_that_is_not_found_is_said_in_the_pane() {
    let mut h = status_list();
    h.world_mut().insert_resource(AddressResolver(no_such_host));
    h.click(&card("Typo"));
    h.click(&pane_button("Join"));
    h.wait_for(
        &L::text("Cannot find nowhere.invalid").within(pane()),
        SCREEN,
    );
    h.frames(2);
    assert_eq!(target_address(&h), None);
    assert_eq!(h.menu_state(), MenuState::Multi);
}

/// The addresses `answer_once` was asked.
static ASKED: Mutex<Option<HashSet<String>>> = Mutex::new(None);

/// Answers as `fixed_status` the first time an address is asked, and only after a long while
/// then.
fn answer_once(ip: &str) -> ServerStatus {
    let first = ASKED
        .lock()
        .unwrap()
        .get_or_insert_with(HashSet::new)
        .insert(ip.to_string());
    if !first {
        thread::sleep(Duration::from_secs(5));
    }
    fixed_status(ip)
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn a_server_removed_then_added_again_is_asked_again() {
    let mut h = open_server_list_with(
        GameFolder::new().with_status_servers(),
        StatusProbe(answer_once),
    );
    h.wait_for(&card("Typo"), SCREEN);
    wait_for_answers(&mut h);
    remove_through_the_pane(&mut h, "Survival");
    add_dialog(&mut h);
    h.type_text("Again");
    h.press(KeyCode::Tab);
    h.type_text("10.0.0.1:8000");
    h.press(KeyCode::Enter);
    h.wait_for(&card("Again"), SCREEN);
    assert!(h.is_visible(&L::text("Pinging...").within(card("Again"))));
    assert!(!h.is_visible(&L::text("3 / 20").within(card("Again"))));
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn remove_asks_first_and_the_focus_moves_to_the_next_server() {
    let mut h = status_list();
    h.click(&card("Creative"));
    h.press(KeyCode::Delete);
    h.wait_for(&remove_dialog("Creative"), SCREEN);
    assert!(h.is_visible(&L::text("10.0.0.2 will leave your server list.")));
    assert_eq!(h.focus_label().as_deref(), Some("Cancel"));

    h.press(KeyCode::Escape);
    assert!(!h.is_visible(&remove_dialog("Creative")));
    h.frames(2);
    assert_eq!(h.menu_state(), MenuState::Multi);
    assert_eq!(saved_names(&h).len(), 5);

    h.click(&pane_button("Remove"));
    h.wait_for(&remove_dialog("Creative"), SCREEN);
    h.click(&L::role(Role::Button, "Remove").within(remove_dialog("Creative")));
    h.frames(2);
    let left = ["Survival", "Old friends", "Friends", "Typo"];
    assert_eq!(saved_names(&h), left);
    assert_eq!(card_names(&mut h), left);
    assert_eq!(h.focus_label().as_deref(), Some("Old friends"));
    assert_eq!(selected_server(&mut h).as_deref(), Some("Old friends"));

    for name in left {
        remove_through_the_pane(&mut h, name);
    }
    assert!(h.is_visible(&L::text("No servers yet")));
    assert!(!h.is_visible(&pane()));
    assert_eq!(h.focus_label().as_deref(), Some("Add server"));
}

#[cfg(unix)]
#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn a_failed_remove_keeps_the_server_and_says_why_in_the_pane() {
    use std::os::unix::fs::PermissionsExt;

    let mut h = status_list();
    let folder = h.folder().to_path_buf();
    let file = saved_file(&h);
    h.click(&card("Creative"));
    h.click(&pane_button("Remove"));
    h.wait_for(&remove_dialog("Creative"), SCREEN);
    let confirm = h.center_of(&L::role(Role::Button, "Remove").within(remove_dialog("Creative")));
    // the list is written through a file next to it, which a read-only folder refuses
    fs::set_permissions(&folder, fs::Permissions::from_mode(0o555)).unwrap();
    h.click_at(confirm);
    h.frames(2);
    // put back before anything can panic: a failed test drops nothing
    fs::set_permissions(&folder, fs::Permissions::from_mode(0o755)).unwrap();

    assert_eq!(saved_file(&h), file);
    assert_eq!(card_names(&mut h).len(), 5);
    assert_eq!(selected_server(&mut h).as_deref(), Some("Creative"));
    let failure = L::text("No permission to write here").within(pane());
    assert!(h.is_visible(&failure));
    // another server selected, the message goes
    h.click(&card("Survival"));
    h.frames(1);
    assert!(!h.is_visible(&failure));
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn a_removed_localhost_stays_removed() {
    let mut h = open_server_list_with(GameFolder::new(), StatusProbe(fixed_status));
    h.wait_for(&card("localhost"), SCREEN);
    assert_eq!(card_names(&mut h), ["localhost"]);
    remove_through_the_pane(&mut h, "localhost");
    h.press(KeyCode::Escape);
    h.click_when_ready(&L::text("Multiplayer"));
    h.wait_for(&L::text("No servers yet"), SCREEN);
    assert_eq!(saved_file(&h), "[]");
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn an_unreadable_server_list_is_said_and_never_overwritten() {
    let folder = GameFolder::new().with_file(SERVER_LIST_SAVE_NAME, "not ron");
    let mut h = open_server_list_with(folder, StatusProbe(fixed_status));
    h.wait_for(&L::text("Cannot read servers.ron"), SCREEN);
    assert!(is_disabled(&mut h, &L::role(Role::Button, "Add server")));
    assert!(is_disabled(&mut h, &L::role(Role::Button, "Refresh")));
    assert_eq!(h.focus_label().as_deref(), Some("Back"));

    ctrl(&mut h, KeyCode::KeyN);
    h.frames(2);
    assert!(!dialog_is_open(&mut h));
    h.press(KeyCode::Escape);
    h.wait_for(&L::text("Singleplayer"), SCREEN);
    assert_eq!(h.menu_state(), MenuState::Main);
    assert_eq!(saved_file(&h), "not ron");
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn list_keys_do_nothing_while_a_dialog_is_open() {
    let mut h = status_list();
    let names = card_names(&mut h);
    add_dialog(&mut h);
    h.press(KeyCode::F5);
    h.press(KeyCode::Delete);
    ctrl(&mut h, KeyCode::KeyN);
    h.frames(2);
    // the Add server dialog alone: neither a second one nor the Remove dialog
    assert_eq!(h.find(&L::marker::<DialogRoot>()).len(), 1);
    assert!(dialog_is_open(&mut h));
    assert!(!h.is_visible(&L::text("Pinging...")));
    assert_eq!(card_names(&mut h), names);
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn the_key_hints_follow_the_focus() {
    let bar = || L::marker::<HintBar>();
    let mut h = status_list();
    h.wait_for(&L::text("Del").within(bar()), SCREEN);
    assert!(h.is_visible(&L::text("F5").within(bar())));
    h.press(KeyCode::Tab);
    h.frames(1);
    assert!(!h.is_visible(&L::text("Del").within(bar())));
    assert!(h.is_visible(&L::text("Ctrl+N").within(bar())));
    add_dialog(&mut h);
    h.frames(1);
    assert!(h.is_visible(&L::text("Add").within(bar().within(dialog()))));
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn hovering_a_card_or_a_button_lightens_it() {
    let mut h = status_list();
    let background = |h: &mut Harness, locator: &L| {
        let entity = h.get(locator);
        h.world().get::<BackgroundColor>(entity).unwrap().0
    };
    h.hover(&card("Creative"));
    h.frames(1);
    assert_eq!(background(&mut h, &card("Creative")), RAISE_HOVER);
    let add_server = L::role(Role::Button, "Add server");
    h.hover(&add_server);
    h.frames(1);
    assert_eq!(background(&mut h, &add_server), RAISE_HOVER);
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn the_wheel_scrolls_only_the_list_under_the_cursor() {
    let names: Vec<String> = (1..=15).map(|i| format!("server {i:02}")).collect();
    let ips: Vec<String> = (1..=15).map(|i| format!("10.0.1.{i}")).collect();
    let servers: Vec<(&str, &str)> = names
        .iter()
        .zip(&ips)
        .map(|(name, ip)| (name.as_str(), ip.as_str()))
        .collect();
    let folder = GameFolder::new().with_servers(&servers);
    let mut h = open_server_list_with(folder, StatusProbe(fixed_status));
    let first = card("server 01");
    h.wait_for(&first, SCREEN);
    let before = h.center_of(&first).y;
    h.hover(&first);
    h.scroll(-5.);
    h.frames(2);
    let after = h.center_of(&first).y;
    assert!(after < before, "the list stayed at {before}, then {after}");

    h.hover(&pane());
    h.scroll(-5.);
    h.frames(2);
    assert_eq!(h.center_of(&first).y, after, "the pane scrolled the list");
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn back_and_escape_return_to_the_main_menu() {
    let mut h = server_list();
    h.click_when_ready(&L::role(Role::Button, "Back"));
    h.wait_for(&L::text("Singleplayer"), SCREEN);
    assert_eq!(h.menu_state(), MenuState::Main);

    h.click(&L::text("Multiplayer"));
    h.wait_for(&card("localhost"), SCREEN);
    assert_eq!(h.focus_label().as_deref(), Some("localhost"));
    h.press(KeyCode::Escape);
    h.wait_for(&L::text("Singleplayer"), SCREEN);
    assert_eq!(h.menu_state(), MenuState::Main);
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn a_long_name_and_address_stay_inside_their_card_at_800x600() {
    let name = "W".repeat(32);
    let address = format!("{}.example.com", "a".repeat(52));
    assert_eq!(address.len(), 64);
    let small = Options {
        width: 800,
        height: 600,
        ..Default::default()
    };
    let mut h = Harness::start(GameFolder::new().with_servers(&[(&name, &address)]), small);
    h.world_mut().insert_resource(StatusProbe(fixed_status));
    h.wait_for(&L::text("Multiplayer"), STARTUP);
    h.click(&L::text("Multiplayer"));
    h.wait_for(&card(&name), SCREEN);
    wait_for_answers(&mut h);
    h.frames(2);
    let inside = |outer: Rect, inner: Rect| outer.contains(inner.min) && outer.contains(inner.max);
    let card_rect = h.rect_of(&card(&name));
    let pane_rect = h.rect_of(&pane());
    for text in [&name, &address] {
        let in_card = h.rect_of(&L::text(text).within(card(&name)));
        assert!(inside(card_rect, in_card), "{in_card:?} in {card_rect:?}");
        let in_pane = h.rect_of(&L::text(text).within(pane()));
        assert!(inside(pane_rect, in_pane), "{in_pane:?} in {pane_rect:?}");
    }
    // the name and the address take more lines of the pane: its buttons stay inside it
    for action in ["Join", "Remove"] {
        let button = h.rect_of(&pane_button(action));
        assert!(
            inside(pane_rect, button),
            "{action}: {button:?} in {pane_rect:?}"
        );
    }
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn add_server_opens_a_dialog_with_a_free_name_ready_to_replace() {
    let mut h = server_list();
    h.click_when_ready(&L::text("Add server"));
    h.wait_for(&dialog(), SCREEN);
    h.frames(2);
    assert_eq!(h.focus_label().as_deref(), Some("Name"));
    assert_eq!(h.value_of::<ServerNameInput>(), "My server");
    h.type_text("Home");
    assert_eq!(h.value_of::<ServerNameInput>(), "Home");
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn tab_and_enter_add_a_server() {
    let mut h = server_list();
    h.wait_for(&card("Friends"), SCREEN);
    add_dialog(&mut h);
    h.type_text("Home");
    h.press(KeyCode::Tab);
    h.type_text("10.0.0.2:8000");
    h.press(KeyCode::Enter);
    h.frames(3);
    assert!(!dialog_is_open(&mut h));
    assert_eq!(card_names(&mut h), ["localhost", "Friends", "Home"]);
    assert_eq!(saved_names(&h), ["localhost", "Friends", "Home"]);
    assert_eq!(h.focus_label().as_deref(), Some("Home"));
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn a_new_server_is_listed_and_saved() {
    let mut h = server_list();
    h.click_when_ready(&L::text("Add server"));
    h.wait_for(&dialog(), SCREEN);
    let name = L::role(Role::TextInput, "Name");
    let address = L::role(Role::TextInput, "Address");

    h.click(&address);
    h.frames(2);
    assert!(h.has_focus::<ServerAddressInput>());
    assert!(!h.has_focus::<ServerNameInput>());
    h.click(&name);
    h.frames(2);
    assert!(h.has_focus::<ServerNameInput>());
    assert!(!h.has_focus::<ServerAddressInput>());

    h.type_text("Home");
    h.click(&address);
    h.type_text("10.0.0.2:8000");
    h.click(&add_button());
    h.wait_for(&card("Home"), SCREEN);
    assert!(h.is_visible(&L::text("10.0.0.2:8000").within(card("Home"))));
    let saved = saved_servers(&h);
    let home = saved
        .iter()
        .find(|s| s.name == "Home")
        .expect("Home is saved");
    assert_eq!(home.ip, "10.0.0.2:8000");
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn enter_in_the_name_field_adds_the_server() {
    let mut h = server_list();
    h.wait_for(&card("Friends"), SCREEN);
    add_dialog(&mut h);
    h.type_text("Home");
    h.press(KeyCode::Tab);
    h.type_text("10.0.0.2:8000");
    shift_tab(&mut h);
    assert!(h.has_focus::<ServerNameInput>());
    h.press(KeyCode::Enter);
    h.wait_for(&card("Home"), SCREEN);
    assert_eq!(saved_names(&h), ["localhost", "Friends", "Home"]);
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn the_server_is_checked_as_you_type_and_nothing_moves() {
    let mut h = server_list();
    h.wait_for(&card("Friends"), SCREEN);
    let file = saved_file(&h);
    add_dialog(&mut h);
    let y = h.center_of(&add_button()).y;
    let add_disabled = |h: &mut Harness| is_disabled(h, &add_button());
    let messages = [
        EMPTY_ADDRESS,
        INVALID_ADDRESS,
        INVALID_PORT,
        SERVER_EXISTS,
        EMPTY_SERVER_NAME,
        SERVER_NAME_EXISTS,
    ];

    // an empty address never typed into says nothing, yet cannot be added
    h.frames(2);
    for message in messages {
        assert!(!h.is_visible(&L::text(message)), "{message}");
    }
    assert!(add_disabled(&mut h));

    h.press(KeyCode::Tab);
    h.type_text("not an ip");
    h.wait_for(&L::text(INVALID_ADDRESS), SCREEN);
    assert!(add_disabled(&mut h));
    assert_eq!(h.center_of(&add_button()).y, y);
    h.press(KeyCode::Enter);
    h.frames(2);
    assert!(dialog_is_open(&mut h));
    assert_eq!(saved_file(&h), file);

    backspace(&mut h, 9);
    h.wait_for(&L::text(EMPTY_ADDRESS), SCREEN);
    assert!(add_disabled(&mut h));
    h.type_text("host:0");
    h.wait_for(&L::text(INVALID_PORT), SCREEN);
    assert!(add_disabled(&mut h));
    assert_eq!(h.center_of(&add_button()).y, y);
    backspace(&mut h, 6);
    h.type_text("192.168.1.20");
    h.wait_for(&L::text(SERVER_EXISTS), SCREEN);
    assert!(add_disabled(&mut h));
    assert_eq!(h.center_of(&add_button()).y, y);
    backspace(&mut h, 12);
    h.type_text("play.example.com");
    h.frames(2);
    for message in messages {
        assert!(!h.is_visible(&L::text(message)), "{message}");
    }
    assert!(!add_disabled(&mut h));

    shift_tab(&mut h);
    backspace(&mut h, "My server".len());
    h.wait_for(&L::text(EMPTY_SERVER_NAME), SCREEN);
    assert!(add_disabled(&mut h));
    h.type_text("friends");
    h.wait_for(&L::text(SERVER_NAME_EXISTS), SCREEN);
    assert!(add_disabled(&mut h));
    assert_eq!(h.center_of(&add_button()).y, y);
    assert_eq!(saved_file(&h), file);
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn escape_in_a_field_closes_only_the_dialog() {
    let mut h = server_list();
    h.wait_for(&card("Friends"), SCREEN);
    let file = saved_file(&h);
    add_dialog(&mut h);
    h.type_text("Ho");
    h.press(KeyCode::Escape);
    assert!(!dialog_is_open(&mut h));
    assert_eq!(h.menu_state(), MenuState::Multi);
    h.frames(2);
    assert_eq!(h.menu_state(), MenuState::Multi);
    assert_eq!(saved_file(&h), file);

    h.press(KeyCode::Escape);
    h.wait_for(&L::text("Singleplayer"), SCREEN);
    assert_eq!(h.menu_state(), MenuState::Main);
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn after_an_add_the_dialog_opens_fresh() {
    let mut h = server_list();
    h.wait_for(&card("Friends"), SCREEN);
    add_dialog(&mut h);
    h.type_text("One");
    h.press(KeyCode::Tab);
    h.type_text("10.0.0.5");
    h.press(KeyCode::Enter);
    h.wait_for(&card("One"), SCREEN);

    add_dialog(&mut h);
    h.frames(2);
    assert_eq!(h.value_of::<ServerNameInput>(), "My server");
    assert_eq!(h.value_of::<ServerAddressInput>(), "");
}
