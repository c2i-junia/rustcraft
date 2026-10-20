//! The graphics settings of the Video tab besides the render distance and the biome blend, and
//! what they change: how the chunks are lit and shaded, and whether the sun casts shadows. With
//! them, the ambient light of the day and of the night.

use bevy::prelude::*;
use shared::DAY_DURATION_IN_TICKS;
use std::f32::consts::TAU;

use crate::world::time::ClientTime;
use crate::world::{ChunkShading, GlobalMaterial, MaterialResource, WorldRenderRequestUpdateEvent};

use super::render::respawn_every_group;
use super::ChunkMaterial;
use crate::world::ClientWorldMap;

/// The graphics settings of the Video tab besides the render distance and the biome blend.
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct GraphicsSettings {
    /// Whether the faces are lit smoothly, with ambient occlusion (`Lighting::smooth`).
    pub smooth_lighting: bool,
    /// From 0, Minecraft's "Moody", to 1, its "Bright": how much the dim light is brightened.
    pub brightness: f32,
    /// Whether the sun and the moon cast shadows.
    pub shadows: bool,
    /// Whether the plants sway in the wind, and the water ripples and reflects the light.
    pub effects: bool,
}

impl Default for GraphicsSettings {
    fn default() -> Self {
        Self {
            smooth_lighting: true,
            brightness: 0.5,
            shadows: true,
            effects: true,
        }
    }
}

impl GraphicsSettings {
    /// Sets the brightness, kept from 0 to 1.
    pub fn set_brightness(&mut self, brightness: f32) {
        if brightness.is_finite() {
            self.brightness = brightness.clamp(0., 1.);
        }
    }
}

/// Shades the chunks as the settings say once they change, and meshes every chunk again when
/// the lighting goes from smooth to flat or back. When the shadows are turned on again, the
/// chunks are drawn into new entities: Bevy 0.19 never again draws the shadows of a mesh once
/// they were turned off.
pub fn apply_graphics_settings_system(
    settings: Res<GraphicsSettings>,
    mut applied: Local<Option<GraphicsSettings>>,
    materials: Res<MaterialResource>,
    mut chunk_materials: ResMut<Assets<ChunkMaterial>>,
    mut world_map: ResMut<ClientWorldMap>,
    mut ev_render: MessageWriter<WorldRenderRequestUpdateEvent>,
    mut commands: Commands,
) {
    if applied.as_ref() == Some(&*settings) {
        return;
    }
    for (material, liquid) in [
        (GlobalMaterial::Blocks, false),
        (GlobalMaterial::Liquids, true),
    ] {
        let Some(handle) = materials.chunk_materials.get(&material) else {
            // Shaded as the settings say when made
            continue;
        };
        let shading = ChunkShading::new(&settings, liquid);
        if chunk_materials
            .get(handle)
            .is_some_and(|material| material.extension.shading != shading)
        {
            if let Some(mut material) = chunk_materials.get_mut(handle) {
                material.extension.shading = shading;
            }
        }
    }
    if applied.is_some_and(|before| !before.shadows && settings.shadows) {
        respawn_every_group(&mut world_map, &mut commands, &mut ev_render);
    } else if applied.is_some_and(|before| before.smooth_lighting != settings.smooth_lighting) {
        ev_render.write_batch(
            world_map
                .blocks
                .chunk_positions()
                .map(WorldRenderRequestUpdateEvent::ChunkToReload),
        );
    }
    *applied = Some(*settings);
}

/// The brightness of the ambient light at noon, and at midnight.
const DAY_AMBIENT: f32 = 400.;
const NIGHT_AMBIENT: f32 = 60.;

/// How bright the full light of the blocks is, as an ambient light: a torch lights the walls
/// around it about as much as the sun and the sky light the ground at noon, by day or by night.
pub const BLOCK_LIGHT: f32 = DAY_AMBIENT * 2.;

/// How much of the light of the day there is at `tick`: 1 while the sun is well up, 0 while it
/// is well under the horizon, and fading at dawn and at dusk.
fn daylight(tick: u64) -> f32 {
    let angle = (tick % DAY_DURATION_IN_TICKS) as f32 / DAY_DURATION_IN_TICKS as f32 * TAU;
    // The height of the sun, as `update_celestial_bodies` turns it
    let height = -angle.sin();
    ((height + 0.1) / 0.35).clamp(0., 1.)
}

/// Dims the ambient light at night: the faces lit by the sky only are dark, if readable.
pub fn daylight_ambient_system(time: Res<ClientTime>, mut ambient: ResMut<GlobalAmbientLight>) {
    let brightness = NIGHT_AMBIENT + (DAY_AMBIENT - NIGHT_AMBIENT) * daylight(time.0);
    if (ambient.brightness - brightness).abs() > 0.5 {
        ambient.brightness = brightness;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::ChunkRender;
    use bevy::ecs::message::Messages;

    #[test]
    fn turning_the_shadows_on_again_draws_the_chunks_into_new_entities() {
        let mut app = App::new();
        app.init_resource::<GraphicsSettings>()
            .init_resource::<MaterialResource>()
            .init_resource::<Assets<ChunkMaterial>>()
            .init_resource::<ClientWorldMap>()
            .add_message::<WorldRenderRequestUpdateEvent>()
            .add_systems(Update, apply_graphics_settings_system);
        let entity = app.world_mut().spawn_empty().id();
        let mut render = ChunkRender::new();
        render.entity = Some(entity);
        render.chunks.insert(IVec3::ZERO);
        app.world_mut()
            .resource_mut::<ClientWorldMap>()
            .render
            .insert(IVec2::ZERO, render);
        let requests = |app: &mut App| {
            app.world_mut()
                .resource_mut::<Messages<WorldRenderRequestUpdateEvent>>()
                .drain()
                .count()
        };
        app.update();

        app.world_mut().resource_mut::<GraphicsSettings>().shadows = false;
        app.update();
        // Turned off, the shadows are only no longer drawn
        assert!(app.world().get_entity(entity).is_ok());
        assert_eq!(requests(&mut app), 0);

        app.world_mut().resource_mut::<GraphicsSettings>().shadows = true;
        app.update();
        assert!(app.world().get_entity(entity).is_err());
        let map = app.world().resource::<ClientWorldMap>();
        assert_eq!(map.render[&IVec2::ZERO].entity, None);
        assert_eq!(requests(&mut app), 1);
    }

    #[test]
    fn the_day_is_bright_the_night_dark_and_dawn_in_between() {
        let tick = |fraction: f64| (DAY_DURATION_IN_TICKS as f64 * fraction) as u64;
        assert_eq!(daylight(tick(0.75)), 1.);
        assert_eq!(daylight(tick(0.25)), 0.);
        let dawn = daylight(tick(0.0));
        assert!(0. < dawn && dawn < 1., "{dawn}");
    }
}
