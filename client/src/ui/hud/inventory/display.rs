use super::items::{left_click_slot, right_click_slot};
use super::CELL_BORDER_COLOR;
use crate::constants::MAX_HOTBAR_SLOTS;
use crate::input::data::GameAction;
use crate::input::keyboard::is_action_just_pressed;
use crate::state::KeyMap;
use crate::ui::hud::hotbar::Hotbar;
use crate::ui::hud::{FloatingStack, InventoryCell, InventoryRoot, StackCount, StackIcon, UIMode};
use crate::world::MaterialResource;
use bevy::input::mouse::{MouseScrollUnit, MouseWheel};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use shared::players::Inventory;
use shared::world::ItemStack;

/// Pixels of trackpad scrolling that select the next hotbar slot.
const SCROLL_PIXELS_PER_SLOT: i32 = 20;

/// Opens the inventory when no dialog is open, and closes it when it is open.
pub fn toggle_inventory_system(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    key_map: Res<KeyMap>,
    ui_mode: Res<UIMode>,
    mut inventory_root: Query<&mut Visibility, With<InventoryRoot>>,
) {
    let Ok(mut visibility) = inventory_root.single_mut() else {
        return;
    };
    if !is_action_just_pressed(GameAction::ToggleInventory, &keyboard_input, &key_map) {
        return;
    }
    let is_hidden = *visibility == Visibility::Hidden;
    let a_dialog_is_open = *ui_mode != UIMode::Closed;
    if is_hidden != a_dialog_is_open {
        *visibility = if is_hidden {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
}

/// The mouse wheel changes the selected hotbar slot, during gameplay only.
pub fn select_hotbar_slot_system(
    mut scroll: MessageReader<MouseWheel>,
    ui_mode: Res<UIMode>,
    mut hotbar: Query<&mut Hotbar>,
) {
    if *ui_mode != UIMode::Closed {
        scroll.clear();
        return;
    }
    let Ok(mut hotbar) = hotbar.single_mut() else {
        return;
    };
    let mut selected = hotbar.selected as i32;
    for event in scroll.read() {
        selected -= match event.unit {
            MouseScrollUnit::Line => event.y as i32,
            MouseScrollUnit::Pixel => event.y as i32 / SCROLL_PIXELS_PER_SLOT,
        };
    }
    let selected = selected.rem_euclid(MAX_HOTBAR_SLOTS as i32) as u32;
    if hotbar.selected != selected {
        hotbar.selected = selected;
    }
}

/// Applies a click on a cell of the open inventory to the slot and to the floating stack.
pub fn click_inventory_cells_system(
    mouse_input: Res<ButtonInput<MouseButton>>,
    mut inventory: ResMut<Inventory>,
    inventory_root: Query<&Visibility, With<InventoryRoot>>,
    cells: Query<(&Interaction, &InventoryCell)>,
    mut floating_stack: Query<&mut FloatingStack>,
) {
    let left = mouse_input.just_pressed(MouseButton::Left);
    let right = mouse_input.just_pressed(MouseButton::Right);
    if !left && !right {
        return;
    }
    let (Ok(&Visibility::Visible), Ok(mut floating_stack)) =
        (inventory_root.single(), floating_stack.single_mut())
    else {
        return;
    };

    for (interaction, cell) in &cells {
        if *interaction == Interaction::None {
            continue;
        }
        if left {
            left_click_slot(&mut inventory, cell.id, &mut floating_stack.items);
        } else {
            right_click_slot(&mut inventory, cell.id, &mut floating_stack.items);
        }
    }
}

/// Shows every slot in its cell, the floating stack under the cursor, and outlines the cell
/// that is selected (in the hotbar) or hovered (in the open inventory).
pub fn display_inventory_system(
    inventory: Res<Inventory>,
    materials: Res<MaterialResource>,
    inventory_root: Query<&Visibility, With<InventoryRoot>>,
    hotbar: Query<&Hotbar>,
    window: Query<&Window, With<PrimaryWindow>>,
    mut floating_stack: Query<(&mut Node, &FloatingStack, &Children)>,
    mut cells: Query<(&Interaction, &mut BorderColor, &InventoryCell, &Children)>,
    mut counts: Query<&mut Text, With<StackCount>>,
    mut icons: Query<(&mut ImageNode, &mut Visibility), (With<StackIcon>, Without<InventoryRoot>)>,
) {
    let is_open = inventory_root
        .single()
        .is_ok_and(|visibility| *visibility == Visibility::Visible);
    let selected = hotbar.single().map(|hotbar| hotbar.selected).ok();

    if let Ok((mut node, floating_stack, children)) = floating_stack.single_mut() {
        if let Some(cursor) = window.single().ok().and_then(Window::cursor_position) {
            node.top = Val::Px(cursor.y);
            node.left = Val::Px(cursor.x);
        }
        show_stack(
            floating_stack.items,
            children,
            &mut counts,
            &mut icons,
            &materials,
        );
    }

    for (interaction, mut border_color, cell, children) in &mut cells {
        let in_hotbar = cell.id < MAX_HOTBAR_SLOTS;
        // The cells of a closed inventory are not shown: no need to update them
        if !in_hotbar && !is_open {
            continue;
        }
        let stack = inventory.inner.get(&cell.id).copied();
        show_stack(stack, children, &mut counts, &mut icons, &materials);

        let is_selected = !is_open && selected == Some(cell.id);
        let is_hovered = is_open && *interaction != Interaction::None;
        let border = if is_selected || is_hovered {
            Color::WHITE
        } else {
            CELL_BORDER_COLOR
        };
        border_color.set_if_neq(BorderColor::all(border));
    }
}

/// Shows `stack` with the count and the icon found among `children`, or hides both.
fn show_stack(
    stack: Option<ItemStack>,
    children: &Children,
    counts: &mut Query<&mut Text, With<StackCount>>,
    icons: &mut Query<(&mut ImageNode, &mut Visibility), (With<StackIcon>, Without<InventoryRoot>)>,
    materials: &MaterialResource,
) {
    let count = stack.map(|stack| stack.nb.to_string()).unwrap_or_default();
    let sprite = stack.and_then(|stack| {
        let atlas = materials.items.as_ref()?;
        atlas.sprite(&format!("{:?}", stack.item_id))
    });

    for child in children.iter() {
        if let Ok(mut text) = counts.get_mut(child) {
            if text.0 != count {
                text.0.clone_from(&count);
            }
        }
        if let Ok((mut image, mut visibility)) = icons.get_mut(child) {
            let shown = match &sprite {
                Some(_) => Visibility::Inherited,
                None => Visibility::Hidden,
            };
            visibility.set_if_neq(shown);
            if sprite.is_some() && image.texture_atlas != sprite {
                image.texture_atlas.clone_from(&sprite);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;
    use shared::world::ItemId;

    fn dirt_stack(nb: u32) -> ItemStack {
        ItemStack {
            item_id: ItemId::Dirt,
            item_type: ItemId::Dirt.get_default_type(),
            nb,
        }
    }

    /// A world holding an inventory dialog, a hotbar and one cell per given slot, without
    /// textures: only the counts and the borders are shown.
    struct Hud {
        world: World,
    }

    impl Hud {
        fn new(inventory_visibility: Visibility, slots: &[u32]) -> Self {
            let mut world = World::new();
            world.insert_resource(Inventory::new());
            world.insert_resource(MaterialResource::default());
            world.insert_resource(ButtonInput::<MouseButton>::default());
            world.spawn((InventoryRoot, inventory_visibility));
            world.spawn(Hotbar { selected: 0 });
            world
                .spawn((FloatingStack { items: None }, Node::default()))
                .with_child((StackCount, Text::new("")));
            for &slot in slots {
                world
                    .spawn((
                        InventoryCell { id: slot },
                        Interaction::None,
                        BorderColor::all(CELL_BORDER_COLOR),
                    ))
                    .with_child((StackCount, Text::new("")));
            }
            Self { world }
        }

        fn cell(&mut self, slot: u32) -> Entity {
            let mut cells = self.world.query::<(Entity, &InventoryCell)>();
            let (cell, _) = cells
                .iter(&self.world)
                .find(|(_, cell)| cell.id == slot)
                .unwrap();
            cell
        }

        /// The count and the border shown by the cell of `slot`.
        fn shown(&mut self, slot: u32) -> (String, BorderColor) {
            self.world
                .run_system_once(display_inventory_system)
                .unwrap();
            let cell = self.cell(slot);
            let count = self.world.get::<Children>(cell).unwrap()[0];
            (
                self.world.get::<Text>(count).unwrap().0.clone(),
                *self.world.get::<BorderColor>(cell).unwrap(),
            )
        }

        fn click(&mut self, slot: u32, button: MouseButton) {
            let cell = self.cell(slot);
            self.world.entity_mut(cell).insert(Interaction::Pressed);
            self.world
                .resource_mut::<ButtonInput<MouseButton>>()
                .press(button);
            self.world
                .run_system_once(click_inventory_cells_system)
                .unwrap();
            self.world
                .resource_mut::<ButtonInput<MouseButton>>()
                .clear();
            self.world.entity_mut(cell).insert(Interaction::None);
        }

        fn floating(&mut self) -> Option<ItemStack> {
            let mut floating = self.world.query::<&FloatingStack>();
            floating.single(&self.world).unwrap().items
        }
    }

    #[test]
    fn a_cell_shows_the_count_of_its_slot_and_nothing_when_empty() {
        let mut hud = Hud::new(Visibility::Visible, &[0, 9]);
        hud.world
            .resource_mut::<Inventory>()
            .inner
            .insert(9, dirt_stack(5));

        assert_eq!(hud.shown(9).0, "5");
        assert_eq!(hud.shown(0).0, "");

        hud.world.resource_mut::<Inventory>().inner.remove(&9);
        assert_eq!(hud.shown(9).0, "");
    }

    #[test]
    fn the_selected_hotbar_cell_is_outlined_while_the_inventory_is_closed() {
        let mut hud = Hud::new(Visibility::Hidden, &[0, 1]);
        assert_eq!(hud.shown(0).1, BorderColor::all(Color::WHITE));
        assert_eq!(hud.shown(1).1, BorderColor::all(CELL_BORDER_COLOR));
    }

    #[test]
    fn clicking_a_cell_of_the_open_inventory_moves_items_to_the_cursor_and_back() {
        let mut hud = Hud::new(Visibility::Visible, &[9]);
        hud.world
            .resource_mut::<Inventory>()
            .inner
            .insert(9, dirt_stack(5));

        hud.click(9, MouseButton::Right);
        assert_eq!(hud.floating(), Some(dirt_stack(3)));
        assert_eq!(hud.shown(9).0, "2");

        hud.click(9, MouseButton::Left);
        assert_eq!(hud.floating(), None);
        assert_eq!(hud.shown(9).0, "5");
    }

    #[test]
    fn clicks_do_nothing_while_the_inventory_is_closed() {
        let mut hud = Hud::new(Visibility::Hidden, &[0]);
        hud.world
            .resource_mut::<Inventory>()
            .inner
            .insert(0, dirt_stack(5));

        hud.click(0, MouseButton::Left);

        assert_eq!(hud.floating(), None);
        assert_eq!(hud.world.resource::<Inventory>().inner[&0], dirt_stack(5));
    }

    /// The hotbar and the inventory as the game builds them, with an atlas of two items.
    #[test]
    fn the_cells_built_by_the_setup_systems_show_the_count_and_the_icon_of_their_slot() {
        use crate::ui::hud::hotbar::setup_hotbar;
        use crate::ui::hud::setup_inventory;
        use crate::world::AtlasWrapper;
        use bevy::image::TextureAtlasSources;
        use shared::MAX_INVENTORY_SLOTS;

        let mut images = Assets::<Image>::default();
        let (dirt, sand) = (images.add(Image::default()), images.add(Image::default()));
        let atlas = AtlasWrapper {
            sources: TextureAtlasSources {
                texture_ids: [(dirt.id(), 0), (sand.id(), 1)].into(),
            },
            handles: [("Dirt".to_string(), dirt), ("Sand".to_string(), sand)].into(),
            texture: Handle::default(),
            layout: Handle::default(),
            uvs: Default::default(),
        };

        let mut world = World::new();
        world.insert_resource(MaterialResource {
            items: Some(atlas),
            ..default()
        });
        let mut inventory = Inventory::new();
        inventory.inner.insert(0, dirt_stack(5));
        inventory.inner.insert(
            MAX_HOTBAR_SLOTS,
            ItemStack {
                item_id: ItemId::Sand,
                item_type: ItemId::Sand.get_default_type(),
                nb: 2,
            },
        );
        world.insert_resource(inventory);
        world.run_system_once(setup_hotbar).unwrap();
        world.run_system_once(setup_inventory).unwrap();

        let mut cells = world.query::<&InventoryCell>();
        assert_eq!(cells.iter(&world).count() as u32, MAX_INVENTORY_SLOTS);

        // What the cell of `slot` shows: its count, and the atlas index of its visible icon
        let shown = |world: &mut World, slot: u32| -> (String, Option<usize>) {
            world.run_system_once(display_inventory_system).unwrap();
            let mut cells = world.query::<(&InventoryCell, &Children)>();
            let (_, children) = cells.iter(world).find(|(cell, _)| cell.id == slot).unwrap();
            let mut count = String::new();
            let mut icon = None;
            for child in children.iter() {
                if let Some(text) = world.get::<Text>(child) {
                    count = text.0.clone();
                }
                if let Some(image) = world.get::<ImageNode>(child) {
                    if *world.get::<Visibility>(child).unwrap() != Visibility::Hidden {
                        icon = image.texture_atlas.as_ref().map(|atlas| atlas.index);
                    }
                }
            }
            (count, icon)
        };

        assert_eq!(shown(&mut world, 0), ("5".to_string(), Some(0)));
        assert_eq!(shown(&mut world, 1), (String::new(), None));
        // The cells of the closed inventory are not updated
        assert_eq!(shown(&mut world, MAX_HOTBAR_SLOTS), (String::new(), None));

        let mut root = world.query_filtered::<&mut Visibility, With<InventoryRoot>>();
        *root.single_mut(&mut world).unwrap() = Visibility::Visible;
        assert_eq!(
            shown(&mut world, MAX_HOTBAR_SLOTS),
            ("2".to_string(), Some(1))
        );
    }
}
