//! Where the game keeps its saves and its assets on each platform.

use std::{ffi::OsString, path::PathBuf};

use bevy_ecs::resource::Resource;
use bevy_log::warn;

#[derive(Resource, Debug, Clone)]
pub struct GameFolderPaths {
    pub game_folder_path: PathBuf,
    pub assets_folder_path: PathBuf,
}

pub fn get_game_folder_paths(
    game_folder_path: Option<String>,
    assets_folder_path: Option<String>,
) -> GameFolderPaths {
    let mut paths = default_game_folder_paths();

    if let Some(game_data) = game_folder_path {
        paths.game_folder_path = game_data.into();
    }
    if let Some(game_assets) = assets_folder_path {
        paths.assets_folder_path = game_assets.into();
    }

    paths
}

#[derive(Clone, Copy)]
enum Platform {
    Linux,
    Windows,
    MacOs,
}

/// Default folders of the current platform, resolved from the process environment.
pub fn default_game_folder_paths() -> GameFolderPaths {
    let platform = if cfg!(target_os = "windows") {
        Platform::Windows
    } else if cfg!(target_os = "macos") {
        Platform::MacOs
    } else {
        Platform::Linux
    };
    game_folder_paths_for(platform, |name| std::env::var_os(name))
}

/// Builds the default folders of `platform`. `env` looks an environment variable up; unset
/// and empty variables are treated alike. Without a usable home folder the game falls back
/// to a `rustcraft` folder in the working directory.
fn game_folder_paths_for(
    platform: Platform,
    env: impl Fn(&str) -> Option<OsString>,
) -> GameFolderPaths {
    let var = |name: &str| {
        env(name)
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
    };

    let folders = match platform {
        Platform::Linux => var("HOME").map(|home| {
            let data_home = var("XDG_DATA_HOME").unwrap_or_else(|| home.join(".local/share"));
            (data_home.join("rustcraft"), home.join(".config/rustcraft"))
        }),
        Platform::Windows => var("APPDATA").map(|app_data| {
            let game_folder = app_data.join("rustcraft");
            (game_folder.clone(), game_folder.join("data"))
        }),
        Platform::MacOs => var("HOME").map(|home| {
            let game_folder = home.join("Library/Application Support/rustcraft");
            (game_folder.clone(), game_folder.join("data"))
        }),
    };

    let (game_folder_path, assets_folder_path) = folders.unwrap_or_else(|| {
        warn!("No home folder found in the environment, using ./rustcraft");
        ("rustcraft".into(), "rustcraft/data".into())
    });
    GameFolderPaths {
        game_folder_path,
        assets_folder_path,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paths_with(platform: Platform, vars: &[(&str, &str)]) -> (PathBuf, PathBuf) {
        let paths = game_folder_paths_for(platform, |name| {
            vars.iter()
                .find(|(key, _)| *key == name)
                .map(|(_, value)| OsString::from(value))
        });
        (paths.game_folder_path, paths.assets_folder_path)
    }

    #[test]
    fn linux_defaults_live_under_home() {
        let (game, assets) = paths_with(Platform::Linux, &[("HOME", "/home/bob")]);
        assert_eq!(game, PathBuf::from("/home/bob/.local/share/rustcraft"));
        assert_eq!(assets, PathBuf::from("/home/bob/.config/rustcraft"));
    }

    #[test]
    fn linux_honours_xdg_data_home_for_the_game_folder() {
        let (game, assets) = paths_with(
            Platform::Linux,
            &[("HOME", "/home/bob"), ("XDG_DATA_HOME", "/data")],
        );
        assert_eq!(game, PathBuf::from("/data/rustcraft"));
        assert_eq!(assets, PathBuf::from("/home/bob/.config/rustcraft"));
    }

    #[test]
    fn empty_variables_count_as_unset() {
        let (game, _) = paths_with(
            Platform::Linux,
            &[("HOME", "/home/bob"), ("XDG_DATA_HOME", "")],
        );
        assert_eq!(game, PathBuf::from("/home/bob/.local/share/rustcraft"));
    }

    #[test]
    fn windows_defaults_live_under_appdata() {
        let (game, assets) = paths_with(Platform::Windows, &[("APPDATA", "C:/Users/bob/AppData")]);
        assert_eq!(game, PathBuf::from("C:/Users/bob/AppData/rustcraft"));
        assert_eq!(assets, PathBuf::from("C:/Users/bob/AppData/rustcraft/data"));
    }

    #[test]
    fn macos_defaults_live_under_application_support() {
        let (game, assets) = paths_with(Platform::MacOs, &[("HOME", "/Users/bob")]);
        let support = PathBuf::from("/Users/bob/Library/Application Support/rustcraft");
        assert_eq!(game, support);
        assert_eq!(assets, support.join("data"));
    }

    #[test]
    fn missing_home_falls_back_to_the_working_directory() {
        for platform in [Platform::Linux, Platform::Windows, Platform::MacOs] {
            let (game, assets) = paths_with(platform, &[]);
            assert_eq!(game, PathBuf::from("rustcraft"));
            assert_eq!(assets, PathBuf::from("rustcraft/data"));
        }
    }
}
