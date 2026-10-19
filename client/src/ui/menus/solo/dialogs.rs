//! The dialogs of the world list. New world checks the name and the seed as they are typed, and
//! writes the world to disk as soon as it is submitted.

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
    check_world_name, create_world, free_world_name, parse_seed, MAX_WORLD_NAME_LENGTH,
};
use super::super::MenuState;
use super::WorldList;
use crate::constants::SAVE_PATH;
use crate::ui::assets::FONT_PATH;
use crate::ui::theme::FONT_M;
use crate::ui::widgets::button::ButtonVariant;
use crate::ui::widgets::dialog::{
    close_dialog, dialog_open, spawn_form_dialog, Dialog, DialogSubmit,
};
use crate::ui::widgets::hints::Hint;
use crate::ui::widgets::text_field::{
    set_text, spawn_text_field, text_field_focused, FieldError, TextField,
};

/// The title of the New world dialog.
pub const NEW_WORLD_TITLE: &str = "New world";

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
    /// The action that submits the dialog, disabled while a field is wrong.
    primary: Entity,
    /// The names of the other worlds, which the name may not take.
    others: Vec<String>,
    /// The name of the world being renamed: kept unchanged, it is not an error, but there is
    /// nothing to submit.
    current: Option<String>,
    /// The texts of the name and seed fields when they were last checked. `EditableText` is
    /// changed on every frame by its layout, so the texts themselves tell when to check again.
    checked: (String, String),
}

/// Triggered once the worlds on disk changed: the screen lists them again and focuses the card
/// of `focus` (none: the first card, or New world when there is none).
#[derive(Event)]
pub struct WorldsChanged {
    pub focus: Option<String>,
}

pub(super) fn plugin(app: &mut App) {
    app.add_systems(
        Update,
        (
            open_new_world_on_ctrl_n
                .run_if(in_state(MenuState::Solo))
                .run_if(not(dialog_open))
                .run_if(not(text_field_focused)),
            validate_world_forms,
        ),
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
        .insert(WorldForm {
            name: name.input,
            seed: Some(seed.input),
            primary: parts.actions[1],
            others,
            current: None,
            checked: Default::default(),
        })
        .observe(create_world_on_submit);
    parts.root
}

/// Opens the New world dialog on Ctrl+N, with the names of the listed worlds.
pub fn open_new_world_on_ctrl_n(
    keys: Res<ButtonInput<KeyCode>>,
    lists: Query<&WorldList>,
    assets: Res<AssetServer>,
    focus: Res<InputFocus>,
    mut commands: Commands,
) {
    let control = keys.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight]);
    if control && keys.just_pressed(KeyCode::KeyN) {
        let others = lists.iter().flat_map(WorldList::names).collect();
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

/// Checks the fields of each world dialog whose text changed since they were last checked:
/// shows why a field is wrong under it, and disables the primary action while one is. A message
/// set by a failed submit stays until the text changes.
fn validate_world_forms(
    mut forms: Query<&mut WorldForm>,
    texts: Query<&EditableText>,
    mut errors: Query<&mut FieldError>,
    disabled: Query<(), With<InteractionDisabled>>,
    mut commands: Commands,
) {
    for mut form in &mut forms {
        let name = value_of(&texts, form.name);
        let seed = form
            .seed
            .map(|seed| value_of(&texts, seed))
            .unwrap_or_default();
        if form.checked.0 == name && form.checked.1 == seed {
            continue;
        }
        let (name_error, name_ok) = check_name(&form, &name);
        let mut fields = vec![(form.name, name_error)];
        let mut ok = name_ok;
        if let Some(seed_field) = form.seed {
            let seed_error = parse_seed(&seed).err();
            ok &= seed_error.is_none();
            fields.push((seed_field, seed_error));
        }
        for (field, message) in fields {
            if let Ok(mut error) = errors.get_mut(field) {
                if error.0.as_deref() != message {
                    error.0 = message.map(str::to_string);
                }
            }
        }
        if ok == disabled.contains(form.primary) {
            if ok {
                commands
                    .entity(form.primary)
                    .remove::<InteractionDisabled>();
            } else {
                commands.entity(form.primary).insert(InteractionDisabled);
            }
        }
        form.checked = (name, seed);
    }
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

/// Creates the world of the New world dialog on disk, checking its fields again, then closes it;
/// on a failure the dialog stays open and says why under the name.
fn create_world_on_submit(
    submit: On<DialogSubmit>,
    forms: Query<&WorldForm>,
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
            commands.trigger(WorldsChanged { focus: Some(name) });
            close_dialog(&mut commands, root, true);
        }
        Err(e) => {
            error!("Could not create the world {name:?}: {e}");
            commands
                .entity(form.name)
                .insert(FieldError(Some(create_failure(&e).to_string())));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
