use super::gestures::{planned_drag, CellGestures};
use super::{item_name, CHEST_TITLE, CRAFTING_TABLE_TITLE, FURNACE_TITLE, INVENTORY_TITLE};
use crate::constants::MAX_HOTBAR_SLOTS;
use crate::input::data::GameAction;
use crate::input::keyboard::is_action_just_pressed;
use crate::network::buffered_client::CurrentFrameInputs;
use crate::state::KeyMap;
use crate::ui::hud::hotbar::{hotbar_key_index, Hotbar};
use crate::ui::hud::item_browser::ItemBrowser;
use crate::ui::hud::{
    ChestGrid, CraftResultCell, CraftingGridCell, CraftingRow, FloatingStack, FurnaceRow,
    InventoryCell, InventoryDialog, InventoryRoot, InventoryTitle, UIMode,
};
use crate::ui::theme::{ACCENT, BEVEL_HI, BEVEL_LO, PANEL, TEXT};
use crate::ui::widgets::slot::{
    count_label, durability_bar, tool_wear, Slot, SlotCount, SlotDurability, SlotIcon,
    SlotSelected, ICON_SIZE, SLOT_SIZE,
};
use crate::ui::widgets::tooltip::Tooltip;
use crate::world::{ClientWorldMap, MaterialResource};
use bevy::input_focus::tab_navigation::TabIndex;
use bevy::input_focus::{FocusCause, InputFocus, InputFocusVisible};
use bevy::picking::hover::Hovered;
use bevy::prelude::*;
use bevy::ui::UiGlobalTransform;
use bevy::window::PrimaryWindow;
use shared::players::inventory::{
    chest_slots, craft_result, InventoryAction, CHEST_SLOT_START, FURNACE_FUEL_SLOT,
    FURNACE_INPUT_SLOT, FURNACE_OUTPUT_SLOT, INVENTORY_GRID_SIZE,
};
use shared::players::Inventory;
use shared::world::{fuel_ticks, grid_slot, smelting_result, ItemStack, CRAFTING_GRID_SIZE};
use shared::MAX_INVENTORY_SLOTS;

/// What a click does on a slot of the hotbar, of the backpack, of the crafting grid, of a chest
/// or of a furnace (`HINT_GRID`), on a slot of the inventory while a chest is open, or while a
/// furnace is open when it holds what smelts or burns, and on the result.
pub const HINT_HOTBAR: &str = "Shift+click: move to the backpack";
pub const HINT_BACKPACK: &str = "Shift+click: move to the hotbar";
pub const HINT_GRID: &str = "Shift+click: move to the inventory";
pub const HINT_TO_CHEST: &str = "Shift+click: move to the chest";
pub const HINT_TO_FURNACE: &str = "Shift+click: move to the furnace";
pub const HINT_RESULT: &str = "Click: craft";

/// The durability bars of the slots, which `show_stack` sizes and colours.
type DurabilityBars<'w, 's> = Query<
    'w,
    's,
    (
        &'static mut Node,
        &'static mut BackgroundColor,
        &'static mut Visibility,
    ),
    (
        With<SlotDurability>,
        Without<SlotIcon>,
        Without<InventoryRoot>,
        Without<FloatingStack>,
        Without<CraftingGridCell>,
        Without<ChestGrid>,
        Without<CraftingRow>,
        Without<FurnaceRow>,
    ),
>;

/// The counts of the slots, which `show_stack` writes and colours.
type SlotCounts<'w, 's> =
    Query<'w, 's, (&'static mut Text, Option<&'static mut TextColor>), With<SlotCount>>;

/// Opens the inventory when no dialog is open, and closes it when it is open. Not while the player
/// types: the key is then a letter of the text.
pub fn toggle_inventory_system(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    key_map: Res<KeyMap>,
    ui_mode: Res<UIMode>,
    mut inventory_root: Query<&mut Visibility, With<InventoryRoot>>,
) {
    let Ok(mut visibility) = inventory_root.single_mut() else {
        return;
    };
    if *ui_mode == UIMode::Typing
        || !is_action_just_pressed(GameAction::ToggleInventory, &keyboard_input, &key_map)
    {
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

/// Sends a press on a cell of the open inventory, or a number key pressed over one, as an action
/// of the frame's input, which the client predicts and the server applies. A press with a held
/// stack starts a drag (`CellGestures`), sent on release; the other button cancels it. Shift
/// turns the left click into a move of the whole stack, and on the result into crafting all; a
/// number key over the result crafts into that hotbar slot. While the focus ring shows, the keys
/// act on the ringed cell, wherever the cursor is: Enter is the left click, Space the right one,
/// and a number key swaps it. Without the ring, the focus is not one the player can see: the
/// keys leave it alone, and a number key goes to the cell under the cursor. While the player
/// types, in the item browser's search, the keys are letters of the text: only the mouse acts.
#[allow(clippy::too_many_arguments)]
pub fn click_inventory_cells_system(
    mouse_input: Res<ButtonInput<MouseButton>>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    ui_mode: Res<UIMode>,
    focus: Res<InputFocus>,
    focus_visible: Res<InputFocusVisible>,
    inventory_root: Query<&Visibility, With<InventoryRoot>>,
    cells: Query<(Entity, &Hovered, &InventoryCell)>,
    result: Query<(Entity, &Hovered), With<CraftResultCell>>,
    inventory: Res<Inventory>,
    world_map: Res<ClientWorldMap>,
    time: Res<Time<Real>>,
    mut gestures: ResMut<CellGestures>,
    mut frame_inputs: ResMut<CurrentFrameInputs>,
) {
    if !matches!(inventory_root.single(), Ok(&Visibility::Visible)) {
        return;
    }
    let left = mouse_input.just_pressed(MouseButton::Left);
    let right = mouse_input.just_pressed(MouseButton::Right);
    let keys = *ui_mode != UIMode::Typing;
    let enter = keys && keyboard_input.just_pressed(KeyCode::Enter);
    let space = keys && keyboard_input.just_pressed(KeyCode::Space);
    let shift = keyboard_input.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]);
    let hotbar = hotbar_key_index(&keyboard_input).filter(|_| keys);
    let focused = focus.get().filter(|_| focus_visible.0);
    let actions = &mut frame_inputs.0.inventory_actions;

    let hovered_slot = cells
        .iter()
        .find(|(_, hovered, _)| hovered.0)
        .map(|(_, _, cell)| cell.id);
    let focused_slot = cells
        .iter()
        .find(|(entity, _, _)| Some(*entity) == focused)
        .map(|(_, _, cell)| cell.id);
    let result_hovered = result.iter().any(|(_, hovered)| hovered.0);
    let result_focused = result.iter().any(|(entity, _)| Some(entity) == focused);

    // A press of the mouse on the hovered cell or result
    let pressed = match (left, right) {
        (true, _) => Some(MouseButton::Left),
        (false, true) => Some(MouseButton::Right),
        (false, false) => None,
    };
    if let Some(button) = pressed {
        if let Some(drag) = gestures.drag {
            if drag.button != button {
                gestures.drag = None;
            }
        } else if let Some(slot) = hovered_slot {
            let in_slot = stack_in(slot, &inventory, &world_map);
            let now = time.elapsed();
            let held = inventory.floating;
            actions.extend(gestures.press(slot, button, shift, held, in_slot, now));
        } else {
            gestures.forget_press();
            if result_hovered {
                actions.push(if shift && left {
                    InventoryAction::CraftAll
                } else {
                    InventoryAction::TakeCraftResult
                });
            }
        }
    }

    // A key on the focused cell or result
    if let Some(slot) = focused_slot {
        if enter {
            actions.push(if shift {
                InventoryAction::QuickMoveSlot(slot)
            } else {
                InventoryAction::LeftClickSlot(slot)
            });
        } else if space {
            actions.push(InventoryAction::RightClickSlot(slot));
        }
    }
    if result_focused && (enter || space) {
        actions.push(if shift && enter {
            InventoryAction::CraftAll
        } else {
            InventoryAction::TakeCraftResult
        });
    }
    let (key_slot, key_on_result) = if focused.is_some() {
        (focused_slot, result_focused)
    } else {
        (hovered_slot, result_hovered)
    };
    match (hotbar, key_slot) {
        (Some(hotbar), Some(slot)) => {
            actions.push(InventoryAction::SwapWithHotbar { slot, hotbar })
        }
        (Some(hotbar), None) if key_on_result => {
            actions.push(InventoryAction::CraftToHotbar(hotbar));
        }
        _ => {}
    }
}

