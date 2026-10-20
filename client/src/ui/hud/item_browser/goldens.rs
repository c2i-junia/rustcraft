//! Golden screenshots of the item browser beside the open inventory, at the sizes where it docks
//! and where it is a drawer, in Creative, and of its recipe view. See `crate::ui::golden`. They need lavapipe, which is only installed
//! on Linux.

use bevy::prelude::*;
use shared::players::{game_mode::GameMode, Inventory, Player};
use shared::world::{ItemId, ItemStack};

use super::bookmarks::Bookmarks;
use super::layout::BrowserLayout;
use super::{
    filter_catalog, layout_item_browser, mark_craftable_items, setup_item_browser,
    show_item_browser, show_item_icons, show_mode_footer, show_view, style_chips,
    style_fill_buttons, style_recipe_cards, update_craftable_now, CatalogPart, CraftableNow,
    ItemBrowserState, ItemView, MissingTint, RecipeCard, RecipeSlot, RecipeTab,
};
use crate::player::CurrentPlayerMarker;
use crate::ui::golden::UiGolden;
use crate::ui::hud::{
    display_crafting_system, display_inventory_system, setup_inventory, style_inventory_panel,
    InventoryRoot,
};
use crate::ui::widgets::list::hide_idle_scrollbars;
use crate::ui::widgets::slot::style_slots;
use crate::world::ClientWorldMap;

/// Builds the panel and the browser in `golden`, opens the inventory, and runs the systems that
/// show them, twice: the browser is laid out from the size of the window and of the panel.
pub(crate) fn show_open_inventory(golden: &mut UiGolden, open: bool) {
    golden.world_mut().init_resource::<ItemBrowserState>();
    golden.world_mut().init_resource::<BrowserLayout>();
    golden.world_mut().init_resource::<CraftableNow>();
    golden.world_mut().init_resource::<Bookmarks>();
    golden.run_system(setup_inventory);
    golden.run_system(setup_item_browser);
    if open {
        let mut root = golden
            .world_mut()
            .query_filtered::<&mut Visibility, With<InventoryRoot>>();
        *root.single_mut(golden.world_mut()).unwrap() = Visibility::Visible;
    }
    show_frames(golden);
}

/// Runs the systems that show the panel and the browser, twice: the browser is laid out from the
/// size of the window and of the panel.
fn show_frames(golden: &mut UiGolden) {
    for _ in 0..2 {
        golden.frames(1);
        golden.run_system(display_inventory_system);
        golden.run_system(display_crafting_system);
        golden.run_system(style_inventory_panel);
        golden.run_system(layout_item_browser);
        golden.run_system(show_item_icons);
        golden.run_system(update_craftable_now);
        golden.run_system(filter_catalog);
        golden.run_system(mark_craftable_items);
        golden.run_system(show_mode_footer);
        golden.run_system(show_view);
        golden.run_system(show_item_icons);
        golden.run_system(mark_craftable_items);
        golden.run_system(style_recipe_cards);
        golden.run_system(show_item_browser);
        golden.run_system(style_chips);
        golden.run_system(style_fill_buttons);
        golden.run_system(style_slots);
        golden.run_system(hide_idle_scrollbars);
    }
}

fn stack(item_id: ItemId, nb: u32) -> ItemStack {
    ItemStack {
        item_id,
        item_type: item_id.get_default_type(),
        nb,
    }
}

/// An inventory holding these stacks, in these slots.
fn inventory_of(stacks: &[(u32, ItemId, u32)]) -> Inventory {
    let mut inventory = Inventory::new();
    for &(slot, item, nb) in stacks {
        inventory.inner.insert(slot, stack(item, nb));
    }
    inventory
}

/// The open inventory holding `inventory` and the browser beside it, in `state`, in a window of
/// `width` by `height`.
fn open_browser(
    (width, height): (u32, u32),
    inventory: Inventory,
    state: ItemBrowserState,
) -> UiGolden {
    let mut golden = UiGolden::with_size(width, height);
    golden.world_mut().insert_resource(inventory);
    golden.world_mut().init_resource::<ClientWorldMap>();
    golden.world_mut().insert_resource(state);
    show_open_inventory(&mut golden, true);
    golden
}

/// Renders the open inventory, holding a few stacks, and the browser beside it, in a window of
/// `width` by `height`, with the browser in `state`.
fn assert_browser_matches(name: &str, size: (u32, u32), state: ItemBrowserState) {
    let inventory = inventory_of(&[
        (0, ItemId::WoodenPickaxe, 1),
        (1, ItemId::Cobblestone, 40),
        (12, ItemId::OakLog, 12),
    ]);
    open_browser(size, inventory, state).assert_matches(name);
}

