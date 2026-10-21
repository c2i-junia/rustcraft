//! The projection of a portal camera. The camera stands behind the twin of the portal, in its
//! wall or further, and must not draw what lies between it and the twin's plane: its near plane
//! is that plane, oblique to its view (Lengyel's method, which Bevy's `PerspectiveProjection`
//! applies to its infinite reverse-Z matrix through `near_clip_plane`).

use bevy::prelude::*;
use shared::world::Portal;

/// How far behind the plane of the twin a portal camera must be for its near plane to be that
/// plane. Nearer, an oblique near plane would squeeze the depth to nothing, and the camera keeps
/// the plain projection: the little it may see behind the plane is the inside of the wall.
pub const MIN_OBLIQUE_DISTANCE: f32 = 0.12;

/// The projection of the camera at `camera` that looks out of `twin`: the main camera's
/// `main`, whose near plane is the plane of the twin.
pub fn portal_projection(
    main: &PerspectiveProjection,
    camera: &Transform,
    twin: &Portal,
) -> PerspectiveProjection {
    let behind = -twin.signed_distance(camera.translation);
    if behind < MIN_OBLIQUE_DISTANCE {
        return PerspectiveProjection {
            near_clip_plane: Vec4::new(0.0, 0.0, -1.0, -main.near),
            ..main.clone()
        };
    }
    let view_from_world = camera.compute_affine().inverse();
    // The normal points away from the camera, to what is drawn
    let normal = view_from_world.transform_vector3(twin.normal()).normalize();
    let point = view_from_world.transform_point3(twin.frame().translation.into());
    PerspectiveProjection {
        near_clip_plane: normal.extend(-normal.dot(point)),
        // Bevy keeps the plain matrix when the normal is exactly (0, 0, -1), a twin seen
        // straight on: its near plane is then the twin's plane. Otherwise `near` does not count.
        near: behind,
        ..main.clone()
    }
}

#[cfg(test)]
mod tests {
    use bevy::camera::CameraProjection;
    use shared::world::{FaceDirection, PortalColor};

    use super::*;

    /// A portal on a wall facing +Z: its plane is z = 1, its centre (1, 11.5, 1).
    fn twin(face: FaceDirection) -> Portal {
        Portal {
            owner: 1,
            color: PortalColor::Orange,
            anchor: IVec3::new(0, 10, 0),
            face,
            up: FaceDirection::PlusY,
        }
    }

    fn main_projection() -> PerspectiveProjection {
        PerspectiveProjection {
            fov: 60f32.to_radians(),
            aspect_ratio: 16.0 / 9.0,
            ..default()
        }
    }

    /// The depth `point` is drawn at, from 1 at the near plane to 0 at infinity; outside of
    /// that, it is clipped.
    fn depth(projection: &PerspectiveProjection, camera: &Transform, point: Vec3) -> f32 {
        let view = camera.compute_affine().inverse();
        let clip = projection.get_clip_from_view() * view.transform_point3(point).extend(1.0);
        clip.z / clip.w
    }

    fn is_drawn(depth: f32) -> bool {
        (0.0..=1.0).contains(&depth)
    }

    /// Five blocks behind the twin, a little to its side, looking out of it.
    fn askew() -> Transform {
        Transform::from_xyz(-1.0, 12.0, -4.0).looking_at(Vec3::new(1.0, 11.5, 1.0), Vec3::Y)
    }

    #[test]
    fn a_point_behind_the_twins_plane_is_clipped() {
        let camera = askew();
        let projection =
            portal_projection(&main_projection(), &camera, &twin(FaceDirection::PlusZ));
        // In the wall, ahead of the camera
        let depth = depth(&projection, &camera, Vec3::new(0.6, 11.6, 0.5));
        assert!(!is_drawn(depth), "{depth}");
        // Without the portal, the plain projection would draw it
        let plain = super::tests::depth(&main_projection(), &camera, Vec3::new(0.6, 11.6, 0.5));
        assert!(is_drawn(plain));
    }

