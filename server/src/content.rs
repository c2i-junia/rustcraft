//! The content files of the server (`docs/design/content-registry.md`, 4.1): read at start, then
//! again on `/reload` and, when they are watched, a moment after one of them is saved. A set of
//! files with an error changes nothing; a good one is installed and sent to every player.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use bevy::prelude::*;
use bevy_renet::RenetServer;
use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use shared::content::{content, install, Content};
use shared::messages::{FullChatMessage, ServerToClientMessage};
use shared::utils::unix_time_ms;
use shared::{ContentSource, GameFolderPaths};

use crate::init::ServerLobby;
use crate::network::commands::SERVER_AUTHOR;
use crate::world::weather::send_to_players;

/// How long the files must stay unchanged before they are read: an editor writes a file in
/// several steps, and a save may touch several files.
const SETTLE: Duration = Duration::from_millis(200);

/// The folder of the content files of the base game, in the assets.
pub fn content_folder(paths: &GameFolderPaths) -> PathBuf {
    shared::content::folder(&paths.assets_folder_path)
}

/// The content files of a server that reads them (`ContentSource::Files`, `WatchedFiles`).
#[derive(Resource)]
pub struct ContentFiles {
    /// Where they are, `content_folder`.
    folder: PathBuf,
    /// Set by `/reload`: they are read on the next tick.
    asked: bool,
    /// A message for each change the watcher saw, when they are watched.
    changes: Option<Mutex<Receiver<()>>>,
    /// When they are read, after a change: `SETTLE` after the last one.
    due: Option<Instant>,
    /// Watches them while it lives.
    _watcher: Option<Mutex<RecommendedWatcher>>,
}

impl ContentFiles {
    /// Reads the files of `source` and installs them, then watches them if `source` says so.
    /// `None` for the content of the process (`ContentSource::Initial`), which reads no file.
    /// Without a folder of files, the server keeps the content of the process.
    pub fn start(paths: &GameFolderPaths, source: ContentSource) -> Option<Self> {
        if source == ContentSource::Initial {
            return None;
        }
        let folder = content_folder(paths);
        if !folder.is_dir() {
            info!(
                "No content files at {}: the content of the process is kept",
                folder.display()
            );
        } else {
            match Content::load(&folder) {
                Ok(new) => {
                    if install_if_new(new) {
                        info!("Content files of {} installed", folder.display());
                    }
                }
                Err(err) => error!("The content files are not used: {err}"),
            }
        }
        let mut files = Self::new(folder);
        if source == ContentSource::WatchedFiles {
            files.watch();
        }
        Some(files)
    }

    fn new(folder: PathBuf) -> Self {
        Self {
            folder,
            asked: false,
            changes: None,
            due: None,
            _watcher: None,
        }
    }

    /// Watches the folder of every namespace, the parent of `folder`, if it exists.
    fn watch(&mut self) {
        let Some(watched) = self.folder.parent().filter(|dir| dir.is_dir()) else {
            info!(
                "No content folder at {}: nothing to watch",
                self.folder.display()
            );
            return;
        };
        let (sender, receiver) = mpsc::channel();
        let watcher = notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
            let event = match event {
                Ok(event) => event,
                Err(err) => {
                    warn!("The watcher of the content files failed: {err}");
                    return;
                }
            };
            if !matches!(event.kind, EventKind::Access(_))
                && event.paths.iter().any(|path| is_ron(path))
            {
                // the receiver is gone only with the server
                let _ = sender.send(());
            }
        })
        .and_then(|mut watcher| {
            watcher.watch(watched, RecursiveMode::Recursive)?;
            Ok(watcher)
        });
        match watcher {
            Ok(watcher) => {
                self.changes = Some(Mutex::new(receiver));
                self._watcher = Some(Mutex::new(watcher));
                info!("Watching the content files of {}", watched.display());
            }
            Err(err) => warn!(
                "Cannot watch the content files of {}: {err}; /reload reads them",
                watched.display()
            ),
        }
    }

    /// Asks for the files to be read on the next tick.
    pub fn ask_reload(&mut self) {
        self.asked = true;
    }

    /// Whether the files are to be read at `now`: they were asked for, or they changed and
    /// have stayed unchanged for `SETTLE`.
    fn take_due(&mut self, now: Instant) -> bool {
        if let Some(changes) = &self.changes {
            let changes = changes.lock().unwrap_or_else(|poison| poison.into_inner());
            if changes.try_iter().count() > 0 {
                self.due = Some(now + SETTLE);
            }
        }
        if self.due.is_some_and(|due| due <= now) {
            self.due = None;
            self.asked = true;
        }
        std::mem::take(&mut self.asked)
    }
}

fn is_ron(path: &Path) -> bool {
    path.extension().is_some_and(|ext| ext == "ron")
}

/// Installs `new` if its properties differ from those of the process. Returns whether it did.
fn install_if_new(new: Content) -> bool {
    if new.same_properties(content()) {
        return false;
    }
    install(new);
    true
}

/// Reads the content files once they are due, installs them and sends them to every player.
/// Tells the players how it went, and above all what is wrong in a file.
pub fn reload_content_system(
    files: Option<ResMut<ContentFiles>>,
    mut server: ResMut<RenetServer>,
    lobby: Res<ServerLobby>,
) {
    let Some(mut files) = files else {
        return;
    };
    if !files.take_due(Instant::now()) {
        return;
    }
    let answer = match Content::load(&files.folder) {
        Ok(new) => {
            if install_if_new(new) {
                info!("Content files reloaded");
                send_to_players(
                    &mut server,
                    &lobby,
                    ServerToClientMessage::Content(content().clone()),
                );
                "Content files reloaded".to_string()
            } else {
                "The content files changed nothing".to_string()
            }
        }
        Err(err) => {
            error!("The content files are not reloaded: {err}");
            format!("Content files not reloaded: {err}")
        }
    };
    send_to_players(
        &mut server,
        &lobby,
        ServerToClientMessage::ChatMessage(FullChatMessage {
            author: SERVER_AUTHOR.to_string(),
            content: answer,
            timestamp: unix_time_ms(),
        }),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_files_are_read_once_asked() {
        let mut files = ContentFiles::new(PathBuf::new());
        let now = Instant::now();
        assert!(!files.take_due(now));
        files.ask_reload();
        assert!(files.take_due(now));
        assert!(!files.take_due(now), "asked once, read once");
    }

    #[test]
    fn the_files_are_read_once_they_stay_unchanged_for_a_moment() {
        let (sender, receiver) = mpsc::channel();
        let mut files = ContentFiles::new(PathBuf::new());
        files.changes = Some(Mutex::new(receiver));
        let start = Instant::now();

        sender.send(()).unwrap();
        assert!(!files.take_due(start));
        // saved again before the moment passed: the moment starts over
        sender.send(()).unwrap();
        sender.send(()).unwrap();
        assert!(!files.take_due(start + SETTLE / 2));
        assert!(!files.take_due(start + SETTLE));
        assert!(files.take_due(start + SETTLE / 2 + SETTLE));
        assert!(!files.take_due(start + SETTLE * 3));
    }

    #[test]
    fn a_server_of_the_content_of_the_process_reads_no_file() {
        let paths = GameFolderPaths {
            game_folder_path: PathBuf::from("/nowhere"),
            assets_folder_path: PathBuf::from("/nowhere"),
        };
        assert!(ContentFiles::start(&paths, ContentSource::Initial).is_none());
    }
}
