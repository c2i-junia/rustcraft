//! The Controls tab of the settings: the actions by category, each with a primary and a secondary
//! key slot. Enter, Space or a click on a slot waits for the key to bind to it, which Escape
//! cancels; Delete, Backspace or a right click clears it. A key bound to two actions is shown on
//! both rows, never refused, and "Reset to defaults", beside Back, brings back the default keys
//! once the player confirms. Every change is saved at once.
//!
//! The search field over the list, which Ctrl+F focuses, keeps the actions whose name, category
//! or keys hold every word typed. Down or Enter goes to the first of them, Escape clears the
//! field, or leaves the settings once it is empty.

use std::ops::Range;

use accesskit::Role;
use bevy::{
    a11y::AccessibilityNode,
    input::{
        keyboard::{KeyboardFocusLost, KeyboardInput},
        ButtonState,
    },
    input_focus::{
        tab_navigation::TabIndex, FocusCause, FocusGained, FocusLost, FocusedInput, InputFocus,
        InputFocusVisible,
    },
    picking::hover::Hovered,
    prelude::*,
    text::EditableText,
    ui::Pressed,
    ui_widgets::{observe, Activate, ScrollArea, ScrollIntoView, Scrollbar},
    window::WindowFocused,
};
use shared::GameFolderPaths;

use crate::input::data::{ActionCategory, GameAction};
use crate::input::keyboard::{
    clear_key, conflicts, default_bindings, set_key, write_keybindings, KeyUse, MAX_KEYS_PER_ACTION,
};
use crate::state::KeyMap;
use crate::ui::assets::{FONT_PATH, MONO_FONT_PATH};
use crate::ui::theme::{
    ACCENT, BEVEL_WIDTH, DANGER, FONT_M, FONT_S, MUTED, RAISE, RAISE_HOVER, SPACE_M, SPACE_S,
    SPACE_XS, TEXT,
};
use crate::ui::widgets::button::{button, ButtonVariant};
use crate::ui::widgets::dialog::{spawn_dialog, Dialog, DialogClosed};
use crate::ui::widgets::focus::{ArrowNav, FocusRing};
use crate::ui::widgets::hints::{key_cap, Hint, HintBar};
use crate::ui::widgets::list::{
    spawn_scroll_list, ScrollListParts, WheelStep, RING_ROOM, SCROLLBAR_WIDTH,
};
use crate::ui::widgets::text_field::{
    set_text, spawn_text_field, Cancel as FieldCancel, Submit, TextField,
};
use crate::ui::widgets::{raised_bevel, sunken_bevel};

use super::{spawn_hints, CloseSettings, SettingsRoot};

/// The slot `slot` of the keys of `action`: 0 for the primary key, 1 for the secondary one.
#[derive(Component, Clone, Copy, PartialEq, Eq, Debug)]
pub struct KeySlot {
    pub action: GameAction,
    pub slot: usize,
}

/// The message under the row of an action naming the keys it shares with other actions, hidden
/// while it shares none.
#[derive(Component)]
pub struct ConflictMessage(pub GameAction);

/// Marks a key slot holding a key that another action has too.
#[derive(Component)]
struct Conflicting;

/// Marks a key slot holding a key that the search names.
#[derive(Component)]
struct SearchHit;

/// The row of an action, hidden while the search does not find it.
#[derive(Component)]
struct ActionRow(GameAction);

/// The name of an action on its row, the parts the search found lit up.
#[derive(Component)]
struct ActionName(GameAction);

/// The label of a category and its first row, hidden while the search finds none of its actions.
#[derive(Component)]
struct CategoryGroup(ActionCategory);

/// The headers of the key columns, over the slots of the list in `frame`.
#[derive(Component)]
struct ColumnHeaders {
    frame: Entity,
}

/// What the list shows while the search finds nothing.
#[derive(Component)]
struct NoMatch;

/// How many actions the search finds, beside the reset button.
#[derive(Component)]
struct MatchCount;

/// The search field of the Controls tab.
#[derive(Component)]
pub struct KeySearchField;

/// What the search field holds. It outlives the tab, so that the search is still there when the
/// player comes back to the tab, and the settings start with an empty one.
#[derive(Resource, Default)]
pub struct KeySearch {
    pub query: String,
}

/// The button bringing back the default keys of every action, after a confirmation.
#[derive(Component)]
pub struct ResetBindings;

/// The key slot waiting for a key to bind, if any.
#[derive(Resource, Default)]
pub struct KeyCapture {
    slot: Option<Entity>,
    /// The modifier pressed on the slot, bound when it is released if nothing else came first.
    modifier: Option<KeyCode>,
    /// Set by the key that binds or cancels: the capture still counts as active until it ends in
    /// `PostUpdate`, so that the Escape handlers of `Update` ignore that key.
    ending: bool,
}

impl KeyCapture {
    /// A capture starting on `slot`.
    fn on(slot: Entity) -> Self {
        KeyCapture {
            slot: Some(slot),
            ..default()
        }
    }

    /// Whether a slot waits for a key: the Escape handlers stand down meanwhile.
    pub fn is_active(&self) -> bool {
        self.slot.is_some()
    }
}

/// What a slot waiting for a key shows.
pub const PRESS_A_KEY: &str = "Press a key...";

/// The key hints of the Controls tab.
pub const CONTROLS_HINTS: [Hint; 5] = [
    Hint {
        keys: "↑↓ ←→",
        action: "Move",
    },
    Hint {
        keys: "Enter",
        action: "Change key",
    },
    Hint {
        keys: "Del",
        action: "Clear",
    },
    Hint {
        keys: "Ctrl+F",
        action: "Search",
    },
    Hint {
        keys: "Esc",
        action: "Back",
    },
];

