use bevy::prelude::*;
use ron::de::from_str;
use serde::Deserialize;
use shared::messages::PlayerId;
use shared::players::Player;
use shared::world::data::WorldSeed;
use shared::GameFolderPaths;
use std::fs;
use std::path::Path;

use crate::world::chunk_files::{has_chunk_files, read_chunks_with, write_chunk, CHUNKS_DIR};
use crate::world::data::{world_dir, SAVE_PATH};
use crate::world::format::{write_meta, WorldData, WorldMeta, FORMAT_VERSION, WORLD_FILE};
use crate::world::legacy::{decode_chunk_v1, LegacyWorldData};
#[cfg(test)]
use crate::world::{
    chunk_files::read_chunks,
    container_files::{read_containers, CONTAINERS_DIR},
};
use std::path::PathBuf;

/// What a `world.ron` turned out to hold.
enum WorldFile {
    /// The current layout: only the metadata, the chunks are in `chunks/` and the contents of
    /// the chests in `containers/`. Version 2, which had no chests, reads the same.
    Current(WorldMeta),
    /// The current layout, with chunk files of format version 1, whose blocks have no level.
    V1(WorldMeta),
    /// One of the two single-file formats, chunks included.
    SingleFile(WorldData),
}

/// Loads the world `file_name`: its metadata and every chunk file, as `open_world` does but
/// with the chunks and the chests read at once.
#[cfg(test)]
pub fn load_world_data(
    file_name: &str,
    game_folder_paths: &GameFolderPaths,
) -> Result<WorldData, Box<dyn std::error::Error>> {
    let world = open_world(file_name, game_folder_paths)?;
    let dir = world_dir(game_folder_paths, file_name)?;
    if !world.map.is_empty() {
        // a single-file world, converted: its chunks are already in memory
        return Ok(world);
    }
    Ok(read_chunks_of(&dir, world)?)
}

