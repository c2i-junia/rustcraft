//! The server list: adding, deleting and joining servers.

use std::fs;
use std::net::{IpAddr, Ipv4Addr};

use bevy::{
    input::keyboard::KeyCode,
    prelude::{Component, Name, With},
};
use bevy_simple_text_input::{TextInputInactive, TextInputValue};
use shared::{GameFolderPaths, GameServerConfig};

use super::harness::{Harness, SCREEN, SERVER};
use super::locator::Locator as L;
use super::seed::GameFolder;
use crate::constants::SERVER_LIST_SAVE_NAME;
use crate::state::GameState;
use crate::ui::button::MenuState;
use crate::ui::menus::multi::{ServerIpInput, ServerNameInput};

/// A client on the server list, with the saved server `Friends`.
fn server_list() -> Harness {
    open_server_list(GameFolder::new().with_servers(&[("Friends", "192.168.1.20:8000")]))
}

fn open_server_list(folder: GameFolder) -> Harness {
    let mut h = Harness::start(folder, Default::default());
    h.click_when_ready("Multiplayer");
    h.wait_for(&L::text("Server list"), SCREEN);
    h
}

impl Harness {
    /// Types `name` and `ip` in the fields, each focused by a click, and clicks "Add server".
    fn add_server(&mut self, name: &str, ip: &str) {
        self.click(&L::marker::<ServerNameInput>());
        self.type_text(name);
        self.click(&L::marker::<ServerIpInput>());
        self.type_text(ip);
        assert_eq!(self.value_of::<ServerNameInput>(), name);
        assert_eq!(self.value_of::<ServerIpInput>(), ip);
        self.click(&L::text("Add server"));
    }

    fn value_of<M: Component>(&mut self) -> String {
        self.world_mut()
            .query_filtered::<&TextInputValue, With<M>>()
            .single(self.world())
            .unwrap()
            .0
            .clone()
    }

    fn is_inactive<M: Component>(&mut self) -> bool {
        self.world_mut()
            .query_filtered::<&TextInputInactive, With<M>>()
            .single(self.world())
            .unwrap()
            .0
    }

    fn saved_servers(&self) -> String {
        fs::read_to_string(self.folder().join(SERVER_LIST_SAVE_NAME)).unwrap_or_default()
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
    h.click(&L::marker::<ServerNameInput>());
    h.frames(2);
    assert!(!h.is_inactive::<ServerNameInput>());
    assert!(h.is_inactive::<ServerIpInput>());

    h.click(&L::marker::<ServerIpInput>());
    h.frames(2);
    assert!(!h.is_inactive::<ServerIpInput>());
    assert!(h.is_inactive::<ServerNameInput>());
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
    assert!(saved.contains("Home"), "{saved}");
    assert!(saved.contains("10.0.0.2:8000"), "{saved}");
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn an_invalid_address_adds_no_server() {
    let mut h = server_list();
    h.wait_for(&L::name("Play Friends"), SCREEN);
    h.add_server("Bad", "not an ip");
    // The click frame handles the button: two more frames are plenty for a negative check
    h.frames(2);
    assert_eq!(h.find(&L::name("Play Bad")).len(), 0);
    let rows = h
        .world_mut()
        .query::<&Name>()
        .iter(h.world())
        .filter(|n| n.starts_with("Play "))
        .count();
    assert_eq!(rows, 2);
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
    let saved = h.saved_servers();
    assert!(!saved.contains("Friends"), "{saved}");
    assert!(saved.contains("localhost"), "{saved}");
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
    // The server runs on its own thread and folder, and is left running when the test ends:
    // a dedicated server has no way to stop.
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
    std::thread::spawn(move || {
        let _keep_alive = server_folder;
        server::init(socket, config, paths)
    });

    let mut h = server_list();
    h.add_server("Local test", &address.to_string());
    h.wait_for_icons("Local test");
    h.click(&L::name("Play Local test"));
    h.wait_until("the game", SERVER, |w| {
        *w.resource::<bevy::prelude::State<GameState>>().get() == GameState::Game
    });
}
