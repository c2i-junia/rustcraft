//! The dialogs of the server list. Add server checks the name and the address as they are typed,
//! and writes the server to `servers.ron` as soon as it is submitted. Remove asks first.

use std::io;

use bevy::{
    input_focus::InputFocus, prelude::*, text::EditableText, ui::InteractionDisabled,
    ui_widgets::SelectAllOnFocus,
};
use shared::GameFolderPaths;

use super::super::servers::{
    add_server, check_server_address, check_server_name, free_server_name, remove_server,
    ServerItem, MAX_SERVER_NAME_LENGTH,
};
use super::super::MenuState;
use super::{AddServerButton, ServerCard};
use crate::constants::SERVER_LIST_SAVE_NAME;
use crate::network::address::MAX_ADDRESS_LENGTH;
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

/// The title of the Add server dialog.
pub const ADD_SERVER_TITLE: &str = "Add server";

/// The key hints of the Add server dialog.
const ADD_SERVER_HINTS: [Hint; 3] = [
    Hint {
        keys: "Tab",
        action: "Move",
    },
    Hint {
        keys: "Enter",
        action: "Add",
    },
    Hint {
        keys: "Esc",
        action: "Cancel",
    },
];

/// The input of the name field of a server dialog.
#[derive(Component)]
pub struct ServerNameInput;

/// The input of the address field of a server dialog.
#[derive(Component)]
pub struct ServerAddressInput;

/// The fields of an open server dialog and what they are checked against, on its root.
#[derive(Component)]
struct ServerForm {
    /// The input of the name field.
    name: Entity,
    /// The input of the address field.
    address: Entity,
    /// The other servers, whose names and addresses the server may not take.
    others: Vec<ServerItem>,
    /// The server being edited, `None` for a new one.
    #[allow(dead_code)] // The Edit dialog, Task 7
    editing: Option<ServerItem>,
}

impl ServerForm {
    /// The names of the other servers.
    fn names(&self) -> Vec<String> {
        self.others.iter().map(|other| other.name.clone()).collect()
    }
}

/// Triggered once `servers.ron` changed: the screen lists the servers again and focuses the card
/// of `focus` (none, or gone: the first card, else Add server); `error` shows in the pane.
#[derive(Event)]
pub struct ServersChanged {
    pub focus: Option<String>,
    pub error: Option<&'static str>,
}

pub(super) fn plugin(app: &mut App) {
    app.add_systems(
        Update,
        open_add_server_on_ctrl_n
            .run_if(in_state(MenuState::Multi))
            .run_if(not(dialog_open))
            .run_if(not(text_field_focused)),
    );
}

/// Opens the Add server dialog: its name field holds a free name, entirely selected, and its
/// address field is empty. `others` are the saved servers. Add stays disabled while the address
/// is empty. Returns the root of the dialog.
pub fn open_add_server_dialog(
    commands: &mut Commands,
    assets: &AssetServer,
    focus: &InputFocus,
    others: Vec<ServerItem>,
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
    let name = field("Name", "", MAX_SERVER_NAME_LENGTH);
    let address = field("Address", "host:port", MAX_ADDRESS_LENGTH);
    let names: Vec<String> = others.iter().map(|other| other.name.clone()).collect();
    let free = free_server_name(&names);
    commands
        .entity(name.input)
        .insert((ServerNameInput, SelectAllOnFocus))
        .entry::<EditableText>()
        .and_modify(move |mut text| set_text(&mut text, &free));
    commands.entity(address.input).insert(ServerAddressInput);
    let dialog = Dialog {
        title: ADD_SERVER_TITLE,
        body: "",
        actions: &[
            ("Cancel", ButtonVariant::Default),
            ("Add", ButtonVariant::Primary),
        ],
        focused: 0,
    };
    let parts = spawn_form_dialog(
        commands,
        &dialog,
        &[("Name", name), ("Address", address)],
        &ADD_SERVER_HINTS,
        font,
        focus,
    );
    // the address is empty until the first check, a few frames later
    commands
        .entity(parts.actions[1])
        .insert(InteractionDisabled);
    commands
        .entity(parts.root)
        .insert(ServerForm {
            name: name.input,
            address: address.input,
            others,
            editing: None,
        })
        .observe(check_server_form)
        .observe(add_server_on_submit);
    parts.root
}

/// Opens the Add server dialog on Ctrl+N, with the listed servers, unless Add server is
/// disabled: the list could not be read.
pub fn open_add_server_on_ctrl_n(
    keys: Res<ButtonInput<KeyCode>>,
    add_server: Query<Has<InteractionDisabled>, With<AddServerButton>>,
    cards: Query<&ServerCard>,
    assets: Res<AssetServer>,
    focus: Res<InputFocus>,
    mut commands: Commands,
) {
    let control = keys.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight]);
    if !control || !keys.just_pressed(KeyCode::KeyN) || add_server.single().unwrap_or(true) {
        return;
    }
    let others = cards.iter().map(|card| card.0.clone()).collect();
    open_add_server_dialog(&mut commands, &assets, &focus, others);
}

