use bevy::{
    input::{
        keyboard::{Key, KeyboardInput},
        mouse::MouseScrollUnit,
        ButtonState,
    },
    input_focus::{
        tab_navigation::TabIndex, FocusCause, FocusGained, FocusedInput, InputFocus,
        InputFocusVisible,
    },
    picking::hover::Hovered,
    prelude::*,
    text::EditableText,
    ui::Checked,
    ui_widgets::{Activate, ScrollIntoView},
};

use shared::players::{game_mode::GameMode, Inventory, Player};
use shared::world::ItemId;

use super::catalog::{count_label, no_match_hint, no_match_message, shown_in};
use super::layout::{browser_layout, BrowserLayout};
use super::recipes::{craftable_now, CRAFTABLE_NOTE};
use super::{
    CatalogGrid, CatalogSlot, CategoryChip, CraftableChip, CraftableDot, CraftableNow,
    DrawerHandle, ItemBrowser, ItemBrowserState, ItemCount, ItemSearchField, ModeFooter, NoMatch,
    NoMatchHint, NoMatchMessage,
};
use crate::player::CurrentPlayerMarker;
use crate::ui::hud::{HudBottom, InventoryDialog, InventoryRoot, UIMode};
use crate::ui::theme::{
    ACCENT, ACCENT_HI, ACCENT_LO, BEVEL_HI, BEVEL_LO, MUTED, OK, PANEL, RAISE, RAISE_HOVER,
    SPACE_L, TEXT,
};
use crate::ui::widgets::slot::{SlotIcon, SLOT_SIZE};
use crate::ui::widgets::text_field::{set_text, Cancel, Submit};
use crate::ui::widgets::tooltip::Tooltip;

/// How far a notch of the wheel moves the row of chips sideways, in logical px.
const CHIPS_WHEEL_STEP: f32 = SLOT_SIZE;

/// Keeps the search of the state up to date with its field.
pub fn read_search(
    fields: Query<&EditableText, (With<ItemSearchField>, Changed<EditableText>)>,
    mut state: ResMut<ItemBrowserState>,
) {
    for text in &fields {
        let query = text.value().to_string();
        if state.query != query {
            state.query = query;
        }
    }
}

/// A chip activated chooses its category.
pub fn choose_category(
    activate: On<Activate>,
    chips: Query<&CategoryChip>,
    mut state: ResMut<ItemBrowserState>,
) {
    if let Ok(chip) = chips.get(activate.entity) {
        if state.category != chip.0 {
            state.category = chip.0;
        }
    }
}

/// The Craftable chip activated turns its filter on or off.
pub fn toggle_craftable(_: On<Activate>, mut state: ResMut<ItemBrowserState>) {
    state.craftable = !state.craftable;
}

/// Follows the game mode of the local player, Survival without one as a new player is, and in
/// Survival the items it can craft now. Only when the inventory or the mode changes.
pub fn update_craftable_now(
    inventory: Res<Inventory>,
    player: Query<&Player, With<CurrentPlayerMarker>>,
    mut now: ResMut<CraftableNow>,
) {
    let survival = player
        .single()
        .map_or(true, |player| player.game_mode == GameMode::Survival);
    if !inventory.is_changed() && survival == now.survival {
        return;
    }
    let items = if survival {
        ItemId::ALL
            .into_iter()
            .filter(|&item| craftable_now(&inventory, item))
            .collect()
    } else {
        Vec::new()
    };
    now.set_if_neq(CraftableNow { survival, items });
}

/// The wheel moves the row of chips sideways: Bevy's `ScrollArea` only moves a row with the
/// sideways scroll of a trackpad.
pub fn wheel_sideways(
    scroll: On<Pointer<Scroll>>,
    mut rows: Query<(&mut ScrollPosition, &ComputedNode)>,
) {
    let Ok((mut position, row)) = rows.get_mut(scroll.entity) else {
        return;
    };
    let step = match scroll.unit {
        MouseScrollUnit::Line => CHIPS_WHEEL_STEP,
        MouseScrollUnit::Pixel => 1.0,
    };
    let end = (row.content_size().x - row.size().x) * row.inverse_scale_factor();
    position.x = (position.x - scroll.y * step).clamp(0.0, end.max(0.0));
}

