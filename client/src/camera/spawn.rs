use bevy::prelude::*;
use bevy_atmosphere::prelude::AtmosphereCamera;

use crate::GameState;

#[derive(Component)]
pub struct CameraController {
    pub distance: f32,
    pub angle_x: f32,
    pub angle_y: f32,
    pub mouse_sensitivity: f32,
}

impl Default for CameraController {
    fn default() -> Self {
        Self {
            distance: 10.0,
            angle_x: 0.0,
            angle_y: 20.0f32.to_radians(),
            mouse_sensitivity: 0.003,
        }
    }
}

pub fn spawn_camera(mut commands: Commands) {
    commands
        .spawn((
            Camera3d::default(),
            Transform::from_translation(Vec3::new(0.0, 5.0, 10.0))
                .looking_at(Vec3::new(0.0, 0.5, 0.0), Vec3::Y),
            GlobalTransform::default(),
            PerspectiveProjection {
                fov: f32::to_radians(60.0),
                ..Default::default()
            },
        ))
        .insert(CameraController::default())
        .insert(AtmosphereCamera::default())
        .insert(StateScoped(GameState::Game));
}
