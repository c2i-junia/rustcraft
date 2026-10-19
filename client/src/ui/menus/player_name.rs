//! The player's name: saved in `player.ron`, asked on the first launch, and changed from the
//! button that shows it on the main menu.

use std::fs;
use std::io;

use bevy::{
    input_focus::InputFocus,
    prelude::*,
    text::EditableText,
    ui::InteractionDisabled,
    ui_widgets::{Activate, SelectAllOnFocus},
};
use ron::ser::PrettyConfig;
use serde::{Deserialize, Serialize};
use shared::messages::{sanitize_username, MAX_USERNAME_LENGTH};
use shared::GameFolderPaths;

use crate::constants::PLAYER_NAME_PATH;
use crate::network::{hash_string_to_u64, CurrentPlayerProfile};
use crate::ui::assets::FONT_PATH;
use crate::ui::theme::FONT_M;
use crate::ui::widgets::button::{set_label, ButtonVariant};
use crate::ui::widgets::dialog::{
    apply_form_check, close_dialog, spawn_form_dialog, CloseRequest, Dialog, DialogSubmit,
    FormChanged,
};
use crate::ui::widgets::hints::Hint;
use crate::ui::widgets::text_field::{set_text, spawn_text_field, value_of, FieldError, TextField};

/// The name of a player who has not chosen one.
pub const DEFAULT_PLAYER_NAME: &str = "Player";

/// The title of the dialog asking for a name on the first launch.
pub const CHOOSE_NAME_TITLE: &str = "Choose your name";

/// The title of the dialog changing the name.
pub const PLAYER_NAME_TITLE: &str = "Player name";

/// The key hints of the name dialog.
const NAME_HINTS: [Hint; 3] = [
    Hint {
        keys: "Tab",
        action: "Move",
    },
    Hint {
        keys: "Enter",
        action: "Save",
    },
    Hint {
        keys: "Esc",
        action: "Cancel",
    },
];

/// Present when no name was given nor saved: the main menu asks for one when it first opens.
#[derive(Resource)]
pub struct AskPlayerName;

/// The button of the main menu that shows the player's name and opens the name dialog.
#[derive(Component)]
pub struct PlayerNameButton;

/// The input of the name field.
#[derive(Component)]
pub struct PlayerNameInput;

/// The name field of an open name dialog, and the name it changes, on its root.
#[derive(Component)]
struct NameForm {
    input: Entity,
    /// Kept unchanged, it is not an error, but there is nothing to save.
    current: Option<String>,
}

/// The content of `player.ron`.
#[derive(Serialize, Deserialize)]
struct SavedPlayerName {
    name: String,
}

/// The saved name, trimmed; `None` when none was saved, or the file cannot be read or holds a
/// name that servers refuse.
pub fn load_player_name(paths: &GameFolderPaths) -> Option<String> {
    let path = paths.game_folder_path.join(PLAYER_NAME_PATH);
    let saved: SavedPlayerName = ron::from_str(&fs::read_to_string(path).ok()?).ok()?;
    sanitize_username(&saved.name).map(str::to_string)
}

/// Writes `name` to `player.ron` in the game folder.
pub fn write_player_name(name: &str, paths: &GameFolderPaths) -> io::Result<()> {
    let path = paths.game_folder_path.join(PLAYER_NAME_PATH);
    let saved = SavedPlayerName {
        name: name.to_string(),
    };
    let serialized =
        ron::ser::to_string_pretty(&saved, PrettyConfig::new()).map_err(io::Error::other)?;
    fs::write(&path, serialized)?;
    info!("Player name saved to {path:?}");
    Ok(())
}

/// What is wrong with the name `typed`; `None` when servers accept it.
fn name_error(typed: &str) -> Option<&'static str> {
    if sanitize_username(typed).is_some() {
        None
    } else if typed.trim().is_empty() {
        Some("Enter a name")
    } else {
        Some("No control characters")
    }
}

