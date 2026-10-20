//! The recipe view: the recipes or the uses of an item, as cards, in the panel of the browser
//! instead of the catalog, and the history the back button and Escape walk back.

use accesskit::Role;
use bevy::{
    a11y::AccessibilityNode,
    input_focus::{tab_navigation::TabIndex, FocusCause, InputFocus, InputFocusVisible},
    picking::hover::Hovered,
    prelude::*,
    ui::InteractionDisabled,
    ui_widgets::{Activate, Button as UiButton},
};
use shared::players::{inventory::craft_result, Inventory};
use shared::world::{recipes, recipes_for, uses_of, ItemId, RecipeView};

use super::catalog::category_detail;
use super::layout::BrowserLayout;
use super::recipes::{
    back_label, empty_view_message, recipe_problem, station_label, tab_label, view_title,
    ITEM_KEYS_HINT,
};
use super::{
    BackButton, BrowserTitle, CatalogPart, CatalogSlot, CraftableNow, FillGridButton,
    ItemBrowserState, ItemCount, ItemView, MissingTint, RecipeCard, RecipeCards, RecipeNote,
    RecipePanel, RecipeSlot, RecipeStop, RecipeTab, RecipeTabChip, ViewedItem, ViewedItemCategory,
    ViewedItemName,
};
use crate::ui::hud::inventory::{grid_in_use, stack_in};
use crate::ui::hud::{item_name, CraftResultCell, InventoryCell, InventoryRoot, UIMode};
use crate::ui::theme::{
    BEVEL_HI, BEVEL_LO, BEVEL_WIDTH, DANGER, DISABLED_ALPHA, FONT_M, FONT_S, MUTED, RAISE, SLOT,
    SPACE_S, SPACE_XS, TEXT,
};
use crate::ui::widgets::button::set_label;
use crate::ui::widgets::focus::FocusRing;
use crate::ui::widgets::hints::key_cap;
use crate::ui::widgets::list::{spawn_scroll_list, ScrollListParts, WheelStep, RING_ROOM};
use crate::ui::widgets::slot::{slot, ICON_SIZE, SLOT_SIZE};
use crate::ui::widgets::tooltip::Tooltip;
use crate::world::{AtlasWrapper, ClientWorldMap, MaterialResource};

/// The side of a cell of the grid of a card.
pub const CARD_CELL: f32 = 40.0;
/// The gap between the cells of the grid of a card.
const CARD_CELL_GAP: f32 = 3.0;
/// The background of a card, a little lighter than the slots.
const CARD: Color = Color::srgb_u8(0x17, 0x1A, 0x20);
/// The dashes around an empty cell of a card.
const DASH: Color = Color::srgb_u8(0x26, 0x2B, 0x34);
/// The length of a dash, and how far a dash starts from the previous one.
const DASH_LENGTH: f32 = 4.0;
const DASH_PERIOD: f32 = 9.0;
/// The tint over an ingredient the player lacks.
const MISSING_TINT: Color = Color::srgba(0.89, 0.35, 0.25, 0.32);
/// The label of the button filling the crafting grid.
pub const FILL_GRID: &str = "+ Fill grid";

/// A text in the game font.
fn label(text: &str, font: &Handle<Font>, size: f32, color: Color) -> impl Bundle {
    (
        Text::new(text),
        TextFont {
            font: font.clone().into(),
            font_size: px(size).into(),
            ..default()
        },
        TextColor(color),
        Pickable::IGNORE,
    )
}

/// The icon of `item`, on its sprite of the items atlas.
fn icon(atlas: &AtlasWrapper, item: ItemId) -> ImageNode {
    ImageNode::from_atlas_image(
        atlas.texture.clone(),
        atlas.sprite(&format!("{item:?}")).unwrap_or_default(),
    )
}

/// A small raised button of the view, labelled `text` in `color`.
fn small_button(text: &str, color: Color, font: &Handle<Font>) -> impl Bundle {
    (
        // the widget before the label: its `Role::Button` must win over the label's node
        UiButton,
        AccessibleLabel::new(text),
        TabIndex(-1),
        FocusRing,
        Hovered::default(),
        Node {
            flex_shrink: 0.0,
            padding: UiRect::axes(px(SPACE_S), px(2)),
            border: UiRect::all(px(BEVEL_WIDTH)),
            ..default()
        },
        BackgroundColor(RAISE),
        BorderColor {
            top: BEVEL_HI,
            left: BEVEL_HI,
            bottom: BEVEL_LO,
            right: BEVEL_LO,
        },
        children![label(text, font, FONT_S, color)],
    )
}

