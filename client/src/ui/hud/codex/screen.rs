//! The screen of the Codex: its sections at the left, the list of the pages of the section with
//! its search, and the page chosen, under a bar with the back button and the link to the wiki.

use accesskit::Role;
use bevy::{
    a11y::AccessibilityNode,
    ecs::entity::Entities,
    input_focus::{tab_navigation::TabGroup, AutoFocus, FocusCause, InputFocus, InputFocusVisible},
    prelude::*,
    text::EditableText,
    ui::{InteractionDisabled, Selected},
    ui_widgets::{observe, Activate},
};

use super::pages::{entries, entry_name, matches, spawn_page, PageContext};
use super::text::{open_on_the_web, LinkHost};
use super::{CodexPage, CodexState, Section, CODEX_TITLE, WIKI_URL};
use crate::i18n::{tr, trf};
use crate::input::data::GameAction;
use crate::input::keyboard::is_action_just_pressed;
use crate::state::KeyMap;
use crate::ui::assets::{FONT_PATH, MONO_FONT_PATH};
use crate::ui::hud::progress::PlayerProgress;
use crate::ui::hud::{InventoryRoot, UIMode, UiDialog};
use crate::ui::theme;
use crate::ui::widgets::button::{button, ButtonVariant};
use crate::ui::widgets::focus::keep_focus_inside;
use crate::ui::widgets::hints::{hint_bar, Hint, HINT_BAR_HEIGHT};
use crate::ui::widgets::list::{
    spawn_list_card, spawn_scroll_list, spawn_scroll_page, CardSelected, WheelStep,
};
use crate::ui::widgets::raised_bevel;
use crate::ui::widgets::text_field::{set_text, spawn_text_field, TextField};
use crate::world::MaterialResource;

const HINTS: [Hint; 5] = [
    Hint {
        keys: "Tab",
        action: "Move",
    },
    Hint {
        keys: "↑↓",
        action: "Choose",
    },
    Hint {
        keys: "Click",
        action: "Follow a link",
    },
    Hint {
        keys: "Bksp",
        action: "Back",
    },
    Hint {
        keys: "F1 Esc",
        action: "Close",
    },
];

const PANEL_WIDTH: f32 = 1120.0;
const PANEL_HEIGHT: f32 = 760.0;
const NAV_WIDTH: f32 = 170.0;
const LIST_WIDTH: f32 = 260.0;

/// The root of the screen.
#[derive(Component)]
pub struct CodexRoot {
    /// The button that opened it, which gets the focus back.
    opener: Option<Entity>,
}

/// A button of a section, at the left.
#[derive(Component)]
pub struct SectionButton(pub Section);

/// The column of the buttons of the sections.
#[derive(Component)]
pub struct SectionNav;

/// The input of the search.
#[derive(Component)]
pub struct CodexSearchField;

/// The list of the pages of the section.
#[derive(Component)]
pub struct CodexList;

/// A card of the list, and its page.
#[derive(Component)]
pub struct CodexEntry(pub CodexPage);

/// The column the page is drawn in.
#[derive(Component)]
pub struct CodexPageBody;

/// The back button.
#[derive(Component)]
pub struct BackButton;

/// The button opening the page on the wiki.
#[derive(Component)]
pub struct WikiButton;

/// A request to close the screen, on its root, applied in `PostUpdate`.
#[derive(Component)]
pub struct CloseCodex;

/// Opens the Codex, on `page` if there is one; from the button `opener`, which gets the focus
/// back when it closes. The inventory closes under it.
pub struct OpenCodex {
    pub page: Option<CodexPage>,
    pub opener: Option<Entity>,
}

impl Command for OpenCodex {
    type Out = ();

