use bevy::diagnostic::FrameTimeDiagnosticsPlugin;
use bevy::pbr::wireframe::{WireframeConfig, WireframePlugin};
use bevy::prelude::*;

use crate::state::{GameSet, GameState};
use crate::ui::hud::chat::*;
use crate::ui::hud::chat_completion::chat_completion_plugin;
use crate::ui::hud::corner::spawn_top_right_column;
use crate::ui::hud::debug::targeted_block::block_text_update_system;
use crate::ui::hud::debug::*;
use crate::ui::hud::fps_counter::{
    cycle_fps_counter_system, draw_fps_graph, record_frame_time, reset_frame_times,
    show_fps_counter, spawn_fps_counter, FrameTimes,
};
use crate::ui::hud::hotbar::{
    select_hotbar_slot_with_keys, select_hotbar_slot_with_wheel, show_selected_item_name,
};
use crate::ui::hud::minimap::{minimap_plugin, spawn_minimap};
use crate::ui::hud::reticle::spawn_reticle;
use crate::ui::menus::death::death_plugin;
use crate::ui::menus::pause::pause_plugin;
use crate::ui::theme::WIREFRAME;
use bevy::ui::UiSystems;

pub mod chat;
pub mod chat_completion;
pub mod corner;
pub mod debug;
pub mod fps_counter;
pub mod health;
pub mod hotbar;
pub mod inventory;
pub mod item_browser;
pub mod minimap;
pub mod reticle;
mod ui_mode;

pub use inventory::*;
pub use ui_mode::*;

/// Everything drawn over the game: reticle, hotbar, inventory, chat, pause menu, minimap, FPS
/// counter and the debug overlays.
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
            default_color: WIREFRAME,
            ..default()
        })
        .insert_resource(UIMode::Closed)
        .insert_resource(DebugOptions::default())
        .init_resource::<FrameTimes>()
        .init_resource::<CellGestures>()
        .add_systems(
            OnEnter(GameState::Game),
            (spawn_reticle, setup_hud, setup_chat).chain(),
        )
        .add_systems(
            OnEnter(GameState::Game),
            (setup_inventory, health::spawn_health_bar).chain(),
        )
        .add_systems(OnEnter(GameState::Game), setup_chunk_ghost)
        .add_systems(
            OnEnter(GameState::Game),
            (
                reset_frame_times,
                (spawn_top_right_column, spawn_minimap, spawn_fps_counter).chain(),
            ),
        )
        .add_systems(
            Update,
            set_ui_mode
                .in_set(GameSet::UiMode)
                .run_if(in_state(GameState::Game)),
        )
        .add_systems(
            Update,
            (
                (
                    toggle_chat_system,
                    open_chat_on_slash,
                    display_chat_messages_system,
                )
                    .chain(),
                // Before the simulation, which predicts the clicks of the frame
                (
                    toggle_inventory_system,
                    close_inventory_system,
                    sync_inventory_focus,
                    // Before a press on a cell cancels a drag, and before the drawer closes
                    throw_held_stack_system.before(item_browser::drawer_keys),
                    click_inventory_cells_system,
                    drag_inventory_cells_system,
                    display_inventory_system,
                    display_crafting_system,
                    display_furnace_system,
                    style_inventory_panel,
                )
                    .chain()
                    .before(GameSet::Simulation),
            )
                .after(GameSet::UiMode)
                .run_if(in_state(GameState::Game)),
        )
        .add_systems(
            PostUpdate,
            (
                // before Bevy reads the font of the field and applies its edits
                match_the_field_font.before(UiSystems::Content),
                fit_chat_width.before(UiSystems::Layout),
                settle_chat_scroll.after(UiSystems::Layout),
            )
                .run_if(in_state(GameState::Game)),
        )
        .add_systems(
            Update,
            // `update_frame_inputs_system` sends the selected slot to the server
            (
                select_hotbar_slot_with_keys,
                select_hotbar_slot_with_wheel,
                // From the slot selected in this frame
                drop_selected_item_system,
                show_selected_item_name,
            )
                .chain()
                .after(GameSet::UiMode)
                .before(GameSet::Input)
                .run_if(in_state(GameState::Game)),
        )
        .add_systems(
            Update,
            (
                fps_text_update_system.run_if(fps_text_shown),
                show_fps_counter,
                (record_frame_time, draw_fps_graph).chain(),
                // The texts of the overlay are written only while it is shown
                (
                    coords_text_update_system,
                    biome_text_update_system,
                    loaded_chunks_text_update_system,
                    block_text_update_system,
                    time_text_update_system,
                )
                    .run_if(debug_overlay_shown),
                chunk_ghost_update_system,
                raycast_debug_update_system,
            )
                .run_if(in_state(GameState::Game)),
        )
        .add_systems(
            Update,
            // The game keys wait while a dialog is open or the player types
            (
                toggle_chunk_debug_mode_system,
                toggle_raycast_debug_mode_system,
                toggle_hud_system,
                cycle_fps_counter_system,
                toggle_wireframe_system,
            )
                .after(GameSet::UiMode)
                .run_if(in_state(GameState::Game).and_then(in_gameplay)),
        );
    app.add_systems(
        Update,
        // The prediction of the frame knows what was refused; the audio hears it after
        report_refused_actions
            .after(GameSet::Simulation)
            .before(GameSet::Reconciliation)
            .run_if(in_state(GameState::Game)),
    );
    app.add_systems(
        Update,
        (
            health::update_health_bar,
            health::raise_item_name_over_the_hearts,
        )
            .chain()
            .after(GameSet::Reconciliation)
            .run_if(in_state(GameState::Game)),
    );
    pause_plugin(app);
    death_plugin(app);
    chat_completion_plugin(app);
    minimap_plugin(app);
    item_browser::item_browser_plugin(app);
}
