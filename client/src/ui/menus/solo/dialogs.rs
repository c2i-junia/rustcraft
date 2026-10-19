//! The dialogs of the world list. New world checks the name and the seed as they are typed, and
//! writes the world to disk as soon as it is submitted. Delete asks first.

use std::io;

use bevy::{
    input_focus::InputFocus,
    prelude::*,
    text::{EditableText, EditableTextFilter},
    ui::InteractionDisabled,
    ui_widgets::SelectAllOnFocus,
};
use shared::GameFolderPaths;

use super::super::worlds::{
    check_world_name, create_world, delete_world, format_size, free_world_name, parse_seed,
    rename_world, WorldSummary, MAX_WORLD_NAME_LENGTH,
};
use super::super::MenuState;
use super::WorldCard;
use crate::constants::SAVE_PATH;
use crate::ui::assets::FONT_PATH;
use crate::ui::theme::FONT_M;
use crate::ui::widgets::button::ButtonVariant;
use crate::ui::widgets::dialog::{
    apply_form_check, close_dialog, dialog_open, spawn_dialog, spawn_form_dialog, CloseRequest,
    Dialog, DialogClosed, DialogSubmit, FormChanged,
};
use crate::ui::widgets::hints::Hint;
use crate::ui::widgets::text_field::{
    set_text, spawn_text_field, text_field_focused, FieldError, TextField,
};

/// The title of the New world dialog.
pub const NEW_WORLD_TITLE: &str = "New world";

/// What the detail pane says when a world could not be deleted for lack of permission.
pub const DELETE_PERMISSION: &str = "No permission to delete this world";

/// The most digits a seed has: a `u32` has at most 10.
const SEED_DIGITS: usize = 10;

/// The key hints of the New world dialog.
const NEW_WORLD_HINTS: [Hint; 3] = [
    Hint {
        keys: "Tab",
        action: "Move",
    },
    Hint {
        keys: "Enter",
        action: "Create",
    },
    Hint {
        keys: "Esc",
        action: "Cancel",
    },
];

/// The key hints of the Rename dialog.
const RENAME_HINTS: [Hint; 3] = [
    Hint {
        keys: "Tab",
        action: "Move",
    },
    Hint {
        keys: "Enter",
        action: "Rename",
    },
    Hint {
        keys: "Esc",
        action: "Cancel",
    },
];

/// The input of the name field of a world dialog.
#[derive(Component)]
pub struct WorldNameInput;

/// The input of the seed field of the New world dialog.
#[derive(Component)]
pub struct WorldSeedInput;

/// The fields of an open world dialog and what they are checked against, on its root.
#[derive(Component)]
struct WorldForm {
    /// The input of the name field.
    name: Entity,
    /// The input of the seed field, if the dialog has one.
    seed: Option<Entity>,
    /// The names of the other worlds, which the name may not take.
    others: Vec<String>,
    /// The name of the world being renamed: kept unchanged, it is not an error, but there is
    /// nothing to submit.
    current: Option<String>,
}

impl WorldForm {
    fn new(
        name: Entity,
        seed: Option<Entity>,
        others: Vec<String>,
        current: Option<String>,
    ) -> Self {
        Self {
            name,
            seed,
            others,
            current,
        }
    }
}

/// Triggered once the worlds on disk changed: the screen lists them again and focuses the card
/// of `focus` (none, or a world no longer listed: the first card, or New world when there is
/// none). `error` then shows under the buttons of the detail pane.
#[derive(Event)]
pub struct WorldsChanged {
    pub focus: Option<String>,
    pub error: Option<&'static str>,
}

pub(super) fn plugin(app: &mut App) {
    app.add_systems(
        Update,
        open_new_world_on_ctrl_n
            .run_if(in_state(MenuState::Solo))
            .run_if(not(dialog_open))
            .run_if(not(text_field_focused)),
    );
}

