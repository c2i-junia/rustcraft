use super::list_screen::*;
use super::MenuState;
use crate::constants::SAVE_PATH;
use crate::state::GameState;
use crate::ui::widgets::text_field::{FieldError, Submit};
use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use bevy::text::EditableText;
use shared::GameFolderPaths;
use std::io;
use std::{
    fs,
    path::{self, Path},
};

pub struct WorldItem {
    pub name: String,
}

#[derive(Component, Default)]
pub struct WorldList {
    pub worlds: HashMap<Entity, WorldItem>,
}

#[derive(Component)]
pub enum SoloButtonAction {
    Add,
    Load(Entity),
    Delete(Entity),
}

#[derive(Component)]
pub struct WorldNameInput;

#[derive(Resource, Default, Debug, Clone)]
pub struct SelectedWorld {
    pub name: Option<String>,
}

pub fn solo_menu_setup(mut commands: Commands, assets: Res<AssetServer>) {
    let screen = spawn_list_screen(
        &mut commands,
        &assets,
        &ListScreen {
            menu: MenuState::Solo,
            title: "World list",
            inputs: &[InputField {
                placeholder: "World name",
                focused: true,
                max_characters: Some(MAX_WORLD_NAME_LENGTH),
            }],
            add_label: "Create world",
            stacked: true,
        },
    );
    commands.entity(screen.list).insert(WorldList::default());
    commands
        .entity(screen.inputs[0])
        .insert(WorldNameInput)
        .observe(create_world_on_submit);
    commands
        .entity(screen.add_button)
        .insert(SoloButtonAction::Add);
}

pub fn list_worlds(
    mut commands: Commands,
    assets: Res<AssetServer>,
    mut list_query: Query<(&mut WorldList, Entity)>,
    game_paths: Res<GameFolderPaths>,
) {
    let Ok((mut list, list_entity)) = list_query.single_mut() else {
        return;
    };
    let save_path = game_paths.game_folder_path.join(SAVE_PATH);
    info!("Looking for worlds in : {}", save_path.display());

    for name in saved_world_names(&save_path) {
        add_world_item(name, &mut commands, &assets, &mut list, list_entity);
    }
}

/// The names of the saved worlds: the folders of `saves_path` that hold a `world.ron`.
/// The saves folder is created when it does not exist. A saves folder that cannot be created
/// or read gives no worlds, and entries that cannot be read or are not valid UTF-8 are skipped.
fn saved_world_names(saves_path: &Path) -> Vec<String> {
    let entries = fs::create_dir_all(saves_path).and_then(|()| fs::read_dir(saves_path));
    let Ok(entries) = entries else {
        error!("Could not read the saves folder {}", saves_path.display());
        return Vec::new();
    };

    let mut names: Vec<String> = entries
        .filter_map(|entry| {
            let entry = entry
                .inspect_err(|err| error!("Could not read an entry of the saves folder: {err}"))
                .ok()?;
            let name = entry
                .file_name()
                .into_string()
                .inspect_err(|_| {
                    warn!(
                        "Skipping a world whose name is not valid UTF-8: {:?}",
                        entry.path()
                    )
                })
                .ok()?;
            entry.path().join("world.ron").exists().then_some(name)
        })
        .collect();
    // The order of a directory listing is not stable: list the worlds alphabetically.
    names.sort();
    names
}

fn add_world_item(
    name: String,
    commands: &mut Commands,
    assets: &Res<AssetServer>,
    list: &mut WorldList,
    list_entity: Entity,
) {
    info!("Adding world to list : name = {name:?}");

    let row = spawn_row(commands, assets, list_entity, &name, None);
    commands
        .entity(row.play)
        .insert(SoloButtonAction::Load(row.row));
    commands
        .entity(row.delete)
        .insert(SoloButtonAction::Delete(row.row));
    list.worlds.insert(row.row, WorldItem { name });
}

const MAX_WORLD_NAME_LENGTH: usize = 32;

/// Shown under the name field when the typed name breaks the rule of `is_valid_world_name`.
pub const INVALID_WORLD_NAME: &str = "Use 1 to 32 letters, digits, spaces, _ or -";
/// Shown under the name field when a world of the list already has the typed name.
pub const WORLD_EXISTS: &str = "A world with this name already exists";

