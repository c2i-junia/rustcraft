use bevy::prelude::*;
use shared::world::{BlockId, ItemId};

use crate::state::{GameSet, GameState};
use crate::ui::hud::reticle::spawn_reticle;
use celestial::{
    celestial_light_system, setup_main_lighting, update_celestial_bodies,
    update_celestial_materials,
};
use time::{time_update_system, ClientTime};
use weather::{ease_weather_system, ClientWeather};

pub mod celestial;
pub mod clouds;
pub mod data;
pub mod lightning;
pub mod precipitation;
pub mod rendering;
pub mod time;
pub mod weather;

pub use data::*;
pub use rendering::*;

/// Where the shadow roles and layers of the groups (`GameSet::Meshing`) and the filter of the
/// portal cameras' casters (`keep_twin_shadows_to_portal_cameras`) stand among Bevy's systems.
/// The roles and layers are set before Bevy finds what each view and each shadow cascade draws,
/// and the filter runs once it has, on the roles of this frame: else, on the frame a far twin is
/// crossed, the groups around the player would still be marked for the portal cameras only and
/// be left out of its cascades, the terrain unshadowed for a frame.
fn order_the_shadows(app: &mut App) {
    use bevy::camera::visibility::VisibilitySystems;
    use bevy::light::SimulationLightSystems;
    app.configure_sets(
        PostUpdate,
        GameSet::Meshing.before(VisibilitySystems::CheckVisibility),
    )
    .add_systems(
        PostUpdate,
        keep_twin_shadows_to_portal_cameras
            .after(SimulationLightSystems::CheckLightVisibility)
            .after(GameSet::Meshing)
            .run_if(in_state(GameState::Game)),
    );
}

/// Chunks and their meshes, textures and atlases, the sky, the world clock and the weather.
pub fn world_plugin(app: &mut App) {
    order_the_shadows(app);
    app.insert_resource(ClientTime(0))
        .init_resource::<ClientWeather>()
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
            overcast::overcast_plugin,
            precipitation::precipitation_plugin,
            lightning::lightning_plugin,
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
                ease_weather_system,
            )
                .run_if(in_state(GameState::Game)),
        )
        .add_systems(
            PostUpdate,
            (
                request_remesh_of_changed_blocks_system,
                world_render_system,
                shadow_casters_system,
                portal_chunks_layers_system,
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

/// The value of the constant `name`, an `f32`, in the WGSL `source`: for the tests that tie the
/// shaders' copies of a constant to the Rust one.
#[cfg(test)]
pub fn wgsl_constant(source: &str, name: &str) -> f32 {
    let declared = format!("const {name}: f32 = ");
    let mut found = source
        .lines()
        .filter_map(|line| line.trim().strip_prefix(&declared));
    let value = found
        .next()
        .unwrap_or_else(|| panic!("no {name} in the shader"));
    assert!(found.next().is_none(), "{name} twice in the shader");
    value.trim_end_matches(';').parse().unwrap()
}

#[cfg(test)]
mod tests {
    use bevy::state::app::StatesPlugin;
    use shared::world::Chunk;

    use super::*;

    /// The order `order_the_shadows` gives, whatever the order the systems were added in: those
    /// of Bevy, here stand-ins in their sets, added first.
    #[test]
    fn the_shadow_roles_are_set_before_bevy_lists_the_casters_and_filtered_after() {
        use bevy::camera::visibility::VisibilitySystems;
        use bevy::ecs::schedule::SingleThreadedExecutor;
        use bevy::light::SimulationLightSystems;

        #[derive(Resource, Default)]
        struct Ran(Vec<&'static str>);
        let ran = |name: &'static str| move |mut ran: ResMut<Ran>| ran.0.push(name);
        let mut app = App::new();
        app.add_plugins(StatesPlugin)
            .insert_state(GameState::Game)
            .init_resource::<Ran>()
            .edit_schedule(PostUpdate, |schedule| {
                schedule.set_executor(SingleThreadedExecutor::new());
            })
            .add_systems(
                PostUpdate,
                (
                    ran("lights").in_set(SimulationLightSystems::CheckLightVisibility),
                    ran("views").in_set(VisibilitySystems::CheckVisibility),
                    ran("roles").in_set(GameSet::Meshing),
                ),
            );
        order_the_shadows(&mut app);
        // The filter itself, by the order it was given
        app.add_systems(
            PostUpdate,
            ran("filter").after(keep_twin_shadows_to_portal_cameras),
        );
        app.world_mut().run_schedule(PostUpdate);
        let ran = &app.world().resource::<Ran>().0;
        let at = |name| ran.iter().position(|ran| *ran == name).unwrap();
        assert!(at("roles") < at("views"), "{ran:?}");
        assert!(at("roles") < at("lights"), "{ran:?}");
        assert!(at("lights") < at("filter"), "{ran:?}");
    }

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
