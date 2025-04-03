use bevy::prelude::*;
use ron::de::from_str;
use shared::world::data::{ServerWorldMap, WorldSeed};
use shared::world::get_game_folder;
use shared::GameFolderPaths;
use std::fs;
use std::path::Path;

use crate::world::data::SAVE_PATH;
use std::path::PathBuf;

#[derive(serde::Serialize, serde::Deserialize)]
pub struct WorldData {
    pub seed: WorldSeed,
    pub map: ServerWorldMap,
    pub time: u64,
}

pub fn load_world_data(
    file_name: &str,
    app: &App,
) -> Result<WorldData, Box<dyn std::error::Error>> {
    let game_folder_path = app.world().get_resource::<GameFolderPaths>().unwrap();

    let file_path: PathBuf = get_game_folder(Some(game_folder_path))
        .join(SAVE_PATH)
        .join(format!("{file_name}.ron"));
    let path: &Path = file_path.as_path();

    if !path.exists() {
        info!(
            "World data file not found: {}. Generating default world and seed.",
            file_path.display()
        );
        let seed = WorldSeed(rand::random::<u32>());
        return Ok(WorldData {
            map: ServerWorldMap {
                name: file_name.to_string(),
                ..Default::default()
            },
            seed,
            time: 0,
        });
    }

    let contents: String = fs::read_to_string(path)?;
    let world_data: WorldData = from_str(&contents)?;

    info!("Found world data file from disk: {}", file_path.display());

    Ok(world_data)
}

pub fn load_world_map(
    file_name: &str,
    app: &App,
) -> Result<ServerWorldMap, Box<dyn std::error::Error>> {
    let world_data = load_world_data(file_name, app)?;
    Ok(world_data.map)
}

pub fn load_world_time(file_name: &str, app: &App) -> Result<u64, Box<dyn std::error::Error>> {
    let world_data = load_world_data(file_name, app)?;
    Ok(world_data.time)
}

pub fn load_world_seed(
    file_name: &str,
    app: &App,
) -> Result<WorldSeed, Box<dyn std::error::Error>> {
    let world_data = load_world_data(file_name, app)?;
    Ok(world_data.seed)
}
