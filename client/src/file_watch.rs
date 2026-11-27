//! A folder of the assets watched while a dev build runs, for the data the game reads again
//! when one of its files is saved: the language files (`i18n/watch.rs`) and the theme
//! (`ui/theme/watch.rs`).

use std::path::Path;
use std::sync::mpsc::{self, Receiver};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use bevy::prelude::*;
use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};

/// How long the files must stay unchanged before they are read: an editor writes a file in
/// several steps.
const SETTLE: Duration = Duration::from_millis(200);

/// Watches the `.ron` files of `folder`. The watcher stops when it is dropped.
pub fn watch(folder: &Path) -> notify::Result<(RecommendedWatcher, Changes)> {
    let (sender, changes) = mpsc::channel();
    let mut watcher = notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
        match event {
            Ok(event)
                if !matches!(event.kind, EventKind::Access(_))
                    && event
                        .paths
                        .iter()
                        .any(|path| path.extension().is_some_and(|e| e == "ron")) =>
            {
                // the receiver is gone only with the app
                let _ = sender.send(());
            }
            Ok(_) => {}
            Err(err) => warn!("A watcher of the assets failed: {err}"),
        }
    })?;
    watcher.watch(folder, RecursiveMode::NonRecursive)?;
    Ok((watcher, Changes::new(changes)))
}

/// The changes a watcher saw in its files, and when the files are to be read.
pub struct Changes {
    /// A message for each change.
    changes: Mutex<Receiver<()>>,
    /// When they are read: `SETTLE` after the last change.
    pub due: Option<Instant>,
}

impl Changes {
    /// The changes sent on `changes`: those of `watch`, or of a test.
    pub fn new(changes: Receiver<()>) -> Self {
        Self {
            changes: Mutex::new(changes),
            due: None,
        }
    }

    /// Whether the files are to be read at `now`: they changed, then stayed unchanged for
    /// `SETTLE`.
    pub fn take_due(&mut self, now: Instant) -> bool {
        let changes = self
            .changes
            .get_mut()
            .unwrap_or_else(|poison| poison.into_inner());
        if changes.try_iter().count() > 0 {
            self.due = Some(now + SETTLE);
        }
        if self.due.is_some_and(|due| due <= now) {
            self.due = None;
            return true;
        }
        false
    }
}
