use std::collections::HashMap;

use crate::entities::stack::stack_update_system;
use crate::mob::*;
use crate::network::buffered_client::{CurrentFrameInputs, PlayerTickInputsBuffer, SyncTime};
use crate::ui::hud::chat::{render_chat, setup_chat};
use crate::ui::menus::{setup_server_connect_loading_screen, update_server_connect_loading_screen};
use bevy::prelude::*;
use bevy_atmosphere::prelude::*;
use shared::messages::mob::MobUpdateEvent;
use shared::messages::{ItemStackUpdateEvent, PlayerSpawnEvent, PlayerUpdateEvent};
use shared::players::{Inventory, ViewMode};
use shared::TICKS_PER_SECOND;
use time::time_update_system;

use crate::world::time::ClientTime;
use crate::world::ClientWorldMap;

use crate::ui::hud::debug::BlockDebugWireframeSettings;
use crate::ui::hud::reticle::spawn_reticle;
use crate::ui::menus::pause::{render_pause_menu, setup_pause_menu};
use bevy::color::palettes::basic::WHITE;
use bevy::diagnostic::FrameTimeDiagnosticsPlugin;
use bevy::pbr::wireframe::{WireframeConfig, WireframePlugin};

use crate::ui::hud::debug::targeted_block::block_text_update_system;
use crate::world::celestial::setup_main_lighting;

use crate::ui::hud::debug::*;
use crate::ui::hud::hotbar::*;
use crate::ui::hud::set_ui_mode;
use crate::world::celestial::*;
use crate::world::*;

use crate::camera::*;
use crate::input::*;
use crate::player::*;
use crate::ui::hud::inventory::*;
use shared::world::{BlockId, ItemId, WorldSeed};

use crate::network::{
    establish_authenticated_connection_to_server, init_server_connection,
    launch_local_server_system, network_failure_handler, poll_network_messages,
    terminate_server_connection, upload_player_inputs_system, CurrentPlayerProfile, TargetServer,
    TargetServerState, UnacknowledgedInputs,
};

use crate::GameState;

#[derive(Resource)]
pub struct PreLoadingCompletion {
    pub textures_loaded: bool,
}

