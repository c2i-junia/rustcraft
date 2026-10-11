use bevy::prelude::*;
use ron::de::from_str;
use shared::messages::PlayerId;
use shared::players::Player;
use shared::world::data::WorldSeed;
use shared::GameFolderPaths;
use std::fs;
use std::path::Path;

use crate::world::data::SAVE_PATH;
use crate::world::legacy::LegacyWorldData;
use crate::world::save::WorldData;
use std::path::PathBuf;

pub fn load_world_data(
    file_name: &str,
    game_folder_paths: &GameFolderPaths,
) -> Result<WorldData, Box<dyn std::error::Error>> {
    let file_path: PathBuf = game_folder_paths
        .game_folder_path
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

    let contents: String =
        fs::read_to_string(path).map_err(|err| format!("{}: {err}", file_path.display()))?;
    let world_data =
        parse_world_data(&contents).map_err(|err| format!("{}: {err}", file_path.display()))?;

    info!("Found world data file from disk: {}", file_path.display());

    Ok(world_data)
}

/// Reads a `world.ron`, written either by this version or by the one that stored chunks as maps.
/// Anything else is an error.
fn parse_world_data(contents: &str) -> Result<WorldData, String> {
    from_str::<WorldData>(contents).or_else(|err| {
        from_str::<LegacyWorldData>(contents)
            .map(WorldData::from)
            .map_err(|legacy_err| {
                format!("not a world file in the current format ({err}) nor in the old one ({legacy_err})")
            })
    })
}

