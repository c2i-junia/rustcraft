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
use shared::world::{recipes, Ingredient, ItemId, ItemTag, RecipeView};

use super::bookmarks::{bookmark_message, write_bookmarks, Bookmarks};
use super::catalog::category_detail;
use super::layout::BrowserLayout;
use super::recipes::{
    back_label, bookmarked_detail, burns_for, card_label, cards_of, empty_view_message,
    fill_button, fuel_note, ingredient_label, item_keys_hint, recipe_problem, smelting_note,
    station_label, tab_label, view_title, BrowserCard,
};
use super::{
    BackButton, BookmarkBar, BrowserRequests, BrowserTitle, CatalogPart, CatalogSlot, CraftableNow,
    FillGridButton, ItemBrowserState, ItemCount, ItemView, MissingTint, RecipeCard, RecipeCards,
    RecipeNote, RecipePanel, RecipeSlot, RecipeStop, RecipeTab, RecipeTabChip, TagCell, ViewedItem,
    ViewedItemCategory, ViewedItemName, OK_HI, OK_LO,
};
use crate::ui::hud::inventory::{grid_in_use, lit_flame, stack_in};
use crate::ui::hud::{item_name, CraftResultCell, InventoryCell, InventoryRoot, UIMode};
use crate::ui::theme::{
    BEVEL_HI, BEVEL_LO, BEVEL_WIDTH, DANGER, DISABLED_ALPHA, FONT_M, FONT_S, MUTED, OK, PANEL,
    RAISE, SLOT, SPACE_S, SPACE_XS, TEXT,
};
use crate::ui::widgets::button::set_label;
use crate::ui::widgets::focus::FocusRing;
use crate::ui::widgets::hints::key_cap;
use crate::ui::widgets::list::{spawn_scroll_list, ScrollListParts, WheelStep, RING_ROOM};
use crate::ui::widgets::slot::{slot, SlotIcon, ICON_SIZE, SLOT_SIZE};
use crate::ui::widgets::toast::{show_toast, ToastKind};
use crate::ui::widgets::tooltip::Tooltip;
use crate::world::{AtlasWrapper, ClientWorldMap, MaterialResource};
use shared::GameFolderPaths;

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
/// The tint over an ingredient the player lacks, on the cards and under the ghost of a recipe.
pub const MISSING_TINT: Color = Color::srgba(0.89, 0.35, 0.25, 0.32);
/// The label of the button filling the crafting grid.
pub const FILL_GRID: &str = "+ Fill grid";
/// The text of a disabled `+ Fill grid`.
fn fill_disabled_text() -> Color {
    MUTED.with_alpha(DISABLED_ALPHA * 2.0)
}
/// The fill of an enabled `+ Fill grid` under the cursor, as the mockup.
const OK_HOVER: Color = Color::srgb_u8(0x8F, 0xD8, 0x77);

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

