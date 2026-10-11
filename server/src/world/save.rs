use crate::init::ServerTime;
use bevy::prelude::*;
use ron::ser::PrettyConfig;
use shared::messages::PlayerId;
use shared::players::Player;
use shared::world::Chunk;
use shared::world::MobId;
use shared::world::ServerMob;
use shared::world::WorldSeed;
use shared::GameFolderPaths;
use std::collections::HashMap;
use std::{fs::File, io::Write, path::Path};

#[derive(Message)]
pub enum SaveRequestEvent {
    World,
    Player(PlayerId),
}

use crate::world::data::{ServerWorldMap, SAVE_PATH};

#[derive(serde::Serialize, serde::Deserialize, Default)]
pub struct WorldData {
    pub map: HashMap<IVec3, Chunk>,
    pub mobs: HashMap<MobId, ServerMob>,
    pub seed: WorldSeed,
    pub name: String,
    pub time: u64,
}

pub fn save_world_system(
    world_map: Res<ServerWorldMap>,
    world_seed: Res<WorldSeed>,
    game_folder_path: Res<GameFolderPaths>,
    time: Res<ServerTime>,
    mut event: MessageReader<SaveRequestEvent>,
) {
    // Reads all events to prevent them from being queued forever and repeatedly request a save
    let mut save_requested = false;
    for ev in event.read() {
        save_requested = true;

        if let SaveRequestEvent::Player(id) = ev {
            if let Some(player) = world_map.players.get(id) {
                save_player(&world_map.name, player, &game_folder_path);
            }
        }
    }

    // If a save was requested by the user
    if save_requested {
        let world_data = WorldData {
            map: world_map
                .chunks
                .chunk_positions()
                .filter_map(|pos| Some((pos, world_map.chunks.chunk(pos)?.clone())))
                .collect(),
            mobs: world_map.mobs.clone(),
            name: world_map.name.clone(),
            seed: *world_seed,
            time: time.0,
        };

        // define save file path
        let save_file_path = format!(
            "{}{}/world.ron",
            game_folder_path.game_folder_path.join(SAVE_PATH).display(),
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

/// Writes `player` into the save folder of the world `world_name`.
pub fn save_player(world_name: &str, player: &Player, game_folder_path: &GameFolderPaths) {
    let save_file_path = format!(
        "{}{}/players/{}.ron",
        game_folder_path.game_folder_path.join(SAVE_PATH).display(),
        world_name,
        player.id
    );

    if let Err(err) = save_player_data(player, &save_file_path) {
        error!(
            "[{}] Could not save data for player {} : {}",
            world_name, player.id, err
        );
    } else {
        info!(
            "[{}] Player {} data saved successfully",
            world_name, player.id
        );
    }
}

pub fn save_world_data(
    world_data: &WorldData,
    file_path: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    // configure RON serialization: the fields and the chunks are listed one per line, but the
    // cells of a chunk (4096 of them) are written on the line of their chunk, without indices
    let pretty_config = PrettyConfig::new()
        .with_depth_limit(2)
        .with_separate_tuple_members(true);

    // serialize combined data (map + seed)
    let serialized = ron::ser::to_string_pretty(world_data, pretty_config)?;
    write_atomically(Path::new(file_path), serialized.as_bytes())?;
    info!("World data saved to {}", file_path);
    Ok(())
}

pub fn save_player_data(
    player: &Player,
    file_path: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    // RON Serialization config
    let pretty_config = PrettyConfig::new()
        .with_depth_limit(3)
        .with_separate_tuple_members(true)
        .with_enumerate_arrays(true);

    // Serialize Complete player data
    let serialized = ron::ser::to_string_pretty(player, pretty_config)?;
    write_atomically(Path::new(file_path), serialized.as_bytes())?;

    Ok(())
}

/// Writes `contents` to `<path>.tmp`, syncs it, then renames it over `path`, so a crash
/// leaves either the old file or the new one, never a truncated one.
fn write_atomically(path: &Path, contents: &[u8]) -> std::io::Result<()> {
    let mut tmp_name = path.as_os_str().to_owned();
    tmp_name.push(".tmp");
    let tmp_path = Path::new(&tmp_name);

    let mut file = File::create(tmp_path)?;
    file.write_all(contents)?;
    file.sync_all()?;
    std::fs::rename(tmp_path, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_atomically_creates_then_replaces_the_file() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("data.ron");

        write_atomically(&path, b"first").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"first");

        write_atomically(&path, b"second, longer").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"second, longer");
        assert!(!tmp.path().join("data.ron.tmp").exists());
    }

    #[test]
    fn failed_write_leaves_the_previous_file_untouched() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("data.ron");
        write_atomically(&path, b"precious").unwrap();

        // A directory squatting on the temporary name makes the new write fail early.
        std::fs::create_dir(tmp.path().join("data.ron.tmp")).unwrap();
        assert!(write_atomically(&path, b"lost").is_err());

        assert_eq!(std::fs::read(&path).unwrap(), b"precious");
    }

    #[test]
    fn save_world_data_writes_a_parseable_file() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("world.ron");
        let world = WorldData {
            name: "w".into(),
            time: 99,
            ..Default::default()
        };

        save_world_data(&world, path.to_str().unwrap()).unwrap();

        let loaded: WorldData = ron::de::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        assert_eq!(loaded.name, "w");
        assert_eq!(loaded.time, 99);
    }

    #[test]
    fn a_saved_chunk_takes_one_line_without_cell_comments() {
        use shared::world::{BlockData, BlockDirection, BlockId, Chunk};

        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("world.ron");
        let mut chunk = Chunk::new();
        chunk.set(
            IVec3::new(1, 2, 3),
            BlockData::new(BlockId::Stone, BlockDirection::Front),
        );
        let mut world = WorldData::default();
        world.map.insert(IVec3::ZERO, chunk.clone());

        save_world_data(&world, path.to_str().unwrap()).unwrap();

        let text = std::fs::read_to_string(path).unwrap();
        assert!(!text.contains("//"), "{}", &text[..200]);
        assert!(text.lines().count() < 20, "{} lines", text.lines().count());
        let loaded: WorldData = ron::de::from_str(&text).unwrap();
        assert_eq!(loaded.map[&IVec3::ZERO], chunk);
    }
}
