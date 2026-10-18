//! The modal dialog: a title, a line of body and a row of buttons over a dimmed backdrop.
//!
//! `spawn_dialog` builds it; observe `DialogClosed` (a global observer) to learn which action
//! closed it, or that Escape did. The focus stays inside while it is open and goes back to where
//! it was when it closes.

use accesskit::Role;
use bevy::{
    a11y::AccessibilityNode,
    input::{
        keyboard::{Key, KeyboardInput},
        ButtonState,
    },
    input_focus::{
        tab_navigation::{TabGroup, TabIndex},
        AutoFocus, FocusCause, FocusedInput, InputFocus,
    },
    prelude::*,
    ui::FocusPolicy,
    ui_widgets::Activate,
};

use super::button::{button, ButtonVariant};
use crate::ui::theme::{
    BEVEL_HI, BEVEL_LO, BEVEL_WIDTH, FONT_L, FONT_M, MUTED, PANEL, SPACE_L, SPACE_S, SPACE_XL, TEXT,
};

/// The z-index of a dialog: over the screens and the in-game overlays.
// First used by the main menu (Lot 1, Task 3).
#[allow(dead_code)]
pub const DIALOG_Z: i32 = 50;

/// What `spawn_dialog` builds.
// First used by the main menu (Lot 1, Task 3).
#[allow(dead_code)]
pub struct Dialog<'a> {
    pub title: &'a str,
    /// An empty body shows nothing.
    pub body: &'a str,
    /// The buttons from left to right: the primary action last.
    pub actions: &'a [(&'a str, ButtonVariant)],
    /// The index of the button focused on open: by convention Cancel.
    pub focused: usize,
}

/// The root of an open dialog.
#[derive(Component)]
pub struct DialogRoot {
    /// Where the focus goes back to when the dialog closes, if that still exists.
    restore_focus: Option<Entity>,
}

/// Triggered on the root of a dialog when it closes: `Some(i)` for the action `i`, `None` for
/// Escape.
#[derive(EntityEvent)]
pub struct DialogClosed {
    pub entity: Entity,
    // first read by the main menu (Lot 1, Task 3)
    #[allow(dead_code)]
    pub action: Option<usize>,
}

/// Asks to close the dialog, with this action. It is carried out in `PostUpdate`, so that the
/// systems of `Update` still see the dialog open in the update of the key.
#[derive(Component)]
struct CloseRequest(Option<usize>);

pub(super) fn plugin(app: &mut App) {
    app.add_systems(PostUpdate, close_dialogs);
}

/// Whether a dialog is open: the Escape handlers of the screens behind it stand down.
// First used by the main menu (Lot 1, Task 3).
#[allow(dead_code)]
pub fn dialog_open(dialogs: Query<(), With<DialogRoot>>) -> bool {
    !dialogs.is_empty()
}

/// Spawns `dialog` over the whole window and returns its root. `focus` is the current focus,
/// given back when the dialog closes.
// First used by the main menu (Lot 1, Task 3).
#[allow(dead_code)]
pub fn spawn_dialog(
    commands: &mut Commands,
    dialog: &Dialog,
    font: Handle<Font>,
    focus: &InputFocus,
) -> Entity {
    let root = commands
        .spawn((
            DialogRoot {
                restore_focus: focus.get(),
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
        TabIndex(-1),
        FocusPolicy::Block,
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
    for (index, (label, variant)) in dialog.actions.iter().enumerate() {
        let mut action = commands.spawn((button(label, *variant, font.clone()), ChildOf(actions)));
        if index == dialog.focused {
            action.insert(AutoFocus);
        }
        action.observe(move |_: On<Activate>, mut commands: Commands| {
            commands.entity(root).insert(CloseRequest(Some(index)));
        });
    }
    root
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
        .insert(CloseRequest(None));
}

/// Closes the dialogs that asked to: triggers `DialogClosed`, despawns the root and puts the
/// focus back where it was, if that still exists.
fn close_dialogs(
    requests: Query<(Entity, &DialogRoot, &CloseRequest)>,
    existing: Query<()>,
    mut focus: ResMut<InputFocus>,
    mut commands: Commands,
) {
    for (root, dialog, request) in &requests {
        commands.trigger(DialogClosed {
            entity: root,
            action: request.0,
        });
        commands.entity(root).despawn();
        if let Some(previous) = dialog.restore_focus.filter(|e| existing.contains(*e)) {
            focus.set(previous, FocusCause::Navigated);
        }
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
