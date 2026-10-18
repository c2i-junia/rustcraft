use super::{CELL_BORDER_COLOR, CRAFTING_TABLE_TITLE, INVENTORY_TITLE};
use crate::constants::MAX_HOTBAR_SLOTS;
use crate::input::data::GameAction;
use crate::input::keyboard::is_action_just_pressed;
use crate::network::buffered_client::CurrentFrameInputs;
use crate::state::KeyMap;
use crate::ui::hud::hotbar::Hotbar;
use crate::ui::hud::{
    CraftResultCell, CraftingGridCell, FloatingStack, InventoryCell, InventoryRoot, InventoryTitle,
    StackCount, StackIcon, UIMode,
};
use crate::world::MaterialResource;
use bevy::input::mouse::{MouseScrollUnit, MouseWheel};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use shared::players::inventory::{InventoryAction, INVENTORY_GRID_SIZE};
use shared::players::Inventory;
use shared::world::{match_recipe, ItemStack, CRAFTING_GRID_SIZE};

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

/// Sends a click on a cell of the open inventory as an action of the frame's input, which the
/// client predicts and the server applies.
pub fn click_inventory_cells_system(
    mouse_input: Res<ButtonInput<MouseButton>>,
    inventory_root: Query<&Visibility, With<InventoryRoot>>,
    cells: Query<(&Interaction, &InventoryCell)>,
    result: Query<&Interaction, With<CraftResultCell>>,
    mut frame_inputs: ResMut<CurrentFrameInputs>,
) {
    let left = mouse_input.just_pressed(MouseButton::Left);
    let right = mouse_input.just_pressed(MouseButton::Right);
    if !left && !right {
        return;
    }
    if !matches!(inventory_root.single(), Ok(&Visibility::Visible)) {
        return;
    }

    for (interaction, cell) in &cells {
        if *interaction == Interaction::None {
            continue;
        }
        frame_inputs.0.inventory_actions.push(if left {
            InventoryAction::LeftClickSlot(cell.id)
        } else {
            InventoryAction::RightClickSlot(cell.id)
        });
    }
    if result
        .iter()
        .any(|interaction| *interaction != Interaction::None)
    {
        frame_inputs
            .0
            .inventory_actions
            .push(InventoryAction::TakeCraftResult);
    }
}

/// Sends `Close` when the inventory gets hidden, whichever key hid it, so that the stack held
/// by the cursor goes back into the inventory.
pub fn close_inventory_system(
    inventory_root: Query<&Visibility, With<InventoryRoot>>,
    mut was_open: Local<bool>,
    mut frame_inputs: ResMut<CurrentFrameInputs>,
) {
    let is_open = matches!(inventory_root.single(), Ok(&Visibility::Visible));
    if *was_open && !is_open {
        frame_inputs
            .0
            .inventory_actions
            .push(InventoryAction::Close);
    }
    *was_open = is_open;
}