/// The key hints while the search field has the focus and holds a search.
pub const SEARCH_HINTS: [Hint; 2] = [
    Hint {
        keys: "↓",
        action: "Results",
    },
    Hint {
        keys: "Esc",
        action: "Clear",
    },
];

/// The key hints while the search field has the focus and is empty.
pub const EMPTY_SEARCH_HINTS: [Hint; 2] = [
    Hint {
        keys: "↓",
        action: "Results",
    },
    Hint {
        keys: "Esc",
        action: "Back",
    },
];

/// What the search field says while it is empty, and its accessible label.
pub const SEARCH_LABEL: &str = "Search actions or keys";

/// The key hints while a slot waits for a key.
pub const CAPTURE_HINTS: [Hint; 2] = [
    Hint {
        keys: "Any key",
        action: "Bind",
    },
    Hint {
        keys: "Esc",
        action: "Cancel",
    },
];

/// The hints the hint bar of the settings shows on the Controls tab; a bar without it shows
/// `CONTROLS_HINTS`.
#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum ControlsHints {
    Browsing,
    Capturing,
    Searching { empty: bool },
}

/// The width of a key slot, and of the header of its column.
const SLOT_WIDTH: f32 = 120.0;
/// The height of a key slot: a key cap and the bevel around it.
const SLOT_HEIGHT: f32 = 34.0;

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<KeyCapture>()
        .init_resource::<KeySearch>()
        .add_systems(
            Update,
            (
                style_key_slots,
                swap_hints,
                align_headers,
                show_keys
                    .run_if(resource_changed::<KeyMap>.or_else(resource_changed::<KeyCapture>)),
                show_conflicts.run_if(
                    resource_changed::<KeyMap>.or_else(any_match_filter::<Added<ConflictMessage>>),
                ),
                (
                    read_search,
                    // the rows found stay until the search changes: a row does not vanish under
                    // the player who rebinds its key
                    filter_rows.run_if(
                        resource_changed::<KeySearch>.or_else(any_match_filter::<Added<ActionRow>>),
                    ),
                    light_up_matches.run_if(
                        resource_changed::<KeySearch>
                            .or_else(resource_changed::<KeyMap>)
                            .or_else(any_match_filter::<Added<ActionRow>>),
                    ),
                )
                    .chain(),
            )
                .run_if(any_with_component::<KeySlot>),
        )
        .add_systems(PostUpdate, end_capture);
}

/// How a key is written on the screen: `KeyW` is `W`, `ShiftLeft` is `Left Shift`,
/// `NumpadSubtract` is `Num -`, the arrow keys are arrows.
fn key_label(key: KeyCode) -> String {
    let named = match key {
        KeyCode::Space => "Space",
        KeyCode::Enter => "Enter",
        KeyCode::Tab => "Tab",
        KeyCode::Backspace => "Backspace",
        KeyCode::ShiftLeft => "Left Shift",
        KeyCode::ShiftRight => "Right Shift",
        KeyCode::ControlLeft => "Left Ctrl",
        KeyCode::ControlRight => "Right Ctrl",
        KeyCode::AltLeft => "Left Alt",
        KeyCode::AltRight => "Right Alt",
        KeyCode::SuperLeft | KeyCode::SuperRight => "Super",
        KeyCode::ArrowUp => "↑",
        KeyCode::ArrowDown => "↓",
        KeyCode::ArrowLeft => "←",
        KeyCode::ArrowRight => "→",
        _ => {
            let name = format!("{key:?}");
            if let Some(digit) = name.strip_prefix("Digit") {
                return digit.to_string();
            }
            if let Some(pad) = name.strip_prefix("Numpad") {
                let short = match pad {
                    "Subtract" => "-",
                    "Add" => "+",
                    "Multiply" => "*",
                    "Divide" => "/",
                    "Decimal" => ".",
                    "Comma" => ",",
                    "Equal" => "=",
                    other => other,
                };
                return format!("Num {short}");
            }
            return name.replace("Key", "");
        }
    };
    named.to_string()
}

/// How a key is named in a text: as on its key cap, but the arrows, which only key caps can draw,
/// are written in words.
fn key_name(key: KeyCode) -> String {
    match key {
        KeyCode::ArrowUp => "Up arrow".to_string(),
        KeyCode::ArrowDown => "Down arrow".to_string(),
        KeyCode::ArrowLeft => "Left arrow".to_string(),
        KeyCode::ArrowRight => "Right arrow".to_string(),
        _ => key_label(key),
    }
}

/// What the row of an action sharing the keys `shared` with other actions or the hotbar says:
/// one sentence per key, in the order the keys first come, naming the other actions of that key
/// ("W is also bound to Move forward and Jump"), and one per hotbar digit ("1 also selects
/// hotbar slot 1").
fn conflict_message(shared: &[(KeyCode, KeyUse)]) -> String {
    // the other actions of a key, or the hotbar slot of a digit
    let mut sentences: Vec<(KeyCode, Vec<&str>, Option<usize>)> = Vec::new();
    for &(key, other) in shared {
        match other {
            KeyUse::Action(other) => {
                match sentences
                    .iter_mut()
                    .find(|(k, _, slot)| *k == key && slot.is_none())
                {
                    Some((_, names, _)) => names.push(other.name()),
                    None => sentences.push((key, vec![other.name()], None)),
                }
            }
            KeyUse::HotbarSlot(slot) => sentences.push((key, Vec::new(), Some(slot))),
        }
    }
    sentences
        .iter()
        .map(|(key, names, slot)| match slot {
            Some(slot) => format!("{} also selects hotbar slot {}", key_name(*key), slot + 1),
            None => format!("{} is also bound to {}", key_name(*key), and_list(names)),
        })
        .collect::<Vec<_>>()
        .join(" · ")
}

