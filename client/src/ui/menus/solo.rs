use super::{MenuButtonAction, MenuState, ScrollingList};
use crate::ui::assets::*;
use crate::ui::style::*;
use crate::world::ClientWorldMap;
use crate::{constants::SAVE_PATH, GameState, LoadWorldEvent};
use bevy::platform::collections::HashMap;
use bevy::prelude::Resource;
use bevy::prelude::*;
use bevy::{
    asset::AssetServer,
    color::Color,
    prelude::{
        Button, Changed, Commands, Component, Entity, EventWriter, NextState, Query, Res, ResMut,
        StateScoped, Text, With,
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
use shared::world::get_game_folder;
use shared::GameFolderPaths;
use std::io;
use std::{
    fs,
    path::{Path, PathBuf},
};

pub struct WorldItem {
    pub name: String,
}

#[derive(Component, Default)]
pub struct WorldList {
    pub worlds: HashMap<Entity, WorldItem>,
}

#[derive(Component)]
pub enum MultiplayerButtonAction {
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

pub fn solo_menu_setup(
    mut commands: Commands,
    assets_server: Res<AssetServer>,
    _paths: Res<GameFolderPaths>,
) {
    let background_image = load_background_image(&assets_server);
    let font = load_font(&assets_server);
    let button_background_image = load_button_background_large_image(&assets_server);

    let txt_font = TextFont {
        font: font.clone(),
        font_size: 20.,
        ..default()
    };

    let txt_color = TextColor(Color::WHITE);

    // let txt_font_inactive = TextFont {
    //     font,
    //     font_size: 20.,
    //     ..default()
    // };

    // let txt_color_inactive = TextColor(Color::srgb(0.3, 0.3, 0.3));

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
            StateScoped(MenuState::Solo),
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
            ImageNode::new(background_image),
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
                BorderColor(BACKGROUND_COLOR),
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
                            BorderColor(BACKGROUND_COLOR),
                            BackgroundColor(Color::BLACK),
                            node,
                        ),
                        WorldNameInput,
                        (
                            TextInput,
                            TextInputSettings {
                                retain_on_submit: true,
                                mask_character: None,
                            },
                            TextInputPlaceholder {
                                value: "World name".into(),
                                // text_style: Some(txt_style_inactive.clone()),
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
                                BorderColor(Color::BLACK),
                                BackgroundColor(BACKGROUND_COLOR),
                                {
                                    let mut style = btn_style.clone();
                                    style.grid_column = GridPlacement::span(2);
                                    style
                                },
                                ImageNode::new(button_background_image.clone()),
                            ),
                            MultiplayerButtonAction::Add,
                        ))
                        .with_children(|btn| {
                            btn.spawn((Text::new("Create world"), txt_font.clone(), txt_color));
                        });

                    wrapper
                        .spawn((
                            (
                                Button,
                                BorderColor(Color::BLACK),
                                BackgroundColor(BACKGROUND_COLOR),
                                {
                                    let mut style = btn_style.clone();
                                    style.grid_column = GridPlacement::span(2);
                                    style
                                },
                                ImageNode::new(button_background_image.clone()),
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

    // create save folder if it not exist
    let save_path: PathBuf = get_game_folder(Some(&game_paths)).join(SAVE_PATH);
    let path: &Path = save_path.as_path();
    if !fs::exists(path).unwrap() && fs::create_dir_all(path).is_ok() {
        info!("Successfully created the saves folder : {}", path.display());
    }

    let paths = fs::read_dir(path).unwrap();

    for path in paths {
        let path_str = path.unwrap().file_name().into_string().unwrap();

        if path_str.ends_with(".ron") {
            add_world_item(
                path_str.replace(".ron", ""),
                &mut commands,
                &assets,
                &mut list,
                list_entity,
                &mut world_map,
                &game_paths,
            );
        }
    }
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

    let base_path = &paths.assets_folder_path;

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
            BorderColor(BACKGROUND_COLOR),
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
        .spawn((
            MultiplayerButtonAction::Load(world),
            (Button, btn_style.clone()),
        ))
        .with_children(|btn| {
            let icon = asset_server.load(format!("{base_path}/graphics/play.png"));
            btn.spawn((ImageNode::new(icon), img_style.clone()));
        })
        .id();

    let delete_btn = commands
        .spawn((
            MultiplayerButtonAction::Delete(world),
            (Button, btn_style.clone()),
        ))
        .with_children(|btn| {
            let icon = asset_server.load(format!("{base_path}/graphics/trash.png"));
            btn.spawn((ImageNode::new(icon), img_style.clone()));
        })
        .id();

    let txt = commands
        .spawn((
            (
                Text::new(format!("{name}\n")),
                TextFont {
                    font: asset_server.load("./fonts/RustCraftRegular-Bmg3.otf"),
                    font_size: 20.,
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
        Query<(&Interaction, &MultiplayerButtonAction), (Changed<Interaction>, With<Button>)>,
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
    mut load_event: EventWriter<LoadWorldEvent>,
    paths: Res<GameFolderPaths>,
) {
    if list_query.is_empty() {
        return;
    }

    let (entity, mut list) = list_query.single_mut().unwrap();

    for (interaction, menu_button_action) in &interaction_query {
        if *interaction == Interaction::Pressed {
            match *menu_button_action {
                MultiplayerButtonAction::Add => {
                    debug!("Interactions !");
                    if !name_query.is_empty() {
                        let mut name = name_query.single_mut().unwrap();

                        // if no name, create default one
                        let new_name = if name.0.is_empty() {
                            generate_new_world_name(&list)
                        } else {
                            name.0.clone()
                        };

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
                MultiplayerButtonAction::Load(world_entity) => {
                    if let Some(world) = list.worlds.get(&world_entity) {
                        // update ressource name
                        selected_world.name = Some(world.name.clone());

                        load_event.write(LoadWorldEvent {
                            world_name: world.name.clone(),
                        });
                        game_state.set(GameState::PreGameLoading);
                        menu_state.set(MenuState::Disabled);
                    }
                }
                MultiplayerButtonAction::Delete(world_entity) => {
                    if let Some(world) = list.worlds.get(&world_entity) {
                        if let Err(e) = delete_save_files(&world.name, &paths) {
                            error!("Error while deleting save files: {}", e);
                        }
                        list.worlds.remove(&world_entity);
                    }
                    commands.entity(entity).remove_children(&[world_entity]);
                    commands.entity(world_entity).despawn();
                }
            }
        }
    }
}

pub fn delete_save_files(
    world_name: &str,
    game_folder_path: &Res<GameFolderPaths>,
) -> Result<(), io::Error> {
    // Delete `world_save.ron`
    match fs::remove_file(format!(
        "{}{}.ron",
        get_game_folder(Some(game_folder_path))
            .join(SAVE_PATH)
            .display(),
        world_name
    )) {
        Ok(_) => info!("Successfully deleted world"),
        Err(ref e) if e.kind() == io::ErrorKind::NotFound => {
            error!("world_save.ron not found, skipping.")
        }
        Err(e) => error!("Failed to delete world: {}", e),
    }

    Ok(())
}
