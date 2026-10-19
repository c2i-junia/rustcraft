//! The pause menu: opening and closing it, the keyboard, the settings over it, saving, and
//! leaving a world or a server.

use std::fs;
use std::path::Path;
use std::time::{Instant, SystemTime};

use accesskit::Role;
use bevy::{
    a11y::AccessibilityNode,
    input::keyboard::KeyCode,
    input_focus::InputFocus,
    prelude::*,
    ui::{InteractionDisabled, UiGlobalTransform},
};

use super::connecting::spawn_test_server;
use super::game::{enter_world, enter_world_with};
use super::harness::{Harness, Options, SCREEN, SERVER, STARTUP};
use super::helpers::is_in;
use super::locator::{is_inside, is_shown, Locator as L};
use super::multi::selected_server;
use super::seed::GameFolder;
use crate::constants::SAVE_PATH;
use crate::network::hash_string_to_u64;
use crate::state::GameState;
use crate::ui::hud::hotbar::Hotbar;
use crate::ui::hud::{HudBottom, InventoryCell, InventoryRoot, UIMode};
use crate::ui::menus::pause::{PauseButton, PauseMenu, PAUSE_TITLE, SAVING, WORLD_SAVED};
use crate::ui::menus::settings::controls::PRESS_A_KEY;
use crate::ui::menus::settings::SettingsRoot;
use crate::ui::menus::MenuState;
use crate::ui::theme::{ACCENT, RAISE, RAISE_HOVER};
use crate::ui::widgets::hints::HintBar;

/// The pause menu, found by its role and title.
fn pause_menu() -> L {
    L::role(Role::Dialog, PAUSE_TITLE)
}

/// A button, found by its role and label.
fn button(label: &str) -> L {
    L::role(Role::Button, label)
}

/// The rectangle of `entity`, in physical pixels.
fn rect(world: &World, entity: Entity) -> Rect {
    let center = world.get::<UiGlobalTransform>(entity).unwrap().translation;
    let size = world.get::<ComputedNode>(entity).unwrap().size();
    Rect::from_center_size(center, size)
}

impl Harness {
    /// How many entities carry `C`, shown or not.
    fn count<C: Component>(&mut self) -> usize {
        let world = self.world_mut();
        world.query_filtered::<(), With<C>>().iter(world).count()
    }

    /// The focused entity.
    #[track_caller]
    fn focused(&self) -> Entity {
        self.world()
            .resource::<InputFocus>()
            .get()
            .expect("something has the focus")
    }

    /// The colour of the focus ring of the focused entity.
    #[track_caller]
    fn focus_ring(&self) -> Color {
        self.world().get::<Outline>(self.focused()).unwrap().color
    }

