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
use shared::world::{Portal, PORTAL_HEIGHT, PORTAL_WIDTH};

use super::material::PortalMaterial;
use super::projection::portal_projection;
use super::{PortalCamera, PortalSurface, STAND_IN_LAYER};
use crate::camera::{CameraController, SKY_COLOR};
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
            RenderLayers::from_layers(&[0, STAND_IN_LAYER]),
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

/// Places each portal camera behind the twin of its portal, after the player's camera, with
/// the twin's plane as its near plane, and the fog, the sky and the tonemapping of the player's
/// camera. A camera whose portal is out of view, or seen from behind, does not draw.
pub(super) fn place_portal_cameras(
    world_map: Res<ClientWorldMap>,
    main: Query<
        (
            &Transform,
            &Projection,
            &Camera,
            &Tonemapping,
            Option<Ref<DistanceFog>>,
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
            Has<DistanceFog>,
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
    for (entity, camera_of, mut transform, mut projection, mut camera, mut tonemapping, has_fog) in
        &mut cameras
    {
        let open = portals
            .get(camera_of.owner, camera_of.color)
            .and_then(|portal| Some((portal, portals.twin(portal)?, portals.through(portal)?)));
        let Some((portal, twin, through)) = open else {
            camera.is_active = false;
            continue;
        };
        let active = sees(main_transform, main_projection, portal);
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
        match &main_fog {
            Some(fog) if fog.is_changed() || !has_fog => {
                commands.entity(entity).insert(DistanceFog::clone(fog));
            }
            None if has_fog => {
                commands.entity(entity).remove::<DistanceFog>();
            }
            _ => {}
        }
    }
}

/// Whether a portal camera started drawing this frame.
#[derive(Resource, Default)]
pub(super) struct PortalViewsStarted(bool);

/// Marks a mesh whose `NoCpuCulling` was taken away for a frame by `show_chunks_to_new_views`.
#[derive(Component)]
pub(super) struct CpuCulledForAFrame;

/// Shows the chunks to the portal cameras that started drawing. Bevy 0.19 hands a mesh culled
/// on the GPU only (`NoCpuCulling`, as the chunks are) to the views there are when it becomes
/// visible, and a camera that is turned on starts with none: it would draw no chunk. So when a
/// portal camera is turned on, the chunks are culled on the CPU for a frame, which every view
/// draws them through, and then given back their `NoCpuCulling`, which hands them to every view
/// anew.
pub(super) fn show_chunks_to_new_views(
    mut started: ResMut<PortalViewsStarted>,
    gpu_culled: Query<Entity, With<NoCpuCulling>>,
    cpu_culled_for_a_frame: Query<Entity, With<CpuCulledForAFrame>>,
    mut commands: Commands,
) {
    for entity in &cpu_culled_for_a_frame {
        commands
            .entity(entity)
            .remove::<CpuCulledForAFrame>()
            .insert(NoCpuCulling);
    }
    if std::mem::take(&mut started.0) {
        for entity in &gpu_culled {
            commands
                .entity(entity)
                .remove::<NoCpuCulling>()
                .insert(CpuCulledForAFrame);
        }
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
