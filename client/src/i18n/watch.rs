//! The language files read again while a dev build runs, a moment after one of them is saved:
//! a changed text shows without a rebuild. The screens shown are rebuilt (`RebuildScreens`);
//! what is not one of them takes the new texts when it is spawned again. A set of files with an
//! error changes nothing. Release builds have no `dev` feature, and so no watcher; the tests
//! never add `watch_plugin`.

use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Instant;

use bevy::prelude::*;
use notify::RecommendedWatcher;
use shared::GameFolderPaths;

use super::{install, lang_folder, read};
use crate::file_watch::{watch, Changes};
use crate::ui::rebuild::RebuildScreens;

/// Watches the language files of the assets folder of the client, which `add_client` inserted.
pub fn watch_plugin(app: &mut App) {
    let folder = lang_folder(&app.world().resource::<GameFolderPaths>().assets_folder_path);
    match watch(&folder) {
        Ok((watcher, changes)) => {
            info!("Watching the language files of {}", folder.display());
            app.insert_resource(LanguageWatcher {
                _watcher: Mutex::new(watcher),
            })
            .insert_resource(LanguageFiles { folder, changes })
            .add_systems(Update, reload_languages);
        }
        Err(err) => warn!(
            "Cannot watch the language files of {}: {err}",
            folder.display()
        ),
    }
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
    changes: Changes,
}

/// Reads the language files again once they settled after a change, puts them in use and has
/// the screens shown rebuilt in them. An error goes to the log, and the texts stay.
fn reload_languages(mut files: ResMut<LanguageFiles>, mut rebuild: MessageWriter<RebuildScreens>) {
    if !files.changes.take_due(Instant::now()) {
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
    use std::path::Path;
    use std::sync::mpsc;

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
            .insert_resource(LanguageFiles {
                folder: dir.to_path_buf(),
                changes: Changes::new(changes),
            })
            .add_systems(Update, reload_languages);
        (app, sender)
    }

    /// Sends a change, then runs the app past `SETTLE`.
    fn change_and_settle(app: &mut App, sender: &mpsc::Sender<()>) {
        sender.send(()).unwrap();
        app.update();
        assert!(app
            .world()
            .resource::<LanguageFiles>()
            .changes
            .due
            .is_some());
        app.world_mut().resource_mut::<LanguageFiles>().changes.due = Some(Instant::now());
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
