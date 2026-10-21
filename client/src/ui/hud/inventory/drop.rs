//! What throws items out of the inventory, beside the Drop key over a cell: a press outside the
//! open inventory with a held stack, and the Drop key in the game.

use bevy::picking::hover::Hovered;
use bevy::prelude::*;
use bevy::ui::UiGlobalTransform;
use bevy::window::PrimaryWindow;
use shared::players::inventory::InventoryAction;
use shared::players::Inventory;

use super::{CellGestures, InventoryDialog, InventoryRoot};
use crate::input::data::GameAction;
use crate::input::keyboard::is_action_just_pressed;
use crate::network::buffered_client::CurrentFrameInputs;
use crate::state::KeyMap;
use crate::ui::hud::hotbar::Hotbar;
use crate::ui::hud::item_browser::layout::BrowserLayout;
use crate::ui::hud::item_browser::{DrawerHandle, ItemBrowser, ItemBrowserState};
use crate::ui::hud::UIMode;

/// Throws the stack held by the cursor on a press outside the panel of the open inventory and
/// the item browser: all of it with the left button, one item with the right one, as in
/// Minecraft. Not while a drag lasts, whose other button only cancels it, nor while the
/// browser's drawer is open, which the press closes. The panel is not pickable: the pointer is
/// inside it when it is within its rectangle.
#[allow(clippy::too_many_arguments)]
pub fn throw_held_stack_system(
    mouse_input: Res<ButtonInput<MouseButton>>,
    inventory: Res<Inventory>,
    gestures: Res<CellGestures>,
    layout: Res<BrowserLayout>,
    browser_state: Res<ItemBrowserState>,
    inventory_root: Query<&Visibility, With<InventoryRoot>>,
    window: Query<&Window, With<PrimaryWindow>>,
    panel: Query<(&ComputedNode, &UiGlobalTransform), With<InventoryDialog>>,
    browser: Query<&Hovered, Or<(With<ItemBrowser>, With<DrawerHandle>)>>,
    mut frame_inputs: ResMut<CurrentFrameInputs>,
) {
    if !matches!(inventory_root.single(), Ok(&Visibility::Visible)) {
        return;
    }
    let all = if mouse_input.just_pressed(MouseButton::Left) {
        true
    } else if mouse_input.just_pressed(MouseButton::Right) {
        false
    } else {
        return;
    };
    let drawer_open = layout.compact && browser_state.drawer_open;
    if inventory.floating.is_none() || gestures.drag.is_some() || drawer_open {
        return;
    }
    let Some(cursor) = window
        .single()
        .ok()
        .and_then(Window::physical_cursor_position)
    else {
        return;
    };
    let in_panel = panel
        .iter()
        .any(|(node, transform)| node.contains_point(*transform, cursor));
    if in_panel || browser.iter().any(Hovered::get) {
        return;
    }
    frame_inputs
        .0
        .inventory_actions
        .push(InventoryAction::DropFloating { all });
}

