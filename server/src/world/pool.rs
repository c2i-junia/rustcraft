//! The threads the server makes and encodes its chunks on.

use std::sync::LazyLock;

use bevy::tasks::{TaskPool, TaskPoolBuilder};

/// Threads of each pool of the server: half the processor, at most 16. A player who joins with
/// a large render distance keeps the generation pool busy for seconds.
fn pool_threads() -> usize {
    std::thread::available_parallelism()
        .map_or(4, |n| n.get() / 2)
        .clamp(1, 16)
}

/// The pool the server encodes chunks on: its own, not Bevy's compute pool, which an
/// integrated server shares with the client of its process. A thread of that pool waiting for a
/// task runs others meanwhile: a frame of the client would wait for the server's work, and the
/// server for the systems of the client.
pub(crate) fn server_pool() -> &'static TaskPool {
    static POOL: LazyLock<TaskPool> = LazyLock::new(|| {
        TaskPoolBuilder::new()
            .num_threads(pool_threads())
            .thread_name("Server Task Pool".to_string())
            .build()
    });
    &POOL
}

/// The pool the server loads and generates chunks on, in the background: apart from
/// `server_pool`, so that a tick encoding chunks does not wait behind the chunks being made.
pub(crate) fn generation_pool() -> &'static TaskPool {
    static POOL: LazyLock<TaskPool> = LazyLock::new(|| {
        TaskPoolBuilder::new()
            .num_threads(pool_threads())
            .thread_name("Server Generation Pool".to_string())
            .build()
    });
    &POOL
}