/// "A", "A and B", "A, B and C".
fn and_list(names: &[&str]) -> String {
    match names.split_last() {
        Some((last, [])) => last.to_string(),
        Some((last, rest)) => format!("{} and {last}", rest.join(", ")),
        None => String::new(),
    }
}

/// What the search finds in the row of an action: the parts of its name to light up, as byte
/// ranges, and the slots holding a key it names.
#[derive(Debug, Default, PartialEq)]
struct RowMatch {
    name: Vec<Range<usize>>,
    slots: Vec<usize>,
}

/// What the search `query` finds in the row of `action`, bound to `keys`, if it finds every
/// word of the query there, regardless of case: in its name or its category, or at the start of
/// a word of the name of one of its keys ("shi" finds Left Shift). An empty query finds every row.
fn search_row(query: &str, action: GameAction, keys: &[KeyCode]) -> Option<RowMatch> {
    // the names are ASCII: the ranges in their lower case are ranges in them
    let name = action.name().to_lowercase();
    let category = action.category().label().to_lowercase();
    let key_words: Vec<(usize, String)> = keys
        .iter()
        .enumerate()
        .flat_map(|(slot, &key)| {
            let words = key_name(key).to_lowercase();
            words
                .split_whitespace()
                .map(|word| (slot, word.to_string()))
                .collect::<Vec<_>>()
        })
        .collect();
    let mut found = RowMatch::default();
    for word in query.to_lowercase().split_whitespace() {
        let in_name = name.find(word).map(|start| start..start + word.len());
        let slots: Vec<usize> = key_words
            .iter()
            .filter(|(_, key_word)| key_word.starts_with(word))
            .map(|&(slot, _)| slot)
            .collect();
        if in_name.is_none() && slots.is_empty() && !category.contains(word) {
            return None;
        }
        found.name.extend(in_name);
        found.slots.extend(slots);
    }
    found.slots.sort_unstable();
    found.slots.dedup();
    Some(found)
}

/// `name` cut into the parts outside and inside `ranges`, each with whether it is inside.
fn highlight<'a>(name: &'a str, ranges: &[Range<usize>]) -> Vec<(&'a str, bool)> {
    let mut lit = vec![false; name.len()];
    for range in ranges {
        lit[range.clone()].fill(true);
    }
    let mut parts = Vec::new();
    let mut start = 0;
    for end in 1..=name.len() {
        if end == name.len() || lit[end] != lit[start] {
            parts.push((&name[start..end], lit[start]));
            start = end;
        }
    }
    parts
}

/// Whether `key` is a modifier, which a capture binds on its release.
fn is_modifier(key: KeyCode) -> bool {
    matches!(
        key,
        KeyCode::ShiftLeft
            | KeyCode::ShiftRight
            | KeyCode::ControlLeft
            | KeyCode::ControlRight
            | KeyCode::AltLeft
            | KeyCode::AltRight
            | KeyCode::SuperLeft
            | KeyCode::SuperRight
    )
}

/// The accessible label of a slot: "Jump, primary key".
fn slot_label(action: GameAction, slot: usize) -> String {
    let which = if slot == 0 { "primary" } else { "secondary" };
    format!("{}, {which} key", action.name())
}

/// A text of the tab.
fn label(content: &str, font: &Handle<Font>, size: f32, color: Color) -> impl Bundle {
    (
        Text::new(content),
        TextFont {
            font: font.clone().into(),
            font_size: px(size).into(),
            ..default()
        },
        TextColor(color),
    )
}

/// A line of the tab: a label that takes the room left, then one column per key slot.
fn line() -> Node {
    Node {
        align_items: AlignItems::Center,
        column_gap: px(SPACE_S),
        ..default()
    }
}

