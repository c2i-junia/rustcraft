pub mod dialogs;

use super::list_screen::*;
use super::worlds::{self, delete_world, start_world};
use super::MenuState;
use crate::constants::SAVE_PATH;
use crate::state::GameState;
use bevy::input_focus::InputFocus;
use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use dialogs::{open_new_world_dialog, WorldsChanged};
use shared::GameFolderPaths;

pub struct WorldItem {
    pub name: String,
}

#[derive(Component, Default)]
pub struct WorldList {
    pub worlds: HashMap<Entity, WorldItem>,
}

impl WorldList {
    /// The names of the listed worlds.
    fn names(&self) -> impl Iterator<Item = String> + '_ {
        self.worlds.values().map(|world| world.name.clone())
    }
}

#[derive(Component)]
pub enum SoloButtonAction {
    Add,
    Load(Entity),
    Delete(Entity),
}

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
            inputs: &[],
            add_label: "New world",
            stacked: true,
        },
    );
    commands.entity(screen.list).insert(WorldList::default());
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

/// The dialogs of the world list, and the row of the world they create.
pub fn solo_plugin(app: &mut App) {
    dialogs::plugin(app);
    app.add_observer(add_created_world);
}

/// Adds the row of the world that was just created.
fn add_created_world(
    changed: On<WorldsChanged>,
    mut list_query: Query<(Entity, &mut WorldList)>,
    mut commands: Commands,
    assets: Res<AssetServer>,
) {
    let (Some(name), Ok((list_entity, mut list))) = (&changed.focus, list_query.single_mut())
    else {
        return;
    };
    add_world_item(name.clone(), &mut commands, &assets, &mut list, list_entity);
}

pub fn solo_action(
    interactions: Query<(&Interaction, &SoloButtonAction), (Changed<Interaction>, With<Button>)>,
    mut list_query: Query<(Entity, &mut WorldList)>,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    focus: Res<InputFocus>,
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
                let others = list.names().collect();
                open_new_world_dialog(&mut commands, &asset_server, &focus, others);
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