/// A world name becomes a folder name under the saves folder, so it must be a single plain
/// path component: letters, digits, spaces, `_` and `-`, 1 to 32 characters.
fn is_valid_world_name(name: &str) -> bool {
    (1..=MAX_WORLD_NAME_LENGTH).contains(&name.chars().count())
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, ' ' | '_' | '-'))
}

/// Worlds that already exist on disk may have any name, but deleting one must stay inside the
/// saves folder: the name has to be a single normal path component.
fn is_single_path_component(name: &str) -> bool {
    let mut components = Path::new(name).components();
    matches!(
        (components.next(), components.next()),
        (Some(path::Component::Normal(component)), None) if component == name
    )
}

fn generate_new_world_name(world_list: &WorldList) -> String {
    (1..)
        .map(|index| format!("new_world_{index}"))
        .find(|candidate| {
            world_list
                .worlds
                .values()
                .all(|world| &world.name != candidate)
        })
        .unwrap_or_default()
}

/// The name of the world to create from the `typed` one: a default name when nothing is typed.
/// It is an error, the message to show, when the name is invalid or is taken in `list`.
fn new_world_name(typed: &str, list: &WorldList) -> Result<String, &'static str> {
    let name = if typed.is_empty() {
        generate_new_world_name(list)
    } else {
        typed.to_string()
    };
    if !is_valid_world_name(&name) {
        return Err(INVALID_WORLD_NAME);
    }
    if list.worlds.values().any(|world| world.name == name) {
        return Err(WORLD_EXISTS);
    }
    Ok(name)
}

/// Creates the world `typed` in the name field `field` and empties the field, or shows under the
/// field why the world cannot be created.
fn create_world(
    typed: &str,
    (field, mut text): (Entity, Mut<EditableText>),
    (list_entity, list): (Entity, &mut WorldList),
    commands: &mut Commands,
    assets: &Res<AssetServer>,
) {
    match new_world_name(typed, list) {
        Ok(name) => {
            add_world_item(name, commands, assets, list, list_entity);
            text.clear();
            commands.entity(field).insert(FieldError(None));
        }
        Err(message) => {
            warn!("Not creating world {typed:?}: {message}");
            commands
                .entity(field)
                .insert(FieldError(Some(message.to_string())));
        }
    }
}

/// Enter in the name field creates the world, as "Create world" does.
fn create_world_on_submit(
    submit: On<Submit>,
    mut texts: Query<&mut EditableText>,
    mut list_query: Query<(Entity, &mut WorldList)>,
    mut commands: Commands,
    assets: Res<AssetServer>,
) {
    let (Ok(text), Ok((list_entity, mut list))) =
        (texts.get_mut(submit.entity), list_query.single_mut())
    else {
        return;
    };
    create_world(
        &submit.value,
        (submit.entity, text),
        (list_entity, &mut list),
        &mut commands,
        &assets,
    );
}

pub fn solo_action(
    interactions: Query<(&Interaction, &SoloButtonAction), (Changed<Interaction>, With<Button>)>,
    mut name_input: Query<(Entity, &mut EditableText), With<WorldNameInput>>,
    mut list_query: Query<(Entity, &mut WorldList)>,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut menu_state: ResMut<NextState<MenuState>>,
    mut game_state: ResMut<NextState<GameState>>,
    mut selected_world: ResMut<SelectedWorld>,
    paths: Res<GameFolderPaths>,
) {
    let Ok((entity, mut list)) = list_query.single_mut() else {
        return;
    };

    for (interaction, action) in &interactions {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match *action {
            SoloButtonAction::Add => {
                let Ok((field, text)) = name_input.single_mut() else {
                    continue;
                };
                let typed = text.value().to_string();
                create_world(
                    &typed,
                    (field, text),
                    (entity, &mut list),
                    &mut commands,
                    &asset_server,
                );
            }
            SoloButtonAction::Load(world_entity) => {
                if let Some(world) = list.worlds.get(&world_entity) {
                    selected_world.name = Some(world.name.clone());
                    game_state.set(GameState::PreGameLoading);
                    menu_state.set(MenuState::Disabled);
                }
            }
            SoloButtonAction::Delete(world_entity) => {
                if let Some(world) = list.worlds.get(&world_entity) {
                    if let Err(e) = delete_save_files(&world.name, &paths) {
                        error!("Error while deleting save files: {}", e);
                        continue;
                    }
                    list.worlds.remove(&world_entity);
                }
                commands.entity(entity).detach_children(&[world_entity]);
                commands.entity(world_entity).despawn();
            }
        }
    }
}

