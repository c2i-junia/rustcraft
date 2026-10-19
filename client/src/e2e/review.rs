//! Snapshots of every screen at three resolutions, for a human to review.

use std::fs;
use std::time::{Duration, SystemTime};

use accesskit::Role;
use bevy::input::keyboard::KeyCode;
use bevy::math::Vec2;
use server::WORLD_FILE;
use shared::world::{grid_slot, ItemId};

use super::game::{enter_world, slot};
use super::harness::{Harness, Options, SCREEN, SERVER, STARTUP};
use super::helpers::{ctrl, ctrl_n};
use super::locator::Locator as L;
use super::multi::wait_for_answers;
use super::seed::{fixed_status, GameFolder};
use super::settings::key_slot;
use crate::constants::{DAY_DURATION_IN_TICKS, SAVE_PATH, SERVER_LIST_SAVE_NAME};
use crate::input::keyboard::default_bindings;
use crate::network::status::StatusProbe;
use crate::state::KeyMap;
use crate::ui::menus::multi::dialogs::ADD_SERVER_TITLE;
use crate::ui::menus::multi::ServerDetail;
use crate::ui::menus::servers::SERVER_EXISTS;
use crate::ui::menus::settings::controls::PRESS_A_KEY;
use crate::ui::menus::worlds::WORLD_EXISTS;
use crate::ui::menus::{ContinueCard, QUIT_TITLE};

/// The resolutions the screens are reviewed at.
const RESOLUTIONS: [(u32, u32); 3] = [(800, 600), (1280, 720), (1920, 1080)];

/// A day, in seconds.
const DAY: u64 = 24 * 60 * 60;

/// The longest name a world can have: 32 of the widest letter.
const LONG_NAME: &str = "WWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWW";

/// The longest address a server can be given: 64 characters, of the widest letter.
const LONG_ADDRESS: &str = "wwwwwwwwwwwwwwwwwwwwwwwwwwwww.wwwwwwwwwwwwwwwwwwwwwwwwwwww:65535";

/// The folder of the menus: `Review` for the Continue card, last played, then older worlds, one
/// with the longest name and one whose `world.ron` cannot be read; and a server of every status
/// `fixed_status` gives, then one with the longest name and address.
fn menus_folder() -> GameFolder {
    let folder = GameFolder::new()
        .with_servers(&[
            ("Survival", "10.0.0.1:8000"),
            ("Creative", "10.0.0.2"),
            ("Old friends", "10.0.0.3:8000"),
            ("Friends", "10.0.0.4:8000"),
            ("Typo", "nowhere.invalid"),
            ("Broken", "10.0.0.5:0"),
            (LONG_NAME, LONG_ADDRESS),
        ])
        .with_played_world("Review", 0, Duration::from_secs(3 * DAY))
        .with_seeded_world(
            "Creative build",
            1,
            11 * DAY_DURATION_IN_TICKS,
            Duration::from_secs(5 * DAY),
        )
        .with_seeded_world(LONG_NAME, 4294967295, 0, Duration::from_secs(10 * DAY))
        .with_world("Broken");
    let broken = folder
        .path()
        .join(SAVE_PATH)
        .join("Broken")
        .join(WORLD_FILE);
    fs::write(&broken, "not a world").unwrap();
    fs::File::options()
        .write(true)
        .open(&broken)
        .and_then(|file| file.set_modified(SystemTime::now() - Duration::from_secs(30 * DAY)))
        .unwrap();
    folder
}

impl Harness {
    /// Runs frames until every image of the UI is loaded, then saves the snapshot `name`.
    fn settled_snapshot(&mut self, name: &str) {
        self.wait_for_ui_images();
        self.snapshot(name);
    }

    /// Lets the game draw a few frames, then saves the snapshot `name`. The atlas of the items
    /// is built at runtime and never counts as loaded, unlike the images of the menus, so
    /// `wait_for_ui_images` would time out: only the frames are waited for.
    fn snapshot_in_game(&mut self, name: &str) {
        self.frames(10);
        self.snapshot(name);
    }