/// Shows every slot in its cell, the floating stack under the cursor, and outlines the cell
/// that is selected (in the hotbar) or hovered (in the open inventory).
pub fn display_inventory_system(
    inventory: Res<Inventory>,
    materials: Res<MaterialResource>,
    inventory_root: Query<&Visibility, With<InventoryRoot>>,
    hotbar: Query<&Hotbar>,
    window: Query<&Window, With<PrimaryWindow>>,
    mut floating_stack: Query<(&mut Node, &Children), With<FloatingStack>>,
    mut cells: Query<(&Interaction, &mut BorderColor, &InventoryCell, &Children)>,
    mut counts: Query<&mut Text, With<StackCount>>,
    mut icons: Query<(&mut ImageNode, &mut Visibility), (With<StackIcon>, Without<InventoryRoot>)>,
) {
    let is_open = inventory_root
        .single()
        .is_ok_and(|visibility| *visibility == Visibility::Visible);
    let selected = hotbar.single().map(|hotbar| hotbar.selected).ok();

    if let Ok((mut node, children)) = floating_stack.single_mut() {
        if let Some(cursor) = window.single().ok().and_then(Window::cursor_position) {
            node.top = Val::Px(cursor.y);
            node.left = Val::Px(cursor.x);
        }
        show_stack(
            inventory.floating,
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

/// Shows the cells of the crafting grid in use and, in the result cell, what the grid makes.
/// The grid is 3x3 and the dialog named after the table while a crafting table is open.
pub fn display_crafting_system(
    inventory: Res<Inventory>,
    materials: Res<MaterialResource>,
    mut grid_cells: Query<(&CraftingGridCell, &mut Node)>,
    mut result: Query<(&Interaction, &mut BorderColor, &Children), With<CraftResultCell>>,
    mut title: Query<&mut Text, (With<InventoryTitle>, Without<StackCount>)>,
    mut counts: Query<&mut Text, With<StackCount>>,
    mut icons: Query<(&mut ImageNode, &mut Visibility), (With<StackIcon>, Without<InventoryRoot>)>,
) {
    // The server checks that the table is still within reach; the dialog follows its updates
    let (size, name) = match inventory.open_table {
        Some(_) => (CRAFTING_GRID_SIZE, CRAFTING_TABLE_TITLE),
        None => (INVENTORY_GRID_SIZE, INVENTORY_TITLE),
    };
    if let Ok(mut title) = title.single_mut() {
        if title.0 != name {
            title.0 = name.to_string();
        }
    }
    for (cell, mut node) in &mut grid_cells {
        let display = if cell.row < size && cell.col < size {
            Display::Flex
        } else {
            Display::None
        };
        if node.display != display {
            node.display = display;
        }
    }

    let made = match_recipe(&inventory.crafting_grid());
    for (interaction, mut border_color, children) in &mut result {
        show_stack(made, children, &mut counts, &mut icons, &materials);
        let border = if *interaction != Interaction::None {
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
    use crate::ui::hud::InventoryTitle;
    use bevy::ecs::system::RunSystemOnce;
    use shared::world::{grid_slot, ItemId};

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
            world.init_resource::<CurrentFrameInputs>();
            world.spawn((InventoryRoot, inventory_visibility));
            world.spawn(Hotbar { selected: 0 });
            world
                .spawn((FloatingStack, Node::default()))
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

        fn spawn_result_cell(&mut self) -> Entity {
            self.world
                .spawn((
                    CraftResultCell,
                    Interaction::None,
                    BorderColor::all(CELL_BORDER_COLOR),
                ))
                .with_child((StackCount, Text::new("")))
                .id()
        }

        fn spawn_grid_cell(&mut self, row: u32, col: u32) -> Entity {
            self.world
                .spawn((
                    InventoryCell {
                        id: grid_slot(row, col),
                    },
                    CraftingGridCell { row, col },
                    Interaction::None,
                    BorderColor::all(CELL_BORDER_COLOR),
                    Node::default(),
                ))
                .with_child((StackCount, Text::new("")))
                .id()
        }

        fn show_crafting(&mut self) {
            self.world.run_system_once(display_crafting_system).unwrap();
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

        /// The count shown by the floating stack.
        fn floating_shown(&mut self) -> String {
            self.world
                .run_system_once(display_inventory_system)
                .unwrap();
            let mut floating = self
                .world
                .query_filtered::<&Children, With<FloatingStack>>();
            let count = floating.single(&self.world).unwrap()[0];
            self.world.get::<Text>(count).unwrap().0.clone()
        }

        fn sent_actions(&self) -> Vec<InventoryAction> {
            self.world
                .resource::<CurrentFrameInputs>()
                .0
                .inventory_actions
                .clone()
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
    fn a_click_on_a_cell_of_the_open_inventory_is_sent_as_an_inventory_action() {
        let mut hud = Hud::new(Visibility::Visible, &[9]);
        hud.world
            .resource_mut::<Inventory>()
            .inner
            .insert(9, dirt_stack(5));

        hud.click(9, MouseButton::Right);
        hud.click(9, MouseButton::Left);

        assert_eq!(
            hud.sent_actions(),
            [
                InventoryAction::RightClickSlot(9),
                InventoryAction::LeftClickSlot(9)
            ]
        );
        // The click is predicted with the other inputs of the frame, not here
        assert_eq!(hud.shown(9).0, "5");
    }

    #[test]
    fn the_result_cell_shows_the_recipe_output() {
        let mut hud = Hud::new(Visibility::Visible, &[]);
        let result = hud.spawn_result_cell();
        let shown = |hud: &mut Hud| {
            hud.show_crafting();
            let count = hud.world.get::<Children>(result).unwrap()[0];
            hud.world.get::<Text>(count).unwrap().0.clone()
        };
        assert_eq!(shown(&mut hud), "");

        hud.world.resource_mut::<Inventory>().inner.insert(
            grid_slot(0, 0),
            ItemStack {
                item_id: ItemId::OakLog,
                item_type: ItemId::OakLog.get_default_type(),
                nb: 1,
            },
        );
        assert_eq!(shown(&mut hud), "4");
    }

    #[test]
    fn clicking_the_result_cell_sends_take_craft_result() {
        let mut hud = Hud::new(Visibility::Visible, &[]);
        let result = hud.spawn_result_cell();
        hud.world.entity_mut(result).insert(Interaction::Pressed);
        hud.world
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);

        hud.world
            .run_system_once(click_inventory_cells_system)
            .unwrap();

        assert_eq!(hud.sent_actions(), [InventoryAction::TakeCraftResult]);
    }

    #[test]
    fn the_grid_shows_only_its_2x2_corner_without_a_table() {
        let mut hud = Hud::new(Visibility::Visible, &[]);
        let inside = hud.spawn_grid_cell(1, 1);
        let outside = [hud.spawn_grid_cell(0, 2), hud.spawn_grid_cell(2, 0)];

        hud.show_crafting();

        assert_ne!(
            hud.world.get::<Node>(inside).unwrap().display,
            Display::None
        );
        for cell in outside {
            assert_eq!(hud.world.get::<Node>(cell).unwrap().display, Display::None);
        }
    }

    #[test]
    fn the_grid_shows_3x3_and_the_title_names_the_table_while_one_is_open() {
        let mut hud = Hud::new(Visibility::Visible, &[]);
        let corner = hud.spawn_grid_cell(2, 2);
        let title = hud
            .world
            .spawn((InventoryTitle, Text::new("Inventory")))
            .id();

        hud.world.resource_mut::<Inventory>().open_table = Some(IVec3::new(1, 2, 3));
        hud.show_crafting();
        assert_ne!(
            hud.world.get::<Node>(corner).unwrap().display,
            Display::None
        );
        assert_eq!(hud.world.get::<Text>(title).unwrap().0, "Crafting Table");

        hud.world.resource_mut::<Inventory>().open_table = None;
        hud.show_crafting();
        assert_eq!(
            hud.world.get::<Node>(corner).unwrap().display,
            Display::None
        );
        assert_eq!(hud.world.get::<Text>(title).unwrap().0, "Inventory");
    }

    #[test]
    fn the_floating_stack_shows_the_stack_held_by_the_cursor() {
        let mut hud = Hud::new(Visibility::Visible, &[]);
        assert_eq!(hud.floating_shown(), "");

        hud.world.resource_mut::<Inventory>().floating = Some(dirt_stack(3));
        assert_eq!(hud.floating_shown(), "3");
    }

    #[test]
    fn hiding_the_inventory_sends_close_once() {
        let mut hud = Hud::new(Visibility::Visible, &[]);
        let close = hud.world.register_system(close_inventory_system);
        hud.world.run_system(close).unwrap();
        assert!(hud.sent_actions().is_empty());

        let mut root = hud
            .world
            .query_filtered::<&mut Visibility, With<InventoryRoot>>();
        *root.single_mut(&mut hud.world).unwrap() = Visibility::Hidden;
        for _ in 0..2 {
            hud.world.run_system(close).unwrap();
        }

        assert_eq!(hud.sent_actions(), [InventoryAction::Close]);
    }

    #[test]
    fn clicks_do_nothing_while_the_inventory_is_closed() {
        let mut hud = Hud::new(Visibility::Hidden, &[0]);
        hud.world
            .resource_mut::<Inventory>()
            .inner
            .insert(0, dirt_stack(5));

        hud.click(0, MouseButton::Left);

        assert!(hud.sent_actions().is_empty());
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
        // The slots of the inventory, then the 3x3 crafting grid
        assert_eq!(cells.iter(&world).count() as u32, MAX_INVENTORY_SLOTS + 9);

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
