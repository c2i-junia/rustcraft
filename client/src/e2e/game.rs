//! The game, through the integrated server: the connection screen, the HUD, the hotbar, the
//! inventory and its crafting grid, the chat, the pause menu, and leaving.

use std::fs;

use accesskit::Role;

use bevy::{
    input::{keyboard::KeyCode, mouse::MouseButtonInput, ButtonState},
    pbr::wireframe::WireframeConfig,
    prelude::*,
    ui::ComputedStackIndex,
    window::{PrimaryWindow, WindowEvent},
};
use shared::players::{Inventory, ViewMode};
use shared::world::{grid_slot, raycast, FaceDirectionExt, ItemId};

use super::harness::{Harness, Options, SCREEN, SERVER, STARTUP};
use super::helpers::is_in;
use super::locator::{is_inside, is_shown, Locator as L};
use super::seed::GameFolder;
use crate::camera::CameraController;
use crate::constants::SAVE_PATH;
use crate::network::hash_string_to_u64;
use crate::player::CurrentPlayerMarker;
use crate::state::GameState;
use crate::ui::hud::chat::{ChatDisplay, ChatInput, ChatRoot};
use crate::ui::hud::debug::HudRoot;
use crate::ui::hud::hotbar::Hotbar;
use crate::ui::hud::{
    CraftResultCell, FloatingStack, InventoryCell, InventoryDialog, InventoryRoot, UIMode,
};
use crate::ui::menus::pause::PauseMenu;
use crate::ui::menus::settings::controls::PRESS_A_KEY;
use crate::ui::menus::settings::SettingsRoot;
use crate::ui::menus::MenuState;
use crate::ui::widgets::slot::SlotIcon;
use crate::world::{ClientWorldMap, MaterialResource};

/// A client in the game, in the world `world` of `folder`, with its hotbar on screen.
#[track_caller]
pub fn enter_world(folder: GameFolder, world: &str) -> Harness {
    enter_world_with(folder, world, Options::default())
}

/// A client started with `options` in the game, in the world `world` of `folder`, with its
/// hotbar on screen.
#[track_caller]
pub fn enter_world_with(folder: GameFolder, world: &str, options: Options) -> Harness {
    let mut h = Harness::start(folder, options);
    h.wait_for(&L::text("Singleplayer"), STARTUP);
    h.click(&L::text("Singleplayer"));
    h.play_world(world);
    h.wait_for_game_state(GameState::Game, SERVER);
    h.wait_for(&L::marker::<Hotbar>(), SCREEN);
    h
}

/// The item and the count of a stack, which is all a player sees of it.
type Shown = Option<(ItemId, u32)>;

/// What the inventory of the client holds in slot `id`.
pub(super) fn slot(w: &World, id: u32) -> Shown {
    let stack = w.resource::<Inventory>().inner.get(&id).copied();
    stack.map(|stack| (stack.item_id, stack.nb))
}

/// What the cursor holds.
pub(super) fn floating(w: &World) -> Shown {
    let stack = w.resource::<Inventory>().floating;
    stack.map(|stack| (stack.item_id, stack.nb))
}

impl Harness {
    /// The visible nodes drawn as the two bars of the reticle, 20 by 2 pixels and 2 by 20.
    fn reticle_bars(&mut self) -> usize {
        let world = self.world_mut();
        let bars: Vec<Entity> = world
            .query::<(Entity, &ComputedNode)>()
            .iter(world)
            .filter(|(_, node)| {
                let size = node.size();
                size == Vec2::new(20., 2.) || size == Vec2::new(2., 20.)
            })
            .map(|(entity, _)| entity)
            .collect();
        bars.into_iter().filter(|&bar| is_shown(world, bar)).count()
    }

    /// Whether a visible line of the chat contains `part`. The lines stay on screen for a
    /// while after the chat is closed, while the chat itself is hidden.
    #[track_caller]
    fn chat_shows(&mut self, part: &str) -> bool {
        let lines = self.find(&L::text_containing(part));
        let world = self.world_mut();
        let display = world
            .query_filtered::<Entity, With<ChatDisplay>>()
            .single(world)
            .unwrap();
        lines
            .into_iter()
            .any(|line| is_inside(world, line, display))
    }

