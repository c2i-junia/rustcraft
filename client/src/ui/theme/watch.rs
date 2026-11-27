//! The theme read again while a dev build runs, a moment after `ui/theme.ron` is saved: a
//! changed colour or size shows without a rebuild. The screens shown are rebuilt
//! (`RebuildScreens`); what is not one of them takes the new theme when it is spawned again. A
//! wrong file changes nothing. Release builds have no `dev` feature, and so no watcher; the tests
//! never add `watch_plugin`.

use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Instant;

use bevy::prelude::*;
use notify::RecommendedWatcher;
use shared::GameFolderPaths;

use super::{install, read, theme_file};
use crate::file_watch::{watch, Changes};
use crate::ui::rebuild::RebuildScreens;

/// Watches the theme of the assets folder of the client, which `add_client` inserted. Without
/// the file there is nothing to watch.
pub fn watch_plugin(app: &mut App) {
    let file = theme_file(&app.world().resource::<GameFolderPaths>().assets_folder_path);
    let Some(folder) = file.parent().filter(|_| file.exists()) else {
        return;
    };
    match watch(folder) {
        Ok((watcher, changes)) => {
            info!("Watching the theme {}", file.display());
            app.insert_resource(ThemeWatcher {
                _watcher: Mutex::new(watcher),
            })
            .insert_resource(ThemeFile { file, changes })
            .add_systems(Update, reload_theme);
        }
        Err(err) => warn!("Cannot watch the theme {}: {err}", file.display()),
    }
}

/// Keeps the watcher of the theme alive.
#[derive(Resource)]
struct ThemeWatcher {
    _watcher: Mutex<RecommendedWatcher>,
}

/// The file of the theme, and the changes the watcher saw in its folder.
#[derive(Resource)]
struct ThemeFile {
    file: PathBuf,
    changes: Changes,
}

/// Reads the theme again once its file settled after a change, puts it in use and has the
/// screens shown rebuilt with it. An error goes to the log, and the theme stays.
fn reload_theme(mut theme: ResMut<ThemeFile>, mut rebuild: MessageWriter<RebuildScreens>) {
    if !theme.changes.take_due(Instant::now()) {
        return;
    }
    match read(&theme.file) {
        Ok(new) => {
            install(new);
            info!("Theme {} reloaded", theme.file.display());
            rebuild.write(RebuildScreens);
        }
        Err(err) => error!("The theme is not reloaded: {err}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::schedule::SingleThreadedExecutor;
    use std::path::Path;
    use std::sync::mpsc;

    use crate::ui::theme;

    /// A copy of the theme of the source tree in a folder of its own.
    fn copied_theme() -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let data = Path::new(env!("CARGO_MANIFEST_DIR")).join("../data");
        let file = dir.path().join("theme.ron");
        std::fs::copy(theme_file(&data), &file).unwrap();
        (dir, file)
    }

    /// Replaces `from` by `to` in `file`.
    fn edit(file: &Path, from: &str, to: &str) {
        let source = std::fs::read_to_string(file).unwrap();
        assert!(source.contains(from));
        std::fs::write(file, source.replace(from, to)).unwrap();
    }

    /// An app that reloads `file` on what `sender` sends, without a watcher. Its systems run on
    /// the thread of the test, which alone sees the theme they install (`INSTALLED_IN_TEST`).
    fn app(file: &Path) -> (App, mpsc::Sender<()>) {
        let (sender, changes) = mpsc::channel();
        let mut app = App::new();
        app.add_message::<RebuildScreens>()
            .insert_resource(ThemeFile {
                file: file.to_path_buf(),
                changes: Changes::new(changes),
            })
            .add_systems(Update, reload_theme)
            .edit_schedule(Update, |schedule| {
                schedule.set_executor(SingleThreadedExecutor::new());
            });
        (app, sender)
    }

    /// Sends a change, then runs the app past the settle delay.
    fn change_and_settle(app: &mut App, sender: &mpsc::Sender<()>) {
        sender.send(()).unwrap();
        app.update();
        assert!(app.world().resource::<ThemeFile>().changes.due.is_some());
        app.world_mut().resource_mut::<ThemeFile>().changes.due = Some(Instant::now());
        app.update();
    }

    fn rebuilds(app: &App) -> usize {
        app.world().resource::<Messages<RebuildScreens>>().len()
    }

    #[test]
    fn a_changed_colour_is_put_in_use_and_rebuilds_the_screens() {
        let (_dir, file) = copied_theme();
        let (mut app, sender) = app(&file);
        app.update();
        assert_eq!(rebuilds(&app), 0, "nothing changed yet");
        let accent = theme::accent();
        edit(&file, "accent: \"#F0B54A\"", "accent: \"#FF0000\"");
        edit(&file, "space_m: 12.0", "space_m: 13.0");
        change_and_settle(&mut app, &sender);
        assert_eq!(theme::accent(), Color::srgb_u8(0xFF, 0, 0));
        assert_ne!(theme::accent(), accent);
        assert_eq!(theme::space_m(), 13.0);
        assert_eq!(
            theme::panel(),
            Color::srgb_u8(0x11, 0x13, 0x18),
            "the rest stays"
        );
        assert_eq!(rebuilds(&app), 1);
    }

    #[test]
    fn a_wrong_file_changes_nothing() {
        let (_dir, file) = copied_theme();
        let (mut app, sender) = app(&file);
        edit(&file, "accent: \"#F0B54A\"", "accent: \"#F0B54\"");
        change_and_settle(&mut app, &sender);
        assert_eq!(rebuilds(&app), 0);
        assert_eq!(theme::accent(), Color::srgb_u8(0xF0, 0xB5, 0x4A));
    }
}