    fn apply(self, world: &mut World) {
        if let Some(page) = self.page {
            world.resource_mut::<CodexState>().open(page);
        }
        let open = world
            .query_filtered::<(), With<CodexRoot>>()
            .iter(world)
            .next()
            .is_some();
        if !open {
            let mut inventories = world.query_filtered::<&mut Visibility, With<InventoryRoot>>();
            for mut visibility in inventories.iter_mut(world) {
                *visibility = Visibility::Hidden;
            }
            let assets = world.resource::<AssetServer>().clone();
            let state = world.resource::<CodexState>().clone();
            spawn_codex(&mut world.commands(), &assets, &state, self.opener);
            world.flush();
        }
    }
}

/// Whether the Codex is open.
pub fn codex_open(roots: Query<(), With<CodexRoot>>) -> bool {
    !roots.is_empty()
}

/// Opens the Codex on its key during gameplay, or with the inventory open, and closes it on its
/// key or on Escape.
pub fn toggle_codex(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    key_map: Res<KeyMap>,
    ui_mode: Res<UIMode>,
    roots: Query<Entity, With<CodexRoot>>,
    pause: Query<(), With<crate::ui::menus::pause::PauseMenu>>,
    mut commands: Commands,
) {
    let key = is_action_just_pressed(GameAction::OpenCodex, &keyboard_input, &key_map);
    if let Ok(root) = roots.single() {
        let escape = is_action_just_pressed(GameAction::Escape, &keyboard_input, &key_map);
        if (key && *ui_mode != UIMode::Typing) || escape {
            commands.entity(root).insert(CloseCodex);
        }
    } else if key && *ui_mode != UIMode::Typing && pause.is_empty() {
        commands.queue(OpenCodex {
            page: None,
            opener: None,
        });
    }
}

fn text(content: impl Into<String>, size: f32, color: Color, font: &Handle<Font>) -> impl Bundle {
    (
        Text::new(content),
        TextFont {
            font: font.clone().into(),
            font_size: px(size).into(),
            ..default()
        },
        TextColor(color),
        Pickable::IGNORE,
    )
}