/// Opens the New world dialog: its name field holds a free name, entirely selected, and its seed
/// field is empty for a random seed. `others` are the names of the worlds on disk. Returns the
/// root of the dialog.
pub fn open_new_world_dialog(
    commands: &mut Commands,
    assets: &AssetServer,
    focus: &InputFocus,
    others: Vec<String>,
) -> Entity {
    let font: Handle<Font> = assets.load(FONT_PATH);
    let mut field = |label, placeholder, max_characters| {
        spawn_text_field(
            commands,
            &TextField {
                label,
                placeholder,
                max_characters: Some(max_characters),
                font: font.clone(),
                font_size: FONT_M,
                reserve_error: true,
            },
        )
    };
    let name = field("Name", "", MAX_WORLD_NAME_LENGTH);
    let seed = field("Seed (optional)", "Random", SEED_DIGITS);
    let free = free_world_name(&others);
    commands
        .entity(name.input)
        .insert((WorldNameInput, SelectAllOnFocus))
        .entry::<EditableText>()
        .and_modify(move |mut text| set_text(&mut text, &free));
    commands.entity(seed.input).insert((
        WorldSeedInput,
        EditableTextFilter::new(|c: char| c.is_ascii_digit()),
    ));
    let dialog = Dialog {
        title: NEW_WORLD_TITLE,
        body: "",
        actions: &[
            ("Cancel", ButtonVariant::Default),
            ("Create", ButtonVariant::Primary),
        ],
        focused: 0,
    };
    let parts = spawn_form_dialog(
        commands,
        &dialog,
        &[("Name", name), ("Seed (optional)", seed)],
        &NEW_WORLD_HINTS,
        font,
        focus,
    );
    commands
        .entity(parts.root)
        .insert(WorldForm::new(name.input, Some(seed.input), others, None))
        .observe(check_world_form)
        .observe(create_world_on_submit);
    parts.root
}

/// Opens the Rename dialog of `world`: its name field holds the current name, entirely selected.
/// `others` are the names of the other worlds on disk. Rename stays disabled while the name is
/// unchanged. Returns the root of the dialog.
pub fn open_rename_dialog(
    commands: &mut Commands,
    assets: &AssetServer,
    focus: &InputFocus,
    world: &str,
    others: Vec<String>,
) -> Entity {
    let font: Handle<Font> = assets.load(FONT_PATH);
    let name = spawn_text_field(
        commands,
        &TextField {
            label: "Name",
            placeholder: "",
            max_characters: Some(MAX_WORLD_NAME_LENGTH),
            font: font.clone(),
            font_size: FONT_M,
            reserve_error: true,
        },
    );
    let current = world.to_string();
    commands
        .entity(name.input)
        .insert((WorldNameInput, SelectAllOnFocus))
        .entry::<EditableText>()
        .and_modify(move |mut text| set_text(&mut text, &current));
    let title = format!("Rename \"{world}\"");
    let dialog = Dialog {
        title: &title,
        body: "",
        actions: &[
            ("Cancel", ButtonVariant::Default),
            ("Rename", ButtonVariant::Primary),
        ],
        focused: 0,
    };
    let parts = spawn_form_dialog(
        commands,
        &dialog,
        &[("Name", name)],
        &RENAME_HINTS,
        font,
        focus,
    );
    // the name is unchanged until the first check, a few frames later
    commands
        .entity(parts.actions[1])
        .insert(InteractionDisabled);
    commands
        .entity(parts.root)
        .insert(WorldForm::new(
            name.input,
            None,
            others,
            Some(world.to_string()),
        ))
        .observe(check_world_form)
        .observe(rename_world_on_submit);
    parts.root
}

/// Opens the New world dialog on Ctrl+N, with the names of the listed worlds.
pub fn open_new_world_on_ctrl_n(
    keys: Res<ButtonInput<KeyCode>>,
    cards: Query<&WorldCard>,
    assets: Res<AssetServer>,
    focus: Res<InputFocus>,
    mut commands: Commands,
) {
    let control = keys.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight]);
    if control && keys.just_pressed(KeyCode::KeyN) {
        let others = cards.iter().map(|card| card.0.name.clone()).collect();
        open_new_world_dialog(&mut commands, &assets, &focus, others);
    }
}

