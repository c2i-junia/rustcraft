//! The camera of an open portal: it stands where the player's camera would be if the space in
//! front of the portal went on behind its twin (`Portals::through`), and draws into an image the
//! size of the window, which the portal shows where it is on the screen.

use bevy::{
    camera::{
        primitives::Aabb,
        visibility::{NoCpuCulling, RenderLayers},
        Hdr, RenderTarget,
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
use super::shot::ShownPortals;
use super::{
    camera_layers, DrawnPortals, OwnView, PortalCamera, PortalRecursion, PORTAL_RECURSION,
};
use crate::camera::{CameraController, FOG_START, SKY_COLOR};
use crate::state::GameState;
use crate::world::ClientWorldMap;

/// An image a portal camera can draw into and a material can read, `size` in pixels. In 16-bit
/// floats: with the physical sky of the shaders, it holds colours brighter than white, before
/// the tonemapping of the player's camera (`shaders.rs`).
pub(super) fn portal_image(size: UVec2) -> Image {
    Image::new_target_texture(size.x, size.y, TextureFormat::Rgba16Float, None)
}

/// Spawns the camera of `level` of `portal`, drawing into `image`. It starts inactive:
/// `place_portal_cameras` turns it on when the portal is in view at that level.
pub(super) fn spawn_portal_camera(
    commands: &mut Commands,
    portal: &Portal,
    level: usize,
    image: Handle<Image>,
) -> Entity {
    commands
        .spawn((
            Camera3d::default(),
            Camera {
                // Before the level above, which shows its image the same frame, the player's
                // camera, of order 0, last
                order: -(level as isize),
                is_active: false,
                clear_color: ClearColorConfig::Custom(SKY_COLOR),
                ..default()
            },
            RenderTarget::Image(image.into()),
            // Set with the portals drawn by `place_portal_cameras`
            RenderLayers::none(),
            // The main camera's, set by `place_portal_cameras`
            Tonemapping::None,
            Projection::Perspective(default()),
            PortalCamera {
                owner: portal.owner,
                color: portal.color,
                level,
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

/// Whether some of the opening of `portal` lies in front of the plane of `twin`, which is the
/// near plane of the cameras of the portal: what lies behind it, they do not draw. (Bevy's
/// frustum leaves the oblique near plane out.)
pub(super) fn in_front_of_twin(portal: &Portal, twin: &Portal) -> bool {
    let half = Vec2::new(PORTAL_WIDTH as f32, PORTAL_HEIGHT as f32) / 2.0;
    [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)]
        .into_iter()
        .map(|(x, y)| {
            portal
                .frame()
                .transform_point3(Vec3::new(x * half.x, y * half.y, 0.0))
        })
        .any(|corner| twin.signed_distance(corner) > 0.0)
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

/// What a camera that portal cameras are placed after is: the player's camera, or a portal
/// camera of the level above.
struct Eye {
    transform: Transform,
    projection: Projection,
    fog: Option<DistanceFog>,
    /// Whether its near plane is the twin's plane: a portal camera's is.
    behind_twin: bool,
}

/// Places the cameras of each open portal, level after level, each behind the twin of the
/// portal, where the camera of the level above would be through the portal (the player's
/// camera for level 1), with the twin's plane as its near plane, the sky and the tonemapping of
/// the player's camera (none with HDR, where the player's camera tonemaps the portals), and its
/// fog (`portal_fog`). A camera draws when the camera of the level
/// above draws (the player's always does) and sees the front of the portal, within its fog and
/// in front of its near plane; and its level is drawn (`PortalRecursion`). A camera sees its
/// own portal one level deeper when that level draws, else in its colour, or not at all beyond
/// its fog (`camera_layers`).
pub(super) fn place_portal_cameras(
    world_map: Res<ClientWorldMap>,
    shown: Res<ShownPortals>,
    drawn: Res<DrawnPortals>,
    recursion: Res<PortalRecursion>,
    main: Query<
        (
            &Transform,
            &Projection,
            &Camera,
            &Tonemapping,
            Has<Hdr>,
            Option<&DistanceFog>,
        ),
        (With<CameraController>, Without<PortalCamera>),
    >,
    mut cameras: Query<
        (
            Entity,
            &mut Transform,
            &mut Projection,
            &mut Camera,
            &mut Tonemapping,
            Option<&mut DistanceFog>,
            &mut RenderLayers,
        ),
        (With<PortalCamera>, Without<CameraController>),
    >,
    mut started: ResMut<PortalViewsStarted>,
    mut commands: Commands,
) {
    let Ok((main_transform, main_projection, main_camera, main_tonemapping, main_hdr, main_fog)) =
        main.single()
    else {
        return;
    };
    let portals = shown.get(world_map.blocks.portals());
    let depth = recursion.0.clamp(1, PORTAL_RECURSION);
    let slots: Vec<usize> = drawn.0.values().map(|drawn| drawn.slot).collect();
    for drawn in drawn.0.values() {
        let Some(views) = &drawn.views else {
            continue;
        };
        let open = portals
            .get(drawn.portal.owner, drawn.portal.color)
            .and_then(|portal| Some((portal, portals.twin(portal)?, portals.through(portal)?)));
        // Where each level's camera stands, if it draws, the player's camera first
        let mut eyes = vec![Some(Eye {
            transform: *main_transform,
            projection: main_projection.clone(),
            fog: main_fog.cloned(),
            behind_twin: false,
        })];
        for (index, entity) in views.cameras.iter().enumerate() {
            let level = index + 1;
            let Ok((entity, mut transform, mut projection, mut camera, mut tonemapping, fog, _)) =
                cameras.get_mut(*entity)
            else {
                eyes.push(None);
                continue;
            };
            let seen = match (&eyes[index], open) {
                (Some(eye), Some((portal, twin, through))) if level <= depth => {
                    (sees(&eye.transform, &eye.projection, portal)
                        && (!eye.behind_twin || in_front_of_twin(portal, twin))
                        && !beyond_fog(eye.transform.translation, eye.fog.as_ref(), portal))
                    .then_some((eye, portal, twin, through))
                }
                _ => None,
            };
            let active = seen.is_some();
            if camera.is_active != active {
                camera.is_active = active;
                started.0 |= active;
            }
            let Some((above, portal, twin, through)) = seen else {
                eyes.push(None);
                continue;
            };
            *transform =
                Transform::from_matrix(Mat4::from(through * above.transform.compute_affine()));
            if let Projection::Perspective(perspective) = main_projection {
                *projection =
                    Projection::Perspective(portal_projection(perspective, &transform, twin));
            }
            camera.clear_color = main_camera.clear_color;
            // Without HDR, each material tonemaps what it draws, and the portal only copies its
            // image: the image must be tonemapped already, as the rest of the screen is. With
            // HDR, the main camera tonemaps the whole screen, the portals included, once
            tonemapping.set_if_neq(if main_hdr {
                Tonemapping::None
            } else {
                *main_tonemapping
            });
            // The fog depends on how far the camera is from the twin, which changes as the player
            // moves: it is set on every frame the camera draws
            let behind = above
                .transform
                .translation
                .distance(portal.frame().translation.into());
            let new_fog = main_fog.map(|main_fog| portal_fog(main_fog, behind));
            match (&new_fog, fog) {
                (Some(new_fog), Some(mut fog)) => *fog = new_fog.clone(),
                (Some(new_fog), None) => {
                    commands.entity(entity).insert(new_fog.clone());
                }
                (None, Some(_)) => {
                    commands.entity(entity).remove::<DistanceFog>();
                }
                (None, None) => {}
            }
            eyes.push(Some(Eye {
                transform: *transform,
                projection: projection.clone(),
                fog: new_fog,
                behind_twin: true,
            }));
        }

        // What each camera that draws sees of its own portal: the level below if it draws
        for (index, entity) in views.cameras.iter().enumerate() {
            let level = index + 1;
            let (Some(eye), Ok((.., mut layers))) = (&eyes[level], cameras.get_mut(*entity)) else {
                continue;
            };
            let deeper = eyes.get(level + 1).is_some_and(Option::is_some);
            let own = if deeper {
                OwnView::Deeper(level)
            } else if beyond_fog(eye.transform.translation, eye.fog.as_ref(), &drawn.portal) {
                OwnView::Hidden
            } else {
                OwnView::Colour
            };
            layers.set_if_neq(camera_layers(drawn.slot, own, slots.iter().copied()));
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

/// Gives the open portals new images when the size of the window changed, to their cameras
/// and the surfaces showing their views.
pub(super) fn resize_portal_images(
    windows: Query<&Window, With<PrimaryWindow>>,
    mut drawn: ResMut<DrawnPortals>,
    mut targets: Query<&mut RenderTarget, With<PortalCamera>>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<PortalMaterial>>,
) {
    let Ok(window) = windows.single() else {
        return;
    };
    let size = window.physical_size().max(UVec2::ONE);
    for views in drawn
        .0
        .values_mut()
        .filter_map(|drawn| drawn.views.as_mut())
    {
        if images.get(&views.images[0]).map(Image::size) == Some(size) {
            continue;
        }
        views.images = [(); 2].map(|_| images.add(portal_image(size)));
        for level in 1..=PORTAL_RECURSION {
            let image = views.image(level).clone();
            if let Ok(mut target) = targets.get_mut(views.cameras[level - 1]) {
                *target = RenderTarget::Image(image.clone().into());
            }
            if let Some(mut material) = materials.get_mut(&views.materials[level - 1]) {
                material.image = Some(image);
            }
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
