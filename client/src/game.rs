use std::collections::HashMap;

use bevy::prelude::*;
use shared::world::ChunkMap;
use shared::TICKS_PER_SECOND;

use crate::camera::camera_plugin;
use crate::input::input_plugin;
use crate::mob::mob_plugin;
use crate::network::{
    init_server_connection, launch_local_server_system, network_plugin,
    terminate_server_connection, TargetServer, TargetServerState,
};
use crate::player::player_plugin;
use crate::state::{GameSet, GameState, PreLoadingCompletion};
use crate::ui::hud::hud_plugin;
use crate::ui::menus::setup_server_connect_loading_screen;
use crate::world::{setup_materials, world_plugin, ClientWorldMap};

/// Assembles the domain plugins, and holds what spans several of them: the session lifecycle
/// (entering and leaving the game).
pub fn game_plugin(app: &mut App) {
    app.add_plugins(bevy_simple_text_input::TextInputPlugin)
        .add_plugins((
            camera_plugin,
            hud_plugin,
            input_plugin,
            mob_plugin,
            network_plugin,
            player_plugin,
            world_plugin,
        ))
        // The order of one in-game frame. `GameSet::Meshing` is in `PostUpdate`.
        .configure_sets(
            Update,
            (
                GameSet::UiMode,
                GameSet::Input,
                GameSet::Simulation,
                GameSet::Reconciliation,
                GameSet::Camera,
            )
                .chain(),
        )
        .insert_resource(PreLoadingCompletion {
            textures_loaded: false,
        })
        .insert_resource(Time::<Fixed>::from_hz(TICKS_PER_SECOND as f64))
        .add_systems(
            OnEnter(GameState::PreGameLoading),
            (
                launch_local_server_system,
                init_server_connection,
                setup_materials,
                setup_server_connect_loading_screen,
            )
                .chain(),
        )
        .add_systems(
            Update,
            check_pre_loading_complete.run_if(in_state(GameState::PreGameLoading)),
        )
        .add_systems(
            OnExit(GameState::Game),
            (clear_resources, terminate_server_connection).chain(),
        )
        // Cancelling the loading screen never enters the game.
        .add_systems(
            OnTransition {
                exited: GameState::PreGameLoading,
                entered: GameState::Menu,
            },
            terminate_server_connection,
        );
}

fn clear_resources(mut world_map: ResMut<ClientWorldMap>) {
    world_map.blocks = ChunkMap::default();
    world_map.render = HashMap::new();
}

fn check_pre_loading_complete(
    loading: Res<PreLoadingCompletion>,
    mut game_state: ResMut<NextState<GameState>>,
    target_server: Res<TargetServer>,
) {
    if loading.textures_loaded && target_server.state == TargetServerState::FullyReady {
        game_state.set(GameState::Game);
    }
}