/// The value of the text field `input`.
fn value_of(texts: &Query<&EditableText>, input: Entity) -> String {
    texts
        .get(input)
        .map(|text| text.value().to_string())
        .unwrap_or_default()
}

/// Checks the fields of a server dialog whose values changed: shows why a field is wrong under
/// it, and disables the primary action while one is. An empty address never typed into shows no
/// message. A message set by a failed submit stays until a value changes.
fn check_server_form(changed: On<FormChanged>, forms: Query<&ServerForm>, mut commands: Commands) {
    let Ok(form) = forms.get(changed.entity) else {
        return;
    };
    let name = check_server_name(&changed.values[0], &form.names());
    let address = check_server_address(&changed.values[1], &form.others);
    let address_shown = changed.touched[1] || !changed.values[1].is_empty();
    let errors = vec![
        name.as_ref().err().copied(),
        address.as_ref().err().copied().filter(|_| address_shown),
    ];
    let submittable = name.is_ok() && address.is_ok();
    apply_form_check(&mut commands, changed.entity, errors, submittable);
}

/// What a field says when servers.ron could not be written, in a few words.
fn save_failure(error: &io::Error) -> &'static str {
    match error.kind() {
        io::ErrorKind::StorageFull => "The disk is full",
        io::ErrorKind::PermissionDenied | io::ErrorKind::ReadOnlyFilesystem => {
            "No permission to write here"
        }
        io::ErrorKind::InvalidData => "Cannot read servers.ron",
        _ => "Could not save the server list",
    }
}

/// Adds the server of the Add server dialog to `servers.ron`, checking its fields again, then
/// closes it and lists the servers with the focus on the new one; on a failure the dialog stays
/// open and says why under the address. A dialog already closing was submitted before in this
/// update.
fn add_server_on_submit(
    submit: On<DialogSubmit>,
    forms: Query<&ServerForm, Without<CloseRequest>>,
    texts: Query<&EditableText>,
    paths: Res<GameFolderPaths>,
    mut commands: Commands,
) {
    let root = submit.entity;
    let Ok(form) = forms.get(root) else {
        return;
    };
    let typed = value_of(&texts, form.address);
    let (Ok(name), Ok(_)) = (
        check_server_name(&value_of(&texts, form.name), &form.names()),
        check_server_address(&typed, &form.others),
    ) else {
        return;
    };
    let server = ServerItem {
        name: name.clone(),
        ip: typed.trim().to_string(),
    };
    let path = paths.game_folder_path.join(SERVER_LIST_SAVE_NAME);
    match add_server(&path, server) {
        Ok(()) => {
            info!("Added the server {name:?} at {:?}", typed.trim());
            // closing first: the card of the new server takes the focus
            close_dialog(&mut commands, root, false);
            commands.trigger(ServersChanged {
                focus: Some(name),
                error: None,
            });
        }
        Err(e) => {
            error!("Could not add the server {name:?} to {path:?}: {e}");
            commands
                .entity(form.address)
                .insert(FieldError(Some(save_failure(&e).to_string())));
        }
    }
}

