//! The client app, built in one place for the game and for the end-to-end tests.

use bevy::prelude::*;
use shared::GameFolderPaths;

use crate::audio::{self, settings::load_audio_settings};
use crate::game;
use crate::input::keyboard::get_bindings;
use crate::network;
use crate::state::{GameState, PlayerNameSupplied, TexturePath};
use crate::ui::menus::{self, solo::SelectedWorld};
use crate::world::ClientWorldMap;

/// What distinguishes one run of the client from another.
pub struct ClientSettings {
    pub game_folder_paths: GameFolderPaths,
    pub texture_path: String,
    pub player_name: String,
}

/// Adds the game to an app that already has the engine plugins: the netcode, the resources, and
/// the plugins of the menus, the game and the sounds.
pub fn add_client(app: &mut App, settings: ClientSettings) {
    let ClientSettings {
        game_folder_paths,
        texture_path,
        player_name,
    } = settings;
    network::add_base_netcode(app);
    app.insert_resource(get_bindings(&game_folder_paths))
        .insert_resource(load_audio_settings(&game_folder_paths))
        .insert_resource(SelectedWorld::default())
        .insert_resource(ClientWorldMap { ..default() })
        .insert_resource(TexturePath { path: texture_path })
        .insert_resource(game_folder_paths)
        .insert_resource(PlayerNameSupplied { name: player_name })
        // The game starts in the default state, the menu
        .init_state::<GameState>()
        .add_plugins((menus::menu_plugin, game::game_plugin, audio::audio_plugin));
}
