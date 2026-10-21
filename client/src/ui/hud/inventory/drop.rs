//! What throws items out of the inventory, beside the Drop key over a cell: a press outside the
//! open inventory with a held stack.

use bevy::picking::hover::Hovered;
use bevy::prelude::*;
use bevy::ui::UiGlobalTransform;
use bevy::window::PrimaryWindow;
use shared::players::inventory::InventoryAction;
use shared::players::Inventory;

use super::{CellGestures, InventoryDialog, InventoryRoot};
use crate::network::buffered_client::CurrentFrameInputs;
use crate::ui::hud::item_browser::layout::BrowserLayout;
use crate::ui::hud::item_browser::{DrawerHandle, ItemBrowser, ItemBrowserState};

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
}
