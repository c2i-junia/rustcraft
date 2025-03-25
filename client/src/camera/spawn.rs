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

const DEFAULT_DISTANCE: f32 = 10.0;
const DEFAULT_MOUSE_SENSITIVITY: f32 = 0.003;

impl Default for CameraController {
    fn default() -> Self {
        Self {
            distance: DEFAULT_DISTANCE,
            angle_x: 0.0,
            angle_y: 20.0f32.to_radians(),
            mouse_sensitivity: DEFAULT_MOUSE_SENSITIVITY,
        }
    }
}

impl From<Quat> for CameraController {
    fn from(quat: Quat) -> Self {
        Self {
            distance: DEFAULT_DISTANCE,
            angle_x: quat.to_euler(EulerRot::XYZ).0,
            angle_y: quat.to_euler(EulerRot::XYZ).1,
            mouse_sensitivity: DEFAULT_MOUSE_SENSITIVITY,
        }
    }
}

#[allow(deprecated)]
pub fn spawn_camera(mut commands: Commands) {
    commands
        .spawn((
            Camera3dBundle {
                projection: Projection::Perspective(PerspectiveProjection {
                    fov: f32::to_radians(60.0),
                    ..Default::default()
                }),
                transform: Transform::from_translation(Vec3::new(0.0, 5.0, 10.0))
                    .looking_at(Vec3::new(0.0, 0.5, 0.0), Vec3::Y),
                ..Default::default()
            },
            CameraController::default(),
            AtmosphereCamera::default(),
            StateScoped(GameState::Game),
        ));
}
