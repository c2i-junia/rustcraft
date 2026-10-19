//! The themed text field: a sunken `EditableText` with a placeholder and an error message.
//!
//! Screens put their marker and their `On<Submit>` / `On<Cancel>` observers on the `input` of
//! `spawn_text_field`, and set its `FieldError` to show an error under the field.

use accesskit::Role;
use bevy::{
    a11y::AccessibilityNode,
    input::{
        keyboard::{Key, KeyboardInput},
        ButtonState,
    },
    input_focus::{tab_navigation::TabIndex, AcquireFocus, FocusCause, FocusedInput, InputFocus},
    prelude::*,
    text::{EditableText, EditableTextSystems, LineBreak, LineHeight, TextCursorStyle, TextEdit},
    ui::UiSystems,
};

use super::{focus::FocusRing, sunken_bevel};
use crate::ui::theme::{ACCENT, BEVEL_WIDTH, DANGER, FONT_S, MUTED, SLOT, SPACE_S, SPACE_XS, TEXT};

/// The height of the line a field keeps for its error: one line of `FONT_S`.
const ERROR_LINE_HEIGHT: f32 = FONT_S * 1.2;

/// What `spawn_text_field` builds.
pub struct TextField<'a> {
    /// The accessible label of the field.
    pub label: &'a str,
    /// The text shown while the field is empty.
    pub placeholder: &'a str,
    pub max_characters: Option<usize>,
    pub font: Handle<Font>,
    pub font_size: f32,
    /// Keeps one line under the field for its error, so that an error appearing moves nothing
    /// below. The message never wraps and is clipped at the width of the field: callers keep
    /// their messages short (a failure from the disk shows as a short text, not the OS message).
    pub reserve_error: bool,
}

/// The entities of a text field: `root` to place it, `input` to observe it and read its value.
#[derive(Clone, Copy)]
pub struct TextFieldParts {
    pub root: Entity,
    pub input: Entity,
}

/// Triggered on the `input` of a text field when Enter is pressed in it, with its value.
#[derive(EntityEvent)]
pub struct Submit {
    pub entity: Entity,
    pub value: String,
}

/// Triggered on the `input` of a text field when Escape is pressed in it.
#[derive(EntityEvent)]
pub struct Cancel {
    pub entity: Entity,
}

/// The error shown under a text field, on its `input`; `None` shows none.
#[derive(Component, Default)]
pub struct FieldError(pub Option<String>);

/// The other parts of a text field, on its `input`.
#[derive(Component)]
struct FieldParts {
    frame: Entity,
    placeholder: Entity,
    message: Entity,
    reserve_error: bool,
}

/// A Submit or a Cancel to trigger once the edits of this update are applied, on the `input`.
#[derive(Component)]
enum FieldRequest {
    Submit,
    Cancel,
}

/// Spawns a text field, without a parent: put `root` in place.
pub fn spawn_text_field(commands: &mut Commands, field: &TextField) -> TextFieldParts {
    let font = TextFont {
        font: field.font.clone().into(),
        font_size: px(field.font_size).into(),
        ..default()
    };
    let root = commands
        .spawn(Node {
            width: percent(100),
            flex_direction: FlexDirection::Column,
            row_gap: px(SPACE_XS),
            ..default()
        })
        .id();
    let frame = commands
        .spawn((
            FocusRing,
            Node {
                border: UiRect::all(px(BEVEL_WIDTH)),
                padding: UiRect::all(px(SPACE_S)),
                ..default()
            },
            BackgroundColor(SLOT),
            sunken_bevel(),
            ChildOf(root),
        ))
        .id();
    let input = commands
        .spawn((
            EditableText {
                max_characters: field.max_characters,
                ..default()
            },
            TextLayout {
                linebreak: LineBreak::NoWrap,
                ..default()
            },
            font.clone(),
            TextColor(TEXT),
            TextCursorStyle {
                color: TEXT,
                selection_color: ACCENT.with_alpha(0.4),
                unfocused_selection_color: Color::NONE,
                selected_text_color: None,
            },
            TabIndex(0),
            // the role before the label: the label's default node must not win
            AccessibilityNode(accesskit::Node::new(Role::TextInput)),
            AccessibleLabel::new(field.label),
            FieldError::default(),
            Node {
                flex_grow: 1.0,
                ..default()
            },
            ChildOf(frame),
        ))
        .observe(request_on_enter_or_escape)
        .id();
    let placeholder = commands
        .spawn((
            Text::new(field.placeholder),
            font,
            TextColor(MUTED),
            Node {
                position_type: PositionType::Absolute,
                left: px(SPACE_S),
                top: px(SPACE_S),
                ..default()
            },
            // a click on the placeholder reaches the field under it
            Pickable::IGNORE,
            ChildOf(frame),
        ))
        .id();
    let message = commands
        .spawn((
            Text::default(),
            TextFont {
                font: field.font.clone().into(),
                font_size: px(FONT_S).into(),
                ..default()
            },
            TextColor(DANGER),
        ))
        .id();
    if field.reserve_error {
        // a line of fixed height that clips the message: out of the flow, a long message neither
        // wraps onto a second line nor widens what holds the field
        let line = commands
            .spawn((
                Node {
                    height: px(ERROR_LINE_HEIGHT),
                    overflow: Overflow::clip(),
                    ..default()
                },
                ChildOf(root),
            ))
            .id();
        commands.entity(message).insert((
            LineHeight::Px(ERROR_LINE_HEIGHT),
            TextLayout {
                linebreak: LineBreak::NoWrap,
                ..default()
            },
            Node {
                position_type: PositionType::Absolute,
                left: px(0),
                top: px(0),
                ..default()
            },
            Visibility::Hidden,
            ChildOf(line),
        ));
    } else {
        commands.entity(message).insert((
            Node {
                display: Display::None,
                ..default()
            },
            ChildOf(root),
        ));
    }
    // a press on the bevel or the padding of the frame focuses the input, instead of bubbling up
    // to the window, which clears the focus
    commands.entity(frame).observe(
        move |mut acquire: On<AcquireFocus>, mut focus: ResMut<InputFocus>| {
            acquire.propagate(false);
            if focus.get() != Some(input) {
                focus.set(input, FocusCause::Pressed);
            }
        },
    );
    commands.entity(input).insert(FieldParts {
        frame,
        placeholder,
        message,
        reserve_error: field.reserve_error,
    });
    TextFieldParts { root, input }
}

