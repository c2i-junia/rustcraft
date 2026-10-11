use super::{MenuButtonAction, MenuState, ScrollingList};
use crate::ui::assets::*;
use crate::ui::style::*;
use crate::world::ClientWorldMap;
use crate::{constants::SAVE_PATH, GameState};
use bevy::platform::collections::HashMap;
use bevy::prelude::Resource;
use bevy::prelude::*;
use bevy::{
    asset::AssetServer,
    color::Color,
    prelude::{
        Button, Changed, Commands, Component, DespawnOnExit, Entity, NextState, Query, Res, ResMut,
        Text, With,
    },
    ui::{
        AlignContent, AlignItems, BackgroundColor, BorderColor, Display, FlexDirection,
        GridPlacement, GridTrack, Interaction, JustifyContent, Node, Overflow, UiRect, Val,
    },
};
use bevy_simple_text_input::TextInput;
use bevy_simple_text_input::TextInputTextColor;
use bevy_simple_text_input::TextInputTextFont;
use bevy_simple_text_input::{
    TextInputInactive, TextInputPlaceholder, TextInputSettings, TextInputValue,
};
use shared::GameFolderPaths;
use std::io;
use std::{
    fs,
    path::{self, Path, PathBuf},
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

pub fn solo_menu_setup(mut commands: Commands, assets_server: Res<AssetServer>) {
    let background_image = load_background_image(&assets_server);
    let font = load_font(&assets_server);
    let button_background_image = load_button_background_large_image(&assets_server);

    let txt_font = TextFont {
        font: font.clone().into(),
        font_size: 20.0.into(),
        ..default()
    };

    let txt_color = TextColor(Color::WHITE);

    let btn_style = Node {
        display: Display::Flex,
        flex_direction: FlexDirection::Column,
        justify_content: JustifyContent::Center,
        align_items: AlignItems::Center,
        border: UiRect::all(Val::Px(2.)),
        height: Val::Px(40.0),
        ..Default::default()
    };

    commands
        .spawn((
            DespawnOnExit(MenuState::Solo),
            (Node {
                width: Val::Vw(100.0),
                height: Val::Vh(100.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                flex_direction: FlexDirection::Column,
                padding: UiRect::horizontal(Val::Percent(20.)),
                row_gap: Val::Percent(2.),
                ..Default::default()
            },),
            fill_image_node(background_image),
        ))
        .with_children(|root| {
            root.spawn((
                Text::new("World list"),
                txt_color,
                txt_font.clone(),
                Node {
                    border: UiRect::all(Val::Px(1.)),
                    flex_direction: FlexDirection::Column,
                    align_content: AlignContent::Center,
                    display: Display::Flex,
                    ..default()
                },
            ));

            root.spawn((
                Node {
                    width: Val::Percent(100.),
                    height: Val::Percent(50.),
                    flex_direction: FlexDirection::Column,
                    overflow: Overflow::clip_y(),
                    border: UiRect::all(Val::Px(2.)),
                    ..Default::default()
                },
                BorderColor::all(BACKGROUND_COLOR),
            ))
            .with_children(|w| {
                w.spawn((
                    (Node {
                        flex_direction: FlexDirection::Column,
                        align_items: AlignItems::Center,
                        padding: UiRect::all(Val::Px(10.)),
                        row_gap: Val::Px(10.),
                        ..Default::default()
                    },),
                    ScrollingList { position: 0. },
                    WorldList {
                        worlds: HashMap::new(),
                    },
                ));
            });

            root.spawn((Node {
                width: Val::Percent(100.),
                display: Display::Grid,
                grid_template_columns: vec![GridTrack::flex(1.), GridTrack::flex(1.)],
                row_gap: Val::Px(5.),
                column_gap: Val::Px(5.),
                ..default()
            },))
                .with_children(|wrapper| {
                    let node = {
                        let mut style = btn_style.clone();
                        style.grid_column = GridPlacement::span(2);
                        style
                    };
                    wrapper.spawn((
                        (
                            BorderColor::all(BACKGROUND_COLOR),
                            BackgroundColor(Color::BLACK),
                            node,
                        ),
                        WorldNameInput,
                        (
                            TextInput,
                            TextInputSettings {
                                retain_on_submit: true,
                                mask_character: None,
                                max_length: None,
                            },
                            TextInputPlaceholder {
                                value: "World name".into(),
                                ..default()
                            },
                            TextInputInactive(false),
                            TextInputTextFont(txt_font.clone()),
                            TextInputTextColor(txt_color),
                            TextInputValue("".to_string()),
                        ),
                    ));

                    wrapper
                        .spawn((
                            (
                                Button,
                                BorderColor::all(Color::BLACK),
                                BackgroundColor(BACKGROUND_COLOR),
                                {
                                    let mut style = btn_style.clone();
                                    style.grid_column = GridPlacement::span(2);
                                    style
                                },
                                fill_image_node(button_background_image.clone()),
                            ),
                            SoloButtonAction::Add,
                        ))
                        .with_children(|btn| {
                            btn.spawn((Text::new("Create world"), txt_font.clone(), txt_color));
                        });

                    wrapper
                        .spawn((
                            (
                                Button,
                                BorderColor::all(Color::BLACK),
                                BackgroundColor(BACKGROUND_COLOR),
                                {
                                    let mut style = btn_style.clone();
                                    style.grid_column = GridPlacement::span(2);
                                    style
                                },
                                fill_image_node(button_background_image.clone()),
                            ),
                            MenuButtonAction::BackToMainMenu,
                        ))
                        .with_children(|btn| {
                            btn.spawn((Text::new("Back to menu"), txt_font.clone(), txt_color));
                        });
                });
        });
}

pub fn list_worlds(
    mut commands: Commands,
    assets: Res<AssetServer>,
    mut list_query: Query<(&mut WorldList, Entity)>,
    mut world_map: ResMut<ClientWorldMap>,
    game_paths: Res<GameFolderPaths>,
) {
    let (mut list, list_entity) = list_query.single_mut().unwrap();

    let save_path: PathBuf = game_paths.game_folder_path.join(SAVE_PATH);
    info!("Looking for worlds in : {}", save_path.display());

    for name in saved_world_names(&save_path) {
        add_world_item(
            name,
            &mut commands,
            &assets,
            &mut list,
            list_entity,
            &mut world_map,
            &game_paths,
        );
    }

    info!("Found {} worlds", list.worlds.len());
}

/// The names of the saved worlds: the folders of `saves_path` that hold a `world.ron`.
/// The saves folder is created when it does not exist. A saves folder that cannot be created
/// or read gives no worlds, and entries that cannot be read or are not valid UTF-8 are skipped.
fn saved_world_names(saves_path: &Path) -> Vec<String> {
    match fs::exists(saves_path) {
        Ok(true) => {}
        Ok(false) => match fs::create_dir_all(saves_path) {
            Ok(()) => info!(
                "Successfully created the saves folder : {}",
                saves_path.display()
            ),
            Err(err) => {
                error!(
                    "Could not create the saves folder {}: {err}",
                    saves_path.display()
                );
                return Vec::new();
            }
        },
        Err(err) => {
            error!(
                "Could not access the saves folder {}: {err}",
                saves_path.display()
            );
            return Vec::new();
        }
    }

    let entries = match fs::read_dir(saves_path) {
        Ok(entries) => entries,
        Err(err) => {
            error!(
                "Could not read the saves folder {}: {err}",
                saves_path.display()
            );
            return Vec::new();
        }
    };

    let mut names = Vec::new();
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(err) => {
                error!("Could not read an entry of the saves folder: {err}");
                continue;
            }
        };
        let Ok(name) = entry.file_name().into_string() else {
            warn!(
                "Skipping a world whose name is not valid UTF-8: {:?}",
                entry.path()
            );
            continue;
        };

        // Check if it's a directory and contains world.ron
        let world_path = entry.path();
        if world_path.is_dir() && world_path.join("world.ron").exists() {
            names.push(name);
        }
    }
    names
}