/// Lays the browser out for the window, which `HudBottom` covers, and the inventory panel.
pub fn layout_item_browser(
    bottom: Single<&ComputedNode, With<HudBottom>>,
    dialog: Single<&ComputedNode, With<InventoryDialog>>,
    mut layout: ResMut<BrowserLayout>,
) {
    let window = bottom.size() * bottom.inverse_scale_factor();
    let inventory_width = dialog.size().x * dialog.inverse_scale_factor();
    // Not laid out yet
    if window.x <= 0.0 || inventory_width <= 0.0 {
        return;
    }
    layout.set_if_neq(browser_layout(window, inventory_width));
}

/// Shows the icon of each new slot of the catalog: its item never changes.
pub fn show_catalog_icons(
    slots: Query<&Children, Added<CatalogSlot>>,
    mut icons: Query<&mut Visibility, With<SlotIcon>>,
) {
    for children in &slots {
        for child in children.iter() {
            if let Ok(mut visibility) = icons.get_mut(child) {
                visibility.set_if_neq(Visibility::Inherited);
            }
        }
    }
}

/// Shows the slots of the items the search and the category keep, and in Survival the Craftable
/// chip, how many they are, and what the catalog says when there is none.
#[allow(clippy::type_complexity)]
pub fn filter_catalog(
    state: Res<ItemBrowserState>,
    craftable: Res<CraftableNow>,
    mut slots: Query<(&CatalogSlot, &mut Node)>,
    mut no_match: Query<&mut Node, (With<NoMatch>, Without<CatalogSlot>)>,
    mut count: Query<&mut Text, With<ItemCount>>,
    mut message: Query<&mut Text, (With<NoMatchMessage>, Without<ItemCount>)>,
    mut hint: Query<
        &mut Text,
        (
            With<NoMatchHint>,
            Without<ItemCount>,
            Without<NoMatchMessage>,
        ),
    >,
) {
    let only_craftable = state.craftable && craftable.survival;
    let (mut shown, mut total) = (0, 0);
    for (slot, mut node) in &mut slots {
        total += 1;
        let kept = shown_in(&state.query, state.category, slot.0)
            && (!only_craftable || craftable.items.contains(&slot.0));
        let display = if kept {
            shown += 1;
            Display::Flex
        } else {
            Display::None
        };
        if node.display != display {
            node.display = display;
        }
    }
    for mut text in &mut count {
        text.0 = count_label(shown, total);
    }
    for mut node in &mut no_match {
        node.display = if shown == 0 {
            Display::Flex
        } else {
            Display::None
        };
    }
    for mut text in &mut message {
        text.0 = no_match_message(&state.query, state.category, only_craftable);
    }
    for mut text in &mut hint {
        text.0 = no_match_hint(&state.query, only_craftable).to_string();
    }
}

/// Shows the footer of the game mode.
pub fn show_mode_footer(
    craftable: Res<CraftableNow>,
    mut footers: Query<(&ModeFooter, &mut Node)>,
) {
    for (footer, mut node) in &mut footers {
        let display = if footer.0 == craftable.survival {
            Display::Flex
        } else {
            Display::None
        };
        if node.display != display {
            node.display = display;
        }
    }
}

