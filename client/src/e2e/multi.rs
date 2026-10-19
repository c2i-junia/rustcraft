//! The server list: adding, deleting and joining servers.

use std::fs;
use std::net::{IpAddr, Ipv4Addr};

use accesskit::Role;
use bevy::{
    input::keyboard::KeyCode,
    input_focus::InputFocus,
    prelude::{Component, Entity, With},
};
use shared::{GameFolderPaths, GameServerConfig};

use super::harness::{Harness, SCREEN, SERVER, STARTUP};
use super::helpers::is_in;
use super::locator::Locator as L;
use super::seed::GameFolder;
use crate::constants::SERVER_LIST_SAVE_NAME;
use crate::state::GameState;
use crate::ui::button::MenuState;
use crate::ui::menus::multi::{ServerIpInput, ServerItem, ServerNameInput, INVALID_ADDRESS};

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

impl Harness {
    /// Types `name` and `ip` in the fields, each focused by a click, and clicks "Add server".
    #[track_caller]
    fn add_server(&mut self, name: &str, ip: &str) {
        self.fill_server(name, ip);
        self.click(&L::text("Add server"));
    }

    /// Types `name` and `ip` in the fields, each focused by a click.
    #[track_caller]
    fn fill_server(&mut self, name: &str, ip: &str) {
        self.click(&L::role(Role::TextInput, "Server name"));
        self.type_text(name);
        self.click(&L::role(Role::TextInput, "Server IP"));
        self.type_text(ip);
        assert_eq!(self.value_of::<ServerNameInput>(), name);
        assert_eq!(self.value_of::<ServerIpInput>(), ip);
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

    /// The servers of the saved list, in order.
    fn saved_servers(&self) -> Vec<ServerItem> {
        let file = self.folder().join(SERVER_LIST_SAVE_NAME);
        let text = fs::read_to_string(&file).expect("a saved server list");
        ron::from_str(&text).expect("a server list in RON")
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
fn clicking_a_field_focuses_it_alone() {
    let mut h = server_list();
    h.click(&L::role(Role::TextInput, "Server name"));
    h.frames(2);
    assert!(h.has_focus::<ServerNameInput>());
    assert!(!h.has_focus::<ServerIpInput>());

    h.click(&L::role(Role::TextInput, "Server IP"));
    h.frames(2);
    assert!(h.has_focus::<ServerIpInput>());
    assert!(!h.has_focus::<ServerNameInput>());
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn a_new_server_is_listed_and_saved() {
    let mut h = server_list();
    h.add_server("Home", "10.0.0.2:8000");
    h.wait_for(&L::name("Play Home"), SCREEN);
    assert!(h.is_visible(&L::text("Home")));

    h.click(&L::text("Back to menu"));
    h.wait_for(&L::text("Singleplayer"), SCREEN);
    let saved = h.saved_servers();
    let home = saved
        .iter()
        .find(|s| s.name == "Home")
        .expect("Home is saved");
    assert_eq!(home.ip, "10.0.0.2:8000");
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn tab_and_enter_add_a_server() {
    let mut h = server_list();
    h.click(&L::role(Role::TextInput, "Server name"));
    h.type_text("Home");
    h.press(KeyCode::Tab);
    h.type_text("10.0.0.2:8000");
    h.press(KeyCode::Enter);
    h.wait_for(&L::name("Play Home"), SCREEN);
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn enter_in_the_name_field_adds_the_server() {
    let mut h = server_list();
    h.fill_server("Home", "10.0.0.2:8000");
    h.click(&L::role(Role::TextInput, "Server name"));
    h.press(KeyCode::Enter);
    h.wait_for(&L::name("Play Home"), SCREEN);
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn an_invalid_address_adds_no_server() {
    let mut h = server_list();
    h.wait_for(&L::name("Play Friends"), SCREEN);
    let rows = h.play_rows();
    h.add_server("Bad", "not an ip");
    // The click frame handles the button: two more frames are plenty for a negative check
    h.frames(2);
    assert_eq!(h.find(&L::name("Play Bad")).len(), 0);
    assert_eq!(h.play_rows(), rows);
    assert!(h.is_visible(&L::text(INVALID_ADDRESS)));
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

    h.click(&L::text("Back to menu"));
    h.wait_for(&L::text("Singleplayer"), SCREEN);
    let saved: Vec<String> = h.saved_servers().into_iter().map(|s| s.name).collect();
    assert!(!saved.iter().any(|name| name == "Friends"), "{saved:?}");
    assert!(saved.iter().any(|name| name == "localhost"), "{saved:?}");
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
fn escape_in_a_field_leaves_the_field_then_the_screen() {
    let mut h = server_list();
    h.click(&L::role(Role::TextInput, "Server name"));
    h.type_text("Ho");
    h.press(KeyCode::Escape);
    assert_eq!(h.menu_state(), MenuState::Multi);
    h.frames(2);
    assert_eq!(h.menu_state(), MenuState::Multi);
    assert_eq!(h.world().resource::<InputFocus>().get(), None);

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
