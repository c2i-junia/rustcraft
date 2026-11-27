//! The language files read again while a dev build runs, a moment after one of them is saved:
//! a changed text shows without a rebuild. The screens shown are rebuilt (`RebuildScreens`);
//! what is not one of them takes the new texts when it is spawned again. A set of files with an
//! error changes nothing. Release builds have no `dev` feature, and so no watcher; the tests
//! never add `watch_plugin`.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use bevy::prelude::*;
use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use shared::GameFolderPaths;

use super::{install, lang_folder, read};
use crate::ui::rebuild::RebuildScreens;

/// How long the files must stay unchanged before they are read: an editor writes a file in
/// several steps.
const SETTLE: Duration = Duration::from_millis(200);

/// Watches the language files of the assets folder of the client, which `add_client` inserted.
pub fn watch_plugin(app: &mut App) {
    let folder = lang_folder(&app.world().resource::<GameFolderPaths>().assets_folder_path);
    let (sender, changes) = mpsc::channel();
    match watch(&folder, sender) {
        Ok(watcher) => {
            info!("Watching the language files of {}", folder.display());
            app.insert_resource(LanguageWatcher {
                _watcher: Mutex::new(watcher),
            })
            .insert_resource(LanguageFiles::new(folder, changes))
            .add_systems(Update, reload_languages);
        }
        Err(err) => warn!(
            "Cannot watch the language files of {}: {err}",
            folder.display()
        ),
    }
}

/// Sends a message on `sender` for each change of a `.ron` file of `folder`.
fn watch(folder: &Path, sender: mpsc::Sender<()>) -> notify::Result<RecommendedWatcher> {
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
            Err(err) => warn!("The watcher of the language files failed: {err}"),
        }
    })?;
    watcher.watch(folder, RecursiveMode::NonRecursive)?;
    Ok(watcher)
}

/// Keeps the watcher of the language files alive.
#[derive(Resource)]
struct LanguageWatcher {
    _watcher: Mutex<RecommendedWatcher>,
}

/// The language files, and the changes the watcher saw in them.
#[derive(Resource)]
struct LanguageFiles {
    folder: PathBuf,
    /// A message for each change.
    changes: Mutex<Receiver<()>>,
    /// When they are read: `SETTLE` after the last change.
    due: Option<Instant>,
}

impl LanguageFiles {
    fn new(folder: PathBuf, changes: Receiver<()>) -> Self {
        Self {
            folder,
            changes: Mutex::new(changes),
            due: None,
        }
    }

    /// Whether the files are to be read at `now`: they changed, then stayed unchanged for
    /// `SETTLE`.
    fn take_due(&mut self, now: Instant) -> bool {
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

/// Reads the language files again once they settled after a change, puts them in use and has
/// the screens shown rebuilt in them. An error goes to the log, and the texts stay.
fn reload_languages(mut files: ResMut<LanguageFiles>, mut rebuild: MessageWriter<RebuildScreens>) {
    if !files.take_due(Instant::now()) {
        return;
    }
    match read(&files.folder) {
        Ok(tables) => {
            install(tables);
            info!("Language files of {} reloaded", files.folder.display());
            rebuild.write(RebuildScreens);
        }
        Err(err) => error!("The language files are not reloaded: {err}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::{tr, Language};

    /// A copy of the language files of the source tree in a folder of its own.
    fn copied_files() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let data = Path::new(env!("CARGO_MANIFEST_DIR")).join("../data");
        for language in Language::ALL {
            let name = format!("{}.ron", language.code());
            std::fs::copy(lang_folder(&data).join(&name), dir.path().join(&name)).unwrap();
        }
        dir
    }

    /// Adds `key: text` to the file of `language` in `dir`.
    fn add_text(dir: &Path, language: Language, key: &str, text: &str) {
        let path = dir.join(format!("{}.ron", language.code()));
        let mut source = std::fs::read_to_string(&path).unwrap();
        let end = source.rfind('}').unwrap();
        source.insert_str(end, &format!("    {key:?}: {text:?},\n"));
        std::fs::write(path, source).unwrap();
    }

    /// An app that reloads the files of `dir` on what `sender` sends, without a watcher.
    fn app(dir: &Path) -> (App, mpsc::Sender<()>) {
        let (sender, changes) = mpsc::channel();
        let mut app = App::new();
        app.add_message::<RebuildScreens>()
            .insert_resource(LanguageFiles::new(dir.to_path_buf(), changes))
            .add_systems(Update, reload_languages);
        (app, sender)
    }

    /// Sends a change, then runs the app past `SETTLE`.
    fn change_and_settle(app: &mut App, sender: &mpsc::Sender<()>) {
        sender.send(()).unwrap();
        app.update();
        assert!(app.world().resource::<LanguageFiles>().due.is_some());
        app.world_mut().resource_mut::<LanguageFiles>().due = Some(Instant::now());
        app.update();
    }

    fn rebuilds(app: &App) -> usize {
        app.world().resource::<Messages<RebuildScreens>>().len()
    }

    #[test]
    fn a_changed_table_is_put_in_use_and_rebuilds_the_screens() {
        let dir = copied_files();
        let (mut app, sender) = app(dir.path());
        app.update();
        assert_eq!(rebuilds(&app), 0, "nothing changed yet");
        // every other text stays the same: the tests beside this one read the tables too
        add_text(dir.path(), Language::English, "test.reloaded", "Reloaded");
        add_text(dir.path(), Language::French, "test.reloaded", "Relu");
        assert_eq!(tr("test.reloaded"), "test.reloaded");
        change_and_settle(&mut app, &sender);
        assert_eq!(tr("test.reloaded"), "Reloaded");
        crate::i18n::in_language(Language::French, || {
            assert_eq!(tr("test.reloaded"), "Relu");
        });
        assert_eq!(rebuilds(&app), 1);
    }

    #[test]
    fn a_wrong_file_changes_nothing() {
        let dir = copied_files();
        let (mut app, sender) = app(dir.path());
        std::fs::write(dir.path().join("fr.ron"), "{ \"home.quit_confirm\": ").unwrap();
        change_and_settle(&mut app, &sender);
        assert_eq!(rebuilds(&app), 0);
        crate::i18n::in_language(Language::French, || {
            assert_eq!(tr("home.quit_confirm"), "Quitter");
        });
    }
}