/// Marks the items craftable now: the dot of their catalog slot, and the note of the tooltip of
/// their slots in the browser.
pub fn mark_craftable_items(
    craftable: Res<CraftableNow>,
    mut slots: Query<(&CatalogSlot, &mut Tooltip, &Children)>,
    mut dots: Query<&mut Node, With<CraftableDot>>,
) {
    for (slot, mut tooltip, children) in &mut slots {
        let now = craftable.items.contains(&slot.0);
        let note = if now { CRAFTABLE_NOTE } else { "" };
        if tooltip.note != note {
            tooltip.note = note.to_string();
        }
        for child in children.iter() {
            if let Ok(mut dot) = dots.get_mut(child) {
                let display = if now { Display::Flex } else { Display::None };
                if dot.display != display {
                    dot.display = display;
                }
            }
        }
    }
}

/// The focusable parts of the browser, and whether each is a slot of the catalog.
type BrowserStops<'w, 's> = Query<
    'w,
    's,
    (&'static mut TabIndex, &'static Node),
    (
        Or<(
            With<ItemSearchField>,
            With<CategoryChip>,
            With<CraftableChip>,
            With<CatalogSlot>,
        )>,
        Without<ItemBrowser>,
        Without<InventoryDialog>,
        Without<CatalogGrid>,
        Without<DrawerHandle>,
    ),
>;

/// Shows the browser while the inventory is open, docked or as an open drawer, with the columns
/// of its layout, and slides the open inventory panel to the left of the docked browser. The
/// handle of the drawer shows while the drawer is closed. Only what shows takes Tab: Bevy's
/// navigation does not look at the visibility.
#[allow(clippy::type_complexity)]
pub fn show_item_browser(
    layout: Res<BrowserLayout>,
    state: Res<ItemBrowserState>,
    inventory_root: Query<&Visibility, (With<InventoryRoot>, Without<ItemBrowser>)>,
    bottom: Single<&ComputedNode, With<HudBottom>>,
    mut dialog: Single<(&mut Node, &ComputedNode), With<InventoryDialog>>,
    mut panel: Single<(&mut Node, &mut Visibility), (With<ItemBrowser>, Without<InventoryDialog>)>,
    mut handle: Single<
        (&mut Visibility, &mut TabIndex),
        (
            With<DrawerHandle>,
            Without<ItemBrowser>,
            Without<InventoryRoot>,
        ),
    >,
    mut grid: Single<
        &mut Node,
        (
            With<CatalogGrid>,
            Without<ItemBrowser>,
            Without<InventoryDialog>,
        ),
    >,
    mut stops: BrowserStops,
) {
    let open = matches!(inventory_root.single(), Ok(&Visibility::Visible));
    let shown = open && (!layout.compact || state.drawer_open);
    let handle_shown = open && layout.compact && !state.drawer_open;
    let (handle_visibility, handle_index) = &mut *handle;
    handle_visibility.set_if_neq(if handle_shown {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    });
    handle_index.set_if_neq(TabIndex(if handle_shown { 0 } else { -1 }));

    let (node, visibility) = &mut *panel;
    visibility.set_if_neq(if shown {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    });
    let (width, right) = (
        px(layout.width),
        px(if layout.compact { 0.0 } else { SPACE_L }),
    );
    if node.width != width || node.right != right {
        node.width = width;
        node.right = right;
    }
    let columns = RepeatedGridTrack::px(layout.columns.max(1) as u16, SLOT_SIZE);
    if grid.grid_template_columns != columns {
        grid.grid_template_columns = columns;
    }

    // `Val::Px` is in logical px: the computed sizes are divided by the scale
    let (dialog_node, dialog_size) = &mut *dialog;
    let left = if open && !layout.compact {
        let window = bottom.size().x * bottom.inverse_scale_factor();
        let inventory_width = dialog_size.size().x * dialog_size.inverse_scale_factor();
        px(layout.inventory_left - (window - inventory_width) / 2.0)
    } else {
        Val::Auto
    };
    if dialog_node.left != left {
        dialog_node.left = left;
    }

    for (mut index, node) in &mut stops {
        let stop = shown && node.display != Display::None;
        index.set_if_neq(TabIndex(if stop { 0 } else { -1 }));
    }
}

