//! The thread that writes the chunks and the metadata of a world to disk, so that saving them
//! never makes the tick wait for the disk (the player files are small and written by the
//! tick). The tick thread hands it snapshots through a channel and the thread writes them in
//! order. Dropping the writer waits for everything queued to be written.

use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Mutex, PoisonError};
use std::thread::JoinHandle;

use bevy::{
    ecs::resource::Resource,
    log::{debug, error, info},
    math::IVec3,
};
use shared::world::Chunk;

use super::chunk_files::{chunk_file_name, write_chunk, CHUNKS_DIR};
use super::format::{write_meta, WorldMeta, WORLD_FILE};

/// What one save writes: the metadata of the world and a copy of each chunk to write.
pub struct SaveJob {
    pub meta: WorldMeta,
    pub chunks: Vec<(IVec3, Chunk)>,
}

/// Writes `job` into the folder `world_dir`. The metadata goes first, so that the seed of a new
/// world is on disk before any of its chunks. Returns the chunks that could not be written;
/// every failure is logged with its path.
pub fn write_job(world_dir: &std::path::Path, job: SaveJob) -> Vec<IVec3> {
    if let Err(err) = write_meta(world_dir, &job.meta) {
        error!(
            "Failed to write {}: {err}",
            world_dir.join(WORLD_FILE).display()
        );
    }

    let chunks_dir = world_dir.join(CHUNKS_DIR);
    if let Err(err) = std::fs::create_dir_all(&chunks_dir) {
        error!("Failed to create {}: {err}", chunks_dir.display());
        return job.chunks.into_iter().map(|(pos, _)| pos).collect();
    }

    let total = job.chunks.len();
    let mut failed = Vec::new();
    for (chunk_pos, chunk) in job.chunks {
        if let Err(err) = write_chunk(&chunks_dir, chunk_pos, &chunk) {
            error!(
                "Failed to write {}: {err}",
                chunks_dir.join(chunk_file_name(chunk_pos)).display()
            );
            failed.push(chunk_pos);
        }
    }
    info!(
        "World [{}] saved: {} of {total} chunks written",
        job.meta.name,
        total - failed.len()
    );
    failed
}

#[derive(Resource)]
pub struct WorldWriter {
    jobs: Option<Sender<SaveJob>>,
    /// The chunks each finished job could not write.
    failures: Mutex<Receiver<Vec<IVec3>>>,
    thread: Option<JoinHandle<()>>,
}

impl WorldWriter {
    /// Starts a writer that saves into `world_dir`.
    pub fn spawn(world_dir: PathBuf) -> std::io::Result<Self> {
        Self::spawn_with(move |job| write_job(&world_dir, job))
    }

    /// Starts a writer that handles each job with `handle`, which returns the chunks it failed
    /// to write.
    pub(super) fn spawn_with(
        mut handle: impl FnMut(SaveJob) -> Vec<IVec3> + Send + 'static,
    ) -> std::io::Result<Self> {
        let (jobs, queue) = mpsc::channel::<SaveJob>();
        let (report, failures) = mpsc::channel();
        let thread = std::thread::Builder::new()
            .name("world-writer".into())
            .spawn(move || {
                // Ends once the sender is dropped and the queue is empty
                for job in queue {
                    // The tick thread may be gone already
                    let _ = report.send(handle(job));
                }
            })?;
        Ok(Self {
            jobs: Some(jobs),
            failures: Mutex::new(failures),
            thread: Some(thread),
        })
    }

    /// Queues `job`. Gives it back when the writer thread is gone, so that nothing is lost
    /// silently.
    pub fn submit(&self, job: SaveJob) -> Result<(), SaveJob> {
        match &self.jobs {
            Some(jobs) => jobs.send(job).map_err(|err| err.0),
            None => Err(job),
        }
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
    use crate::world::chunk_files::read_chunks;
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
            chunks: positions.iter().map(|&p| (p, stone_chunk())).collect(),
        }
    }

    #[test]
    fn dropping_the_writer_waits_for_every_queued_job() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().to_path_buf();
        let writer = WorldWriter::spawn(dir.clone()).unwrap();
        for x in 0..20 {
            assert!(writer.submit(job(&[IVec3::new(x, 0, 0)])).is_ok());
        }

        drop(writer);

        assert_eq!(read_chunks(&dir.join(CHUNKS_DIR)).unwrap().len(), 20);
        assert!(dir.join("world.ron").is_file());
    }

    #[test]
    fn jobs_are_written_in_order_so_the_last_snapshot_wins() {
        let tmp = tempfile::tempdir().unwrap();
        let writer = WorldWriter::spawn(tmp.path().to_path_buf()).unwrap();
        let old = SaveJob {
            meta: meta(1),
            chunks: vec![(IVec3::ZERO, Chunk::new())],
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
        // A directory squatting on the temporary name makes this chunk's write fail
        std::fs::create_dir(chunks_dir.join("0_0_0.bin.tmp")).unwrap();
        let writer = WorldWriter::spawn(tmp.path().to_path_buf()).unwrap();

        assert!(writer.submit(job(&[IVec3::ZERO, IVec3::X])).is_ok());
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
        assert_eq!(written.keys().copied().collect::<Vec<_>>(), vec![IVec3::X]);
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
            thread: None,
        };

        let given_back = writer.submit(job(&[IVec3::X])).err().unwrap();

        assert_eq!(given_back.chunks.len(), 1);
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