    #[test]
    fn a_point_in_front_of_it_is_not() {
        let camera = askew();
        let projection =
            portal_projection(&main_projection(), &camera, &twin(FaceDirection::PlusZ));
        for point in [Vec3::new(1.2, 11.4, 1.5), Vec3::new(4.0, 13.0, 30.0)] {
            let depth = depth(&projection, &camera, point);
            assert!(is_drawn(depth), "{point}: {depth}");
        }
    }

    #[test]
    fn depth_stays_reversed_beyond_the_plane() {
        let camera = askew();
        let projection =
            portal_projection(&main_projection(), &camera, &twin(FaceDirection::PlusZ));
        let depths: Vec<f32> = [2.0, 5.0, 20.0, 200.0]
            .into_iter()
            .map(|z| depth(&projection, &camera, Vec3::new(1.5, 11.5, z)))
            .collect();
        assert!(
            depths.windows(2).all(|pair| pair[0] > pair[1]),
            "farther is not deeper: {depths:?}"
        );
    }

    #[test]
    fn a_portal_seen_straight_on_is_clipped_too() {
        // A twin facing -Z, the camera behind it looking along -Z: the normal of the plane in
        // the view is exactly (0, 0, -1)
        let twin = twin(FaceDirection::MinusZ);
        let camera = Transform::from_xyz(1.0, 11.5, 4.0);
        let projection = portal_projection(&main_projection(), &camera, &twin);
        // The plane is z = 0; the wall cells are 0 < z < 1
        assert!(!is_drawn(depth(
            &projection,
            &camera,
            Vec3::new(1.0, 11.5, 0.5)
        )));
        assert!(is_drawn(depth(
            &projection,
            &camera,
            Vec3::new(1.0, 11.5, -0.5)
        )));
    }

    #[test]
    fn a_floor_or_a_ceiling_seen_straight_on_is_clipped_too() {
        // A twin on the floor, its plane y = 11, the camera under it looking up; and one on the
        // ceiling, its plane y = 10, the camera over it looking down. Whatever the camera's up,
        // the plane's normal in the view is (0, 0, -1) or a hair from it, where Bevy keeps
        // the plain matrix or makes the oblique one
        let floor = Portal {
            up: FaceDirection::MinusZ,
            ..twin(FaceDirection::PlusY)
        };
        let ceiling = Portal {
            up: FaceDirection::PlusX,
            ..twin(FaceDirection::MinusY)
        };
        let centre = |portal: &Portal| Vec3::from(portal.frame().translation);
        for (portal, side) in [(floor, -1.0), (ceiling, 1.0)] {
            let at = centre(&portal);
            for up in [Vec3::X, Vec3::NEG_X, Vec3::Z, Vec3::NEG_Z] {
                let camera = Transform::from_translation(at + Vec3::Y * side * 4.0)
                    .looking_to(Vec3::Y * -side, up);
                let projection = portal_projection(&main_projection(), &camera, &portal);
                let wall = at + Vec3::Y * side * 0.5;
                let beyond = at - Vec3::Y * side * 0.5;
                assert!(
                    !is_drawn(depth(&projection, &camera, wall)),
                    "{:?} {up}",
                    portal.face
                );
                assert!(
                    is_drawn(depth(&projection, &camera, beyond)),
                    "{:?} {up}",
                    portal.face
                );
            }
        }
    }

    #[test]
    fn within_0_12_block_of_the_plane_the_projection_is_plain() {
        let camera =
            Transform::from_xyz(1.0, 11.5, 0.9).looking_to(Vec3::new(0.3, 0.0, 1.0), Vec3::Y);
        let projection =
            portal_projection(&main_projection(), &camera, &twin(FaceDirection::PlusZ));
        assert_eq!(
            projection.get_clip_from_view(),
            main_projection().get_clip_from_view()
        );
        // Further, it is not
        let camera = camera.with_translation(Vec3::new(1.0, 11.5, 0.8));
        let projection =
            portal_projection(&main_projection(), &camera, &twin(FaceDirection::PlusZ));
        assert_ne!(
            projection.get_clip_from_view(),
            main_projection().get_clip_from_view()
        );
    }
}