/// The value of the text field `input`.
fn value_of(texts: &Query<&EditableText>, input: Entity) -> String {
    texts
        .get(input)
        .map(|text| text.value().to_string())
        .unwrap_or_default()
}

/// What is wrong with the name `typed` in `form`, and whether it can be submitted. The unchanged
/// name of a renamed world shows no message, yet cannot be submitted.
fn check_name(form: &WorldForm, typed: &str) -> (Option<&'static str>, bool) {
    if form.current.as_deref() == Some(typed.trim()) {
        return (None, false);
    }
    match check_world_name(typed, &form.others) {
        Ok(_) => (None, true),
        Err(message) => (Some(message), false),
    }
}

/// Checks the fields of a world dialog whose values changed: shows why a field is wrong under
/// it, and disables the primary action while one is. A message set by a failed submit stays
/// until a value changes.
fn check_world_form(changed: On<FormChanged>, forms: Query<&WorldForm>, mut commands: Commands) {
    let Ok(form) = forms.get(changed.entity) else {
        return;
    };
    let (name_error, mut submittable) = check_name(form, &changed.values[0]);
    let mut errors = vec![name_error];
    if form.seed.is_some() {
        let seed_error = parse_seed(&changed.values[1]).err();
        submittable &= seed_error.is_none();
        errors.push(seed_error);
    }
    apply_form_check(&mut commands, changed.entity, errors, submittable);
}

/// What the name field says when `create_world` failed: a short text by kind, since the error
/// line holds one line.
fn create_failure(error: &io::Error) -> &'static str {
    match error.kind() {
        io::ErrorKind::StorageFull => "The disk is full",
        io::ErrorKind::PermissionDenied | io::ErrorKind::ReadOnlyFilesystem => {
            "No permission to write here"
        }
        io::ErrorKind::AlreadyExists => "A folder with this name already exists",
        _ => "Could not create the world",
    }
}

/// What the name field says when `rename_world` failed: a short text by kind, like
/// `create_failure`.
fn rename_failure(error: &io::Error) -> &'static str {
    match error.kind() {
        io::ErrorKind::StorageFull => "The disk is full",
        io::ErrorKind::PermissionDenied | io::ErrorKind::ReadOnlyFilesystem => {
            "No permission to rename this world"
        }
        io::ErrorKind::AlreadyExists => "A folder with this name already exists",
        io::ErrorKind::ResourceBusy => "The world is in use",
        _ => "Could not rename the world",
    }
}

/// What the detail pane says when `delete_world` failed: a short text by kind, like
/// `create_failure`.
fn delete_failure(error: &io::Error) -> &'static str {
    match error.kind() {
        io::ErrorKind::PermissionDenied | io::ErrorKind::ReadOnlyFilesystem => DELETE_PERMISSION,
        io::ErrorKind::ResourceBusy => "The world is in use",
        _ => "Could not delete the world",
    }
}

/// Creates the world of the New world dialog on disk, checking its fields again, then closes it
/// and lists the worlds with the focus on the new one; on a failure the dialog stays open and
/// says why under the name. A dialog already closing was submitted before in this update.
fn create_world_on_submit(
    submit: On<DialogSubmit>,
    forms: Query<&WorldForm, Without<CloseRequest>>,
    texts: Query<&EditableText>,
    paths: Res<GameFolderPaths>,
    mut commands: Commands,
) {
    let root = submit.entity;
    let Ok(form) = forms.get(root) else {
        return;
    };
    let Ok(name) = check_world_name(&value_of(&texts, form.name), &form.others) else {
        return;
    };
    let seed = form
        .seed
        .map(|seed| value_of(&texts, seed))
        .unwrap_or_default();
    let Ok(seed) = parse_seed(&seed) else {
        return;
    };
    let saves = paths.game_folder_path.join(SAVE_PATH);
    match create_world(&saves, &name, seed.unwrap_or_else(rand::random)) {
        Ok(()) => {
            info!("Created the world {name:?}");
            // closing first: the card of the new world takes the focus
            close_dialog(&mut commands, root, false);
            commands.trigger(WorldsChanged {
                focus: Some(name),
                error: None,
            });
        }
        Err(e) => {
            error!("Could not create the world {name:?}: {e}");
            commands
                .entity(form.name)
                .insert(FieldError(Some(create_failure(&e).to_string())));
        }
    }
}