/// Opens the dialog that asks to remove `server`, Cancel focused. Confirmed, the server leaves
/// `servers.ron` and the card of `next` takes the focus; a failure keeps the card and says why
/// under the buttons of the detail pane. Returns the root of the dialog.
pub fn open_remove_server_dialog(
    commands: &mut Commands,
    assets: &AssetServer,
    focus: &InputFocus,
    server: &ServerItem,
    next: Option<String>,
) -> Entity {
    let title = format!("Remove \"{}\"?", server.name);
    let body = format!("{} will leave your server list.", server.ip);
    let dialog = Dialog {
        title: &title,
        body: &body,
        actions: &[
            ("Cancel", ButtonVariant::Default),
            ("Remove", ButtonVariant::Danger),
        ],
        focused: 0,
    };
    let root = spawn_dialog(commands, &dialog, assets.load(FONT_PATH), focus);
    let name = server.name.clone();
    commands.entity(root).observe(
        move |closed: On<DialogClosed>, paths: Res<GameFolderPaths>, mut commands: Commands| {
            if closed.action != Some(1) {
                return;
            }
            let path = paths.game_folder_path.join(SERVER_LIST_SAVE_NAME);
            let changed = match remove_server(&path, &name) {
                Ok(()) => {
                    info!("Removed the server {name:?}");
                    ServersChanged {
                        focus: next.clone(),
                        error: None,
                    }
                }
                Err(e) => {
                    error!("Could not remove the server {name:?} from {path:?}: {e}");
                    ServersChanged {
                        focus: Some(name.clone()),
                        error: Some(save_failure(&e)),
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
    use std::fs;

    use tempfile::TempDir;

    use super::*;
    use crate::network::address::EMPTY_ADDRESS;
    use crate::ui::menus::servers::read_servers;

    /// A game folder whose `servers.ron` holds `file`, and a world with its paths.
    fn folder_with(file: &str) -> (TempDir, World) {
        let folder = TempDir::new().unwrap();
        fs::write(folder.path().join(SERVER_LIST_SAVE_NAME), file).unwrap();
        let mut world = World::new();
        world.insert_resource(GameFolderPaths {
            game_folder_path: folder.path().to_path_buf(),
            assets_folder_path: folder.path().to_path_buf(),
        });
        (folder, world)
    }

    /// An Add server form whose fields hold `name` and `address`, without other servers: its
    /// root and its address field.
    fn form(world: &mut World, name: &str, address: &str) -> (Entity, Entity) {
        let name = world
            .spawn((EditableText::new(name), FieldError(None)))
            .id();
        let address = world
            .spawn((EditableText::new(address), FieldError(None)))
            .id();
        let root = world
            .spawn(ServerForm {
                name,
                address,
                others: Vec::new(),
                editing: None,
            })
            .observe(add_server_on_submit)
            .id();
        (root, address)
    }

    #[test]
    fn a_failed_save_says_why_in_a_few_words() {
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
            (io::ErrorKind::InvalidData, "Cannot read servers.ron"),
            (io::ErrorKind::Other, "Could not save the server list"),
        ];
        for (kind, message) in cases {
            assert_eq!(save_failure(&io::Error::from(kind)), message, "{kind:?}");
        }
    }

    #[test]
    fn a_second_submit_of_a_closing_dialog_adds_one_server() {
        let (folder, mut world) = folder_with("[]");
        let (root, address) = form(&mut world, "Home", " 10.0.0.2:8000 ");
        // two submits in the same update, such as Enter and a click on Add
        let mut commands = world.commands();
        commands.trigger(DialogSubmit { entity: root });
        commands.trigger(DialogSubmit { entity: root });
        world.flush();
        let saved = read_servers(&folder.path().join(SERVER_LIST_SAVE_NAME)).unwrap();
        assert_eq!(
            saved,
            [ServerItem {
                name: "Home".to_string(),
                ip: "10.0.0.2:8000".to_string(),
            }]
        );
        assert_eq!(world.get::<FieldError>(address).unwrap().0, None);
    }

    #[test]
    fn a_failed_save_keeps_the_dialog_open_and_the_file_as_it_was() {
        let (folder, mut world) = folder_with("not ron");
        let (root, address) = form(&mut world, "Home", "10.0.0.2");
        world.commands().trigger(DialogSubmit { entity: root });
        world.flush();
        let error = world.get::<FieldError>(address).unwrap();
        assert_eq!(error.0.as_deref(), Some("Cannot read servers.ron"));
        assert!(world.get::<CloseRequest>(root).is_none());
        let file = fs::read_to_string(folder.path().join(SERVER_LIST_SAVE_NAME)).unwrap();
        assert_eq!(file, "not ron");
    }

    #[test]
    fn an_empty_address_says_why_only_once_typed_into() {
        let mut world = World::new();
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
        let (name, address) = (field("Name"), field("Address"));
        let dialog = Dialog {
            title: ADD_SERVER_TITLE,
            body: "",
            actions: &[
                ("Cancel", ButtonVariant::Default),
                ("Add", ButtonVariant::Primary),
            ],
            focused: 0,
        };
        let parts = spawn_form_dialog(
            &mut commands,
            &dialog,
            &[("Name", name), ("Address", address)],
            &ADD_SERVER_HINTS,
            Handle::default(),
            &InputFocus::default(),
        );
        commands
            .entity(parts.root)
            .insert(ServerForm {
                name: name.input,
                address: address.input,
                others: Vec::new(),
                editing: None,
            })
            .observe(check_server_form);
        world.flush();
        let (root, add) = (parts.root, parts.actions[1]);
        let mut check = |touched: bool| {
            world.commands().trigger(FormChanged {
                entity: root,
                values: vec!["Home".to_string(), String::new()],
                touched: vec![false, touched],
            });
            world.flush();
            let error = world.get::<FieldError>(address.input).unwrap().0.clone();
            (error, world.get::<InteractionDisabled>(add).is_some())
        };
        assert_eq!(check(false), (None, true));
        assert_eq!(check(true), (Some(EMPTY_ADDRESS.to_string()), true));
    }
}