    /// Whether the focus is inside the pause menu.
    #[track_caller]
    fn focus_in_pause_menu(&mut self) -> bool {
        let focused = self.world().resource::<InputFocus>().get();
        let world = self.world_mut();
        let menu = world
            .query_filtered::<Entity, With<PauseMenu>>()
            .single(world)
            .unwrap();
        focused.is_some_and(|focused| is_inside(world, focused, menu))
    }
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn the_pause_menu_opens_on_back_to_game_and_escape_returns_to_the_game() {
    let mut h = enter_world(GameFolder::new().with_world("Pause"), "Pause");
    // nothing to aim at, and no hotbar beneath the hints, from the frame the menu first shows
    h.key_down(KeyCode::Escape);
    assert!(h.is_visible(&pause_menu()));
    assert!(!h.is_visible(&L::marker::<HudBottom>()));
    assert_eq!(h.reticle_bars(), 0);
    h.key_up(KeyCode::Escape);
    // the world under the title
    assert!(h.is_visible(&L::text("Pause").within(pause_menu())));
    assert_eq!(h.focus_label().as_deref(), Some("Back to game"));
    let labels = [
        "Back to game",
        "Settings",
        "Save now",
        "Save and quit to title",
    ];
    let heights: Vec<f32> = labels
        .iter()
        .map(|label| h.center_of(&button(label).within(pause_menu())).y)
        .collect();
    assert!(
        heights.windows(2).all(|pair| pair[0] < pair[1]),
        "{heights:?}"
    );
    assert!(h.is_visible(&L::text("Esc").within(L::marker::<HintBar>())));

    // the hotbar and the reticle are back in the frame the menu is gone
    h.key_down(KeyCode::Escape);
    assert_eq!(h.count::<PauseMenu>(), 0);
    assert!(h.is_visible(&L::marker::<HudBottom>()));
    assert_eq!(h.reticle_bars(), 2);
    h.key_up(KeyCode::Escape);
    assert!(*h.world().resource::<UIMode>() == UIMode::Closed);
    // the Escape that closed it does not open it again
    h.frames(2);
    assert_eq!(h.count::<PauseMenu>(), 0);
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn the_keyboard_alone_drives_the_pause_menu() {
    let mut h = enter_world(GameFolder::new().with_world("Keys"), "Keys");
    h.press(KeyCode::Escape);
    h.wait_for(&pause_menu(), SCREEN);
    h.press(KeyCode::ArrowDown);
    assert_eq!(h.focus_label().as_deref(), Some("Settings"));
    assert_eq!(h.focus_ring(), ACCENT);

    // Enter opens the settings, and Escape gives the focus back to Settings
    h.press(KeyCode::Enter);
    h.wait_for(&L::role(Role::Tab, "Controls"), SCREEN);
    h.press(KeyCode::Escape);
    assert_eq!(h.count::<SettingsRoot>(), 0);
    assert_eq!(h.count::<PauseMenu>(), 1);
    let focused = h.focused();
    assert_eq!(
        h.world().get::<PauseButton>(focused),
        Some(&PauseButton::Settings)
    );
    assert_eq!(h.focus_ring(), ACCENT);
    h.press(KeyCode::Enter);
    h.wait_for(&L::role(Role::Tab, "Controls"), SCREEN);
    h.press(KeyCode::Escape);

    // opened by a click, too
    h.click(&button("Settings"));
    h.wait_for(&L::role(Role::Tab, "Controls"), SCREEN);
    h.press(KeyCode::Escape);
    let focused = h.focused();
    assert_eq!(
        h.world().get::<PauseButton>(focused),
        Some(&PauseButton::Settings)
    );

    // Tab stays in the menu, and so does a click beside its buttons
    for _ in 0..5 {
        h.press(KeyCode::Tab);
        assert!(h.focus_in_pause_menu());
    }
    h.click_at(Vec2::new(10., 10.));
    assert!(h.focus_in_pause_menu());

    for _ in 0..4 {
        h.press(KeyCode::ArrowUp);
    }
    assert_eq!(h.focus_label().as_deref(), Some("Back to game"));
    h.press(KeyCode::Enter);
    assert_eq!(h.count::<PauseMenu>(), 0);
    assert!(*h.world().resource::<UIMode>() == UIMode::Closed);
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn the_settings_over_the_pause_menu_leave_no_hotbar_beneath_their_hints() {
    let small = Options {
        width: 800,
        height: 600,
        ..default()
    };
    let mut h = enter_world_with(GameFolder::new().with_world("Hints"), "Hints", small);
    h.press(KeyCode::Escape);
    h.wait_for(&pause_menu(), SCREEN);
    h.click(&button("Settings"));
    h.wait_for(&L::role(Role::Tab, "Controls"), SCREEN);

    let hints = h.get(&L::marker::<HintBar>().within(L::marker::<SettingsRoot>()));
    let mut hud = h.find(&L::marker::<Hotbar>());
    hud.extend(h.find(&L::marker::<InventoryCell>()));
    let world = h.world();
    let bar = rect(world, hints);
    for entity in hud {
        let overlap = rect(world, entity).intersect(bar);
        assert!(overlap.is_empty(), "{entity} shows beneath the hints");
    }
    assert!(!h.is_visible(&L::marker::<HudBottom>()));

    h.press(KeyCode::Escape);
    h.press(KeyCode::Escape);
    assert!(h.is_visible(&L::marker::<HudBottom>()));
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn the_pause_menu_resumes_saves_and_leaves() {
    let mut h = enter_world(GameFolder::new().with_world("Pause"), "Pause");
    h.press(KeyCode::Escape);
    h.wait_for(&button("Back to game"), SCREEN);
    h.snapshot("pause");
    h.click(&button("Back to game"));
    assert!(!h.is_visible(&button("Back to game")));

    let player = h
        .folder()
        .join(SAVE_PATH)
        .join("Pause/players")
        .join(format!("{}.ron", hash_string_to_u64("Tester")));
    let meta = h.folder().join(SAVE_PATH).join("Pause/world.ron");
    let written = || fs::metadata(&meta).unwrap().modified().unwrap();
    let seeded = written();

    // The settings open over the game, which goes on beneath them
    h.press(KeyCode::Escape);
    let save = h.get(&button("Save now"));
    let surface = |h: &Harness| h.world().get::<BackgroundColor>(save).unwrap().0;
    h.hover(&button("Save now"));
    h.frames(1);
    assert_eq!(surface(&h), RAISE_HOVER);
    h.click(&button("Settings"));
    h.wait_for(&L::role(Role::Tab, "Controls"), SCREEN);
    assert!(is_in(h.world(), GameState::Game));
    assert!(*h.world().resource::<UIMode>() == UIMode::Opened);
    // The buttons of the pause menu beneath them are out of the cursor's reach
    h.hover(&button("Save now"));
    h.frames(1);
    assert_eq!(surface(&h), RAISE);
    // A key bound in the settings does not also do what it does in the game
    h.click(&L::role(Role::Button, "Jump, secondary key"));
    h.wait_for(&L::text(PRESS_A_KEY), SCREEN);
    h.press(KeyCode::KeyE);
    h.wait_for(&L::text("E").within(L::row("Jump")), SCREEN);
    assert!(!h.is_visible(&L::marker::<InventoryRoot>()));
    h.press(KeyCode::Delete);
    h.click(&L::role(Role::Tab, "Audio"));
    h.wait_for(&L::role(Role::Slider, "Master"), SCREEN);
    // Escape closes the settings, and only them
    h.press(KeyCode::Escape);
    assert!(!h.is_visible(&L::marker::<SettingsRoot>()));
    h.frames(2);
    assert!(h.is_visible(&button("Back to game")));
    h.click(&button("Back to game"));
    // No click on the settings reached the "Save now" beneath them
    assert!(!player.exists());
    h.press(KeyCode::Escape);
    save_now_says_when_the_world_is_saved(&mut h, &player, written, seeded);

    // Leaving saves the world again: world.ron is written after the click
    let before = written();
    h.click(&button("Save and quit to title"));
    h.wait_until("the main menu", SERVER, |w| {
        is_in(w, GameState::Menu) && *w.resource::<State<MenuState>>().get() == MenuState::Main
    });
    h.wait_until("world.ron written", SERVER, |_| written() > before);
}

/// Save now reads "Saving..." and is disabled until the world is written, then says so in a
/// toast. The player is written at once by the tick and world.ron a little later, on another
/// thread, before the chunks: both are on disk by the time the toast shows.
fn save_now_says_when_the_world_is_saved(
    h: &mut Harness,
    player: &Path,
    written: impl Fn() -> SystemTime,
    seeded: SystemTime,
) {
    h.click(&button("Save now"));
    let saving = h.get(&button(SAVING));
    assert!(h.world().get::<InteractionDisabled>(saving).is_some());
    let saved = L::role(Role::Status, WORLD_SAVED);
    h.wait_for(&saved, SERVER);
    h.snapshot("world saved");
    let toast = h.get(&saved);
    assert!(player.exists());
    assert!(written() > seeded);
    let save = h.get(&button("Save now"));
    assert_eq!(save, saving);
    assert!(h.world().get::<InteractionDisabled>(save).is_none());
    assert_eq!(h.focused(), save);

    // and again from the keyboard
    h.press(KeyCode::Enter);
    let start = Instant::now();
    while h.find(&saved).iter().all(|&found| found == toast) {
        assert!(start.elapsed() < SERVER, "no second toast");
        h.frames(1);
    }
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn the_pause_menu_of_a_server_disconnects_back_to_its_card() {
    let (addr, server) = spawn_test_server("Local test");
    let ip = format!("127.0.0.1:{}", addr.port());
    let folder = GameFolder::new().with_servers(&[("Local test", &ip)]);
    let mut h = Harness::start(folder, Options::default());
    h.wait_for(&L::text("Multiplayer"), STARTUP);
    h.click(&L::text("Multiplayer"));
    h.join_server_card("Local test");
    h.wait_for_game_state(GameState::Game, SERVER);

    // A server is not the player's to save
    h.press(KeyCode::Escape);
    h.wait_for(&button("Disconnect").within(pause_menu()), SCREEN);
    assert!(!h.is_visible(&button("Save now")));
    assert!(!h.is_visible(&button("Save and quit to title")));

    h.click(&button("Disconnect"));
    h.wait_until("the server list", SERVER, |w| {
        is_in(w, GameState::Menu) && *w.resource::<State<MenuState>>().get() == MenuState::Multi
    });
    h.frames(2);
    assert_eq!(selected_server(&mut h).as_deref(), Some("Local test"));
    assert_eq!(h.focus_label().as_deref(), Some("Local test"));
    let world = h.world_mut();
    let dialogs: Vec<Entity> = world
        .query::<(Entity, &AccessibilityNode)>()
        .iter(world)
        .filter(|(_, node)| node.role() == Role::Dialog)
        .map(|(entity, _)| entity)
        .collect();
    assert!(!dialogs.iter().any(|&dialog| is_shown(world, dialog)));

    // our `Exit` stopped the server
    let start = Instant::now();
    while !server.is_finished() {
        assert!(start.elapsed() < SERVER, "the server still runs");
        h.frames(1);
    }
    assert_eq!(server.join().unwrap(), Ok(()));
}