/// Whether Escape closes the drawer rather than the inventory: while it is open over the open
/// inventory. Never in an app without the browser, as the pause menu's tests are.
pub fn drawer_takes_escape(
    state: Option<Res<ItemBrowserState>>,
    layout: Option<Res<BrowserLayout>>,
    inventory_root: Query<&Visibility, With<InventoryRoot>>,
) -> bool {
    let open = matches!(inventory_root.single(), Ok(&Visibility::Visible));
    let (Some(state), Some(layout)) = (state, layout) else {
        return false;
    };
    open && layout.compact && state.drawer_open
}

/// The handle activated opens the drawer.
pub fn open_drawer(_: On<Activate>, mut state: ResMut<ItemBrowserState>) {
    state.drawer_open = true;
}

/// Opens and closes the drawer: B toggles it, Escape closes it, and so does a press of the mouse
/// on neither the drawer nor its handle. The keys wait while the player types. The inventory
/// closing closes it too: it opens closed. A focus inside the drawer it closes is let go of, for
/// the inventory to take it back.
#[allow(clippy::too_many_arguments)]
pub fn drawer_keys(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    ui_mode: Res<UIMode>,
    layout: Res<BrowserLayout>,
    mut state: ResMut<ItemBrowserState>,
    inventory_root: Query<&Visibility, With<InventoryRoot>>,
    panel: Single<(Entity, &Hovered), With<ItemBrowser>>,
    handle: Single<&Hovered, With<DrawerHandle>>,
    parents: Query<&ChildOf>,
    mut focus: ResMut<InputFocus>,
) {
    let open = matches!(inventory_root.single(), Ok(&Visibility::Visible));
    if !open || !layout.compact {
        if state.drawer_open && !open {
            state.drawer_open = false;
        }
        return;
    }
    let typing = *ui_mode == UIMode::Typing;
    let (panel, over_panel) = *panel;
    let toggle = !typing && keys.just_pressed(KeyCode::KeyB);
    let escape = !typing && keys.just_pressed(KeyCode::Escape);
    let outside = mouse.any_just_pressed([MouseButton::Left, MouseButton::Right])
        && !over_panel.get()
        && !handle.get();
    let drawer_open = if toggle {
        !state.drawer_open
    } else {
        state.drawer_open && !escape && !outside
    };
    if drawer_open == state.drawer_open {
        return;
    }
    state.drawer_open = drawer_open;
    let inside = focus
        .get()
        .is_some_and(|focused| parents.iter_ancestors(focused).any(|e| e == panel));
    if !drawer_open && inside {
        focus.clear();
    }
}

/// Focuses the search, with its ring, on Ctrl+F, or on the character `/` typed while the
/// inventory is open and the player is not typing already, and opens the drawer it is in. On the
/// release of `/`, as the chat does, so that neither the key nor its repeat is typed in the field.
#[allow(clippy::too_many_arguments)]
pub fn focus_search(
    mut key_events: MessageReader<KeyboardInput>,
    keys: Res<ButtonInput<KeyCode>>,
    ui_mode: Res<UIMode>,
    layout: Res<BrowserLayout>,
    mut state: ResMut<ItemBrowserState>,
    inventory_root: Query<&Visibility, With<InventoryRoot>>,
    field: Single<Entity, With<ItemSearchField>>,
    mut armed: Local<Option<KeyCode>>,
    mut focus: ResMut<InputFocus>,
    mut focus_visible: ResMut<InputFocusVisible>,
) {
    // read on every frame, so that a slash typed in a field never focuses the search later
    let ready =
        *ui_mode == UIMode::Opened && matches!(inventory_root.single(), Ok(&Visibility::Visible));
    if !ready {
        *armed = None;
    }
    let mut slash = false;
    for key in key_events.read() {
        match key.state {
            ButtonState::Pressed
                if ready && !key.repeat && key.logical_key == Key::Character("/".into()) =>
            {
                *armed = Some(key.key_code);
            }
            ButtonState::Released if *armed == Some(key.key_code) => {
                *armed = None;
                slash = true;
            }
            _ => {}
        }
    }
    let ctrl = keys.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight]);
    let ctrl_f = ready && ctrl && keys.just_pressed(KeyCode::KeyF);
    if !slash && !ctrl_f {
        return;
    }
    if layout.compact && !state.drawer_open {
        state.drawer_open = true;
    }
    focus.set(*field, FocusCause::Navigated);
    focus_visible.0 = true;
}

