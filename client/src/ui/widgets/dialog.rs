//! The modal dialog: a title, a line of body and a row of buttons over a dimmed backdrop.
//!
//! `spawn_dialog` builds it; observe `DialogClosed` (a global observer) to learn which action
//! closed it, or that Escape did. The focus stays inside while it is open and goes back to where
//! it was when it closes. Its own key hints replace those of the screen while it is open.
//!
//! `spawn_form_dialog` adds text fields above the buttons: observe `DialogSubmit` and close it
//! with `close_dialog` once the submission succeeded.

use accesskit::Role;
use bevy::{
    a11y::AccessibilityNode,
    input::{
        keyboard::{Key, KeyboardInput},
        ButtonState,
    },
    input_focus::{
        tab_navigation::TabGroup, AcquireFocus, AutoFocus, FocusCause, FocusedInput, InputFocus,
    },
    prelude::*,
    ui::{FocusPolicy, InteractionDisabled},
    ui_widgets::Activate,
};

use super::button::{button, ButtonVariant};
use super::hints::{hint_bar, Hint, HintBar};
use super::text_field::{Cancel, Submit, TextFieldParts, TextFieldSystems};
use crate::ui::theme::{
    BEVEL_HI, BEVEL_LO, BEVEL_WIDTH, FONT_L, FONT_M, FONT_S, MUTED, PANEL, SPACE_L, SPACE_M,
    SPACE_S, SPACE_XL, SPACE_XS, TEXT,
};

/// The z-index of a dialog: over the screens and the in-game overlays.
pub const DIALOG_Z: i32 = 50;

/// What `spawn_dialog` builds.
pub struct Dialog<'a> {
    pub title: &'a str,
    /// An empty body shows nothing.
    pub body: &'a str,
    /// The buttons from left to right: the primary action last.
    pub actions: &'a [(&'a str, ButtonVariant)],
    /// The index of the button focused on open: by convention Cancel.
    pub focused: usize,
}

/// The entities of a dialog: its root, and its buttons in the order of its actions.
pub struct DialogParts {
    pub root: Entity,
    #[allow(dead_code)] // disabled by the New world dialog (Lot 2, Task 4)
    pub actions: Vec<Entity>,
}

/// The root of an open dialog.
#[derive(Component)]
pub struct DialogRoot {
    /// Where the focus goes back to when the dialog closes, if that still exists.
    restore_focus: Option<Entity>,
    /// The index of the primary action: the last one.
    primary: usize,
}

/// Triggered on the root of a dialog when it closes: `Some(i)` for the action `i`, `None` for
/// Escape.
#[derive(EntityEvent)]
pub struct DialogClosed {
    pub entity: Entity,
    pub action: Option<usize>,
}

/// Triggered on the root of a form dialog when Enter is pressed in one of its fields or its
/// primary (last) action is activated, unless that action has `InteractionDisabled`. The dialog
/// stays open: the caller closes it with `close_dialog` once the submission succeeded.
#[derive(EntityEvent)]
pub struct DialogSubmit {
    pub entity: Entity,
}

/// The key hints of every dialog. None has an arrow, so the key caps need no glyph font.
const DIALOG_HINTS: [Hint; 3] = [
    Hint {
        keys: "Tab",
        action: "Move",
    },
    Hint {
        keys: "Enter",
        action: "Select",
    },
    Hint {
        keys: "Esc",
        action: "Cancel",
    },
];

/// The hint bar of a dialog, which hides the hint bars of the screens while it is there.
#[derive(Component)]
struct DialogHints;

/// Asks to close the dialog. It is carried out in `PostUpdate`, so that the systems of `Update`
/// still see the dialog open in the update of the key.
#[derive(Component)]
struct CloseRequest {
    /// The action it closes with, `None` for Escape.
    action: Option<usize>,
    /// Whether the focus goes back where it was before the dialog opened.
    restore_focus: bool,
}

impl CloseRequest {
    fn new(action: Option<usize>) -> Self {
        Self {
            action,
            restore_focus: true,
        }
    }
}

pub(super) fn plugin(app: &mut App) {
    // after the fields: Escape in a field of a form closes it in the update of the key
    app.add_systems(
        PostUpdate,
        (close_dialogs, hide_screen_hints)
            .chain()
            .after(TextFieldSystems),
    );
}

/// Whether a dialog is open: the Escape handlers of the screens behind it stand down.
pub fn dialog_open(dialogs: Query<(), With<DialogRoot>>) -> bool {
    !dialogs.is_empty()
}

/// Spawns `dialog` over the whole window and returns its root. `focus` is the current focus,
/// given back when the dialog closes.
pub fn spawn_dialog(
    commands: &mut Commands,
    dialog: &Dialog,
    font: Handle<Font>,
    focus: &InputFocus,
) -> Entity {
    debug_assert!(dialog.focused < dialog.actions.len());
    build_dialog(commands, dialog, &[], &DIALOG_HINTS, font, focus).root
}