/// A slot of a card showing `ingredient`, `count` of it for the output; an ingredient of
/// `recipe` has the tint shown when the player lacks it. A family shows its first member, until
/// `cycle_tag_cells` shows the others in turn.
#[allow(clippy::too_many_arguments)]
fn spawn_card_slot(
    commands: &mut Commands,
    parent: Entity,
    ingredient: Ingredient,
    recipe: Option<u16>,
    count: u32,
    size: f32,
    atlas: &AtlasWrapper,
    font: &Handle<Font>,
) {
    let name = ingredient_label(ingredient);
    let item = ingredient.members()[0];
    let mut cell = commands.spawn((
        RecipeSlot { item, count },
        RecipeStop,
        Tooltip {
            title: name.clone(),
            detail: category_detail(item),
            note: String::new(),
            hint: item_keys_hint(false),
        },
        slot(&name, None, icon(atlas, item), font.clone()),
        ChildOf(parent),
    ));
    if let Ingredient::Tag(tag) = ingredient {
        cell.insert(TagCell(tag));
    }
    cell.entry::<Node>().and_modify(move |mut node| {
        node.width = px(size);
        node.height = px(size);
    });
    if let Some(recipe) = recipe {
        // after the icon: drawn over it
        cell.with_child((
            MissingTint { recipe, ingredient },
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

/// An empty cell of a card, dashed around.
fn empty_cell() -> impl Bundle {
    (
        Node {
            width: px(CARD_CELL),
            height: px(CARD_CELL),
            flex_shrink: 0.0,
            ..default()
        },
        BackgroundColor(SLOT.with_alpha(0.5)),
        Pickable::IGNORE,
        Children::spawn(SpawnIter(dashes())),
    )
}

/// The frame of a card named `name` at the end of `cards`, and the row of its body.
fn spawn_card_frame(commands: &mut Commands, cards: Entity, name: String) -> (Entity, Entity) {
    let card = commands
        .spawn((
            RecipeCard,
            // the role before the label: the label's default node must not win
            AccessibilityNode(accesskit::Node::new(Role::Group)),
            AccessibleLabel::new(name),
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
    (card, body)
}

/// The arrow of a card, from its ingredients to its output.
fn card_arrow(font: &Handle<Font>) -> impl Bundle {
    (
        label("->", font, FONT_M, MUTED),
        TextLayout {
            linebreak: LineBreak::NoWrap,
            ..default()
        },
        Node {
            flex_shrink: 0.0,
            ..default()
        },
    )
}

/// A card of `recipe` at the end of `cards`: its 3 × 3 grid, the arrow and its output, then the
/// station or why it cannot be made now, and the button that fills the grid with it.
fn spawn_card(
    commands: &mut Commands,
    cards: Entity,
    recipe: &RecipeView,
    atlas: &AtlasWrapper,
    font: &Handle<Font>,
) {
    let output = recipe.output.item_id;
    let (card, body) =
        spawn_card_frame(commands, cards, card_label(&BrowserCard::Crafting(*recipe)));
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
            let ingredient = (row < recipe.height && col < recipe.width)
                .then(|| recipe.cells[row * recipe.width + col])
                .flatten();
            match ingredient {
                Some(ingredient) => spawn_card_slot(
                    commands,
                    grid,
                    ingredient,
                    Some(recipe.index),
                    1,
                    CARD_CELL,
                    atlas,
                    font,
                ),
                None => {
                    commands.spawn((empty_cell(), ChildOf(grid)));
                }
            }
        }
    }
    commands.spawn((card_arrow(font), ChildOf(body)));
    spawn_card_slot(
        commands,
        body,
        Ingredient::Item(output),
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
                FillGridButton(recipe.index),
                // enabled by `style_recipe_cards` once the recipe can be made
                InteractionDisabled,
                small_button(FILL_GRID, fill_disabled_text(), font),
            ),
        ],
    ));
}

/// A card of the furnace named `name` at the end of `cards`, laid out as the furnace: a column of
/// the `input` cell, the lit flame and the `fuel` cell, as wide as the grid of a crafting card, an
/// empty cell dashed. Returns the card and the row of its body, after the column.
#[allow(clippy::too_many_arguments)]
fn spawn_furnace_card(
    commands: &mut Commands,
    cards: Entity,
    name: String,
    input: Option<ItemId>,
    fuel: Option<ItemId>,
    atlas: &AtlasWrapper,
    font: &Handle<Font>,
    assets: &AssetServer,
) -> (Entity, Entity) {
    let (card, body) = spawn_card_frame(commands, cards, name);
    let column = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                flex_shrink: 0.0,
                width: px(3.0 * CARD_CELL + 2.0 * CARD_CELL_GAP),
                row_gap: px(CARD_CELL_GAP),
                ..default()
            },
            ChildOf(body),
        ))
        .id();
    let cell = |commands: &mut Commands, item: Option<ItemId>| match item {
        Some(item) => spawn_card_slot(
            commands,
            column,
            Ingredient::Item(item),
            None,
            1,
            CARD_CELL,
            atlas,
            font,
        ),
        None => {
            commands.spawn((empty_cell(), ChildOf(column)));
        }
    };
    cell(commands, input);
    commands.spawn((lit_flame(assets), ChildOf(column)));
    cell(commands, fuel);
    (card, body)
}