/// The systems of the text fields: they trigger `Submit` and `Cancel` in `PostUpdate`.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub(super) struct TextFieldSystems;

pub(super) fn plugin(app: &mut App) {
    app.add_systems(
        PostUpdate,
        (emit_requests, show_placeholders, show_errors)
            .in_set(TextFieldSystems)
            .after(EditableTextSystems)
            .before(UiSystems::Layout),
    );
}

/// Whether the focus is in a text field: it takes Escape for itself, so the Escape handlers of the
/// screen behind it stand down.
pub fn text_field_focused(focus: Res<InputFocus>, fields: Query<(), With<EditableText>>) -> bool {
    focus.get().is_some_and(|entity| fields.contains(entity))
}

/// Replaces the value of a field with `value`.
pub fn set_text(text: &mut EditableText, value: &str) {
    text.queue_edit(TextEdit::SelectAll);
    text.queue_edit(TextEdit::Insert(value.into()));
}

/// Asks for a Submit on Enter and for a Cancel on Escape. They are triggered in `PostUpdate`,
/// after the edits of this update: a character typed in the same update as Enter is submitted,
/// and the systems of `Update` still see the field focused in the update of the key.
fn request_on_enter_or_escape(
    mut event: On<FocusedInput<KeyboardInput>>,
    texts: Query<&EditableText>,
    mut commands: Commands,
) {
    let key = &event.input;
    if key.state != ButtonState::Pressed || key.repeat {
        return;
    }
    let field = event.focused_entity;
    let Ok(text) = texts.get(field) else {
        return;
    };
    // while the IME composes, Enter and Escape belong to it: they bubble on, untouched
    if text.is_composing() {
        return;
    }
    let request = match key.logical_key {
        Key::Enter => FieldRequest::Submit,
        Key::Escape => FieldRequest::Cancel,
        _ => return,
    };
    event.propagate(false);
    commands.entity(field).insert(request);
}

/// Triggers the requested Submits and Cancels, once `EditableTextSystems` has applied the edits of
/// this update, so that a Submit carries the characters typed in the same update as Enter.
fn emit_requests(requests: Query<(Entity, &FieldRequest, &EditableText)>, mut commands: Commands) {
    for (entity, request, text) in &requests {
        commands.entity(entity).remove::<FieldRequest>();
        match request {
            FieldRequest::Submit => commands.trigger(Submit {
                entity,
                value: text.value().to_string(),
            }),
            FieldRequest::Cancel => commands.trigger(Cancel { entity }),
        }
    }
}

/// Shows the placeholder of each field while it is empty.
fn show_placeholders(
    fields: Query<(&EditableText, &FieldParts), Changed<EditableText>>,
    mut nodes: Query<&mut Node>,
) {
    for (text, parts) in &fields {
        let display = if text.value() == "" {
            Display::Flex
        } else {
            Display::None
        };
        if let Ok(mut node) = nodes.get_mut(parts.placeholder) {
            if node.display != display {
                node.display = display;
            }
        }
    }
}

/// Shows the error of each field under it, and turns its bevel red while there is one. A field
/// that reserves its error line hides the message instead of removing its line.
fn show_errors(
    fields: Query<(&FieldError, &FieldParts), Changed<FieldError>>,
    mut nodes: Query<&mut Node>,
    mut visibilities: Query<&mut Visibility>,
    mut texts: Query<&mut Text>,
    mut borders: Query<&mut BorderColor>,
) {
    for (error, parts) in &fields {
        if let Ok(mut border) = borders.get_mut(parts.frame) {
            *border = match error.0 {
                Some(_) => BorderColor::all(DANGER),
                None => sunken_bevel(),
            };
        }
        if let Some(message) = &error.0 {
            if let Ok(mut text) = texts.get_mut(parts.message) {
                text.0.clone_from(message);
            }
        }
        if parts.reserve_error {
            if let Ok(mut visibility) = visibilities.get_mut(parts.message) {
                *visibility = if error.0.is_some() {
                    Visibility::Inherited
                } else {
                    Visibility::Hidden
                };
            }
        } else if let Ok(mut node) = nodes.get_mut(parts.message) {
            node.display = if error.0.is_some() {
                Display::Flex
            } else {
                Display::None
            };
        }
    }
}

#[cfg(test)]
mod tests {
    use bevy::ecs::system::RunSystemOnce;

    use super::*;

    #[test]
    fn a_focused_text_field_is_seen_and_nothing_else_is() {
        let mut world = World::new();
        world.init_resource::<InputFocus>();
        let field = world.spawn(EditableText::default()).id();
        let other = world.spawn_empty().id();
        let focused = |world: &mut World| world.run_system_once(text_field_focused).unwrap();
        assert!(!focused(&mut world));
        world.insert_resource(InputFocus::from_entity(other));
        assert!(!focused(&mut world));
        world.insert_resource(InputFocus::from_entity(field));
        assert!(focused(&mut world));
    }
}
