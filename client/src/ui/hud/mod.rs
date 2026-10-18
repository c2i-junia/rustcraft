use bevy::color::palettes::basic::WHITE;
use bevy::diagnostic::FrameTimeDiagnosticsPlugin;
use bevy::pbr::wireframe::{WireframeConfig, WireframePlugin};
use bevy::prelude::*;

use crate::state::{GameSet, GameState};
use crate::ui::hud::chat::*;
use crate::ui::hud::debug::targeted_block::block_text_update_system;
use crate::ui::hud::debug::*;
use crate::ui::hud::hotbar::*;
use crate::ui::hud::reticle::spawn_reticle;
use crate::ui::menus::pause::{render_pause_menu, setup_pause_menu};

pub mod chat;
pub mod debug;
pub mod hotbar;
pub mod inventory;
pub mod reticle;
mod ui_mode;

pub use inventory::*;
pub use ui_mode::*;

/// Everything drawn over the game: reticle, hotbar, inventory, chat, pause menu and the debug
/// overlays.
pub fn hud_plugin(app: &mut App) {
    app.add_plugins(FrameTimeDiagnosticsPlugin::default())
        .add_plugins(WireframePlugin::default())
        .insert_resource(BlockDebugWireframeSettings { is_enabled: false })
        .insert_resource(WireframeConfig {
            // The global wireframe config enables drawing of wireframes on every mesh,
            // except those with `NoWireframe`. Meshes with `Wireframe` will always have a wireframe,
            // regardless of the global configuration.
            global: false,
            // Controls the default color of all wireframes. Used as the default color for global wireframes.
            // Can be changed per mesh using the `WireframeColor` component.
            default_color: WHITE.into(),
            ..default()
        })
        .insert_resource(UIMode::Closed)
        .insert_resource(DebugOptions::default())
        .add_systems(
            OnEnter(GameState::Game),
            (spawn_reticle, setup_hud, setup_chat, setup_pause_menu).chain(),
        )
        .add_systems(
            OnEnter(GameState::Game),
            (setup_hotbar, setup_inventory).chain(),
        )
        .add_systems(OnEnter(GameState::Game), setup_chunk_ghost)
        .add_systems(
            Update,
            set_ui_mode
                .in_set(GameSet::UiMode)
                .run_if(in_state(GameState::Game)),
        )
        .add_systems(
            Update,
            (
                render_pause_menu,
                (
                    toggle_chat_system,
                    fade_chat_messages_system,
                    display_chat_messages_system,
                    send_chat_message_system,
                )
                    .chain(),
                // Before the simulation, which predicts the clicks of the frame
                (
                    toggle_inventory_system,
                    close_inventory_system,
                    select_hotbar_slot_system,
                    click_inventory_cells_system,
                    display_inventory_system,
                )
                    .chain()
                    .before(GameSet::Simulation),
            )
                .after(GameSet::UiMode)
                .run_if(in_state(GameState::Game)),
        )
        .add_systems(
            Update,
            (
                toggle_chunk_debug_mode_system,
                toggle_raycast_debug_mode_system,
                fps_text_update_system,
                coords_text_update_system,
                loaded_chunks_text_update_system,
                block_text_update_system,
                time_text_update_system,
                toggle_hud_system,
                chunk_ghost_update_system,
                raycast_debug_update_system,
                toggle_wireframe_system,
            )
                .run_if(in_state(GameState::Game)),
        );
}