fn spawn_codex(
    commands: &mut Commands,
    assets: &AssetServer,
    state: &CodexState,
    opener: Option<Entity>,
) {
    let font: Handle<Font> = assets.load(FONT_PATH);
    let root = commands
        .spawn((
            CodexRoot { opener },
            Name::new("Codex"),
            UiDialog,
            AccessibilityNode(accesskit::Node::new(Role::Dialog)),
            AccessibleLabel::new(tr(CODEX_TITLE)),
            Node {
                position_type: PositionType::Absolute,
                width: percent(100),
                height: percent(100),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                padding: UiRect {
                    top: px(theme::space_l()),
                    bottom: px(HINT_BAR_HEIGHT + theme::space_l()),
                    left: px(theme::space_l()),
                    right: px(theme::space_l()),
                },
                ..default()
            },
            BackgroundColor(theme::bevel_lo().with_alpha(0.75)),
            Visibility::Visible,
            // over the pause menu (5)
            GlobalZIndex(6),
            TabGroup::modal(),
            DespawnOnExit(crate::state::GameState::Game),
        ))
        .id();
    let panel = commands
        .spawn((
            Node {
                width: px(PANEL_WIDTH),
                max_width: percent(100),
                height: px(PANEL_HEIGHT),
                max_height: percent(100),
                flex_direction: FlexDirection::Column,
                row_gap: px(theme::space_m()),
                padding: UiRect::all(px(theme::space_l())),
                border: UiRect::all(px(theme::bevel_width())),
                ..default()
            },
            BackgroundColor(theme::panel()),
            raised_bevel(),
            ChildOf(root),
        ))
        .id();

    // the bar: the title, then the back, the wiki and the close buttons
    let bar = commands
        .spawn((
            Node {
                align_items: AlignItems::Center,
                column_gap: px(theme::space_s()),
                ..default()
            },
            ChildOf(panel),
        ))
        .id();
    commands.spawn((
        text(tr(CODEX_TITLE), theme::font_xl(), theme::accent(), &font),
        Node {
            flex_grow: 1.0,
            ..default()
        },
        ChildOf(bar),
    ));
    commands.spawn((
        BackButton,
        button(tr("codex.back"), ButtonVariant::Default, font.clone()),
        observe(|_: On<Activate>, mut state: ResMut<CodexState>| {
            state.back();
        }),
        ChildOf(bar),
    ));
    commands.spawn((
        WikiButton,
        button(tr("codex.wiki"), ButtonVariant::Default, font.clone()),
        observe(|_: On<Activate>, state: Res<CodexState>| {
            let path = state
                .page
                .as_ref()
                .map(CodexPage::wiki_path)
                .unwrap_or_default();
            open_on_the_web(&format!("{WIKI_URL}{path}"));
        }),
        ChildOf(bar),
    ));
    commands.spawn((
        button(tr("codex.close"), ButtonVariant::Default, font.clone()),
        observe(move |_: On<Activate>, mut commands: Commands| {
            commands.entity(root).insert(CloseCodex);
        }),
        ChildOf(bar),
    ));

    let body = commands
        .spawn((
            Node {
                flex_grow: 1.0,
                min_height: px(0),
                column_gap: px(theme::space_l()),
                ..default()
            },
            ChildOf(panel),
        ))
        .id();
    commands.spawn((
        SectionNav,
        AccessibilityNode(accesskit::Node::new(Role::TabList)),
        AccessibleLabel::new(tr(CODEX_TITLE)),
        Node {
            width: px(NAV_WIDTH),
            flex_shrink: 0.0,
            flex_direction: FlexDirection::Column,
            row_gap: px(theme::space_s()),
            ..default()
        },
        ChildOf(body),
    ));

    let middle = commands
        .spawn((
            Node {
                width: px(LIST_WIDTH),
                flex_shrink: 0.0,
                flex_direction: FlexDirection::Column,
                row_gap: px(theme::space_s()),
                ..default()
            },
            ChildOf(body),
        ))
        .id();
    let field = spawn_text_field(
        commands,
        &TextField {
            label: tr("codex.search"),
            placeholder: tr("codex.search"),
            max_characters: Some(40),
            font: font.clone(),
            font_size: theme::font_s(),
            reserve_error: false,
        },
    );
    commands.entity(field.root).insert(ChildOf(middle));
    commands
        .entity(field.input)
        .insert((CodexSearchField, AutoFocus));
    let query = state.query.clone();
    if !query.is_empty() {
        commands
            .entity(field.input)
            .queue(move |mut entity: EntityWorldMut| {
                if let Some(mut text) = entity.get_mut::<EditableText>() {
                    set_text(&mut text, &query);
                }
            });
    }
    let list = spawn_scroll_list(commands, WheelStep::Item);
    commands.entity(list.frame).insert(ChildOf(middle));
    commands
        .entity(list.list)
        .insert((
            CodexList,
            AccessibilityNode(accesskit::Node::new(Role::List)),
            AccessibleLabel::new(tr(CODEX_TITLE)),
        ))
        .observe(select_entry);

    let (page_frame, page_column) = spawn_scroll_page(commands, theme::space_s());
    commands.entity(page_frame).insert((
        Node {
            flex_grow: 1.0,
            min_width: px(0),
            min_height: px(0),
            ..default()
        },
        ChildOf(body),
    ));
    commands
        .entity(page_column)
        .insert((CodexPageBody, LinkHost::Codex));

    commands.spawn((
        hint_bar(&HINTS, font, assets.load(MONO_FONT_PATH)),
        ChildOf(root),
    ));
    keep_focus_inside(commands, root, Some(field.input));
}

/// A card of the list chosen shows its page, without going on the history: the history is of
/// the links followed.
fn select_entry(
    selected: On<CardSelected>,
    entries: Query<&CodexEntry>,
    mut state: ResMut<CodexState>,
) {
    if let Ok(entry) = entries.get(selected.card) {
        if state.page.as_ref() != Some(&entry.0) {
            state.page = Some(entry.0.clone());
        }
    }
}