/// A dialog whose `fields` (caption, field) sit between its body and its actions. The first
/// field has the focus (`dialog.focused` is not used); `hints` replace the dialog's own hints.
#[allow(dead_code)] // opens the New world dialog (Lot 2, Task 4)
pub fn spawn_form_dialog(
    commands: &mut Commands,
    dialog: &Dialog,
    fields: &[(&str, TextFieldParts)],
    hints: &[Hint],
    font: Handle<Font>,
    focus: &InputFocus,
) -> DialogParts {
    debug_assert!(!fields.is_empty() && !dialog.actions.is_empty());
    build_dialog(commands, dialog, fields, hints, font, focus)
}

/// Closes the dialog in `PostUpdate` with `DialogClosed { action: Some(last) }`; the focus goes
/// back where it was only when `restore_focus` (the caller may have focused something new).
#[allow(dead_code)] // closes the New world dialog (Lot 2, Task 4)
pub fn close_dialog(commands: &mut Commands, root: Entity, restore_focus: bool) {
    commands
        .entity(root)
        .queue(move |mut dialog: EntityWorldMut| {
            let action = dialog.get::<DialogRoot>().map(|root| root.primary);
            dialog.insert(CloseRequest {
                action,
                restore_focus,
            });
        });
}

/// Spawns a dialog, a form one when there are `fields`: `spawn_dialog` and `spawn_form_dialog`.
fn build_dialog(
    commands: &mut Commands,
    dialog: &Dialog,
    fields: &[(&str, TextFieldParts)],
    hints: &[Hint],
    font: Handle<Font>,
    focus: &InputFocus,
) -> DialogParts {
    let primary = dialog.actions.len().saturating_sub(1);
    let root = commands
        .spawn((
            DialogRoot {
                restore_focus: focus.get(),
                primary,
            },
            // the role before the label: the label's default node must not win
            AccessibilityNode(accesskit::Node::new(Role::Dialog)),
            AccessibleLabel::new(dialog.title),
            TabGroup::modal(),
            GlobalZIndex(DIALOG_Z),
            Node {
                position_type: PositionType::Absolute,
                width: percent(100),
                height: percent(100),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
        ))
        .observe(escape_closes)
        .id();
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            width: percent(100),
            height: percent(100),
            ..default()
        },
        BackgroundColor(BEVEL_LO.with_alpha(0.6)),
        FocusPolicy::Block,
        ChildOf(root),
    ));
    commands.spawn((
        hint_bar(hints, font.clone(), font.clone()),
        DialogHints,
        ChildOf(root),
    ));
    let panel = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(px(SPACE_XL)),
                row_gap: px(SPACE_L),
                border: UiRect::all(px(BEVEL_WIDTH)),
                min_width: px(360),
                max_width: percent(90),
                ..default()
            },
            BackgroundColor(PANEL),
            BorderColor {
                top: BEVEL_HI,
                left: BEVEL_HI,
                bottom: BEVEL_LO,
                right: BEVEL_LO,
            },
            ChildOf(root),
        ))
        .id();
    let text = |content: &str, size: f32, color: Color| {
        (
            Text::new(content),
            TextFont {
                font: font.clone().into(),
                font_size: px(size).into(),
                ..default()
            },
            TextColor(color),
        )
    };
    commands.spawn((text(dialog.title, FONT_L, TEXT), ChildOf(panel)));
    if !dialog.body.is_empty() {
        commands.spawn((text(dialog.body, FONT_M, MUTED), ChildOf(panel)));
    }
    if !fields.is_empty() {
        let form = commands
            .spawn((
                Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: px(SPACE_M),
                    ..default()
                },
                ChildOf(panel),
            ))
            .id();
        for (caption, field) in fields {
            let column = commands
                .spawn((
                    Node {
                        flex_direction: FlexDirection::Column,
                        row_gap: px(SPACE_XS),
                        ..default()
                    },
                    ChildOf(form),
                ))
                .id();
            commands.spawn((text(caption, FONT_S, MUTED), ChildOf(column)));
            commands.entity(field.root).insert(ChildOf(column));
        }
    }
    let actions = commands
        .spawn((
            Node {
                justify_content: JustifyContent::FlexEnd,
                column_gap: px(SPACE_S),
                ..default()
            },
            ChildOf(panel),
        ))
        .id();
    let mut buttons = Vec::with_capacity(dialog.actions.len());
    for (label, variant) in dialog.actions {
        buttons.push(
            commands
                .spawn((button(label, *variant, font.clone()), ChildOf(actions)))
                .id(),
        );
    }
    let primary_button = buttons[primary];
    for (index, &action) in buttons.iter().enumerate() {
        // the primary action of a form submits it, the others close the dialog
        if !fields.is_empty() && index == primary {
            commands.entity(action).observe(
                move |_: On<Activate>,
                      disabled: Query<(), With<InteractionDisabled>>,
                      mut commands: Commands| {
                    submit(root, primary_button, &disabled, &mut commands);
                },
            );
        } else {
            commands
                .entity(action)
                .observe(move |_: On<Activate>, mut commands: Commands| {
                    commands.entity(root).insert(CloseRequest::new(Some(index)));
                });
        }
    }
    // the focus goes to the first field of a form, else to the chosen button
    let first = match fields.first() {
        Some((_, field)) => field.input,
        None => buttons[dialog.focused],
    };
    commands.entity(first).insert(AutoFocus);
    for (_, field) in fields {
        commands
            .entity(field.input)
            .observe(
                move |_: On<Submit>,
                      disabled: Query<(), With<InteractionDisabled>>,
                      mut commands: Commands| {
                    submit(root, primary_button, &disabled, &mut commands);
                },
            )
            .observe(move |_: On<Cancel>, mut commands: Commands| {
                commands.entity(root).insert(CloseRequest::new(None));
            });
    }
    // a click that no button took (a button stops the event) keeps the focus on the dialog
    commands.entity(root).observe(
        move |mut event: On<AcquireFocus>,
              parents: Query<&ChildOf>,
              mut focus: ResMut<InputFocus>| {
            event.propagate(false);
            let inside =
                std::iter::successors(focus.get(), |e| parents.get(*e).ok().map(ChildOf::parent))
                    .any(|e| e == root);
            if !inside {
                focus.set(first, FocusCause::Pressed);
            }
        },
    );
    DialogParts {
        root,
        actions: buttons,
    }
}