/// Renames the world of the Rename dialog on disk, checking its name again, then closes it and
/// lists the worlds with the focus on the renamed one; on a failure the dialog stays open and
/// says why under the name, and the worlds are listed again. A dialog already closing was
/// submitted before in this update.
fn rename_world_on_submit(
    submit: On<DialogSubmit>,
    forms: Query<&WorldForm, Without<CloseRequest>>,
    texts: Query<&EditableText>,
    paths: Res<GameFolderPaths>,
    mut commands: Commands,
) {
    let root = submit.entity;
    let Ok(form) = forms.get(root) else {
        return;
    };
    let typed = value_of(&texts, form.name);
    let (None, true) = check_name(form, &typed) else {
        return;
    };
    let (Some(from), Ok(to)) = (&form.current, check_world_name(&typed, &form.others)) else {
        return;
    };
    let saves = paths.game_folder_path.join(SAVE_PATH);
    match rename_world(&saves, from, &to) {
        Ok(()) => {
            info!("Renamed the world {from:?} to {to:?}");
            // closing first: the card of the renamed world takes the focus
            close_dialog(&mut commands, root, false);
            commands.trigger(WorldsChanged {
                focus: Some(to),
                error: None,
            });
        }
        // a rename put back may have failed too: the cards show what is on disk, and the
        // dialog, still open, keeps the focus
        Err(e) => {
            error!("Could not rename the world {from:?} to {to:?}: {e}");
            commands
                .entity(form.name)
                .insert(FieldError(Some(rename_failure(&e).to_string())));
            commands.trigger(WorldsChanged {
                focus: Some(from.clone()),
                error: None,
            });
        }
    }
}

/// Asks before deleting `world`, Cancel focused. Delete deletes it and lists the worlds again,
/// the focus on the card of `next`; a failure shows under the buttons of the detail pane.
pub fn open_delete_world_dialog(
    commands: &mut Commands,
    assets: &AssetServer,
    focus: &InputFocus,
    world: &WorldSummary,
    next: Option<String>,
) -> Entity {
    let title = format!("Delete \"{}\"?", world.name);
    let body = format!(
        "Its {} of saves will be deleted for good.",
        format_size(world.size)
    );
    let dialog = Dialog {
        title: &title,
        body: &body,
        actions: &[
            ("Cancel", ButtonVariant::Default),
            ("Delete", ButtonVariant::Danger),
        ],
        focused: 0,
    };
    let root = spawn_dialog(commands, &dialog, assets.load(FONT_PATH), focus);
    let name = world.name.clone();
    commands.entity(root).observe(
        move |closed: On<DialogClosed>, paths: Res<GameFolderPaths>, mut commands: Commands| {
            if closed.action != Some(1) {
                return;
            }
            let saves = paths.game_folder_path.join(SAVE_PATH);
            let changed = match delete_world(&saves, &name) {
                Ok(()) => WorldsChanged {
                    focus: next.clone(),
                    error: None,
                },
                // part of the world may be gone: the cards show what is left on disk
                Err(e) => {
                    error!("Could not delete the world {name:?}: {e}");
                    WorldsChanged {
                        focus: Some(name.clone()),
                        error: Some(delete_failure(&e)),
                    }
                }
            };
            commands.trigger(changed);
        },
    );
    root
}

#[cfg(test)]
mod tests {
    use bevy::text::TextEdit;
    use tempfile::TempDir;

    use super::*;
    use crate::ui::menus::worlds::EMPTY_WORLD_NAME;
    use crate::ui::widgets::dialog::notice_form_changes;

    /// A world form without a seed whose name field holds `text`: its root and its name field.
    fn form(world: &mut World, text: EditableText) -> (Entity, Entity) {
        let name = world.spawn((text, FieldError(None))).id();
        let root = world
            .spawn(WorldForm::new(name, None, Vec::new(), None))
            .id();
        (root, name)
    }

