//! Snapshots of every screen at four resolutions, for a human to review.

use std::fs;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant, SystemTime};

use accesskit::Role;
use bevy::asset::AssetServer;
use bevy::input::keyboard::KeyCode;
use bevy::input::mouse::MouseButton;
use bevy::math::Vec2;
use bevy::prelude::{Entity, With};
use bevy::time::{Real, Time};
use server::WORLD_FILE;
use shared::world::{grid_slot, ItemId};

use super::game::{enter_world, hold_click, place_ahead, slot};
use super::harness::{Harness, Options, SCREEN, SERVER, STARTUP};
use super::helpers::{ctrl, ctrl_n};
use super::locator::Locator as L;
use super::multi::wait_for_answers;
use super::seed::{fixed_status, GameFolder};
use super::settings::key_slot;
use crate::camera::CameraController;
use crate::constants::{SAVE_PATH, SERVER_LIST_SAVE_NAME};
use crate::input::keyboard::default_bindings;
use crate::network::address::{AddressResolver, ServerAddress};
use crate::network::leave::Leaving;
use crate::network::session::{ConnectStep, ConnectWatch, SessionOrigin};
use crate::network::status::{ServerStatus, StatusProbe};
use crate::network::{CurrentPlayerProfile, LocalServerHandle, TargetServer};
use crate::state::{GameState, KeyMap};
use crate::ui::hud::chat::{ChatLine, CHAT_FADE, CHAT_FADE_AFTER};
use crate::ui::hud::hotbar::Hotbar;
use crate::ui::hud::item_browser::layout::BrowserLayout;
use crate::ui::hud::{UIMode, FURNACE_TITLE};
use crate::ui::menus::connecting::{ConnectingCard, BLINK};
use crate::ui::menus::multi::dialogs::ADD_SERVER_TITLE;
use crate::ui::menus::multi::ServerDetail;
use crate::ui::menus::pause::WORLD_SAVED;
use crate::ui::menus::servers::SERVER_EXISTS;
use crate::ui::menus::settings::controls::PRESS_A_KEY;
use crate::ui::menus::worlds::WORLD_EXISTS;
use crate::ui::menus::{ContinueCard, QUIT_TITLE};
use crate::ui::quit::spawn_quitting_card;
use crate::world::ClientWorldMap;
use shared::DAY_DURATION_IN_TICKS;

/// The resolutions the screens are reviewed at: the smallest, the design height, and `UiScale`
/// 1.5 and 2.
const RESOLUTIONS: [(u32, u32); 4] = [(800, 600), (1280, 720), (1920, 1080), (2560, 1440)];

/// A day, in seconds.
const DAY: u64 = 24 * 60 * 60;

/// The longest name a world can have: 32 of the widest letter.
const LONG_NAME: &str = "WWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWW";

/// The longest address a server can be given: 64 characters, of the widest letter.
const LONG_ADDRESS: &str = "wwwwwwwwwwwwwwwwwwwwwwwwwwwww.wwwwwwwwwwwwwwwwwwwwwwwwwwww:65535";

/// The discard port of this computer, where nothing answers.
const DISCARD: SocketAddr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 9);

/// Every server is offline: an answer is only looked for when a join gets none.
fn offline(_: &str) -> ServerStatus {
    ServerStatus::Offline
}

