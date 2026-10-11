use crate::init::ServerTime;
use crate::world::format::{write_atomically, WorldMeta};
use crate::world::writer::{SaveJob, WorldWriter};
use bevy::prelude::*;
use ron::ser::PrettyConfig;
use shared::messages::PlayerId;
use shared::players::Player;
use shared::world::WorldSeed;
use shared::{GameFolderPaths, TICKS_PER_SECOND};
use std::path::Path;

#[derive(Message)]
pub enum SaveRequestEvent {
    World,
    Player(PlayerId),
}

use crate::world::data::{ServerWorldMap, SAVE_PATH};

/// Game ticks between two autosaves: 60 seconds.
pub const AUTOSAVE_INTERVAL_TICKS: u64 = 60 * TICKS_PER_SECOND;

/// When the last save was queued, to space the autosaves.
#[derive(Resource)]
pub struct Autosave {
    pub last_save_tick: u64,
}

/// Reacts to the save requests, and to the autosave coming due, by handing the metadata and
/// the dirty chunks to the writer thread. The player files are written here, they are small.
///
/// Runs after the broadcast, which is what feeds `dirty_chunks` from the change log.
pub fn save_world_system(
    mut world_map: ResMut<ServerWorldMap>,
    world_seed: Res<WorldSeed>,
    game_folder_path: Res<GameFolderPaths>,
    time: Res<ServerTime>,
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
            if let Some(player) = world_map.players.get(id) {
                save_player(&world_map.name, player, &game_folder_path);
            }
        }
    }

    let autosave_due = time.0.saturating_sub(autosave.last_save_tick) >= AUTOSAVE_INTERVAL_TICKS;
    if save_requested || autosave_due {
        autosave.last_save_tick = time.0;
        queue_save(&mut world_map, *world_seed, time.0, &mut writer);
    }
}

/// Hands the metadata and a copy of every dirty chunk to the writer. A chunk is dirty until it
/// is handed over, so a second save queued while the first is being written does not copy it
/// again unless it changed.
fn queue_save(
    world_map: &mut ServerWorldMap,
    seed: WorldSeed,
    tick: u64,
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
        .filter_map(|pos| Some((pos, world_map.chunks.chunk(pos)?.clone())))
        .collect();
    let job = SaveJob {
        meta: WorldMeta::new(world_map.name.clone(), seed, tick, world_map.mobs.clone()),
        chunks,
    };

    if let Err(job) = writer.submit(job) {
        error!(
            "[{}] The world writer is gone, nothing was saved",
            world_map.name
        );
        world_map
            .dirty_chunks
            .extend(job.chunks.into_iter().map(|(pos, _)| pos));
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

#[cfg(test)]
mod tests {
    use super::*;

    mod saving {
        use super::*;
        use crate::world::chunk_files::{read_chunks, CHUNKS_DIR};
        use shared::world::{BlockData, BlockDirection, BlockId, ChunkMap};
        use std::sync::{Arc, Mutex};
        use std::time::{Duration, Instant};

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
                ..Default::default()
            }
        }

        fn app(map: ServerWorldMap, writer: WorldWriter, tick: u64) -> App {
            let mut app = App::new();
            app.add_message::<SaveRequestEvent>()
                .insert_resource(map)
                .insert_resource(WorldSeed(1))
                .insert_resource(ServerTime(tick))
                .insert_resource(GameFolderPaths {
                    game_folder_path: "unused".into(),
                    assets_folder_path: "unused".into(),
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

            queue_save(&mut map, WorldSeed(1), 1, &mut writer);
            queue_save(&mut map, WorldSeed(1), 2, &mut writer);
            map.dirty_chunks.insert(IVec3::X);
            queue_save(&mut map, WorldSeed(1), 3, &mut writer);
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
            let mut app = app(map, WorldWriter::spawn(tmp.path().into()).unwrap(), 10);
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
                ("w", 10, 1)
            );
        }

        #[test]
        fn nothing_is_written_between_requests_until_the_autosave_is_due() {
            let tmp = tempfile::tempdir().unwrap();
            let mut map = world_map(1);
            map.dirty_chunks.insert(IVec3::ZERO);
            let writer = WorldWriter::spawn(tmp.path().into()).unwrap();
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
        fn a_chunk_that_failed_to_write_is_written_by_the_next_save() {
            let tmp = tempfile::tempdir().unwrap();
            let chunks_dir = tmp.path().join(CHUNKS_DIR);
            std::fs::create_dir_all(&chunks_dir).unwrap();
            let squatter = chunks_dir.join("0_0_0.bin.tmp");
            std::fs::create_dir(&squatter).unwrap();
            let mut map = world_map(1);
            map.dirty_chunks.insert(IVec3::ZERO);
            let mut app = app(map, WorldWriter::spawn(tmp.path().into()).unwrap(), 10);
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

            queue_save(&mut map, WorldSeed(1), 1, &mut writer);

            assert!(map.dirty_chunks.contains(&IVec3::ZERO));
        }
    }
}
