use accesskit::Role;
use bevy::{
    a11y::AccessibilityNode, input_focus::tab_navigation::TabIndex, picking::hover::Hovered,
    prelude::*, text::EditableText, ui_widgets::Button as UiButton,
};
use shared::world::{ItemCategory, ItemId};

use super::catalog::{catalog, category_detail, NO_MATCH_HINT};
use super::creative::{TRASH_HINT, TRASH_LABEL};
use super::recipes::item_keys_hint;
use super::{
    choose_category, clear_or_leave_search, open_drawer, search_arrow_down, spawn_recipe_panel,
    submit_search, toggle_craftable, BookmarkBar, BookmarkRow, BrowserTitle, CatalogGrid,
    CatalogList, CatalogPart, CatalogSlot, CategoryChip, CraftableChip, CraftableDot, DrawerHandle,
    ItemBrowser, ItemBrowserState, ItemCount, ItemSearchField, ModeFooter, NoMatch, NoMatchHint,
    NoMatchMessage, TrashSlot, ITEM_BROWSER_TITLE, SEARCH_LABEL,
};
use crate::ui::assets::{load_font, MONO_FONT_PATH};
use crate::ui::hud::{item_name, HudBottom};
use crate::ui::theme::{
    BEVEL_HI, BEVEL_LO, BEVEL_WIDTH, DANGER, FONT_L, FONT_S, MUTED, OK, PANEL, RAISE, SPACE_L,
    SPACE_S, SPACE_XS, TEXT,
};
use crate::ui::widgets::focus::{ArrowNav, FocusRing};
use crate::ui::widgets::hints::key_cap;
use crate::ui::widgets::list::{spawn_scroll_list, ScrollListParts, WheelStep, RING_ROOM};
use crate::ui::widgets::slot::{slot, SLOT_SIZE};
use crate::ui::widgets::text_field::{set_text, spawn_text_field, TextField};
use crate::ui::widgets::tooltip::Tooltip;
use crate::world::{AtlasWrapper, MaterialResource};

/// The most characters the search field takes.
const SEARCH_MAX_CHARACTERS: usize = 32;
/// The label of the chip keeping the items craftable now.
pub const CRAFTABLE_CHIP: &str = "Craftable";
/// The side of the dot of a craftable item, its border included.
const DOT_SIZE: f32 = 8.0;
/// The room above and below the label of a chip; beside it, `SPACE_XS`, so that the chips take
/// three rows in four columns.
const CHIP_PADDING_Y: f32 = 2.0;
/// The width of the handle of the drawer.
const HANDLE_WIDTH: f32 = 34.0;
/// The room the title takes on the handle, turned a quarter: its width, upright.
const HANDLE_TITLE_LENGTH: f32 = 48.0;

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

/// The icon of the catalog slot of `item`, on its sprite of the items atlas.
fn icon(atlas: &AtlasWrapper, item: ItemId) -> ImageNode {
    ImageNode::from_atlas_image(
        atlas.texture.clone(),
        atlas.sprite(&format!("{item:?}")).unwrap_or_default(),
    )
}

/// Builds the item browser, hidden, in `HudBottom` beside the inventory panel.
pub fn setup_item_browser(
    mut commands: Commands,
    materials: Res<MaterialResource>,
    asset_server: Res<AssetServer>,
    state: Res<ItemBrowserState>,
    bottom: Single<Entity, With<HudBottom>>,
) {
    let Some(atlas) = materials.items.as_ref() else {
        return;
    };
    let font = load_font(&asset_server);
    let glyph_font: Handle<Font> = asset_server.load(MONO_FONT_PATH);

    let panel = commands
        .spawn((
            ItemBrowser,
            // the role before the label: the label's default node must not win
            AccessibilityNode(accesskit::Node::new(Role::Group)),
            AccessibleLabel::new(ITEM_BROWSER_TITLE),
            Node {
                position_type: PositionType::Absolute,
                top: px(SPACE_L),
                bottom: px(SPACE_L),
                right: px(SPACE_L),
                flex_direction: FlexDirection::Column,
                row_gap: px(SPACE_S),
                padding: UiRect::all(px(SPACE_S)),
                border: UiRect::all(px(BEVEL_WIDTH)),
                ..default()
            },
            BackgroundColor(PANEL),
            BorderColor {
                top: BEVEL_HI,
                left: BEVEL_HI,
                bottom: BEVEL_LO,
                right: BEVEL_LO,
            },
            // A click outside it closes the drawer
            Hovered::default(),
            // The arrows move between its chips and its slots as they lie
            ArrowNav,
            Visibility::Hidden,
            ChildOf(*bottom),
        ))
        .id();

    commands.spawn((
        Node {
            justify_content: JustifyContent::SpaceBetween,
            align_items: AlignItems::Baseline,
            column_gap: px(SPACE_S),
            ..default()
        },
        ChildOf(panel),
        children![
            (BrowserTitle, label(ITEM_BROWSER_TITLE, &font, FONT_L, TEXT)),
            (ItemCount, label("", &font, FONT_S, MUTED)),
        ],
    ));

    spawn_handle(&mut commands, *bottom, &font, &glyph_font);
    spawn_search_field(&mut commands, panel, &state, &font, &glyph_font);
    spawn_chips(&mut commands, panel, &font);
    spawn_bookmark_bar(&mut commands, panel, &font);
    spawn_grid(&mut commands, panel, atlas, &font);
    spawn_recipe_panel(&mut commands, panel, atlas, &font, &glyph_font);

    spawn_footer(&mut commands, panel, &font, &glyph_font);
}