/// Every host name is the discard port, so that the longest address can be joined.
fn to_discard(_: &ServerAddress) -> std::io::Result<SocketAddr> {
    Ok(DISCARD)
}

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
    /// Runs frames until every image of the UI is loaded, checks that every control shows the
    /// focus, then saves the snapshot `name`.
    #[track_caller]
    fn settled_snapshot(&mut self, name: &str) {
        self.wait_for_ui_images();
        self.assert_focus_rings();
        self.snapshot(name);
    }

    /// Lets the game draw a few frames, checks that every control shows the focus, then saves the
    /// snapshot `name`. The atlas of the items is built at runtime and never counts as loaded,
    /// unlike the images of the menus, so `wait_for_ui_images` would time out: only the frames
    /// are waited for.
    #[track_caller]
    fn snapshot_in_game(&mut self, name: &str) {
        self.frames(10);
        self.assert_focus_rings();
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

    /// Shows the connecting card at `step` and saves the snapshot `name`. No session stays at a
    /// step long enough to be looked at, so the session is not tracked while the card is shown:
    /// its origin is set aside, then put back.
    fn connecting_snapshot(&mut self, step: ConnectStep, name: &str) {
        let origin = self.world_mut().remove_resource::<SessionOrigin>().unwrap();
        self.world_mut().resource_mut::<ConnectWatch>().step = step;
        self.wait_for_ui_images();
        // the marker of the current step blinks: the snapshot is taken early in its shown half
        self.wait_until("the marker of the current step shown", SCREEN, |w| {
            w.resource::<Time<Real>>().elapsed_secs() % (2.0 * BLINK) < BLINK / 4.0
        });
        self.snapshot(name);
        self.world_mut().insert_resource(origin);
    }

    /// Reviews the connecting card of a session at each of its steps, the first one `first`,
    /// from its first frame, then cancels it. `prefix` names the snapshots.
    fn review_connecting(&mut self, first: ConnectStep, prefix: &str) {
        self.wait_for_game_state(GameState::PreGameLoading, SCREEN);
        self.wait_for(&L::marker::<ConnectingCard>(), SCREEN);
        self.hover_at(Vec2::ZERO);
        let chunks = ConnectStep::LoadingChunks {
            received: 12,
            total: 27,
        };
        self.connecting_snapshot(first, prefix);
        self.connecting_snapshot(chunks, &format!("{prefix}_chunks"));
        self.connecting_snapshot(ConnectStep::Spawning, &format!("{prefix}_spawning"));
        self.click(&L::role(Role::Button, "Cancel"));
        self.wait_for_game_state(GameState::Menu, SCREEN);
    }

    /// Resizes the window to each resolution and saves the snapshot `name` of the error card
    /// titled `title`.
    fn review_error_card(&mut self, title: &str, name: &str) {
        self.wait_for(&L::role(Role::Dialog, title), SERVER);
        self.hover_at(Vec2::ZERO);
        for (width, height) in RESOLUTIONS {
            self.resize(width, height);
            self.settled_snapshot(name);
        }
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

    // The connecting card of a server and of a world of the longest name, the server at the
    // longest address, at each step; then the error cards: with Back only or with Try again,
    // of a server and of a world
    let folder = GameFolder::new()
        .with_servers(&[("Nowhere", "127.0.0.1:9"), (LONG_NAME, LONG_ADDRESS)])
        .with_broken_world(LONG_NAME);
    let mut h = Harness::start(folder, Options::default());
    h.world_mut().insert_resource(StatusProbe(offline));
    h.world_mut().insert_resource(AddressResolver(to_discard));
    h.wait_for(&L::text("Multiplayer"), STARTUP);
    h.click(&L::text("Multiplayer"));
    for (width, height) in RESOLUTIONS {
        h.resize(width, height);
        h.join_server_card(LONG_NAME);
        h.review_connecting(ConnectStep::Connecting, "connecting");
    }
    // a name no server accepts
    let name = std::mem::take(&mut h.world_mut().resource_mut::<CurrentPlayerProfile>().name);
    h.join_server_card("Nowhere");
    h.review_error_card("Your name is not accepted", "connect_error_name");
    h.world_mut().resource_mut::<CurrentPlayerProfile>().name = name;
    h.press(KeyCode::Escape);
    // the one wait for a server that never answers
    h.join_server_card("Nowhere");
    h.review_error_card("Cannot reach \"Nowhere\"", "connect_error_no_answer");
    h.press(KeyCode::Escape);
    h.wait_for(&L::role(Role::Button, "Back"), SCREEN);
    h.click(&L::role(Role::Button, "Back"));
    h.wait_for(&L::text("Singleplayer"), SCREEN);
    h.click(&L::text("Singleplayer"));
    for (width, height) in RESOLUTIONS {
        h.resize(width, height);
        // a session at an address starts no server: the world stays at its first step
        h.world_mut().resource_mut::<TargetServer>().address = Some(DISCARD);
        h.play_world(LONG_NAME);
        h.review_connecting(ConnectStep::StartingWorld, "connecting_world");
    }
    // the server of the last session still runs
    let (stop, stopped) = mpsc::channel::<()>();
    let running = thread::spawn(move || {
        let _ = stopped.recv();
        Ok(())
    });
    h.world_mut()
        .insert_resource(LocalServerHandle(Some(running)));
    h.play_world(LONG_NAME);
    let cannot_open = format!("Cannot open \"{LONG_NAME}\"");
    h.wait_for(&L::text_containing("still closing"), SERVER);
    h.review_error_card(&cannot_open, "connect_error_world_busy");
    drop(stop);
    h.press(KeyCode::Escape);
    h.wait_until("the last server stopped", SCREEN, |w| {
        w.resource::<LocalServerHandle>().0.is_none()
    });
    // a world that cannot be read, with the path of its file in the reason
    h.play_world(LONG_NAME);
    h.wait_for(&L::text_containing("could not be loaded"), SERVER);
    h.review_error_card(&cannot_open, "connect_error_world_long");
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
            (3, ItemId::Furnace, 1),
            (12, ItemId::Sand, 8),
            (13, ItemId::Coal, 2),
        ],
    );
    let mut h = enter_world(folder, "Review");
    h.wait_until("the saved inventory", SERVER, |w| {
        slot(w, 2) == Some((ItemId::OakPlanks, 5))
    });
    // A furnace is opened at each size, looking down at it; the other screens keep the view of
    // the horizon
    let world = h.world_mut();
    let camera = world
        .query_filtered::<Entity, With<CameraController>>()
        .single(world)
        .unwrap();
    let horizon = world.get::<CameraController>(camera).unwrap().angle_y;
    let look = |h: &mut Harness, angle: f32| {
        h.world_mut()
            .get_mut::<CameraController>(camera)
            .unwrap()
            .angle_y = angle;
        h.frames(2);
    };
    let mut furnace = None;
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

        // Placed ahead of the player at the first size, once the leftover of the grid was
        // shown, then aimed at again
        let placed = match furnace {
            Some((_, pitch)) => {
                look(&mut h, pitch);
                None
            }
            None => {
                h.press(KeyCode::Digit4);
                let placed = place_ahead(&mut h);
                let pitch = h.world().get::<CameraController>(camera).unwrap().angle_y;
                furnace = Some((placed, pitch));
                Some(placed)
            }
        };
        hold_click(&mut h, &L::marker::<Hotbar>(), MouseButton::Right, 1);
        h.wait_for(&L::text(FURNACE_TITLE), SCREEN);
        // Lit with sand and coal, smelting from then on
        if let Some(placed) = placed {
            h.key_down(KeyCode::ShiftLeft);
            h.click(&L::cell(12));
            h.click(&L::cell(13));
            h.key_up(KeyCode::ShiftLeft);
            h.wait_until("the furnace lit", SERVER, |w| {
                w.resource::<ClientWorldMap>()
                    .blocks
                    .furnace(placed)
                    .is_some_and(|furnace| furnace.is_lit())
            });
        }
        h.hover(&L::cell(10));
        h.snapshot_in_game("furnace");
        // The uses of a log beside it: its planks, its charcoal and its burning
        h.hover(&L::cell(11));
        h.press(KeyCode::KeyU);
        h.wait_for(&L::text("Burns for 1.5 items"), SCREEN);
        h.hover_at(Vec2::ZERO);
        h.snapshot_in_game("furnace_log_uses");
        // its cards scrolled to the last one
        h.hover(&L::text("Furnace · 10 s"));
        h.scroll(-5.);
        h.hover_at(Vec2::ZERO);
        h.snapshot_in_game("furnace_log_uses_scrolled");
        h.press(KeyCode::Escape);
        if h.world().resource::<BrowserLayout>().compact {
            h.press(KeyCode::Escape);
        }
        h.press(KeyCode::KeyE);
        look(&mut h, horizon);

        h.press(KeyCode::Escape);
        h.wait_for(&L::role(Role::Button, "Back to game"), SCREEN);
        h.snapshot_in_game("pause");
        h.click(&L::text("Settings"));
        h.wait_for(&L::text("Move forward"), SCREEN);
        h.snapshot_in_game("settings_in_game");
        // the focus back on Settings, with its ring
        h.press(KeyCode::Escape);
        h.wait_for(&L::role(Role::Button, "Back to game"), SCREEN);
        h.snapshot_in_game("pause_after_settings");
        h.click(&L::role(Role::Button, "Save now"));
        h.wait_for(&L::role(Role::Status, WORLD_SAVED), SERVER);
        h.snapshot_in_game("toast_saved");
        // leaving, as Save and quit to title shows it, without leaving
        h.world_mut().insert_resource(Leaving {
            since: Instant::now(),
        });
        h.snapshot_in_game("pause_saving");
        h.world_mut().remove_resource::<Leaving>();
        h.press(KeyCode::Escape);
        h.wait_until("the pause menu closed", SCREEN, |w| {
            *w.resource::<UIMode>() == UIMode::Closed
        });

        h.receive_chat(0..30);
        h.press(KeyCode::KeyT);
        h.type_text("hello");
        h.snapshot_in_game("chat");
        h.press(KeyCode::Escape);
        h.snapshot_in_game("chat_recent");
        h.type_text("/");
        h.snapshot_in_game("chat_slash");
        // The suggestions of the completion go away first, then the chat
        h.press(KeyCode::Escape);
        h.press(KeyCode::Escape);
        h.wait_until("the chat closed", SCREEN, |w| {
            *w.resource::<UIMode>() == UIMode::Closed
        });

        let assets = h.world().resource::<AssetServer>().clone();
        let card = spawn_quitting_card(&mut h.world_mut().commands(), &assets);
        h.world_mut().flush();
        h.snapshot_in_game("quitting");
        h.world_mut().despawn(card);

        // the lines received have faded by the next size, whose HUD is bare
        let faded = Instant::now() - (CHAT_FADE_AFTER + CHAT_FADE);
        let world = h.world_mut();
        for mut line in world.query::<&mut ChatLine>().iter_mut(world) {
            line.received = faded;
        }
        h.frames(1);
    }
}
