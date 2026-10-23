//! What throws items out of the inventory, beside the Drop key over a cell: a press outside the
//! open inventory with a held stack, a stack picked up from a cell and released outside it, and
//! the Drop key in the game.

use bevy::ecs::system::SystemParam;
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

/// Where the pointer is with respect to the open inventory. The panel is not pickable: the
/// pointer is inside it when it is within its rectangle.
#[derive(SystemParam)]
pub struct InventoryPointer<'w, 's> {
    window: Query<'w, 's, &'static Window, With<PrimaryWindow>>,
    panel:
        Query<'w, 's, (&'static ComputedNode, &'static UiGlobalTransform), With<InventoryDialog>>,
    browser: Query<'w, 's, &'static Hovered, Or<(With<ItemBrowser>, With<DrawerHandle>)>>,
}

impl InventoryPointer<'_, '_> {
    /// Whether the pointer is in the window, beside neither the panel nor the item browser.
    fn is_outside(&self) -> bool {
        let Some(cursor) = self
            .window
            .single()
            .ok()
            .and_then(Window::physical_cursor_position)
        else {
            return false;
        };
        let in_panel = self
            .panel
            .iter()
            .any(|(node, transform)| node.contains_point(*transform, cursor));
        !in_panel && !self.browser.iter().any(Hovered::get)
    }
}

/// Throws the stack held by the cursor on a press outside the panel of the open inventory and
/// the item browser: all of it with the left button, one item with the right one, as in
/// Minecraft. Not while a drag lasts, whose other button only cancels it, nor while the
/// browser's drawer is open, which the press closes.
pub fn throw_held_stack_system(
    mouse_input: Res<ButtonInput<MouseButton>>,
    inventory: Res<Inventory>,
    gestures: Res<CellGestures>,
    layout: Res<BrowserLayout>,
    browser_state: Res<ItemBrowserState>,
    inventory_root: Query<&Visibility, With<InventoryRoot>>,
    pointer: InventoryPointer,
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
    if !pointer.is_outside() {
        return;
    }
    frame_inputs
        .0
        .inventory_actions
        .push(InventoryAction::DropFloating { all });
}

/// Drag and drop out of the inventory: the stack a press picked up from a cell is thrown, all of
/// it, when the button is released outside the panel and the item browser. Released over them,
/// it stays on the cursor, as in Minecraft.
pub fn throw_carried_stack_system(
    mouse_input: Res<ButtonInput<MouseButton>>,
    inventory: Res<Inventory>,
    mut gestures: ResMut<CellGestures>,
    inventory_root: Query<&Visibility, With<InventoryRoot>>,
    pointer: InventoryPointer,
    mut frame_inputs: ResMut<CurrentFrameInputs>,
) {
    let Some(button) = gestures.carry else {
        return;
    };
    let open = matches!(inventory_root.single(), Ok(&Visibility::Visible));
    if open && mouse_input.pressed(button) {
        return;
    }
    gestures.carry = None;
    if !open || !mouse_input.just_released(button) || inventory.floating.is_none() {
        return;
    }
    if pointer.is_outside() {
        frame_inputs
            .0
            .inventory_actions
            .push(InventoryAction::DropFloating { all: true });
    }
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

    /// Moves the cursor to `at` and releases `button`, carried since a press on a cell, and
    /// returns what the frame sends.
    fn release_carried(world: &mut World, button: MouseButton, at: Vec2) -> Vec<InventoryAction> {
        world.resource_mut::<CellGestures>().carry = Some(button);
        let mut window = world.query::<&mut Window>();
        window
            .single_mut(world)
            .unwrap()
            .set_physical_cursor_position(Some(DVec2::new(at.x as f64, at.y as f64)));
        let mut buttons = world.resource_mut::<ButtonInput<MouseButton>>();
        buttons.press(button);
        buttons.clear();
        buttons.release(button);
        world.run_system_once(throw_carried_stack_system).unwrap();
        world.resource_mut::<ButtonInput<MouseButton>>().clear();
        assert_eq!(world.resource::<CellGestures>().carry, None);
        std::mem::take(
            &mut world
                .resource_mut::<CurrentFrameInputs>()
                .0
                .inventory_actions,
        )
    }

    #[test]
    fn a_stack_carried_out_of_the_inventory_is_thrown_whole_on_release() {
        let mut world = open_inventory();
        for button in [MouseButton::Left, MouseButton::Right] {
            assert_eq!(
                release_carried(&mut world, button, OUTSIDE),
                [InventoryAction::DropFloating { all: true }],
                "{button:?}"
            );
        }
    }

    #[test]
    fn a_stack_carried_and_released_over_the_panel_or_the_browser_stays_on_the_cursor() {
        let mut world = open_inventory();
        assert!(release_carried(&mut world, MouseButton::Left, PANEL_CENTRE).is_empty());

        let browser = world
            .query_filtered::<Entity, With<ItemBrowser>>()
            .single(&world)
            .unwrap();
        world.entity_mut(browser).insert(Hovered(true));
        assert!(release_carried(&mut world, MouseButton::Left, OUTSIDE).is_empty());
    }

    #[test]
    fn a_carry_lasts_while_its_button_is_down_and_ends_with_the_inventory() {
        let mut world = open_inventory();
        world.resource_mut::<CellGestures>().carry = Some(MouseButton::Left);
        world
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        world.run_system_once(throw_carried_stack_system).unwrap();
        assert_eq!(
            world.resource::<CellGestures>().carry,
            Some(MouseButton::Left)
        );

        let mut root = world.query_filtered::<&mut Visibility, With<InventoryRoot>>();
        *root.single_mut(&mut world).unwrap() = Visibility::Hidden;
        world.run_system_once(throw_carried_stack_system).unwrap();
        assert_eq!(world.resource::<CellGestures>().carry, None);
        assert!(world
            .resource::<CurrentFrameInputs>()
            .0
            .inventory_actions
            .is_empty());
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
