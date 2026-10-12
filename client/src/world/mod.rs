use bevy::prelude::*;
use shared::world::{BlockId, ItemId};

use crate::state::{GameSet, GameState};
use crate::ui::hud::reticle::spawn_reticle;
use celestial::{setup_main_lighting, update_celestial_bodies};
use time::{time_update_system, ClientTime};

pub mod celestial;
pub mod data;
pub mod rendering;
pub mod time;

pub use data::*;
pub use rendering::*;

/// Chunks and their meshes, textures and atlases, the sky and the world clock.
pub fn world_plugin(app: &mut App) {
    app.insert_resource(ClientTime(0))
        .insert_resource(GlobalAmbientLight {
            color: Color::WHITE,
            brightness: 400.0,
            ..default()
        })
        .insert_resource(MaterialResource { ..default() })
        .insert_resource(AtlasHandles::<BlockId>::default())
        .insert_resource(AtlasHandles::<ItemId>::default())
        .insert_resource(RenderDistance { ..default() })
        .add_message::<WorldRenderRequestUpdateEvent>()
        .add_systems(
            Update,
            create_all_atlases.run_if(in_state(GameState::PreGameLoading)),
        )
        .add_systems(
            OnEnter(GameState::Game),
            setup_main_lighting.before(spawn_reticle),
        )
        .add_systems(
            Update,
            (
                render_distance_update_system.after(GameSet::UiMode),
                chunk_force_reload_system.after(GameSet::UiMode),
                update_celestial_bodies,
            )
                .run_if(in_state(GameState::Game)),
        )
        .add_systems(
            PostUpdate,
            (request_remesh_of_changed_blocks_system, world_render_system)
                .chain()
                .in_set(GameSet::Meshing)
                .run_if(in_state(GameState::Game)),
        )
        .add_systems(
            FixedPostUpdate,
            time_update_system.run_if(in_state(GameState::Game)),
        );
}