    /// Reviews the Singleplayer screen from the main menu, with the worlds of `menus_folder`:
    /// the list, the New world dialog and its error, the Delete and Rename dialogs, and the
    /// longest name, selected by the keyboard, in the pane and in the dialogs. Goes back to the
    /// main menu.
    fn review_worlds(&mut self) {
        let card = |name: &str| L::role(Role::ListItem, name);
        self.click(&L::text("Singleplayer"));
        self.wait_for(&card("Review"), SCREEN);
        // the cursor in a corner, so that it hovers nothing
        self.hover_at(Vec2::ZERO);
        self.settled_snapshot("solo");
        ctrl_n(self);
        self.settled_snapshot("solo_new_world");
        self.type_text("Review");
        self.wait_for(&L::text(WORLD_EXISTS), SCREEN);
        self.settled_snapshot("solo_new_world_error");
        self.press(KeyCode::Escape);
        // the focus is back on the card of Review
        self.press(KeyCode::Delete);
        self.wait_for(&L::role(Role::Dialog, "Delete \"Review\"?"), SCREEN);
        self.settled_snapshot("solo_delete");
        self.press(KeyCode::Escape);
        self.click(&L::role(Role::Button, "Rename"));
        self.wait_for(&L::role(Role::Dialog, "Rename \"Review\""), SCREEN);
        self.settled_snapshot("solo_rename");
        self.press(KeyCode::Escape);

        // by the keyboard, for the focus ring around the selected card
        self.click(&card("Review"));
        self.press(KeyCode::ArrowDown);
        self.press(KeyCode::ArrowDown);
        self.wait_for(&L::text("Seed 4294967295"), SCREEN);
        self.hover_at(Vec2::ZERO);
        self.settled_snapshot("solo_long_name");
        self.press(KeyCode::Delete);
        self.wait_for(
            &L::role(Role::Dialog, &format!("Delete \"{LONG_NAME}\"?")),
            SCREEN,
        );
        self.settled_snapshot("solo_long_name_delete");
        self.press(KeyCode::Escape);
        self.click(&L::role(Role::Button, "Rename"));
        self.wait_for(
            &L::role(Role::Dialog, &format!("Rename \"{LONG_NAME}\"")),
            SCREEN,
        );
        self.settled_snapshot("solo_long_name_rename");
        self.press(KeyCode::Escape);
        self.click(&L::role(Role::Button, "Back"));
    }

