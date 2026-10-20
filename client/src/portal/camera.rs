//! The camera of an open portal: it stands where the player's camera would be if the space in
//! front of the portal went on behind its twin (`Portals::through`), and draws into an image the
//! size of the window, which the portal shows where it is on the screen.

use bevy::{
    camera::{
        primitives::Aabb,
        visibility::{NoCpuCulling, RenderLayers},
        RenderTarget,
    },
    core_pipeline::tonemapping::Tonemapping,
    prelude::*,
    render::render_resource::TextureFormat,
    window::PrimaryWindow,
};
use shared::world::{Portal, PORTAL_HEIGHT, PORTAL_VIEW_RADIUS, PORTAL_WIDTH};
use shared::CHUNK_SIZE;

use super::material::PortalMaterial;
use super::projection::portal_projection;
use super::{DrawnPortals, PortalCamera, PortalSurface, PORTAL_CHUNKS_LAYER, STAND_IN_LAYER};
use crate::camera::{CameraController, FOG_START, SKY_COLOR};
use crate::state::GameState;
use crate::world::ClientWorldMap;

/// An image a portal camera can draw into and a material can read, `size` in pixels.
pub(super) fn portal_image(size: UVec2) -> Image {
    Image::new_target_texture(size.x, size.y, TextureFormat::Rgba8UnormSrgb, None)
}

/// Spawns the camera of `portal`, drawing into `image`. It starts inactive:
/// `place_portal_cameras` turns it on when the portal is in view.
pub(super) fn spawn_portal_camera(
    commands: &mut Commands,
    portal: &Portal,
    image: Handle<Image>,
) -> Entity {
    commands
        .spawn((
            Camera3d::default(),
            Camera {
                // Before the main camera, which shows its image the same frame
                order: -1,
                is_active: false,
                clear_color: ClearColorConfig::Custom(SKY_COLOR),
                ..default()
            },
            RenderTarget::Image(image.into()),
            // The world, the chunks the player has only for the portals, and the colours of the
            // portals
            RenderLayers::from_layers(&[0, STAND_IN_LAYER, PORTAL_CHUNKS_LAYER]),
            // The main camera's, set by `place_portal_cameras`
            Tonemapping::None,
            Projection::Perspective(default()),
            PortalCamera {
                owner: portal.owner,
                color: portal.color,
            },
            DespawnOnExit(GameState::Game),
        ))
        .id()
}

/// Whether the camera at `transform`, of projection `projection`, sees the front of `portal`:
/// it is in front of the portal's plane, and the box of the portal is in its frustum.
pub(super) fn sees(transform: &Transform, projection: &Projection, portal: &Portal) -> bool {
    let half = Vec3::new(PORTAL_WIDTH as f32 / 2.0, PORTAL_HEIGHT as f32 / 2.0, 0.5);
    let opening = Aabb {
        center: Vec3::new(0.0, 0.0, -0.5).into(),
        half_extents: half.into(),
    };
    portal.signed_distance(transform.translation) > 0.0
        && projection
            .compute_frustum(&GlobalTransform::from(*transform))
            .intersects_obb(&opening, &portal.frame(), true, false)
}

/// How far from its centre a portal's opening reaches: half its diagonal.
const OPENING_REACH: f32 = 1.81;

/// Whether `portal` is wholly beyond the end of the fog `fog` of a camera at `eye`, where
/// nothing of it can be seen.
pub(super) fn beyond_fog(eye: Vec3, fog: Option<&DistanceFog>, portal: &Portal) -> bool {
    let Some(FogFalloff::Linear { end, .. }) = fog.map(|fog| &fog.falloff) else {
        return false;
    };
    eye.distance(portal.frame().translation.into()) - OPENING_REACH > *end
}

/// Hides the portals wholly beyond the fog of the player's camera: drawn unfogged, they would
/// show on the sky. A portal camera beyond it then does not see them either, which only matters
/// for a portal far from the player near the twin of another.
pub(super) fn hide_portals_beyond_the_fog(
    drawn: Res<DrawnPortals>,
    main: Query<(&Transform, Option<&DistanceFog>), With<CameraController>>,
    mut roots: Query<&mut Visibility>,
) {
    let Ok((main_transform, main_fog)) = main.single() else {
        return;
    };
    for drawn in drawn.0.values() {
        let Ok(mut visibility) = roots.get_mut(drawn.root) else {
            continue;
        };
        let shown = if beyond_fog(main_transform.translation, main_fog, &drawn.portal) {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        };
        visibility.set_if_neq(shown);
    }
}

