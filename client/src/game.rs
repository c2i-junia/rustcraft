use std::collections::HashMap;

use bevy::prelude::*;
use shared::world::ChunkMap;
use shared::TICKS_PER_SECOND;

use crate::camera::camera_plugin;
use crate::dropped_items::dropped_items_plugin;
use crate::input::input_plugin;
use crate::mob::mob_plugin;
use crate::network::session::{
    player_name_accepted, refuse_an_invalid_name, track_connection, ConnectStep, ConnectWatch,
};
use crate::network::{
    init_server_connection, launch_local_server_system, network_plugin,
    terminate_server_connection, DecodingChunkMessages, LastWorldClosing, TargetServer,
    TargetServerState,
};
use crate::player::player_plugin;
use crate::state::{GameSet, GameState, PreLoadingCompletion};
use crate::ui::hud::hud_plugin;
use crate::ui::menus::connecting::setup_connecting_card;
use crate::world::{setup_materials, world_plugin, ClientWorldMap};

/// Assembles the domain plugins, and holds what spans several of them: the session lifecycle
/// (entering and leaving the game).
pub fn game_plugin(app: &mut App) {
    app.add_plugins((
        camera_plugin,
        dropped_items_plugin,
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
            // a name no server accepts ends the session before anything starts
            refuse_an_invalid_name,
            launch_local_server_system.run_if(player_name_accepted),
            init_server_connection.run_if(player_name_accepted),
            setup_materials,
            setup_connecting_card,
        )
            .chain(),
    )
    // a world waiting for the last one to close is launched, then connected to, once it closed
    .add_systems(
        Update,
        (launch_local_server_system, init_server_connection)
            .chain()
            .before(track_connection)
            .run_if(in_state(GameState::PreGameLoading))
            .run_if(resource_exists::<LastWorldClosing>),
    )
    .add_systems(
        OnExit(GameState::PreGameLoading),
        |mut commands: Commands| {
            commands.remove_resource::<LastWorldClosing>();
        },
    )
    .add_systems(
        Update,
        // the game is entered from a step tracked in an earlier frame, which a cancel can still
        // end
        check_pre_loading_complete
            .before(track_connection)
            .run_if(in_state(GameState::PreGameLoading)),
    );
    add_session_exits(app);
}

/// The ways out of a session: leaving the game, and cancelling the loading screen, which never
/// enters it. Both forget the world received and end the connection.
pub(crate) fn add_session_exits(app: &mut App) {
    app.add_systems(
        OnExit(GameState::Game),
        (clear_resources, terminate_server_connection).chain(),
    )
    .add_systems(
        OnTransition {
            exited: GameState::PreGameLoading,
            entered: GameState::Menu,
        },
        (clear_resources, terminate_server_connection).chain(),
    );
}

fn clear_resources(
    mut world_map: ResMut<ClientWorldMap>,
    mut decoding: ResMut<DecodingChunkMessages>,
) {
    world_map.blocks = ChunkMap::default();
    world_map.render = HashMap::new();
    decoding.clear();
}

/// Enters the game once the textures are loaded, the player spawned and the chunks around it
/// received, unless the session ended in this frame (a cancel): the first end of a session wins.
fn check_pre_loading_complete(
    loading: Res<PreLoadingCompletion>,
    mut game_state: ResMut<NextState<GameState>>,
    target_server: Res<TargetServer>,
    watch: Option<Res<ConnectWatch>>,
) {
    if !matches!(*game_state, NextState::Unchanged) {
        return;
    }
    let spawning = watch.is_some_and(|watch| watch.step == ConnectStep::Spawning);
    if loading.textures_loaded && target_server.state == TargetServerState::FullyReady && spawning {
        game_state.set(GameState::Game);
    }
}

#[cfg(test)]
mod tests {
    use bevy::state::app::StatesPlugin;

    use shared::world::Chunk;

    use super::*;
    use crate::network::buffered_client::PlayerTickInputsBuffer;
    use crate::network::session::{end_session, SessionEnd, SessionOrigin};
    use crate::network::UnacknowledgedInputs;
    use crate::world::ChunkRender;

    #[test]
    fn a_cancel_in_the_frame_loading_completes_keeps_the_game_out() {
        let mut app = App::new();
        app.add_plugins(StatesPlugin)
            .init_state::<GameState>()
            .insert_resource(PreLoadingCompletion {
                textures_loaded: true,
            })
            .insert_resource(TargetServer {
                address: None,
                username: None,
                session_token: None,
                state: TargetServerState::FullyReady,
            })
            .insert_resource(ConnectWatch::at(ConnectStep::Spawning))
            .add_systems(
                Update,
                (
                    |mut commands: Commands, mut game: ResMut<NextState<GameState>>| {
                        let origin = SessionOrigin::World {
                            name: "Survie".into(),
                        };
                        end_session(&mut commands, &mut game, &origin, None);
                    },
                    check_pre_loading_complete,
                )
                    .chain(),
            );
        app.update();
        assert!(matches!(
            app.world().resource::<NextState<GameState>>(),
            NextState::Pending(GameState::Menu)
        ));
        assert!(app.world().contains_resource::<SessionEnd>());
    }

    #[test]
    fn cancelling_the_loading_screen_forgets_the_chunks_it_received() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, StatesPlugin))
            .insert_state(GameState::PreGameLoading)
            .insert_resource(TargetServer {
                address: None,
                username: None,
                session_token: None,
                state: TargetServerState::FullyReady,
            })
            .init_resource::<UnacknowledgedInputs>()
            .init_resource::<PlayerTickInputsBuffer>()
            .init_resource::<ClientWorldMap>()
            .init_resource::<DecodingChunkMessages>();
        add_session_exits(&mut app);
        let mut map = app.world_mut().resource_mut::<ClientWorldMap>();
        map.blocks.insert_chunk(IVec3::ZERO, Chunk::new());
        map.render.insert(IVec3::ZERO, ChunkRender::new());
        app.update();

        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Menu);
        app.update();

        let map = app.world().resource::<ClientWorldMap>();
        assert_eq!(map.blocks.chunk_positions().count(), 0);
        assert!(map.render.is_empty());
    }
}
