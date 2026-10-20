use bevy::prelude::*;
use shared::world::{BlockId, ItemId};

use crate::state::{GameSet, GameState};
use crate::ui::hud::reticle::spawn_reticle;
use celestial::{
    celestial_light_system, setup_main_lighting, update_celestial_bodies,
    update_celestial_materials,
};
use time::{time_update_system, ClientTime};

pub mod celestial;
pub mod clouds;
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
        .init_resource::<ClimateMap>()
        // loaded from `video.ron` by `add_client`
        .init_resource::<BiomeBlend>()
        .init_resource::<GraphicsSettings>()
        .insert_resource(AtlasHandles::<BlockId>::default())
        .insert_resource(AtlasHandles::<ItemId>::default())
        // loaded from the game folder by `add_client`
        .init_resource::<RenderDistance>()
        .init_resource::<clouds::mesh::CloudStyle>()
        .add_plugins((
            chunk_material_plugin,
            clouds::clouds_plugin,
            shaders::shaders_plugin,
            celestial::celestial_material_plugin,
        ))
        .add_message::<WorldRenderRequestUpdateEvent>()
        .add_message::<BlockChanged>()
        .add_message::<ChunkLoadEvent>()
        .add_systems(
            Update,
            create_all_atlases.run_if(in_state(GameState::PreGameLoading)),
        )
        // Whatever the state: the first chunks arrive while the game loads
        .add_systems(
            Update,
            (update_climate_map_system, apply_graphics_settings_system),
        )
        .add_systems(
            OnEnter(GameState::Game),
            (
                setup_main_lighting.before(spawn_reticle),
                remesh_every_chunk,
            ),
        )
        .add_systems(
            Update,
            (
                (render_distance_update_system, send_render_distance_system)
                    .chain()
                    .after(GameSet::UiMode),
                chunk_force_reload_system.after(GameSet::UiMode),
                (update_celestial_bodies, celestial_light_system).chain(),
                update_celestial_materials,
                daylight_ambient_system,
            )
                .run_if(in_state(GameState::Game)),
        )
        .add_systems(
            PostUpdate,
            (
                request_remesh_of_changed_blocks_system,
                world_render_system,
                shadow_casters_system,
            )
                .chain()
                .in_set(GameSet::Meshing)
                .run_if(in_state(GameState::Game)),
        )
        .add_systems(
            FixedPostUpdate,
            time_update_system.run_if(in_state(GameState::Game)),
        );
}

/// Asks a mesh for every chunk held on entering the game: requests written while loading were
/// not read.
fn remesh_every_chunk(
    world: Res<ClientWorldMap>,
    mut ev_render: MessageWriter<WorldRenderRequestUpdateEvent>,
) {
    ev_render.write_batch(
        world
            .blocks
            .chunk_positions()
            .map(WorldRenderRequestUpdateEvent::ChunkToReload),
    );
}

#[cfg(test)]
mod tests {
    use bevy::state::app::StatesPlugin;
    use shared::world::Chunk;

    use super::*;

    #[test]
    fn chunks_received_while_loading_are_meshed_in_game() {
        let mut app = App::new();
        app.add_plugins(StatesPlugin)
            .insert_state(GameState::PreGameLoading)
            .add_message::<WorldRenderRequestUpdateEvent>()
            .init_resource::<ClientWorldMap>()
            .add_systems(OnEnter(GameState::Game), remesh_every_chunk);
        let chunks = [IVec3::ZERO, IVec3::new(0, -1, 0)];
        let mut map = app.world_mut().resource_mut::<ClientWorldMap>();
        for pos in chunks {
            map.blocks.insert_chunk(pos, Chunk::new());
        }
        app.update();

        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Game);
        app.update();

        let requests: Vec<_> = app
            .world_mut()
            .resource_mut::<Messages<WorldRenderRequestUpdateEvent>>()
            .drain()
            .collect();
        for pos in chunks {
            assert!(
                requests.contains(&WorldRenderRequestUpdateEvent::ChunkToReload(pos)),
                "{pos} in {requests:?}"
            );
        }
    }
}