/// Places each portal camera behind the twin of its portal, after the player's camera, with
/// the twin's plane as its near plane, the sky and the tonemapping of the player's camera, and
/// its fog (`portal_fog`). A camera whose portal is out of view, seen from behind, or beyond the
/// player's fog, does not draw.
pub(super) fn place_portal_cameras(
    world_map: Res<ClientWorldMap>,
    main: Query<
        (
            &Transform,
            &Projection,
            &Camera,
            &Tonemapping,
            Option<&DistanceFog>,
        ),
        (With<CameraController>, Without<PortalCamera>),
    >,
    mut cameras: Query<
        (
            Entity,
            &PortalCamera,
            &mut Transform,
            &mut Projection,
            &mut Camera,
            &mut Tonemapping,
            Option<&mut DistanceFog>,
        ),
        Without<CameraController>,
    >,
    mut started: ResMut<PortalViewsStarted>,
    mut commands: Commands,
) {
    let Ok((main_transform, main_projection, main_camera, main_tonemapping, main_fog)) =
        main.single()
    else {
        return;
    };
    let portals = world_map.blocks.portals();
    for (entity, camera_of, mut transform, mut projection, mut camera, mut tonemapping, fog) in
        &mut cameras
    {
        let open = portals
            .get(camera_of.owner, camera_of.color)
            .and_then(|portal| Some((portal, portals.twin(portal)?, portals.through(portal)?)));
        let Some((portal, twin, through)) = open else {
            camera.is_active = false;
            continue;
        };
        let active = sees(main_transform, main_projection, portal)
            && !beyond_fog(main_transform.translation, main_fog, portal);
        if camera.is_active != active {
            camera.is_active = active;
            started.0 |= active;
        }
        if !active {
            continue;
        }
        *transform = Transform::from_matrix(Mat4::from(through * main_transform.compute_affine()));
        if let Projection::Perspective(perspective) = main_projection {
            *projection = Projection::Perspective(portal_projection(perspective, &transform, twin));
        }
        camera.clear_color = main_camera.clear_color;
        // The main camera draws without HDR, so each material tonemaps what it draws, and the
        // portal's only copies its image: the image must be tonemapped already, as the rest of
        // the screen is
        tonemapping.set_if_neq(*main_tonemapping);
        // The fog depends on how far the camera is from the twin, which changes as the player
        // moves: it is set on every frame the camera draws
        let behind = main_transform
            .translation
            .distance(portal.frame().translation.into());
        match (main_fog, fog) {
            (Some(main_fog), Some(mut fog)) => *fog = portal_fog(main_fog, behind),
            (Some(main_fog), None) => {
                commands.entity(entity).insert(portal_fog(main_fog, behind));
            }
            (None, Some(_)) => {
                commands.entity(entity).remove::<DistanceFog>();
            }
            (None, None) => {}
        }
    }
}

/// Whether a portal camera started drawing this frame.
#[derive(Resource, Default)]
pub(super) struct PortalViewsStarted(bool);

/// Marks a mesh whose `NoCpuCulling` was taken away for a frame by `show_chunks_to_new_views`.
#[derive(Component)]
pub(crate) struct CpuCulledForAFrame;

