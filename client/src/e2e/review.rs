//! Snapshots of every screen at three resolutions, for a human to review.

use bevy::{input::keyboard::KeyCode, prelude::*};
use shared::world::ItemId;

use super::game::enter_world;
use super::harness::{Harness, SCREEN};
use super::locator::Locator as L;
use super::seed::GameFolder;

/// The resolutions the screens are reviewed at.
const RESOLUTIONS: [(u32, u32); 3] = [(800, 600), (1280, 720), (1920, 1080)];

impl Harness {
    /// Runs frames until every image of the UI is loaded, then saves the snapshot `name`.
    fn settled_snapshot(&mut self, name: &str) {
        self.wait_until("the images of the UI", SCREEN, |w| {
            let server = w.resource::<AssetServer>();
            let mut nodes = w.try_query::<&ImageNode>().unwrap();
            nodes
                .iter(w)
                .all(|node| server.is_loaded_with_dependencies(&node.image))
        });
        self.snapshot(name);
    }

    /// Lets the game draw a few frames, then saves the snapshot `name`. Some images of the game
    /// never count as loaded, unlike those of the menus, so only the frames are waited for.
    fn snapshot_in_game(&mut self, name: &str) {
        self.frames(10);
        self.snapshot(name);
    }

    /// Opens the screen behind `button`, waits for `title`, takes the snapshot `name`, and goes
    /// back with the button `back`.
    fn review_screen(&mut self, button: &str, title: &str, name: &str, back: &str) {
        self.click(&L::text(button));
        self.wait_for(&L::text(title), SCREEN);
        self.settled_snapshot(name);
        self.click(&L::text(back));
    }
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
#[ignore = "review snapshots: UI_SNAPSHOTS=1 cargo test -p client review -- --ignored"]
fn every_screen_at_three_resolutions() {
    // The menus, with one client that is resized
    let mut h = Harness::new();
    h.wait_for(&L::text("Singleplayer"), SCREEN);
    for (width, height) in RESOLUTIONS {
        h.resize(width, height);
        h.settled_snapshot("main");
        h.review_screen("Singleplayer", "Create world", "solo", "Back to menu");
        h.wait_for(&L::text("Singleplayer"), SCREEN);
        h.review_screen("Multiplayer", "Server list", "multi", "Back to menu");
        h.wait_for(&L::text("Settings"), SCREEN);
        h.click(&L::text("Settings"));
        h.wait_for(&L::text("Controls"), SCREEN);
        h.settled_snapshot("settings");
        h.review_screen("Controls", "Keyboard Controls", "controls", "Back");
        h.wait_for(&L::text("Controls"), SCREEN);
        h.review_screen("Audio", "Master", "audio", "Back");
        h.wait_for(&L::text("Controls"), SCREEN);
        h.click(&L::text("Back"));
        h.wait_for(&L::text("Singleplayer"), SCREEN);
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
        ],
    );
    let mut h = enter_world(folder, "Review");
    for (width, height) in RESOLUTIONS {
        h.resize(width, height);
        h.snapshot_in_game("hud");

        h.press(KeyCode::KeyE);
        h.snapshot_in_game("inventory");
        h.press(KeyCode::KeyE);

        h.press(KeyCode::KeyT);
        h.type_text("hello");
        h.snapshot_in_game("chat");
        h.press(KeyCode::Escape);

        h.press(KeyCode::Escape);
        h.wait_for(&L::text("Resume"), SCREEN);
        h.snapshot_in_game("pause");
        h.press(KeyCode::Escape);
    }
}
