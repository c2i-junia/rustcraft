//! The thread that writes the chunks, the metadata and the players of a world to disk, so
//! that saving them never makes the tick wait for the disk. The tick thread hands it snapshots through a channel and the thread writes them in
//! order. Dropping the writer waits for everything queued to be written.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread::JoinHandle;

use bevy::{
    ecs::resource::Resource,
    log::{debug, error, info},
    math::IVec3,
};
use shared::messages::PlayerId;
use shared::world::Chunk;
use shared::SaveCounter;

use super::chunk_files::{
    chunk_file_name, region_file_name, region_of, remove_interrupted_writes, write_region_pending,
    CHUNKS_DIR,
};
use super::container_files::{
    remove_containers, write_containers_pending, ChunkContainers, ChunkContents, Stored,
    CONTAINERS_DIR, FURNACES_DIR,
};
use super::format::{
    commit, write_meta_pending, write_pending, PendingFile, WorldMeta, WORLD_FILE,
};

/// The folder of a world that holds a file per player.
const PLAYERS_DIR: &str = "players";

/// What one save writes: the metadata of the world, the file of each player to write, and a
/// copy of each chunk to write, with what its chests and furnaces hold.
pub struct SaveJob {
    pub meta: WorldMeta,
    pub players: Vec<(PlayerId, String)>,
    pub chunks: Vec<(IVec3, Chunk, ChunkContents)>,
}

/// What writing one job did.
pub struct JobOutcome {
    pub meta_written: bool,
    pub chunks_dir_ok: bool,
    /// The chunks that could not be written: all of them without a chunks folder.
    pub failed_chunks: Vec<IVec3>,
}

impl JobOutcome {
    /// Whether everything of the job is on disk.
    pub fn is_complete(&self) -> bool {
        self.meta_written && self.chunks_dir_ok && self.failed_chunks.is_empty()
    }
}

/// How many threads encode and write the files of the regions of a job.
const WRITER_THREADS: usize = 8;

