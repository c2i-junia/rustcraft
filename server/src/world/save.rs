use crate::init::ServerTime;
use bevy::prelude::*;
use ron::ser::PrettyConfig;
use shared::world::get_game_folder;
use shared::world::ServerWorldMap;
use shared::world::WorldSeed;
use shared::GameFolderPaths;
use std::{fs::File, io::Write, path::Path};

#[derive(Event)]
pub struct SaveRequestEvent;

use crate::world::data::SAVE_PATH;

#[derive(serde::Serialize, serde::Deserialize)]
pub struct WorldData {
    pub seed: WorldSeed,
    pub map: ServerWorldMap,
    pub time: u64,
}

pub fn save_world_system(
    world_map: Res<ServerWorldMap>,
    world_seed: Res<WorldSeed>,
    game_folder_path: Res<GameFolderPaths>,
    time: Res<ServerTime>,
    mut event: EventReader<SaveRequestEvent>,
) {
    // Reads all events to prevent them from being queued forever and repeatedly request a save
    let mut save_requested = false;
    for _ in event.read() {
        save_requested = true;
    }

    // If a save was requested by the user
    if save_requested {
        let world_data = WorldData {
            map: world_map.clone(),
            seed: *world_seed,
            time: time.0,
        };

        // define save file path
        let save_file_path = format!(
            "{}{}.ron",
            get_game_folder(Some(&game_folder_path))
                .join(SAVE_PATH)
                .display(),
            world_map.name
        );

        // save seed and world data
        if let Err(e) = save_world_data(&world_data, &save_file_path) {
            error!("Failed to save world data: {}", e);
        } else {
            info!("World data saved successfully! Name: {}", world_map.name);
        }
    }
}

pub fn save_world_data(
    world_data: &WorldData,
    file_path: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    // configure RON serialization
    let pretty_config = PrettyConfig::new()
        .with_depth_limit(3)
        .with_separate_tuple_members(true)
        .with_enumerate_arrays(true);

    // serialize combined data (map + seed)
    let serialized = ron::ser::to_string_pretty(world_data, pretty_config)?;
    let path = Path::new(file_path);
    let mut file = File::create(path)?;
    file.write_all(serialized.as_bytes())?;
    info!("World data saved to {}", file_path);
    Ok(())
}
