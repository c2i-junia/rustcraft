//! The client app, built in one place for the game and for the end-to-end tests.

use bevy::app::{TaskPoolOptions, TaskPoolThreadAssignmentPolicy};
use bevy::prelude::*;
use shared::GameFolderPaths;

use crate::audio::{self, settings::load_audio_settings};
use crate::game;
use crate::input::keyboard::get_bindings;
use crate::network;
use crate::state::{GameState, PlayerNameSupplied, TexturePath};
use crate::ui::menus::player_name::{load_player_name, AskPlayerName, DEFAULT_PLAYER_NAME};
use crate::ui::menus::{self, solo::SelectedWorld};
use crate::ui::quit::quit_plugin;
use crate::ui::settings::load_interface_settings;
use crate::ui::widgets::widgets_plugin;
use crate::world::load_video_settings;
use crate::world::ClientWorldMap;

/// The threads of the task pools. The chunks are meshed on the async compute pool, which
/// also compiles the pipelines in the background: Bevy gives it at most 4 threads, where
/// meshing the chunks of a world that opens keeps many more busy.
pub fn task_pool_plugin() -> TaskPoolPlugin {
    let mut options = TaskPoolOptions::default();
    options.async_compute = TaskPoolThreadAssignmentPolicy {
        max_threads: 8,
        percent: 0.25,
        ..options.async_compute
    };
    TaskPoolPlugin {
        task_pool_options: options,
    }
}

/// What distinguishes one run of the client from another.
pub struct ClientSettings {
    pub game_folder_paths: GameFolderPaths,
    pub texture_path: String,
    /// Overrides the saved name; with neither, the main menu asks for one.
    pub player_name: Option<String>,
}

/// Adds the game to an app that already has the engine plugins: the netcode, the resources, and
/// the plugins of the widgets, the menus, the game and the sounds.
pub fn add_client(app: &mut App, settings: ClientSettings) {
    let ClientSettings {
        game_folder_paths,
        texture_path,
        player_name,
    } = settings;
    let player_name = match player_name.or_else(|| load_player_name(&game_folder_paths)) {
        Some(name) => name,
        None => {
            app.insert_resource(AskPlayerName);
            DEFAULT_PLAYER_NAME.to_string()
        }
    };
    network::add_base_netcode(app);
    let (render_distance, clouds) = load_video_settings(&game_folder_paths);
    app.insert_resource(get_bindings(&game_folder_paths))
        .insert_resource(load_audio_settings(&game_folder_paths))
        .insert_resource(load_interface_settings(&game_folder_paths))
        .insert_resource(render_distance)
        .insert_resource(clouds)
        .insert_resource(SelectedWorld::default())
        .insert_resource(ClientWorldMap { ..default() })
        .insert_resource(TexturePath { path: texture_path })
        .insert_resource(game_folder_paths)
        .insert_resource(PlayerNameSupplied { name: player_name })
        // The game starts in the default state, the menu
        .init_state::<GameState>()
        .add_plugins((
            widgets_plugin,
            menus::menu_plugin,
            game::game_plugin,
            audio::audio_plugin,
            quit_plugin,
        ));
}