/// Builds the view, hidden, in `panel`: the back button and its Esc cap, the item, the two tabs,
/// and the scrolling column of cards.
pub fn spawn_recipe_panel(
    commands: &mut Commands,
    panel: Entity,
    atlas: &AtlasWrapper,
    font: &Handle<Font>,
    glyph_font: &Handle<Font>,
) {
    let view = commands
        .spawn((
            RecipePanel,
            Node {
                display: Display::None,
                flex_direction: FlexDirection::Column,
                flex_grow: 1.0,
                min_height: px(0),
                row_gap: px(SPACE_S),
                ..default()
            },
            ChildOf(panel),
        ))
        .id();

    commands.spawn((
        Node {
            align_items: AlignItems::Center,
            column_gap: px(SPACE_S),
            // the room of the focus ring of the back button
            padding: UiRect::all(px(RING_ROOM)),
            margin: UiRect::all(px(-RING_ROOM)),
            ..default()
        },
        ChildOf(view),
        children![
            (BackButton, RecipeStop, small_button("<- Items", TEXT, font)),
            key_cap("Esc", font.clone(), glyph_font.clone()),
        ],
    ));
    commands
        .entity(view)
        .with_child((
            Node {
                align_items: AlignItems::Center,
                column_gap: px(SPACE_S),
                ..default()
            },
            Pickable::IGNORE,
            children![
                (
                    ViewedItem,
                    icon(atlas, ItemId::Dirt),
                    Node {
                        width: px(ICON_SIZE),
                        height: px(ICON_SIZE),
                        flex_shrink: 0.0,
                        ..default()
                    },
                    Pickable::IGNORE,
                ),
                (
                    Node {
                        flex_direction: FlexDirection::Column,
                        ..default()
                    },
                    Pickable::IGNORE,
                    children![
                        (ViewedItemName, label("", font, FONT_M, TEXT)),
                        (ViewedItemCategory, label("", font, FONT_S, MUTED)),
                    ],
                ),
            ],
        ))
        .with_child((
            Node {
                column_gap: px(SPACE_XS),
                padding: UiRect::all(px(RING_ROOM)),
                margin: UiRect::all(px(-RING_ROOM)),
                ..default()
            },
            children![
                (
                    RecipeTabChip(RecipeTab::Recipes),
                    RecipeStop,
                    small_button("Recipes", MUTED, font),
                ),
                (
                    RecipeTabChip(RecipeTab::Uses),
                    RecipeStop,
                    small_button("Uses", MUTED, font)
                ),
            ],
        ));

    let ScrollListParts { frame, well, list } =
        spawn_scroll_list(commands, WheelStep::Px(SLOT_SIZE + SPACE_XS));
    commands.entity(frame).insert(ChildOf(view));
    // the cards sit on the panel: no well around them
    commands
        .entity(well)
        .insert((BackgroundColor(Color::NONE), BorderColor::all(Color::NONE)));
    commands
        .entity(well)
        .entry::<Node>()
        .and_modify(|mut node| node.border = UiRect::ZERO);
    commands.spawn((
        RecipeCards(None),
        Node {
            flex_direction: FlexDirection::Column,
            row_gap: px(SPACE_S),
            padding: UiRect::vertical(px(RING_ROOM)),
            ..default()
        },
        ChildOf(list),
    ));
}

/// The dashes around an empty cell of a card, as the mockup draws it: five on each side.
fn dashes() -> impl Iterator<Item = (Node, BackgroundColor, Pickable)> {
    (0..5).flat_map(|index| {
        let along = px(index as f32 * DASH_PERIOD);
        let dash = |node: Node| (node, BackgroundColor(DASH), Pickable::IGNORE);
        let (long, thin) = (px(DASH_LENGTH), px(BEVEL_WIDTH));
        [
            dash(Node {
                position_type: PositionType::Absolute,
                left: along,
                top: px(0),
                width: long,
                height: thin,
                ..default()
            }),
            dash(Node {
                position_type: PositionType::Absolute,
                left: along,
                bottom: px(0),
                width: long,
                height: thin,
                ..default()
            }),
            dash(Node {
                position_type: PositionType::Absolute,
                top: along,
                left: px(0),
                width: thin,
                height: long,
                ..default()
            }),
            dash(Node {
                position_type: PositionType::Absolute,
                top: along,
                right: px(0),
                width: thin,
                height: long,
                ..default()
            }),
        ]
    })
}