/// Opens the world `file_name`: its metadata only, the chunks are read from their files when
/// they are needed (`read_chunk`). A world saved in one of the two single-file formats is first
/// converted to the current layout, and keeps the chunks it held in memory. A world that does
/// not exist yet is a new one. Anything unreadable in `world.ron` is an error, so that a world
/// is never regenerated over a save; only a damaged chunk file is skipped.
pub fn open_world(
    file_name: &str,
    game_folder_paths: &GameFolderPaths,
) -> Result<WorldData, Box<dyn std::error::Error>> {
    let dir = world_dir(game_folder_paths, file_name)?;
    let file_path = dir.join(WORLD_FILE);

    if !file_path.exists() {
        // Chunks without the file that holds their seed: a new world would not match them
        let chunks_dir = dir.join(CHUNKS_DIR);
        if has_chunk_files(&chunks_dir).map_err(|err| format!("{}: {err}", chunks_dir.display()))? {
            return Err(format!(
                "{} is missing but {} holds chunk files: restore world.ron, or remove the folder {} to start a new world",
                file_path.display(),
                chunks_dir.display(),
                dir.display()
            )
            .into());
        }
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
        fs::read_to_string(&file_path).map_err(|err| format!("{}: {err}", file_path.display()))?;
    let world_file =
        parse_world_file(&contents).map_err(|err| format!("{}: {err}", file_path.display()))?;

    let world_data = match world_file {
        WorldFile::Current(meta) => WorldData::from(meta),
        WorldFile::V1(meta) => {
            convert_v1_chunks(&dir, &meta)
                .map_err(|err| format!("could not convert {}: {err}", dir.display()))?;
            WorldData::from(meta)
        }
        WorldFile::SingleFile(world_data) => {
            convert_single_file_world(&dir, &world_data)
                .map_err(|err| format!("could not convert {}: {err}", file_path.display()))?;
            world_data
        }
    };

    info!("Found world data file from disk: {}", file_path.display());

    Ok(world_data)
}

/// `world`, with the chunk and chest files of the folder of the world `dir`.
#[cfg(test)]
fn read_chunks_of(dir: &Path, world: WorldData) -> Result<WorldData, String> {
    let chunks_dir = dir.join(CHUNKS_DIR);
    let map = read_chunks(&chunks_dir).map_err(|err| format!("{}: {err}", chunks_dir.display()))?;
    let containers_dir = dir.join(CONTAINERS_DIR);
    let containers = read_containers(&containers_dir)
        .map_err(|err| format!("{}: {err}", containers_dir.display()))?;
    Ok(WorldData {
        map,
        containers,
        ..world
    })
}

/// Tells the layout of a `world.ron` by its `format_version`, which the single-file formats
/// lack. A version this code does not know is an error: a newer game wrote it.
fn parse_world_file(contents: &str) -> Result<WorldFile, String> {
    #[derive(Deserialize)]
    struct VersionProbe {
        #[serde(default)]
        format_version: u32,
    }

    let version = from_str::<VersionProbe>(contents)
        .map_err(|err| format!("not a world file ({err})"))?
        .format_version;
    match version {
        0 => parse_single_file_world(contents).map(WorldFile::SingleFile),
        1 => from_str::<WorldMeta>(contents)
            .map(WorldFile::V1)
            .map_err(|err| format!("not a valid world file ({err})")),
        2 | 3 | FORMAT_VERSION => from_str::<WorldMeta>(contents)
            .map(WorldFile::Current)
            .map_err(|err| format!("not a valid world file ({err})")),
        newer => Err(format!(
            "this world was saved in format version {newer}, newer than the version {FORMAT_VERSION} this game understands"
        )),
    }
}

/// Reads a `world.ron` written either by the version that stored chunks as sequences of cells
/// or by the one before, that stored them as maps. Anything else is an error.
fn parse_single_file_world(contents: &str) -> Result<WorldData, String> {
    from_str::<WorldData>(contents).or_else(|err| {
        from_str::<LegacyWorldData>(contents)
            .map(WorldData::from)
            .map_err(|legacy_err| {
                format!("not a world file in the current format ({err}) nor in the old one ({legacy_err})")
            })
    })
}

/// Writes the world loaded from a single `world.ron` in the current layout, then keeps the old
/// file as `world.ron.legacy`.
///
/// The old `world.ron` stays in place, untouched, until the very last step, and a `world.ron`
/// without a `format_version` is converted again whenever it is found. So the step that makes
/// the conversion count is the atomic replacement of `world.ron` by the new metadata, and a
/// process that dies before it leaves the old world to be converted again (the chunk files
/// already written are simply rewritten), while one that dies after it leaves a complete
/// new layout. There is no moment when `world.ron` is missing or half written.
fn convert_single_file_world(dir: &Path, world: &WorldData) -> Result<(), String> {
    let world_file = dir.join(WORLD_FILE);
    let chunks_dir = dir.join(CHUNKS_DIR);
    fs::create_dir_all(&chunks_dir).map_err(|err| format!("{}: {err}", chunks_dir.display()))?;
    for (&chunk_pos, chunk) in &world.map {
        write_chunk(&chunks_dir, chunk_pos, chunk)
            .map_err(|err| format!("chunk {chunk_pos}: {err}"))?;
    }
    // The chunk files must be on disk before the new world.ron says the world is complete
    sync_dir(&chunks_dir);

    let legacy_name = unused_backup_path(&world_file);
    fs::copy(&world_file, &legacy_name)
        .and_then(|_| fs::OpenOptions::new().write(true).open(&legacy_name))
        .and_then(|backup| backup.sync_all())
        .map_err(|err| format!("could not keep the old world file: {err}"))?;

    write_meta(dir, &WorldMeta::from(world)).map_err(|err| err.to_string())?;
    sync_dir(dir);
    info!(
        "Converted the world {} to one file per chunk, the old file is kept as {:?}",
        dir.display(),
        legacy_name
    );
    Ok(())
}

/// Name of the folder that keeps the chunk files of format version 1 once converted.
pub const V1_CHUNKS_DIR: &str = "chunks.v1";

/// Converts the chunk files of a world of format version 1, whose blocks had no level, then
/// writes its `world.ron` again with the current version. The old files are kept in
/// `chunks.v1`.
///
/// The old folder is first renamed to `chunks.v1`, and from then on is the source of the
/// conversion: a `chunks` folder next to it while `world.ron` still says version 1 is what an
/// interrupted conversion left, and is written again. The new `world.ron` comes last, so it
/// only says the current version once every chunk is converted.
fn convert_v1_chunks(dir: &Path, meta: &WorldMeta) -> Result<(), String> {
    let chunks_dir = dir.join(CHUNKS_DIR);
    let v1_dir = dir.join(V1_CHUNKS_DIR);
    fn at(path: &Path) -> impl Fn(std::io::Error) -> String + '_ {
        move |err| format!("{}: {err}", path.display())
    }

    if !v1_dir.exists() && chunks_dir.exists() {
        fs::rename(&chunks_dir, &v1_dir).map_err(at(&chunks_dir))?;
        sync_dir(dir);
    }
    let chunks = read_chunks_with(&v1_dir, decode_chunk_v1).map_err(at(&v1_dir))?;
    if chunks_dir.exists() {
        fs::remove_dir_all(&chunks_dir).map_err(at(&chunks_dir))?;
    }
    fs::create_dir_all(&chunks_dir).map_err(at(&chunks_dir))?;
    for (&chunk_pos, chunk) in &chunks {
        write_chunk(&chunks_dir, chunk_pos, chunk)
            .map_err(|err| format!("chunk {chunk_pos}: {err}"))?;
    }
    // The chunk files must be on disk before the new world.ron says the world is converted
    sync_dir(&chunks_dir);

    let converted = WorldMeta::new(meta.name.clone(), meta.seed, meta.time, meta.mobs.clone());
    write_meta(dir, &converted).map_err(|err| err.to_string())?;
    sync_dir(dir);
    info!(
        "Converted the {} chunks of the world {} to format version {FORMAT_VERSION}, the old files are kept in {V1_CHUNKS_DIR}",
        chunks.len(),
        dir.display()
    );
    Ok(())
}