/// Triggers `DialogSubmit` on the form dialog `root`, unless its `primary` action is disabled.
fn submit(
    root: Entity,
    primary: Entity,
    disabled: &Query<(), With<InteractionDisabled>>,
    commands: &mut Commands,
) {
    if !disabled.contains(primary) {
        commands.trigger(DialogSubmit { entity: root });
    }
}

/// Asks to close the dialog on Escape, which stops there: nothing behind the dialog sees it.
fn escape_closes(mut event: On<FocusedInput<KeyboardInput>>, mut commands: Commands) {
    let key = &event.input;
    if key.state != ButtonState::Pressed || key.repeat || key.logical_key != Key::Escape {
        return;
    }
    event.propagate(false);
    commands
        .entity(event.event_target())
        .insert(CloseRequest::new(None));
}

/// Closes the dialogs that asked to: triggers `DialogClosed`, despawns the root and puts the
/// focus back where it was, if that still exists and the request says so.
fn close_dialogs(
    requests: Query<(Entity, &DialogRoot, &CloseRequest)>,
    existing: Query<()>,
    mut focus: ResMut<InputFocus>,
    mut commands: Commands,
) {
    for (root, dialog, request) in &requests {
        commands.trigger(DialogClosed {
            entity: root,
            action: request.action,
        });
        commands.entity(root).despawn();
        if !request.restore_focus {
            continue;
        }
        if let Some(previous) = dialog.restore_focus.filter(|e| existing.contains(*e)) {
            focus.set(previous, FocusCause::Navigated);
        }
    }
}

/// Hides the hint bars of the screens while a dialog shows its own, and shows them again when
/// the last dialog has closed.
fn hide_screen_hints(
    dialogs: Query<(), With<DialogRoot>>,
    mut bars: Query<&mut Visibility, (With<HintBar>, Without<DialogHints>)>,
) {
    let visibility = if dialogs.is_empty() {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    for mut bar in &mut bars {
        bar.set_if_neq(visibility);
    }
}

#[cfg(test)]
mod tests {
    use bevy::ecs::world::CommandQueue;

    use super::*;

    #[test]
    fn a_dialog_has_the_dialog_role_and_its_title() {
        let mut world = World::new();
        let mut queue = CommandQueue::default();
        let dialog = Dialog {
            title: "Quit Rustcraft?",
            body: "",
            actions: &[("Cancel", ButtonVariant::Default)],
            focused: 0,
        };
        let root = spawn_dialog(
            &mut Commands::new(&mut queue, &world),
            &dialog,
            Handle::default(),
            &InputFocus::default(),
        );
        queue.apply(&mut world);
        let node = world.get::<AccessibilityNode>(root).unwrap();
        assert_eq!(node.role(), Role::Dialog);
        assert_eq!(node.label(), Some("Quit Rustcraft?"));
    }
}