/// A card of the furnace making `output` of `input`, any fuel doing: the arrow and the output
/// after the column, and under them the furnace and how long it takes. A furnace is filled by
/// hand: there is no `+ Fill grid`.
fn spawn_smelting_card(
    commands: &mut Commands,
    cards: Entity,
    (input, output): (ItemId, ItemId),
    atlas: &AtlasWrapper,
    font: &Handle<Font>,
    assets: &AssetServer,
) {
    let name = card_label(&BrowserCard::Smelting { input, output });
    let (card, body) = spawn_furnace_card(
        commands,
        cards,
        name,
        Some(input),
        None,
        atlas,
        font,
        assets,
    );
    commands.spawn((card_arrow(font), ChildOf(body)));
    spawn_card_slot(
        commands,
        body,
        Ingredient::Item(output),
        None,
        1,
        SLOT_SIZE,
        atlas,
        font,
    );
    commands.spawn((label(&smelting_note(), font, FONT_S, MUTED), ChildOf(card)));
}

/// A card of `item` burning in the furnace, `ticks` long: how many items it smelts after the
/// column, and under them how long it burns.
fn spawn_fuel_card(
    commands: &mut Commands,
    cards: Entity,
    (item, ticks): (ItemId, u16),
    atlas: &AtlasWrapper,
    font: &Handle<Font>,
    assets: &AssetServer,
) {
    let name = card_label(&BrowserCard::Fuel { item, ticks });
    let (card, body) =
        spawn_furnace_card(commands, cards, name, None, Some(item), atlas, font, assets);
    commands.spawn((label(&burns_for(ticks), font, FONT_S, TEXT), ChildOf(body)));
    commands.spawn((label(&fuel_note(ticks), font, FONT_S, MUTED), ChildOf(card)));
}

/// The parts of the panel that `show_view` writes.
#[derive(bevy::ecs::system::SystemParam)]
pub struct ViewParts<'w, 's> {
    catalog_parts: Query<'w, 's, &'static mut Node, (With<CatalogPart>, Without<RecipePanel>)>,
    catalog_roots: Query<'w, 's, Entity, Or<(With<CatalogPart>, With<BookmarkBar>)>>,
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
    scrolls: Query<'w, 's, &'static mut ScrollPosition>,
}

/// Shows the open view instead of the catalog, or the catalog: the title, the back button, the
/// item, its tabs and, when the view changed, its cards. A focus left in a part that goes away
/// moves: to the back button in a view (from the cards or from the catalog, its search
/// included), to the item's slot in the catalog.
pub fn show_view(
    state: Res<ItemBrowserState>,
    bookmarks: Res<Bookmarks>,
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
            (false, false, Some(view)) => {
                bookmarked_detail(view.item, bookmarks.contains(view.item))
            }
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
        let count = cards_of(ItemView { tab: tab.0, ..view }).len();
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
    // the search and the bookmarks too, after Enter in the search or on a bookmark
    let catalog_focused = parts.catalog_roots.iter().any(inside);
    if inside(cards) || catalog_focused {
        focus.set(*parts.back, FocusCause::Navigated);
    }
    built.0 = Some(view);
    commands.entity(cards).despawn_related::<Children>();
    // the cards of another view start from the top
    if let Ok(list) = parts.parents.get(cards) {
        if let Ok(mut scroll) = parts.scrolls.get_mut(list.parent()) {
            *scroll = ScrollPosition::default();
        }
    }
    let shown = cards_of(view);
    if shown.is_empty() {
        commands.spawn((
            label(&empty_view_message(view), &font, FONT_S, TEXT),
            ChildOf(cards),
        ));
    }
    let assets = &asset_server;
    for card in &shown {
        match *card {
            BrowserCard::Crafting(recipe) => {
                spawn_card(&mut commands, cards, &recipe, atlas, &font)
            }
            BrowserCard::Smelting { input, output } => {
                spawn_smelting_card(&mut commands, cards, (input, output), atlas, &font, assets)
            }
            BrowserCard::Fuel { item, ticks } => {
                spawn_fuel_card(&mut commands, cards, (item, ticks), atlas, &font, assets)
            }
        }
    }
}