    #[test]
    fn a_failed_delete_says_why_in_a_few_words() {
        let cases = [
            (io::ErrorKind::PermissionDenied, DELETE_PERMISSION),
            (io::ErrorKind::ReadOnlyFilesystem, DELETE_PERMISSION),
            (io::ErrorKind::ResourceBusy, "The world is in use"),
            (io::ErrorKind::Other, "Could not delete the world"),
        ];
        for (kind, message) in cases {
            assert_eq!(delete_failure(&io::Error::from(kind)), message, "{kind:?}");
        }
    }

    /// A bare app noticing the changes of a world dialog without a seed whose name field holds
    /// `text`. Returns the app, the name field and the primary action.
    fn checked_form(text: EditableText) -> (App, Entity, Entity) {
        let mut app = App::new();
        app.add_systems(Update, notice_form_changes);
        let world = app.world_mut();
        let mut commands = world.commands();
        let name = spawn_text_field(
            &mut commands,
            &TextField {
                label: "Name",
                placeholder: "",
                max_characters: Some(MAX_WORLD_NAME_LENGTH),
                font: Handle::default(),
                font_size: FONT_M,
                reserve_error: true,
            },
        );
        let dialog = Dialog {
            title: NEW_WORLD_TITLE,
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
            &[("Name", name)],
            &NEW_WORLD_HINTS,
            Handle::default(),
            &InputFocus::default(),
        );
        commands
            .entity(parts.root)
            .insert(WorldForm::new(name.input, None, Vec::new(), None))
            .observe(check_world_form);
        world.flush();
        world.entity_mut(name.input).insert(text);
        (app, name.input, parts.actions[1])
    }

    #[test]
    fn a_form_opened_with_an_empty_name_is_checked_at_once() {
        let (mut app, name, primary) = checked_form(EditableText::default());
        app.update();
        let error = app.world().get::<FieldError>(name).unwrap();
        assert_eq!(error.0.as_deref(), Some(EMPTY_WORLD_NAME));
        assert!(app.world().get::<InteractionDisabled>(primary).is_some());
    }

    #[test]
    fn a_name_still_being_filled_is_not_checked_yet() {
        let mut text = EditableText::default();
        text.queue_edit(TextEdit::Insert("Fresh".into()));
        let (mut app, name, primary) = checked_form(text);
        app.update();
        assert_eq!(app.world().get::<FieldError>(name).unwrap().0, None);
        assert!(app.world().get::<InteractionDisabled>(primary).is_none());
    }

    #[test]
    fn a_second_submit_of_a_closing_dialog_creates_nothing() {
        let folder = tempfile::TempDir::new().unwrap();
        let mut world = World::new();
        world.insert_resource(GameFolderPaths {
            game_folder_path: folder.path().to_path_buf(),
            assets_folder_path: folder.path().to_path_buf(),
        });
        let (root, name) = form(&mut world, EditableText::new("Fresh"));
        world.entity_mut(root).observe(create_world_on_submit);
        // two submits in the same update, such as Enter and a click on Create
        let mut commands = world.commands();
        commands.trigger(DialogSubmit { entity: root });
        commands.trigger(DialogSubmit { entity: root });
        world.flush();
        assert!(folder.path().join(SAVE_PATH).join("Fresh").is_dir());
        assert_eq!(world.get::<FieldError>(name).unwrap().0, None);
    }

    #[test]
    fn a_failed_rename_says_why_in_a_few_words() {
        let cases = [
            (io::ErrorKind::StorageFull, "The disk is full"),
            (
                io::ErrorKind::PermissionDenied,
                "No permission to rename this world",
            ),
            (io::ErrorKind::ResourceBusy, "The world is in use"),
            (io::ErrorKind::Other, "Could not rename the world"),
        ];
        for (kind, message) in cases {
            assert_eq!(rename_failure(&io::Error::from(kind)), message, "{kind:?}");
        }
    }