/// The footer: in Survival the gestures of the catalog, where a click opens the recipes, and A,
/// which bookmarks; in
/// Creative the trash, beside what it is for.
fn spawn_footer(
    commands: &mut Commands,
    panel: Entity,
    font: &Handle<Font>,
    glyph_font: &Handle<Font>,
) {
    let footer = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                padding: UiRect::top(px(SPACE_S)),
                border: UiRect::top(px(BEVEL_WIDTH)),
                ..default()
            },
            BorderColor::all(BEVEL_LO),
            Pickable::IGNORE,
            ChildOf(panel),
        ))
        .id();
    let row = commands
        .spawn((
            ModeFooter(true),
            Node {
                flex_wrap: FlexWrap::Wrap,
                align_items: AlignItems::Center,
                column_gap: px(SPACE_XS),
                row_gap: px(SPACE_XS),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(footer),
        ))
        .id();
    for (index, (key, action)) in [("LMB", "recipes"), ("RMB", "uses"), ("A", "bookmark")]
        .into_iter()
        .enumerate()
    {
        if index > 0 {
            commands.spawn((
                Node {
                    width: px(SPACE_S),
                    ..default()
                },
                ChildOf(row),
            ));
        }
        commands.spawn((key_cap(key, font.clone(), glyph_font.clone()), ChildOf(row)));
        commands.spawn((label(action, font, FONT_S, MUTED), ChildOf(row)));
    }

    let creative = commands
        .spawn((
            ModeFooter(false),
            Node {
                display: Display::None,
                align_items: AlignItems::Center,
                column_gap: px(SPACE_S),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(footer),
        ))
        .id();
    commands
        .spawn((
            TrashSlot,
            slot(TRASH_LABEL, None, ImageNode::default(), font.clone()),
            ChildOf(creative),
        ))
        .with_child((
            label("X", font, FONT_L, DANGER),
            Node {
                position_type: PositionType::Absolute,
                ..default()
            },
        ));
    commands.spawn((label(TRASH_HINT, font, FONT_S, MUTED), ChildOf(creative)));
}

/// The handle of the drawer, at mid-height of the right edge: a `B` key cap over the title of the
/// browser, turned a quarter to read downwards. Shown by `show_item_browser`.
fn spawn_handle(
    commands: &mut Commands,
    bottom: Entity,
    font: &Handle<Font>,
    glyph_font: &Handle<Font>,
) {
    commands
        .spawn((
            DrawerHandle,
            // the widget before the label: its `Role::Button` must win over the label's node
            UiButton,
            AccessibleLabel::new(ITEM_BROWSER_TITLE),
            TabIndex(-1),
            FocusRing,
            Hovered::default(),
            Node {
                position_type: PositionType::Absolute,
                right: px(0),
                top: percent(50),
                width: px(HANDLE_WIDTH),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: px(SPACE_S),
                padding: UiRect::vertical(px(SPACE_S)),
                // against the edge of the window: no bevel on that side
                border: UiRect {
                    left: px(BEVEL_WIDTH),
                    right: px(0),
                    top: px(BEVEL_WIDTH),
                    bottom: px(BEVEL_WIDTH),
                },
                ..default()
            },
            UiTransform::from_translation(Val2::percent(0.0, -50.0)),
            BackgroundColor(PANEL),
            BorderColor {
                top: BEVEL_HI,
                left: BEVEL_HI,
                bottom: BEVEL_LO,
                right: BEVEL_LO,
            },
            Visibility::Hidden,
            ChildOf(bottom),
            children![
                key_cap("B", font.clone(), glyph_font.clone()),
                (
                    Node {
                        width: px(FONT_S),
                        height: px(HANDLE_TITLE_LENGTH),
                        ..default()
                    },
                    Pickable::IGNORE,
                    children![(
                        label(ITEM_BROWSER_TITLE, font, FONT_S, TEXT),
                        Node {
                            position_type: PositionType::Absolute,
                            left: percent(50),
                            top: percent(50),
                            ..default()
                        },
                        UiTransform {
                            translation: Val2::percent(-50.0, -50.0),
                            rotation: Rot2::degrees(90.0),
                            ..default()
                        },
                    )],
                ),
            ],
        ))
        .observe(open_drawer);
}

/// The search field, holding the search of the state, with a `/` key cap at its right end.
fn spawn_search_field(
    commands: &mut Commands,
    panel: Entity,
    state: &ItemBrowserState,
    font: &Handle<Font>,
    glyph_font: &Handle<Font>,
) {
    let field = spawn_text_field(
        commands,
        &TextField {
            label: SEARCH_LABEL,
            placeholder: SEARCH_LABEL,
            max_characters: Some(SEARCH_MAX_CHARACTERS),
            font: font.clone(),
            font_size: FONT_S,
            reserve_error: false,
        },
    );
    commands
        .entity(field.root)
        .insert((CatalogPart, ChildOf(panel)));
    // the cap at the right end: the placeholder sits at the left of the field
    commands
        .entity(field.frame)
        .entry::<Node>()
        .and_modify(|mut node| node.align_items = AlignItems::Center);
    commands.spawn((
        key_cap("/", font.clone(), glyph_font.clone()),
        ChildOf(field.frame),
    ));
    let mut input = commands.entity(field.input);
    input
        .insert(ItemSearchField)
        .observe(submit_search)
        .observe(search_arrow_down)
        .observe(clear_or_leave_search);
    if !state.query.is_empty() {
        let query = state.query.clone();
        input
            .entry::<EditableText>()
            .and_modify(move |mut text| set_text(&mut text, &query));
    }
}

/// The label of the bookmarks, and of their row.
pub const BOOKMARKS_LABEL: &str = "Bookmarks";

/// The category chips, All first, then Craftable: they wrap, so that every one is in view.
fn spawn_chips(commands: &mut Commands, panel: Entity, font: &Handle<Font>) {
    let row = commands
        .spawn((
            AccessibilityNode(accesskit::Node::new(Role::Toolbar)),
            AccessibleLabel::new("Categories"),
            CatalogPart,
            Node {
                flex_shrink: 0.0,
                flex_wrap: FlexWrap::Wrap,
                column_gap: px(SPACE_XS),
                row_gap: px(SPACE_XS),
                ..default()
            },
            ChildOf(panel),
        ))
        .id();
    let chips = std::iter::once(None).chain(ItemCategory::ALL.map(Some));
    for category in chips {
        let name = category.map_or("All", ItemCategory::name);
        commands
            .spawn((
                // the widget before the label: its `Role::Button` must win over the label's node
                UiButton,
                AccessibleLabel::new(name),
                CategoryChip(category),
                TabIndex(0),
                FocusRing,
                Hovered::default(),
                Node {
                    flex_shrink: 0.0,
                    padding: UiRect::axes(px(SPACE_XS), px(CHIP_PADDING_Y)),
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
                ChildOf(row),
                children![label(name, font, FONT_S, MUTED)],
            ))
            .observe(choose_category);
    }
    commands
        .spawn((
            // the widget before the label: its `Role::Button` must win over the label's node
            UiButton,
            AccessibleLabel::new(CRAFTABLE_CHIP),
            CraftableChip,
            TabIndex(0),
            FocusRing,
            Hovered::default(),
            Node {
                flex_shrink: 0.0,
                padding: UiRect::axes(px(SPACE_XS), px(CHIP_PADDING_Y)),
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
            ChildOf(row),
            children![label(CRAFTABLE_CHIP, font, FONT_S, MUTED)],
        ))
        .observe(toggle_craftable);
}

/// The slot of `item` in the catalog, in `parent`: the grid or the row of bookmarks. Its tooltip
/// and the dot of an item craftable now are kept up to date by `mark_craftable_items`.
pub(super) fn spawn_catalog_slot(
    commands: &mut Commands,
    parent: Entity,
    item: ItemId,
    atlas: &AtlasWrapper,
    font: &Handle<Font>,
) -> Entity {
    let name = item_name(item);
    commands
        .spawn((
            CatalogSlot(item),
            Tooltip {
                title: name.clone(),
                detail: category_detail(item),
                note: String::new(),
                hint: item_keys_hint(false),
            },
            slot(&name, None, icon(atlas, item), font.clone()),
            ChildOf(parent),
        ))
        // after the icon and the count: drawn over them
        .with_child((
            CraftableDot,
            Node {
                display: Display::None,
                position_type: PositionType::Absolute,
                top: px(1),
                right: px(1),
                width: px(DOT_SIZE),
                height: px(DOT_SIZE),
                border: UiRect::all(px(1)),
                ..default()
            },
            BackgroundColor(OK),
            BorderColor::all(BEVEL_LO),
            Pickable::IGNORE,
        ))
        .id()
}

/// The bookmarks, hidden until there is one: a label over a row of slots, which wraps as the
/// chips do. `show_bookmarks` fills the row.
fn spawn_bookmark_bar(commands: &mut Commands, panel: Entity, font: &Handle<Font>) {
    commands.spawn((
        BookmarkBar,
        Node {
            display: Display::None,
            flex_shrink: 0.0,
            flex_direction: FlexDirection::Column,
            row_gap: px(SPACE_XS),
            ..default()
        },
        ChildOf(panel),
        children![
            label(BOOKMARKS_LABEL, font, FONT_S, MUTED),
            (
                BookmarkRow,
                // the role before the label: the label's default node must not win
                AccessibilityNode(accesskit::Node::new(Role::Grid)),
                AccessibleLabel::new(BOOKMARKS_LABEL),
                Node {
                    flex_wrap: FlexWrap::Wrap,
                    column_gap: px(SPACE_XS),
                    row_gap: px(SPACE_XS),
                    // under the grid's columns, which leave room for the focus ring
                    padding: UiRect::horizontal(px(RING_ROOM)),
                    ..default()
                },
            ),
        ],
    ));
}

/// The scrolling grid of the catalog, a slot per item, and what it says when nothing matches.
fn spawn_grid(commands: &mut Commands, panel: Entity, atlas: &AtlasWrapper, font: &Handle<Font>) {
    let ScrollListParts { frame, well, list } =
        spawn_scroll_list(commands, WheelStep::Px(SLOT_SIZE + SPACE_XS));
    commands
        .entity(frame)
        .insert((CatalogPart, CatalogList, Hovered::default(), ChildOf(panel)));
    // the slots sit on the panel, as in the inventory: no well around them
    commands
        .entity(well)
        .insert((BackgroundColor(Color::NONE), BorderColor::all(Color::NONE)));
    commands
        .entity(well)
        .entry::<Node>()
        .and_modify(|mut node| node.border = UiRect::ZERO);
    let grid = commands
        .spawn((
            CatalogGrid,
            AccessibilityNode(accesskit::Node::new(Role::Grid)),
            AccessibleLabel::new(ITEM_BROWSER_TITLE),
            Node {
                display: Display::Grid,
                grid_template_columns: RepeatedGridTrack::px(1, SLOT_SIZE),
                row_gap: px(SPACE_XS),
                column_gap: px(SPACE_XS),
                padding: UiRect::vertical(px(RING_ROOM)),
                ..default()
            },
            ChildOf(list),
        ))
        .id();
    for item in catalog() {
        spawn_catalog_slot(commands, grid, item, atlas, font);
    }
    commands.spawn((
        NoMatch,
        Node {
            display: Display::None,
            flex_direction: FlexDirection::Column,
            row_gap: px(SPACE_XS),
            padding: UiRect::vertical(px(SPACE_S)),
            ..default()
        },
        ChildOf(list),
        children![
            (NoMatchMessage, label("", font, FONT_S, TEXT)),
            (NoMatchHint, label(NO_MATCH_HINT, font, FONT_S, MUTED)),
        ],
    ));
}