fn add_world_item(
    name: String,
    commands: &mut Commands,
    asset_server: &Res<AssetServer>,
    list: &mut WorldList,
    list_entity: Entity,
    world_map: &mut ClientWorldMap,
    paths: &Res<GameFolderPaths>,
) {
    info!(
        "Adding world to list : name = {:?}, entity={:?}",
        name, list_entity
    );

    let base_path = paths.assets_folder_path.display();

    // udpate the name of the world_map
    world_map.name = name.clone();

    let btn_style = Node {
        display: Display::Flex,
        flex_direction: FlexDirection::Column,
        justify_content: JustifyContent::Center,
        align_items: AlignItems::Center,
        border: UiRect::all(Val::Px(2.)),
        height: Val::Percent(80.),
        ..default()
    };

    let img_style = Node {
        height: Val::Percent(100.),
        ..default()
    };

    let world = commands
        .spawn((
            BorderColor::all(BACKGROUND_COLOR),
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(5.),
                width: Val::Percent(100.),
                height: Val::Vh(10.),
                padding: UiRect::horizontal(Val::Percent(2.)),
                border: UiRect::all(Val::Px(2.)),
                ..default()
            },
        ))
        .id();

    let play_btn = commands
        .spawn((SoloButtonAction::Load(world), (Button, btn_style.clone())))
        .with_children(|btn| {
            let icon = asset_server.load(format!("{base_path}/graphics/play.png"));
            btn.spawn((ImageNode::new(icon), img_style.clone()));
        })
        .id();

    let delete_btn = commands
        .spawn((SoloButtonAction::Delete(world), (Button, btn_style.clone())))
        .with_children(|btn| {
            let icon = asset_server.load(format!("{base_path}/graphics/trash.png"));
            btn.spawn((ImageNode::new(icon), img_style.clone()));
        })
        .id();

    let txt = commands
        .spawn((
            (
                Text::new(name.clone()),
                TextFont {
                    font: asset_server
                        .load("./fonts/RustCraftRegular-Bmg3.otf")
                        .into(),
                    font_size: 20.0.into(),
                    ..default()
                },
                TextColor(Color::WHITE),
            ),
            Node {
                display: Display::Flex,
                flex_direction: FlexDirection::Column,
                ..default()
            },
        ))
        .id();

    commands
        .entity(world)
        .add_children(&[play_btn, delete_btn, txt]);

    commands.entity(list_entity).add_children(&[world]);

    list.worlds.insert(world, WorldItem { name: name.clone() });
}