/// Puts the focus, with its ring, on the first item the catalog shows, if any.
pub fn focus_first_result(
    grid: Single<&Children, With<CatalogGrid>>,
    slots: Query<&Node, With<CatalogSlot>>,
    mut focus: ResMut<InputFocus>,
    mut focus_visible: ResMut<InputFocusVisible>,
) {
    let first = grid.iter().find(|&slot| {
        slots
            .get(slot)
            .is_ok_and(|node| node.display != Display::None)
    });
    if let Some(first) = first {
        focus.set(first, FocusCause::Navigated);
        focus_visible.0 = true;
    }
}

/// Enter in the search goes to its first result.
pub fn submit_search(_: On<Submit>, mut commands: Commands) {
    commands.run_system_cached(focus_first_result);
}

/// Down in the search goes to its first result.
pub fn search_arrow_down(input: On<FocusedInput<KeyboardInput>>, mut commands: Commands) {
    let key = &input.input;
    if key.key_code == KeyCode::ArrowDown && key.state == ButtonState::Pressed {
        commands.run_system_cached(focus_first_result);
    }
}

/// Escape in the search clears it, or leaves it once it is empty: the inventory takes the focus
/// back.
pub fn clear_or_leave_search(
    cancel: On<Cancel>,
    mut texts: Query<&mut EditableText>,
    mut focus: ResMut<InputFocus>,
) {
    let Ok(mut text) = texts.get_mut(cancel.entity) else {
        return;
    };
    if text.value() == "" {
        focus.clear();
    } else {
        set_text(&mut text, "");
    }
}

/// The chosen chips take the accent, the Craftable chip green, with dark text; the others are
/// raised, lighter under the cursor. The Craftable chip shows in Survival only.
#[allow(clippy::type_complexity)]
pub fn style_chips(
    state: Res<ItemBrowserState>,
    craftable: Res<CraftableNow>,
    mut chips: Query<
        (
            Entity,
            Option<&CategoryChip>,
            Has<CraftableChip>,
            &Hovered,
            Has<Checked>,
            &mut BackgroundColor,
            &mut BorderColor,
            &mut Node,
            &Children,
        ),
        Or<(With<CategoryChip>, With<CraftableChip>)>,
    >,
    mut texts: Query<&mut TextColor>,
    mut commands: Commands,
) {
    for (
        entity,
        category,
        is_craftable,
        hovered,
        checked,
        mut background,
        mut border,
        mut node,
        children,
    ) in &mut chips
    {
        let chosen = match category {
            Some(chip) => chip.0 == state.category,
            None => state.craftable,
        };
        if is_craftable {
            let display = if craftable.survival {
                Display::Flex
            } else {
                Display::None
            };
            if node.display != display {
                node.display = display;
            }
        }
        let (fill, edge, text) = chip_look(chosen, is_craftable, hovered.get());
        background.set_if_neq(BackgroundColor(fill));
        border.set_if_neq(edge);
        for child in children.iter() {
            if let Ok(mut color) = texts.get_mut(child) {
                color.set_if_neq(TextColor(text));
            }
        }
        // Read by screen readers as the toggled state of the button
        if chosen && !checked {
            commands.entity(entity).insert(Checked);
        } else if !chosen && checked {
            commands.entity(entity).remove::<Checked>();
        }
    }
}

