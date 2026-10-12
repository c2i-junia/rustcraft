use crate::camera::CameraController;
use crate::player::*;
use crate::ui::hud::UIMode;
use bevy::{input::mouse::MouseMotion, prelude::*, window::PrimaryWindow};
use shared::players::ViewMode;

/// Height of the player's eyes above the centre of its box.
const EYE_HEIGHT: f32 = 0.8;
/// How far behind the player the camera is in third-person view.
const THIRD_PERSON_DISTANCE: f32 = 10.0;
/// Largest angle the camera looks up or down by, short of flipping over.
const MAX_PITCH_DEGREES: f32 = 89.0;

/// Turns the camera with the mouse and places it on the player (first person) or on a sphere
/// around it (third person).
pub fn camera_control_system(
    windows: Query<&Window, With<PrimaryWindow>>,
    mut mouse_motion_events: MessageReader<MouseMotion>,
    mut camera_query: Query<
        (&mut Transform, &mut CameraController),
        (With<Camera>, Without<CurrentPlayerMarker>),
    >,
    player_query: Query<&Transform, With<CurrentPlayerMarker>>,
    view_mode: Res<ViewMode>,
    ui_mode: Res<UIMode>,
) {
    // The window can be missing while the app is closing, the player while it is spawning
    let (Ok(window), Ok(player_transform)) = (windows.single(), player_query.single()) else {
        return;
    };
    let player_position = player_transform.translation;

    // if the window is not focused, ignore camera movement
    if !window.focused || *ui_mode != UIMode::Closed {
        mouse_motion_events.clear();
    }
    let delta: Vec2 = mouse_motion_events.read().map(|event| event.delta).sum();
    let max_pitch = MAX_PITCH_DEGREES.to_radians();

    for (mut camera_transform, mut controller) in camera_query.iter_mut() {
        controller.angle_x -= delta.x * controller.mouse_sensitivity;
        controller.angle_y = (controller.angle_y + delta.y * controller.mouse_sensitivity)
            .clamp(-max_pitch, max_pitch);

        match *view_mode {
            ViewMode::FirstPerson => {
                controller.distance = 0.0;
                camera_transform.translation = player_position + Vec3::Y * EYE_HEIGHT;
                // Horizontal rotation first, then the vertical one, inverted so that moving
                // the mouse up looks up
                camera_transform.rotation = Quat::from_rotation_y(controller.angle_x)
                    * Quat::from_rotation_x(-controller.angle_y);
            }
            ViewMode::ThirdPerson => {
                controller.distance = THIRD_PERSON_DISTANCE;
                let offset = Vec3::new(
                    controller.distance * controller.angle_y.cos() * controller.angle_x.sin(),
                    controller.distance * controller.angle_y.sin(),
                    controller.distance * controller.angle_y.cos() * controller.angle_x.cos(),
                );
                *camera_transform = Transform::from_translation(player_position + offset)
                    .looking_at(player_position, Vec3::Y);
            }
        }
    }
}