/// A slot of a card showing `item`, `count` of it for the output; an ingredient of `recipe` has
/// the tint shown when the player lacks it.
fn spawn_card_slot(
    commands: &mut Commands,
    parent: Entity,
    item: ItemId,
    recipe: Option<u16>,
    count: u32,
    size: f32,
    atlas: &AtlasWrapper,
    font: &Handle<Font>,
) {
    let name = item_name(item);
    let mut cell = commands.spawn((
        RecipeSlot { item, count },
        RecipeStop,
        Tooltip {
            title: name.clone(),
            detail: category_detail(item),
            note: String::new(),
            hint: ITEM_KEYS_HINT.to_string(),
        },
        slot(&name, None, icon(atlas, item), font.clone()),
        ChildOf(parent),
    ));
    cell.entry::<Node>().and_modify(move |mut node| {
        node.width = px(size);
        node.height = px(size);
    });
    if let Some(recipe) = recipe {
        // after the icon: drawn over it
        cell.with_child((
            MissingTint { recipe, item },
            Node {
                display: Display::None,
                position_type: PositionType::Absolute,
                left: px(0),
                right: px(0),
                top: px(0),
                bottom: px(0),
                ..default()
            },
            BackgroundColor(MISSING_TINT),
            Pickable::IGNORE,
        ));
    }
}

/// A card of `recipe` at the end of `cards`: its 3 × 3 grid, the arrow and its output, then the
/// station or why it cannot be made now, and the button that will fill the grid.
fn spawn_card(
    commands: &mut Commands,
    cards: Entity,
    recipe: &RecipeView,
    atlas: &AtlasWrapper,
    font: &Handle<Font>,
) {
    let output = recipe.output.item_id;
    let card = commands
        .spawn((
            RecipeCard,
            // the role before the label: the label's default node must not win
            AccessibilityNode(accesskit::Node::new(Role::Group)),
            AccessibleLabel::new(format!("Recipe of {}", item_name(output))),
            Node {
                flex_direction: FlexDirection::Column,
                row_gap: px(SPACE_S),
                padding: UiRect::axes(px(SPACE_XS), px(SPACE_S)),
                border: UiRect::all(px(BEVEL_WIDTH)),
                ..default()
            },
            BackgroundColor(CARD),
            BorderColor {
                top: BEVEL_LO,
                left: BEVEL_LO,
                bottom: BEVEL_HI,
                right: BEVEL_HI,
            },
            ChildOf(cards),
        ))
        .id();
    let body = commands
        .spawn((
            Node {
                align_items: AlignItems::Center,
                column_gap: px(SPACE_XS),
                ..default()
            },
            ChildOf(card),
        ))
        .id();
    let grid = commands
        .spawn((
            Node {
                display: Display::Grid,
                flex_shrink: 0.0,
                grid_template_columns: RepeatedGridTrack::px(3, CARD_CELL),
                row_gap: px(CARD_CELL_GAP),
                column_gap: px(CARD_CELL_GAP),
                ..default()
            },
            ChildOf(body),
        ))
        .id();
    for row in 0..3 {
        for col in 0..3 {
            let item = (row < recipe.height && col < recipe.width)
                .then(|| recipe.cells[row * recipe.width + col])
                .flatten();
            match item {
                Some(item) => spawn_card_slot(
                    commands,
                    grid,
                    item,
                    Some(recipe.index),
                    1,
                    CARD_CELL,
                    atlas,
                    font,
                ),
                None => {
                    commands.spawn((
                        Node {
                            width: px(CARD_CELL),
                            height: px(CARD_CELL),
                            ..default()
                        },
                        BackgroundColor(SLOT.with_alpha(0.5)),
                        Pickable::IGNORE,
                        ChildOf(grid),
                        Children::spawn(SpawnIter(dashes())),
                    ));
                }
            }
        }
    }
    commands.spawn((
        label("->", font, FONT_M, MUTED),
        TextLayout {
            linebreak: LineBreak::NoWrap,
            ..default()
        },
        Node {
            flex_shrink: 0.0,
            ..default()
        },
        ChildOf(body),
    ));
    spawn_card_slot(
        commands,
        body,
        output,
        None,
        recipe.output.nb,
        SLOT_SIZE,
        atlas,
        font,
    );

    commands.spawn((
        Node {
            flex_wrap: FlexWrap::Wrap,
            justify_content: JustifyContent::SpaceBetween,
            align_items: AlignItems::Center,
            column_gap: px(SPACE_S),
            row_gap: px(SPACE_XS),
            ..default()
        },
        ChildOf(card),
        children![
            (
                RecipeNote(recipe.index),
                label(station_label(recipe), font, FONT_S, MUTED),
            ),
            (
                FillGridButton,
                // in this lot, the server cannot fill the grid yet: muted, out of Tab
                InteractionDisabled,
                small_button(FILL_GRID, MUTED.with_alpha(DISABLED_ALPHA * 2.0), font),
            ),
        ],
    ));
}