const MAX_WORLD_NAME_LENGTH: usize = 32;

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
    let mut index = 1;

    loop {
        let candidate = format!("new_world_{index}");
        if !world_list
            .worlds
            .values()
            .any(|world| world.name == candidate)
        {
            return candidate;
        }
        index += 1;
    }
}

pub fn solo_action(
    (interaction_query, mut name_query, mut list_query): (
        Query<(&Interaction, &SoloButtonAction), (Changed<Interaction>, With<Button>)>,
        Query<&mut TextInputValue, With<WorldNameInput>>,
        Query<(Entity, &mut WorldList), With<WorldList>>,
    ),
    (asset_server, mut menu_state, mut game_state, mut world_map, mut selected_world): (
        Res<AssetServer>,
        ResMut<NextState<MenuState>>,
        ResMut<NextState<GameState>>,
        ResMut<ClientWorldMap>,
        ResMut<SelectedWorld>,
    ),
    mut commands: Commands,
    paths: Res<GameFolderPaths>,
) {
    if list_query.is_empty() {
        return;
    }

    let (entity, mut list) = list_query.single_mut().unwrap();

    for (interaction, menu_button_action) in &interaction_query {
        if *interaction == Interaction::Pressed {
            match *menu_button_action {
                SoloButtonAction::Add => {
                    debug!("Interactions !");
                    if !name_query.is_empty() {
                        let mut name = name_query.single_mut().unwrap();

                        // if no name, create default one
                        let new_name = if name.0.is_empty() {
                            generate_new_world_name(&list)
                        } else {
                            name.0.clone()
                        };

                        if !is_valid_world_name(&new_name) {
                            warn!("Not creating world {new_name:?}: invalid name");
                            continue;
                        }
                        if list.worlds.values().any(|world| world.name == new_name) {
                            warn!("Not creating world {new_name:?}: it already exists");
                            continue;
                        }

                        add_world_item(
                            new_name,
                            &mut commands,
                            &asset_server,
                            &mut list,
                            entity,
                            &mut world_map,
                            &paths,
                        );

                        name.0 = "".into();
                        debug!("Creating world");
                    }
                }
                SoloButtonAction::Load(world_entity) => {
                    if let Some(world) = list.worlds.get(&world_entity) {
                        // update ressource name
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
        Ok(_) => info!(
            "Successfully deleted world directory: {}",
            world_dir.display()
        ),
        Err(ref e) if e.kind() == io::ErrorKind::NotFound => {
            error!(
                "World directory not found, skipping: {}",
                world_dir.display()
            )
        }
        Err(e) => error!(
            "Failed to delete world directory {}: {}",
            world_dir.display(),
            e
        ),
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn lists_the_folders_that_hold_a_world() {
        let folder = TempFolder::new("list");
        make_world(&folder.0, "alpha");
        fs::create_dir(folder.0.join("empty")).unwrap();
        fs::write(folder.0.join("file"), "").unwrap();

        assert_eq!(saved_world_names(&folder.0), vec!["alpha".to_string()]);
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
