use crate::init::ServerTime;
use crate::world::container_files::{ChunkContainers, ChunkContents};
use crate::world::format::{SavedWeather, WorldMeta};
use crate::world::generation::WorldGeneration;
use crate::world::weather::ServerWeather;
use crate::world::writer::{SaveJob, WorldWriter};
use bevy::prelude::*;
use ron::ser::PrettyConfig;
use shared::messages::PlayerId;
use shared::players::Player;
use shared::world::{to_local_pos, WorldGenSettings, WorldSeed};
use shared::TICKS_PER_SECOND;
#[cfg(test)]
use std::path::Path;

#[derive(Message)]
pub enum SaveRequestEvent {
    World,
    Player(PlayerId),
}

use crate::world::data::ServerWorldMap;

/// Game ticks between two autosaves: 60 seconds.
pub const AUTOSAVE_INTERVAL_TICKS: u64 = 60 * TICKS_PER_SECOND;

/// When the last save was queued, to space the autosaves.
#[derive(Resource)]
pub struct Autosave {
    pub last_save_tick: u64,
}

/// Reacts to the save requests, and to the autosave coming due, by handing the metadata, the
/// files of the players to save and the dirty chunks to the writer thread.
///
/// Runs after the broadcast, which is what feeds `dirty_chunks` from the change log.
pub fn save_world_system(
    mut world_map: ResMut<ServerWorldMap>,
    world_seed: Res<WorldSeed>,
    generation: Option<Res<WorldGeneration>>,
    time: Res<ServerTime>,
    weather: Res<ServerWeather>,
    mut writer: ResMut<WorldWriter>,
    mut autosave: ResMut<Autosave>,
    mut event: MessageReader<SaveRequestEvent>,
) {
    // A chunk the writer could not write is dirty again, so that this save retries it
    for chunk_pos in writer.take_failed_chunks() {
        world_map.dirty_chunks.insert(chunk_pos);
    }

    // Reads all events to prevent them from being queued forever and repeatedly request a save
    let mut save_requested = false;
    for ev in event.read() {
        save_requested = true;

        if let SaveRequestEvent::Player(id) = ev {
            let world_map = world_map.as_mut();
            if let Some(player) = world_map.players.get(id) {
                queue_player_save(&mut world_map.players_to_save, &world_map.name, player);
            }
        }
    }

    let autosave_due = time.0.saturating_sub(autosave.last_save_tick) >= AUTOSAVE_INTERVAL_TICKS;
    if autosave_due {
        // A crash must not revert the connected players to their state at join
        let world_map = world_map.as_mut();
        for player in world_map.players.values() {
            queue_player_save(&mut world_map.players_to_save, &world_map.name, player);
        }
    }
    if save_requested || autosave_due {
        autosave.last_save_tick = time.0;
        let settings = generation.map(|g| g.settings.clone()).unwrap_or_default();
        queue_save(
            &mut world_map,
            (*world_seed, settings),
            time.0,
            weather.saved(),
            &mut writer,
        );
    }
}

/// Hands the metadata, with the seed and the generation settings of the world and the
/// `weather`, the files of the players to save and a copy of every dirty chunk, with its chests
/// and furnaces, to the writer. A chunk is dirty until it
/// is handed over, so a second save queued while the first is being written does not copy it
/// again unless it changed.
fn queue_save(
    world_map: &mut ServerWorldMap,
    (seed, generation): (WorldSeed, WorldGenSettings),
    tick: u64,
    weather: SavedWeather,
    writer: &mut WorldWriter,
) {
    let dirty: Vec<IVec3> = world_map.dirty_chunks.drain().collect();
    if dirty.is_empty() {
        debug!(
            "[{}] No chunk to save, writing the metadata only",
            world_map.name
        );
    }
    let chunks = dirty
        .into_iter()
        .filter_map(|pos| {
            let chunk = world_map.chunks.chunk(pos)?.clone();
            let contents = ChunkContents {
                chests: by_local_pos(world_map.chunks.containers_of_chunk(pos)),
                furnaces: by_local_pos(world_map.chunks.furnaces_of_chunk(pos)),
            };
            Some((pos, chunk, contents))
        })
        .collect();
    let job = SaveJob {
        meta: WorldMeta {
            dropped_items: world_map.dropped_items.to_saved(),
            weather: Some(weather),
            generation,
            ..WorldMeta::new(world_map.name.clone(), seed, tick, world_map.mobs.clone())
        },
        players: std::mem::take(&mut world_map.players_to_save),
        chunks,
    };

    if let Err(job) = writer.submit(job) {
        error!(
            "[{}] The world writer is gone, nothing was saved",
            world_map.name
        );
        world_map
            .dirty_chunks
            .extend(job.chunks.into_iter().map(|(pos, ..)| pos));
    }
}