pub fn delete_save_files(
    world_name: &str,
    game_folder_path: &GameFolderPaths,
) -> Result<(), io::Error> {
    if !is_single_path_component(world_name) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("invalid world name {world_name:?}"),
        ));
    }

    // Delete the entire world directory
    let world_dir = game_folder_path
        .game_folder_path
        .join(SAVE_PATH)
        .join(world_name);

    match fs::remove_dir_all(&world_dir) {
        Ok(()) => info!("Deleted world directory: {}", world_dir.display()),
        // Nothing was ever saved for this world: there is nothing to delete
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(e) => {
            return Err(io::Error::new(
                e.kind(),
                format!("could not delete {}: {e}", world_dir.display()),
            ))
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// An empty folder of its own in the temp folder, removed at the end of the test.
    struct TempFolder(PathBuf);

    impl TempFolder {
        fn new(name: &str) -> Self {
            let path =
                std::env::temp_dir().join(format!("rustcraft-solo-{name}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TempFolder {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn make_world(saves: &Path, name: impl AsRef<std::ffi::OsStr>) {
        let world = saves.join(name.as_ref());
        fs::create_dir_all(&world).unwrap();
        fs::write(world.join("world.ron"), "").unwrap();
    }

    #[test]
    fn world_names_are_a_single_plain_component() {
        for valid in ["a", "My World", "new_world_1", "a-b", &"x".repeat(32)] {
            assert!(is_valid_world_name(valid), "{valid:?} should be valid");
        }
        for invalid in [
            "",
            ".",
            "..",
            "/",
            "a/b",
            "..\\x",
            "/etc",
            "C:\\x",
            "a.b",
            "é",
            &"x".repeat(33),
        ] {
            assert!(
                !is_valid_world_name(invalid),
                "{invalid:?} should be invalid"
            );
        }
    }

    #[test]
    fn a_new_world_name_is_checked() {
        let mut list = WorldList::default();
        assert_eq!(new_world_name("", &list), Ok("new_world_1".to_string()));
        assert_eq!(new_world_name("bad/name", &list), Err(INVALID_WORLD_NAME));
        list.worlds.insert(
            Entity::PLACEHOLDER,
            WorldItem {
                name: "Survie".into(),
            },
        );
        assert_eq!(new_world_name("Survie", &list), Err(WORLD_EXISTS));
        assert_eq!(new_world_name("Nouveau", &list), Ok("Nouveau".to_string()));
    }

    #[test]
    fn existing_worlds_can_be_deleted_whatever_their_name_if_it_is_one_component() {
        for name in ["a", "world.v2", "é", "a b", "..x", "x.."] {
            assert!(
                is_single_path_component(name),
                "{name:?} should be accepted"
            );
        }
        for name in ["", ".", "..", "/", "a/b", "a/", "/etc", "../x"] {
            assert!(
                !is_single_path_component(name),
                "{name:?} should be refused"
            );
        }
    }

    #[test]
    fn a_world_with_a_name_outside_the_creation_rule_is_deleted() {
        let folder = TempFolder::new("delete-legacy");
        let paths = GameFolderPaths {
            game_folder_path: folder.0.clone(),
            assets_folder_path: folder.0.join("assets"),
        };
        let saves = folder.0.join(SAVE_PATH);
        make_world(&saves, "world.v2");

        delete_save_files("world.v2", &paths).unwrap();
        assert!(!saves.join("world.v2").exists());
    }

    #[test]
    fn deleting_a_world_removes_its_chunk_files_and_players_too() {
        let folder = TempFolder::new("delete-chunks");
        let paths = GameFolderPaths {
            game_folder_path: folder.0.clone(),
            assets_folder_path: folder.0.join("assets"),
        };
        let saves = folder.0.join(SAVE_PATH);
        make_world(&saves, "full");
        fs::create_dir_all(saves.join("full/chunks")).unwrap();
        fs::write(saves.join("full/chunks/-1_4_0.bin"), "x").unwrap();
        fs::create_dir_all(saves.join("full/players")).unwrap();
        fs::write(saves.join("full/players/7.ron"), "x").unwrap();
        make_world(&saves, "other");

        delete_save_files("full", &paths).unwrap();

        assert!(!saves.join("full").exists());
        assert!(saves.join("other/world.ron").is_file());
    }

    #[test]
    fn deleting_refuses_names_that_leave_the_saves_folder() {
        let folder = TempFolder::new("delete");
        let paths = GameFolderPaths {
            game_folder_path: folder.0.join("game"),
            assets_folder_path: folder.0.join("assets"),
        };
        let saves = paths.game_folder_path.join(SAVE_PATH);
        make_world(&saves, "keep");
        make_world(&saves, "gone");
        let outside = folder.0.join("game/outside");
        fs::create_dir_all(&outside).unwrap();

        for name in ["..", "../outside", "/", ""] {
            assert!(delete_save_files(name, &paths).is_err());
        }
        assert!(outside.is_dir() && saves.join("keep").is_dir());

        delete_save_files("gone", &paths).unwrap();
        assert!(!saves.join("gone").exists() && saves.join("keep").is_dir());
    }

    #[test]
    fn deleting_a_world_that_is_already_gone_succeeds() {
        let folder = TempFolder::new("delete-gone");
        let paths = GameFolderPaths {
            game_folder_path: folder.0.join("game"),
            assets_folder_path: folder.0.join("assets"),
        };
        delete_save_files("never-saved", &paths).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn a_world_that_cannot_be_deleted_is_an_error() {
        use std::os::unix::fs::PermissionsExt;

        let folder = TempFolder::new("delete-denied");
        let paths = GameFolderPaths {
            game_folder_path: folder.0.join("game"),
            assets_folder_path: folder.0.join("assets"),
        };
        let saves = paths.game_folder_path.join(SAVE_PATH);
        make_world(&saves, "locked");
        // Nothing can be removed from a folder that is not writable
        let read_only = fs::Permissions::from_mode(0o555);
        fs::set_permissions(saves.join("locked"), read_only).unwrap();

        let result = delete_save_files("locked", &paths);

        fs::set_permissions(saves.join("locked"), fs::Permissions::from_mode(0o755)).unwrap();
        assert!(result.is_err());
        assert!(saves.join("locked/world.ron").is_file());
    }

    #[test]
    fn lists_the_folders_that_hold_a_world() {
        let folder = TempFolder::new("list");
        make_world(&folder.0, "alpha");
        fs::create_dir(folder.0.join("empty")).unwrap();
        fs::write(folder.0.join("file"), "").unwrap();

        assert_eq!(saved_world_names(&folder.0), vec!["alpha".to_string()]);
    }

    #[test]
    fn lists_the_worlds_alphabetically() {
        let folder = TempFolder::new("sorted");
        for name in ["delta", "alpha", "Charlie", "bravo"] {
            make_world(&folder.0, name);
        }

        assert_eq!(
            saved_world_names(&folder.0),
            vec!["Charlie", "alpha", "bravo", "delta"]
        );
    }

    #[test]
    fn a_missing_saves_folder_is_created_and_has_no_worlds() {
        let folder = TempFolder::new("missing");
        let saves = folder.0.join("saves");

        assert!(saved_world_names(&saves).is_empty());
        assert!(saves.is_dir());
    }

    #[test]
    fn an_unreadable_saves_folder_has_no_worlds() {
        let folder = TempFolder::new("unreadable");
        let not_a_folder = folder.0.join("saves");
        fs::write(&not_a_folder, "").unwrap();

        assert!(saved_world_names(&not_a_folder).is_empty());
        // Below a file, the folder cannot be created either.
        assert!(saved_world_names(&not_a_folder.join("saves")).is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn a_world_whose_name_is_not_utf8_is_skipped() {
        use std::os::unix::ffi::OsStrExt;

        let folder = TempFolder::new("non-utf8");
        make_world(&folder.0, "alpha");
        make_world(&folder.0, std::ffi::OsStr::from_bytes(b"bad\xff"));

        assert_eq!(saved_world_names(&folder.0), vec!["alpha".to_string()]);
    }
}
