use bevy::prelude::*;
use ron::de::from_str;
use shared::messages::{PlayerId, PlayerSave};
use shared::world::data::WorldSeed;
use shared::world::get_game_folder;
use shared::GameFolderPaths;
use std::fs;
use std::path::Path;

use crate::world::data::SAVE_PATH;
use crate::world::save::WorldData;
use std::path::PathBuf;

pub fn load_world_data(
    file_name: &str,
    game_folder_paths: Option<&GameFolderPaths>,
) -> Result<WorldData, Box<dyn std::error::Error>> {
    let file_path: PathBuf = get_game_folder(game_folder_paths)
        .join(SAVE_PATH)
        .join(format!("{file_name}/world.ron"));
    let path: &Path = file_path.as_path();

    if !path.exists() {
        info!(
            "World data file not found: {}. Generating default world and seed.",
            file_path.display()
        );
        let seed = WorldSeed(rand::random::<u32>());
        return Ok(WorldData {
            name: file_name.to_string(),
            seed,
            ..default()
        });
    }

    let contents: String = fs::read_to_string(path)?;
    let world_data: WorldData = from_str(&contents)?;

    info!("Found world data file from disk: {}", file_path.display());

    Ok(world_data)
}

pub fn load_player_data(
    world_name: &str,
    player_id: &PlayerId,
    game_folder_paths: &GameFolderPaths,
) -> PlayerSave {
    let file_path: PathBuf = get_game_folder(Some(game_folder_paths))
        .join(SAVE_PATH)
        .join(format!("{world_name}/players/{player_id}.ron"));
    let path: &Path = file_path.as_path();

    if path.exists() {
        if let Ok(contents) = fs::read_to_string(path) {
            if let Ok(player_data) = from_str::<PlayerSave>(&contents) {
                info!("Found player data file from disk: {}", file_path.display());

                return player_data;
            }
        }
    } else {
        info!(
            "Player data file not found: {}. Generating default world and seed.",
            file_path.display()
        );
    }

    PlayerSave {
        position: Vec3::new(0., 80., 0.),
        camera_transform: Transform::default(),
        is_flying: false,
    }
}