/// Puts the focus on the selected hotbar slot when the inventory opens, without its ring until a
/// key moves it, and takes it back when the inventory closes. While it is open, a click on no
/// slot clears the focus, and a crafting cell that disappears lets go of it: it goes back to the
/// last focused cell while it is shown, or to the selected slot, so that the keyboard always has
/// a cell to start from. A focus in the item browser beside the panel is kept as one inside it.
/// The slots of the closed inventory leave the Tab and arrow navigation, as do the hidden cells,
/// of the crafting grid or of the chest, so that Tab in the game never lands on the hotbar.
pub fn sync_inventory_focus(
    inventory_root: Query<&Visibility, With<InventoryRoot>>,
    hotbar: Query<&Hotbar>,
    mut slots: Query<
        (Entity, &mut TabIndex, Option<&InventoryCell>),
        Or<(With<InventoryCell>, With<CraftResultCell>)>,
    >,
    nodes: Query<&Node>,
    dialog: Query<(), Or<(With<InventoryDialog>, With<ItemBrowser>)>>,
    parents: Query<&ChildOf>,
    mut was_open: Local<bool>,
    mut last_focused: Local<Option<Entity>>,
    mut focus: ResMut<InputFocus>,
    mut focus_visible: ResMut<InputFocusVisible>,
) {
    let is_open = matches!(inventory_root.single(), Ok(&Visibility::Visible));
    // A cell is hidden by its own node, or by that of a row holding it
    let shown = |cell: Entity| {
        std::iter::once(cell)
            .chain(parents.iter_ancestors(cell))
            .all(|entity| {
                nodes
                    .get(entity)
                    .map_or(true, |node| node.display != Display::None)
            })
    };
    for (cell, mut index, _) in &mut slots {
        index.set_if_neq(TabIndex(if is_open && shown(cell) { 0 } else { -1 }));
    }
    let in_dialog = focus.get().is_some_and(|focused| {
        parents
            .iter_ancestors(focused)
            .any(|ancestor| dialog.contains(ancestor))
    });
    let on_hidden_cell = focus
        .get()
        .is_some_and(|focused| slots.contains(focused) && !shown(focused));

    if is_open {
        if !*was_open {
            *last_focused = None;
            focus_visible.0 = false;
        }
        if *was_open && in_dialog && !on_hidden_cell {
            *last_focused = focus.get();
        } else {
            let selected = hotbar.single().map(|hotbar| hotbar.selected).ok();
            let selected_cell = || {
                slots.iter().find_map(|(entity, _, cell)| {
                    cell.is_some_and(|cell| Some(cell.id) == selected)
                        .then_some(entity)
                })
            };
            let target = last_focused
                .filter(|&cell| slots.contains(cell) && shown(cell))
                .or_else(selected_cell);
            if let Some(target) = target {
                focus.set(target, FocusCause::Navigated);
            }
            *last_focused = target;
        }
    } else if *was_open {
        if in_dialog {
            focus.clear();
        }
        *last_focused = None;
    }
    *was_open = is_open;
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

/// Shows every slot in its cell and the floating stack centred on the cursor, or on the focused
/// slot while the focus shows, and selects the cell of the selected hotbar slot, whether the
/// inventory is open or not.
pub fn display_inventory_system(
    inventory: Res<Inventory>,
    world_map: Res<ClientWorldMap>,
    materials: Res<MaterialResource>,
    inventory_root: Query<&Visibility, With<InventoryRoot>>,
    hotbar: Query<&Hotbar>,
    window: Query<&Window, With<PrimaryWindow>>,
    ui_scale: Res<UiScale>,
    focus: Res<InputFocus>,
    focus_visible: Res<InputFocusVisible>,
    gestures: Res<CellGestures>,
    slots: Query<(&UiGlobalTransform, &ComputedNode), With<Slot>>,
    mut floating_stack: Query<(&mut Node, &Children), With<FloatingStack>>,
    mut cells: Query<(&mut SlotSelected, &InventoryCell, &mut Tooltip, &Children)>,
    mut counts: SlotCounts,
    mut icons: Query<(&mut ImageNode, &mut Visibility), (With<SlotIcon>, Without<InventoryRoot>)>,
    mut bars: DurabilityBars,
) {
    let is_open = inventory_root
        .single()
        .is_ok_and(|visibility| *visibility == Visibility::Visible);
    let selected = hotbar.single().map(|hotbar| hotbar.selected).ok();
    // A drag over two slots or more shows what it would leave; over one, it is a click
    let plan = gestures
        .drag
        .filter(|drag| drag.slots.count_ones() > 1)
        .and_then(|drag| planned_drag(&inventory, &world_map, drag.slots, drag.one_each()));
    let held = match &plan {
        Some(plan) => plan.held,
        None => inventory.floating,
    };

    if let Ok((mut node, children)) = floating_stack.single_mut() {
        let focused_slot = focus
            .get()
            .filter(|_| focus_visible.0)
            .and_then(|focused| slots.get(focused).ok())
            .map(|(transform, slot)| transform.translation * slot.inverse_scale_factor());
        // `Val::Px` is multiplied by the UI scale: the window position is divided by it
        let cursor = || {
            let cursor = window.single().ok()?.cursor_position()?;
            Some(cursor / ui_scale.0)
        };
        if let Some(center) = focused_slot.or_else(cursor) {
            let top_left = center - SLOT_SIZE / 2.;
            // A write marks the node changed, which lays the UI out again: only on a move
            if node.left != Val::Px(top_left.x) || node.top != Val::Px(top_left.y) {
                node.left = Val::Px(top_left.x);
                node.top = Val::Px(top_left.y);
            }
        }
        show_stack(
            held,
            TEXT,
            children,
            &mut counts,
            &mut icons,
            &mut bars,
            &materials,
        );
    }

    for (mut is_selected, cell, mut tooltip, children) in &mut cells {
        is_selected.set_if_neq(SlotSelected(selected == Some(cell.id)));
        let planned = plan.as_ref().and_then(|plan| {
            let (_, stack) = plan.slots.iter().find(|(slot, _)| *slot == cell.id)?;
            Some(*stack)
        });
        let stack = planned.or_else(|| stack_in(cell.id, &inventory, &world_map));
        let hint = if inventory.open_chest.is_some() && cell.id < MAX_INVENTORY_SLOTS {
            HINT_TO_CHEST
        } else if inventory.open_furnace.is_some() && cell.id < MAX_INVENTORY_SLOTS {
            // A shift-click leaves what neither smelts nor burns
            match stack {
                Some(stack)
                    if smelting_result(stack.item_id).is_some()
                        || fuel_ticks(stack.item_id).is_some() =>
                {
                    HINT_TO_FURNACE
                }
                _ => "",
            }
        } else if cell.id < MAX_HOTBAR_SLOTS {
            HINT_HOTBAR
        } else if cell.id < MAX_INVENTORY_SLOTS {
            HINT_BACKPACK
        } else {
            HINT_GRID
        };
        tooltip.set_if_neq(tooltip_of(stack, hint, is_open, &inventory));
        // The cells of a closed inventory are not shown: no need to update them
        if cell.id >= MAX_HOTBAR_SLOTS && !is_open {
            continue;
        }
        let color = if planned.is_some() { ACCENT } else { TEXT };
        show_stack(
            stack,
            color,
            children,
            &mut counts,
            &mut icons,
            &mut bars,
            &materials,
        );
    }
}

/// What the cell of `slot` shows: a slot of the open chest or furnace shows what it holds, once
/// the server sent it.
pub(crate) fn stack_in(
    slot: u32,
    inventory: &Inventory,
    world_map: &ClientWorldMap,
) -> Option<ItemStack> {
    if chest_slots().contains(&slot) {
        let chest = world_map.blocks.container(inventory.open_chest?)?;
        return chest.slots[(slot - CHEST_SLOT_START) as usize];
    }
    let furnace = || world_map.blocks.furnace(inventory.open_furnace?);
    match slot {
        FURNACE_INPUT_SLOT => furnace()?.input,
        FURNACE_FUEL_SLOT => furnace()?.fuel,
        FURNACE_OUTPUT_SLOT => furnace()?.output,
        _ => inventory.inner.get(&slot).copied(),
    }
}

/// The width and height of the crafting grid in use: 3 at a crafting table, 2 in the inventory
/// alone and beside a chest. The server checks that the table is still within reach; the panel
/// follows its updates.
pub fn grid_in_use(inventory: &Inventory) -> u32 {
    match (inventory.open_chest, inventory.open_table) {
        (None, Some(_)) => CRAFTING_GRID_SIZE,
        _ => INVENTORY_GRID_SIZE,
    }
}

/// Draws the panel, a raised bevel on its background, while the inventory is open: closed, only
/// the hotbar shows.
pub fn style_inventory_panel(
    inventory_root: Query<&Visibility, With<InventoryRoot>>,
    mut panel: Query<(&mut BackgroundColor, &mut BorderColor), With<InventoryDialog>>,
) {
    let is_open = inventory_root
        .single()
        .is_ok_and(|visibility| *visibility == Visibility::Visible);
    let (background, border) = if is_open {
        (
            PANEL,
            BorderColor {
                top: BEVEL_HI,
                left: BEVEL_HI,
                bottom: BEVEL_LO,
                right: BEVEL_LO,
            },
        )
    } else {
        (Color::NONE, BorderColor::all(Color::NONE))
    };
    for (mut panel_background, mut panel_border) in &mut panel {
        panel_background.set_if_neq(BackgroundColor(background));
        panel_border.set_if_neq(border);
    }
}

/// Shows the cells of the crafting grid in use and, in the result cell, what the grid makes.
/// The grid is 3x3 and the dialog named after the table while a crafting table is open. While a
/// chest or a furnace is open, its slots take the place of the crafting grid, and the dialog its
/// name.
pub fn display_crafting_system(
    inventory: Res<Inventory>,
    materials: Res<MaterialResource>,
    inventory_root: Query<&Visibility, With<InventoryRoot>>,
    mut grid_cells: Query<(&CraftingGridCell, &mut Node)>,
    mut rows: Query<
        (&mut Node, Has<ChestGrid>, Has<FurnaceRow>),
        (
            Or<(With<ChestGrid>, With<CraftingRow>, With<FurnaceRow>)>,
            Without<CraftingGridCell>,
        ),
    >,
    mut result: Query<(&Children, &mut Tooltip), With<CraftResultCell>>,
    mut title: Query<&mut Text, (With<InventoryTitle>, Without<SlotCount>)>,
    mut counts: SlotCounts,
    mut icons: Query<(&mut ImageNode, &mut Visibility), (With<SlotIcon>, Without<InventoryRoot>)>,
    mut bars: DurabilityBars,
) {
    // The server checks that the table, the chest or the furnace is still within reach; the
    // dialog follows its updates
    let size = grid_in_use(&inventory);
    let name = match (
        inventory.open_chest,
        inventory.open_furnace,
        inventory.open_table,
    ) {
        (Some(_), _, _) => CHEST_TITLE,
        (None, Some(_), _) => FURNACE_TITLE,
        (None, None, Some(_)) => CRAFTING_TABLE_TITLE,
        (None, None, None) => INVENTORY_TITLE,
    };
    if let Ok(mut title) = title.single_mut() {
        if title.0 != name {
            title.0 = name.to_string();
        }
    }
    let chest_open = inventory.open_chest.is_some();
    let furnace_open = inventory.open_furnace.is_some();
    for (mut node, is_chest, is_furnace) in &mut rows {
        let shown = if is_chest {
            chest_open
        } else if is_furnace {
            furnace_open
        } else {
            !chest_open && !furnace_open
        };
        let display = match (shown, is_chest) {
            (true, true) => Display::Grid,
            (true, false) => Display::Flex,
            (false, _) => Display::None,
        };
        if node.display != display {
            node.display = display;
        }
    }
    for (cell, mut node) in &mut grid_cells {
        // A cell beyond the grid in use still shows what was left in it, to be taken out
        let holds_items = inventory.inner.contains_key(&grid_slot(cell.row, cell.col));
        let display = if (cell.row < size && cell.col < size) || holds_items {
            Display::Flex
        } else {
            Display::None
        };
        if node.display != display {
            node.display = display;
        }
    }

    let made = craft_result(&inventory, grid_in_use(&inventory));
    let is_open = inventory_root
        .single()
        .is_ok_and(|visibility| *visibility == Visibility::Visible);
    for (children, mut tooltip) in &mut result {
        tooltip.set_if_neq(tooltip_of(made, HINT_RESULT, is_open, &inventory));
        show_stack(
            made,
            TEXT,
            children,
            &mut counts,
            &mut icons,
            &mut bars,
            &materials,
        );
    }
}

/// The tooltip of a slot holding `stack`: nothing while the inventory is closed, while the cursor
/// holds a stack, or on an empty slot.
fn tooltip_of(
    stack: Option<ItemStack>,
    hint: &str,
    is_open: bool,
    inventory: &Inventory,
) -> Tooltip {
    match stack {
        Some(stack) if is_open && inventory.floating.is_none() => Tooltip {
            title: item_name(stack.item_id),
            detail: match tool_wear(&stack) {
                Some((wear, durability)) => durability_detail(wear, durability),
                None => count_detail(stack.nb),
            },
            note: String::new(),
            hint: hint.to_string(),
        },
        _ => Tooltip::default(),
    }
}

/// The count of a stack in words.
fn count_detail(nb: u32) -> String {
    match nb {
        1 => "1 item".to_string(),
        nb => format!("{nb} items"),
    }
}

/// What is left of a tool, in words.
fn durability_detail(wear: u32, durability: u32) -> String {
    format!(
        "Durability: {} / {durability}",
        durability.saturating_sub(wear)
    )
}

/// Shows `stack` with the count, in `count_color`, the icon and the durability bar found among
/// `children`, or hides them.
fn show_stack(
    stack: Option<ItemStack>,
    count_color: Color,
    children: &Children,
    counts: &mut SlotCounts,
    icons: &mut Query<(&mut ImageNode, &mut Visibility), (With<SlotIcon>, Without<InventoryRoot>)>,
    bars: &mut DurabilityBars,
    materials: &MaterialResource,
) {
    let bar = stack.as_ref().and_then(durability_bar);
    let count = stack.map(|stack| count_label(stack.nb)).unwrap_or_default();
    let sprite = stack.and_then(|stack| {
        let atlas = materials.items.as_ref()?;
        atlas.sprite(&format!("{:?}", stack.item_id))
    });

    for child in children.iter() {
        if let Ok((mut text, color)) = counts.get_mut(child) {
            if text.0 != count {
                text.0.clone_from(&count);
            }
            if let Some(mut color) = color {
                color.set_if_neq(TextColor(count_color));
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
        if let Ok((mut node, mut color, mut visibility)) = bars.get_mut(child) {
            let Some((left, bar_color)) = bar else {
                visibility.set_if_neq(Visibility::Hidden);
                continue;
            };
            visibility.set_if_neq(Visibility::Inherited);
            color.set_if_neq(BackgroundColor(bar_color));
            let width = px(ICON_SIZE * left);
            if node.width != width {
                node.width = width;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::hud::{
        drag_inventory_cells_system, InventoryDialog, InventoryTitle, DOUBLE_CLICK_WINDOW,
    };
    use crate::ui::theme::PANEL;
    use crate::ui::widgets::slot::slot_durability;
    use bevy::ecs::system::RunSystemOnce;
    use shared::players::inventory::{FURNACE_FUEL_SLOT, FURNACE_INPUT_SLOT, FURNACE_OUTPUT_SLOT};
    use shared::world::ItemType;
    use shared::world::{grid_slot, Furnace, ItemId};

    fn dirt_stack(nb: u32) -> ItemStack {
        ItemStack {
            item_id: ItemId::Dirt,
            item_type: ItemId::Dirt.get_default_type(),
            nb,
        }
    }

    /// A world holding an inventory panel, a hotbar and one cell per given slot, without
    /// textures: only the counts and the selection are shown.
    struct Hud {
        world: World,
    }

    impl Hud {
        fn new(inventory_visibility: Visibility, slots: &[u32]) -> Self {
            let mut world = World::new();
            world.insert_resource(Inventory::new());
            world.init_resource::<ClientWorldMap>();
            world.insert_resource(MaterialResource::default());
            world.insert_resource(ButtonInput::<MouseButton>::default());
            world.insert_resource(ButtonInput::<KeyCode>::default());
            world.init_resource::<CurrentFrameInputs>();
            world.init_resource::<UiScale>();
            world.init_resource::<InputFocus>();
            world.init_resource::<InputFocusVisible>();
            // A clock's first update starts it: the next ones move it on
            let mut time = Time::<Real>::default();
            time.update_with_duration(std::time::Duration::ZERO);
            world.insert_resource(time);
            world.init_resource::<CellGestures>();
            world.insert_resource(UIMode::Opened);
            world.spawn((
                InventoryDialog,
                BackgroundColor(Color::NONE),
                BorderColor::all(Color::NONE),
            ));
            world.spawn((InventoryRoot, inventory_visibility));
            world.spawn(Hotbar { selected: 0 });
            world
                .spawn((FloatingStack, Node::default()))
                .with_child((SlotCount, Text::new("")));
            for &slot in slots {
                world
                    .spawn((
                        InventoryCell { id: slot },
                        Hovered(false),
                        Tooltip::default(),
                        SlotSelected::default(),
                    ))
                    .with_child((SlotCount, Text::new(""), TextColor(TEXT)));
            }
            Self { world }
        }

        fn spawn_result_cell(&mut self) -> Entity {
            self.world
                .spawn((
                    CraftResultCell,
                    Hovered(false),
                    Tooltip::default(),
                    SlotSelected::default(),
                ))
                .with_child((SlotCount, Text::new("")))
                .id()
        }

        fn spawn_grid_cell(&mut self, row: u32, col: u32) -> Entity {
            self.world
                .spawn((
                    InventoryCell {
                        id: grid_slot(row, col),
                    },
                    CraftingGridCell { row, col },
                    Hovered(false),
                    Tooltip::default(),
                    SlotSelected::default(),
                    Node::default(),
                ))
                .with_child((SlotCount, Text::new("")))
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

        /// The count shown by the cell of `slot`, and whether it is selected.
        fn shown(&mut self, slot: u32) -> (String, SlotSelected) {
            self.world
                .run_system_once(display_inventory_system)
                .unwrap();
            let cell = self.cell(slot);
            let count = self.world.get::<Children>(cell).unwrap()[0];
            (
                self.world.get::<Text>(count).unwrap().0.clone(),
                *self.world.get::<SlotSelected>(cell).unwrap(),
            )
        }

        fn click_with_shift(&mut self, slot: u32, button: MouseButton) {
            self.world
                .resource_mut::<ButtonInput<KeyCode>>()
                .press(KeyCode::ShiftLeft);
            self.click(slot, button);
            self.world
                .resource_mut::<ButtonInput<KeyCode>>()
                .release(KeyCode::ShiftLeft);
        }

        /// Presses `key` while the cell of `slot` is hovered, if any.
        fn press_key_over(&mut self, slot: Option<u32>, key: KeyCode) {
            let cell = slot.map(|slot| self.cell(slot));
            if let Some(cell) = cell {
                self.world.entity_mut(cell).insert(Hovered(true));
            }
            self.world.resource_mut::<ButtonInput<KeyCode>>().press(key);
            self.world
                .run_system_once(click_inventory_cells_system)
                .unwrap();
            let mut keys = self.world.resource_mut::<ButtonInput<KeyCode>>();
            keys.release(key);
            keys.clear();
            if let Some(cell) = cell {
                self.world.entity_mut(cell).insert(Hovered(false));
            }
        }

        /// The tooltip of `entity` once the cells are displayed.
        fn tooltip_of(&mut self, entity: Entity) -> Tooltip {
            self.world
                .run_system_once(display_inventory_system)
                .unwrap();
            self.show_crafting();
            self.world.get::<Tooltip>(entity).unwrap().clone()
        }

        /// Presses `button` over the cell of `slot`, then releases it there.
        fn click(&mut self, slot: u32, button: MouseButton) {
            self.press(slot, button);
            self.release(button);
            self.hover(None);
        }

        /// Moves the pointer onto the cell of `slot`, or off every cell, for a frame.
        fn hover(&mut self, slot: Option<u32>) {
            let mut cells = self.world.query::<(Entity, &InventoryCell)>();
            let cells: Vec<_> = cells
                .iter(&self.world)
                .map(|(entity, cell)| (entity, cell.id))
                .collect();
            for (entity, id) in cells {
                self.world
                    .entity_mut(entity)
                    .insert(Hovered(Some(id) == slot));
            }
        }

        /// Runs the systems of the clicks and of the drags for a frame.
        fn frame(&mut self) {
            self.world
                .run_system_once(click_inventory_cells_system)
                .unwrap();
            self.world
                .run_system_once(drag_inventory_cells_system)
                .unwrap();
            self.world
                .resource_mut::<ButtonInput<MouseButton>>()
                .clear();
            self.world.resource_mut::<ButtonInput<KeyCode>>().clear();
        }

        /// Moves the pointer onto the cell of `slot` and holds `button` down there.
        fn press(&mut self, slot: u32, button: MouseButton) {
            self.hover(Some(slot));
            self.world
                .resource_mut::<ButtonInput<MouseButton>>()
                .press(button);
            self.frame();
        }

        /// Moves the pointer onto the cell of `slot`, a button held or not.
        fn enter(&mut self, slot: u32) {
            self.hover(Some(slot));
            self.frame();
        }

        fn release(&mut self, button: MouseButton) {
            self.world
                .resource_mut::<ButtonInput<MouseButton>>()
                .release(button);
            self.frame();
        }

        /// Lets `ms` milliseconds go by.
        fn wait(&mut self, ms: u64) {
            self.world
                .resource_mut::<Time<Real>>()
                .update_with_duration(std::time::Duration::from_millis(ms));
        }

        fn hold(&mut self, stack: Option<ItemStack>) {
            self.world.resource_mut::<Inventory>().floating = stack;
        }

        /// The count shown by the cell of `slot` and its colour.
        fn shown_count(&mut self, slot: u32) -> (String, Color) {
            let count = self.shown(slot).0;
            let cell = self.cell(slot);
            let child = self.world.get::<Children>(cell).unwrap()[0];
            (count, self.world.get::<TextColor>(child).unwrap().0)
        }

        fn clear_sent(&mut self) {
            self.world
                .resource_mut::<CurrentFrameInputs>()
                .0
                .inventory_actions
                .clear();
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

        hud.world
            .resource_mut::<Inventory>()
            .inner
            .insert(9, dirt_stack(1));
        assert_eq!(hud.shown(9).0, "");

        hud.world.resource_mut::<Inventory>().inner.remove(&9);
        assert_eq!(hud.shown(9).0, "");
    }

    #[test]
    fn the_selected_hotbar_cell_stays_selected_when_the_inventory_opens() {
        for visibility in [Visibility::Hidden, Visibility::Visible] {
            let mut hud = Hud::new(visibility, &[0, 1]);
            assert_eq!(hud.shown(0).1, SlotSelected(true), "{visibility:?}");
            assert_eq!(hud.shown(1).1, SlotSelected(false), "{visibility:?}");
        }
    }

    #[test]
    fn the_panel_has_its_background_only_while_open() {
        let background = |visibility: Visibility| {
            let mut hud = Hud::new(visibility, &[]);
            hud.world.run_system_once(style_inventory_panel).unwrap();
            let mut panel = hud
                .world
                .query_filtered::<&BackgroundColor, With<InventoryDialog>>();
            *panel.single(&hud.world).unwrap()
        };
        assert_eq!(background(Visibility::Hidden), BackgroundColor(Color::NONE));
        assert_eq!(background(Visibility::Visible), BackgroundColor(PANEL));
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
    fn shift_click_sends_a_quick_move() {
        let mut hud = Hud::new(Visibility::Visible, &[12]);

        hud.click_with_shift(12, MouseButton::Left);
        assert_eq!(hud.sent_actions(), [InventoryAction::QuickMoveSlot(12)]);

        // Shift only changes the left button
        hud.world
            .resource_mut::<CurrentFrameInputs>()
            .0
            .inventory_actions
            .clear();
        hud.click_with_shift(12, MouseButton::Right);
        assert_eq!(hud.sent_actions(), [InventoryAction::RightClickSlot(12)]);
    }

    #[test]
    fn a_number_key_over_a_slot_sends_a_swap() {
        let mut hud = Hud::new(Visibility::Visible, &[12]);

        hud.press_key_over(Some(12), KeyCode::Digit3);

        assert_eq!(
            hud.sent_actions(),
            [InventoryAction::SwapWithHotbar {
                slot: 12,
                hotbar: 2
            }]
        );
    }

    #[test]
    fn a_number_key_over_nothing_sends_nothing() {
        // The open inventory always has a focus, hidden until a key moves it
        let mut hud = Hud::new(Visibility::Visible, &[12]);
        let cell = hud.cell(12);
        hud.world.insert_resource(InputFocus::from_entity(cell));
        hud.press_key_over(None, KeyCode::Digit3);
        assert!(hud.sent_actions().is_empty());

        let mut closed = Hud::new(Visibility::Hidden, &[12]);
        closed.press_key_over(Some(12), KeyCode::Digit3);
        assert!(closed.sent_actions().is_empty());
    }

    #[test]
    fn enter_space_and_shift_enter_act_on_the_focused_cell() {
        let mut hud = Hud::new(Visibility::Visible, &[12, 15]);
        let cell = hud.cell(12);
        hud.world.insert_resource(InputFocus::from_entity(cell));
        hud.world.insert_resource(InputFocusVisible(true));

        hud.press_key_over(None, KeyCode::Enter);
        hud.press_key_over(None, KeyCode::Space);
        hud.world
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::ShiftLeft);
        hud.press_key_over(None, KeyCode::Enter);
        hud.world
            .resource_mut::<ButtonInput<KeyCode>>()
            .release(KeyCode::ShiftLeft);
        hud.press_key_over(None, KeyCode::Digit2);
        // The ringed cell wins over the cursor
        hud.press_key_over(Some(15), KeyCode::Enter);
        hud.press_key_over(Some(15), KeyCode::Digit2);

        assert_eq!(
            hud.sent_actions(),
            [
                InventoryAction::LeftClickSlot(12),
                InventoryAction::RightClickSlot(12),
                InventoryAction::QuickMoveSlot(12),
                InventoryAction::SwapWithHotbar {
                    slot: 12,
                    hotbar: 1
                },
                InventoryAction::LeftClickSlot(12),
                InventoryAction::SwapWithHotbar {
                    slot: 12,
                    hotbar: 1
                },
            ]
        );
    }

    #[test]
    fn keys_leave_a_focus_without_its_ring_alone() {
        let mut hud = Hud::new(Visibility::Visible, &[12, 15]);
        let result = hud.spawn_result_cell();
        let cell = hud.cell(12);
        hud.world.insert_resource(InputFocus::from_entity(cell));

        hud.press_key_over(None, KeyCode::Enter);
        hud.press_key_over(None, KeyCode::Space);
        hud.world
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::ShiftLeft);
        hud.press_key_over(None, KeyCode::Enter);
        hud.world
            .resource_mut::<ButtonInput<KeyCode>>()
            .release(KeyCode::ShiftLeft);
        hud.world.insert_resource(InputFocus::from_entity(result));
        hud.press_key_over(None, KeyCode::Enter);
        assert!(hud.sent_actions().is_empty());

        // A number key goes to the hovered cell only
        hud.press_key_over(Some(15), KeyCode::Digit2);
        assert_eq!(
            hud.sent_actions(),
            [InventoryAction::SwapWithHotbar {
                slot: 15,
                hotbar: 1
            }]
        );
    }

    #[test]
    fn a_worn_tool_shows_its_durability_in_its_tooltip_and_bar() {
        let mut hud = Hud::new(Visibility::Visible, &[0, 1]);
        let bar = |hud: &mut Hud, slot: u32| {
            let cell = hud.cell(slot);
            hud.world.spawn((slot_durability(), ChildOf(cell))).id()
        };
        let (worn_bar, new_bar) = (bar(&mut hud, 0), bar(&mut hud, 1));
        let durability = ItemId::WoodenPickaxe.tool().unwrap().durability;
        let pickaxe = |wear| ItemStack {
            item_id: ItemId::WoodenPickaxe,
            item_type: ItemType::Tool { wear },
            nb: 1,
        };
        {
            let mut inventory = hud.world.resource_mut::<Inventory>();
            inventory.inner.insert(0, pickaxe(durability - 10));
            inventory.inner.insert(1, pickaxe(0));
        }

        let cell = hud.cell(0);
        assert_eq!(
            hud.tooltip_of(cell).detail,
            format!("Durability: 10 / {durability}")
        );
        let shown = |hud: &Hud, bar: Entity| *hud.world.get::<Visibility>(bar).unwrap();
        assert_eq!(shown(&hud, worn_bar), Visibility::Inherited);
        assert_eq!(
            hud.world.get::<Node>(worn_bar).unwrap().width,
            px(ICON_SIZE * 10.0 / durability as f32)
        );
        // An unworn tool shows no bar, like in Minecraft
        assert_eq!(shown(&hud, new_bar), Visibility::Hidden);
    }

    #[test]
    fn a_cell_s_tooltip_names_its_item_count_and_hint() {
        let mut hud = Hud::new(Visibility::Visible, &[0, 12]);
        let grid = hud.spawn_grid_cell(0, 0);
        let result = hud.spawn_result_cell();
        {
            let mut inventory = hud.world.resource_mut::<Inventory>();
            inventory.inner.insert(12, dirt_stack(5));
            inventory.inner.insert(0, dirt_stack(1));
            inventory.inner.insert(
                grid_slot(0, 0),
                ItemStack {
                    item_id: ItemId::OakLog,
                    item_type: ItemId::OakLog.get_default_type(),
                    nb: 1,
                },
            );
        }
        let (backpack, hotbar) = (hud.cell(12), hud.cell(0));

        assert_eq!(
            hud.tooltip_of(backpack),
            Tooltip {
                title: "Dirt".into(),
                detail: "5 items".into(),
                hint: HINT_BACKPACK.into(),
                ..default()
            }
        );
        assert_eq!(
            hud.tooltip_of(hotbar),
            Tooltip {
                title: "Dirt".into(),
                detail: "1 item".into(),
                hint: HINT_HOTBAR.into(),
                ..default()
            }
        );
        assert_eq!(
            hud.tooltip_of(grid),
            Tooltip {
                title: "Oak Log".into(),
                detail: "1 item".into(),
                hint: HINT_GRID.into(),
                ..default()
            }
        );
        assert_eq!(
            hud.tooltip_of(result),
            Tooltip {
                title: "Oak Planks".into(),
                detail: "4 items".into(),
                hint: HINT_RESULT.into(),
                ..default()
            }
        );
    }

    #[test]
    fn no_tooltip_on_an_empty_slot_while_holding_a_stack_or_while_closed() {
        for case in ["empty", "holding", "closed"] {
            let visibility = if case == "closed" {
                Visibility::Hidden
            } else {
                Visibility::Visible
            };
            let mut hud = Hud::new(visibility, &[0, 12]);
            let result = hud.spawn_result_cell();
            let mut inventory = hud.world.resource_mut::<Inventory>();
            if case != "empty" {
                inventory.inner.insert(12, dirt_stack(5));
                inventory.inner.insert(0, dirt_stack(5));
                inventory.inner.insert(
                    grid_slot(0, 0),
                    ItemStack {
                        item_id: ItemId::OakLog,
                        item_type: ItemId::OakLog.get_default_type(),
                        nb: 1,
                    },
                );
            }
            if case == "holding" {
                inventory.floating = Some(dirt_stack(2));
            }
            for cell in [hud.cell(0), hud.cell(12), result] {
                assert_eq!(hud.tooltip_of(cell), Tooltip::default(), "{case}");
            }
        }
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
        hud.world.entity_mut(result).insert(Hovered(true));
        hud.world
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);

        hud.world
            .run_system_once(click_inventory_cells_system)
            .unwrap();

        assert_eq!(hud.sent_actions(), [InventoryAction::TakeCraftResult]);
    }

    /// Opens a chest at (1, 2, 3) that holds `contents`, as the server sent it.
    fn open_chest(hud: &mut Hud, contents: shared::world::Container) {
        let chest = IVec3::new(1, 2, 3);
        let mut blocks = shared::world::ChunkMap::with_loaded_region(IVec3::ZERO, IVec3::splat(15));
        blocks.fill_box(chest, chest, shared::world::BlockId::Chest);
        blocks.insert_container(chest, contents);
        hud.world.resource_mut::<ClientWorldMap>().blocks = blocks;
        hud.world.resource_mut::<Inventory>().open_chest = Some(chest);
    }

    #[test]
    fn a_chest_cell_shows_what_the_open_chest_holds() {
        let slot = CHEST_SLOT_START + 4;
        let mut hud = Hud::new(Visibility::Visible, &[slot]);
        assert_eq!(hud.shown(slot).0, "");

        let mut contents = shared::world::Container::default();
        contents.slots[4] = Some(dirt_stack(7));
        open_chest(&mut hud, contents);

        assert_eq!(hud.shown(slot).0, "7");
    }

    #[test]
    fn the_chest_takes_the_place_of_the_crafting_row_and_names_the_dialog() {
        let mut hud = Hud::new(Visibility::Visible, &[]);
        let title = hud
            .world
            .spawn((InventoryTitle, Text::new("Inventory")))
            .id();
        let crafting = hud.world.spawn((CraftingRow, Node::default())).id();
        let chest = hud
            .world
            .spawn((
                ChestGrid,
                Node {
                    display: Display::None,
                    ..default()
                },
            ))
            .id();
        let display = |hud: &Hud, entity| hud.world.get::<Node>(entity).unwrap().display;

        open_chest(&mut hud, Default::default());
        hud.show_crafting();
        assert_eq!(hud.world.get::<Text>(title).unwrap().0, CHEST_TITLE);
        assert_eq!(display(&hud, crafting), Display::None);
        assert_eq!(display(&hud, chest), Display::Grid);

        hud.world.resource_mut::<Inventory>().open_chest = None;
        hud.show_crafting();
        assert_eq!(hud.world.get::<Text>(title).unwrap().0, INVENTORY_TITLE);
        assert_eq!(display(&hud, crafting), Display::Flex);
        assert_eq!(display(&hud, chest), Display::None);
    }

    #[test]
    fn with_a_chest_open_shift_click_moves_between_the_inventory_and_the_chest() {
        let chest_slot = CHEST_SLOT_START;
        let mut hud = Hud::new(Visibility::Visible, &[0, MAX_HOTBAR_SLOTS, chest_slot]);
        let mut contents = shared::world::Container::default();
        contents.slots[0] = Some(dirt_stack(2));
        open_chest(&mut hud, contents);
        hud.world
            .resource_mut::<Inventory>()
            .inner
            .extend([(0, dirt_stack(1)), (MAX_HOTBAR_SLOTS, dirt_stack(1))]);

        for (slot, hint) in [
            (0, HINT_TO_CHEST),
            (MAX_HOTBAR_SLOTS, HINT_TO_CHEST),
            (chest_slot, HINT_GRID),
        ] {
            let cell = hud.cell(slot);
            let tooltip = hud.tooltip_of(cell);
            assert_eq!(tooltip.hint, hint, "slot {slot}");
        }
    }

    /// Opens a furnace at (1, 2, 3) that holds `furnace`, as the server sent it.
    fn open_furnace(hud: &mut Hud, furnace: Furnace) {
        let pos = IVec3::new(1, 2, 3);
        let mut blocks = shared::world::ChunkMap::with_loaded_region(IVec3::ZERO, IVec3::splat(15));
        blocks.fill_box(pos, pos, shared::world::BlockId::Furnace);
        blocks.insert_furnace(pos, furnace);
        hud.world.resource_mut::<ClientWorldMap>().blocks = blocks;
        hud.world.resource_mut::<Inventory>().open_furnace = Some(pos);
    }

    fn stack(item_id: ItemId, nb: u32) -> ItemStack {
        ItemStack {
            item_id,
            item_type: item_id.get_default_type(),
            nb,
        }
    }

    #[test]
    fn the_furnace_cells_show_what_the_open_furnace_holds() {
        let slots = [FURNACE_INPUT_SLOT, FURNACE_FUEL_SLOT, FURNACE_OUTPUT_SLOT];
        let mut hud = Hud::new(Visibility::Visible, &slots);
        for slot in slots {
            assert_eq!(hud.shown(slot).0, "", "slot {slot}");
        }

        open_furnace(
            &mut hud,
            Furnace {
                input: Some(stack(ItemId::Sand, 5)),
                fuel: Some(stack(ItemId::Coal, 2)),
                output: Some(stack(ItemId::Glass, 3)),
                ..default()
            },
        );

        let shown = slots.map(|slot| hud.shown(slot).0);
        assert_eq!(shown, ["5", "2", "3"]);
    }

    #[test]
    fn the_furnace_takes_the_place_of_the_crafting_row_and_names_the_dialog() {
        let mut hud = Hud::new(Visibility::Visible, &[]);
        let title = hud
            .world
            .spawn((InventoryTitle, Text::new("Inventory")))
            .id();
        let crafting = hud.world.spawn((CraftingRow, Node::default())).id();
        let hidden = || Node {
            display: Display::None,
            ..default()
        };
        let chest = hud.world.spawn((ChestGrid, hidden())).id();
        let furnace = hud.world.spawn((FurnaceRow, hidden())).id();
        let display = |hud: &Hud, entity| hud.world.get::<Node>(entity).unwrap().display;

        open_furnace(&mut hud, Furnace::default());
        hud.show_crafting();
        assert_eq!(hud.world.get::<Text>(title).unwrap().0, FURNACE_TITLE);
        assert_eq!(display(&hud, crafting), Display::None);
        assert_eq!(display(&hud, chest), Display::None);
        assert_eq!(display(&hud, furnace), Display::Flex);

        hud.world.resource_mut::<Inventory>().open_furnace = None;
        hud.show_crafting();
        assert_eq!(hud.world.get::<Text>(title).unwrap().0, INVENTORY_TITLE);
        assert_eq!(display(&hud, crafting), Display::Flex);
        assert_eq!(display(&hud, furnace), Display::None);
    }

    #[test]
    fn with_a_furnace_open_a_hint_says_only_the_moves_a_shift_click_makes() {
        let slots = [
            0,
            1,
            MAX_HOTBAR_SLOTS,
            FURNACE_INPUT_SLOT,
            FURNACE_OUTPUT_SLOT,
        ];
        let mut hud = Hud::new(Visibility::Visible, &slots);
        open_furnace(
            &mut hud,
            Furnace {
                input: Some(stack(ItemId::Sand, 1)),
                output: Some(stack(ItemId::Glass, 1)),
                ..default()
            },
        );
        hud.world.resource_mut::<Inventory>().inner.extend([
            (0, stack(ItemId::Sand, 1)),
            (1, stack(ItemId::Dirt, 1)),
            (MAX_HOTBAR_SLOTS, stack(ItemId::Coal, 1)),
        ]);

        for (slot, hint) in [
            // It smelts, or it burns: the furnace takes it
            (0, HINT_TO_FURNACE),
            (MAX_HOTBAR_SLOTS, HINT_TO_FURNACE),
            // Neither: a shift-click leaves it
            (1, ""),
            (FURNACE_INPUT_SLOT, HINT_GRID),
            (FURNACE_OUTPUT_SLOT, HINT_GRID),
        ] {
            let cell = hud.cell(slot);
            let tooltip = hud.tooltip_of(cell);
            assert_eq!(tooltip.hint, hint, "slot {slot}");
        }
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
    fn a_hidden_cell_holding_items_is_shown() {
        let mut hud = Hud::new(Visibility::Visible, &[]);
        let corner = hud.spawn_grid_cell(2, 2);
        hud.world
            .resource_mut::<Inventory>()
            .inner
            .insert(grid_slot(2, 2), dirt_stack(1));

        hud.show_crafting();

        assert_ne!(
            hud.world.get::<Node>(corner).unwrap().display,
            Display::None
        );
    }

    #[test]
    fn the_result_cell_shows_nothing_while_items_lie_beyond_the_grid() {
        let mut hud = Hud::new(Visibility::Visible, &[]);
        let result = hud.spawn_result_cell();
        // A furnace left in the grid of a closed table: the inventory alone cannot make it
        let cobblestone = ItemStack {
            item_id: ItemId::Cobblestone,
            item_type: ItemId::Cobblestone.get_default_type(),
            nb: 1,
        };
        let mut inventory = hud.world.resource_mut::<Inventory>();
        for (row, col) in [
            (0, 0),
            (0, 1),
            (0, 2),
            (1, 0),
            (1, 2),
            (2, 0),
            (2, 1),
            (2, 2),
        ] {
            inventory.inner.insert(grid_slot(row, col), cobblestone);
        }

        hud.show_crafting();

        let count = hud.world.get::<Children>(result).unwrap()[0];
        assert_eq!(hud.world.get::<Text>(count).unwrap().0, "");
    }

    #[test]
    fn the_floating_stack_shows_the_stack_held_by_the_cursor() {
        let mut hud = Hud::new(Visibility::Visible, &[]);
        assert_eq!(hud.floating_shown(), "");

        hud.world.resource_mut::<Inventory>().floating = Some(dirt_stack(3));
        assert_eq!(hud.floating_shown(), "3");
    }

    #[test]
    fn the_open_inventory_takes_the_focus_and_gives_it_back_on_closing() {
        let mut hud = Hud::new(Visibility::Hidden, &[]);
        let mut dialog = hud.world.query_filtered::<Entity, With<InventoryDialog>>();
        let dialog = dialog.single(&hud.world).unwrap();
        let mut spawn = |id: u32, display: Display| {
            let node = Node {
                display,
                ..default()
            };
            let cell = (InventoryCell { id }, TabIndex(0), node, ChildOf(dialog));
            hud.world.spawn(cell).id()
        };
        let first = spawn(0, Display::Flex);
        let second = spawn(1, Display::Flex);
        let hidden = spawn(grid_slot(2, 2), Display::None);
        let indices = |hud: &Hud| {
            [first, second, hidden].map(|cell| hud.world.get::<TabIndex>(cell).unwrap().0)
        };
        let set_open = |hud: &mut Hud, visibility: Visibility| {
            let mut root = hud
                .world
                .query_filtered::<&mut Visibility, With<InventoryRoot>>();
            *root.single_mut(&mut hud.world).unwrap() = visibility;
        };
        let sync = hud.world.register_system(sync_inventory_focus);

        // Closed, Tab skips the hotbar
        hud.world.run_system(sync).unwrap();
        assert_eq!(indices(&hud), [-1, -1, -1]);

        // Open, the focus is on the selected slot, without its ring
        let mut hotbar = hud.world.query::<&mut Hotbar>();
        hotbar.single_mut(&mut hud.world).unwrap().selected = 1;
        hud.world.resource_mut::<InputFocusVisible>().0 = true;
        set_open(&mut hud, Visibility::Visible);
        hud.world.run_system(sync).unwrap();
        assert_eq!(hud.world.resource::<InputFocus>().get(), Some(second));
        assert!(!hud.world.resource::<InputFocusVisible>().0);
        // A hidden cell of the crafting grid takes no focus
        assert_eq!(indices(&hud), [0, 0, -1]);

        set_open(&mut hud, Visibility::Hidden);
        hud.world.run_system(sync).unwrap();
        assert_eq!(hud.world.resource::<InputFocus>().get(), None);
        assert_eq!(indices(&hud), [-1, -1, -1]);
    }

    #[test]
    fn the_open_inventory_takes_the_focus_back_after_a_click_on_empty_space() {
        let mut hud = Hud::new(Visibility::Visible, &[]);
        let mut dialog = hud.world.query_filtered::<Entity, With<InventoryDialog>>();
        let dialog = dialog.single(&hud.world).unwrap();
        let mut spawn = |id: u32| {
            let cell = (InventoryCell { id }, TabIndex(0), ChildOf(dialog));
            hud.world.spawn(cell).id()
        };
        let selected = spawn(0);
        let last = spawn(12);
        let sync = hud.world.register_system(sync_inventory_focus);
        hud.world.run_system(sync).unwrap();
        assert_eq!(hud.world.resource::<InputFocus>().get(), Some(selected));

        // The arrows moved the focus, then a click on empty space cleared it
        hud.world.insert_resource(InputFocus::from_entity(last));
        hud.world.run_system(sync).unwrap();
        hud.world.resource_mut::<InputFocus>().clear();
        hud.world.resource_mut::<InputFocusVisible>().0 = true;
        hud.world.run_system(sync).unwrap();
        assert_eq!(hud.world.resource::<InputFocus>().get(), Some(last));
        // The ring is left as it was: hidden after a click, shown after a key
        assert!(hud.world.resource::<InputFocusVisible>().0);

        // Without the last cell, the focus goes back to the selected slot
        hud.world.despawn(last);
        hud.world.resource_mut::<InputFocus>().clear();
        hud.world.run_system(sync).unwrap();
        assert_eq!(hud.world.resource::<InputFocus>().get(), Some(selected));
    }

    #[test]
    fn the_open_inventory_never_takes_the_focus_back_to_a_hidden_cell() {
        let mut hud = Hud::new(Visibility::Visible, &[]);
        let mut dialog = hud.world.query_filtered::<Entity, With<InventoryDialog>>();
        let dialog = dialog.single(&hud.world).unwrap();
        let selected = hud
            .world
            .spawn((InventoryCell { id: 0 }, TabIndex(0), ChildOf(dialog)))
            .id();
        let corner = hud
            .world
            .spawn((
                InventoryCell {
                    id: grid_slot(2, 2),
                },
                TabIndex(0),
                Node::default(),
                ChildOf(dialog),
            ))
            .id();
        let sync = hud.world.register_system(sync_inventory_focus);
        hud.world.run_system(sync).unwrap();

        // The focus went to the leftover corner of the grid, which got hidden once emptied
        hud.world.insert_resource(InputFocus::from_entity(corner));
        hud.world.run_system(sync).unwrap();
        hud.world.get_mut::<Node>(corner).unwrap().display = Display::None;
        hud.world.resource_mut::<InputFocus>().clear();
        hud.world.run_system(sync).unwrap();

        assert_eq!(hud.world.resource::<InputFocus>().get(), Some(selected));
    }

    #[test]
    fn the_cells_of_a_hidden_row_leave_the_navigation() {
        let mut hud = Hud::new(Visibility::Visible, &[]);
        let mut dialog = hud.world.query_filtered::<Entity, With<InventoryDialog>>();
        let dialog = dialog.single(&hud.world).unwrap();
        let row = hud
            .world
            .spawn((
                ChestGrid,
                Node {
                    display: Display::None,
                    ..default()
                },
                ChildOf(dialog),
            ))
            .id();
        let cell = hud
            .world
            .spawn((
                InventoryCell {
                    id: CHEST_SLOT_START,
                },
                TabIndex(0),
                Node::default(),
                ChildOf(row),
            ))
            .id();
        let sync = hud.world.register_system(sync_inventory_focus);

        hud.world.run_system(sync).unwrap();
        assert_eq!(*hud.world.get::<TabIndex>(cell).unwrap(), TabIndex(-1));

        hud.world.get_mut::<Node>(row).unwrap().display = Display::Grid;
        hud.world.run_system(sync).unwrap();
        assert_eq!(*hud.world.get::<TabIndex>(cell).unwrap(), TabIndex(0));
    }

    #[test]
    fn the_focus_leaves_a_crafting_cell_that_disappears() {
        let mut hud = Hud::new(Visibility::Visible, &[]);
        let mut dialog = hud.world.query_filtered::<Entity, With<InventoryDialog>>();
        let dialog = dialog.single(&hud.world).unwrap();
        let selected = hud
            .world
            .spawn((InventoryCell { id: 0 }, TabIndex(0), ChildOf(dialog)))
            .id();
        let corner = hud
            .world
            .spawn((
                InventoryCell {
                    id: grid_slot(2, 2),
                },
                TabIndex(0),
                Node::default(),
                ChildOf(dialog),
            ))
            .id();
        let sync = hud.world.register_system(sync_inventory_focus);
        hud.world.run_system(sync).unwrap();

        // The arrows ringed the leftover corner of the grid, and Enter emptied it
        hud.world.insert_resource(InputFocus::from_entity(corner));
        hud.world.resource_mut::<InputFocusVisible>().0 = true;
        hud.world.run_system(sync).unwrap();
        hud.world.get_mut::<Node>(corner).unwrap().display = Display::None;
        hud.world.run_system(sync).unwrap();

        assert_eq!(hud.world.resource::<InputFocus>().get(), Some(selected));
        // The ring stays as it was
        assert!(hud.world.resource::<InputFocusVisible>().0);
    }

    #[test]
    fn enter_on_the_focused_result_cell_crafts() {
        let mut hud = Hud::new(Visibility::Visible, &[]);
        let result = hud.spawn_result_cell();
        hud.world.insert_resource(InputFocus::from_entity(result));
        hud.world.insert_resource(InputFocusVisible(true));

        hud.press_key_over(None, KeyCode::Enter);

        assert_eq!(hud.sent_actions(), [InventoryAction::TakeCraftResult]);
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

    /// While the player types in the item browser's search, the keys are text, not game keys.
    #[test]
    fn game_keys_wait_while_the_search_has_the_focus() {
        let mut hud = Hud::new(Visibility::Visible, &[12]);
        hud.world.insert_resource(UIMode::Typing);
        hud.world
            .insert_resource(crate::input::keyboard::default_bindings());

        hud.world
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyE);
        hud.world.run_system_once(toggle_inventory_system).unwrap();
        hud.world.resource_mut::<ButtonInput<KeyCode>>().clear();
        let mut root = hud
            .world
            .query_filtered::<&Visibility, With<InventoryRoot>>();
        assert_eq!(
            root.single(&hud.world).unwrap(),
            Visibility::Visible,
            "E closed the inventory"
        );

        for key in [KeyCode::Digit3, KeyCode::Enter, KeyCode::Space] {
            hud.press_key_over(Some(12), key);
        }
        assert!(hud.sent_actions().is_empty(), "{:?}", hud.sent_actions());

        // The mouse still works
        hud.click(12, MouseButton::Left);
        assert_eq!(hud.sent_actions(), [InventoryAction::LeftClickSlot(12)]);
    }

    /// The hotbar and the inventory as the game builds them, with an atlas of two items.
    #[test]
    fn the_cells_built_by_the_setup_systems_show_the_count_and_the_icon_of_their_slot() {
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
            tiles: Default::default(),
            tile_rects: Default::default(),
        };

        // The setup loads the font of the slots and the sprites of the furnace
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<Font>()
            .init_asset::<Image>();
        let world = app.world_mut();
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
        world.init_resource::<ClientWorldMap>();
        world.init_resource::<UiScale>();
        world.init_resource::<InputFocus>();
        world.init_resource::<InputFocusVisible>();
        world.init_resource::<CellGestures>();
        world.run_system_once(setup_inventory).unwrap();

        let mut cells = world.query::<&InventoryCell>();
        // The slots of the inventory, then the 3x3 crafting grid, the slots of a chest and those
        // of a furnace
        assert_eq!(
            cells.iter(world).count() as u32,
            MAX_INVENTORY_SLOTS + 9 + shared::world::CHEST_SLOTS + 3
        );

        // What the cell of `slot` shows: its count, and the atlas index of its visible icon
        let shown = |world: &mut World, slot: u32| -> (String, Option<usize>) {
            world.run_system_once(display_inventory_system).unwrap();
            let mut cells = world.query::<(&InventoryCell, &Children)>();
            let (_, children) = cells.iter(world).find(|(cell, _)| cell.id == slot).unwrap();
            let mut count = String::new();
            let mut icon = None;
            for child in children.iter() {
                if world.get::<SlotCount>(child).is_some() {
                    count = world.get::<Text>(child).unwrap().0.clone();
                }
                if let Some(image) = world.get::<ImageNode>(child) {
                    if *world.get::<Visibility>(child).unwrap() != Visibility::Hidden {
                        icon = image.texture_atlas.as_ref().map(|atlas| atlas.index);
                    }
                }
            }
            (count, icon)
        };

        assert_eq!(shown(world, 0), ("5".to_string(), Some(0)));
        assert_eq!(shown(world, 1), (String::new(), None));
        // The cells of the closed inventory are not updated
        assert_eq!(shown(world, MAX_HOTBAR_SLOTS), (String::new(), None));

        let mut root = world.query_filtered::<&mut Visibility, With<InventoryRoot>>();
        *root.single_mut(world).unwrap() = Visibility::Visible;
        assert_eq!(shown(world, MAX_HOTBAR_SLOTS), ("2".to_string(), Some(1)));
    }

    fn bits(slots: &[u32]) -> u128 {
        slots.iter().fold(0, |bits, slot| bits | 1 << slot)
    }

    #[test]
    fn a_press_with_a_held_stack_sends_its_click_on_release() {
        for (button, click) in [
            (MouseButton::Left, InventoryAction::LeftClickSlot(9)),
            (MouseButton::Right, InventoryAction::RightClickSlot(9)),
        ] {
            let mut hud = Hud::new(Visibility::Visible, &[9, 10]);
            hud.hold(Some(dirt_stack(5)));

            hud.press(9, button);
            assert!(hud.sent_actions().is_empty(), "{button:?}");
            hud.release(button);
            assert_eq!(hud.sent_actions(), [click], "{button:?}");
            assert_eq!(hud.world.resource::<CellGestures>().drag, None);
        }
    }

    #[test]
    fn a_drag_over_slots_sends_one_drag() {
        for (button, one_each) in [(MouseButton::Left, false), (MouseButton::Right, true)] {
            let mut hud = Hud::new(Visibility::Visible, &[9, 10, 11]);
            hud.hold(Some(dirt_stack(10)));

            hud.press(9, button);
            hud.enter(10);
            hud.enter(11);
            hud.enter(10);
            assert!(hud.sent_actions().is_empty(), "{button:?}");
            hud.release(button);

            assert_eq!(
                hud.sent_actions(),
                [InventoryAction::Drag {
                    slots: bits(&[9, 10, 11]),
                    one_each
                }],
                "{button:?}"
            );
        }
    }

    #[test]
    fn a_slot_the_drag_cannot_fill_does_not_join_it() {
        let mut hud = Hud::new(Visibility::Visible, &[9, 10, 11]);
        hud.world
            .resource_mut::<Inventory>()
            .inner
            .insert(10, stack(ItemId::Sand, 1));
        hud.hold(Some(dirt_stack(4)));

        hud.press(9, MouseButton::Left);
        hud.enter(10);
        hud.enter(11);
        hud.release(MouseButton::Left);

        assert_eq!(
            hud.sent_actions(),
            [InventoryAction::Drag {
                slots: bits(&[9, 11]),
                one_each: false
            }]
        );
    }

    #[test]
    fn a_drag_takes_no_more_slots_than_the_cursor_holds_items() {
        for button in [MouseButton::Left, MouseButton::Right] {
            let mut hud = Hud::new(Visibility::Visible, &[9, 10, 11, 12]);
            hud.hold(Some(dirt_stack(2)));

            hud.press(12, button);
            for slot in [11, 10, 9] {
                hud.enter(slot);
            }
            hud.release(button);

            assert_eq!(
                hud.sent_actions(),
                [InventoryAction::Drag {
                    slots: bits(&[11, 12]),
                    one_each: button == MouseButton::Right
                }],
                "{button:?}"
            );
        }
    }

    #[test]
    fn the_other_button_closing_or_escape_cancels_a_drag() {
        for case in ["other button", "closed", "escape"] {
            let mut hud = Hud::new(Visibility::Visible, &[9, 10]);
            hud.hold(Some(dirt_stack(4)));
            hud.press(9, MouseButton::Left);
            hud.enter(10);

            match case {
                "other button" => hud.press(10, MouseButton::Right),
                "closed" => {
                    let mut root = hud
                        .world
                        .query_filtered::<&mut Visibility, With<InventoryRoot>>();
                    *root.single_mut(&mut hud.world).unwrap() = Visibility::Hidden;
                    hud.frame();
                }
                _ => {
                    hud.world
                        .resource_mut::<ButtonInput<KeyCode>>()
                        .press(KeyCode::Escape);
                    hud.frame();
                }
            }
            hud.release(MouseButton::Left);
            hud.release(MouseButton::Right);

            assert!(hud.sent_actions().is_empty(), "{case}");
            assert_eq!(hud.world.resource::<CellGestures>().drag, None, "{case}");
        }
    }

    #[test]
    fn a_press_without_a_held_stack_acts_at_once() {
        let mut hud = Hud::new(Visibility::Visible, &[9]);
        hud.world
            .resource_mut::<Inventory>()
            .inner
            .insert(9, dirt_stack(5));

        hud.press(9, MouseButton::Left);
        assert_eq!(hud.sent_actions(), [InventoryAction::LeftClickSlot(9)]);
        hud.release(MouseButton::Left);
        assert_eq!(hud.sent_actions(), [InventoryAction::LeftClickSlot(9)]);
        assert_eq!(hud.world.resource::<CellGestures>().drag, None);
    }

    /// A click on the cell of `slot`, holding 5 dirt, that picks them up, as the prediction
    /// would.
    fn pick_up(hud: &mut Hud, slot: u32) {
        hud.world
            .resource_mut::<Inventory>()
            .inner
            .insert(slot, dirt_stack(5));
        hud.click(slot, MouseButton::Left);
        hud.world.resource_mut::<Inventory>().inner.remove(&slot);
        hud.hold(Some(dirt_stack(5)));
    }

    #[test]
    fn a_double_click_on_a_stack_collects_its_item() {
        let mut hud = Hud::new(Visibility::Visible, &[9, 10]);
        pick_up(&mut hud, 9);
        hud.wait(100);
        hud.click(9, MouseButton::Left);
        assert_eq!(
            hud.sent_actions(),
            [
                InventoryAction::LeftClickSlot(9),
                InventoryAction::CollectAll
            ]
        );

        // A third press is a press
        hud.clear_sent();
        hud.wait(50);
        hud.click(9, MouseButton::Left);
        assert_eq!(hud.sent_actions(), [InventoryAction::LeftClickSlot(9)]);
    }

    #[test]
    fn a_second_press_too_late_or_on_another_cell_is_no_double_click() {
        for (wait, second) in [(DOUBLE_CLICK_WINDOW.as_millis() as u64 + 10, 9), (100, 10)] {
            let mut hud = Hud::new(Visibility::Visible, &[9, 10]);
            pick_up(&mut hud, 9);
            hud.clear_sent();
            hud.wait(wait);
            hud.click(second, MouseButton::Left);
            assert_eq!(
                hud.sent_actions(),
                [InventoryAction::LeftClickSlot(second)],
                "{wait} ms, then {second}"
            );
        }
    }

    #[test]
    fn a_shift_double_click_moves_every_stack_of_the_item_it_moved() {
        let mut hud = Hud::new(Visibility::Visible, &[9]);
        hud.world
            .resource_mut::<Inventory>()
            .inner
            .insert(9, dirt_stack(5));

        hud.click_with_shift(9, MouseButton::Left);
        // The prediction moved the dirt away
        hud.world.resource_mut::<Inventory>().inner.remove(&9);
        hud.wait(100);
        hud.click_with_shift(9, MouseButton::Left);

        assert_eq!(
            hud.sent_actions(),
            [
                InventoryAction::QuickMoveSlot(9),
                InventoryAction::MoveAllOfType {
                    slot: 9,
                    item: ItemId::Dirt
                }
            ]
        );
    }

    #[test]
    fn shift_on_the_result_crafts_all_and_a_number_key_crafts_to_the_hotbar() {
        let mut hud = Hud::new(Visibility::Visible, &[]);
        let result = hud.spawn_result_cell();
        let shift = |hud: &mut Hud, down: bool| {
            let mut keys = hud.world.resource_mut::<ButtonInput<KeyCode>>();
            if down {
                keys.press(KeyCode::ShiftLeft);
            } else {
                keys.release(KeyCode::ShiftLeft);
            }
        };
        let press_on_result = |hud: &mut Hud, button: MouseButton| {
            hud.world.entity_mut(result).insert(Hovered(true));
            hud.world
                .resource_mut::<ButtonInput<MouseButton>>()
                .press(button);
            hud.frame();
            hud.release(button);
            hud.world.entity_mut(result).insert(Hovered(false));
        };

        shift(&mut hud, true);
        press_on_result(&mut hud, MouseButton::Left);
        // Shift only changes the left button
        press_on_result(&mut hud, MouseButton::Right);
        shift(&mut hud, false);
        // Over the result, a number key
        hud.world.entity_mut(result).insert(Hovered(true));
        hud.press_key_over(None, KeyCode::Digit3);
        hud.world.entity_mut(result).insert(Hovered(false));
        // On the ringed result, Shift+Enter, Space and a number key
        hud.world.insert_resource(InputFocus::from_entity(result));
        hud.world.insert_resource(InputFocusVisible(true));
        shift(&mut hud, true);
        hud.press_key_over(None, KeyCode::Enter);
        hud.press_key_over(None, KeyCode::Space);
        shift(&mut hud, false);
        hud.press_key_over(None, KeyCode::Digit1);

        assert_eq!(
            hud.sent_actions(),
            [
                InventoryAction::CraftAll,
                InventoryAction::TakeCraftResult,
                InventoryAction::CraftToHotbar(2),
                InventoryAction::CraftAll,
                InventoryAction::TakeCraftResult,
                InventoryAction::CraftToHotbar(0),
            ]
        );
    }

    #[test]
    fn a_drag_shows_its_planned_counts_in_the_accent_colour() {
        let mut hud = Hud::new(Visibility::Visible, &[9, 10, 11]);
        hud.world
            .resource_mut::<Inventory>()
            .inner
            .insert(10, dirt_stack(1));
        hud.hold(Some(dirt_stack(7)));

        // A single slot is a click: no preview
        hud.press(9, MouseButton::Left);
        assert_eq!(hud.shown_count(9), (String::new(), TEXT));
        assert_eq!(hud.floating_shown(), "7");

        // 7 over two slots: 3 each, 1 stays held
        hud.enter(10);
        assert_eq!(hud.shown_count(9), ("3".to_string(), ACCENT));
        assert_eq!(hud.shown_count(10), ("4".to_string(), ACCENT));
        assert_eq!(hud.shown_count(11), (String::new(), TEXT));
        assert_eq!(hud.floating_shown(), "");

        // Released, the cells show what the inventory holds
        hud.release(MouseButton::Left);
        assert_eq!(hud.shown_count(9), (String::new(), TEXT));
        assert_eq!(hud.shown_count(10), (String::new(), TEXT));
        assert_eq!(hud.floating_shown(), "7");
    }
}