/// Spawns the search field, the headers of the key columns and the scrolling list of the actions
/// under `panel`, and the count of the actions found and the reset button under `footer`, the
/// right end of the footer of the settings. The field holds `search`.
/// `fonts` are the font of the texts and the one that draws the arrows of the key caps.
pub fn spawn_controls_tab(
    commands: &mut Commands,
    (panel, footer): (Entity, Entity),
    key_map: &KeyMap,
    search: &KeySearch,
    fonts: (Handle<Font>, Handle<Font>),
) {
    let (font, glyph_font) = fonts;
    let content = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                flex_grow: 1.0,
                min_height: px(0),
                row_gap: px(SPACE_XS),
                ..default()
            },
            // Up from the first row goes to the search field
            ArrowNav,
            ChildOf(panel),
        ))
        .id();
    // the headers sit over the slots: clear of the frame of the list, its padding and its
    // scrollbar, while it shows (`align_headers`)
    let header = commands
        .spawn((
            Node {
                padding: UiRect::right(px(BEVEL_WIDTH + RING_ROOM + SPACE_XS + SCROLLBAR_WIDTH)),
                ..line()
            },
            ChildOf(content),
        ))
        .id();
    // the search field takes the room left of the headers
    let field = spawn_text_field(
        commands,
        &TextField {
            label: SEARCH_LABEL,
            placeholder: SEARCH_LABEL,
            max_characters: Some(32),
            font: font.clone(),
            font_size: FONT_S,
            reserve_error: false,
        },
    );
    commands
        .entity(field.root)
        .insert(ChildOf(header))
        .entry::<Node>()
        .and_modify(|mut node| {
            node.width = Val::Auto;
            node.min_width = px(0);
            node.flex_grow = 1.0;
        });
    commands
        .entity(field.input)
        .insert(KeySearchField)
        .observe(clear_search_or_leave)
        .observe(|_: On<Submit>, mut commands: Commands| {
            commands.run_system_cached(focus_first_result);
        })
        .observe(
            |input: On<FocusedInput<KeyboardInput>>, mut commands: Commands| {
                let key = &input.input;
                if key.key_code == KeyCode::ArrowDown && key.state == ButtonState::Pressed {
                    commands.run_system_cached(focus_first_result);
                }
            },
        );
    if !search.query.is_empty() {
        let query = search.query.clone();
        commands
            .entity(field.input)
            .entry::<EditableText>()
            .and_modify(move |mut text| set_text(&mut text, &query));
    }
    for column in ["Primary", "Secondary"] {
        commands.spawn((
            Node {
                width: px(SLOT_WIDTH),
                justify_content: JustifyContent::Center,
                ..default()
            },
            ChildOf(header),
            children![label(column, &font, FONT_S, MUTED)],
        ));
    }
    // a row a notch of the wheel
    let row = WheelStep::Px(SLOT_HEIGHT + 2.0 * RING_ROOM);
    let ScrollListParts { frame, list, .. } = spawn_scroll_list(commands, row);
    commands.entity(frame).insert(ChildOf(content));
    commands.entity(header).insert(ColumnHeaders { frame });
    commands.entity(list).insert(ArrowNav);
    for category in ActionCategory::ALL {
        let first = category == ActionCategory::ALL[0];
        // the label goes with the first row: a row brought into view brings it too
        let group = commands
            .spawn((
                Node {
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::top(px(if first { SPACE_S } else { SPACE_M })),
                    ..default()
                },
                CategoryGroup(category),
                ChildOf(list),
                children![label(category.label(), &font, FONT_S, MUTED)],
            ))
            .id();
        let actions = GameAction::LISTED
            .into_iter()
            .filter(|action| action.category() == category);
        for (index, action) in actions.enumerate() {
            let parent = if index == 0 { group } else { list };
            // the conflict message wraps under the label and the slots
            let row = commands
                .spawn((
                    Node {
                        padding: UiRect::vertical(px(RING_ROOM)),
                        flex_wrap: FlexWrap::Wrap,
                        ..line()
                    },
                    ActionRow(action),
                    ChildOf(parent),
                ))
                .id();
            commands.spawn((
                label(action.name(), &font, FONT_M, TEXT),
                ActionName(action),
                Node {
                    flex_grow: 1.0,
                    ..default()
                },
                ChildOf(row),
            ));
            for slot in 0..MAX_KEYS_PER_ACTION {
                let key_slot = spawn_key_slot(commands, KeySlot { action, slot });
                commands.entity(key_slot).insert(ChildOf(row));
                let key = bound_key(key_map, action, slot);
                let content = key.map_or(SlotContent::Empty, SlotContent::Key);
                show_in_slot(commands, key_slot, content, &font, &glyph_font);
            }
            commands.spawn((
                ConflictMessage(action),
                label("", &font, FONT_S, DANGER),
                Node {
                    width: percent(100),
                    display: Display::None,
                    ..default()
                },
                ChildOf(row),
            ));
        }
    }
    commands.spawn((
        NoMatch,
        label("", &font, FONT_M, MUTED),
        Node {
            display: Display::None,
            align_self: AlignSelf::Center,
            padding: UiRect::vertical(px(SPACE_M)),
            ..default()
        },
        ChildOf(list),
    ));
    commands.spawn((MatchCount, label("", &font, FONT_S, MUTED), ChildOf(footer)));
    commands.spawn((
        button("Reset to defaults", ButtonVariant::Default, font),
        ResetBindings,
        observe(ask_reset),
        ChildOf(footer),
    ));
}

/// The title of the confirmation of the reset.
const RESET_TITLE: &str = "Reset key bindings?";

/// Asks to confirm the reset, Cancel focused; Reset binds the default keys to every action and
/// saves them. The focus goes back to the reset button either way.
fn ask_reset(activate: On<Activate>, assets: Res<AssetServer>, mut commands: Commands) {
    let dialog = Dialog {
        title: RESET_TITLE,
        body: "Every action gets its default keys back.",
        actions: &[
            ("Cancel", ButtonVariant::Default),
            ("Reset", ButtonVariant::Danger),
        ],
        focused: 0,
    };
    let focus = InputFocus::from_entity(activate.entity);
    let root = spawn_dialog(&mut commands, &dialog, assets.load(FONT_PATH), &focus);
    commands.entity(root).observe(
        |closed: On<DialogClosed>, mut key_map: ResMut<KeyMap>, paths: Res<GameFolderPaths>| {
            if closed.action == Some(1) {
                *key_map = default_bindings();
                write_keybindings(&key_map, &paths);
            }
        },
    );
}

