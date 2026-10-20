use crate::camera::CameraController;
use crate::player::*;
use crate::ui::hud::UIMode;
use crate::world::ClientWorldMap;
use bevy::math::bounding::Aabb3d;
use bevy::{input::mouse::MouseMotion, prelude::*, window::PrimaryWindow};
use shared::players::ViewMode;
use shared::world::ChunkMap;

/// Height of the player's eyes above the centre of its box, standing.
const EYE_HEIGHT: f32 = 0.8;
/// How far behind the player the camera is in third-person view.
const THIRD_PERSON_DISTANCE: f32 = 10.0;
/// Largest angle the camera looks up or down by, short of flipping over.
const MAX_PITCH_DEGREES: f32 = 89.0;
/// Half the side of the box kept free of blocks around the third-person camera. The box holds
/// the corners of the near plane (0.18 from the camera at most, with the default near plane and
/// the field of view on a 21:9 screen), so that the plane never cuts through a block, even when
/// the camera grazes one.
const CAMERA_CLEARANCE: f32 = 0.25;
/// Step by which the camera's box moves away from the player while looking for room: smaller
/// than the box, so that it never steps over a block.
const CAMERA_PROBE_STEP: f32 = 0.1;
/// Halvings that narrow the gap between the last free step and the first blocked one, so that
/// the camera slides smoothly instead of moving by whole steps.
const CAMERA_PROBE_REFINEMENTS: u32 = 12;

/// How far from `target` the third-person camera can be along `direction`: the full
/// `distance`, or less when a block stands in the way, so that the camera never ends up
/// inside, behind or against the terrain.
fn unobstructed_distance(blocks: &ChunkMap, target: Vec3, direction: Vec3, distance: f32) -> f32 {
    let Some(direction) = direction.try_normalize() else {
        return distance;
    };
    let is_free = |d: f32| {
        let camera_box = Aabb3d::new(target + direction * d, Vec3::splat(CAMERA_CLEARANCE));
        !blocks.check_collision_box(&camera_box)
    };
    let mut free = 0.0;
    while free < distance {
        let next = (free + CAMERA_PROBE_STEP).min(distance);
        if !is_free(next) {
            let mut blocked = next;
            for _ in 0..CAMERA_PROBE_REFINEMENTS {
                let middle = (free + blocked) / 2.0;
                if is_free(middle) {
                    free = middle;
                } else {
                    blocked = middle;
                }
            }
            return free;
        }
        free = next;
    }
    distance
}

/// Turns the camera with the mouse and places it on the player (first person) or on a sphere
/// around it (third person).
pub fn camera_control_system(
    windows: Query<&Window, With<PrimaryWindow>>,
    mut mouse_motion_events: MessageReader<MouseMotion>,
    mut camera_query: Query<
        (&mut Transform, &mut CameraController),
        (With<Camera>, Without<CurrentPlayerMarker>),
    >,
    player_query: Query<(&Transform, &Children), With<CurrentPlayerMarker>>,
    bodies: Query<
        &Transform,
        (
            With<PlayerBody>,
            Without<Camera>,
            Without<CurrentPlayerMarker>,
        ),
    >,
    view_mode: Res<ViewMode>,
    ui_mode: Res<UIMode>,
    world_map: Res<ClientWorldMap>,
) {
    // The window can be missing while the app is closing, the player while it is spawning
    let (Ok(window), Ok((player_transform, children))) = (windows.single(), player_query.single())
    else {
        return;
    };
    let player_position = player_transform.translation;
    // The eyes go down with the body of a sneaking player
    let eyes = bodies
        .iter_many(children)
        .next()
        .map_or(Vec3::Y * EYE_HEIGHT, |body| {
            body.translation + Vec3::Y * EYE_HEIGHT * body.scale.y
        });

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
                camera_transform.translation = player_position + eyes;
                // Horizontal rotation first, then the vertical one, inverted so that moving
                // the mouse up looks up
                camera_transform.rotation = Quat::from_rotation_y(controller.angle_x)
                    * Quat::from_rotation_x(-controller.angle_y);
            }
            ViewMode::ThirdPerson => {
                controller.distance = THIRD_PERSON_DISTANCE;
                let direction = Vec3::new(
                    controller.angle_y.cos() * controller.angle_x.sin(),
                    controller.angle_y.sin(),
                    controller.angle_y.cos() * controller.angle_x.cos(),
                );
                let distance = unobstructed_distance(
                    &world_map.blocks,
                    player_position,
                    direction,
                    controller.distance,
                );
                // `looking_at` needs the camera away from the player to give a rotation
                let distance = distance.max(CAMERA_CLEARANCE);
                *camera_transform =
                    Transform::from_translation(player_position + direction * distance)
                        .looking_at(player_position, Vec3::Y);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::world::BlockId;

    const PLAYER: Vec3 = Vec3::new(0.5, 70.5, 0.5);

    fn world() -> ChunkMap {
        ChunkMap::with_loaded_region(IVec3::new(-32, 32, -32), IVec3::new(32, 95, 32))
    }

    #[test]
    fn the_camera_keeps_its_full_distance_in_the_open() {
        assert_eq!(
            unobstructed_distance(&world(), PLAYER, Vec3::NEG_Y, THIRD_PERSON_DISTANCE),
            THIRD_PERSON_DISTANCE
        );
    }

    #[test]
    fn the_camera_stays_above_the_ground_below_the_player() {
        let mut blocks = world();
        // Ground whose top is 3.5 blocks under the player's centre
        blocks.fill_box(IVec3::new(-5, 60, -5), IVec3::new(5, 66, 5), BlockId::Stone);
        let distance = unobstructed_distance(&blocks, PLAYER, Vec3::NEG_Y, THIRD_PERSON_DISTANCE);
        assert!(
            (distance - (3.5 - CAMERA_CLEARANCE)).abs() < 1e-4,
            "{distance}"
        );
    }

    #[test]
    fn the_camera_keeps_its_distance_from_the_ground_it_grazes() {
        let mut blocks = world();
        // Ground whose top is 3.5 blocks under the player's centre
        blocks.fill_box(
            IVec3::new(-20, 60, -20),
            IVec3::new(20, 66, 20),
            BlockId::Stone,
        );
        // Low enough for the camera to end up 0.05 above the ground, missing it
        let direction = Vec3::new(0.0, -0.345, (1.0f32 - 0.345 * 0.345).sqrt());
        let distance = unobstructed_distance(&blocks, PLAYER, direction, THIRD_PERSON_DISTANCE);
        let camera = PLAYER + direction * distance;
        assert!(camera.y - 67.0 >= CAMERA_CLEARANCE - 1e-4, "{camera}");
    }

    #[test]
    fn the_camera_stops_in_front_of_a_wall_behind_the_player() {
        let mut blocks = world();
        blocks.fill_box(IVec3::new(-5, 60, 4), IVec3::new(5, 80, 4), BlockId::Stone);
        let direction = Vec3::new(0.0, 1.0, 1.0);
        let distance = unobstructed_distance(&blocks, PLAYER, direction, THIRD_PERSON_DISTANCE);
        let camera = PLAYER + direction.normalize() * distance;
        assert!(camera.z < 4.0, "{camera}");
    }
}
