use bevy::prelude::*;
use shared::messages::{PlayerSpawnEvent, PlayerUpdateEvent};
use shared::players::{Inventory, Player, ViewMode};

use crate::state::{GameSet, GameState};

mod controller;
pub mod hand;
mod hurt;
mod interactions;
mod labels;
pub mod model;
pub mod skin;
mod stance;
mod update;

pub use controller::*;
pub use hurt::*;
pub use interactions::*;
pub use labels::*;
pub use stance::*;
pub use update::*;

/// The local player and the other players: their spawn, movement, prediction and labels.
pub fn player_plugin(app: &mut App) {
    hurt_plugin(app);
    app.init_resource::<hand::ViewModelMaterials>()
        .insert_resource(ViewMode::FirstPerson)
        .insert_resource(Inventory::new())
        .add_message::<PlayerSpawnEvent>()
        .add_message::<PlayerUpdateEvent>()
        .add_message::<LocalPlayerMoved>()
        .add_message::<InventoryActionPredicted>()
        .add_systems(
            Update,
            spawn_players_system.run_if(in_state(GameState::PreGameLoading)),
        )
        .add_systems(
            Update,
            first_and_third_person_view_system
                .after(GameSet::UiMode)
                .run_if(in_state(GameState::Game)),
        )
        .add_systems(
            Update,
            (spawn_players_system, player_labels_system).run_if(in_state(GameState::Game)),
        )
        .add_systems(
            Update,
            update_frame_inputs_system
                .in_set(GameSet::Input)
                .run_if(in_state(GameState::Game)),
        )
        .add_systems(
            Update,
            (
                handle_block_interactions,
                predict_inventory_system,
                player_movement_system,
            )
                .chain()
                .in_set(GameSet::Simulation)
                .run_if(in_state(GameState::Game))
                .run_if(local_player_is_simulated),
        )
        .add_systems(
            Update,
            update_players_system
                .in_set(GameSet::Reconciliation)
                .run_if(in_state(GameState::Game)),
        )
        .add_systems(
            Update,
            (
                detect_local_hurt,
                advance_hurt_feedback,
                update_hurt_vignette,
            )
                .chain()
                .after(GameSet::Reconciliation)
                .before(GameSet::Camera)
                .run_if(in_state(GameState::Game)),
        )
        .add_systems(
            Update,
            (
                crouch_bodies_system,
                fov_system,
                model::animate_players_system,
            )
                .after(GameSet::Reconciliation)
                .before(GameSet::Camera)
                .run_if(in_state(GameState::Game)),
        )
        .add_systems(
            Update,
            (
                hand::track_the_local_held_item,
                hand::hold_items_in_hands,
                hand::spawn_view_model,
                hand::move_the_hand,
                hand::draw_held_items,
            )
                .chain()
                .after(model::animate_players_system)
                .run_if(in_state(GameState::Game)),
        )
        .add_systems(
            Update,
            model::hide_the_body_from_an_eye_through_a_portal
                .after(GameSet::Camera)
                .run_if(in_state(GameState::Game)),
        )
        .add_systems(
            PreUpdate,
            pre_input_update_system.run_if(in_state(GameState::Game)),
        )
        .add_systems(
            OnTransition {
                exited: GameState::PreGameLoading,
                entered: GameState::Menu,
            },
            despawn_players,
        );
}

/// The players spawn on the loading screen and live until the game is left (`DespawnOnExit`).
/// Cancelling the loading screen never enters the game, so they and their labels are despawned
/// on that transition too: the next session would take its own player for one already there.
fn despawn_players(
    mut commands: Commands,
    players: Query<Entity, Or<(With<Player>, With<PlayerLabel>)>>,
) {
    for entity in &players {
        commands.entity(entity).despawn();
    }
}