/// The light and dark edges of the bevel of the green chip, as the mockup.
const OK_HI: Color = Color::srgb_u8(0xAE, 0xE0, 0x9C);
const OK_LO: Color = Color::srgb_u8(0x3F, 0x73, 0x30);

/// The fill, the bevel and the text colour of a chip: chosen, in the accent or green, else
/// raised, lighter under the cursor.
fn chip_look(chosen: bool, green: bool, hovered: bool) -> (Color, BorderColor, Color) {
    let bevel = |light: Color, dark: Color| BorderColor {
        top: light,
        left: light,
        bottom: dark,
        right: dark,
    };
    match (chosen, green) {
        (true, false) => (ACCENT, bevel(ACCENT_HI, ACCENT_LO), PANEL),
        (true, true) => (OK, bevel(OK_HI, OK_LO), PANEL),
        (false, _) => (
            if hovered { RAISE_HOVER } else { RAISE },
            bevel(BEVEL_HI, BEVEL_LO),
            if hovered { TEXT } else { MUTED },
        ),
    }
}

/// Brings a chip or a slot of the catalog into view when it takes the focus, from the arrows or
/// Tab.
pub fn scroll_focused_into_view(
    gained: On<FocusGained>,
    targets: Query<(), Or<(With<CatalogSlot>, With<CategoryChip>)>>,
    mut commands: Commands,
) {
    // the event bubbles up from the focused entity: handle it once, on its first step
    let entity = gained.original_event_target();
    if gained.entity == entity && targets.contains(entity) {
        commands.trigger(ScrollIntoView { entity });
    }
}

#[cfg(test)]
mod tests {
    use bevy::ecs::system::RunSystemOnce;
    use shared::world::{ItemCategory, ItemId};

    use super::*;
    use crate::player::CurrentPlayerMarker;
    use crate::ui::hud::item_browser::catalog::catalog;
    use crate::ui::widgets::tooltip::Tooltip;
    use shared::players::{game_mode::GameMode, Inventory, Player};
    use shared::world::ItemStack;

    /// A world holding the parts of the browser that its systems read, without textures.
    fn browser(inventory: Visibility) -> World {
        let mut world = World::new();
        world.init_resource::<ItemBrowserState>();
        world.init_resource::<CraftableNow>();
        world.init_resource::<Inventory>();
        world.insert_resource(browser_layout(Vec2::new(1280.0, 720.0), 502.0));
        world.spawn((InventoryRoot, inventory));
        let window = ComputedNode {
            size: Vec2::new(1280.0, 720.0),
            inverse_scale_factor: 1.0,
            ..default()
        };
        world.spawn((HudBottom, window));
        let dialog = ComputedNode {
            size: Vec2::new(502.0, 400.0),
            inverse_scale_factor: 1.0,
            ..default()
        };
        world.spawn((InventoryDialog, Node::default(), dialog));
        world.spawn((ItemBrowser, Node::default(), Visibility::Hidden));
        world.spawn((DrawerHandle, Visibility::Hidden, TabIndex(-1)));
        world.spawn((CatalogGrid, Node::default()));
        world.spawn((ItemSearchField, Node::default(), TabIndex(0)));
        world.spawn((CategoryChip(None), Node::default(), TabIndex(0)));
        for item in catalog() {
            world.spawn((
                CatalogSlot(item),
                Node::default(),
                TabIndex(0),
                Tooltip::default(),
                children![(CraftableDot, Node::default())],
            ));
        }
        world.spawn((ItemCount, Text::default()));
        world.spawn((NoMatch, Node::default()));
        world.spawn((NoMatchMessage, Text::default()));
        world
    }

    fn set_search(world: &mut World, query: &str, category: Option<ItemCategory>) {
        let mut state = world.resource_mut::<ItemBrowserState>();
        state.query = query.to_string();
        state.category = category;
        world.run_system_once(filter_catalog).unwrap();
    }