/// Spawns an empty key slot, which the player focuses, clicks and types into.
fn spawn_key_slot(commands: &mut Commands, slot: KeySlot) -> Entity {
    commands
        .spawn((
            slot,
            // the role before the label: the label's default node must not win
            AccessibilityNode(accesskit::Node::new(Role::Button)),
            AccessibleLabel::new(slot_label(slot.action, slot.slot)),
            TabIndex(0),
            FocusRing,
            Hovered::default(),
            Node {
                width: px(SLOT_WIDTH),
                height: px(SLOT_HEIGHT),
                border: UiRect::all(px(BEVEL_WIDTH)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                // a long key name stays inside
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(RAISE),
            raised_bevel(),
        ))
        .observe(slot_key)
        .observe(slot_click)
        // pressed while a mouse button is down on it, as a button is
        .observe(|press: On<Pointer<Press>>, mut commands: Commands| {
            commands.entity(press.entity).insert(Pressed);
        })
        .observe(|release: On<Pointer<Release>>, mut commands: Commands| {
            commands.entity(release.entity).remove::<Pressed>();
        })
        .observe(|drag_end: On<Pointer<DragEnd>>, mut commands: Commands| {
            commands.entity(drag_end.entity).remove::<Pressed>();
        })
        .observe(|cancel: On<Pointer<Cancel>>, mut commands: Commands| {
            commands.entity(cancel.entity).remove::<Pressed>();
        })
        .observe(show_focused_row)
        .observe(|lost: On<FocusLost>, mut capture: ResMut<KeyCapture>| {
            if capture.slot == Some(lost.entity) {
                *capture = KeyCapture::default();
            }
        })
        .id()
}

/// Scrolls the list to show the whole row of the focused slot, with the room of its ring, and the
/// label of its category above the first row of a category.
fn show_focused_row(
    gained: On<FocusGained>,
    parents: Query<&ChildOf>,
    lists: Query<(), With<ScrollArea>>,
    mut commands: Commands,
) {
    // the item of the list that holds the slot: its row, or the group of a first row
    let mut item = gained.entity;
    while let Ok(parent) = parents.get(item) {
        if lists.contains(parent.parent()) {
            commands.trigger(ScrollIntoView { entity: item });
            return;
        }
        item = parent.parent();
    }
}

/// The key in the slot `slot` of `action`, if any.
fn bound_key(key_map: &KeyMap, action: GameAction, slot: usize) -> Option<KeyCode> {
    key_map
        .map
        .get(&action)
        .and_then(|keys| keys.get(slot))
        .copied()
}

/// What a key slot shows.
enum SlotContent {
    Key(KeyCode),
    Capturing,
    Empty,
}

/// Replaces what the slot shows.
fn show_in_slot(
    commands: &mut Commands,
    slot: Entity,
    content: SlotContent,
    font: &Handle<Font>,
    glyph_font: &Handle<Font>,
) {
    commands.entity(slot).despawn_related::<Children>();
    match content {
        SlotContent::Key(key) => {
            commands.spawn((
                key_cap(&key_label(key), font.clone(), glyph_font.clone()),
                ChildOf(slot),
            ));
        }
        SlotContent::Capturing => {
            commands.spawn((
                label(PRESS_A_KEY, font, FONT_S, TEXT),
                Pickable::IGNORE,
                ChildOf(slot),
            ));
        }
        SlotContent::Empty => {}
    }
}

/// Shows the keys of every slot, or `PRESS_A_KEY` in the one capturing.
fn show_keys(
    slots: Query<(Entity, &KeySlot)>,
    key_map: Res<KeyMap>,
    capture: Res<KeyCapture>,
    assets: Res<AssetServer>,
    mut commands: Commands,
) {
    let font: Handle<Font> = assets.load(FONT_PATH);
    let glyph_font: Handle<Font> = assets.load(MONO_FONT_PATH);
    for (entity, slot) in &slots {
        let content = if capture.slot == Some(entity) {
            SlotContent::Capturing
        } else {
            let key = bound_key(&key_map, slot.action, slot.slot);
            key.map_or(SlotContent::Empty, SlotContent::Key)
        };
        show_in_slot(&mut commands, entity, content, &font, &glyph_font);
    }
}

/// Shows under each row the keys its action shares with other actions, and marks the slots
/// holding them.
fn show_conflicts(
    key_map: Res<KeyMap>,
    mut messages: Query<(&ConflictMessage, &mut Text, &mut Node)>,
    slots: Query<(Entity, &KeySlot)>,
    mut commands: Commands,
) {
    let conflicts = conflicts(&key_map);
    for (message, mut text, mut node) in &mut messages {
        match conflicts.get(&message.0) {
            Some(shared) => {
                text.0 = conflict_message(shared);
                node.display = Display::Flex;
            }
            None => node.display = Display::None,
        }
    }
    for (entity, slot) in &slots {
        let shared = conflicts.get(&slot.action);
        let conflicting = bound_key(&key_map, slot.action, slot.slot)
            .is_some_and(|key| shared.is_some_and(|shared| shared.iter().any(|&(k, _)| k == key)));
        if conflicting {
            commands.entity(entity).insert(Conflicting);
        } else {
            commands.entity(entity).remove::<Conflicting>();
        }
    }
}

/// Lightens the slot under the cursor, inverts the bevel of the pressed one as on a button,
/// outlines the capturing one and the ones holding a key the search names in the accent, and one
/// holding a key bound to another action too in red.
fn style_key_slots(
    mut slots: Query<
        (
            Entity,
            &Hovered,
            Has<Pressed>,
            Has<Conflicting>,
            Has<SearchHit>,
            &mut BackgroundColor,
            &mut BorderColor,
        ),
        With<KeySlot>,
    >,
    capture: Res<KeyCapture>,
) {
    for (entity, hovered, pressed, conflicting, hit, mut background, mut border) in &mut slots {
        let surface = if hovered.get() { RAISE_HOVER } else { RAISE };
        background.set_if_neq(BackgroundColor(surface));
        let edge = if capture.slot == Some(entity) || hit {
            BorderColor::all(ACCENT)
        } else if conflicting {
            BorderColor::all(DANGER)
        } else if pressed {
            sunken_bevel()
        } else {
            raised_bevel()
        };
        border.set_if_neq(edge);
    }
}

/// The keys pressed on a focused slot. While it captures, every key stops here: the first one
/// pressed is bound to the slot, or cancels the capture if it is Escape; a modifier is bound
/// when it is released, if no other key came first. Otherwise Enter or Space starts the capture,
/// as they activate a button, and Delete or Backspace clears the slot.
fn slot_key(
    mut event: On<FocusedInput<KeyboardInput>>,
    slots: Query<&KeySlot>,
    mut capture: ResMut<KeyCapture>,
    mut key_map: ResMut<KeyMap>,
    paths: Res<GameFolderPaths>,
) {
    let entity = event.event_target();
    let Ok(&KeySlot { action, slot }) = slots.get(entity) else {
        return;
    };
    let input = event.input.clone();
    if capture.slot == Some(entity) {
        event.propagate(false);
        let key = input.key_code;
        if capture.ending || input.repeat || matches!(key, KeyCode::Unidentified(_)) {
            return;
        }
        let pressed = input.state == ButtonState::Pressed;
        // a modifier alone binds on its release, so that a shortcut such as Alt+Tab does not
        let bound = match (pressed, is_modifier(key)) {
            (true, true) => {
                capture.modifier = Some(key);
                return;
            }
            (true, false) => key != KeyCode::Escape,
            (false, _) if capture.modifier == Some(key) => true,
            (false, _) => return,
        };
        if bound {
            set_key(&mut key_map, action, slot, key);
            write_keybindings(&key_map, &paths);
        }
        capture.ending = true;
        return;
    }
    if input.state != ButtonState::Pressed {
        return;
    }
    match input.key_code {
        KeyCode::Enter | KeyCode::Space if !input.repeat => {
            event.propagate(false);
            *capture = KeyCapture::on(entity);
        }
        KeyCode::Delete | KeyCode::Backspace => {
            event.propagate(false);
            clear_key(&mut key_map, action, slot);
            write_keybindings(&key_map, &paths);
        }
        _ => {}
    }
}

/// A left click on a slot starts its capture; a right click clears it.
fn slot_click(
    mut click: On<Pointer<Click>>,
    slots: Query<&KeySlot>,
    mut capture: ResMut<KeyCapture>,
    mut key_map: ResMut<KeyMap>,
    paths: Res<GameFolderPaths>,
) {
    let entity = click.event_target();
    let Ok(&KeySlot { action, slot }) = slots.get(entity) else {
        return;
    };
    click.propagate(false);
    if capture.slot == Some(entity) {
        return;
    }
    match click.button {
        PointerButton::Primary => {
            *capture = KeyCapture::on(entity);
        }
        PointerButton::Secondary => {
            clear_key(&mut key_map, action, slot);
            write_keybindings(&key_map, &paths);
        }
        PointerButton::Middle => {}
    }
}

/// Shows in the hint bar of the settings `CAPTURE_HINTS` while a slot waits for a key, the hints
/// of the search while its field has the focus, and `CONTROLS_HINTS` otherwise.
fn swap_hints(
    capture: Res<KeyCapture>,
    focus: Res<InputFocus>,
    fields: Query<&EditableText, With<KeySearchField>>,
    bars: Query<(Entity, &ChildOf, Option<&ControlsHints>), With<HintBar>>,
    roots: Query<(), With<SettingsRoot>>,
    assets: Res<AssetServer>,
    mut commands: Commands,
) {
    let wanted = if capture.is_active() {
        ControlsHints::Capturing
    } else if let Some(text) = focus.get().and_then(|entity| fields.get(entity).ok()) {
        ControlsHints::Searching {
            empty: text.value() == "",
        }
    } else {
        ControlsHints::Browsing
    };
    for (bar, child_of, shown) in &bars {
        let root = child_of.parent();
        let shown = shown.copied().unwrap_or(ControlsHints::Browsing);
        if !roots.contains(root) || shown == wanted {
            continue;
        }
        commands.entity(bar).despawn();
        let hints: &[Hint] = match wanted {
            ControlsHints::Browsing => &CONTROLS_HINTS,
            ControlsHints::Capturing => &CAPTURE_HINTS,
            ControlsHints::Searching { empty: false } => &SEARCH_HINTS,
            ControlsHints::Searching { empty: true } => &EMPTY_SEARCH_HINTS,
        };
        let new_bar = spawn_hints(&mut commands, root, hints, &assets);
        commands.entity(new_bar).insert(wanted);
    }
}

/// Keeps the headers over their slots, which the scrollbar of the list pushes left only while it
/// shows: a list the search shortens shows none.
fn align_headers(
    mut headers: Query<(&ColumnHeaders, &mut Node)>,
    children: Query<&Children>,
    bars: Query<&Node, (With<Scrollbar>, Without<ColumnHeaders>)>,
) {
    for (headers, mut node) in &mut headers {
        let bar_shown = children
            .get(headers.frame)
            .into_iter()
            .flat_map(|children| children.iter())
            .filter_map(|child| bars.get(child).ok())
            .any(|bar| bar.display != Display::None);
        let bar_room = if bar_shown {
            SPACE_XS + SCROLLBAR_WIDTH
        } else {
            0.0
        };
        let padding = UiRect::right(px(BEVEL_WIDTH + RING_ROOM + bar_room));
        if node.padding != padding {
            node.padding = padding;
        }
    }
}

/// The keys bound to `action`.
fn keys_of(key_map: &KeyMap, action: GameAction) -> &[KeyCode] {
    key_map.map.get(&action).map_or(&[], Vec::as_slice)
}

/// Keeps the search up to date with its field.
fn read_search(
    fields: Query<&EditableText, (With<KeySearchField>, Changed<EditableText>)>,
    mut search: ResMut<KeySearch>,
) {
    for text in &fields {
        let query = text.value().to_string();
        if search.query != query {
            search.query = query;
        }
    }
}

/// Shows the rows the search finds and the categories that hold one, hides the others, and tells
/// how many it finds, or that it finds none.
fn filter_rows(
    search: Res<KeySearch>,
    key_map: Res<KeyMap>,
    rows: Query<(Entity, &ActionRow)>,
    groups: Query<(Entity, &CategoryGroup)>,
    no_match: Single<Entity, With<NoMatch>>,
    count: Single<Entity, With<MatchCount>>,
    mut nodes: Query<&mut Node>,
    mut texts: Query<&mut Text>,
) {
    let found: Vec<GameAction> = rows
        .iter()
        .map(|(_, row)| row.0)
        .filter(|&action| search_row(&search.query, action, keys_of(&key_map, action)).is_some())
        .collect();
    let mut show = |entity: Entity, shown: bool| {
        let display = if shown { Display::Flex } else { Display::None };
        if let Ok(mut node) = nodes.get_mut(entity) {
            if node.display != display {
                node.display = display;
            }
        }
    };
    for (entity, row) in &rows {
        show(entity, found.contains(&row.0));
    }
    for (entity, group) in &groups {
        show(
            entity,
            found.iter().any(|action| action.category() == group.0),
        );
    }
    show(*no_match, found.is_empty());
    let query = search.query.trim();
    if let Ok(mut text) = texts.get_mut(*no_match) {
        text.0 = format!("No action or key matches \u{201c}{query}\u{201d}");
    }
    if let Ok(mut text) = texts.get_mut(*count) {
        text.0 = if query.is_empty() {
            String::new()
        } else {
            format!("{} of {}", found.len(), rows.iter().count())
        };
    }
}

/// Lights up in the accent the parts of the names that the search finds, and marks the slots
/// holding a key it names.
fn light_up_matches(
    search: Res<KeySearch>,
    key_map: Res<KeyMap>,
    mut names: Query<(Entity, &ActionName, &TextFont, &mut Text)>,
    slots: Query<(Entity, &KeySlot)>,
    mut commands: Commands,
) {
    let found = |action: GameAction| {
        search_row(&search.query, action, keys_of(&key_map, action)).unwrap_or_default()
    };
    for (entity, name, font, mut text) in &mut names {
        let action = name.0;
        commands.entity(entity).despawn_related::<Children>();
        let ranges = found(action).name;
        if ranges.is_empty() {
            text.0 = action.name().to_string();
            continue;
        }
        text.0.clear();
        for (part, lit) in highlight(action.name(), &ranges) {
            commands.spawn((
                TextSpan::new(part),
                font.clone(),
                TextColor(if lit { ACCENT } else { TEXT }),
                ChildOf(entity),
            ));
        }
    }
    for (entity, slot) in &slots {
        if found(slot.action).slots.contains(&slot.slot) {
            commands.entity(entity).insert(SearchHit);
        } else {
            commands.entity(entity).remove::<SearchHit>();
        }
    }
}

/// Gives the focus, with its ring, to the primary slot of the first row shown.
fn focus_first_result(
    rows: Query<(&ActionRow, &Node)>,
    slots: Query<(Entity, &KeySlot)>,
    mut focus: ResMut<InputFocus>,
    mut visible: ResMut<InputFocusVisible>,
) {
    // the rows in the order of the list: by category, then as listed
    let shown = ActionCategory::ALL
        .into_iter()
        .flat_map(|category| {
            GameAction::LISTED
                .into_iter()
                .filter(move |action| action.category() == category)
        })
        .find(|&action| {
            rows.iter()
                .any(|(row, node)| row.0 == action && node.display != Display::None)
        });
    let first = shown.and_then(|action| {
        slots
            .iter()
            .find(|(_, slot)| **slot == KeySlot { action, slot: 0 })
    });
    if let Some((slot, _)) = first {
        focus.set(slot, FocusCause::Navigated);
        visible.0 = true;
    }
}

/// Escape in the search field clears it, or leaves the settings when it is empty, as Escape does
/// everywhere else in them.
fn clear_search_or_leave(
    cancel: On<FieldCancel>,
    mut texts: Query<&mut EditableText>,
    roots: Query<Entity, With<SettingsRoot>>,
    mut commands: Commands,
) {
    let Ok(mut text) = texts.get_mut(cancel.entity) else {
        return;
    };
    if text.value() == "" {
        for root in &roots {
            commands.entity(root).insert(CloseSettings);
        }
    } else {
        set_text(&mut text, "");
    }
}

/// Ends the capture that got its key, whose slot is gone, or whose window lost the keyboard.
fn end_capture(
    mut capture: ResMut<KeyCapture>,
    slots: Query<(), With<KeySlot>>,
    mut focus_lost: MessageReader<KeyboardFocusLost>,
    mut window_focus: MessageReader<WindowFocused>,
) {
    // the keys released in another window never come: a modifier held when the window lost the
    // keyboard must not be bound when it comes back
    let window_left =
        focus_lost.read().count() > 0 || window_focus.read().any(|focused| !focused.focused);
    let gone = capture.slot.is_some_and(|slot| !slots.contains(slot));
    if capture.is_active() && (capture.ending || gone || window_left) {
        *capture = KeyCapture::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_have_human_labels() {
        let cases = [
            (KeyCode::KeyW, "W"),
            (KeyCode::Digit1, "1"),
            (KeyCode::Space, "Space"),
            (KeyCode::ShiftLeft, "Left Shift"),
            (KeyCode::ControlRight, "Right Ctrl"),
            (KeyCode::F3, "F3"),
            (KeyCode::ArrowLeft, "←"),
            (KeyCode::NumpadSubtract, "Num -"),
            (KeyCode::NumpadAdd, "Num +"),
            (KeyCode::Numpad0, "Num 0"),
            (KeyCode::NumpadEnter, "Num Enter"),
            (KeyCode::SuperLeft, "Super"),
            (KeyCode::SuperRight, "Super"),
        ];
        for (key, label) in cases {
            assert_eq!(key_label(key), label, "{key:?}");
        }
    }

    #[test]
    fn a_conflict_message_names_the_key_and_the_other_action() {
        assert_eq!(
            conflict_message(&[(KeyCode::KeyW, KeyUse::Action(GameAction::MoveForward))]),
            "W is also bound to Move forward"
        );
        assert_eq!(
            conflict_message(&[
                (KeyCode::KeyW, KeyUse::Action(GameAction::MoveForward)),
                (KeyCode::KeyE, KeyUse::Action(GameAction::ToggleInventory)),
            ]),
            "W is also bound to Move forward · E is also bound to Inventory"
        );
        // the font of the texts has no arrows: only key caps draw them
        assert_eq!(
            conflict_message(&[(KeyCode::ArrowUp, KeyUse::Action(GameAction::MoveForward))]),
            "Up arrow is also bound to Move forward"
        );
        // one sentence per key, naming every other action of that key
        assert_eq!(
            conflict_message(&[
                (KeyCode::ArrowUp, KeyUse::Action(GameAction::MoveForward)),
                (KeyCode::ArrowUp, KeyUse::Action(GameAction::MoveBackward)),
            ]),
            "Up arrow is also bound to Move forward and Move backward"
        );
        assert_eq!(
            conflict_message(&[
                (KeyCode::KeyW, KeyUse::Action(GameAction::MoveForward)),
                (KeyCode::KeyE, KeyUse::Action(GameAction::ToggleInventory)),
                (KeyCode::KeyW, KeyUse::Action(GameAction::MoveBackward)),
                (KeyCode::KeyW, KeyUse::Action(GameAction::OpenChat)),
            ]),
            "W is also bound to Move forward, Move backward and Chat \
             · E is also bound to Inventory"
        );
    }

    #[test]
    fn a_conflict_message_names_the_hotbar_slot_of_a_digit() {
        assert_eq!(
            conflict_message(&[(KeyCode::Digit1, KeyUse::HotbarSlot(0))]),
            "1 also selects hotbar slot 1"
        );
        // a sentence of its own, beside the one naming the other actions of the key
        assert_eq!(
            conflict_message(&[
                (KeyCode::Digit3, KeyUse::Action(GameAction::Jump)),
                (KeyCode::Digit3, KeyUse::Action(GameAction::OpenChat)),
                (KeyCode::Digit3, KeyUse::HotbarSlot(2)),
                (KeyCode::Escape, KeyUse::Action(GameAction::Escape)),
            ]),
            "3 is also bound to Jump and Chat · 3 also selects hotbar slot 3 \
             · Escape is also bound to Pause"
        );
    }

    #[test]
    // lists of the byte ranges found, not ranges to collect
    #[allow(clippy::single_range_in_vec_init)]
    fn the_search_finds_a_name_a_category_or_a_key() {
        let jump = [KeyCode::Space];
        // in the name, regardless of case, lit up
        assert_eq!(
            search_row("JU", GameAction::Jump, &jump),
            Some(RowMatch {
                name: vec![0..2],
                slots: vec![],
            })
        );
        assert_eq!(
            search_row("fly", GameAction::FlyDown, &[KeyCode::ShiftLeft]),
            Some(RowMatch {
                name: vec![8..11],
                slots: vec![],
            })
        );
        // the start of a word of a key's name, the slot marked
        assert_eq!(
            search_row("shi", GameAction::FlyDown, &[KeyCode::ShiftLeft]),
            Some(RowMatch {
                name: vec![],
                slots: vec![0],
            })
        );
        assert_eq!(
            search_row(
                "arrow",
                GameAction::MoveForward,
                &[KeyCode::KeyW, KeyCode::ArrowUp]
            ),
            Some(RowMatch {
                name: vec![],
                slots: vec![1],
            })
        );
        assert_eq!(
            search_row("hift", GameAction::FlyDown, &[KeyCode::ShiftLeft]),
            None
        );
        // the category
        assert_eq!(
            search_row("debug", GameAction::ReloadChunks, &[KeyCode::KeyR]),
            Some(RowMatch::default())
        );
        assert_eq!(search_row("xyz", GameAction::Jump, &jump), None);
    }

    #[test]
    // lists of the byte ranges found, not ranges to collect
    #[allow(clippy::single_range_in_vec_init)]
    fn the_search_wants_every_word_and_an_empty_one_finds_all() {
        let raise = [KeyCode::KeyP];
        assert_eq!(
            search_row("render p", GameAction::RenderDistancePlus, &raise),
            Some(RowMatch {
                name: vec![6..12],
                slots: vec![0],
            })
        );
        assert_eq!(
            search_row(
                "render p",
                GameAction::RenderDistanceMinus,
                &[KeyCode::KeyO]
            ),
            None
        );
        assert_eq!(
            search_row("  ", GameAction::Jump, &[]),
            Some(RowMatch::default())
        );
        // a key named twice is marked once
        assert_eq!(
            search_row("up arrow", GameAction::MoveForward, &[KeyCode::ArrowUp]),
            Some(RowMatch {
                name: vec![],
                slots: vec![0],
            })
        );
    }

    #[test]
    // lists of the byte ranges found, not ranges to collect
    #[allow(clippy::single_range_in_vec_init)]
    fn a_name_is_cut_around_the_parts_found() {
        assert_eq!(highlight("Jump", &[]), [("Jump", false)]);
        assert_eq!(
            highlight("Fly down", &[0..3]),
            [("Fly", true), (" down", false)]
        );
        // overlapping parts make one
        assert_eq!(
            highlight("Move forward", &[2..6, 5..8, 11..12]),
            [("Mo", false), ("ve for", true), ("war", false), ("d", true)]
        );
    }

    #[test]
    fn slots_are_labelled_by_their_action_and_column() {
        assert_eq!(slot_label(GameAction::Jump, 0), "Jump, primary key");
        assert_eq!(slot_label(GameAction::Jump, 1), "Jump, secondary key");
    }
}