/// Keeps the search of the state up to date with its field.
pub fn read_codex_search(
    fields: Query<&EditableText, (With<CodexSearchField>, Changed<EditableText>)>,
    mut state: ResMut<CodexState>,
) {
    for text in &fields {
        let query = text.value().to_string();
        if state.query != query {
            state.query = query;
        }
    }
}

/// Whether the player met a creature, for the bestiary: all are shown until the server told
/// the progress.
fn seen_by(progress: &PlayerProgress) -> impl Fn(shared::world::MobKind) -> bool + '_ {
    move |kind| progress.0.as_ref().is_none_or(|p| p.has_seen(kind))
}

/// How many creatures the player met, which redraw the list and the page when it grows.
fn seen_count(progress: &PlayerProgress) -> usize {
    progress.0.as_ref().map_or(usize::MAX, |p| p.seen.len())
}

/// What the list was last built for.
#[derive(Default, PartialEq)]
pub struct ListBuilt {
    section: Option<Section>,
    query: String,
    seen: usize,
}

/// Builds the buttons of the sections and the list of the section again when the section, the
/// search or the creatures met change.
#[allow(clippy::too_many_arguments)]
pub fn show_codex_list(
    state: Res<CodexState>,
    progress: Res<PlayerProgress>,
    nav: Single<Entity, With<SectionNav>>,
    list: Single<Entity, With<CodexList>>,
    assets: Res<AssetServer>,
    mut built: Local<ListBuilt>,
    mut fields: Query<&mut EditableText, With<CodexSearchField>>,
    mut commands: Commands,
) {
    let seen_count = seen_count(&progress);
    let now = ListBuilt {
        section: Some(state.section),
        query: state.query.clone(),
        seen: seen_count,
    };
    if *built == now {
        return;
    }
    let section_changed = built.section != now.section;
    *built = now;
    let font: Handle<Font> = assets.load(FONT_PATH);
    // A section chosen anew starts unfiltered: its field too
    if state.query.is_empty() {
        for mut text in &mut fields {
            if !text.value().to_string().is_empty() {
                set_text(&mut text, "");
            }
        }
    }
    if section_changed {
        commands.entity(*nav).despawn_related::<Children>();
        for section in Section::ALL {
            let variant = if section == state.section {
                ButtonVariant::Primary
            } else {
                ButtonVariant::Ghost
            };
            let button_entity = commands
                .spawn((
                    SectionButton(section),
                    button(section.label(), variant, font.clone()),
                    observe(choose_section),
                    ChildOf(*nav),
                ))
                .id();
            // the theme's padding, but aligned left
            commands
                .entity(button_entity)
                .entry::<Node>()
                .and_modify(|mut node| node.justify_content = JustifyContent::FlexStart);
        }
    }
    commands.entity(*list).despawn_related::<Children>();
    let seen = seen_by(&progress);
    let shown: Vec<CodexPage> = entries(state.section)
        .into_iter()
        .filter(|page| matches(&state.query, page))
        // a creature not met is found by no search
        .filter(|page| state.query.is_empty() || entry_name(page, &seen) == page.title())
        .collect();
    if shown.is_empty() {
        let message = if state.query.is_empty() {
            tr("codex.empty").to_string()
        } else {
            trf("codex.no_match", &[("query", &state.query)])
        };
        commands.spawn((
            text(message, theme::font_s(), theme::muted(), &font),
            Node {
                margin: UiRect::all(px(theme::space_m())),
                ..default()
            },
            ChildOf(*list),
        ));
        return;
    }
    for page in shown {
        let name = entry_name(&page, &seen);
        let card = spawn_list_card(&mut commands, *list, &name);
        commands.entity(card).insert(CodexEntry(page.clone()));
        if state.page.as_ref() == Some(&page) {
            commands.entity(card).insert(Selected);
        }
        commands.entity(card).insert(Node {
            border: UiRect::all(px(theme::bevel_width())),
            padding: UiRect::axes(px(theme::space_m()), px(theme::space_s())),
            align_items: AlignItems::Center,
            ..default()
        });
        commands.spawn((
            text(name, theme::font_s(), theme::text(), &font),
            ChildOf(card),
        ));
    }
}

