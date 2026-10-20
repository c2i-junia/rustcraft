//! What the stance of a player shows: a sneaking player crouches and the camera lowers its
//! eyes with it, a sprinting one sees wider, and a zooming one narrower.

use bevy::prelude::*;
use shared::players::Player;

use crate::camera::{Zoom, FOV_DEGREES};

use super::CurrentPlayerMarker;

/// The body of a player, a child of its entity, squashed toward the feet while it sneaks. The
/// entity of the player stays at its position, which the other systems read.
#[derive(Component)]
pub struct PlayerBody;

/// Height of a sneaking body, relative to a standing one.
pub const SNEAK_BODY_SCALE: f32 = 0.8;

/// Field of view of a sprinting player, relative to `FOV_DEGREES`.
const SPRINT_FOV_SCALE: f32 = 1.1;

/// How fast the body and the field of view follow the stance: the share of the way left that
/// is covered in a second, as an exponential decay rate.
const STANCE_EASING: f32 = 15.0;

/// The share of the way to the stance covered in a frame of `delta` seconds.
fn easing(delta: f32) -> f32 {
    1.0 - (-delta * STANCE_EASING).exp()
}

/// The local transform of the body of a player of `height` at `scale` of its standing height:
/// its feet stay where they are.
pub fn body_transform(height: f32, scale: f32) -> Transform {
    Transform::from_translation(Vec3::Y * (-height * (1.0 - scale) / 2.0))
        .with_scale(Vec3::new(1.0, scale, 1.0))
}

/// Eases the body of each player toward its stance.
pub fn crouch_bodies_system(
    time: Res<Time>,
    players: Query<(&Player, &Children)>,
    mut bodies: Query<&mut Transform, With<PlayerBody>>,
) {
    let t = easing(time.delta_secs());
    for (player, children) in &players {
        let target = if player.is_sneaking {
            SNEAK_BODY_SCALE
        } else {
            1.0
        };
        let mut children = bodies.iter_many_mut(children);
        while let Some(mut body) = children.fetch_next() {
            if body.scale.y == target {
                continue;
            }
            let mut scale = body.scale.y.lerp(target, t);
            if (scale - target).abs() < 1e-3 {
                scale = target;
            }
            *body = body_transform(player.height, scale);
        }
    }
}

/// Widens the field of view of the camera while the local player sprints, and narrows it while
/// they zoom.
pub fn fov_system(
    time: Res<Time>,
    zoom: Res<Zoom>,
    player: Query<&Player, With<CurrentPlayerMarker>>,
    mut projections: Query<&mut Projection, With<Camera3d>>,
) {
    let Ok(player) = player.single() else {
        return;
    };
    let scale = if player.is_sprinting {
        SPRINT_FOV_SCALE
    } else {
        1.0
    };
    let target = FOV_DEGREES.to_radians() * scale * zoom.scale();
    let t = easing(time.delta_secs());
    for mut projection in &mut projections {
        // Reached through `bypass_change_detection` first: a camera at its field of view must
        // not be marked changed every frame
        let Projection::Perspective(perspective) = projection.bypass_change_detection() else {
            continue;
        };
        if perspective.fov == target {
            continue;
        }
        let mut fov = perspective.fov.lerp(target, t);
        if (fov - target).abs() < 1e-4 {
            fov = target;
        }
        if let Projection::Perspective(perspective) = projection.as_mut() {
            perspective.fov = fov;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_crouched_body_keeps_its_feet_on_the_ground() {
        let height = 1.8;
        for scale in [1.0, SNEAK_BODY_SCALE, 0.5] {
            let body = body_transform(height, scale);
            let feet = body.translation.y - height * body.scale.y / 2.0;
            assert!(
                (feet + height / 2.0).abs() < 1e-6,
                "scale {scale}: feet at {feet}"
            );
        }
    }

    #[test]
    fn the_stance_is_reached_whatever_the_frame_rate() {
        for delta in [1.0 / 30.0, 1.0 / 144.0] {
            let mut scale = 1.0_f32;
            let frames = (0.5 / delta) as u32;
            for _ in 0..frames {
                scale = scale.lerp(SNEAK_BODY_SCALE, easing(delta));
            }
            assert!((scale - SNEAK_BODY_SCALE).abs() < 1e-3, "{delta}: {scale}");
        }
    }
}