/// The browser showing `view`, with `history` behind it.
fn viewing(view: ItemView, history: &[ItemView]) -> ItemBrowserState {
    ItemBrowserState {
        view: Some(view),
        history: history.to_vec(),
        ..default()
    }
}

/// Five oak planks and a stack of logs: a pickaxe misses its sticks.
fn planks_and_logs() -> Inventory {
    inventory_of(&[
        (0, ItemId::WoodenPickaxe, 1),
        (1, ItemId::OakPlanks, 5),
        (12, ItemId::OakLog, 12),
    ])
}

const PICKAXE_RECIPES: ItemView = ItemView {
    item: ItemId::WoodenPickaxe,
    tab: RecipeTab::Recipes,
};

/// How many entities hold `C`, and how many of them are displayed.
fn count<C: Component>(golden: &mut UiGolden) -> (usize, usize) {
    let mut nodes = golden.world_mut().query_filtered::<&Node, With<C>>();
    let world = golden.world_mut();
    let all = nodes.iter(world).count();
    let shown = nodes
        .iter(world)
        .filter(|node| node.display != Display::None)
        .count();
    (all, shown)
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn a_view_hides_the_catalog_and_builds_its_cards() {
    let state = viewing(PICKAXE_RECIPES, &[]);
    let mut golden = open_browser((1280, 720), planks_and_logs(), state);
    assert_eq!(count::<CatalogPart>(&mut golden), (3, 0));
    assert_eq!(count::<RecipeCard>(&mut golden).0, 2);
    // Five ingredients and the output on each card
    assert_eq!(count::<RecipeSlot>(&mut golden).0, 12);
    // The two sticks of the oak card, everything on the spruce card
    assert_eq!(count::<MissingTint>(&mut golden), (10, 2 + 5));

    golden.world_mut().resource_mut::<ItemBrowserState>().back();
    show_frames(&mut golden);
    assert_eq!(count::<CatalogPart>(&mut golden), (3, 3));
    assert_eq!(count::<RecipeCard>(&mut golden).0, 0);
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn the_recipes_of_the_wooden_pickaxe_at_1280x720() {
    let stick = ItemView {
        item: ItemId::Stick,
        tab: RecipeTab::Recipes,
    };
    let state = viewing(PICKAXE_RECIPES, &[stick]);
    open_browser((1280, 720), planks_and_logs(), state)
        .assert_matches("item_browser_recipes_1280x720");
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn the_uses_of_oak_planks_at_854x480() {
    let uses = ItemView {
        item: ItemId::OakPlanks,
        tab: RecipeTab::Uses,
    };
    open_browser((854, 480), planks_and_logs(), viewing(uses, &[]))
        .assert_matches("item_browser_uses_854x480");
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn the_creative_panel_at_1280x720() {
    let mut golden = UiGolden::with_size(1280, 720);
    golden.world_mut().insert_resource(inventory_of(&[
        (0, ItemId::WoodenPickaxe, 1),
        (1, ItemId::Cobblestone, 40),
        (12, ItemId::OakLog, 12),
    ]));
    golden.world_mut().init_resource::<ClientWorldMap>();
    golden.world_mut().init_resource::<ItemBrowserState>();
    let mut player = Player::new(1, "tester".into(), Vec3::ZERO, Transform::default());
    player.game_mode = GameMode::Creative;
    golden.world_mut().spawn((player, CurrentPlayerMarker {}));
    show_open_inventory(&mut golden, true);
    golden.assert_matches("item_browser_creative_1280x720");
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn the_item_browser_docks_eight_columns_at_1280x720() {
    assert_browser_matches(
        "item_browser_1280x720",
        (1280, 720),
        ItemBrowserState::default(),
    );
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn the_item_browser_docks_four_columns_at_854x480() {
    assert_browser_matches(
        "item_browser_854x480",
        (854, 480),
        ItemBrowserState::default(),
    );
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn the_drawer_is_a_handle_at_640x480() {
    assert_browser_matches(
        "item_browser_drawer_closed_640x480",
        (640, 480),
        ItemBrowserState::default(),
    );
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn the_open_drawer_covers_the_right_of_the_inventory_at_640x480() {
    let state = ItemBrowserState {
        drawer_open: true,
        ..default()
    };
    assert_browser_matches("item_browser_drawer_open_640x480", (640, 480), state);
}