/// What the blocks of one kind of a chunk hold, given by global position, by local position
/// and sorted.
fn by_local_pos<'a, T: Clone + 'a>(
    held: impl Iterator<Item = (IVec3, &'a T)>,
) -> ChunkContainers<T> {
    let mut held: ChunkContainers<T> = held
        .map(|(global, value)| (to_local_pos(&global), value.clone()))
        .collect();
    held.sort_by_key(|(local, _)| (local.x, local.y, local.z));
    held
}

/// Puts the file of `player`, of the world `world_name`, among `players_to_save`: the next save
/// hands it to the writer, which syncs it with the chunks. Syncing it alone took the tick
/// several milliseconds.
pub fn queue_player_save(
    players_to_save: &mut Vec<(PlayerId, String)>,
    world_name: &str,
    player: &Player,
) {
    match player_file(player) {
        Ok(contents) => players_to_save.push((player.id, contents)),
        Err(err) => error!(
            "[{}] Could not save data for player {} : {}",
            world_name, player.id, err
        ),
    }
}

/// What the file of `player` holds.
fn player_file(player: &Player) -> Result<String, ron::Error> {
    // RON Serialization config
    let pretty_config = PrettyConfig::new()
        .with_depth_limit(3)
        .with_separate_tuple_members(true)
        .with_enumerate_arrays(true);

    // Serialize Complete player data
    ron::ser::to_string_pretty(player, pretty_config)
}

#[cfg(test)]
pub fn save_player_data(
    player: &Player,
    file_path: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    crate::world::format::write_atomically(file_path, player_file(player)?.as_bytes())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    mod saving {
        use super::*;
        use crate::world::chunk_files::{read_chunks, CHUNKS_DIR};
        use crate::world::format::FORMAT_VERSION;
        use shared::world::{BlockData, BlockDirection, BlockId, ChunkMap, WeatherKind};
        use std::sync::{Arc, Mutex};
        use std::time::{Duration, Instant};

        const CLEAR: SavedWeather = SavedWeather {
            kind: WeatherKind::Clear,
            ticks_left: 1,
        };

        fn stone() -> BlockData {
            BlockData::new(BlockId::Stone, BlockDirection::Front)
        }

        fn world_map(chunks: i32) -> ServerWorldMap {
            ServerWorldMap {
                name: "w".into(),
                chunks: ChunkMap::with_loaded_region(
                    IVec3::ZERO,
                    IVec3::new(chunks * 16 - 1, 15, 15),
                ),
                ..default()
            }
        }

        fn app(map: ServerWorldMap, writer: WorldWriter, tick: u64) -> App {
            let mut app = App::new();
            app.add_message::<SaveRequestEvent>()
                .insert_resource(map)
                .insert_resource(WorldSeed(1))
                .insert_resource(ServerTime(tick))
                .insert_resource(ServerWeather {
                    kind: WeatherKind::Rain,
                    ticks_left: 777,
                    strike_in: 1,
                })
                .insert_resource(writer)
                .insert_resource(Autosave { last_save_tick: 0 })
                .add_systems(Update, save_world_system);
            app
        }

        /// Stops the writer, which writes everything queued first.
        fn flush(app: &mut App) {
            drop(app.world_mut().remove_resource::<WorldWriter>());
        }

        #[test]
        fn only_the_dirty_chunks_are_copied_and_a_second_save_does_not_copy_them_again() {
            let jobs = Arc::new(Mutex::new(Vec::new()));
            let sink = jobs.clone();
            let mut writer = WorldWriter::spawn_with(move |job| {
                sink.lock().unwrap().push(job.chunks.len());
                Vec::new()
            })
            .unwrap();
            let mut map = world_map(3);
            map.dirty_chunks.extend([IVec3::ZERO, IVec3::X]);

            queue_save(
                &mut map,
                (WorldSeed(1), Default::default()),
                1,
                CLEAR,
                &mut writer,
            );
            queue_save(
                &mut map,
                (WorldSeed(1), Default::default()),
                2,
                CLEAR,
                &mut writer,
            );
            map.dirty_chunks.insert(IVec3::X);
            queue_save(
                &mut map,
                (WorldSeed(1), Default::default()),
                3,
                CLEAR,
                &mut writer,
            );
            drop(writer);

            assert_eq!(*jobs.lock().unwrap(), vec![2, 0, 1]);
            assert!(map.dirty_chunks.is_empty());
        }

        #[test]
        fn a_save_request_writes_the_dirty_chunks_and_the_metadata() {
            let tmp = tempfile::tempdir().unwrap();
            let mut map = world_map(2);
            map.chunks.set_block(IVec3::new(1, 2, 3), stone());
            map.dirty_chunks.insert(IVec3::ZERO);
            let mut app = app(
                map,
                WorldWriter::spawn(tmp.path().into(), None).unwrap(),
                10,
            );
            app.world_mut().write_message(SaveRequestEvent::World);

            app.update();
            flush(&mut app);

            let chunks = read_chunks(&tmp.path().join(CHUNKS_DIR)).unwrap();
            assert_eq!(chunks.len(), 1);
            assert_eq!(
                chunks[&IVec3::ZERO].get(IVec3::new(1, 2, 3)),
                Some(&stone())
            );
            let meta: WorldMeta =
                ron::de::from_str(&std::fs::read_to_string(tmp.path().join("world.ron")).unwrap())
                    .unwrap();
            assert_eq!(
                (meta.name.as_str(), meta.time, meta.format_version),
                ("w", 10, FORMAT_VERSION)
            );
            let weather = meta.weather.unwrap();
            assert_eq!((weather.kind, weather.ticks_left), (WeatherKind::Rain, 777));
        }

        #[test]
        fn a_save_writes_the_contents_of_the_chests_of_the_dirty_chunks() {
            use crate::world::container_files::{read_containers, CONTAINERS_DIR};
            use shared::world::Container;

            let tmp = tempfile::tempdir().unwrap();
            let mut map = world_map(2);
            let chest = IVec3::new(17, 2, 3);
            map.chunks
                .set_block(chest, BlockData::new(BlockId::Chest, BlockDirection::Front));
            map.chunks.insert_container(chest, Container::default());
            map.dirty_chunks.insert(IVec3::X);
            let mut app = app(
                map,
                WorldWriter::spawn(tmp.path().into(), None).unwrap(),
                10,
            );
            app.world_mut().write_message(SaveRequestEvent::World);

            app.update();
            flush(&mut app);

            let containers =
                read_containers::<Container>(&tmp.path().join(CONTAINERS_DIR)).unwrap();
            assert_eq!(
                containers,
                std::collections::HashMap::from([(chest, Container::default())])
            );
        }

        #[test]
        fn a_save_writes_the_furnaces_of_the_dirty_chunks() {
            use crate::world::container_files::{read_containers, FURNACES_DIR};
            use shared::world::Furnace;

            let tmp = tempfile::tempdir().unwrap();
            let mut map = world_map(2);
            let furnace = IVec3::new(17, 2, 3);
            map.chunks.set_block(
                furnace,
                BlockData::new(BlockId::LitFurnace, BlockDirection::Front),
            );
            let burning = Furnace {
                burn_left: 9,
                burn_total: 100,
                ..Default::default()
            };
            map.chunks.insert_furnace(furnace, burning.clone());
            map.dirty_chunks.insert(IVec3::X);
            let mut app = app(
                map,
                WorldWriter::spawn(tmp.path().into(), None).unwrap(),
                10,
            );
            app.world_mut().write_message(SaveRequestEvent::World);

            app.update();
            flush(&mut app);

            let furnaces = read_containers::<Furnace>(&tmp.path().join(FURNACES_DIR)).unwrap();
            assert_eq!(
                furnaces,
                std::collections::HashMap::from([(furnace, burning)])
            );
        }

        #[test]
        fn nothing_is_written_between_requests_until_the_autosave_is_due() {
            let tmp = tempfile::tempdir().unwrap();
            let mut map = world_map(1);
            map.dirty_chunks.insert(IVec3::ZERO);
            let writer = WorldWriter::spawn(tmp.path().into(), None).unwrap();
            let mut app = app(map, writer, 100 + AUTOSAVE_INTERVAL_TICKS - 1);
            app.world_mut().resource_mut::<Autosave>().last_save_tick = 100;

            app.update();
            assert!(app
                .world()
                .resource::<ServerWorldMap>()
                .dirty_chunks
                .contains(&IVec3::ZERO));

            app.world_mut().resource_mut::<ServerTime>().0 += 1;
            app.update();
            flush(&mut app);

            assert!(app
                .world()
                .resource::<ServerWorldMap>()
                .dirty_chunks
                .is_empty());
            assert_eq!(read_chunks(&tmp.path().join(CHUNKS_DIR)).unwrap().len(), 1);
        }

        #[test]
        fn the_autosave_writes_the_connected_players_with_their_current_state() {
            let tmp = tempfile::tempdir().unwrap();
            let players_dir = tmp.path().join("players");
            let mut map = world_map(1);
            let mut player = Player::new(7, "Zed".into(), Vec3::ZERO, Transform::default());
            player.position = Vec3::new(12.0, 34.0, 56.0);
            map.players.insert(7, player);
            let writer = WorldWriter::spawn(tmp.path().into(), None).unwrap();
            let mut app = app(map, writer, AUTOSAVE_INTERVAL_TICKS - 1);

            app.update();
            assert!(
                !players_dir.join("7.ron").exists(),
                "autosave is not due yet"
            );

            app.world_mut().resource_mut::<ServerTime>().0 += 1;
            app.update();
            flush(&mut app);

            let saved: Player =
                ron::de::from_str(&std::fs::read_to_string(players_dir.join("7.ron")).unwrap())
                    .unwrap();
            assert_eq!(saved.position, Vec3::new(12.0, 34.0, 56.0));
        }

        #[test]
        fn a_chunk_that_failed_to_write_is_written_by_the_next_save() {
            let tmp = tempfile::tempdir().unwrap();
            let chunks_dir = tmp.path().join(CHUNKS_DIR);
            std::fs::create_dir_all(&chunks_dir).unwrap();
            let squatter = chunks_dir.join("0_0_0.region.tmp");
            std::fs::create_dir(&squatter).unwrap();
            let mut map = world_map(1);
            map.dirty_chunks.insert(IVec3::ZERO);
            let mut app = app(
                map,
                WorldWriter::spawn(tmp.path().into(), None).unwrap(),
                10,
            );
            app.world_mut().write_message(SaveRequestEvent::World);
            app.update();

            // the writer reports the failure, the next update makes the chunk dirty again
            let start = Instant::now();
            while !app
                .world()
                .resource::<ServerWorldMap>()
                .dirty_chunks
                .contains(&IVec3::ZERO)
            {
                assert!(
                    start.elapsed() < Duration::from_secs(10),
                    "no failure reported"
                );
                std::thread::sleep(Duration::from_millis(5));
                app.update();
            }
            std::fs::remove_dir(&squatter).unwrap();
            app.world_mut().write_message(SaveRequestEvent::World);
            app.update();
            flush(&mut app);

            assert_eq!(read_chunks(&chunks_dir).unwrap().len(), 1);
        }

        #[test]
        fn a_save_with_a_dead_writer_keeps_the_chunks_dirty() {
            let writer = WorldWriter::spawn_with(|_| Vec::new()).unwrap();
            let mut map = world_map(1);
            map.dirty_chunks.insert(IVec3::ZERO);
            // The thread ends once the sender is gone; a job queued after is given back
            let mut writer = writer;
            writer.close_for_test();

            queue_save(
                &mut map,
                (WorldSeed(1), Default::default()),
                1,
                CLEAR,
                &mut writer,
            );

            assert!(map.dirty_chunks.contains(&IVec3::ZERO));
        }
    }
}
