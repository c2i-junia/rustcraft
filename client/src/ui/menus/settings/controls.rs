//! The Controls tab of the settings: the actions by category, each with a primary and a secondary
//! key slot. Enter or a click on a slot waits for the key to bind to it, which Escape cancels;
//! Delete, Backspace or a right click clears it. Every change is saved at once.

use accesskit::Role;
use bevy::{
    a11y::AccessibilityNode,
    input::{keyboard::KeyboardInput, ButtonState},
    input_focus::{tab_navigation::TabIndex, FocusGained, FocusLost, FocusedInput},
    picking::hover::Hovered,
    prelude::*,
    ui_widgets::{ScrollArea, ScrollIntoView},
};
use shared::GameFolderPaths;

use crate::input::data::{ActionCategory, GameAction};
use crate::input::keyboard::{clear_key, set_key, write_keybindings, MAX_KEYS_PER_ACTION};
use crate::state::KeyMap;
use crate::ui::assets::{FONT_PATH, MONO_FONT_PATH};
use crate::ui::theme::{
    ACCENT, BEVEL_HI, BEVEL_LO, BEVEL_WIDTH, FOCUS_RING_OFFSET, FOCUS_RING_WIDTH, FONT_M, FONT_S,
    MUTED, RAISE, RAISE_HOVER, SPACE_M, SPACE_S, SPACE_XS, TEXT,
};
use crate::ui::widgets::focus::{ArrowNav, FocusRing};
use crate::ui::widgets::hints::{key_cap, Hint};

/// The slot `slot` of the keys of `action`: 0 for the primary key, 1 for the secondary one.
#[derive(Component, Clone, Copy, PartialEq, Eq, Debug)]
pub struct KeySlot {
    pub action: GameAction,
    pub slot: usize,
}

/// The key slot waiting for a key to bind, if any.
#[derive(Resource, Default)]
pub struct KeyCapture {
    slot: Option<Entity>,
    /// The key that ends the capture came: it ends in `PostUpdate`, so that the Escape handlers
    /// of `Update` still see it active in the frame of that key.
    ending: bool,
}

impl KeyCapture {
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
        keys: "Tab",
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
        keys: "Esc",
        action: "Back",
    },
    Hint {
        keys: "↑↓",
        action: "Move",
    },
];

/// The width of a key slot, and of the header of its column.
const SLOT_WIDTH: f32 = 120.0;
/// The height of a key slot: a key cap and the bevel around it.
const SLOT_HEIGHT: f32 = 34.0;
/// The room around the list for the focus ring of the slots, which the list would clip.
const RING_ROOM: f32 = FOCUS_RING_WIDTH + FOCUS_RING_OFFSET;

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<KeyCapture>()
        .add_systems(
            Update,
            (
                style_key_slots,
                show_keys
                    .run_if(resource_changed::<KeyMap>.or_else(resource_changed::<KeyCapture>)),
            )
                .run_if(any_with_component::<KeySlot>),
        )
        .add_systems(PostUpdate, end_capture);
}

/// How a key is written on the screen: `KeyW` is `W`, `ShiftLeft` is `Left Shift`, the arrow
/// keys are arrows.
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
        KeyCode::ArrowUp => "↑",
        KeyCode::ArrowDown => "↓",
        KeyCode::ArrowLeft => "←",
        KeyCode::ArrowRight => "→",
        _ => {
            let name = format!("{key:?}");
            return match name.strip_prefix("Digit") {
                Some(digit) => digit.to_string(),
                None => name.replace("Key", ""),
            };
        }
    };
    named.to_string()
}

/// The accessible label of a slot: "Jump, primary key".
fn slot_label(action: GameAction, slot: usize) -> String {
    let which = if slot == 0 { "primary" } else { "secondary" };
    format!("{}, {which} key", action.name())
}

