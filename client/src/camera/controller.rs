use crate::camera::CameraController;
use crate::player::*;
use crate::ui::hud::UIMode;
use bevy::{input::mouse::MouseMotion, prelude::*, window::PrimaryWindow};

// System to control the camera based on mouse movement
pub fn camera_control_system(
    windows: Query<&Window, With<PrimaryWindow>>,
    mut mouse_motion_events: EventReader<MouseMotion>,
    mut camera_query: Query<
        (&mut Transform, &mut CameraController),
        (With<Camera>, Without<CurrentPlayerMarker>),
    >,
    player_query: Query<&Transform, With<CurrentPlayerMarker>>,
    view_mode: Res<ViewMode>,
    ui_mode: Res<UIMode>,
) {
    let window = windows.single().unwrap();

    // if the window is not focused, ignore camera movement
    if !window.focused || *ui_mode != UIMode::Closed {
        mouse_motion_events.clear();
    }

    let mut delta = Vec2::ZERO;

    // accumulate mouse movements
    for event in mouse_motion_events.read() {
        delta += event.delta;
    }

    for (mut camera_transform, mut controller) in camera_query.iter_mut() {
        // first-person view
        if *view_mode == ViewMode::FirstPerson {
            // distance is set to 0 for first-person view
            controller.distance = 0.0;

            // place the camera at the player's head height (e.g. 1.8 units)
            let player_transform = player_query.single().unwrap();
            let player_position = player_transform.translation;

            // apply mouse sensitivity and adjust camera angle
            controller.angle_x -= delta.x * controller.mouse_sensitivity;
            controller.angle_y += delta.y * controller.mouse_sensitivity;

            // limit vertical angle to prevent flipping
            controller.angle_y = controller
                .angle_y
                .clamp(-89.0f32.to_radians(), 89.0f32.to_radians());

            // adjust the camera's position to be at the player's eye level
            camera_transform.translation = Vec3::new(
                player_position.x,
                player_position.y + 0.8, // adjust height for the player's eyes
                player_position.z,
            );

            // calculate rotation: apply both horizontal (Y) and vertical (X) rotation
            let rotation_x = Quat::from_rotation_y(controller.angle_x); // horizontal rotation
            let rotation_y = Quat::from_rotation_x(-controller.angle_y); // vertical rotation (inverted to correct direction)

            // apply the combined rotations
            camera_transform.rotation = rotation_x * rotation_y;
        } else if *view_mode == ViewMode::ThirdPerson {
            // in third-person view, place the camera behind the player
            controller.distance = 10.0;

            // apply mouse sensitivity and adjust camera angle
            controller.angle_x -= delta.x * controller.mouse_sensitivity;
            controller.angle_y += delta.y * controller.mouse_sensitivity;

            // limit vertical angle to prevent flipping
            controller.angle_y = controller
                .angle_y
                .clamp(-89.0f32.to_radians(), 89.0f32.to_radians());

            let player_transform = player_query.single().unwrap();
            let player_position = player_transform.translation;

            // calculate the new camera position
            let x = player_position.x
                + controller.distance * controller.angle_y.cos() * controller.angle_x.sin();
            let y = player_position.y + controller.distance * controller.angle_y.sin();
            let z = player_position.z
                + controller.distance * controller.angle_y.cos() * controller.angle_x.cos();

            let camera_position = Vec3::new(x, y, z);

            // update the camera's position in third-person view
            *camera_transform =
                Transform::from_translation(camera_position).looking_at(player_position, Vec3::Y);
        }
    }
}
