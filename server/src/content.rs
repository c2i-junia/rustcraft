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
use shared::content::{content, install, tell_new_names, tell_replaced, Content};
use shared::messages::{FullChatMessage, PlayerId, ServerToClientMessage};
use shared::utils::unix_time_ms;
use shared::{ContentSource, GameFolderPaths, GameServerConfig};

use crate::init::ServerLobby;
use crate::network::commands::SERVER_AUTHOR;
use crate::network::extensions::SendGameMessageExtension;
use crate::network::permissions::Permissions;
use crate::world::weather::send_to_players;

/// How long the files must stay unchanged before they are read: an editor writes a file in
/// several steps, and a save may touch several files.
const SETTLE: Duration = Duration::from_millis(200);

/// The folder of the content files of the base game, in the assets: the one the game started
/// on (`shared::content::start`), the source tree's when the assets have none.
pub fn content_folder(paths: &GameFolderPaths) -> Option<PathBuf> {
    shared::content::files_folder(&shared::content::folder(&paths.assets_folder_path))
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
    /// Why the files were not used at start, told in the chat to the first operator who joins:
    /// a solo world opened on a broken file plays with the content the game holds.
    untold: Option<String>,
}

impl ContentFiles {
    /// Reads the files of `source` and installs them, then watches them if `source` says so.
    /// `None` for the content of the process (`ContentSource::Initial`), which reads no file.
    /// Without a folder of files, the server keeps the content of the process.
    pub fn start(paths: &GameFolderPaths, source: ContentSource) -> Option<Self> {
        if source == ContentSource::Initial {
            return None;
        }
        let Some(folder) = content_folder(paths) else {
            info!(
                "No content files in {}: the content of the process is kept",
                paths.assets_folder_path.display()
            );
            return None;
        };
        let mut untold = None;
        match Content::load_telling(&folder, Some(content())) {
            Ok((new, new_names, replaced)) => {
                tell_new_names(&new_names);
                tell_replaced(&replaced);
                if install_if_new(new) {
                    info!("Content files of {} installed", folder.display());
                }
            }
            Err(err) => {
                error!("The content files are not used: {err}");
                untold = Some(format!("Content files not used: {err}"));
            }
        }
        let mut files = Self::new(folder);
        files.untold = untold;
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
            untold: None,
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

/// The players of `lobby` who may run the commands of the operators: those told what is wrong
/// in a content file, whose errors name the paths of the server.
fn operators<'a>(
    lobby: &'a ServerLobby,
    permissions: &'a Permissions,
    is_solo: bool,
) -> impl Iterator<Item = PlayerId> + 'a {
    lobby
        .players
        .iter()
        .filter(move |(_, player)| permissions.may_run(&player.name, player.account, is_solo))
        .map(|(id, _)| *id)
}

/// A chat message of the server.
fn server_chat(content: String) -> ServerToClientMessage {
    ServerToClientMessage::ChatMessage(FullChatMessage {
        author: SERVER_AUTHOR.to_string(),
        content,
        timestamp: unix_time_ms(),
    })
}

/// Reads the content files once they are due, installs them and sends them to every player.
/// Tells the players how it went, and the operators alone what is wrong in a file, and why the
/// files were not used at start once one joined.
pub fn reload_content_system(
    files: Option<ResMut<ContentFiles>>,
    mut server: ResMut<RenetServer>,
    lobby: Res<ServerLobby>,
    (permissions, config): (Res<Permissions>, Res<GameServerConfig>),
) {
    let Some(mut files) = files else {
        return;
    };
    if files.untold.is_some() {
        let told: Vec<PlayerId> = operators(&lobby, &permissions, config.is_solo).collect();
        if !told.is_empty() {
            let message = server_chat(files.untold.take().unwrap_or_default());
            for id in told {
                server.send_game_message(id, message.clone());
            }
        }
    }
    if !files.take_due(Instant::now()) {
        return;
    }
    let mut told = Vec::new();
    match Content::load_telling(&files.folder, Some(content())) {
        Ok((new, new_names, replaced)) => {
            tell_new_names(&new_names);
            tell_replaced(&replaced);
            told = new_names.iter().map(ToString::to_string).collect();
            let answer = if install_if_new(new) {
                info!("Content files reloaded");
                send_to_players(
                    &mut server,
                    &lobby,
                    ServerToClientMessage::Content(content().clone()),
                );
                "Content files reloaded"
            } else {
                "The content files changed nothing"
            };
            told.insert(0, answer.to_string());
        }
        Err(err) => {
            error!("The content files are not reloaded: {err}");
            let message = server_chat(format!("Content files not reloaded: {err}"));
            for id in operators(&lobby, &permissions, config.is_solo) {
                server.send_game_message(id, message.clone());
            }
        }
    }
    // the answer, then the blocks and the items it numbered anew, of which a name may be mistyped
    for content in told {
        send_to_players(&mut server, &lobby, server_chat(content));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::init::LobbyPlayer;

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

    /// The files a game started on when its assets have none, the source tree's, are those the
    /// server reads again on `/reload`.
    #[test]
    fn a_server_whose_assets_have_no_content_files_reads_those_the_game_started_on() {
        let paths = GameFolderPaths {
            game_folder_path: PathBuf::from("/nowhere"),
            assets_folder_path: PathBuf::from("/nowhere"),
        };
        let files = ContentFiles::start(&paths, ContentSource::Files).unwrap();
        let started =
            shared::content::start(&shared::content::folder(&paths.assets_folder_path)).unwrap();
        assert_eq!(files.folder, started);
        assert!(files.folder.ends_with("data/content/rustcraft"));
    }

    /// A solo world opened on a broken file plays with the content the game holds, and tells
    /// why to the first operator who joins.
    #[test]
    fn a_server_started_on_a_broken_file_keeps_its_error_for_the_operators() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = GameFolderPaths {
            game_folder_path: tmp.path().to_path_buf(),
            assets_folder_path: tmp.path().join("assets"),
        };
        let folder = shared::content::folder(&paths.assets_folder_path);
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::write(folder.join("broken.ron"), "(blocks: {").unwrap();

        let files = ContentFiles::start(&paths, ContentSource::Files).unwrap();

        let untold = files.untold.unwrap();
        assert!(untold.starts_with("Content files not used: "), "{untold}");
        assert!(untold.contains("broken.ron"), "{untold}");
    }

    #[test]
    fn only_the_operators_are_told_what_is_wrong_in_a_file() {
        let tmp = tempfile::tempdir().unwrap();
        Permissions::add_operators(
            tmp.path(),
            &[crate::network::permissions::Entry::named("op")],
        )
        .unwrap();
        let permissions = Permissions::load(tmp.path()).unwrap();
        let mut lobby = ServerLobby::default();
        lobby.players.insert(1, LobbyPlayer::new("op".to_string()));
        lobby
            .players
            .insert(2, LobbyPlayer::new("guest".to_string()));

        let told: Vec<PlayerId> = operators(&lobby, &permissions, false).collect();
        assert_eq!(told, [1]);
        let mut solo: Vec<PlayerId> = operators(&lobby, &permissions, true).collect();
        solo.sort();
        assert_eq!(solo, [1, 2], "a solo player is an operator");
    }
}