pub fn game_plugin(app: &mut App) {
    app.add_plugins(FrameTimeDiagnosticsPlugin::default())
        .add_plugins(WireframePlugin::default())
        .add_plugins(bevy_simple_text_input::TextInputPlugin)
        .add_plugins(AtmospherePlugin)
        .insert_resource(WorldSeed(0))
        .insert_resource(ClientTime(0))
        .insert_resource(FirstChunkReceived(false))
        .insert_resource(AmbientLight {
            color: Color::WHITE,
            brightness: 400.0,
            ..default()
        })
        .insert_resource(PreLoadingCompletion {
            textures_loaded: false,
        })
        .insert_resource(BlockDebugWireframeSettings { is_enabled: false })
        .insert_resource(WireframeConfig {
            // The global wireframe config enables drawing of wireframes on every mesh,
            // except those with `NoWireframe`. Meshes with `Wireframe` will always have a wireframe,
            // regardless of the global configuration.
            global: false,
            // Controls the default color of all wireframes. Used as the default color for global wireframes.
            // Can be changed per mesh using the `WireframeColor` component.
            default_color: WHITE.into(),
        })
        .insert_resource(MaterialResource { ..default() })
        .insert_resource(AtlasHandles::<BlockId>::default())
        .insert_resource(AtlasHandles::<ItemId>::default())
        .insert_resource(RenderDistance { ..default() })
        .insert_resource(UIMode::Closed)
        .insert_resource(ViewMode::FirstPerson)
        .insert_resource(DebugOptions::default())
        .insert_resource(Inventory::new())
        .init_resource::<CurrentPlayerProfile>()
        .init_resource::<ParticleAssets>()
        .init_resource::<FoxFeetTargets>()
        .init_resource::<Animations>()
        .init_resource::<TargetedMob>()
        .init_resource::<PlayerTickInputsBuffer>()
        .init_resource::<CurrentFrameInputs>()
        .init_resource::<SyncTime>()
        .init_resource::<UnacknowledgedInputs>()
        .insert_resource(Time::<Fixed>::from_hz(TICKS_PER_SECOND as f64))
        .add_event::<WorldRenderRequestUpdateEvent>()
        .add_event::<PlayerSpawnEvent>()
        .add_event::<PlayerUpdateEvent>()
        .add_event::<MobUpdateEvent>()
        .add_event::<ItemStackUpdateEvent>()
        .add_systems(
            OnEnter(GameState::PreGameLoading),
            (
                launch_local_server_system,
                init_server_connection,
                setup_materials,
                setup_server_connect_loading_screen,
                spawn_camera,
            )
                .chain(),
        )
        .add_systems(
            Update,
            (
                establish_authenticated_connection_to_server,
                create_all_atlases,
                check_pre_loading_complete,
                spawn_players_system,
                update_server_connect_loading_screen,
            )
                .run_if(in_state(GameState::PreGameLoading)),
        )
        .add_systems(
            OnEnter(GameState::Game),
            (
                setup_main_lighting,
                spawn_reticle,
                setup_hud,
                setup_chat,
                setup_pause_menu,
            )
                .chain(),
        )
        .add_systems(
            OnEnter(GameState::Game),
            (setup_hotbar, setup_inventory).chain(),
        )
        .add_systems(OnEnter(GameState::Game), setup_chunk_ghost)
        .add_systems(
            Update,
            (
                render_pause_menu,
                render_chat,
                render_inventory_hotbar,
                set_ui_mode,
            )
                .run_if(in_state(GameState::Game)),
        )
        .add_systems(
            Update,
            (
                render_distance_update_system,
                first_and_third_person_view_system,
                toggle_chunk_debug_mode_system,
                toggle_raycast_debug_mode_system,
                chunk_force_reload_system,
                (player_movement_system, camera_control_system).chain(),
                fps_text_update_system,
                coords_text_update_system,
                total_blocks_text_update_system,
                block_text_update_system,
                time_text_update_system,
                toggle_hud_system,
                chunk_ghost_update_system,
                raycast_debug_update_system,
                toggle_wireframe_system,
                handle_mouse_system,
                update_celestial_bodies,
            )
                .run_if(in_state(GameState::Game)),
        )
        .add_systems(
            Update,
            (
                setup_fox_once_loaded,
                simulate_particles,
                update_targetted_mob_color,
                stack_update_system,
            )
                .run_if(in_state(GameState::Game)),
        )
        .add_observer(observe_on_step)
        .add_systems(
            PostUpdate,
            (world_render_system).run_if(in_state(GameState::Game)),
        )
        .add_systems(
            Update,
            (
                network_failure_handler,
                spawn_players_system,
                update_players_system,
                spawn_mobs_system,
                player_labels_system,
            )
                .run_if(in_state(GameState::Game)),
        )
        .add_systems(
            PreUpdate,
            pre_input_update_system.run_if(in_state(GameState::Game)),
        )
        .add_systems(
            FixedPreUpdate,
            poll_network_messages.run_if(in_state(GameState::Game)),
        )
        .add_systems(
            FixedUpdate,
            (upload_player_inputs_system, handle_block_interactions)
                .run_if(in_state(GameState::Game)),
        )
        .add_systems(
            FixedPostUpdate,
            time_update_system.run_if(in_state(GameState::Game)),
        )
        .add_systems(
            OnExit(GameState::Game),
            (clear_resources, terminate_server_connection).chain(),
        );
}

fn clear_resources(mut world_map: ResMut<ClientWorldMap>) {
    world_map.map = HashMap::new();
    world_map.total_blocks_count = 0;
    world_map.total_chunks_count = 0;
    world_map.name = "".into();
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
