//! The threads the server makes and encodes its chunks on.

use std::sync::LazyLock;

use bevy::tasks::{TaskPool, TaskPoolBuilder};

/// The pool the server makes and encodes chunks on: its own, not Bevy's compute pool, which an
/// integrated server shares with the client of its process. A thread of that pool waiting for a
/// task runs others meanwhile: a frame of the client would wait for a batch of chunks being
/// generated, and the server for the systems of the client.
pub(crate) fn server_pool() -> &'static TaskPool {
    static POOL: LazyLock<TaskPool> = LazyLock::new(|| {
        let threads = std::thread::available_parallelism()
            .map_or(4, |n| n.get() / 2)
            .clamp(1, 8);
        TaskPoolBuilder::new()
            .num_threads(threads)
            .thread_name("Server Task Pool".to_string())
            .build()
    });
    &POOL
}
