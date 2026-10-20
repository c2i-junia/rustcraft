use bevy::prelude::*;
use shared::world::liquid::liquid_height;
use shared::CHUNK_SIZE;

use crate::world::{ClientWorldMap, RenderDistance, ServerRenderDistanceLimit};

use super::CameraController;

/// The colour behind the world.
pub const SKY_COLOR: Color = Color::srgb_u8(117, 159, 183);
const UNDERWATER_COLOR: Color = Color::srgb_u8(30, 62, 130);

/// Under water, a fog the colour of the water hides what is more than a few blocks away. Out
/// of it, a fog the colour of the sky hides the edge of the chunks shown: it thickens over the
/// last part of the render distance, and the world fades into the sky at its end.
pub fn underwater_fog_system(
    world_map: Res<ClientWorldMap>,
    render_distance: Res<RenderDistance>,
    server_limit: Option<Res<ServerRenderDistanceLimit>>,
    mut cameras: Query<
        (Entity, &Transform, &mut Camera, Option<&DistanceFog>),
        With<CameraController>,
    >,
    mut commands: Commands,
) {
    let distance = render_distance
        .distance
        .min(server_limit.map_or(u32::MAX, |limit| limit.0));
    for (entity, transform, mut camera, fog) in &mut cameras {
        let eye = transform.translation;
        let under_water = liquid_height(&world_map.blocks, eye.floor().as_ivec3())
            .is_some_and(|height| eye.y - eye.y.floor() < height);
        let (color, start, end) = if under_water {
            (UNDERWATER_COLOR, 2.0, 16.0)
        } else {
            // Every chunk within the render distance of the chunk of the player is shown: the
            // edge is at least this far from the player along x and z
            let end = (distance * CHUNK_SIZE as u32) as f32;
            (SKY_COLOR, end * FOG_START, end)
        };
        if fog.is_some_and(|fog| {
            fog.color == color
                && matches!(fog.falloff, FogFalloff::Linear { start: s, end: e } if s == start && e == end)
        }) {
            continue;
        }
        commands.entity(entity).insert(DistanceFog {
            color,
            falloff: FogFalloff::Linear { start, end },
            ..default()
        });
        camera.clear_color = ClearColorConfig::Custom(color);
    }
}

/// Where the fog of the sky starts, as a part of the render distance.
const FOG_START: f32 = 0.6;

#[cfg(test)]
mod tests {
    use super::*;
    use shared::world::{BlockId, ChunkMap};

    fn fog_of(app: &App, camera: Entity) -> (Color, f32) {
        let fog = app.world().get::<DistanceFog>(camera).unwrap();
        let FogFalloff::Linear { end, .. } = fog.falloff else {
            panic!("a linear fog");
        };
        (fog.color, end)
    }

    #[test]
    fn under_water_the_fog_is_near_and_out_of_it_at_the_end_of_the_render_distance() {
        let mut blocks = ChunkMap::with_loaded_region(IVec3::ZERO, IVec3::splat(15));
        blocks.fill_box(IVec3::ZERO, IVec3::new(15, 3, 15), BlockId::Water);
        let mut app = App::new();
        app.insert_resource(ClientWorldMap {
            blocks,
            ..default()
        })
        .insert_resource(RenderDistance { distance: 24 })
        .add_systems(Update, underwater_fog_system);
        let camera = app
            .world_mut()
            .spawn((
                Camera::default(),
                Transform::from_xyz(5.5, 2.5, 5.5),
                CameraController::default(),
            ))
            .id();

        app.update();
        assert_eq!(fog_of(&app, camera), (UNDERWATER_COLOR, 16.0));

        // In the top cell of the water, above its surface
        app.world_mut()
            .get_mut::<Transform>(camera)
            .unwrap()
            .translation
            .y = 3.95;
        app.update();
        assert_eq!(fog_of(&app, camera), (SKY_COLOR, 24.0 * 16.0));
        assert!(matches!(
            app.world().get::<Camera>(camera).unwrap().clear_color,
            ClearColorConfig::Custom(color) if color == SKY_COLOR
        ));

        // A server that allows less brings the fog nearer
        app.insert_resource(ServerRenderDistanceLimit(8));
        app.update();
        assert_eq!(fog_of(&app, camera), (SKY_COLOR, 8.0 * 16.0));
    }
}
