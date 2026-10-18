use bevy::prelude::*;
use shared::world::liquid::liquid_height;

use crate::world::ClientWorldMap;

use super::CameraController;

/// The colour behind the world.
pub const SKY_COLOR: Color = Color::srgb_u8(117, 159, 183);
const UNDERWATER_COLOR: Color = Color::srgb_u8(30, 62, 130);

/// Under water, a fog the colour of the water hides what is more than a few blocks away.
pub fn underwater_fog_system(
    world_map: Res<ClientWorldMap>,
    mut cameras: Query<(Entity, &Transform, &mut Camera, Has<DistanceFog>), With<CameraController>>,
    mut commands: Commands,
) {
    for (entity, transform, mut camera, has_fog) in &mut cameras {
        let eye = transform.translation;
        let under_water = liquid_height(&world_map.blocks, eye.floor().as_ivec3())
            .is_some_and(|height| eye.y - eye.y.floor() < height);
        if under_water == has_fog {
            continue;
        }
        if under_water {
            commands.entity(entity).insert(DistanceFog {
                color: UNDERWATER_COLOR,
                falloff: FogFalloff::Linear {
                    start: 2.0,
                    end: 16.0,
                },
                ..default()
            });
            camera.clear_color = ClearColorConfig::Custom(UNDERWATER_COLOR);
        } else {
            commands.entity(entity).remove::<DistanceFog>();
            camera.clear_color = ClearColorConfig::Custom(SKY_COLOR);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::world::{BlockId, ChunkMap};

    #[test]
    fn the_camera_gets_a_fog_under_water_and_loses_it_out_of_it() {
        let mut blocks = ChunkMap::with_loaded_region(IVec3::ZERO, IVec3::splat(15));
        blocks.fill_box(IVec3::ZERO, IVec3::new(15, 3, 15), BlockId::Water);
        let mut app = App::new();
        app.insert_resource(ClientWorldMap {
            blocks,
            ..default()
        })
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
        assert!(app.world().get::<DistanceFog>(camera).is_some());

        // In the top cell of the water, above its surface
        app.world_mut()
            .get_mut::<Transform>(camera)
            .unwrap()
            .translation
            .y = 3.95;
        app.update();
        assert!(app.world().get::<DistanceFog>(camera).is_none());

        app.world_mut()
            .get_mut::<Transform>(camera)
            .unwrap()
            .translation
            .y = 4.5;
        app.update();
        assert!(app.world().get::<DistanceFog>(camera).is_none());
        assert!(matches!(
            app.world().get::<Camera>(camera).unwrap().clear_color,
            ClearColorConfig::Custom(color) if color == SKY_COLOR
        ));
    }
}
