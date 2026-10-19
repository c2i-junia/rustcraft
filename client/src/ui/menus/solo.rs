use super::list_screen::*;
use super::worlds::{self, check_world_name, delete_world, start_world, MAX_WORLD_NAME_LENGTH};
use super::MenuState;
use crate::constants::SAVE_PATH;
use crate::state::GameState;
use crate::ui::widgets::text_field::{FieldError, Submit};
use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use bevy::text::EditableText;
use shared::GameFolderPaths;

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

    // This list stays alphabetical until the world cards replace it
    let mut names: Vec<String> = worlds::list_worlds(&save_path)
        .into_iter()
        .map(|world| world.name)
        .collect();
    names.sort();
    for name in names {
        add_world_item(name, &mut commands, &assets, &mut list, list_entity);
    }
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
    if typed.is_empty() {
        return Ok(generate_new_world_name(list));
    }
    let others: Vec<String> = list
        .worlds
        .values()
        .map(|world| world.name.clone())
        .collect();
    check_world_name(typed, &others)
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
                    start_world(
                        &world.name,
                        &mut selected_world,
                        &mut game_state,
                        &mut menu_state,
                    );
                }
            }
            SoloButtonAction::Delete(world_entity) => {
                if let Some(world) = list.worlds.get(&world_entity) {
                    let saves = paths.game_folder_path.join(SAVE_PATH);
                    if let Err(e) = delete_world(&saves, &world.name) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::menus::worlds::{INVALID_WORLD_NAME, WORLD_EXISTS};

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
}
