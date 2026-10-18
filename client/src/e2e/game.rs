//! The game, through the integrated server: the connection screen, the HUD, the hotbar, the
//! inventory and its crafting grid, the chat, the pause menu, and leaving.

use std::fs;

use bevy::{input::keyboard::KeyCode, prelude::*};
use shared::players::Inventory;
use shared::world::{grid_slot, ItemId};

use super::harness::{Harness, Options, SCREEN, SERVER};
use super::helpers::is_in;
use super::locator::{is_inside, is_shown, Locator as L};
use super::seed::GameFolder;
use crate::constants::SAVE_PATH;
use crate::network::hash_string_to_u64;
use crate::state::GameState;
use crate::ui::button::MenuState;
use crate::ui::hud::chat::{ChatDisplay, ChatRoot};
use crate::ui::hud::debug::HudRoot;
use crate::ui::hud::hotbar::Hotbar;
use crate::ui::hud::{
    CraftResultCell, FloatingStack, InventoryCell, InventoryRoot, StackCount, StackIcon,
};
use crate::world::MaterialResource;

/// A client in the game, in the world `world` of `folder`, with its hotbar on screen.
#[track_caller]
pub fn enter_world(folder: GameFolder, world: &str) -> Harness {
    let mut h = Harness::start(folder, Options::default());
    h.click_when_ready(&L::text("Singleplayer"));
    h.wait_for_icons(world);
    h.click(&L::name(&format!("Play {world}")));
    h.wait_for_game_state(GameState::Game, SERVER);
    h.wait_for(&L::marker::<Hotbar>(), SCREEN);
    h
}

/// The item and the count of a stack, which is all a player sees of it.
type Shown = Option<(ItemId, u32)>;

/// What the inventory of the client holds in slot `id`.
fn slot(w: &World, id: u32) -> Shown {
    let stack = w.resource::<Inventory>().inner.get(&id).copied();
    stack.map(|stack| (stack.item_id, stack.nb))
}

/// What the cursor holds.
fn floating(w: &World) -> Shown {
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

    /// The index in the items atlas of the icon shown by the result cell.
    #[track_caller]
    fn result_icon(&mut self) -> Option<usize> {
        let icon = self.get(&L::marker::<StackIcon>().within(L::marker::<CraftResultCell>()));
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
    h.click_when_ready(&L::text("Multiplayer"));
    h.wait_for_icons("Nowhere");
    h.click(&L::name("Play Nowhere"));
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

    // Whether the overlay starts shown is not the point: F3 flips it, and flips it back
    let shown = h.is_visible(&L::marker::<HudRoot>());
    h.press(KeyCode::F3);
    assert_eq!(h.is_visible(&L::marker::<HudRoot>()), !shown);
    h.press(KeyCode::F3);
    assert_eq!(h.is_visible(&L::marker::<HudRoot>()), shown);
    h.snapshot("hud");
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
        &L::marker::<StackIcon>().within(L::marker::<FloatingStack>()),
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

    // Four planks make a crafting table
    let count = L::marker::<StackCount>().within(L::marker::<CraftResultCell>());
    assert_eq!(h.text_of(&count), "1");
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
    assert!(h.is_visible(&L::text_containing("brouillon")));
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
    assert!(!player.exists());
    h.press(KeyCode::Escape);
    h.click(&L::text("Save"));
    h.wait_until("the saved player", SERVER, |_| player.exists());

    // Leaving saves the world: world.ron is written after the click
    let meta = h.folder().join(SAVE_PATH).join("Pause/world.ron");
    let written = || fs::metadata(&meta).unwrap().modified().unwrap();
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