/// The files of the chunks of a region written to be committed: the region's, and those of
/// the chests and the furnaces of its chunks. A chunk left without chests, or furnaces, has
/// that file removed once it is committed: the folder of the kind and the chunk.
struct RegionFiles {
    files: Vec<(Owner, PendingFile)>,
    without_containers: Vec<(&'static str, IVec3)>,
}

/// Whose a file of a job is.
#[derive(Clone, PartialEq)]
enum Owner {
    Meta,
    Player,
    /// The chests or the furnaces of a chunk.
    Chunk(IVec3),
    Region(Vec<IVec3>),
}

/// Writes `job` into the folder `world_dir`. Every file is written aside, then all are synced
/// at once and put in place, the metadata first, so that the seed of a new world is on disk
/// before any of its chunks, then the players. Every failure is logged with its path.
pub fn write_job(world_dir: &std::path::Path, job: SaveJob) -> JobOutcome {
    let mut head = Vec::new();
    match write_meta_pending(world_dir, &job.meta) {
        Ok(meta) => head.push((Owner::Meta, meta)),
        Err(err) => error!(
            "Failed to write {}: {err}",
            world_dir.join(WORLD_FILE).display()
        ),
    }
    let players_dir = world_dir.join(PLAYERS_DIR);
    for (id, contents) in &job.players {
        let path = players_dir.join(format!("{id}.ron"));
        match std::fs::create_dir_all(&players_dir)
            .and_then(|()| write_pending(&path, contents.as_bytes()))
        {
            Ok(player) => head.push((Owner::Player, player)),
            Err(err) => error!("Failed to write {}: {err}", path.display()),
        }
    }

    let chunks_dir = world_dir.join(CHUNKS_DIR);
    if let Err(err) = std::fs::create_dir_all(&chunks_dir) {
        error!("Failed to create {}: {err}", chunks_dir.display());
        return JobOutcome {
            meta_written: commit_owned(world_dir, head).0,
            chunks_dir_ok: false,
            failed_chunks: job.chunks.into_iter().map(|(pos, ..)| pos).collect(),
        };
    }

    let total = job.chunks.len();
    let mut by_region: HashMap<IVec3, Vec<&(IVec3, Chunk, ChunkContents)>> = HashMap::new();
    for chunk in &job.chunks {
        by_region.entry(region_of(chunk.0)).or_default().push(chunk);
    }
    let regions: Vec<_> = by_region.into_iter().collect();
    let mut failed = Vec::new();
    let mut written = Vec::new();
    let per_thread = regions.len().div_ceil(WRITER_THREADS).max(1);
    std::thread::scope(|scope| {
        let threads: Vec<_> = regions
            .chunks(per_thread)
            .map(|part| {
                let chunks_dir = &chunks_dir;
                scope.spawn(move || {
                    part.iter()
                        .map(|(region, chunks)| {
                            write_region_files(world_dir, chunks_dir, *region, chunks)
                        })
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        for thread in threads {
            for (files, region_failed) in
                thread.join().expect("writing the files of chunks panicked")
            {
                written.extend(files);
                failed.extend(region_failed);
            }
        }
    });

    for region in &mut written {
        head.append(&mut region.files);
    }
    let (meta_written, failed_chunks) = commit_owned(world_dir, head);
    failed.extend(failed_chunks);
    // Once the chunk without a chest is in place: a crash before leaves the chest it had,
    // still with its contents
    for region in written {
        for (dir, chunk_pos) in region.without_containers {
            if failed.contains(&chunk_pos) {
                continue;
            }
            let containers_dir = world_dir.join(dir);
            if let Err(err) = remove_containers(&containers_dir, chunk_pos) {
                error!(
                    "Failed to write {}: {err}",
                    containers_dir.join(chunk_file_name(chunk_pos)).display()
                );
                failed.push(chunk_pos);
            }
        }
    }
    failed.sort_by_key(|pos| (pos.x, pos.y, pos.z));
    failed.dedup();
    info!(
        "World [{}] saved: {} of {total} chunks written",
        job.meta.name,
        total - failed.len()
    );
    JobOutcome {
        meta_written,
        chunks_dir_ok: true,
        failed_chunks: failed,
    }
}

/// Commits `files` in order, logging each failure with its path. Whether the metadata is in
/// place, and the chunks of the files that are not.
fn commit_owned(
    world_dir: &std::path::Path,
    files: Vec<(Owner, PendingFile)>,
) -> (bool, Vec<IVec3>) {
    let (owners, pending): (Vec<_>, Vec<_>) = files.into_iter().unzip();
    let paths: Vec<_> = pending.iter().map(|file| file.path().to_owned()).collect();
    let mut meta_written = false;
    let mut failed = Vec::new();
    for ((owner, path), result) in owners
        .into_iter()
        .zip(paths)
        .zip(commit(world_dir, pending))
    {
        match (owner, result) {
            (Owner::Meta, Ok(())) => meta_written = true,
            (owner, Err(err)) => {
                error!("Failed to write {}: {err}", path.display());
                match owner {
                    Owner::Chunk(chunk_pos) => failed.push(chunk_pos),
                    Owner::Region(chunks) => failed.extend(chunks),
                    Owner::Meta | Owner::Player => {}
                }
            }
            (_, Ok(())) => {}
        }
    }
    (meta_written, failed)
}

/// Writes the files of the chunks of `region` aside, in the folder of the world `world_dir`:
/// the file of the region, and those of the chests and the furnaces of each chunk that has
/// some. A chunk whose chests or furnaces could not be written is left out of the region,
/// which keeps what it held of it. The files, and the chunks not written, each failure being
/// logged.
fn write_region_files(
    world_dir: &std::path::Path,
    chunks_dir: &std::path::Path,
    region: IVec3,
    chunks: &[&(IVec3, Chunk, ChunkContents)],
) -> (Option<RegionFiles>, Vec<IVec3>) {
    let mut files = Vec::new();
    let mut failed = Vec::new();
    let mut without_containers = Vec::new();
    let mut in_region = Vec::new();
    for (chunk_pos, chunk, contents) in chunks.iter().copied() {
        let written = write_held(world_dir, *chunk_pos, &contents.chests, &mut files)
            .and_then(|()| write_held(world_dir, *chunk_pos, &contents.furnaces, &mut files));
        if let Err(err) = written {
            error!("Failed to write {err}");
            failed.push(*chunk_pos);
            continue;
        }
        if contents.chests.is_empty() {
            without_containers.push((CONTAINERS_DIR, *chunk_pos));
        }
        if contents.furnaces.is_empty() {
            without_containers.push((FURNACES_DIR, *chunk_pos));
        }
        in_region.push((*chunk_pos, chunk));
    }
    let positions: Vec<IVec3> = in_region.iter().map(|(pos, _)| *pos).collect();
    match write_region_pending(chunks_dir, region, in_region) {
        Ok(file) => files.insert(0, (Owner::Region(positions), file)),
        Err(err) => {
            error!(
                "Failed to write {}: {err}",
                chunks_dir.join(region_file_name(region)).display()
            );
            // The chests and the furnaces are not put in place without their chunks
            failed.extend(positions);
            return (None, failed);
        }
    }
    (
        Some(RegionFiles {
            files,
            without_containers,
        }),
        failed,
    )
}

/// Writes aside, among `files`, the file of the chunk at `chunk_pos` that holds `held`, in its
/// folder of the world `world_dir`, when it holds some. The error names the file.
fn write_held<T: Stored>(
    world_dir: &std::path::Path,
    chunk_pos: IVec3,
    held: &ChunkContainers<T>,
    files: &mut Vec<(Owner, PendingFile)>,
) -> Result<(), String> {
    if held.is_empty() {
        return Ok(());
    }
    // The folder only exists in a world that has had such a block
    let dir = world_dir.join(T::DIR);
    let file = std::fs::create_dir_all(&dir)
        .and_then(|()| write_containers_pending(&dir, chunk_pos, held))
        .map_err(|err| format!("{}: {err}", dir.join(chunk_file_name(chunk_pos)).display()))?;
    files.push((Owner::Chunk(chunk_pos), file));
    Ok(())
}

#[derive(Resource)]
pub struct WorldWriter {
    jobs: Option<Sender<SaveJob>>,
    /// The chunks each finished job could not write.
    failures: Mutex<Receiver<Vec<IVec3>>>,
    /// The jobs queued and not written yet, with what they could not write reported.
    unfinished: Arc<AtomicUsize>,
    thread: Option<JoinHandle<()>>,
}

impl WorldWriter {
    /// Starts a writer that saves into `world_dir`, and counts in `saves` each job written in
    /// full and each one that was not.
    pub fn spawn(world_dir: PathBuf, saves: Option<SaveCounter>) -> std::io::Result<Self> {
        let mut cleaned = false;
        Self::spawn_with(move |job| {
            // The chunks are read when needed, not listed on load: the files of interrupted
            // writes are removed here, before the first save
            if !cleaned {
                remove_interrupted_writes(&world_dir.join(CHUNKS_DIR));
                remove_interrupted_writes(&world_dir.join(CONTAINERS_DIR));
                remove_interrupted_writes(&world_dir.join(FURNACES_DIR));
                cleaned = true;
            }
            let outcome = write_job(&world_dir, job);
            match (&saves, outcome.is_complete()) {
                (Some(saves), true) => saves.bump(),
                (Some(saves), false) => saves.bump_failure(),
                (None, _) => {}
            }
            outcome.failed_chunks
        })
    }

    /// Starts a writer that handles each job with `handle`, which returns the chunks it failed
    /// to write.
    pub(super) fn spawn_with(
        mut handle: impl FnMut(SaveJob) -> Vec<IVec3> + Send + 'static,
    ) -> std::io::Result<Self> {
        let (jobs, queue) = mpsc::channel::<SaveJob>();
        let (report, failures) = mpsc::channel();
        let unfinished = Arc::new(AtomicUsize::new(0));
        let finished = Arc::clone(&unfinished);
        let thread = std::thread::Builder::new()
            .name("world-writer".into())
            .spawn(move || {
                // Ends once the sender is dropped and the queue is empty
                for job in queue {
                    // The tick thread may be gone already
                    let _ = report.send(handle(job));
                    finished.fetch_sub(1, Ordering::Release);
                }
            })?;
        Ok(Self {
            jobs: Some(jobs),
            failures: Mutex::new(failures),
            unfinished,
            thread: Some(thread),
        })
    }

    /// Queues `job`. Gives it back when the writer thread is gone, so that nothing is lost
    /// silently.
    pub fn submit(&self, job: SaveJob) -> Result<(), Box<SaveJob>> {
        let Some(jobs) = &self.jobs else {
            return Err(Box::new(job));
        };
        self.unfinished.fetch_add(1, Ordering::Acquire);
        jobs.send(job).map_err(|err| {
            self.unfinished.fetch_sub(1, Ordering::Release);
            Box::new(err.0)
        })
    }

    /// Whether every job queued is written, and what it could not write reported to
    /// `take_failed_chunks`: a chunk may be read back from its file then.
    pub fn is_idle(&self) -> bool {
        self.unfinished.load(Ordering::Acquire) == 0
    }

    /// Stops the thread as a dead one would be, waiting for it.
    #[cfg(test)]
    pub(super) fn close_for_test(&mut self) {
        drop(self.jobs.take());
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }

    /// The chunks that the jobs finished since the last call could not write.
    pub fn take_failed_chunks(&self) -> Vec<IVec3> {
        let failures = self.failures.lock().unwrap_or_else(PoisonError::into_inner);
        failures.try_iter().flatten().collect()
    }
}

impl Drop for WorldWriter {
    /// Waits for the queued jobs to be written. Returns at once when the thread is dead.
    fn drop(&mut self) {
        drop(self.jobs.take());
        if let Some(thread) = self.thread.take() {
            if thread.join().is_err() {
                error!("The world writer thread panicked: the last save may be incomplete");
            }
        }
        let failed = self.take_failed_chunks();
        if failed.is_empty() {
            debug!("World writer stopped");
        } else {
            error!(
                "{} chunks could not be written before the server stopped",
                failed.len()
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::chunk_files::{read_chunks, REGION_SIZE};
    use shared::world::{BlockData, BlockDirection, BlockId};
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant};

    fn meta(time: u64) -> WorldMeta {
        WorldMeta::new(
            "w".into(),
            shared::world::WorldSeed(5),
            time,
            Default::default(),
        )
    }

    fn stone_chunk() -> Chunk {
        let mut chunk = Chunk::new();
        chunk.set(
            IVec3::ONE,
            BlockData::new(BlockId::Stone, BlockDirection::Front),
        );
        chunk
    }

    fn job(positions: &[IVec3]) -> SaveJob {
        SaveJob {
            meta: meta(1),
            players: Vec::new(),
            chunks: positions
                .iter()
                .map(|&p| (p, stone_chunk(), ChunkContents::default()))
                .collect(),
        }
    }

    #[test]
    fn the_writer_is_idle_once_every_job_queued_is_written() {
        let (release, wait) = mpsc::channel::<()>();
        let wait = Mutex::new(wait);
        let writer = WorldWriter::spawn_with(move |_| {
            wait.lock().unwrap().recv().unwrap();
            vec![IVec3::X]
        })
        .unwrap();
        assert!(writer.is_idle());

        assert!(writer.submit(job(&[IVec3::X])).is_ok());
        assert!(writer.submit(job(&[IVec3::X])).is_ok());
        assert!(!writer.is_idle());
        release.send(()).unwrap();
        release.send(()).unwrap();
        while !writer.is_idle() {
            std::thread::yield_now();
        }
        // What the jobs could not write is reported by then
        assert_eq!(writer.take_failed_chunks(), vec![IVec3::X, IVec3::X]);
    }

    #[test]
    fn dropping_the_writer_waits_for_every_queued_job() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().to_path_buf();
        let writer = WorldWriter::spawn(dir.clone(), None).unwrap();
        for x in 0..20 {
            assert!(writer.submit(job(&[IVec3::new(x, 0, 0)])).is_ok());
        }

        drop(writer);

        assert_eq!(read_chunks(&dir.join(CHUNKS_DIR)).unwrap().len(), 20);
        assert!(dir.join("world.ron").is_file());
    }

    #[test]
    fn the_chests_of_a_chunk_are_written_with_it_and_removed_once_gone() {
        use crate::world::container_files::read_containers;
        use shared::world::Container;

        let tmp = tempfile::tempdir().unwrap();
        let containers_dir = tmp.path().join(CONTAINERS_DIR);
        let with_chest = SaveJob {
            meta: meta(1),
            players: Vec::new(),
            chunks: vec![(
                IVec3::X,
                stone_chunk(),
                ChunkContents {
                    chests: vec![(IVec3::ONE, Container::default())],
                    ..Default::default()
                },
            )],
        };
        let outcome = write_job(tmp.path(), with_chest);
        assert!(outcome.is_complete());
        assert_eq!(
            read_containers::<Container>(&containers_dir)
                .unwrap()
                .keys()
                .copied()
                .collect::<Vec<_>>(),
            vec![IVec3::new(17, 1, 1)]
        );

        assert!(write_job(tmp.path(), job(&[IVec3::X])).is_complete());
        assert!(read_containers::<Container>(&containers_dir)
            .unwrap()
            .is_empty());
    }

    #[test]
    fn the_furnaces_of_a_chunk_are_written_with_it_and_removed_once_gone() {
        use crate::world::container_files::{read_containers, FURNACES_DIR};
        use shared::world::Furnace;

        let tmp = tempfile::tempdir().unwrap();
        let furnaces_dir = tmp.path().join(FURNACES_DIR);
        let burning = Furnace {
            burn_left: 7,
            burn_total: 300,
            ..Default::default()
        };
        let with_furnace = SaveJob {
            meta: meta(1),
            players: Vec::new(),
            chunks: vec![(
                IVec3::X,
                stone_chunk(),
                ChunkContents {
                    furnaces: vec![(IVec3::ONE, burning.clone())],
                    ..Default::default()
                },
            )],
        };
        assert!(write_job(tmp.path(), with_furnace).is_complete());
        assert_eq!(
            read_containers::<Furnace>(&furnaces_dir).unwrap(),
            HashMap::from([(IVec3::new(17, 1, 1), burning)])
        );
        assert!(!tmp.path().join(CONTAINERS_DIR).exists());

        assert!(write_job(tmp.path(), job(&[IVec3::X])).is_complete());
        assert!(read_containers::<Furnace>(&furnaces_dir)
            .unwrap()
            .is_empty());
    }

    #[test]
    fn the_players_of_a_job_are_written_in_their_folder() {
        let tmp = tempfile::tempdir().unwrap();
        let mut with_player = job(&[IVec3::ZERO]);
        with_player.players = vec![(7, "(name: \"Zed\")".into())];

        assert!(write_job(tmp.path(), with_player).is_complete());

        let players_dir = tmp.path().join(PLAYERS_DIR);
        assert_eq!(
            std::fs::read_to_string(players_dir.join("7.ron")).unwrap(),
            "(name: \"Zed\")"
        );
        assert_eq!(std::fs::read_dir(players_dir).unwrap().count(), 1);
    }

    #[test]
    fn a_world_without_chests_or_furnaces_has_no_folder_for_them() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(write_job(tmp.path(), job(&[IVec3::ZERO])).is_complete());
        assert!(!tmp.path().join(CONTAINERS_DIR).exists());
        assert!(!tmp
            .path()
            .join(crate::world::container_files::FURNACES_DIR)
            .exists());
    }

    #[test]
    fn jobs_are_written_in_order_so_the_last_snapshot_wins() {
        let tmp = tempfile::tempdir().unwrap();
        let writer = WorldWriter::spawn(tmp.path().to_path_buf(), None).unwrap();
        let old = SaveJob {
            meta: meta(1),
            players: Vec::new(),
            chunks: vec![(IVec3::ZERO, Chunk::new(), ChunkContents::default())],
        };
        assert!(writer.submit(old).is_ok());
        assert!(writer.submit(job(&[IVec3::ZERO])).is_ok());
        drop(writer);

        let chunks = read_chunks(&tmp.path().join(CHUNKS_DIR)).unwrap();
        assert_eq!(chunks[&IVec3::ZERO], stone_chunk());
    }

    #[test]
    fn a_chunk_that_cannot_be_written_is_reported_and_the_others_are_written() {
        let tmp = tempfile::tempdir().unwrap();
        let chunks_dir = tmp.path().join(CHUNKS_DIR);
        std::fs::create_dir_all(&chunks_dir).unwrap();
        // A directory squatting on the temporary name makes the write of this chunk's region
        // fail, and only that one's
        std::fs::create_dir(chunks_dir.join("0_0_0.region.tmp")).unwrap();
        let writer = WorldWriter::spawn(tmp.path().to_path_buf(), None).unwrap();
        let elsewhere = IVec3::X * REGION_SIZE;

        assert!(writer.submit(job(&[IVec3::ZERO, elsewhere])).is_ok());
        let start = Instant::now();
        let failed = loop {
            let failed = writer.take_failed_chunks();
            if !failed.is_empty() {
                break failed;
            }
            assert!(start.elapsed() < Duration::from_secs(10), "no report");
            std::thread::sleep(Duration::from_millis(5));
        };

        assert_eq!(failed, vec![IVec3::ZERO]);
        let written = read_chunks(&chunks_dir).unwrap();
        assert_eq!(written.keys().copied().collect::<Vec<_>>(), vec![elsewhere]);
    }

    #[test]
    fn a_written_save_is_counted_and_a_failed_one_is_not() {
        let tmp = tempfile::tempdir().unwrap();
        let counter = SaveCounter::default();
        let mut writer =
            WorldWriter::spawn(tmp.path().to_path_buf(), Some(counter.clone())).unwrap();

        assert!(writer.submit(job(&[IVec3::X])).is_ok());
        let start = Instant::now();
        while counter.get() != 1 {
            assert!(start.elapsed() < Duration::from_secs(10), "not counted");
            std::thread::sleep(Duration::from_millis(5));
        }

        // A directory squatting on the temporary name makes this chunk's write fail
        std::fs::create_dir(tmp.path().join(CHUNKS_DIR).join("0_0_0.region.tmp")).unwrap();
        assert!(writer.submit(job(&[IVec3::ZERO])).is_ok());
        writer.close_for_test();

        assert_eq!(counter.get(), 1);
        assert_eq!(counter.failures(), 1);
    }

    #[test]
    fn a_save_whose_world_file_cannot_be_written_is_a_failure() {
        let tmp = tempfile::tempdir().unwrap();
        // A directory squatting on the temporary name makes world.ron's write fail
        std::fs::create_dir(tmp.path().join("world.ron.tmp")).unwrap();
        let counter = SaveCounter::default();
        let mut writer =
            WorldWriter::spawn(tmp.path().to_path_buf(), Some(counter.clone())).unwrap();

        assert!(writer.submit(job(&[IVec3::X])).is_ok());
        writer.close_for_test();

        assert_eq!(counter.get(), 0);
        assert_eq!(counter.failures(), 1);
        // the chunks are written all the same, and none is reported
        assert_eq!(read_chunks(&tmp.path().join(CHUNKS_DIR)).unwrap().len(), 1);
        assert!(writer.take_failed_chunks().is_empty());
    }

    #[test]
    fn a_dead_writer_gives_the_job_back_and_dropping_it_returns() {
        // The state after the writer thread died: nobody receives the jobs any more. (A thread
        // that really panics cannot be tested: the dev profile aborts on panic.)
        let (jobs, queue) = mpsc::channel();
        let (_report, failures) = mpsc::channel();
        drop(queue);
        let writer = WorldWriter {
            jobs: Some(jobs),
            failures: Mutex::new(failures),
            unfinished: Default::default(),
            thread: None,
        };

        let given_back = writer.submit(job(&[IVec3::X])).err().unwrap();

        assert_eq!(given_back.chunks.len(), 1);
        // Nothing was queued
        assert!(writer.is_idle());
        drop(writer);
    }

    #[test]
    fn the_handler_sees_the_jobs_one_at_a_time_in_order() {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let sink = seen.clone();
        let writer = WorldWriter::spawn_with(move |job| {
            sink.lock().unwrap().push(job.meta.time);
            Vec::new()
        })
        .unwrap();
        for time in 1..=5 {
            let mut j = job(&[]);
            j.meta = meta(time);
            assert!(writer.submit(j).is_ok());
        }
        drop(writer);

        assert_eq!(*seen.lock().unwrap(), vec![1, 2, 3, 4, 5]);
    }
}