/// Opens the name dialog. On the first launch (`current` is `None`) its field is empty;
/// otherwise it holds the current name, entirely selected, and Save stays disabled while the
/// name is unchanged.
pub fn open_player_name_dialog(
    commands: &mut Commands,
    assets: &AssetServer,
    focus: &InputFocus,
    current: Option<&str>,
) {
    let font: Handle<Font> = assets.load(FONT_PATH);
    let field = spawn_text_field(
        commands,
        &TextField {
            label: "Name",
            placeholder: DEFAULT_PLAYER_NAME,
            max_characters: Some(MAX_USERNAME_LENGTH),
            font: font.clone(),
            font_size: FONT_M,
            reserve_error: true,
        },
    );
    commands
        .entity(field.input)
        .insert((PlayerNameInput, SelectAllOnFocus));
    if let Some(current) = current {
        let current = current.to_string();
        commands
            .entity(field.input)
            .entry::<EditableText>()
            .and_modify(move |mut text| set_text(&mut text, &current));
    }
    let (title, body) = match current {
        None => (
            CHOOSE_NAME_TITLE,
            "Other players see it. You can change it later.",
        ),
        Some(_) => (
            PLAYER_NAME_TITLE,
            "Your progress in each world is kept under it.",
        ),
    };
    let dialog = Dialog {
        title,
        body,
        actions: &[
            ("Cancel", ButtonVariant::Default),
            ("Save", ButtonVariant::Primary),
        ],
        focused: 0,
    };
    let parts = spawn_form_dialog(
        commands,
        &dialog,
        &[("Name", field)],
        &NAME_HINTS,
        font,
        focus,
    );
    // nothing to save until the first check, a few frames later
    commands
        .entity(parts.actions[1])
        .insert(InteractionDisabled);
    commands
        .entity(parts.root)
        .insert(NameForm {
            input: field.input,
            current: current.map(str::to_string),
        })
        .observe(check_name_form)
        .observe(save_name_on_submit);
}

/// Asks for a name when the main menu first opens, if none was given nor saved.
pub fn ask_player_name_on_first_launch(
    assets: Res<AssetServer>,
    focus: Res<InputFocus>,
    mut commands: Commands,
) {
    commands.remove_resource::<AskPlayerName>();
    open_player_name_dialog(&mut commands, &assets, &focus, None);
}

/// The name button of the main menu opens the name dialog.
pub fn change_name_on_activate(
    _: On<Activate>,
    profile: Res<CurrentPlayerProfile>,
    assets: Res<AssetServer>,
    focus: Res<InputFocus>,
    mut commands: Commands,
) {
    open_player_name_dialog(&mut commands, &assets, &focus, Some(&profile.name));
}

/// Shows why the typed name is wrong, once the field was typed in, and disables Save while it is
/// or while it is unchanged.
fn check_name_form(changed: On<FormChanged>, forms: Query<&NameForm>, mut commands: Commands) {
    let Ok(form) = forms.get(changed.entity) else {
        return;
    };
    let typed = &changed.values[0];
    let error = name_error(typed);
    let unchanged = form.current.as_deref() == Some(typed.trim());
    let submittable = error.is_none() && !unchanged;
    let shown = error.filter(|_| changed.touched[0]);
    apply_form_check(&mut commands, changed.entity, vec![shown], submittable);
}

/// Saves the name of the dialog, checking it again, makes it the player's, then closes the
/// dialog; when it cannot be saved the dialog stays open and says so. A dialog already closing
/// was submitted before in this update.
fn save_name_on_submit(
    submit: On<DialogSubmit>,
    forms: Query<&NameForm, Without<CloseRequest>>,
    texts: Query<&EditableText>,
    buttons: Query<Entity, With<PlayerNameButton>>,
    paths: Res<GameFolderPaths>,
    mut profile: ResMut<CurrentPlayerProfile>,
    mut commands: Commands,
) {
    let root = submit.entity;
    let Ok(form) = forms.get(root) else {
        return;
    };
    let typed = value_of(&texts, form.input);
    let Some(name) = sanitize_username(&typed) else {
        return;
    };
    if let Err(e) = write_player_name(name, &paths) {
        error!("Could not save the player name: {e}");
        commands
            .entity(form.input)
            .insert(FieldError(Some("Could not save the name".to_string())));
        return;
    }
    *profile = CurrentPlayerProfile {
        id: hash_string_to_u64(name),
        name: name.to_string(),
    };
    for button in &buttons {
        commands.entity(button).queue(set_label(name));
    }
    close_dialog(&mut commands, root, true);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_name_is_saved_and_loaded_back_only_when_servers_accept_it() {
        let dir = std::env::temp_dir().join(format!("rustcraft-player-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let paths = GameFolderPaths {
            game_folder_path: dir.clone(),
            assets_folder_path: dir.join("assets"),
        };
        assert_eq!(load_player_name(&paths), None);

        write_player_name("Steve", &paths).unwrap();
        assert_eq!(load_player_name(&paths).as_deref(), Some("Steve"));

        let file = dir.join(PLAYER_NAME_PATH);
        for (content, expected) in [
            ("(name: \"  Alex \")", Some("Alex")),
            ("(name: \"\")", None),
            ("(name: \"a\\nb\")", None),
            ("garbage", None),
            ("()", None),
        ] {
            fs::write(&file, content).unwrap();
            assert_eq!(load_player_name(&paths).as_deref(), expected, "{content}");
        }
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_name_servers_refuse_says_why() {
        assert_eq!(name_error("Steve"), None);
        assert_eq!(name_error("   "), Some("Enter a name"));
        assert_eq!(name_error("a\tb"), Some("No control characters"));
    }
}
