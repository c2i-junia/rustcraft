mod app;
mod audio;
mod camera;
mod constants;
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
    log::{LogPlugin, DEFAULT_FILTER},
    prelude::*,
    render::{
        settings::{RenderCreation, WgpuFeatures, WgpuSettings},
        RenderPlugin,
    },
    window::PresentMode,
};
use bevy_inspector_egui::{
    bevy_egui::{EguiGlobalSettings, EguiPlugin, EguiPrimaryContextPass},
    DefaultInspectorConfigPlugin,
};
use clap::Parser;
use constants::{TEXTURE_PATH_BASE, TEXTURE_PATH_CUSTOM};
use shared::get_game_folder_paths;
use ui::{hud::debug::inspector::inspector_ui, menus};

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
        help = "Allows overriding of the asset folder path, defaults to <game_folder_path>/data"
    )]
    assets_folder_path: Option<String>,

    #[arg(short, long, help = "Player name to use for the game")]
    player_name: Option<String>,
}

fn main() {
    let args = Args::parse();
    let texture_path = if args.use_custom_textures {
        TEXTURE_PATH_CUSTOM
    } else {
        TEXTURE_PATH_BASE
    };
    let game_folder_paths = get_game_folder_paths(args.game_folder_path, args.assets_folder_path);

    let mut app = App::new();
    app.add_plugins(engine_plugins());

    // Logging is set up by the engine plugins above, so these lines come after them
    info!("Using {texture_path} for textures");
    info!(
        "Starting application with game folder: {}",
        game_folder_paths.game_folder_path.display()
    );

    add_debug_inspector(&mut app);
    app::add_client(
        &mut app,
        ClientSettings {
            game_folder_paths,
            texture_path: texture_path.to_string(),
            player_name: args.player_name.unwrap_or_else(|| "Player".to_string()),
        },
    );
    app.run();
}

/// Bevy's default plugins, set up for a pixel-art game in a native window.
fn engine_plugins() -> PluginGroupBuilder {
    DefaultPlugins
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
            file_path: "../data".to_string(),
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
            ..default()
        })
}

/// The egui window listing the entities and the resources of the app.
fn add_debug_inspector(app: &mut App) {
    // The menu and game cameras carry `PrimaryEguiContext` themselves: the automatic context
    // would stay on the first camera ever created, which is the menu one.
    app.insert_resource(EguiGlobalSettings {
        auto_create_primary_context: false,
        ..default()
    })
    .add_plugins(EguiPlugin::default())
    .add_plugins(DefaultInspectorConfigPlugin)
    .add_systems(EguiPrimaryContextPass, inspector_ui);
}