/// Drop in the game throws one item of the selected hotbar slot, Ctrl+Drop its stack. Only
/// during gameplay: not while a dialog is open or the player types.
pub fn drop_selected_item_system(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    key_map: Res<KeyMap>,
    ui_mode: Res<UIMode>,
    inventory: Res<Inventory>,
    hotbar: Query<&Hotbar>,
    mut frame_inputs: ResMut<CurrentFrameInputs>,
) {
    if *ui_mode != UIMode::Closed
        || !is_action_just_pressed(GameAction::Drop, &keyboard_input, &key_map)
    {
        return;
    }
    let Ok(hotbar) = hotbar.single() else {
        return;
    };
    if !inventory.inner.contains_key(&hotbar.selected) {
        return;
    }
    let all = keyboard_input.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight]);
    frame_inputs
        .0
        .inventory_actions
        .push(InventoryAction::DropSlot {
            slot: hotbar.selected,
            all,
        });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::hud::CellDrag;
    use bevy::ecs::system::RunSystemOnce;
    use bevy::math::DVec2;
    use shared::world::{ItemId, ItemStack};

    /// The centre of the panel, and its size, in physical pixels.
    const PANEL_CENTRE: Vec2 = Vec2::new(640.0, 500.0);
    const PANEL_SIZE: Vec2 = Vec2::new(400.0, 300.0);
    const OUTSIDE: Vec2 = Vec2::new(10.0, 10.0);

    /// An open inventory whose panel lies around `PANEL_CENTRE`, beside an item browser, the
    /// cursor holding five dirt.
    fn open_inventory() -> World {
        let mut world = World::new();
        world.insert_resource(ButtonInput::<MouseButton>::default());
        let mut inventory = Inventory::new();
        inventory.floating = Some(ItemStack {
            item_id: ItemId::Dirt,
            item_type: ItemId::Dirt.get_default_type(),
            nb: 5,
        });
        world.insert_resource(inventory);
        world.init_resource::<CellGestures>();
        world.init_resource::<BrowserLayout>();
        world.init_resource::<ItemBrowserState>();
        world.init_resource::<CurrentFrameInputs>();
        world.spawn((InventoryRoot, Visibility::Visible));
        world.spawn((Window::default(), PrimaryWindow));
        world.spawn((
            InventoryDialog,
            ComputedNode {
                size: PANEL_SIZE,
                ..default()
            },
            UiGlobalTransform::from_translation(PANEL_CENTRE),
        ));
        world.spawn((ItemBrowser, Hovered(false)));
        world.spawn((DrawerHandle, Hovered(false)));
        world
    }

    /// Presses `button` with the cursor at `at`, and returns what the frame sends.
    fn press(world: &mut World, button: MouseButton, at: Vec2) -> Vec<InventoryAction> {
        let mut window = world.query::<&mut Window>();
        window
            .single_mut(world)
            .unwrap()
            .set_physical_cursor_position(Some(DVec2::new(at.x as f64, at.y as f64)));
        world
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(button);
        world.run_system_once(throw_held_stack_system).unwrap();
        let mut buttons = world.resource_mut::<ButtonInput<MouseButton>>();
        buttons.release(button);
        buttons.clear();
        std::mem::take(
            &mut world
                .resource_mut::<CurrentFrameInputs>()
                .0
                .inventory_actions,
        )
    }

    #[test]
    fn a_press_outside_throws_the_held_stack_left_all_of_it_right_one_item() {
        let mut world = open_inventory();
        assert_eq!(
            press(&mut world, MouseButton::Left, OUTSIDE),
            [InventoryAction::DropFloating { all: true }]
        );
        assert_eq!(
            press(&mut world, MouseButton::Right, OUTSIDE),
            [InventoryAction::DropFloating { all: false }]
        );
    }

    #[test]
    fn a_press_inside_the_panel_or_over_the_browser_throws_nothing() {
        let mut world = open_inventory();
        // On the panel between two slots, and on its corner
        for at in [PANEL_CENTRE, PANEL_CENTRE - PANEL_SIZE / 2.0 + 1.0] {
            assert!(press(&mut world, MouseButton::Left, at).is_empty(), "{at}");
        }
        for over in [
            world
                .query_filtered::<Entity, With<ItemBrowser>>()
                .single(&world)
                .unwrap(),
            world
                .query_filtered::<Entity, With<DrawerHandle>>()
                .single(&world)
                .unwrap(),
        ] {
            world.entity_mut(over).insert(Hovered(true));
            assert!(press(&mut world, MouseButton::Left, OUTSIDE).is_empty());
            world.entity_mut(over).insert(Hovered(false));
        }
    }

    #[test]
    fn a_press_outside_throws_nothing_without_a_held_stack_or_an_open_inventory() {
        let mut world = open_inventory();
        world.resource_mut::<Inventory>().floating = None;
        assert!(press(&mut world, MouseButton::Left, OUTSIDE).is_empty());

        let mut world = open_inventory();
        let mut root = world.query_filtered::<&mut Visibility, With<InventoryRoot>>();
        *root.single_mut(&mut world).unwrap() = Visibility::Hidden;
        assert!(press(&mut world, MouseButton::Left, OUTSIDE).is_empty());
    }

    #[test]
    fn a_press_outside_during_a_drag_only_cancels_it() {
        let mut world = open_inventory();
        world.resource_mut::<CellGestures>().drag = Some(CellDrag {
            button: MouseButton::Left,
            start: 9,
            slots: 1 << 9,
        });
        assert!(press(&mut world, MouseButton::Right, OUTSIDE).is_empty());
    }

    #[test]
    fn a_press_outside_the_open_drawer_only_closes_it() {
        let mut world = open_inventory();
        world.resource_mut::<BrowserLayout>().compact = true;
        world.resource_mut::<ItemBrowserState>().drawer_open = true;
        assert!(press(&mut world, MouseButton::Left, OUTSIDE).is_empty());

        // Closed, the drawer leaves the press to the inventory
        world.resource_mut::<ItemBrowserState>().drawer_open = false;
        assert_eq!(
            press(&mut world, MouseButton::Left, OUTSIDE),
            [InventoryAction::DropFloating { all: true }]
        );
    }

    /// A player in the game, its hotbar on slot 2 which holds five dirt.
    fn in_game() -> World {
        let mut world = World::new();
        world.insert_resource(ButtonInput::<KeyCode>::default());
        world.insert_resource(crate::input::keyboard::default_bindings());
        world.insert_resource(UIMode::Closed);
        let mut inventory = Inventory::new();
        inventory.inner.insert(
            2,
            ItemStack {
                item_id: ItemId::Dirt,
                item_type: ItemId::Dirt.get_default_type(),
                nb: 5,
            },
        );
        world.insert_resource(inventory);
        world.init_resource::<CurrentFrameInputs>();
        world.spawn(Hotbar { selected: 2 });
        world
    }

    /// Presses Drop, with Ctrl held or not, and returns what the frame sends.
    fn press_drop(world: &mut World, ctrl: bool) -> Vec<InventoryAction> {
        let mut keys = world.resource_mut::<ButtonInput<KeyCode>>();
        if ctrl {
            keys.press(KeyCode::ControlLeft);
        }
        keys.press(KeyCode::KeyQ);
        world.run_system_once(drop_selected_item_system).unwrap();
        let mut keys = world.resource_mut::<ButtonInput<KeyCode>>();
        keys.release_all();
        keys.clear();
        std::mem::take(
            &mut world
                .resource_mut::<CurrentFrameInputs>()
                .0
                .inventory_actions,
        )
    }

    #[test]
    fn drop_in_game_throws_one_item_of_the_selected_slot_and_ctrl_drop_its_stack() {
        let mut world = in_game();
        assert_eq!(
            press_drop(&mut world, false),
            [InventoryAction::DropSlot {
                slot: 2,
                all: false
            }]
        );
        assert_eq!(
            press_drop(&mut world, true),
            [InventoryAction::DropSlot { slot: 2, all: true }]
        );
    }

    #[test]
    fn drop_in_game_throws_nothing_from_an_empty_slot_in_a_dialog_or_while_typing() {
        let mut world = in_game();
        world
            .query::<&mut Hotbar>()
            .single_mut(&mut world)
            .unwrap()
            .selected = 3;
        assert!(press_drop(&mut world, false).is_empty());

        for mode in [UIMode::Opened, UIMode::Typing] {
            let mut world = in_game();
            world.insert_resource(mode);
            assert!(press_drop(&mut world, true).is_empty());
        }
    }
}