/// The bevel of a raised control.
fn raised_bevel() -> BorderColor {
    BorderColor {
        top: BEVEL_HI,
        left: BEVEL_HI,
        bottom: BEVEL_LO,
        right: BEVEL_LO,
    }
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

/// Spawns the headers of the key columns and the scrolling list of the actions under `panel`.
/// `fonts` are the font of the texts and the one that draws the arrows of the key caps.
pub fn spawn_controls_tab(
    commands: &mut Commands,
    panel: Entity,
    key_map: &KeyMap,
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
                // the room for the rings goes into the margin of the panel: the texts and the
                // slots line up with the rest of the panel
                margin: UiRect::horizontal(px(-RING_ROOM)),
                ..default()
            },
            ChildOf(panel),
        ))
        .id();
    let header = commands
        .spawn((
            Node {
                padding: UiRect::horizontal(px(RING_ROOM)),
                ..line()
            },
            ChildOf(content),
        ))
        .id();
    commands.spawn((
        Node {
            flex_grow: 1.0,
            ..default()
        },
        ChildOf(header),
    ));
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
    let list = commands
        .spawn((
            ScrollArea,
            ArrowNav,
            Node {
                flex_direction: FlexDirection::Column,
                flex_grow: 1.0,
                min_height: px(0),
                overflow: Overflow::scroll_y(),
                row_gap: px(SPACE_XS),
                padding: UiRect::all(px(RING_ROOM)),
                ..default()
            },
            ChildOf(content),
        ))
        .id();
    for category in ActionCategory::ALL {
        let first = category == ActionCategory::ALL[0];
        commands.spawn((
            label(category.label(), &font, FONT_S, MUTED),
            Node {
                margin: UiRect::top(px(if first { 0.0 } else { SPACE_M })),
                ..default()
            },
            ChildOf(list),
        ));
        for action in GameAction::LISTED
            .into_iter()
            .filter(|action| action.category() == category)
        {
            let row = commands.spawn((line(), ChildOf(list))).id();
            commands.spawn((
                label(action.name(), &font, FONT_M, TEXT),
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
        }
    }
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
                ..default()
            },
            BackgroundColor(RAISE),
            raised_bevel(),
        ))
        .observe(slot_key)
        .observe(slot_click)
        .observe(|gained: On<FocusGained>, mut commands: Commands| {
            commands.trigger(ScrollIntoView {
                entity: gained.entity,
            });
        })
        .observe(|lost: On<FocusLost>, mut capture: ResMut<KeyCapture>| {
            if capture.slot == Some(lost.entity) {
                *capture = KeyCapture::default();
            }
        })
        .id()
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

/// Lightens the slot under the cursor, and outlines the capturing one in the accent.
fn style_key_slots(
    mut slots: Query<(Entity, &Hovered, &mut BackgroundColor, &mut BorderColor), With<KeySlot>>,
    capture: Res<KeyCapture>,
) {
    for (entity, hovered, mut background, mut border) in &mut slots {
        let surface = if hovered.get() { RAISE_HOVER } else { RAISE };
        background.set_if_neq(BackgroundColor(surface));
        let edge = if capture.slot == Some(entity) {
            BorderColor::all(ACCENT)
        } else {
            raised_bevel()
        };
        border.set_if_neq(edge);
    }
}

/// The keys pressed on a focused slot. While it captures, every key stops here: the first one
/// pressed is bound to the slot, or cancels the capture if it is Escape. Otherwise Enter starts
/// the capture, and Delete or Backspace clears the slot.
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
        if capture.ending || input.state != ButtonState::Pressed || input.repeat {
            return;
        }
        if input.key_code != KeyCode::Escape {
            set_key(&mut key_map, action, slot, input.key_code);
            write_keybindings(&key_map, &paths);
        }
        capture.ending = true;
        return;
    }
    if input.state != ButtonState::Pressed {
        return;
    }
    match input.key_code {
        KeyCode::Enter if !input.repeat => {
            event.propagate(false);
            *capture = KeyCapture {
                slot: Some(entity),
                ending: false,
            };
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
            *capture = KeyCapture {
                slot: Some(entity),
                ending: false,
            };
        }
        PointerButton::Secondary => {
            clear_key(&mut key_map, action, slot);
            write_keybindings(&key_map, &paths);
        }
        PointerButton::Middle => {}
    }
}

/// Ends the capture that got its key, or whose slot is gone.
fn end_capture(mut capture: ResMut<KeyCapture>, slots: Query<(), With<KeySlot>>) {
    let gone = capture.slot.is_some_and(|slot| !slots.contains(slot));
    if capture.ending || gone {
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
        ];
        for (key, label) in cases {
            assert_eq!(key_label(key), label, "{key:?}");
        }
    }

    #[test]
    fn slots_are_labelled_by_their_action_and_column() {
        assert_eq!(slot_label(GameAction::Jump, 0), "Jump, primary key");
        assert_eq!(slot_label(GameAction::Jump, 1), "Jump, secondary key");
    }
}