    fn shown_slots(world: &mut World) -> Vec<ItemId> {
        let mut slots = world.query::<(&CatalogSlot, &Node)>();
        slots
            .iter(world)
            .filter(|(_, node)| node.display != Display::None)
            .map(|(slot, _)| slot.0)
            .collect()
    }

    fn single<C: Component + Clone, M: Component>(world: &mut World) -> C {
        let mut query = world.query_filtered::<&C, With<M>>();
        query.single(world).unwrap().clone()
    }

    #[test]
    fn the_search_and_the_category_filter_the_slots_and_the_count() {
        let mut world = browser(Visibility::Visible);
        set_search(&mut world, "oak", None);
        assert_eq!(
            shown_slots(&mut world),
            [ItemId::OakLog, ItemId::OakPlanks, ItemId::OakLeaves]
        );
        assert_eq!(single::<Text, ItemCount>(&mut world).0, "3 / 33");
        assert_eq!(single::<Node, NoMatch>(&mut world).display, Display::None);

        set_search(&mut world, "oak", Some(ItemCategory::Wood));
        assert_eq!(shown_slots(&mut world).len(), 2);
        assert_eq!(single::<Text, ItemCount>(&mut world).0, "2 / 33");

        set_search(&mut world, "zzz", Some(ItemCategory::Wood));
        assert!(shown_slots(&mut world).is_empty());
        assert_eq!(single::<Node, NoMatch>(&mut world).display, Display::Flex);
        assert_eq!(
            single::<Text, NoMatchMessage>(&mut world).0,
            "No item matches \"zzz\" in Wood."
        );
    }

    /// The tab indices of the field, the chip and the slots, in that order.
    fn tab_indices(world: &mut World) -> Vec<i32> {
        let mut stops = world.query_filtered::<&TabIndex, Or<(
            With<ItemSearchField>,
            With<CategoryChip>,
            With<CatalogSlot>,
        )>>();
        stops.iter(world).map(|index| index.0).collect()
    }

    #[test]
    fn the_browser_shows_only_while_the_inventory_is_open() {
        let mut world = browser(Visibility::Hidden);
        world.run_system_once(filter_catalog).unwrap();
        world.run_system_once(show_item_browser).unwrap();
        assert_eq!(
            single::<Visibility, ItemBrowser>(&mut world),
            Visibility::Hidden
        );
        assert!(tab_indices(&mut world).iter().all(|&index| index == -1));
        assert_eq!(single::<Node, InventoryDialog>(&mut world).left, Val::Auto);

        let mut root = world.query_filtered::<&mut Visibility, With<InventoryRoot>>();
        *root.single_mut(&mut world).unwrap() = Visibility::Visible;
        set_search(&mut world, "oak", None);
        world.run_system_once(show_item_browser).unwrap();
        assert_eq!(
            single::<Visibility, ItemBrowser>(&mut world),
            Visibility::Inherited
        );
        let indices = tab_indices(&mut world);
        assert_eq!(indices.iter().filter(|&&index| index == 0).count(), 2 + 3);
        // Centred, the panel would be at 389: it slides to 276
        assert_eq!(
            single::<Node, InventoryDialog>(&mut world).left,
            px(276.0 - 389.0)
        );
        assert_eq!(single::<Node, ItemBrowser>(&mut world).width, px(470.0));
    }

    /// The world holds `nb` of `item` in the backpack, and the systems that follow it ran.
    fn give(world: &mut World, item_id: ItemId, nb: u32) {
        let stack = ItemStack {
            item_id,
            item_type: item_id.get_default_type(),
            nb,
        };
        world.resource_mut::<Inventory>().inner.insert(9, stack);
        world.run_system_once(update_craftable_now).unwrap();
        world.run_system_once(filter_catalog).unwrap();
        world.run_system_once(mark_craftable_items).unwrap();
    }