    /// Reviews the Multiplayer screen from the main menu, with the servers of `menus_folder`: the
    /// list once every server answered, the Add server dialog and its error, the Remove and Edit
    /// dialogs, and the longest name and address, selected by the keyboard, in the pane and in
    /// the dialogs. Goes back to the main menu.
    fn review_servers(&mut self) {
        let card = |name: &str| L::role(Role::ListItem, name);
        self.click(&L::text("Multiplayer"));
        self.wait_for(&card(LONG_NAME), SCREEN);
        wait_for_answers(self);
        self.hover_at(Vec2::ZERO);
        self.settled_snapshot("multi");
        ctrl(self, KeyCode::KeyN);
        self.wait_for(&L::role(Role::Dialog, ADD_SERVER_TITLE), SCREEN);
        self.settled_snapshot("multi_add");
        self.press(KeyCode::Tab);
        self.type_text("10.0.0.2");
        self.wait_for(&L::text(SERVER_EXISTS), SCREEN);
        self.settled_snapshot("multi_add_error");
        self.press(KeyCode::Escape);
        // the focus is back on the card of Survival
        self.press(KeyCode::Delete);
        self.wait_for(&L::role(Role::Dialog, "Remove \"Survival\"?"), SCREEN);
        self.settled_snapshot("multi_remove");
        self.press(KeyCode::Escape);
        self.click(&L::role(Role::Button, "Edit"));
        self.wait_for(&L::role(Role::Dialog, "Edit \"Survival\""), SCREEN);
        self.settled_snapshot("multi_edit");
        self.press(KeyCode::Escape);

        // by the keyboard, for the focus ring around the selected card
        self.click(&card("Survival"));
        self.press(KeyCode::End);
        self.wait_for(
            &L::text(LONG_ADDRESS).within(L::marker::<ServerDetail>()),
            SCREEN,
        );
        self.hover_at(Vec2::ZERO);
        self.settled_snapshot("multi_long_name");
        self.press(KeyCode::Delete);
        self.wait_for(
            &L::role(Role::Dialog, &format!("Remove \"{LONG_NAME}\"?")),
            SCREEN,
        );
        self.settled_snapshot("multi_long_name_remove");
        self.press(KeyCode::Escape);
        self.click(&L::role(Role::Button, "Edit"));
        self.wait_for(
            &L::role(Role::Dialog, &format!("Edit \"{LONG_NAME}\"")),
            SCREEN,
        );
        self.settled_snapshot("multi_long_name_edit");
        self.press(KeyCode::Escape);
        self.click(&L::role(Role::Button, "Back"));
    }
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
#[ignore = "review snapshots: UI_SNAPSHOTS=1 cargo test -p client review -- --ignored"]
fn every_screen_at_three_resolutions() {
    // The menus, with one client that is resized and a world for the Continue card
    let mut h = Harness::start(menus_folder(), Options::default());
    h.world_mut().insert_resource(StatusProbe(fixed_status));
    h.wait_for(&L::text("Singleplayer"), STARTUP);
    for (width, height) in RESOLUTIONS {
        h.resize(width, height);
        h.wait_for(&L::marker::<ContinueCard>(), SCREEN);
        h.settled_snapshot("main");
        h.press(KeyCode::Escape);
        h.wait_for(&L::role(Role::Dialog, QUIT_TITLE), SCREEN);
        h.settled_snapshot("quit_dialog");
        h.press(KeyCode::Escape);
        h.review_worlds();
        h.wait_for(&L::text("Singleplayer"), SCREEN);
        h.review_servers();
        h.wait_for(&L::text("Settings"), SCREEN);
        h.click(&L::text("Settings"));
        h.wait_for(&L::text("Move forward"), SCREEN);
        h.settled_snapshot("settings_controls");
        h.click(&key_slot("Jump", "secondary"));
        h.wait_for(&L::text(PRESS_A_KEY), SCREEN);
        h.settled_snapshot("settings_capture");
        h.press(KeyCode::Escape);
        // The up arrow on three actions: the row of Move forward names the two others
        for action in ["Jump", "Move backward"] {
            h.click(&key_slot(action, "secondary"));
            h.wait_for(&L::text(PRESS_A_KEY), SCREEN);
            h.press(KeyCode::ArrowUp);
        }
        h.wait_for(
            &L::text_containing("Move backward").within(L::row("Move forward")),
            SCREEN,
        );
        // The longest message, on the row of Jump, is below the list at 800x600: scrolled to
        h.hover(&L::text("Move left"));
        h.scroll(-2.);
        h.settled_snapshot("settings_conflict");
        h.click(&L::role(Role::Button, "Reset to defaults"));
        h.wait_for(&L::role(Role::Dialog, "Reset key bindings?"), SCREEN);
        h.settled_snapshot("settings_reset");
        h.click(
            &L::role(Role::Button, "Reset").within(L::role(Role::Dialog, "Reset key bindings?")),
        );
        h.wait_until("the default bindings", SCREEN, |w| {
            w.resource::<KeyMap>().map == default_bindings().map
        });
        h.click(&L::role(Role::Tab, "Audio"));
        h.wait_for(&L::role(Role::Slider, "Master"), SCREEN);
        h.settled_snapshot("settings_audio");
        h.click(&L::role(Role::Tab, "Interface"));
        h.wait_for(&L::role(Role::RadioGroup, "UI scale"), SCREEN);
        h.settled_snapshot("settings_interface");
        h.click(&L::role(Role::Button, "Back"));
        h.wait_for(&L::text("Singleplayer"), SCREEN);
    }
    drop(h);

    // The Singleplayer screen without a world
    let mut h = Harness::start(GameFolder::new(), Options::default());
    h.wait_for(&L::text("Singleplayer"), STARTUP);
    h.click(&L::text("Singleplayer"));
    for (width, height) in RESOLUTIONS {
        h.resize(width, height);
        h.wait_for(&L::text("No worlds yet"), SCREEN);
        h.hover_at(Vec2::ZERO);
        h.settled_snapshot("solo_empty");
    }
    drop(h);

    // The Multiplayer screen without a server, then with a server list that cannot be read
    let folder = GameFolder::new().with_file(SERVER_LIST_SAVE_NAME, "[]");
    let mut h = Harness::start(folder.clone(), Options::default());
    h.world_mut().insert_resource(StatusProbe(fixed_status));
    h.wait_for(&L::text("Multiplayer"), STARTUP);
    h.click(&L::text("Multiplayer"));
    for (width, height) in RESOLUTIONS {
        h.resize(width, height);
        h.wait_for(&L::text("No servers yet"), SCREEN);
        h.hover_at(Vec2::ZERO);
        h.settled_snapshot("multi_empty");
    }
    h.click(&L::role(Role::Button, "Back"));
    fs::write(folder.path().join(SERVER_LIST_SAVE_NAME), "not a list").unwrap();
    h.wait_for(&L::text("Multiplayer"), SCREEN);
    h.click(&L::text("Multiplayer"));
    for (width, height) in RESOLUTIONS {
        h.resize(width, height);
        h.wait_for(&L::text("Cannot read servers.ron"), SCREEN);
        h.hover_at(Vec2::ZERO);
        h.settled_snapshot("multi_unreadable");
    }
    drop(h);

    // The game, with a world whose inventory is not empty
    let folder = GameFolder::new().with_world("Review").with_items(
        "Review",
        "Tester",
        &[
            (0, ItemId::Dirt, 64),
            (1, ItemId::Stone, 12),
            (2, ItemId::OakPlanks, 5),
            (10, ItemId::Sand, 33),
            (11, ItemId::OakLog, 8),
            (grid_slot(2, 2), ItemId::Cobblestone, 1),
        ],
    );
    let mut h = enter_world(folder, "Review");
    h.wait_until("the saved inventory", SERVER, |w| {
        slot(w, 2) == Some((ItemId::OakPlanks, 5))
    });
    for (width, height) in RESOLUTIONS {
        h.resize(width, height);
        // The name shows when the selection changes, so the slot 3 is left first
        h.press(KeyCode::Digit1);
        h.press(KeyCode::Digit3);
        h.snapshot_in_game("hud");

        h.press(KeyCode::KeyE);
        h.hover(&L::cell(10));
        h.snapshot_in_game("inventory");
        h.click(&L::cell(11));
        h.hover(&L::cell(20));
        h.snapshot_in_game("inventory_held");
        h.click(&L::cell(11));
        h.press(KeyCode::KeyE);

        h.press(KeyCode::KeyT);
        h.type_text("hello");
        h.snapshot_in_game("chat");
        h.press(KeyCode::Escape);

        h.press(KeyCode::Escape);
        h.wait_for(&L::text("Resume"), SCREEN);
        h.snapshot_in_game("pause");
        h.click(&L::text("Settings"));
        h.wait_for(&L::text("Move forward"), SCREEN);
        h.snapshot_in_game("settings_in_game");
        h.press(KeyCode::Escape);
        h.press(KeyCode::Escape);
    }
}