/// The recipes `view` shows.
fn recipes_of(view: ItemView) -> Vec<RecipeView> {
    match view.tab {
        RecipeTab::Recipes => recipes_for(view.item).collect(),
        RecipeTab::Uses => uses_of(view.item).collect(),
    }
}

/// The parts of the panel that `show_view` writes.
#[derive(bevy::ecs::system::SystemParam)]
pub struct ViewParts<'w, 's> {
    catalog_parts: Query<'w, 's, &'static mut Node, (With<CatalogPart>, Without<RecipePanel>)>,
    catalog_roots: Query<'w, 's, Entity, With<CatalogPart>>,
    panel: Single<'w, 's, (Entity, &'static mut Node), (With<RecipePanel>, Without<CatalogPart>)>,
    texts: Query<
        'w,
        's,
        (&'static mut Text, Has<BrowserTitle>, Has<ViewedItemName>),
        Or<(
            With<BrowserTitle>,
            With<ViewedItemName>,
            With<ViewedItemCategory>,
        )>,
    >,
    count: Query<'w, 's, &'static mut Visibility, With<ItemCount>>,
    icon: Query<'w, 's, &'static mut ImageNode, With<ViewedItem>>,
    back: Single<'w, 's, Entity, With<BackButton>>,
    tabs: Query<'w, 's, (Entity, &'static RecipeTabChip)>,
    cards: Single<'w, 's, (Entity, &'static mut RecipeCards)>,
    catalog_slots: Query<
        'w,
        's,
        (Entity, &'static CatalogSlot, &'static Node),
        (Without<CatalogPart>, Without<RecipePanel>),
    >,
    parents: Query<'w, 's, &'static ChildOf>,
}

/// Shows the open view instead of the catalog, or the catalog: the title, the back button, the
/// item, its tabs and, when the view changed, its cards. A focus left in a part that goes away
/// moves: to the back button in a view (from the cards or from the catalog, its search
/// included), to the item's slot in the catalog.
pub fn show_view(
    state: Res<ItemBrowserState>,
    materials: Res<MaterialResource>,
    asset_server: Res<AssetServer>,
    mut parts: ViewParts,
    mut focus: ResMut<InputFocus>,
    mut commands: Commands,
) {
    let Some(atlas) = materials.items.as_ref() else {
        return;
    };
    let font = crate::ui::assets::load_font(&asset_server);
    let view = state.view;
    let shown = if view.is_some() {
        Display::None
    } else {
        Display::Flex
    };
    for mut node in &mut parts.catalog_parts {
        if node.display != shown {
            node.display = shown;
        }
    }
    let (panel, panel_node) = &mut *parts.panel;
    let panel = *panel;
    let view_display = if view.is_some() {
        Display::Flex
    } else {
        Display::None
    };
    if panel_node.display != view_display {
        panel_node.display = view_display;
    }
    for mut visibility in &mut parts.count {
        visibility.set_if_neq(if view.is_some() {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        });
    }
    for (mut text, is_title, is_name) in &mut parts.texts {
        let content = match (is_title, is_name, view) {
            (true, _, None) => super::ITEM_BROWSER_TITLE.to_string(),
            (true, _, Some(view)) => view_title(view.tab).to_string(),
            (false, true, Some(view)) => item_name(view.item),
            (false, false, Some(view)) => category_detail(view.item),
            (false, _, None) => continue,
        };
        if text.0 != content {
            text.0 = content;
        }
    }

    let focused = focus.get();
    let inside = |ancestor: Entity| {
        focused.is_some_and(|focused| {
            focused == ancestor || parts.parents.iter_ancestors(focused).any(|e| e == ancestor)
        })
    };
    let (cards, mut built) = parts.cards.into_inner();
    let Some(view) = view else {
        // Back to the catalog: the focus goes back to the item it was on
        if inside(panel) {
            let slot = built.0.and_then(|last| {
                parts.catalog_slots.iter().find_map(|(entity, slot, node)| {
                    (slot.0 == last.item && node.display != Display::None).then_some(entity)
                })
            });
            match slot {
                Some(slot) => focus.set(slot, FocusCause::Navigated),
                None => focus.clear(),
            }
        }
        if built.0.is_some() {
            built.0 = None;
            commands.entity(cards).despawn_related::<Children>();
        }
        return;
    };

    commands
        .entity(*parts.back)
        .queue(set_label(&back_label(&state.history)));
    for (chip, tab) in &parts.tabs {
        let count = match tab.0 {
            RecipeTab::Recipes => recipes_for(view.item).count(),
            RecipeTab::Uses => uses_of(view.item).count(),
        };
        commands
            .entity(chip)
            .queue(set_label(&tab_label(tab.0, count)));
    }
    for mut image in &mut parts.icon {
        let sprite = atlas.sprite(&format!("{:?}", view.item));
        if image.texture_atlas != sprite {
            image.texture_atlas = sprite;
        }
    }

    if built.0 == Some(view) {
        return;
    }
    // the search too, after Enter in it
    let catalog_focused = parts.catalog_roots.iter().any(inside);
    if inside(cards) || catalog_focused {
        focus.set(*parts.back, FocusCause::Navigated);
    }
    built.0 = Some(view);
    commands.entity(cards).despawn_related::<Children>();
    let shown = recipes_of(view);
    if shown.is_empty() {
        commands.spawn((
            label(&empty_view_message(view), &font, FONT_S, TEXT),
            ChildOf(cards),
        ));
    }
    for recipe in &shown {
        spawn_card(&mut commands, cards, recipe, atlas, &font);
    }
}

/// Tints the ingredients the player lacks, and says under each card where it is made or why it
/// cannot be made now, with the crafting grid in use.
pub fn style_recipe_cards(
    inventory: Res<Inventory>,
    mut tints: Query<(&MissingTint, &mut Node)>,
    mut notes: Query<(&RecipeNote, &mut Text, &mut TextColor)>,
) {
    let all: Vec<RecipeView> = recipes().collect();
    let grid_size = grid_in_use(&inventory);
    for (tint, mut node) in &mut tints {
        let lacking = shared::world::missing(&inventory, &all[tint.recipe as usize])
            .iter()
            .any(|(item, _)| *item == tint.item);
        let display = if lacking {
            Display::Flex
        } else {
            Display::None
        };
        if node.display != display {
            node.display = display;
        }
    }
    for (note, mut text, mut color) in &mut notes {
        let recipe = &all[note.0 as usize];
        let (content, tone) = match recipe_problem(&inventory, recipe, grid_size) {
            Some(problem) => (problem, DANGER),
            None => (station_label(recipe).to_string(), MUTED),
        };
        if text.0 != content {
            text.0 = content;
        }
        color.set_if_neq(TextColor(tone));
    }
}

/// The back button activated goes back in the history.
pub fn go_back(
    activate: On<Activate>,
    buttons: Query<(), With<BackButton>>,
    mut state: ResMut<ItemBrowserState>,
) {
    if buttons.contains(activate.entity) {
        state.back();
    }
}

/// A tab chip activated shows that tab of the same item, without going on the history.
pub fn choose_tab(
    activate: On<Activate>,
    chips: Query<&RecipeTabChip>,
    mut state: ResMut<ItemBrowserState>,
) {
    let (Ok(chip), Some(view)) = (chips.get(activate.entity), state.view) else {
        return;
    };
    if view.tab != chip.0 {
        state.view = Some(ItemView {
            item: view.item,
            tab: chip.0,
        });
    }
}

/// A press on a slot of a card opens the recipes of its item, a right press its uses; in Survival
/// a slot of the catalog does the same. Enter and Space do so on the focused slot while its ring
/// shows, and not while the player types.
#[allow(clippy::too_many_arguments)]
pub fn click_items(
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    ui_mode: Res<UIMode>,
    focus: Res<InputFocus>,
    focus_visible: Res<InputFocusVisible>,
    layout: Res<BrowserLayout>,
    craftable: Res<CraftableNow>,
    inventory_root: Query<&Visibility, With<InventoryRoot>>,
    catalog: Query<(Entity, &Hovered, &CatalogSlot)>,
    cards: Query<(Entity, &Hovered, &RecipeSlot)>,
    mut state: ResMut<ItemBrowserState>,
) {
    if !matches!(inventory_root.single(), Ok(&Visibility::Visible)) {
        return;
    }
    let typing = *ui_mode == UIMode::Typing;
    let (left, right) = (
        mouse.just_pressed(MouseButton::Left),
        mouse.just_pressed(MouseButton::Right),
    );
    let (enter, space) = (
        !typing && keys.just_pressed(KeyCode::Enter),
        !typing && keys.just_pressed(KeyCode::Space),
    );
    let focused = focus.get().filter(|_| focus_visible.0);
    let tab = |entity: Entity, hovered: &Hovered| {
        let (recipes, uses) = if focused == Some(entity) {
            (enter, space)
        } else if hovered.get() {
            (left, right)
        } else {
            (false, false)
        };
        if recipes {
            Some(RecipeTab::Recipes)
        } else if uses {
            Some(RecipeTab::Uses)
        } else {
            None
        }
    };
    let from_catalog = catalog
        .iter()
        .filter(|_| craftable.survival)
        .find_map(|(entity, hovered, slot)| Some((slot.0, tab(entity, hovered)?)));
    let from_cards = || {
        cards
            .iter()
            .find_map(|(entity, hovered, slot)| Some((slot.item, tab(entity, hovered)?)))
    };
    if let Some((item, tab)) = from_catalog.or_else(from_cards) {
        state.open(ItemView { item, tab }, layout.compact);
    }
}

/// R shows the recipes, U the uses, of the item of a slot: the focused one while its ring shows,
/// else the one under the cursor, in the catalog, a card, the inventory, the crafting grid and
/// its result, or a chest. In both modes, not while the player types.
#[allow(clippy::too_many_arguments)]
pub fn item_keys(
    keys: Res<ButtonInput<KeyCode>>,
    ui_mode: Res<UIMode>,
    focus: Res<InputFocus>,
    focus_visible: Res<InputFocusVisible>,
    layout: Res<BrowserLayout>,
    inventory: Res<Inventory>,
    world_map: Res<ClientWorldMap>,
    inventory_root: Query<&Visibility, With<InventoryRoot>>,
    catalog: Query<(Entity, &Hovered, &CatalogSlot)>,
    cards: Query<(Entity, &Hovered, &RecipeSlot)>,
    cells: Query<(Entity, &Hovered, &InventoryCell)>,
    result: Query<(Entity, &Hovered), With<CraftResultCell>>,
    mut state: ResMut<ItemBrowserState>,
) {
    let tab = if keys.just_pressed(KeyCode::KeyR) {
        RecipeTab::Recipes
    } else if keys.just_pressed(KeyCode::KeyU) {
        RecipeTab::Uses
    } else {
        return;
    };
    if *ui_mode == UIMode::Typing || !matches!(inventory_root.single(), Ok(&Visibility::Visible)) {
        return;
    }
    // Every item slot, whether it is hovered, and its item
    let slots = catalog
        .iter()
        .map(|(entity, hovered, slot)| (entity, hovered.get(), Some(slot.0)))
        .chain(
            cards
                .iter()
                .map(|(entity, hovered, slot)| (entity, hovered.get(), Some(slot.item))),
        )
        .chain(cells.iter().map(|(entity, hovered, cell)| {
            let stack = stack_in(cell.id, &inventory, &world_map);
            (entity, hovered.get(), stack.map(|stack| stack.item_id))
        }))
        .chain(result.iter().map(|(entity, hovered)| {
            let made = craft_result(&inventory, grid_in_use(&inventory));
            (entity, hovered.get(), made.map(|stack| stack.item_id))
        }));
    let focused = focus.get().filter(|_| focus_visible.0);
    let (mut on_focus, mut on_hover) = (None, None);
    for (entity, hovered, item) in slots {
        if focused == Some(entity) {
            on_focus = item;
        } else if hovered && on_hover.is_none() {
            on_hover = item;
        }
    }
    if let Some(item) = on_focus.or(on_hover) {
        state.open(ItemView { item, tab }, layout.compact);
    }
}

/// Escape goes back in the history of the view while it shows; the drawer and the inventory wait
/// for the view to be closed. Not while the player types.
pub fn view_keys(
    keys: Res<ButtonInput<KeyCode>>,
    ui_mode: Res<UIMode>,
    layout: Res<BrowserLayout>,
    inventory_root: Query<&Visibility, With<InventoryRoot>>,
    mut state: ResMut<ItemBrowserState>,
) {
    let open = matches!(inventory_root.single(), Ok(&Visibility::Visible));
    let shown = open && (!layout.compact || state.drawer_open);
    if shown
        && state.view.is_some()
        && *ui_mode != UIMode::Typing
        && keys.just_pressed(KeyCode::Escape)
    {
        state.back();
    }
}

/// Whether Escape belongs to the browser rather than to the inventory: while its view shows, or
/// while it is an open drawer. Never in an app without the browser, as the pause menu's tests
/// are.
pub fn browser_takes_escape(
    state: Option<Res<ItemBrowserState>>,
    layout: Option<Res<BrowserLayout>>,
    inventory_root: Query<&Visibility, With<InventoryRoot>>,
) -> bool {
    let open = matches!(inventory_root.single(), Ok(&Visibility::Visible));
    let (Some(state), Some(layout)) = (state, layout) else {
        return false;
    };
    open && if layout.compact {
        state.drawer_open
    } else {
        state.view.is_some()
    }
}

#[cfg(test)]
mod tests {
    use bevy::ecs::system::RunSystemOnce;
    use shared::world::{grid_slot, ItemStack};

    use super::*;

    fn put(world: &mut World, slot: u32, item_id: ItemId, nb: u32) {
        let stack = ItemStack {
            item_id,
            item_type: item_id.get_default_type(),
            nb,
        };
        world.resource_mut::<Inventory>().inner.insert(slot, stack);
    }

    /// The items whose tints show, and the note and its colour.
    fn styled(world: &mut World) -> (Vec<ItemId>, String, Color) {
        world.run_system_once(style_recipe_cards).unwrap();
        let mut tints = world.query::<(&MissingTint, &Node)>();
        let tinted = tints
            .iter(world)
            .filter(|(_, node)| node.display != Display::None)
            .map(|(tint, _)| tint.item)
            .collect();
        let mut notes = world.query::<(&Text, &TextColor)>();
        let (text, color) = notes.single(world).unwrap();
        (tinted, text.0.clone(), color.0)
    }

    #[test]
    fn missing_ingredients_are_tinted_and_named() {
        // The cells of the oak pickaxe: three planks, then two sticks
        let pickaxe = recipes_for(ItemId::WoodenPickaxe).next().unwrap();
        let mut world = World::new();
        world.init_resource::<Inventory>();
        for item in pickaxe.cells.iter().flatten() {
            world.spawn((
                MissingTint {
                    recipe: pickaxe.index,
                    item: *item,
                },
                Node::default(),
            ));
        }
        world.spawn((RecipeNote(pickaxe.index), Text::default(), TextColor(TEXT)));

        put(&mut world, 3, ItemId::OakPlanks, 5);
        assert_eq!(
            styled(&mut world),
            (
                vec![ItemId::Stick, ItemId::Stick],
                "Missing 2 Stick".to_string(),
                DANGER
            )
        );

        put(&mut world, grid_slot(0, 0), ItemId::Stick, 2);
        assert_eq!(
            styled(&mut world),
            (vec![], "Needs a Crafting Table".to_string(), DANGER)
        );

        world.resource_mut::<Inventory>().open_table = Some(IVec3::ZERO);
        assert_eq!(
            styled(&mut world),
            (vec![], "Crafting Table 3x3".to_string(), MUTED)
        );
    }
}