/// `world.ron.legacy`, or `world.ron.legacy.1`, `.2`, ... when that name is taken: an earlier
/// backup is never overwritten.
fn unused_backup_path(world_file: &Path) -> PathBuf {
    let with_suffix = |suffix: String| {
        let mut name = world_file.as_os_str().to_owned();
        name.push(suffix);
        PathBuf::from(name)
    };
    let first = with_suffix(".legacy".into());
    if !first.exists() {
        return first;
    }
    (1..)
        .map(|n| with_suffix(format!(".legacy.{n}")))
        .find(|path| !path.exists())
        .unwrap_or(first)
}

/// Asks the system to write the entries of the folder `dir` to disk. Best effort: not every
/// platform can open a folder.
fn sync_dir(dir: &Path) {
    if let Ok(dir) = fs::File::open(dir) {
        let _ = dir.sync_all();
    }
}

/// Loads the saved player `player_id` of `world_name`, or a new player at `spawn`.
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
    spawn: Vec3,
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
        spawn,
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
            player.game_mode = saved.game_mode;
            player.health = saved.health;
            player.death_cause = saved.death_cause;
            player.inventory = saved.inventory;
            // A table or a chest is used only while the player is there
            player.inventory.open_table = None;
            player.inventory.open_chest = None;
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
    use crate::world::chunk_files::chunk_file_name;
    use crate::world::legacy::encode_chunk_v1;
    use crate::world::save::save_player_data;
    use shared::world::{ItemId, ItemStack};

    const WORLD: &str = "world";
    const SPAWN: Vec3 = Vec3::new(0.5, 66.0, 0.5);

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
        save_player_data(&saved, &player_file(&tmp, 7)).unwrap();

        let loaded = load_player_data(WORLD, &7, "new name", Vec3::ZERO, &paths).unwrap();

        assert_eq!(loaded.id, 7);
        assert_eq!(loaded.name, "new name");
        assert_eq!(loaded.position, saved.position);
        assert_eq!(loaded.camera_transform, saved.camera_transform);
        assert!(loaded.is_flying);
        assert_eq!(loaded.inventory, saved.inventory);
    }

    #[test]
    fn a_saved_player_keeps_its_game_mode_and_health_even_dead() {
        use shared::players::game_mode::{DeathCause, GameMode, MAX_HEALTH};

        let tmp = tempfile::tempdir().unwrap();
        let paths = paths(&tmp);
        let mut saved = Player::new(7, "builder".into(), Vec3::ZERO, Transform::default());
        saved.game_mode = GameMode::Creative;
        saved.health = 0;
        saved.death_cause = Some(DeathCause::Fall);
        save_player_data(&saved, &player_file(&tmp, 7)).unwrap();

        let loaded = load_player_data(WORLD, &7, "builder", SPAWN, &paths).unwrap();
        assert_eq!(loaded.game_mode, GameMode::Creative);
        assert!(loaded.is_dead());
        assert_eq!(loaded.death_cause, Some(DeathCause::Fall));

        // A save from before the game modes is a survival player in full health
        let old = "(id: 7, name: \"old\", position: (1.0, 2.0, 3.0), camera_transform: (translation: (0.0, 0.0, 0.0), rotation: (0.0, 0.0, 0.0, 1.0), scale: (1.0, 1.0, 1.0)), velocity: (0.0, 0.0, 0.0), on_ground: true, is_flying: true, inventory: (inner: {}), height: 1.8, width: 0.8, last_input_processed: 0)";
        fs::write(player_file(&tmp, 7), old).unwrap();
        let loaded = load_player_data(WORLD, &7, "old", SPAWN, &paths).unwrap();
        assert_eq!(loaded.position, Vec3::new(1.0, 2.0, 3.0));
        assert_eq!(loaded.game_mode, GameMode::Survival);
        assert_eq!(loaded.health, MAX_HEALTH);
    }

    #[test]
    fn a_loaded_player_has_no_open_table() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = paths(&tmp);
        let mut saved = Player::new(7, "crafter".into(), Vec3::ZERO, Transform::default());
        saved.inventory.open_table = Some(bevy::math::IVec3::new(1, 2, 3));
        save_player_data(&saved, &player_file(&tmp, 7)).unwrap();

        let loaded = load_player_data(WORLD, &7, "crafter", Vec3::ZERO, &paths).unwrap();

        assert_eq!(loaded.inventory.open_table, None);
    }

    #[test]
    fn missing_file_spawns_a_new_player() {
        let tmp = tempfile::tempdir().unwrap();
        let loaded = load_player_data(WORLD, &7, "fresh", SPAWN, &paths(&tmp)).unwrap();

        assert_eq!(loaded.position, SPAWN);
        assert!(!loaded.is_flying);
        assert!(loaded.inventory.inner.is_empty());
    }

    #[test]
    fn corrupt_file_is_moved_aside_and_not_overwritten() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = paths(&tmp);
        let file = player_file(&tmp, 7);
        fs::write(&file, "this is not ron").unwrap();

        let loaded = load_player_data(WORLD, &7, "fresh", SPAWN, &paths).unwrap();

        assert_eq!(loaded.position, SPAWN);
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

        let result = load_player_data(WORLD, &7, "fresh", SPAWN, &paths);

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

    /// The `world.ron` of the single-file format that stored chunks as sequences of cells.
    fn dense_single_file(world: &WorldData) -> String {
        let config = ron::ser::PrettyConfig::new()
            .with_depth_limit(2)
            .with_separate_tuple_members(true);
        ron::ser::to_string_pretty(world, config).unwrap()
    }

    fn sample_world() -> (WorldData, shared::world::Chunk) {
        use shared::world::{BlockData, BlockDirection, BlockId, Chunk};

        let mut chunk = Chunk::new();
        chunk.set(
            IVec3::new(1, 2, 3),
            BlockData::new(BlockId::Stone, BlockDirection::Right),
        );
        let mut saved = WorldData {
            name: WORLD.into(),
            seed: WorldSeed(77),
            time: 5,
            ..default()
        };
        saved.map.insert(IVec3::new(-1, 4, 2), chunk.clone());
        saved.map.insert(IVec3::ZERO, Chunk::new());
        (saved, chunk)
    }

    fn world_dir_of(tmp: &tempfile::TempDir) -> PathBuf {
        tmp.path().join(SAVE_PATH).join(WORLD)
    }

    fn file_names(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        names.sort();
        names
    }

    #[test]
    fn a_world_in_the_current_layout_round_trips() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = paths(&tmp);
        let (saved, chunk) = sample_world();
        let dir = world_dir_of(&tmp);
        write_meta(&dir, &WorldMeta::from(&saved)).unwrap();
        for (&pos, chunk) in &saved.map {
            fs::create_dir_all(dir.join(CHUNKS_DIR)).unwrap();
            write_chunk(&dir.join(CHUNKS_DIR), pos, chunk).unwrap();
        }

        let loaded = load_world_data(WORLD, &paths).unwrap();

        assert_eq!(loaded.map.len(), 2);
        assert_eq!(loaded.map[&IVec3::new(-1, 4, 2)], chunk);
        assert!(loaded.map[&IVec3::ZERO].is_empty());
        assert_eq!(
            (loaded.time, loaded.seed.0, loaded.name),
            (5, 77, WORLD.into())
        );
        // nothing is converted: no legacy file appears
        assert!(!dir.join("world.ron.legacy").exists());
    }

    #[test]
    fn the_chests_of_a_world_are_loaded_with_it() {
        use crate::world::container_files::write_containers;
        use shared::world::Container;
        use std::collections::HashMap;

        let tmp = tempfile::tempdir().unwrap();
        let paths = paths(&tmp);
        let (saved, _) = sample_world();
        let dir = world_dir_of(&tmp);
        write_meta(&dir, &WorldMeta::from(&saved)).unwrap();
        let mut container = Container::default();
        container.slots[3] = Some(ItemStack {
            item_id: ItemId::Stick,
            item_type: ItemId::Stick.get_default_type(),
            nb: 9,
        });
        fs::create_dir_all(dir.join(CONTAINERS_DIR)).unwrap();
        let containers = vec![(IVec3::new(1, 2, 3), container.clone())];
        write_containers(&dir.join(CONTAINERS_DIR), IVec3::new(-1, 4, 2), &containers).unwrap();

        let loaded = load_world_data(WORLD, &paths).unwrap();

        assert_eq!(
            loaded.containers,
            HashMap::from([(IVec3::new(-15, 66, 35), container)])
        );
    }

    #[test]
    fn a_world_of_format_version_2_loads_as_it_is() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = paths(&tmp);
        let (saved, chunk) = sample_world();
        let dir = world_dir_of(&tmp);
        let mut meta = WorldMeta::from(&saved);
        meta.format_version = 2;
        write_meta(&dir, &meta).unwrap();
        fs::create_dir_all(dir.join(CHUNKS_DIR)).unwrap();
        write_chunk(&dir.join(CHUNKS_DIR), IVec3::new(-1, 4, 2), &chunk).unwrap();

        let loaded = load_world_data(WORLD, &paths).unwrap();

        assert_eq!(loaded.map[&IVec3::new(-1, 4, 2)], chunk);
        assert!(loaded.containers.is_empty());
        assert_eq!(file_names(&dir), ["chunks", "players", "world.ron"]);
    }

    #[test]
    fn a_world_without_chunk_files_loads_with_no_chunk() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = paths(&tmp);
        let (saved, _) = sample_world();
        write_meta(&world_dir_of(&tmp), &WorldMeta::from(&saved)).unwrap();

        assert!(load_world_data(WORLD, &paths).unwrap().map.is_empty());
    }

    #[test]
    fn a_corrupt_chunk_file_is_skipped_and_the_rest_loads() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = paths(&tmp);
        let (saved, chunk) = sample_world();
        let dir = world_dir_of(&tmp);
        write_meta(&dir, &WorldMeta::from(&saved)).unwrap();
        fs::create_dir_all(dir.join(CHUNKS_DIR)).unwrap();
        write_chunk(&dir.join(CHUNKS_DIR), IVec3::X, &chunk).unwrap();
        fs::write(dir.join(CHUNKS_DIR).join("0_0_0.bin"), b"garbage").unwrap();

        let loaded = load_world_data(WORLD, &paths).unwrap();

        assert_eq!(
            loaded.map.keys().copied().collect::<Vec<_>>(),
            vec![IVec3::X]
        );
    }

    /// A world of format version 1: the metadata of `sample_world` and its chunks, written
    /// without their levels.
    fn v1_world(tmp: &tempfile::TempDir) -> (GameFolderPaths, shared::world::Chunk) {
        let paths = paths(tmp);
        let (saved, chunk) = sample_world();
        let dir = world_dir_of(tmp);
        let mut meta = WorldMeta::from(&saved);
        meta.format_version = 1;
        write_meta(&dir, &meta).unwrap();
        fs::create_dir_all(dir.join(CHUNKS_DIR)).unwrap();
        for (&pos, chunk) in &saved.map {
            fs::write(
                dir.join(CHUNKS_DIR).join(chunk_file_name(pos)),
                encode_chunk_v1(chunk),
            )
            .unwrap();
        }
        (paths, chunk)
    }

    fn assert_converted(tmp: &tempfile::TempDir, chunk: &shared::world::Chunk) {
        let dir = world_dir_of(tmp);
        let loaded = load_world_data(WORLD, &paths(tmp)).unwrap();
        assert_eq!(loaded.map.len(), 2);
        assert_eq!(&loaded.map[&IVec3::new(-1, 4, 2)], chunk);
        assert_eq!((loaded.seed.0, loaded.time), (77, 5));
        assert!(matches!(
            parse_world_file(&fs::read_to_string(dir.join(WORLD_FILE)).unwrap()),
            Ok(WorldFile::Current(_))
        ));
        assert_eq!(
            file_names(&dir.join(V1_CHUNKS_DIR)),
            ["-1_4_2.bin", "0_0_0.bin"]
        );
    }

    #[test]
    fn a_world_of_format_version_1_is_converted_and_its_old_chunks_kept() {
        let tmp = tempfile::tempdir().unwrap();
        let (_, chunk) = v1_world(&tmp);

        assert_converted(&tmp, &chunk);
        // A second load reads the converted world as it is
        assert_converted(&tmp, &chunk);
    }

    #[test]
    fn an_interrupted_conversion_of_format_version_1_is_redone() {
        let tmp = tempfile::tempdir().unwrap();
        let (_, chunk) = v1_world(&tmp);
        // The conversion stopped after renaming the old folder and writing one new file
        let dir = world_dir_of(&tmp);
        fs::rename(dir.join(CHUNKS_DIR), dir.join(V1_CHUNKS_DIR)).unwrap();
        fs::create_dir_all(dir.join(CHUNKS_DIR)).unwrap();
        fs::write(dir.join(CHUNKS_DIR).join("5_5_5.bin"), b"half written").unwrap();

        assert_converted(&tmp, &chunk);
        assert_eq!(
            file_names(&dir.join(CHUNKS_DIR)),
            ["-1_4_2.bin", "0_0_0.bin"]
        );
    }

    #[test]
    fn a_world_in_a_newer_format_version_is_refused_and_left_alone() {
        let tmp = tempfile::tempdir().unwrap();
        let text =
            r#"(format_version: 5, name: "world", seed: (1), time: 0, mobs: {}, biomes: [])"#;
        let paths = write_world_file(&tmp, text);

        let err = load_world_data(WORLD, &paths).err().unwrap().to_string();

        assert!(err.contains("world.ron"), "{err}");
        assert!(err.contains("version 5"), "{err}");
        assert_eq!(
            fs::read_to_string(world_dir_of(&tmp).join("world.ron")).unwrap(),
            text
        );
        assert_eq!(file_names(&world_dir_of(&tmp)), ["players", "world.ron"]);
    }

    #[test]
    fn a_single_file_world_is_converted_to_the_current_layout() {
        let tmp = tempfile::tempdir().unwrap();
        let (saved, chunk) = sample_world();
        let text = dense_single_file(&saved);
        let paths = write_world_file(&tmp, &text);
        let dir = world_dir_of(&tmp);

        let loaded = load_world_data(WORLD, &paths).unwrap();

        assert_eq!(loaded.map[&IVec3::new(-1, 4, 2)], chunk);
        assert_eq!(
            file_names(&dir),
            ["chunks", "players", "world.ron", "world.ron.legacy"]
        );
        assert_eq!(
            fs::read_to_string(dir.join("world.ron.legacy")).unwrap(),
            text
        );
        assert_eq!(
            file_names(&dir.join(CHUNKS_DIR)),
            ["-1_4_2.bin", "0_0_0.bin"]
        );
        // the next load reads the new layout and gives the same world
        let again = load_world_data(WORLD, &paths).unwrap();
        assert_eq!(again.map, loaded.map);
        assert_eq!((again.time, again.seed.0), (5, 77));
        assert!(fs::read_to_string(dir.join("world.ron"))
            .unwrap()
            .contains("format_version"));
    }

    #[test]
    fn a_world_of_the_map_format_is_converted_with_its_relocated_blocks() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = write_world_file(
            &tmp,
            r#"(map: {
                (0,0,0): (map: {(1,2,3):(id:Stone,direction:Front,breaking_progress:0),(16,2,3):(id:OakLeaves,direction:Front,breaking_progress:0)}, ts: 7, sent_to_clients: []),
                (1,0,0): (map: {}, ts: 0, sent_to_clients: []),
            }, mobs: {}, seed: (42), name: "world", time: 3)"#,
        );

        load_world_data(WORLD, &paths).unwrap();
        let reloaded = load_world_data(WORLD, &paths).unwrap();

        assert_eq!(reloaded.map.len(), 2);
        assert!(reloaded.map[&IVec3::X].get(IVec3::new(0, 2, 3)).is_some());
        assert_eq!((reloaded.seed.0, reloaded.time), (42, 3));
        assert!(world_dir_of(&tmp).join("world.ron.legacy").is_file());
    }

    #[test]
    fn a_conversion_interrupted_before_the_new_world_file_is_redone() {
        let tmp = tempfile::tempdir().unwrap();
        let (saved, _) = sample_world();
        let text = dense_single_file(&saved);
        let paths = write_world_file(&tmp, &text);
        let dir = world_dir_of(&tmp);
        // The process died after one chunk file and a partial temporary file were written
        fs::create_dir_all(dir.join(CHUNKS_DIR)).unwrap();
        write_chunk(
            &dir.join(CHUNKS_DIR),
            IVec3::ZERO,
            &shared::world::Chunk::new(),
        )
        .unwrap();
        fs::write(dir.join(CHUNKS_DIR).join("-1_4_2.bin.tmp"), b"partial").unwrap();

        let loaded = load_world_data(WORLD, &paths).unwrap();

        assert_eq!(loaded.map.len(), 2);
        assert_eq!(load_world_data(WORLD, &paths).unwrap().map, loaded.map);
        assert!(dir.join(CHUNKS_DIR).join("-1_4_2.bin").is_file());
    }

    #[test]
    fn a_conversion_that_fails_leaves_the_old_world_in_place() {
        let tmp = tempfile::tempdir().unwrap();
        let (saved, _) = sample_world();
        let text = dense_single_file(&saved);
        let paths = write_world_file(&tmp, &text);
        let dir = world_dir_of(&tmp);
        // a file where the chunk folder should be
        fs::write(dir.join(CHUNKS_DIR), b"in the way").unwrap();

        assert!(load_world_data(WORLD, &paths).is_err());

        assert_eq!(fs::read_to_string(dir.join("world.ron")).unwrap(), text);
        assert!(!dir.join("world.ron.legacy").exists());
    }

    #[test]
    fn a_second_conversion_keeps_the_first_backup() {
        let tmp = tempfile::tempdir().unwrap();
        let (saved, _) = sample_world();
        let first_text = dense_single_file(&saved);
        let paths = write_world_file(&tmp, &first_text);
        let dir = world_dir_of(&tmp);
        load_world_data(WORLD, &paths).unwrap();

        // an old world file put back by hand
        let mut other = WorldData {
            name: WORLD.into(),
            time: 99,
            ..default()
        };
        other.map.insert(IVec3::ZERO, shared::world::Chunk::new());
        let second_text = dense_single_file(&other);
        fs::write(dir.join("world.ron"), &second_text).unwrap();
        load_world_data(WORLD, &paths).unwrap();

        assert_eq!(
            fs::read_to_string(dir.join("world.ron.legacy")).unwrap(),
            first_text
        );
        assert_eq!(
            fs::read_to_string(dir.join("world.ron.legacy.1")).unwrap(),
            second_text
        );
    }

    #[test]
    fn chunk_files_without_a_world_file_are_an_error_not_a_new_world() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = paths(&tmp);
        let chunks_dir = world_dir_of(&tmp).join(CHUNKS_DIR);
        fs::create_dir_all(&chunks_dir).unwrap();
        write_chunk(&chunks_dir, IVec3::ZERO, &shared::world::Chunk::new()).unwrap();

        let err = load_world_data(WORLD, &paths).err().unwrap().to_string();

        assert!(err.contains("world.ron"), "{err}");
        assert!(!world_dir_of(&tmp).join("world.ron").exists());
    }

    #[test]
    fn an_empty_or_absent_chunk_folder_without_a_world_file_is_a_new_world() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = paths(&tmp);
        assert!(load_world_data(WORLD, &paths).unwrap().map.is_empty());

        fs::create_dir_all(world_dir_of(&tmp).join(CHUNKS_DIR)).unwrap();
        assert!(load_world_data(WORLD, &paths).unwrap().map.is_empty());
    }

    #[test]
    fn a_world_name_that_leaves_the_saves_folder_is_refused() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = paths(&tmp);
        assert!(load_world_data("../world", &paths).is_err());
        assert!(load_world_data("", &paths).is_err());
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
            ..default()
        };
        world.map.insert(IVec3::new(2, 0, -1), chunk.clone());
        let config = ron::ser::PrettyConfig::new()
            .with_depth_limit(3)
            .with_separate_tuple_members(true)
            .with_enumerate_arrays(true);
        let text = ron::ser::to_string_pretty(&world, config).unwrap();
        assert!(text.lines().count() > 4096);

        let loaded = parse_single_file_world(&text).unwrap();

        assert_eq!(loaded.map[&IVec3::new(2, 0, -1)], chunk);
    }
}