/// The member of `tag` its cells show `elapsed` after the start: each in turn, a second each.
pub fn tag_member(tag: ItemTag, elapsed: std::time::Duration) -> ItemId {
    let members = tag.members();
    members[(elapsed.as_secs() % members.len() as u64) as usize]
}

/// Shows in each cell of a family the member of the second, `tag_member`, every cell in step: its
/// icon, and the item a click and the item keys act on.
pub fn cycle_tag_cells(
    time: Res<Time<Real>>,
    materials: Res<MaterialResource>,
    mut cells: Query<(&TagCell, &mut RecipeSlot, Option<&Children>)>,
    mut icons: Query<&mut ImageNode, With<SlotIcon>>,
) {
    for (tag, mut slot, children) in &mut cells {
        let member = tag_member(tag.0, time.elapsed());
        if slot.item == member {
            continue;
        }
        slot.item = member;
        let (Some(atlas), Some(children)) = (materials.items.as_ref(), children) else {
            continue;
        };
        for child in children.iter() {
            if let Ok(mut icon) = icons.get_mut(child) {
                icon.texture_atlas = atlas.sprite(&format!("{member:?}"));
            }
        }
    }
}

/// Tints the ingredients the player lacks, and says under each card where it is made or why it
/// cannot be made now, with the crafting grid in use. `+ Fill grid` is enabled, and takes Tab,
/// when it can be made, or when only ingredients are missing: it then lays a ghost of it.
pub fn style_recipe_cards(
    inventory: Res<Inventory>,
    mut tints: Query<(&MissingTint, &mut Node)>,
    mut notes: Query<(&RecipeNote, &mut Text, &mut TextColor)>,
    buttons: Query<(Entity, &FillGridButton, Has<InteractionDisabled>)>,
    mut commands: Commands,
) {
    let all: Vec<RecipeView> = recipes().collect();
    let grid_size = grid_in_use(&inventory);
    for (tint, mut node) in &mut tints {
        let lacking = shared::world::missing(&inventory, &all[tint.recipe as usize])
            .iter()
            .any(|(ingredient, _)| *ingredient == tint.ingredient);
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
    for (button, fill, disabled) in &buttons {
        let enabled = fill_button(&inventory, &all[fill.0 as usize], grid_size).is_some();
        if enabled && disabled {
            commands
                .entity(button)
                .remove::<InteractionDisabled>()
                .insert(RecipeStop);
        } else if !enabled && !disabled {
            commands
                .entity(button)
                .remove::<RecipeStop>()
                .insert((InteractionDisabled, TabIndex(-1)));
        }
    }
}

/// An enabled `+ Fill grid` is green, lighter under the cursor, as the mockup's; a disabled one is
/// raised and muted.
#[allow(clippy::type_complexity)]
pub fn style_fill_buttons(
    mut buttons: Query<
        (
            &Hovered,
            Has<InteractionDisabled>,
            &mut BackgroundColor,
            &mut BorderColor,
            &Children,
        ),
        With<FillGridButton>,
    >,
    mut texts: Query<&mut TextColor>,
) {
    for (hovered, disabled, mut background, mut border, children) in &mut buttons {
        let (fill, edge, text) = if disabled {
            (RAISE, bevel(BEVEL_HI, BEVEL_LO), fill_disabled_text())
        } else if hovered.get() {
            (OK_HOVER, bevel(OK_HI, OK_LO), PANEL)
        } else {
            (OK, bevel(OK_HI, OK_LO), PANEL)
        };
        background.set_if_neq(BackgroundColor(fill));
        border.set_if_neq(edge);
        for child in children.iter() {
            if let Ok(mut color) = texts.get_mut(child) {
                color.set_if_neq(TextColor(text));
            }
        }
    }
}

/// A raised bevel: `light` on the top and left edges, `dark` on the others.
fn bevel(light: Color, dark: Color) -> BorderColor {
    BorderColor {
        top: light,
        left: light,
        bottom: dark,
        right: dark,
    }
}

/// `+ Fill grid` activated asks `express_fill` to fill the grid with its recipe, as many times
/// as possible with Shift, or to lay its ghost.
pub fn fill_grid(
    activate: On<Activate>,
    buttons: Query<&FillGridButton>,
    keys: Res<ButtonInput<KeyCode>>,
    mut requests: ResMut<BrowserRequests>,
) {
    if let Ok(fill) = buttons.get(activate.entity) {
        let shift = keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]);
        requests.fill = Some((fill.0, shift));
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
/// a slot of the catalog does the same, but for a left press with Ctrl, which fills the grid
/// (`express_fill`). Enter and Space do so on the focused slot while its ring shows, and not while
/// the player types.
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
    let ctrl = keys.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight]);
    let focused = focus.get().filter(|_| focus_visible.0);
    let tab = |entity: Entity, hovered: &Hovered, left: bool| {
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
        .find_map(|(entity, hovered, slot)| Some((slot.0, tab(entity, hovered, left && !ctrl)?)));
    let from_cards = || {
        cards
            .iter()
            .find_map(|(entity, hovered, slot)| Some((slot.item, tab(entity, hovered, left)?)))
    };
    if let Some((item, tab)) = from_catalog.or_else(from_cards) {
        state.open(ItemView { item, tab }, layout.compact);
    }
}