    /// The place in the UI stack of the one node carrying `C`, higher being nearer the viewer.
    #[track_caller]
    fn stack_index<C: Component>(&mut self) -> u32 {
        let world = self.world_mut();
        world
            .query_filtered::<&ComputedStackIndex, With<C>>()
            .single(world)
            .unwrap()
            .0
    }

    /// The index in the items atlas of the icon shown by the result cell.
    #[track_caller]
    fn result_icon(&mut self) -> Option<usize> {
        let icon = self.get(&L::marker::<SlotIcon>().within(L::marker::<CraftResultCell>()));
        self.world()
            .get::<ImageNode>(icon)?
            .texture_atlas
            .as_ref()
            .map(|atlas| atlas.index)
    }

    /// The index in the items atlas of the icon of `item`.
    fn icon_index(&self, item: ItemId) -> Option<usize> {
        let atlas = self.world().resource::<MaterialResource>().items.as_ref()?;
        Some(atlas.sprite(&format!("{item:?}"))?.index)
    }
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn the_connection_screen_shows_then_cancel_returns_to_the_menu() {
    // Nothing answers on the discard port
    let folder = GameFolder::new().with_servers(&[("Nowhere", "127.0.0.1:9")]);
    let mut h = Harness::start(folder, Options::default());
    h.wait_for(&L::text("Multiplayer"), STARTUP);
    h.click(&L::text("Multiplayer"));
    h.join_server_card("Nowhere");
    h.wait_for(&L::text_containing("Connecting to server"), SCREEN);
    h.click(&L::text("[Cancel]"));
    h.wait_for_game_state(GameState::Menu, SCREEN);
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn the_hud_shows_and_f3_toggles_the_debug_overlay() {
    let mut h = enter_world(GameFolder::new().with_world("Hud"), "Hud");
    let cells = L::marker::<InventoryCell>().within(L::marker::<Hotbar>());
    assert_eq!(h.find(&cells).len(), 9);
    assert_eq!(h.reticle_bars(), 2);

    // The overlay starts hidden, F3 shows it and hides it again
    let overlay = L::marker::<HudRoot>();
    assert!(!h.is_visible(&overlay));
    h.press(KeyCode::F3);
    assert!(h.is_visible(&overlay));
    h.press(KeyCode::F3);
    assert!(!h.is_visible(&overlay));
    h.snapshot("hud");

    // Shown, the overlay stays under the pause menu, the inventory and the chat
    h.press(KeyCode::F3);
    h.press(KeyCode::Escape);
    assert!(h.stack_index::<HudRoot>() < h.stack_index::<PauseMenu>());
    h.press(KeyCode::Escape);
    h.press(KeyCode::KeyE);
    assert!(h.stack_index::<HudRoot>() < h.stack_index::<InventoryDialog>());
    // Nothing to aim at: the reticle does not show through the panel
    assert_eq!(h.reticle_bars(), 0);
    h.press(KeyCode::KeyE);
    assert_eq!(h.reticle_bars(), 2);
    h.press(KeyCode::KeyT);
    assert!(h.stack_index::<HudRoot>() < h.stack_index::<ChatRoot>());
    // The chat leaves the reticle
    assert_eq!(h.reticle_bars(), 2);
    h.press(KeyCode::Escape);
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn the_wheel_moves_the_hotbar_selection() {
    let mut h = enter_world(GameFolder::new().with_world("Wheel"), "Wheel");
    let selected = |h: &mut Harness| {
        h.world_mut()
            .query::<&Hotbar>()
            .single(h.world())
            .unwrap()
            .selected
    };
    assert_eq!(selected(&mut h), 0);
    h.scroll(-1.);
    assert_eq!(selected(&mut h), 1);
    h.scroll(1.);
    h.scroll(1.);
    assert_eq!(selected(&mut h), 8);
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn the_inventory_opens_closes_and_moves_stacks() {
    let planks = ItemId::OakPlanks;
    let folder = GameFolder::new().with_world("Bag").with_items(
        "Bag",
        "Tester",
        &[
            (0, ItemId::Stick, 3),
            (10, ItemId::Dirt, 5),
            (grid_slot(0, 0), planks, 1),
            (grid_slot(0, 1), planks, 1),
            (grid_slot(1, 0), planks, 1),
            (grid_slot(1, 1), planks, 1),
        ],
    );
    let mut h = enter_world(folder, "Bag");
    h.wait_until("the saved inventory", SERVER, |w| {
        slot(w, 10) == Some((ItemId::Dirt, 5))
    });

    h.press(KeyCode::KeyE);
    h.wait_for(&L::marker::<InventoryRoot>(), SCREEN);
    h.snapshot("inventory");

    // A stack of the inventory goes to another slot
    h.click(&L::cell(10));
    h.wait_for(
        &L::marker::<SlotIcon>().within(L::marker::<FloatingStack>()),
        SCREEN,
    );
    h.click(&L::cell(20));
    h.wait_until("the dirt in slot 20", SERVER, |w| {
        slot(w, 20) == Some((ItemId::Dirt, 5)) && slot(w, 10).is_none()
    });

    // A stack of the hotbar goes into the inventory
    h.click(&L::cell(0));
    h.wait_until("the sticks held", SERVER, |w| {
        floating(w) == Some((ItemId::Stick, 3))
    });
    h.click(&L::cell(30));
    h.wait_until("the sticks in slot 30", SERVER, |w| {
        slot(w, 30) == Some((ItemId::Stick, 3)) && slot(w, 0).is_none()
    });

    // Four planks make a crafting table, a single one: no count is shown
    assert_eq!(h.text_of(&L::marker::<CraftResultCell>()), "");
    assert_eq!(h.result_icon(), h.icon_index(ItemId::CraftingTable));
    h.click(&L::marker::<CraftResultCell>());
    h.wait_until("the crafting table held", SERVER, |w| {
        let grid = [(0, 0), (0, 1), (1, 0), (1, 1)];
        floating(w) == Some((ItemId::CraftingTable, 1))
            && grid
                .iter()
                .all(|&(row, col)| slot(w, grid_slot(row, col)).is_none())
    });

    // Closing puts the held stack back into the inventory
    h.press(KeyCode::KeyE);
    assert!(!h.is_visible(&L::marker::<InventoryRoot>()));
    h.wait_until("the crafting table in a slot", SERVER, |w| {
        let inventory = w.resource::<Inventory>();
        inventory.floating.is_none()
            && inventory
                .inner
                .values()
                .any(|stack| stack.item_id == ItemId::CraftingTable)
    });
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn escape_closes_the_inventory() {
    let mut h = enter_world(GameFolder::new().with_world("Escape"), "Escape");
    // Escape opens the pause menu during gameplay
    h.press(KeyCode::Escape);
    assert!(h.is_visible(&L::text("Resume")));
    h.press(KeyCode::Escape);
    assert!(!h.is_visible(&L::text("Resume")));

    h.press(KeyCode::KeyE);
    assert!(h.is_visible(&L::marker::<InventoryRoot>()));
    h.press(KeyCode::Escape);
    assert!(!h.is_visible(&L::marker::<InventoryRoot>()));
    assert!(!h.is_visible(&L::text("Resume")));
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn the_chat_sends_a_message_and_escape_closes_it() {
    let mut h = enter_world(GameFolder::new().with_world("Chat"), "Chat");
    h.press(KeyCode::KeyT);
    assert!(h.is_visible(&L::marker::<ChatRoot>()));
    h.type_text("bonjour");
    h.press(KeyCode::Enter);
    h.wait_for(&L::text_containing("bonjour"), SERVER);
    assert!(h.chat_shows("bonjour"));

    // A message dropped with Escape is not sent
    h.press(KeyCode::KeyT);
    h.type_text("brouillon");
    assert_eq!(h.value_of::<ChatInput>(), "brouillon");
    h.press(KeyCode::Escape);
    assert!(!h.is_visible(&L::marker::<ChatRoot>()));
    // The server answers in order: once the next message is shown, the dropped one would be too
    h.press(KeyCode::KeyT);
    h.type_text("ensuite");
    h.press(KeyCode::Enter);
    h.wait_for(&L::text_containing("ensuite"), SERVER);
    assert!(h.chat_shows("ensuite"));
    assert!(!h.is_visible(&L::text_containing("brouillon")));
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn typing_in_the_chat_keeps_the_game_keys_quiet() {
    let mut h = enter_world(GameFolder::new().with_world("Quiet"), "Quiet");
    h.press(KeyCode::KeyT);
    // The key that opens the chat is not typed into it
    assert_eq!(h.value_of::<ChatInput>(), "");
    h.type_text("e");
    assert!(!h.is_visible(&L::marker::<InventoryRoot>()));
    // Neither the digit, the debug keys nor the wheel reach the game
    h.type_text("3");
    h.press(KeyCode::F3);
    h.press(KeyCode::F6);
    h.scroll(-1.);
    assert_eq!(
        h.world_mut()
            .query::<&Hotbar>()
            .single(h.world())
            .unwrap()
            .selected,
        0
    );
    assert!(!h.is_visible(&L::marker::<HudRoot>()));
    assert!(!h.world().resource::<WireframeConfig>().global);
    assert_eq!(h.value_of::<ChatInput>(), "e3");

    // Escape closes the chat, and the pause menu does not open
    h.press(KeyCode::Escape);
    assert!(!h.is_visible(&L::marker::<ChatRoot>()));
    assert!(!h.is_visible(&L::text("Resume")));

    // The closed field holds no key
    h.press(KeyCode::KeyE);
    assert!(h.is_visible(&L::marker::<InventoryRoot>()));

    // An open inventory closes with its key, and the debug keys wait for the pause menu too
    h.press(KeyCode::KeyE);
    assert!(!h.is_visible(&L::marker::<InventoryRoot>()));
    h.press(KeyCode::Escape);
    assert!(h.is_visible(&L::text("Resume")));
    h.press(KeyCode::F3);
    h.press(KeyCode::F6);
    assert!(!h.is_visible(&L::marker::<HudRoot>()));
    assert!(!h.world().resource::<WireframeConfig>().global);
    h.press(KeyCode::Escape);
    h.press(KeyCode::F3);
    assert!(h.is_visible(&L::marker::<HudRoot>()));
}

/// Whether the client world holds a block at `position`.
fn is_block(w: &World, position: IVec3) -> bool {
    w.resource::<ClientWorldMap>()
        .blocks
        .get_block(position)
        .is_some()
}

/// The aimed block and the cell a block placed against it takes, once the aimed one is loaded.
fn aimed_cells(w: &World, camera: Entity, player: Entity) -> Option<(IVec3, IVec3)> {
    let target = raycast(
        &w.resource::<ClientWorldMap>().blocks,
        w.get::<Transform>(camera)?,
        &w.get::<Transform>(player)?.translation,
        *w.resource::<ViewMode>(),
    )?;
    Some((target.position, target.position + target.face.to_ivec3()))
}

/// Presses `button` over `locator`, holds it for `frames` frames, then releases it.
#[track_caller]
fn hold_click(h: &mut Harness, locator: &L, button: MouseButton, frames: usize) {
    let center = h.center_of(locator);
    h.move_cursor(center);
    h.frames(1);
    let window = h
        .world_mut()
        .query_filtered::<Entity, With<PrimaryWindow>>()
        .single(h.world())
        .unwrap();
    for (state, held) in [(ButtonState::Pressed, frames), (ButtonState::Released, 1)] {
        let input = MouseButtonInput {
            button,
            state,
            window,
        };
        h.world_mut().write_message(input);
        h.world_mut()
            .write_message(WindowEvent::MouseButtonInput(input));
        h.frames(held);
    }
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn the_click_that_closes_a_ui_does_not_reach_the_world() {
    let folder =
        GameFolder::new()
            .with_world("Held")
            .with_items("Held", "Tester", &[(0, ItemId::Dirt, 64)]);
    let mut h = enter_world(folder, "Held");
    h.wait_until("the saved inventory", SERVER, |w| {
        slot(w, 0) == Some((ItemId::Dirt, 64))
    });

    // Look down ahead of the player, at the top of a block, and wait for it to land within reach of the ground it
    // aims at: the cell a block placed there takes
    let world = h.world_mut();
    let camera = world
        .query_filtered::<Entity, With<CameraController>>()
        .single(world)
        .unwrap();
    let player = world
        .query_filtered::<Entity, With<CurrentPlayerMarker>>()
        .single(world)
        .unwrap();
    world.get_mut::<CameraController>(camera).unwrap().angle_y = 0.6;
    h.wait_until("the ground within reach", SERVER, |w| {
        aimed_cells(w, camera, player).is_some_and(|(target, _)| {
            let feet = w.get::<Transform>(player).unwrap().translation;
            target.as_vec3().distance(feet) < 3.
        })
    });
    // Settled on the ground
    h.frames(30);
    let (target, cell) = aimed_cells(h.world(), camera, player).unwrap();
    assert!(is_block(h.world(), target) && !is_block(h.world(), cell));

    // The click that closes the chat
    h.press(KeyCode::KeyT);
    hold_click(&mut h, &L::marker::<Hotbar>(), MouseButton::Right, 6);
    assert!(!h.is_visible(&L::marker::<ChatRoot>()));
    assert!(is_block(h.world(), target) && !is_block(h.world(), cell));

    // The click that resumes the game
    h.press(KeyCode::Escape);
    // The legacy button answers the left button only: a held left button would start breaking
    // the aimed block, which shows in its breaking progress
    hold_click(&mut h, &L::text("Resume"), MouseButton::Left, 6);
    assert!(!h.is_visible(&L::text("Resume")));
    assert!(is_block(h.world(), target) && !is_block(h.world(), cell));
    let progress = h
        .world()
        .resource::<ClientWorldMap>()
        .blocks
        .get_block(target)
        .unwrap()
        .breaking_progress;
    assert_eq!(progress, 0);

    // A new click in gameplay does place a block there
    hold_click(&mut h, &L::marker::<Hotbar>(), MouseButton::Right, 6);
    h.wait_until("the placed block", SERVER, |w| is_block(w, cell));
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn clicking_outside_the_chat_closes_it() {
    let mut h = enter_world(GameFolder::new().with_world("Outside"), "Outside");
    h.press(KeyCode::KeyT);
    h.type_text("draft");
    h.click(&L::marker::<Hotbar>());
    assert!(!h.is_visible(&L::marker::<ChatRoot>()));
    assert!(*h.world().resource::<UIMode>() == UIMode::Closed);

    h.press(KeyCode::KeyE);
    assert!(h.is_visible(&L::marker::<InventoryRoot>()));
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn the_pause_menu_resumes_saves_and_leaves() {
    let mut h = enter_world(GameFolder::new().with_world("Pause"), "Pause");
    h.press(KeyCode::Escape);
    h.wait_for(&L::text("Resume"), SCREEN);
    h.snapshot("pause");
    h.click(&L::text("Resume"));
    assert!(!h.is_visible(&L::text("Resume")));

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
    h.click(&L::text("Settings"));
    h.wait_for(&L::role(Role::Tab, "Controls"), SCREEN);
    assert!(is_in(h.world(), GameState::Game));
    assert!(*h.world().resource::<UIMode>() == UIMode::Opened);
    // The buttons of the pause menu beneath them are out of the cursor's reach
    h.hover(&L::text("Save"));
    let save = h.get(&L::text("Save"));
    let save = h.world().get::<ChildOf>(save).unwrap().parent();
    assert_eq!(h.world().get::<Interaction>(save), Some(&Interaction::None));
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
    assert!(h.is_visible(&L::text("Resume")));
    h.click(&L::text("Resume"));
    // No click on the settings reached the "Save" beneath them
    assert!(!player.exists());
    h.press(KeyCode::Escape);
    h.click(&L::text("Save"));
    // Save writes the player at once and world.ron a little later, on another thread
    h.wait_until("the saved player", SERVER, |_| player.exists());
    h.wait_until("world.ron saved", SERVER, |_| written() > seeded);

    // Leaving saves the world again: world.ron is written after the click
    let before = written();
    h.click(&L::text("Back to menu"));
    h.wait_until("the main menu", SERVER, |w| {
        is_in(w, GameState::Menu) && *w.resource::<State<MenuState>>().get() == MenuState::Main
    });
    h.wait_until("world.ron written", SERVER, |_| written() > before);
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn dropping_the_harness_in_game_stops_the_local_server() {
    let folder = GameFolder::new().with_world("Drop");
    let world = folder.path().join(SAVE_PATH).join("Drop/world.ron");
    // The folder outlives the harness, which deletes its own copy when dropped
    let _kept = folder.clone();
    let written = || fs::metadata(&world).unwrap().modified().unwrap();
    let h = enter_world(folder, "Drop");
    let before = written();
    drop(h);
    assert!(
        written() > before,
        "world.ron was not saved when the harness was dropped"
    );
}