    #[test]
    fn the_unchanged_name_of_a_renamed_world_cannot_be_submitted() {
        let mut world = World::new();
        let folder = tempfile::TempDir::new().unwrap();
        world.insert_resource(GameFolderPaths {
            game_folder_path: folder.path().to_path_buf(),
            assets_folder_path: folder.path().to_path_buf(),
        });
        let saves = folder.path().join(SAVE_PATH);
        create_world(&saves, "Survie", 1).unwrap();
        let name = world
            .spawn((EditableText::new(" Survie "), FieldError(None)))
            .id();
        let root = world
            .spawn(WorldForm::new(
                name,
                None,
                Vec::new(),
                Some("Survie".into()),
            ))
            .observe(rename_world_on_submit)
            .id();
        world.commands().trigger(DialogSubmit { entity: root });
        world.flush();
        assert!(saves.join("Survie").is_dir());
    }

    /// A world whose saves are in a temporary folder, with a Rename dialog of `from` whose name
    /// field holds `typed`. Returns the folder, the dialog's root and its name field.
    fn rename_form(world: &mut World, from: &str, typed: &str) -> (TempDir, Entity, Entity) {
        let folder = TempDir::new().unwrap();
        world.insert_resource(GameFolderPaths {
            game_folder_path: folder.path().to_path_buf(),
            assets_folder_path: folder.path().to_path_buf(),
        });
        let name = world
            .spawn((EditableText::new(typed), FieldError(None)))
            .id();
        let root = world
            .spawn(WorldForm::new(name, None, Vec::new(), Some(from.into())))
            .observe(rename_world_on_submit)
            .id();
        (folder, root, name)
    }

    #[test]
    fn a_second_submit_of_a_closing_rename_renames_once() {
        let mut world = World::new();
        let (folder, root, name) = rename_form(&mut world, "Survie", "Base");
        let saves = folder.path().join(SAVE_PATH);
        create_world(&saves, "Survie", 1).unwrap();
        // two submits in the same update, such as Enter and a click on Rename
        let mut commands = world.commands();
        commands.trigger(DialogSubmit { entity: root });
        commands.trigger(DialogSubmit { entity: root });
        world.flush();
        assert!(saves.join("Base").is_dir());
        assert!(!saves.join("Survie").exists());
        assert_eq!(world.get::<FieldError>(name).unwrap().0, None);
    }

    #[test]
    fn a_failed_rename_lists_the_worlds_again_and_keeps_its_dialog() {
        let mut world = World::new();
        // no world on disk: the rename fails
        let (_folder, root, name) = rename_form(&mut world, "Survie", "Base");
        world.init_resource::<Changes>();
        world.add_observer(|changed: On<WorldsChanged>, mut changes: ResMut<Changes>| {
            changes.0.push((changed.focus.clone(), changed.error));
        });
        world.commands().trigger(DialogSubmit { entity: root });
        world.flush();
        let changes = &world.resource::<Changes>().0;
        assert_eq!(changes, &[(Some("Survie".to_string()), None)]);
        let error = world.get::<FieldError>(name).unwrap();
        assert_eq!(error.0.as_deref(), Some("Could not rename the world"));
        assert!(world.get::<CloseRequest>(root).is_none());
    }

    /// The `WorldsChanged` triggered, in order: their focus and error.
    #[derive(Resource, Default)]
    struct Changes(Vec<(Option<String>, Option<&'static str>)>);

    #[test]
    fn a_failed_create_says_why_in_a_few_words() {
        let cases = [
            (io::ErrorKind::StorageFull, "The disk is full"),
            (
                io::ErrorKind::PermissionDenied,
                "No permission to write here",
            ),
            (
                io::ErrorKind::ReadOnlyFilesystem,
                "No permission to write here",
            ),
            (
                io::ErrorKind::AlreadyExists,
                "A folder with this name already exists",
            ),
            (io::ErrorKind::Other, "Could not create the world"),
        ];
        for (kind, message) in cases {
            assert_eq!(create_failure(&io::Error::from(kind)), message, "{kind:?}");
        }
    }
}
