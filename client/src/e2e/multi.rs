//! The server list: adding, deleting and joining servers.

use std::fs;
use std::net::{IpAddr, Ipv4Addr};

use accesskit::Role;
use bevy::{
    input::keyboard::KeyCode,
    input_focus::InputFocus,
    prelude::{Component, Entity, With},
    ui::InteractionDisabled,
};
use shared::{GameFolderPaths, GameServerConfig};

use super::harness::{Harness, SCREEN, SERVER, STARTUP};
use super::helpers::{ctrl, is_in};
use super::locator::Locator as L;
use super::seed::GameFolder;
use crate::constants::SERVER_LIST_SAVE_NAME;
use crate::network::address::{EMPTY_ADDRESS, INVALID_ADDRESS, INVALID_PORT};
use crate::state::GameState;
use crate::ui::button::MenuState;
use crate::ui::menus::multi::dialogs::{ServerAddressInput, ServerNameInput, ADD_SERVER_TITLE};
use crate::ui::menus::servers::{
    read_servers, ServerItem, EMPTY_SERVER_NAME, SERVER_EXISTS, SERVER_NAME_EXISTS,
};

/// A client on the server list, with the saved servers `localhost` and `Friends`.
fn server_list() -> Harness {
    open_server_list(GameFolder::new().with_servers(&[
        ("localhost", "127.0.0.1:8000"),
        ("Friends", "192.168.1.20:8000"),
    ]))
}

/// A client on the server list, with the game folder `folder`.
fn open_server_list(folder: GameFolder) -> Harness {
    let mut h = Harness::start(folder, Default::default());
    h.wait_for(&L::text("Multiplayer"), STARTUP);
    h.click(&L::text("Multiplayer"));
    h.wait_for(&L::text("Server list"), SCREEN);
    h
}

/// The Add server dialog.
fn dialog() -> L {
    L::role(Role::Dialog, ADD_SERVER_TITLE)
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

impl Harness {
    /// Adds the server `name` at `ip` through the Add server dialog, opened by its button.
    #[track_caller]
    fn add_server(&mut self, name: &str, ip: &str) {
        self.click_when_ready(&L::text("Add server"));
        self.wait_for(&dialog(), SCREEN);
        self.type_text(name);
        self.press(KeyCode::Tab);
        self.type_text(ip);
        self.press(KeyCode::Enter);
        self.wait_for(&L::name(&format!("Play {name}")), SCREEN);
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
    let mut h = server_list();
    h.wait_for_icons("Friends");
    h.assert_screenshot("menu_multi");
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn localhost_and_the_saved_servers_are_listed() {
    let mut h = server_list();
    for text in [
        "localhost",
        "127.0.0.1:8000",
        "Friends",
        "192.168.1.20:8000",
    ] {
        h.wait_for(&L::text(text), SCREEN);
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
    h.wait_for(&L::name("Play Friends"), SCREEN);
    add_dialog(&mut h);
    h.type_text("Home");
    h.press(KeyCode::Tab);
    h.type_text("10.0.0.2:8000");
    h.press(KeyCode::Enter);
    h.frames(3);
    assert!(!dialog_is_open(&mut h));
    assert_eq!(h.find(&L::name("Play Home")).len(), 1);
    assert_eq!(saved_names(&h), ["localhost", "Friends", "Home"]);
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
    h.wait_for(&L::name("Play Home"), SCREEN);
    assert!(h.is_visible(&L::text("Home")));
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
    h.wait_for(&L::name("Play Friends"), SCREEN);
    add_dialog(&mut h);
    h.type_text("Home");
    h.press(KeyCode::Tab);
    h.type_text("10.0.0.2:8000");
    shift_tab(&mut h);
    assert!(h.has_focus::<ServerNameInput>());
    h.press(KeyCode::Enter);
    h.wait_for(&L::name("Play Home"), SCREEN);
    assert_eq!(saved_names(&h), ["localhost", "Friends", "Home"]);
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn the_server_is_checked_as_you_type_and_nothing_moves() {
    let mut h = server_list();
    h.wait_for(&L::name("Play Friends"), SCREEN);
    let file = saved_file(&h);
    add_dialog(&mut h);
    let y = h.center_of(&add_button()).y;
    let add_disabled = |h: &mut Harness| {
        let add = h.get(&add_button());
        h.world().get::<InteractionDisabled>(add).is_some()
    };
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
    h.wait_for(&L::name("Play Friends"), SCREEN);
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
    h.wait_for(&L::name("Play Friends"), SCREEN);
    add_dialog(&mut h);
    h.type_text("One");
    h.press(KeyCode::Tab);
    h.type_text("10.0.0.5");
    h.press(KeyCode::Enter);
    h.wait_for(&L::name("Play One"), SCREEN);

    add_dialog(&mut h);
    h.frames(2);
    assert_eq!(h.value_of::<ServerNameInput>(), "My server");
    assert_eq!(h.value_of::<ServerAddressInput>(), "");
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn deleting_a_server_removes_it() {
    let mut h = server_list();
    h.wait_for_icons("Friends");
    h.click(&L::name("Delete Friends"));
    h.frames(2);
    assert!(!h.is_visible(&L::text("Friends")));
    assert!(h.is_visible(&L::text("localhost")));
    assert_eq!(saved_names(&h), ["localhost"]);
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn back_and_escape_return_to_the_main_menu() {
    let mut h = server_list();
    h.click(&L::text("Back to menu"));
    h.wait_for(&L::text("Singleplayer"), SCREEN);
    assert_eq!(h.menu_state(), MenuState::Main);

    h.click(&L::text("Multiplayer"));
    h.wait_for(&L::text("Server list"), SCREEN);
    h.press(KeyCode::Escape);
    h.wait_for(&L::text("Singleplayer"), SCREEN);
    assert_eq!(h.menu_state(), MenuState::Main);
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn connecting_to_a_server_enters_the_game() {
    // The server runs on its own thread and is left running when the test ends: a dedicated
    // server has no way to stop. Its folder belongs to the test, which deletes it at the end.
    // The server may then fail to save into it, and only writes an error; it cannot fail the
    // test, which has stopped listening to the server, or keep the process from exiting.
    let server_folder = GameFolder::new();
    let socket = server::acquire_local_ephemeral_udp_socket(IpAddr::V4(Ipv4Addr::LOCALHOST))
        .expect("a socket for the server");
    let address = socket.local_addr().unwrap();
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
        if let Err(error) = server::init(socket, config, paths) {
            let _ = stopped.send(error);
        }
    });

    let mut h = server_list();
    h.add_server("Local test", &address.to_string());
    h.wait_for_icons("Local test");
    h.click(&L::name("Play Local test"));
    h.wait_until("the game", SERVER, |w| {
        if let Ok(error) = server_error.try_recv() {
            panic!("the server stopped: {error}");
        }
        is_in(w, GameState::Game)
    });
}
