mod camera;
mod constants;
mod entities;
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
};
use bevy_inspector_egui::quick::WorldInspectorPlugin;
use clap::Parser;
use constants::{TEXTURE_PATH_BASE, TEXTURE_PATH_CUSTOM};
use input::{data::GameAction, keyboard::get_bindings};
use menus::solo::SelectedWorld;
use serde::{Deserialize, Serialize};
use shared::{GameFolderPaths, SpecialFlag};
use std::collections::BTreeMap;
use ui::menus::{self, splash};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Flag to use custom textures
    #[arg(long, help = "Use custom textures instead of base textures")]
    use_custom_textures: bool,

    #[arg(short, long, default_value = "../")]
    game_folder_path: String,

    #[arg(
        short,
        long,
        help = "Allows overriding of the asset folder path, defaults to <game_folder_path>/data"
    )]
    assets_folder_path: Option<String>,

    #[arg(long)]
    special_flag: bool,

    #[arg(short, long, help = "Player name to use for the game")]
    player_name: Option<String>,
}

#[derive(Component)]
pub struct MenuCamera;

pub const TEXT_COLOR: Color = Color::srgb(0.9, 0.9, 0.9);

// Enum that will be used as a global state for the game
#[derive(Clone, Copy, Default, Eq, PartialEq, Debug, Hash, States)]
pub enum GameState {
    Splash,
    #[default]
    Menu,
    PreGameLoading,
    Game,
}

#[derive(Event)]
pub struct LoadWorldEvent {
    pub world_name: String,
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

    let game_folder_path = args.game_folder_path.clone();
    let special_flag = args.special_flag;

    println!(
        "Using {} for textures",
        if args.use_custom_textures {
            "custom textures"
        } else {
            "base textures"
        }
    );

    println!(
        "Starting application with game folder: {}",
        game_folder_path
    );

    let assets_folder_path = args
        .assets_folder_path
        .clone()
        .unwrap_or(format!("{}/data", game_folder_path));

    println!(
        "Starting application with assets folder: {:?}",
        assets_folder_path
    );

    let game_folder_paths = GameFolderPaths {
        game_folder_path: game_folder_path.clone(),
        assets_folder_path,
    };

    let special_flag = SpecialFlag { special_flag };

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
                ..Default::default()
            }),
    );
    app.add_plugins(WorldInspectorPlugin::new());

    app.add_event::<LoadWorldEvent>();
    network::add_base_netcode(&mut app);
    app.insert_resource(get_bindings(&game_folder_path.clone()))
        .insert_resource(SelectedWorld::default())
        // Declare the game state, whose starting value is determined by the `Default` trait
        .insert_resource(ClientWorldMap { ..default() })
        .insert_resource(TexturePath {
            path: texture_path.to_string(),
        })
        .insert_resource(game_folder_paths)
        .insert_resource(special_flag)
        .insert_resource(PlayerNameSupplied {
            name: args.player_name.unwrap_or_else(|| "Player".to_string()),
        })
        .init_state::<GameState>()
        .enable_state_scoped_entities::<GameState>()
        // Adds the plugins for each state
        .add_plugins((splash::splash_plugin, menus::menu_plugin, game::game_plugin))
        .run();
}