/// A button of a section chosen shows its list.
fn choose_section(
    activate: On<Activate>,
    buttons: Query<&SectionButton>,
    mut state: ResMut<CodexState>,
) {
    if let Ok(button) = buttons.get(activate.entity) {
        if state.section != button.0 {
            state.section = button.0;
            state.query.clear();
        }
    }
}

/// Draws the page chosen again when it changes, and enables the back and the wiki buttons.
#[allow(clippy::too_many_arguments)]
pub fn show_codex_page(
    state: Res<CodexState>,
    progress: Res<PlayerProgress>,
    body: Single<(Entity, &ChildOf), With<CodexPageBody>>,
    back: Single<Entity, With<BackButton>>,
    wiki: Single<Entity, With<WikiButton>>,
    materials: Res<MaterialResource>,
    assets: Res<AssetServer>,
    mut scrolls: Query<&mut ScrollPosition>,
    mut built: Local<Option<(Option<CodexPage>, usize)>>,
    mut commands: Commands,
) {
    let seen_count = seen_count(&progress);
    let now = (state.page.clone(), seen_count);
    if built.as_ref() == Some(&now) {
        return;
    }
    *built = Some(now);
    let enable = |commands: &mut Commands, entity: Entity, on: bool| {
        if on {
            commands.entity(entity).remove::<InteractionDisabled>();
        } else {
            commands.entity(entity).insert(InteractionDisabled);
        }
    };
    enable(&mut commands, *back, !state.history.is_empty());
    enable(&mut commands, *wiki, state.page.is_some());
    let (body, frame) = *body;
    commands.entity(body).despawn_related::<Children>();
    if let Ok(mut scroll) = scrolls.get_mut(frame.parent()) {
        *scroll = ScrollPosition::default();
    }
    if let Ok(mut scroll) = scrolls.get_mut(body) {
        *scroll = ScrollPosition::default();
    }
    let font: Handle<Font> = assets.load(FONT_PATH);
    let Some(page) = &state.page else {
        commands.spawn((
            text(tr("codex.pick"), theme::font_s(), theme::muted(), &font),
            ChildOf(body),
        ));
        return;
    };
    let seen = seen_by(&progress);
    let ctx = PageContext {
        atlas: materials.items.as_ref(),
        font,
        assets: &assets,
        seen: &seen,
    };
    spawn_page(&mut commands, body, page, true, &ctx);
}

/// Backspace goes back through the history, while the search does not take it.
pub fn codex_keys(
    keys: Res<ButtonInput<KeyCode>>,
    ui_mode: Res<UIMode>,
    mut state: ResMut<CodexState>,
) {
    if *ui_mode != UIMode::Typing && keys.just_pressed(KeyCode::Backspace) {
        state.back();
    }
}

/// Closes the screen, the focus back on the button that opened it, with its ring when a key
/// closed it.
pub fn close_codex(
    roots: Query<(Entity, &CodexRoot), With<CloseCodex>>,
    entities: &Entities,
    keys: Res<ButtonInput<KeyCode>>,
    mut focus: ResMut<InputFocus>,
    mut visible: ResMut<InputFocusVisible>,
    mut commands: Commands,
) {
    for (root, screen) in &roots {
        commands.entity(root).despawn();
        match screen.opener {
            Some(opener) if entities.contains(opener) => {
                focus.set(opener, FocusCause::Navigated);
                visible.0 |= keys.get_just_pressed().next().is_some();
            }
            _ => focus.clear(),
        }
    }
}
