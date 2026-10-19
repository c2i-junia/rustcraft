//! The game, through the integrated server: the HUD, the hotbar, the inventory and its crafting
//! grid. The chat is in `chat`, the pause menu and leaving in `pause`.

use std::fs;

use accesskit::Role;

use bevy::{
    input::{keyboard::KeyCode, mouse::MouseButtonInput, ButtonState},
    prelude::*,
    ui::ComputedStackIndex,
    window::{PrimaryWindow, WindowEvent},
};
use shared::players::{Inventory, ViewMode};
use shared::world::{grid_slot, raycast, FaceDirectionExt, ItemId};

use super::harness::{Harness, Options, SCREEN, SERVER, STARTUP};
use super::locator::{is_shown, Locator as L};
use super::seed::GameFolder;
use crate::camera::CameraController;
use crate::constants::SAVE_PATH;
use crate::dropped_items::DroppedItemRoot;
use crate::player::CurrentPlayerMarker;
use crate::state::GameState;
use crate::ui::hud::chat::ChatRoot;
use crate::ui::hud::debug::HudRoot;
use crate::ui::hud::hotbar::Hotbar;
use crate::ui::hud::{
    CraftResultCell, FloatingStack, InventoryCell, InventoryDialog, InventoryRoot,
};
use crate::ui::menus::pause::PauseMenu;
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
    pub(super) fn reticle_bars(&mut self) -> usize {
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
    assert!(h.is_visible(&L::role(Role::Button, "Back to game")));
    h.press(KeyCode::Escape);
    assert!(!h.is_visible(&L::role(Role::Button, "Back to game")));

    h.press(KeyCode::KeyE);
    assert!(h.is_visible(&L::marker::<InventoryRoot>()));
    h.press(KeyCode::Escape);
    assert!(!h.is_visible(&L::marker::<InventoryRoot>()));
    assert!(!h.is_visible(&L::role(Role::Button, "Back to game")));
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
    // A held left button would start breaking the aimed block, which shows in its breaking
    // progress
    hold_click(
        &mut h,
        &L::role(Role::Button, "Back to game"),
        MouseButton::Left,
        6,
    );
    assert!(!h.is_visible(&L::role(Role::Button, "Back to game")));
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

/// How many items lying in the world the client shows.
fn dropped_items(w: &mut World) -> usize {
    w.query::<&DroppedItemRoot>().iter(w).count()
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn a_broken_block_drops_an_item_that_the_player_picks_up() {
    let folder = GameFolder::new().with_world("Drops").with_items(
        "Drops",
        "Tester",
        &[(0, ItemId::Dirt, 1)],
    );
    let mut h = enter_world(folder, "Drops");
    h.wait_until("the saved inventory", SERVER, |w| {
        slot(w, 0) == Some((ItemId::Dirt, 1))
    });

    // Look down, at the ground ahead of the player
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
            let centre = w.get::<Transform>(player).unwrap().translation;
            target.as_vec3().distance(centre) < 3.
        })
    });
    // Settled on the ground
    h.frames(30);

    // Place the dirt, whatever the ground is, and aim at it
    let (_, target) = aimed_cells(h.world(), camera, player).unwrap();
    hold_click(&mut h, &L::marker::<Hotbar>(), MouseButton::Right, 6);
    h.wait_until("the placed dirt", SERVER, |w| {
        is_block(w, target) && slot(w, 0).is_none()
    });
    assert_eq!(aimed_cells(h.world(), camera, player).unwrap().0, target);

    // Mine it: the block goes, and nothing reaches the inventory at once
    let center = h.center_of(&L::marker::<Hotbar>());
    h.move_cursor(center);
    h.frames(1);
    let window = h
        .world_mut()
        .query_filtered::<Entity, With<PrimaryWindow>>()
        .single(h.world())
        .unwrap();
    let click = |state| MouseButtonInput {
        button: MouseButton::Left,
        state,
        window,
    };
    let pressed = click(ButtonState::Pressed);
    h.world_mut().write_message(pressed);
    h.world_mut()
        .write_message(WindowEvent::MouseButtonInput(pressed));
    h.wait_until("the mined block to break", SERVER, |w| !is_block(w, target));
    let released = click(ButtonState::Released);
    h.world_mut().write_message(released);
    h.world_mut()
        .write_message(WindowEvent::MouseButtonInput(released));
    assert_eq!(slot(h.world(), 0), None);

    // Its item appears where the block was, then the player walks to it and picks it up
    h.wait_until("the dropped item", SERVER, |w| {
        w.try_query::<&DroppedItemRoot>()
            .is_some_and(|mut items| items.iter(w).count() == 1)
    });
    h.key_down(KeyCode::KeyW);
    h.wait_until("the picked up item", SERVER, |w| {
        slot(w, 0) == Some((ItemId::Dirt, 1))
    });
    h.key_up(KeyCode::KeyW);
    h.frames(30);
    assert_eq!(dropped_items(h.world_mut()), 0);
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