/// Loads the saved player `player_id` of `world_name`, or a new player at the default spawn.
///
/// Only what is worth persisting is taken from the file: position, camera, fly mode and
/// inventory. A file that cannot be read is reported and moved aside to `<name>.ron.corrupt`
/// (copied if it cannot be moved) so that the next save does not destroy it. If it cannot
/// be preserved either, this fails: the caller must not let the player play, as the next
/// save would overwrite the unreadable file.
pub fn load_player_data(
    world_name: &str,
    player_id: &PlayerId,
    player_name: &str,
    game_folder_paths: &GameFolderPaths,
) -> Result<Player, String> {
    let file_path: PathBuf = game_folder_paths
        .game_folder_path
        .join(SAVE_PATH)
        .join(format!("{world_name}/players/{player_id}.ron"));
    let path: &Path = file_path.as_path();

    let mut player = Player::new(
        *player_id,
        player_name.to_string(),
        Vec3::new(0., 80., 0.),
        Transform::default(),
    );

    if !path.exists() {
        info!(
            "Player data file not found: {}. Spawning a new player.",
            file_path.display()
        );
        return Ok(player);
    }

    let saved = fs::read_to_string(path)
        .map_err(|err| err.to_string())
        .and_then(|contents| from_str::<Player>(&contents).map_err(|err| err.to_string()));
    match saved {
        Ok(saved) => {
            info!("Found player data file from disk: {}", file_path.display());
            player.position = saved.position;
            player.camera_transform = saved.camera_transform;
            player.is_flying = saved.is_flying;
            player.inventory = saved.inventory;
        }
        Err(err) => {
            let mut corrupt_path = path.as_os_str().to_owned();
            corrupt_path.push(".corrupt");
            error!(
                "Player data file {} is unreadable ({}), moving it to {:?} and spawning a new player",
                file_path.display(),
                err,
                corrupt_path
            );
            fs::rename(path, &corrupt_path)
                .or_else(|_| fs::copy(path, &corrupt_path).map(|_| ()))
                .map_err(|err| {
                    format!(
                        "could not preserve the unreadable player file {}: {err}",
                        file_path.display()
                    )
                })?;
        }
    }

    Ok(player)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::save::save_player_data;
    use shared::world::{ItemId, ItemStack};

    const WORLD: &str = "world";

    fn paths(tmp: &tempfile::TempDir) -> GameFolderPaths {
        let players_dir = tmp.path().join(SAVE_PATH).join(WORLD).join("players");
        fs::create_dir_all(players_dir).unwrap();
        GameFolderPaths {
            game_folder_path: tmp.path().to_path_buf(),
            assets_folder_path: tmp.path().join("assets"),
        }
    }

    fn player_file(tmp: &tempfile::TempDir, id: PlayerId) -> PathBuf {
        tmp.path()
            .join(SAVE_PATH)
            .join(WORLD)
            .join(format!("players/{id}.ron"))
    }

    fn dirt(nb: u32) -> ItemStack {
        ItemStack {
            item_id: ItemId::Dirt,
            item_type: ItemId::Dirt.get_default_type(),
            nb,
        }
    }

    #[test]
    fn saved_player_is_restored_with_its_inventory() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = paths(&tmp);
        let mut saved = Player::new(
            7,
            "old name".into(),
            Vec3::new(1.5, 90.25, -3.0),
            Transform::from_xyz(1., 2., 3.).looking_at(Vec3::new(5., 2., 3.), Vec3::Y),
        );
        saved.is_flying = true;
        saved.inventory.inner.insert(0, dirt(12));
        saved.inventory.inner.insert(5, dirt(3));
        save_player_data(&saved, player_file(&tmp, 7).to_str().unwrap()).unwrap();

        let loaded = load_player_data(WORLD, &7, "new name", &paths).unwrap();

        assert_eq!(loaded.id, 7);
        assert_eq!(loaded.name, "new name");
        assert_eq!(loaded.position, saved.position);
        assert_eq!(loaded.camera_transform, saved.camera_transform);
        assert!(loaded.is_flying);
        assert_eq!(loaded.inventory, saved.inventory);
    }

    #[test]
    fn missing_file_spawns_a_new_player() {
        let tmp = tempfile::tempdir().unwrap();
        let loaded = load_player_data(WORLD, &7, "fresh", &paths(&tmp)).unwrap();

        assert_eq!(loaded.position, Vec3::new(0., 80., 0.));
        assert!(!loaded.is_flying);
        assert!(loaded.inventory.inner.is_empty());
    }

    #[test]
    fn corrupt_file_is_moved_aside_and_not_overwritten() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = paths(&tmp);
        let file = player_file(&tmp, 7);
        fs::write(&file, "this is not ron").unwrap();

        let loaded = load_player_data(WORLD, &7, "fresh", &paths).unwrap();

        assert_eq!(loaded.position, Vec3::new(0., 80., 0.));
        assert!(!file.exists());
        let corrupt = file.with_extension("ron.corrupt");
        assert_eq!(fs::read_to_string(corrupt).unwrap(), "this is not ron");
    }

    #[test]
    fn unpreservable_corrupt_file_is_never_given_up_on() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = paths(&tmp);
        let file = player_file(&tmp, 7);
        fs::write(&file, "this is not ron").unwrap();
        // A non-empty directory squats the quarantine name: neither rename nor copy can work.
        let squatter = file.with_extension("ron.corrupt");
        fs::create_dir(&squatter).unwrap();
        fs::write(squatter.join("keep"), "x").unwrap();

        let result = load_player_data(WORLD, &7, "fresh", &paths);

        assert!(result.is_err());
        assert_eq!(fs::read_to_string(file).unwrap(), "this is not ron");
    }

    #[test]
    fn a_world_saved_with_item_stacks_still_loads() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = paths(&tmp);
        let world_dir = tmp.path().join(SAVE_PATH).join(WORLD);
        fs::write(
            world_dir.join("world.ron"),
            r#"(map: {}, mobs: {}, seed: (42), name: "world", time: 3, item_stacks: [])"#,
        )
        .unwrap();

        let data = load_world_data(WORLD, &paths).unwrap();

        assert_eq!(data.seed.0, 42);
        assert_eq!(data.time, 3);
    }

    fn write_world_file(tmp: &tempfile::TempDir, contents: &str) -> GameFolderPaths {
        let paths = paths(tmp);
        fs::write(
            tmp.path().join(SAVE_PATH).join(WORLD).join("world.ron"),
            contents,
        )
        .unwrap();
        paths
    }

    #[test]
    fn a_world_in_the_current_format_round_trips() {
        use shared::world::{BlockData, BlockDirection, BlockId, Chunk};

        let tmp = tempfile::tempdir().unwrap();
        let paths = paths(&tmp);
        let mut chunk = Chunk::new();
        chunk.set(
            IVec3::new(1, 2, 3),
            BlockData::new(BlockId::Stone, BlockDirection::Right),
        );
        let mut saved = WorldData {
            name: WORLD.into(),
            time: 5,
            ..Default::default()
        };
        saved.map.insert(IVec3::new(-1, 4, 2), chunk.clone());
        saved.map.insert(IVec3::ZERO, Chunk::new());
        let file = tmp.path().join(SAVE_PATH).join(WORLD).join("world.ron");
        crate::world::save::save_world_data(&saved, file.to_str().unwrap()).unwrap();

        let loaded = load_world_data(WORLD, &paths).unwrap();

        assert_eq!(loaded.map.len(), 2);
        assert_eq!(loaded.map[&IVec3::new(-1, 4, 2)], chunk);
        assert!(loaded.map[&IVec3::ZERO].is_empty());
        assert_eq!(loaded.time, 5);
    }

    #[test]
    fn a_world_saved_before_dense_chunks_still_loads() {
        use shared::world::BlockId;

        let tmp = tempfile::tempdir().unwrap();
        let paths = write_world_file(
            &tmp,
            r#"(map: {
                (0,0,0): (
                    map: {(1,2,3):(id:Stone,direction:Front,breaking_progress:4),(16,2,3):(id:OakLeaves,direction:Front,breaking_progress:0)},
                    ts: 7,
                    sent_to_clients: [5],
                ),
                (1,0,0): (map: {}, ts: 0, sent_to_clients: []),
            }, mobs: {}, seed: (42), name: "world", time: 3)"#,
        );

        let data = load_world_data(WORLD, &paths).unwrap();

        assert_eq!(data.seed.0, 42);
        assert_eq!(data.time, 3);
        let stone = data.map[&IVec3::ZERO].get(IVec3::new(1, 2, 3)).unwrap();
        assert_eq!((stone.id, stone.breaking_progress), (BlockId::Stone, 4));
        let leaf = data.map[&IVec3::X].get(IVec3::new(0, 2, 3)).unwrap();
        assert_eq!(leaf.id, BlockId::OakLeaves);
    }

    #[test]
    fn a_world_file_in_neither_format_is_an_error() {
        for contents in [
            "this is not ron",
            "",
            r#"(map: {(0,0,0): (blocks: [])}, mobs: {}, seed: (1), name: "w", time: 0)"#,
            r#"(map: {(0,0,0): [None, None]}, mobs: {}, seed: (1), name: "w", time: 0)"#,
        ] {
            let tmp = tempfile::tempdir().unwrap();
            let paths = write_world_file(&tmp, contents);
            assert!(load_world_data(WORLD, &paths).is_err(), "{contents:?}");
        }
    }

    #[test]
    fn a_world_written_with_one_line_per_cell_still_loads() {
        use shared::world::{BlockData, BlockDirection, BlockId, Chunk};

        // The layout of the first version of the dense format: indexed, one cell per line.
        let mut chunk = Chunk::new();
        chunk.set(
            IVec3::new(4, 5, 6),
            BlockData::new(BlockId::Sand, BlockDirection::Left),
        );
        let mut world = WorldData {
            name: WORLD.into(),
            ..Default::default()
        };
        world.map.insert(IVec3::new(2, 0, -1), chunk.clone());
        let config = ron::ser::PrettyConfig::new()
            .with_depth_limit(3)
            .with_separate_tuple_members(true)
            .with_enumerate_arrays(true);
        let text = ron::ser::to_string_pretty(&world, config).unwrap();
        assert!(text.lines().count() > 4096);

        let loaded = parse_world_data(&text).unwrap();

        assert_eq!(loaded.map[&IVec3::new(2, 0, -1)], chunk);
    }
}
