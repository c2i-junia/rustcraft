// Reading the `RenderDevice` resource overflows the default limit of the trait solver (benches)
#![recursion_limit = "256"]
mod app;
mod audio;
mod camera;
mod constants;
mod dropped_items;
#[cfg(test)]
mod e2e;
mod game;
mod input;
mod mob;
mod network;
mod player;
mod state;
mod ui;
mod world;

use app::ClientSettings;
use bevy::{
    app::PluginGroupBuilder,
    asset::io::file::FileAssetReader,
    log::{LogPlugin, DEFAULT_FILTER},
    prelude::*,
    render::{
        settings::{RenderCreation, WgpuFeatures, WgpuSettings},
        RenderPlugin,
    },
    window::PresentMode,
};
#[cfg(debug_assertions)]
use bevy_inspector_egui::{
    bevy_egui::{EguiGlobalSettings, EguiPlugin, EguiPrimaryContextPass},
    DefaultInspectorConfigPlugin,
};
use clap::Parser;
use constants::{TEXTURE_PATH_BASE, TEXTURE_PATH_CUSTOM};
use shared::get_game_folder_paths;
use std::path::{Path, PathBuf};
#[cfg(debug_assertions)]
use ui::hud::debug::{debug_overlay_shown, inspector::inspector_ui};
use ui::menus;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Flag to use custom textures
    #[arg(long, help = "Use custom textures instead of base textures")]
    use_custom_textures: bool,

    #[arg(short, long)]
    game_folder_path: Option<String>,

    #[arg(
        short,
        long,
        help = "Allows overriding of the asset folder path, defaults to the data folder shipped with the game"
    )]
    assets_folder_path: Option<String>,

    #[arg(
        short,
        long,
        help = "Player name to use for the game, instead of the one saved in player.ron"
    )]
    player_name: Option<String>,
}

fn main() {
    let args = Args::parse();
    // the blocks come from the base mod: a broken one stops the game here, saying why
    if let Err(err) = shared::world::load_blocks() {
        eprintln!("The blocks of the base mod do not read: {err}");
        std::process::exit(1);
    }
    let texture_path = if args.use_custom_textures {
        TEXTURE_PATH_CUSTOM
    } else {
        TEXTURE_PATH_BASE
    };
    let explicit_assets_folder = args.assets_folder_path.is_some();
    let mut game_folder_paths =
        get_game_folder_paths(args.game_folder_path, args.assets_folder_path);
    if !explicit_assets_folder {
        if let Some(folder) = bundled_assets_folder(&FileAssetReader::get_base_path()) {
            game_folder_paths.assets_folder_path = folder;
        }
    }

    let mut app = App::new();
    app.add_plugins(engine_plugins());

    // Logging is set up by the engine plugins above, so these lines come after them
    info!("Using {texture_path} for textures");
    info!(
        "Starting application with game folder: {}",
        game_folder_paths.game_folder_path.display()
    );
    info!(
        "Reading the assets from: {}",
        game_folder_paths.assets_folder_path.display()
    );

    #[cfg(debug_assertions)]
    add_debug_inspector(&mut app);
    app::add_client(
        &mut app,
        ClientSettings {
            game_folder_paths,
            texture_path: texture_path.to_string(),
            player_name: args.player_name,
        },
    );
    app.run();
}

/// The folder of the assets shipped with the game, relative to Bevy's base path: next to
/// `bin/` in a release, at the root of the repository under `cargo run`.
const DATA_FOLDER: &str = "../data";

/// The assets shipped with the game, when they are there. The textures are then read from
/// the folder Bevy loads the fonts and the sounds from, wherever the game was unpacked.
fn bundled_assets_folder(base_path: &Path) -> Option<PathBuf> {
    let folder = base_path.join(DATA_FOLDER);
    folder.is_dir().then_some(folder)
}

/// Bevy's default plugins, set up for a pixel-art game in a native window.
fn engine_plugins() -> PluginGroupBuilder {
    DefaultPlugins
        .set(app::task_pool_plugin())
        // Ensures that pixel-art textures will remain pixelated, and not become a blurry mess
        .set(ImagePlugin::default_nearest())
        .set(RenderPlugin {
            render_creation: RenderCreation::Automatic(Box::new(WgpuSettings {
                // Drawing wireframes, for the block debug mode. Native only: it does not
                // exist in WebGL or WebGPU.
                features: WgpuFeatures::POLYGON_MODE_LINE,
                ..default()
            })),
            ..default()
        })
        .set(AssetPlugin {
            file_path: DATA_FOLDER.to_string(),
            // The textures are loaded by absolute path from the assets folder, which the
            // command line can place anywhere: paths outside `file_path` must be allowed.
            unapproved_path_mode: bevy::asset::UnapprovedPathMode::Allow,
            ..default()
        })
        .set(LogPlugin {
            // The warnings come from screens without tab groups, see the redesign lots
            filter: format!("{},bevy_input_focus::tab_navigation=error", DEFAULT_FILTER),
            ..default()
        })
        .set(WindowPlugin {
            primary_window: Some(Window {
                title: "Rustcraft".to_string(),
                present_mode: PresentMode::AutoVsync,
                ..default()
            }),
            // the close button asks to quit: a world being played is saved first
            close_when_requested: false,
            ..default()
        })
}

/// The egui window listing the entities and the resources of the app. It follows the debug
/// overlay (F3) and is left out of release builds.
#[cfg(debug_assertions)]
fn add_debug_inspector(app: &mut App) {
    // The menu and game cameras carry `PrimaryEguiContext` themselves: the automatic context
    // would stay on the first camera ever created, which is the menu one.
    app.insert_resource(EguiGlobalSettings {
        auto_create_primary_context: false,
        ..default()
    })
    .add_plugins(EguiPlugin::default())
    .add_plugins(DefaultInspectorConfigPlugin)
    .add_systems(
        EguiPrimaryContextPass,
        inspector_ui.run_if(debug_overlay_shown),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_data_folder_next_to_the_binaries_is_used() {
        let release = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(release.path().join("bin")).unwrap();
        std::fs::create_dir_all(release.path().join("data")).unwrap();

        let folder = bundled_assets_folder(&release.path().join("bin")).unwrap();
        assert_eq!(
            folder.canonicalize().unwrap(),
            release.path().join("data").canonicalize().unwrap()
        );
    }

    #[test]
    fn without_a_data_folder_nothing_is_bundled() {
        let release = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(release.path().join("bin")).unwrap();

        assert_eq!(bundled_assets_folder(&release.path().join("bin")), None);
    }
}