    /// Puts the local player in `mode`, and runs the systems that follow it.
    fn set_mode(world: &mut World, mode: GameMode) {
        let mut player = Player::new(1, "tester".into(), Vec3::ZERO, Transform::default());
        player.game_mode = mode;
        let mut players = world.query_filtered::<Entity, With<CurrentPlayerMarker>>();
        let existing: Vec<Entity> = players.iter(world).collect();
        for entity in existing {
            world.despawn(entity);
        }
        world.spawn((player, CurrentPlayerMarker {}));
        world.run_system_once(update_craftable_now).unwrap();
        world.run_system_once(filter_catalog).unwrap();
        world.run_system_once(mark_craftable_items).unwrap();
    }

    #[test]
    fn the_craftable_chip_keeps_what_can_be_crafted_now() {
        let mut world = browser(Visibility::Visible);
        give(&mut world, ItemId::OakLog, 1);
        assert_eq!(
            *world.resource::<CraftableNow>(),
            CraftableNow {
                survival: true,
                items: vec![ItemId::OakPlanks]
            }
        );
        world.resource_mut::<ItemBrowserState>().craftable = true;
        world.run_system_once(filter_catalog).unwrap();
        assert_eq!(shown_slots(&mut world), [ItemId::OakPlanks]);
        assert_eq!(single::<Text, ItemCount>(&mut world).0, "1 / 33");

        // Nothing craftable: the empty state says so
        give(&mut world, ItemId::Dirt, 1);
        assert!(shown_slots(&mut world).is_empty());
        assert_eq!(
            single::<Text, NoMatchMessage>(&mut world).0,
            "No craftable item."
        );

        // Creative has no such filter
        set_mode(&mut world, GameMode::Creative);
        assert_eq!(shown_slots(&mut world).len(), 33);
        assert!(!world.resource::<CraftableNow>().survival);
    }

    /// Whether the dot of the slot of `item` shows, and the note of its tooltip.
    fn dot_and_note(world: &mut World, item: ItemId) -> (bool, String) {
        let mut slots = world.query::<(&CatalogSlot, &Children, &Tooltip)>();
        let (_, children, tooltip) = slots.iter(world).find(|(slot, ..)| slot.0 == item).unwrap();
        let dot = world.get::<Node>(children[0]).unwrap().display != Display::None;
        (dot, tooltip.note.clone())
    }

    #[test]
    fn a_craftable_item_shows_a_dot_and_says_so_in_its_tooltip() {
        let mut world = browser(Visibility::Visible);
        give(&mut world, ItemId::OakLog, 1);
        assert_eq!(
            dot_and_note(&mut world, ItemId::OakPlanks),
            (true, "You have the ingredients".to_string())
        );
        assert_eq!(
            dot_and_note(&mut world, ItemId::Stick),
            (false, String::new())
        );

        set_mode(&mut world, GameMode::Creative);
        assert_eq!(
            dot_and_note(&mut world, ItemId::OakPlanks),
            (false, String::new())
        );
    }

    #[test]
    fn escape_belongs_to_the_open_drawer() {
        let takes_escape = |world: &mut World| world.run_system_once(drawer_takes_escape).unwrap();
        // Docked: Escape closes the inventory
        let mut world = browser(Visibility::Visible);
        world.resource_mut::<ItemBrowserState>().drawer_open = true;
        assert!(!takes_escape(&mut world));

        world.insert_resource(browser_layout(Vec2::new(640.0, 480.0), 502.0));
        assert!(takes_escape(&mut world));
        world.resource_mut::<ItemBrowserState>().drawer_open = false;
        assert!(!takes_escape(&mut world));

        let mut world = browser(Visibility::Hidden);
        world.insert_resource(browser_layout(Vec2::new(640.0, 480.0), 502.0));
        world.resource_mut::<ItemBrowserState>().drawer_open = true;
        assert!(!takes_escape(&mut world));
    }
}
