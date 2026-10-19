//! The modal dialog: a title, a line of body and a row of buttons over a dimmed backdrop.
//!
//! `spawn_dialog` builds it; observe `DialogClosed` (a global observer) to learn which action
//! closed it, or that Escape did. The focus stays inside while it is open and goes back to where
//! it was when it closes. Its own key hints replace those of the screen while it is open.
//!
//! `spawn_form_dialog` adds text fields above the buttons: observe `DialogSubmit` and close it
//! with `close_dialog` once the submission succeeded. Observe `FormChanged` to check the fields
//! as they are typed, and show the result with `apply_form_check`.

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
    text::EditableText,
    ui::{FocusPolicy, InteractionDisabled},
    ui_widgets::Activate,
};

use super::button::{button, ButtonVariant};
use super::hints::{hint_bar, Hint, HintBar};
use super::text_field::{Cancel, FieldError, Submit, TextFieldParts, TextFieldSystems};
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
    pub actions: Vec<Entity>,
}

/// The root of an open dialog.
#[derive(Component)]
pub struct DialogRoot {
    /// Where the focus goes back to when the dialog closes, if that still exists.
    restore_focus: Option<Entity>,
    /// The index of the primary action: the last one, if there is any.
    primary: Option<usize>,
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

/// Triggered on the root of a form dialog once its fields hold their first values, then each
/// time a value changed, never while a field has pending edits. `values` and `touched` follow
/// the order of the fields; a field is touched once its value differed from its first value.
#[derive(EntityEvent)]
pub struct FormChanged {
    pub entity: Entity,
    pub values: Vec<String>,
    pub touched: Vec<bool>,
}

/// The fields of a form dialog and their values, on its root. Opaque outside: it is only named
/// by `notice_form_changes`, which the tests of the screens add to a bare app.
#[derive(Component)]
pub(crate) struct FormFields {
    /// The inputs of the fields, in order.
    inputs: Vec<Entity>,
    /// The action that submits the form.
    primary: Option<Entity>,
    /// The values of the fields once none had pending edits any more, `None` before.
    first: Option<Vec<String>>,
    /// The values of the fields when `FormChanged` was last triggered, `None` before the first.
    /// `EditableText` is changed on every frame by its layout, so the values themselves tell
    /// when they changed; starting from `None` rather than from empty values, a form opened
    /// with empty fields is checked at once.
    checked: Option<Vec<String>>,
    /// Whether each field's value ever differed from its first value.
    touched: Vec<bool>,
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
/// still see the dialog open in the update of the key. A dialog with it is closing: a second
/// submission in the same update has nothing left to do.
#[derive(Component)]
pub struct CloseRequest {
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
    )
    .add_systems(Update, notice_form_changes);
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
/// back where it was only when `restore_focus`. With `false` the caller sets the focus itself,
/// typically on what the submission made.
pub fn close_dialog(commands: &mut Commands, root: Entity, restore_focus: bool) {
    commands
        .entity(root)
        .queue(move |mut dialog: EntityWorldMut| {
            let action = dialog.get::<DialogRoot>().and_then(|root| root.primary);
            dialog.insert(CloseRequest {
                action,
                restore_focus,
            });
        });
}

/// Shows `errors` under the fields of the form `root` (one per field, `None`: no message), and
/// disables its primary action unless `submittable`. Changes only what differs.
pub fn apply_form_check(
    commands: &mut Commands,
    root: Entity,
    errors: Vec<Option<&'static str>>,
    submittable: bool,
) {
    commands.queue(move |world: &mut World| {
        let Some(form) = world.get::<FormFields>(root) else {
            return;
        };
        let (inputs, primary) = (form.inputs.clone(), form.primary);
        debug_assert_eq!(errors.len(), inputs.len(), "one error per field");
        for (input, message) in inputs.into_iter().zip(errors) {
            if let Some(mut error) = world.get_mut::<FieldError>(input) {
                if error.0.as_deref() != message {
                    error.0 = message.map(str::to_string);
                }
            }
        }
        let Some(mut primary) = primary.and_then(|primary| world.get_entity_mut(primary).ok())
        else {
            return;
        };
        if submittable == primary.contains::<InteractionDisabled>() {
            if submittable {
                primary.remove::<InteractionDisabled>();
            } else {
                primary.insert(InteractionDisabled);
            }
        }
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
    let primary = dialog.actions.len().checked_sub(1);
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
            // a word wider than the dialog, such as a long address, breaks rather than leaves it
            TextLayout::linebreak(LineBreak::WordOrCharacter),
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
    let primary_button = primary.map(|index| buttons[index]);
    if !fields.is_empty() {
        commands.entity(root).insert(FormFields {
            inputs: fields.iter().map(|(_, field)| field.input).collect(),
            primary: primary_button,
            first: None,
            checked: None,
            touched: vec![false; fields.len()],
        });
    }
    for (index, &action) in buttons.iter().enumerate() {
        // the primary action of a form submits it, the others close the dialog
        if !fields.is_empty() && Some(index) == primary {
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
    // the focus goes to the first field of a form, else to the chosen button, if it exists
    let first = match fields.first() {
        Some((_, field)) => Some(field.input),
        None => buttons.get(dialog.focused).copied(),
    };
    if let Some(first) = first {
        commands.entity(first).insert(AutoFocus);
    }
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
                if let Some(chosen) = first {
                    focus.set(chosen, FocusCause::Pressed);
                }
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
    primary: Option<Entity>,
    disabled: &Query<(), With<InteractionDisabled>>,
    commands: &mut Commands,
) {
    if !primary.is_some_and(|primary| disabled.contains(primary)) {
        commands.trigger(DialogSubmit { entity: root });
    }
}

/// Triggers `FormChanged` on each form dialog whose values changed since it was last triggered,
/// unless one of its fields has edits waiting: they are applied in `PostUpdate`, so its value is
/// about to change, and a field filled on open would be checked empty for a frame.
pub(crate) fn notice_form_changes(
    mut forms: Query<(Entity, &mut FormFields)>,
    texts: Query<&EditableText>,
    mut commands: Commands,
) {
    for (root, mut form) in &mut forms {
        let pending = form.inputs.iter().any(|&input| {
            texts
                .get(input)
                .is_ok_and(|text| !text.pending_edits.is_empty())
        });
        if pending {
            continue;
        }
        let values: Vec<String> = form
            .inputs
            .iter()
            .map(|&input| {
                texts
                    .get(input)
                    .map(|text| text.value().to_string())
                    .unwrap_or_default()
            })
            .collect();
        if form.checked.as_ref() == Some(&values) {
            continue;
        }
        let form = &mut *form;
        let first = form.first.get_or_insert_with(|| values.clone());
        for ((touched, value), first) in form.touched.iter_mut().zip(&values).zip(first) {
            *touched |= value != first;
        }
        form.checked = Some(values.clone());
        commands.trigger(FormChanged {
            entity: root,
            values,
            touched: form.touched.clone(),
        });
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
    nodes: Query<(&Node, Option<&ChildOf>)>,
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
        // a node gone or hidden cannot take the focus back: nothing has it, and the screen's own
        // rule chooses
        match dialog.restore_focus.filter(|e| shown(*e, &nodes)) {
            Some(previous) => focus.set(previous, FocusCause::Navigated),
            None => focus.clear(),
        }
    }
}

/// Whether `entity` exists and neither it nor an ancestor is `Display::None`.
fn shown(entity: Entity, nodes: &Query<(&Node, Option<&ChildOf>)>) -> bool {
    let mut current = entity;
    loop {
        let Ok((node, parent)) = nodes.get(current) else {
            return false;
        };
        if node.display == Display::None {
            return false;
        }
        match parent {
            Some(parent) => current = parent.parent(),
            None => return true,
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
    use bevy::text::TextEdit;

    use super::*;
    use crate::ui::widgets::text_field::{spawn_text_field, TextField};

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

    /// The builder behind `spawn_dialog`, past its `debug_assert`: a release build does not panic.
    #[test]
    fn a_dialog_without_actions_or_with_a_focus_out_of_range_builds_without_focus() {
        for actions in [&[][..], &[("Cancel", ButtonVariant::Default)][..]] {
            let mut world = World::new();
            let mut queue = CommandQueue::default();
            let dialog = Dialog {
                title: "Probe?",
                body: "",
                actions,
                focused: 1,
            };
            let parts = build_dialog(
                &mut Commands::new(&mut queue, &world),
                &dialog,
                &[],
                &DIALOG_HINTS,
                Handle::default(),
                &InputFocus::default(),
            );
            queue.apply(&mut world);
            assert_eq!(parts.actions.len(), actions.len());
            assert_eq!(
                world.get::<DialogRoot>(parts.root).unwrap().primary,
                actions.len().checked_sub(1)
            );
            let mut autofocus = world.query_filtered::<(), With<AutoFocus>>();
            assert_eq!(autofocus.iter(&world).count(), 0);
        }
    }

    /// The `FormChanged` triggered, in order: their values and touched fields.
    #[derive(Resource, Default)]
    struct Changes(Vec<(Vec<String>, Vec<bool>)>);

    /// A bare app noticing the changes of a form dialog whose fields hold `""` and a pending
    /// "Fresh". Returns the app, the dialog and its two inputs.
    fn form_app() -> (App, DialogParts, [Entity; 2]) {
        let mut app = App::new();
        app.init_resource::<Changes>()
            .add_systems(Update, notice_form_changes)
            .add_observer(|changed: On<FormChanged>, mut changes: ResMut<Changes>| {
                changes
                    .0
                    .push((changed.values.clone(), changed.touched.clone()));
            });
        let world = app.world_mut();
        let mut commands = world.commands();
        let mut field = |label| {
            spawn_text_field(
                &mut commands,
                &TextField {
                    label,
                    placeholder: "",
                    max_characters: None,
                    font: Handle::default(),
                    font_size: FONT_M,
                    reserve_error: true,
                },
            )
        };
        let (name, seed) = (field("Name"), field("Seed"));
        let dialog = Dialog {
            title: "Probe form",
            body: "",
            actions: &[
                ("Cancel", ButtonVariant::Default),
                ("Create", ButtonVariant::Primary),
            ],
            focused: 0,
        };
        let parts = spawn_form_dialog(
            &mut commands,
            &dialog,
            &[("Name", name), ("Seed", seed)],
            &DIALOG_HINTS,
            Handle::default(),
            &InputFocus::default(),
        );
        world.flush();
        world
            .get_mut::<EditableText>(seed.input)
            .unwrap()
            .queue_edit(TextEdit::Insert("Fresh".into()));
        (app, parts, [name.input, seed.input])
    }

    fn changes(app: &App) -> &[(Vec<String>, Vec<bool>)] {
        &app.world().resource::<Changes>().0
    }

    /// A field's text holding `value` with nothing pending: `EditableText::new` queues a move of
    /// the cursor, which the bare app never applies.
    fn settled(value: &str) -> EditableText {
        let mut text = EditableText::new(value);
        text.pending_edits.clear();
        text
    }

    fn values(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| value.to_string()).collect()
    }

    #[test]
    fn a_form_is_checked_once_its_fields_hold_their_first_values() {
        let (mut app, _, [_, seed]) = form_app();
        // the bare app applies no edits: the second field stays pending
        app.update();
        assert!(changes(&app).is_empty());
        app.world_mut().entity_mut(seed).insert(settled("Fresh"));
        app.update();
        assert_eq!(
            changes(&app),
            &[(values(&["", "Fresh"]), vec![false, false])]
        );
        app.update();
        app.update();
        assert_eq!(changes(&app).len(), 1);
    }

    #[test]
    fn a_changed_value_is_reported_and_stays_touched() {
        let (mut app, _, [_, seed]) = form_app();
        app.world_mut().entity_mut(seed).insert(settled("Fresh"));
        app.update();
        app.world_mut().entity_mut(seed).insert(settled("Fresh2"));
        app.update();
        app.world_mut().entity_mut(seed).insert(settled("Fresh"));
        app.update();
        assert_eq!(
            &changes(&app)[1..],
            &[
                (values(&["", "Fresh2"]), vec![false, true]),
                (values(&["", "Fresh"]), vec![false, true]),
            ]
        );
    }

    #[test]
    fn apply_form_check_sets_the_errors_and_the_primary() {
        let (mut app, parts, [name, seed]) = form_app();
        let primary = parts.actions[1];
        let world = app.world_mut();
        apply_form_check(
            &mut world.commands(),
            parts.root,
            vec![Some("Bad"), None],
            false,
        );
        world.flush();
        assert_eq!(
            world.get::<FieldError>(name).unwrap().0.as_deref(),
            Some("Bad")
        );
        assert_eq!(world.get::<FieldError>(seed).unwrap().0, None);
        assert!(world.get::<InteractionDisabled>(primary).is_some());
        apply_form_check(&mut world.commands(), parts.root, vec![None, None], true);
        world.flush();
        assert_eq!(world.get::<FieldError>(name).unwrap().0, None);
        assert_eq!(world.get::<FieldError>(seed).unwrap().0, None);
        assert!(world.get::<InteractionDisabled>(primary).is_none());
    }

    /// Closes a dialog that restores the focus to `previous`, spawned by `spawn_previous`, while
    /// the dialog itself has the focus. Returns the focus afterwards, and `previous`.
    fn close_with_focus_on(
        spawn_previous: impl FnOnce(&mut World) -> Entity,
    ) -> (Option<Entity>, Entity) {
        let mut app = App::new();
        app.init_resource::<InputFocus>()
            .add_systems(Update, close_dialogs);
        let world = app.world_mut();
        let previous = spawn_previous(world);
        let dialog = world
            .spawn((
                DialogRoot {
                    restore_focus: Some(previous),
                    primary: None,
                },
                CloseRequest::new(None),
            ))
            .id();
        world
            .resource_mut::<InputFocus>()
            .set(dialog, FocusCause::Navigated);
        app.update();
        (app.world().resource::<InputFocus>().get(), previous)
    }

    #[test]
    fn closing_a_dialog_gives_the_focus_back_to_a_shown_node() {
        let (focus, previous) = close_with_focus_on(|world| {
            let screen = world.spawn(Node::default()).id();
            world.spawn((Node::default(), ChildOf(screen))).id()
        });
        assert_eq!(focus, Some(previous));
    }

    #[test]
    fn closing_a_dialog_clears_the_focus_when_the_node_is_hidden_or_gone() {
        let hidden = Node {
            display: Display::None,
            ..default()
        };
        // itself hidden
        let (focus, _) = close_with_focus_on(|world| world.spawn(hidden.clone()).id());
        assert_eq!(focus, None);
        // an ancestor hidden
        let (focus, _) = close_with_focus_on(|world| {
            let pane = world.spawn(hidden.clone()).id();
            world.spawn((Node::default(), ChildOf(pane))).id()
        });
        assert_eq!(focus, None);
        // gone
        let (focus, _) = close_with_focus_on(|world| {
            let gone = world.spawn(Node::default()).id();
            world.despawn(gone);
            gone
        });
        assert_eq!(focus, None);
    }
}
