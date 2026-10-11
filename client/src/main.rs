mod camera;
mod constants;
mod game;
mod input;
mod mob;
mod network;
mod player;
mod ui;
mod world;

use crate::world::ClientWorldMap;
use bevy::{
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
use input::{data::GameAction, keyboard::get_bindings};
use menus::solo::SelectedWorld;
use serde::{Deserialize, Serialize};
use shared::get_game_folder_paths;
use std::collections::BTreeMap;
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

#[derive(Component)]
pub struct MenuCamera;

pub const TEXT_COLOR: Color = Color::srgb(0.9, 0.9, 0.9);

// Enum that will be used as a global state for the game
#[derive(Clone, Copy, Default, Eq, PartialEq, Debug, Hash, States)]
pub enum GameState {
    #[default]
    Menu,
    PreGameLoading,
    Game,
}

#[derive(Resource, Serialize, Deserialize)]
pub struct KeyMap {
    pub map: BTreeMap<GameAction, Vec<KeyCode>>,
}

#[derive(Resource, Debug)]
pub struct TexturePath {
    pub path: String,
}

#[derive(Resource, Debug)]
pub struct PlayerNameSupplied {
    pub name: String,
}

fn main() {
    // Parse command-line arguments
    let args = Args::parse();

    // Determine which texture path to use
    let texture_path = if args.use_custom_textures {
        TEXTURE_PATH_CUSTOM
    } else {
        TEXTURE_PATH_BASE
    };

    let game_folder_paths = get_game_folder_paths(args.game_folder_path, args.assets_folder_path);

    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins
            // Ensures that pixel-art textures will remain pixelated, and not become a blurry mess
            .set(ImagePlugin::default_nearest())
            .set(RenderPlugin {
                render_creation: RenderCreation::Automatic(WgpuSettings {
                    // WARNING: This is a native-only feature. It will not work with WebGL or WebGPU
                    features: WgpuFeatures::POLYGON_MODE_LINE,
                    ..default()
                }),
                ..default()
            })
            .set(AssetPlugin {
                file_path: "../data".to_string(),
                // TODO: Remove unapproved_path_mode once the asset loading system has been improved
                unapproved_path_mode: bevy::asset::UnapprovedPathMode::Allow,
                ..Default::default()
            })
            .set(WindowPlugin {
                primary_window: Some(Window {
                    title: "Rustcraft".to_string(),
                    present_mode: PresentMode::AutoVsync,
                    ..default()
                }),
                ..default()
            }),
    );

    // Logging is set up by the DefaultPlugins above, so these lines come after them
    info!(
        "Using {} for textures",
        if args.use_custom_textures {
            "custom textures"
        } else {
            "base textures"
        }
    );
    info!(
        "Starting application with game folder: {}",
        game_folder_paths.game_folder_path.display()
    );

    // The menu and game cameras carry `PrimaryEguiContext` themselves: the automatic context
    // would stay on the first camera ever created, which is the menu one.
    app.insert_resource(EguiGlobalSettings {
        auto_create_primary_context: false,
        ..default()
    })
    .add_plugins(EguiPlugin::default())
    .add_plugins(DefaultInspectorConfigPlugin)
    .add_systems(EguiPrimaryContextPass, inspector_ui);

    network::add_base_netcode(&mut app);
    app.insert_resource(get_bindings(&game_folder_paths))
        .insert_resource(SelectedWorld::default())
        // Declare the game state, whose starting value is determined by the `Default` trait
        .insert_resource(ClientWorldMap { ..default() })
        .insert_resource(TexturePath {
            path: texture_path.to_string(),
        })
        .insert_resource(game_folder_paths)
        .insert_resource(PlayerNameSupplied {
            name: args.player_name.unwrap_or_else(|| "Player".to_string()),
        })
        .init_state::<GameState>()
        // Adds the plugins for each state
        .add_plugins((menus::menu_plugin, game::game_plugin))
        .run();
}