/// Shows the chunks to the portal cameras that started drawing. Bevy 0.19 hands a mesh culled
/// on the GPU only (`NoCpuCulling`, as the chunks are) to the views there are when it becomes
/// visible, and a camera that is turned on starts with none: it would draw no chunk. So when a
/// portal camera is turned on, the chunks are culled on the CPU for a frame, which every view
/// draws them through, and then given back their `NoCpuCulling`, which hands them to every view
/// anew. The meshes already culled on the CPU for a frame, which would be given their
/// `NoCpuCulling` back too late for this frame's query, stay culled on the CPU for another.
pub(super) fn show_chunks_to_new_views(
    mut started: ResMut<PortalViewsStarted>,
    gpu_culled: Query<Entity, With<NoCpuCulling>>,
    cpu_culled_for_a_frame: Query<Entity, With<CpuCulledForAFrame>>,
    mut commands: Commands,
) {
    if std::mem::take(&mut started.0) {
        for entity in &gpu_culled {
            commands
                .entity(entity)
                .remove::<NoCpuCulling>()
                .insert(CpuCulledForAFrame);
        }
        return;
    }
    for entity in &cpu_culled_for_a_frame {
        commands
            .entity(entity)
            .remove::<CpuCulledForAFrame>()
            .insert(NoCpuCulling);
    }
}

/// Gives the portal cameras new images when the size of the window changed, and the surfaces
/// of their portals with them.
pub(super) fn resize_portal_images(
    windows: Query<&Window, With<PrimaryWindow>>,
    mut cameras: Query<(&PortalCamera, &mut RenderTarget)>,
    surfaces: Query<(&PortalSurface, &MeshMaterial3d<PortalMaterial>)>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<PortalMaterial>>,
) {
    let Ok(window) = windows.single() else {
        return;
    };
    let size = window.physical_size().max(UVec2::ONE);
    for (camera_of, mut target) in &mut cameras {
        let RenderTarget::Image(image) = &*target else {
            continue;
        };
        if images.get(&image.handle).map(Image::size) == Some(size) {
            continue;
        }
        let image = images.add(portal_image(size));
        *target = RenderTarget::Image(image.clone().into());
        let surface = surfaces.iter().find(|(surface, _)| {
            surface.owner == camera_of.owner && surface.color == camera_of.color
        });
        if let Some(mut material) = surface.and_then(|(_, material)| materials.get_mut(&material.0))
        {
            material.image = Some(image);
        }
    }
}

/// The fog of the camera of a portal whose player's camera, of fog `main`, stands `behind` blocks
/// from the portal, which puts the portal camera as far from the twin: it ends
/// `PORTAL_VIEW_RADIUS` chunks behind the twin, where the chunks sent around it end, and starts
/// as the player's does over its render distance. Never farther than the player's: a player
/// sees no farther through a portal than around itself, and under water the fog of the water,
/// between the eye and the portal, stays as near.
pub(super) fn portal_fog(main: &DistanceFog, behind: f32) -> DistanceFog {
    let view = (PORTAL_VIEW_RADIUS * CHUNK_SIZE) as f32;
    let falloff = match main.falloff {
        FogFalloff::Linear { start, end } => FogFalloff::Linear {
            start: start.min(behind + FOG_START * view),
            end: end.min(behind + view),
        },
        ref other => other.clone(),
    };
    DistanceFog {
        falloff,
        ..main.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn linear(color: Color, start: f32, end: f32) -> DistanceFog {
        DistanceFog {
            color,
            falloff: FogFalloff::Linear { start, end },
            ..default()
        }
    }

    fn falloff(fog: &DistanceFog) -> (f32, f32) {
        let FogFalloff::Linear { start, end } = fog.falloff else {
            panic!("a linear fog");
        };
        (start, end)
    }

    const VIEW: f32 = (PORTAL_VIEW_RADIUS * CHUNK_SIZE) as f32;

    #[test]
    fn a_portal_cameras_fog_ends_the_view_radius_behind_the_twin() {
        let main = linear(SKY_COLOR, 0.6 * 384.0, 384.0);
        let fog = portal_fog(&main, 5.0);
        assert_eq!(fog.color, SKY_COLOR);
        assert_eq!(falloff(&fog), (5.0 + FOG_START * VIEW, 5.0 + VIEW));
    }

    #[test]
    fn under_water_or_with_a_short_render_distance_a_portal_cameras_fog_is_the_players() {
        let water = linear(Color::srgb_u8(30, 62, 130), 2.0, 16.0);
        assert_eq!(falloff(&portal_fog(&water, 5.0)), (2.0, 16.0));
        let short = linear(SKY_COLOR, 0.6 * 32.0, 32.0);
        assert_eq!(falloff(&portal_fog(&short, 5.0)), (0.6 * 32.0, 32.0));
    }
}