/// The slot the item keys act on, R, U and A: the focused one while its ring shows, else the
/// one under the cursor, in the catalog (its bookmarks included), a card, the inventory, the
/// crafting grid and its result, or a chest.
#[derive(bevy::ecs::system::SystemParam)]
pub struct KeyedSlot<'w, 's> {
    focus: Res<'w, InputFocus>,
    focus_visible: Res<'w, InputFocusVisible>,
    inventory: Res<'w, Inventory>,
    world_map: Res<'w, ClientWorldMap>,
    catalog: Query<'w, 's, (Entity, &'static Hovered, &'static CatalogSlot)>,
    cards: Query<'w, 's, (Entity, &'static Hovered, &'static RecipeSlot)>,
    cells: Query<'w, 's, (Entity, &'static Hovered, &'static InventoryCell)>,
    result: Query<'w, 's, (Entity, &'static Hovered), With<CraftResultCell>>,
}

impl KeyedSlot<'_, '_> {
    /// The item of the slot the keys act on, if it holds one.
    pub fn item(&self) -> Option<ItemId> {
        // Every item slot, whether it is hovered, and its item
        let slots = self
            .catalog
            .iter()
            .map(|(entity, hovered, slot)| (entity, hovered.get(), Some(slot.0)))
            .chain(
                self.cards
                    .iter()
                    .map(|(entity, hovered, slot)| (entity, hovered.get(), Some(slot.item))),
            )
            .chain(self.cells.iter().map(|(entity, hovered, cell)| {
                let stack = stack_in(cell.id, &self.inventory, &self.world_map);
                (entity, hovered.get(), stack.map(|stack| stack.item_id))
            }))
            .chain(self.result.iter().map(|(entity, hovered)| {
                let made = craft_result(&self.inventory, grid_in_use(&self.inventory));
                (entity, hovered.get(), made.map(|stack| stack.item_id))
            }));
        let focused = self.focus.get().filter(|_| self.focus_visible.0);
        let (mut on_focus, mut on_hover) = (None, None);
        for (entity, hovered, item) in slots {
            if focused == Some(entity) {
                on_focus = item;
            } else if hovered && on_hover.is_none() {
                on_hover = item;
            }
        }
        on_focus.or(on_hover)
    }
}

/// R shows the recipes, U the uses, of the item of the `KeyedSlot`. In both modes, not while the
/// player types.
pub fn item_keys(
    keys: Res<ButtonInput<KeyCode>>,
    ui_mode: Res<UIMode>,
    layout: Res<BrowserLayout>,
    inventory_root: Query<&Visibility, With<InventoryRoot>>,
    slot: KeyedSlot,
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
    if let Some(item) = slot.item() {
        state.open(ItemView { item, tab }, layout.compact);
    }
}

/// A bookmarks the item of the `KeyedSlot`, or removes it from the bookmarks, saves them and says
/// so in a toast. In both modes, not while the player types.
#[allow(clippy::too_many_arguments)]
pub fn bookmark_key(
    keys: Res<ButtonInput<KeyCode>>,
    ui_mode: Res<UIMode>,
    inventory_root: Query<&Visibility, With<InventoryRoot>>,
    slot: KeyedSlot,
    paths: Res<GameFolderPaths>,
    asset_server: Res<AssetServer>,
    mut bookmarks: ResMut<Bookmarks>,
    mut commands: Commands,
) {
    if !keys.just_pressed(KeyCode::KeyA)
        || *ui_mode == UIMode::Typing
        || !matches!(inventory_root.single(), Ok(&Visibility::Visible))
    {
        return;
    }
    let Some(item) = slot.item() else {
        return;
    };
    let added = bookmarks.toggle(item);
    if let Some(key) = &bookmarks.key {
        write_bookmarks(&paths, key, &bookmarks.items);
    }
    show_toast(
        &mut commands,
        &bookmark_message(item, added),
        ToastKind::Info,
        crate::ui::assets::load_font(&asset_server),
    );
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
    use std::time::Duration;

    use bevy::ecs::system::RunSystemOnce;
    use shared::world::{grid_slot, recipes_for, Ingredient, ItemStack, ItemTag};

    use super::*;

    fn put(world: &mut World, slot: u32, item_id: ItemId, nb: u32) {
        let stack = ItemStack {
            item_id,
            item_type: item_id.get_default_type(),
            nb,
        };
        world.resource_mut::<Inventory>().inner.insert(slot, stack);
    }

    /// The ingredients whose tints show, and the note and its colour.
    fn styled(world: &mut World) -> (Vec<Ingredient>, String, Color) {
        world.run_system_once(style_recipe_cards).unwrap();
        let mut tints = world.query::<(&MissingTint, &Node)>();
        let tinted = tints
            .iter(world)
            .filter(|(_, node)| node.display != Display::None)
            .map(|(tint, _)| tint.ingredient)
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
        for ingredient in pickaxe.cells.iter().flatten() {
            world.spawn((
                MissingTint {
                    recipe: pickaxe.index,
                    ingredient: *ingredient,
                },
                Node::default(),
            ));
        }
        world.spawn((RecipeNote(pickaxe.index), Text::default(), TextColor(TEXT)));
        let (planks, stick) = (
            Ingredient::Tag(ItemTag::Planks),
            Ingredient::Item(ItemId::Stick),
        );

        // Any species of planks counts, all of them together
        put(&mut world, 3, ItemId::SprucePlanks, 1);
        put(&mut world, 4, ItemId::JunglePlanks, 1);
        assert_eq!(
            styled(&mut world),
            (
                vec![planks, planks, planks, stick, stick],
                "Missing 1 Planks, 2 Stick".to_string(),
                DANGER
            )
        );
        put(&mut world, 5, ItemId::OakPlanks, 5);
        assert_eq!(
            styled(&mut world),
            (vec![stick, stick], "Missing 2 Stick".to_string(), DANGER)
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

    #[test]
    fn fill_grid_is_enabled_when_the_recipe_fits_and_no_chest_or_furnace_is_open() {
        let sticks = recipes_for(ItemId::Stick).next().unwrap();
        let pickaxe = recipes_for(ItemId::WoodenPickaxe).next().unwrap();
        let mut world = World::new();
        world.init_resource::<Inventory>();
        let stick_button = world
            .spawn((FillGridButton(sticks.index), InteractionDisabled))
            .id();
        let pickaxe_button = world
            .spawn((FillGridButton(pickaxe.index), InteractionDisabled))
            .id();
        let enabled = |world: &mut World| {
            world.run_system_once(style_recipe_cards).unwrap();
            [stick_button, pickaxe_button].map(|button| {
                let entity = world.entity(button);
                let enabled = !entity.contains::<InteractionDisabled>();
                assert_eq!(entity.contains::<RecipeStop>(), enabled);
                enabled
            })
        };
        // The sticks lack their planks: their button lays a ghost; the pickaxe needs a table
        assert_eq!(enabled(&mut world), [true, false]);

        put(&mut world, 3, ItemId::OakPlanks, 5);
        assert_eq!(enabled(&mut world), [true, false]);
        world.resource_mut::<Inventory>().open_table = Some(IVec3::ZERO);
        assert_eq!(enabled(&mut world), [true, true]);
        world.resource_mut::<Inventory>().open_table = None;

        // The chest takes the place of the grid, and so does the furnace
        world.resource_mut::<Inventory>().open_chest = Some(IVec3::ZERO);
        assert_eq!(enabled(&mut world), [false, false]);
        assert_eq!(world.get::<TabIndex>(stick_button), Some(&TabIndex(-1)));
        world.resource_mut::<Inventory>().open_chest = None;
        world.resource_mut::<Inventory>().open_furnace = Some(IVec3::ZERO);
        assert_eq!(enabled(&mut world), [false, false]);
    }

    #[test]
    fn a_family_shows_its_members_one_per_second_in_turn() {
        let at = |ms| tag_member(ItemTag::Planks, Duration::from_millis(ms));
        assert_eq!(at(0), ItemId::OakPlanks);
        assert_eq!(at(999), ItemId::OakPlanks);
        assert_eq!(at(1000), ItemId::SprucePlanks);
        assert_eq!(at(2500), ItemId::JunglePlanks);
        assert_eq!(at(3000), ItemId::OakPlanks);
        assert_eq!(at(7200), ItemId::SprucePlanks);
    }

    #[test]
    fn the_cells_of_a_family_cycle_in_step() {
        let mut world = World::new();
        world.init_resource::<MaterialResource>();
        let mut time = Time::<Real>::default();
        time.update_with_duration(Duration::ZERO);
        world.insert_resource(time);
        let cell = |world: &mut World| {
            world
                .spawn((
                    TagCell(ItemTag::Planks),
                    RecipeSlot {
                        item: ItemId::OakPlanks,
                        count: 1,
                    },
                ))
                .id()
        };
        let cells = [cell(&mut world), cell(&mut world)];
        let stick = world
            .spawn(RecipeSlot {
                item: ItemId::Stick,
                count: 1,
            })
            .id();
        let shown_after = |world: &mut World, ms: u64| {
            world
                .resource_mut::<Time<Real>>()
                .update_with_duration(Duration::from_millis(ms));
            world.run_system_once(cycle_tag_cells).unwrap();
            [cells[0], cells[1], stick].map(|cell| world.get::<RecipeSlot>(cell).unwrap().item)
        };
        assert_eq!(
            shown_after(&mut world, 400),
            [ItemId::OakPlanks, ItemId::OakPlanks, ItemId::Stick]
        );
        assert_eq!(
            shown_after(&mut world, 700),
            [ItemId::SprucePlanks, ItemId::SprucePlanks, ItemId::Stick]
        );
        assert_eq!(
            shown_after(&mut world, 1000),
            [ItemId::JunglePlanks, ItemId::JunglePlanks, ItemId::Stick]
        );
        assert_eq!(
            shown_after(&mut world, 1000),
            [ItemId::OakPlanks, ItemId::OakPlanks, ItemId::Stick]
        );
    }
}
