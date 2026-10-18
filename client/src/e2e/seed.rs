//! The game folder a test starts the client with.

use std::path::Path;

use tempfile::TempDir;

/// A temporary game folder, deleted when it is dropped.
pub struct GameFolder {
    dir: TempDir,
}

impl GameFolder {
    pub fn new() -> Self {
        Self {
            dir: TempDir::new().expect("a temporary game folder"),
        }
    }

    pub fn path(&self) -> &Path {
        self.dir.path()
    }
}
