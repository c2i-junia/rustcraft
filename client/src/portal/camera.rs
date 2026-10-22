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
use shared::world::{
    vertical_render_distance, within_render_distance, world_position_to_chunk_position, Portal,
    PORTAL_HEIGHT, PORTAL_VIEW_RADIUS, PORTAL_WIDTH,
};
use shared::CHUNK_SIZE;
use std::f32::consts::FRAC_PI_2;

use super::material::PortalMaterial;
use super::projection::portal_projection;
use super::shot::ShownPortals;
use super::{
    camera_layers, DrawnPortals, OwnView, PortalCamera, PortalRecursion, PORTAL_RECURSION,
};
use crate::camera::{CameraController, FOG_START, SKY_COLOR};
use crate::player::CurrentPlayerMarker;
use crate::state::GameState;
use crate::world::{ClientWorldMap, RenderDistance, ServerRenderDistanceLimit};

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
pub(super) const OPENING_REACH: f32 = 1.81;

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
    player: Query<&Transform, (With<CurrentPlayerMarker>, Without<PortalCamera>)>,
    render_distance: Option<Res<RenderDistance>>,
    server_limit: Option<Res<ServerRenderDistanceLimit>>,
    mut started: ResMut<PortalViewsStarted>,
    mut commands: Commands,
) {
    let Ok((main_transform, main_projection, main_camera, main_tonemapping, main_hdr, main_fog)) =
        main.single()
    else {
        return;
    };
    // The player's own chunks, as the server sends them (`ChunkReach`)
    let own = player
        .single()
        .ok()
        .zip(render_distance)
        .map(|(player, render)| {
            let distance = render
                .distance
                .min(server_limit.map_or(u32::MAX, |limit| limit.0));
            (
                world_position_to_chunk_position(player.translation),
                distance as i32,
            )
        });
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
            // The fog depends on how far the camera is from the twin and on where it looks, which
            // change as the player moves: it is set on every frame the camera draws
            let behind = above
                .transform
                .translation
                .distance(portal.frame().translation.into());
            let new_fog = main_fog.map(|main_fog| {
                let wide = match main_projection {
                    Projection::Perspective(perspective) => half_diagonal(perspective),
                    _ => FRAC_PI_2,
                };
                let reach = held_reach(twin, *transform.forward(), wide, own);
                portal_fog(main_fog, behind, reach)
            });
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

/// The angle from the axis of the view of `projection` to its corners.
pub(super) fn half_diagonal(projection: &PerspectiveProjection) -> f32 {
    let tan = (projection.fov / 2.0).tan();
    (tan * (1.0 + projection.aspect_ratio * projection.aspect_ratio).sqrt()).atan()
}

/// How far past the middle of `twin` the chunks the client holds reach, in blocks, as a camera
/// looking out of it `forward`, its view `half_diagonal` wide, can see them. The client holds
/// the chunks the server sends around the twin (`PORTAL_VIEW_RADIUS`), and, when the twin is
/// among them, the player's own: those within its render distance of the chunk `player.0`,
/// `player.1`. Each is a box of chunks, `vertical_render_distance` high, whose reach is that of
/// its nearest face, and is the larger of the two. A face is as far as the view can see it: its
/// distance over the cosine of the smallest angle between the view and the way to the face, so
/// that a side the view can never point at counts for more, and one it cannot turn to at all,
/// or that lies behind the twin, where nothing is drawn, not at all. The top and bottom of the
/// player's box count only nearer than they are to the player at most (`vertical_render_distance`
/// chunks), which the player's own fog does not count either (`underwater.rs`): a level view
/// through a close twin is not clamped by them, as the player's is not. A box reaching the
/// bedrock, at y = 0, has no bottom: nothing is seen under it. A ray crosses the opening up to
/// `OPENING_REACH` from its middle, so each face counts that much nearer. Through a close twin,
/// the reach is the player's render distance or more, and the portal camera gets the player's
/// fog (`portal_fog`).
pub(super) fn held_reach(
    twin: &Portal,
    forward: Vec3,
    half_diagonal: f32,
    player: Option<(IVec3, i32)>,
) -> f32 {
    let middle = Vec3::from(twin.frame().translation);
    let sent = (twin.chunk(), PORTAL_VIEW_RADIUS, f32::INFINITY);
    let own = player
        .filter(|&(center, distance)| within_render_distance(twin.chunk(), center, distance))
        .map(|(center, distance)| {
            let height = (vertical_render_distance(distance) * CHUNK_SIZE) as f32;
            (center, distance, height)
        });
    [Some(sent), own]
        .into_iter()
        .flatten()
        .map(|(center, radius, uncounted_from)| {
            let vertical = vertical_render_distance(radius);
            let reach = IVec3::new(radius, vertical, radius);
            let low = ((center - reach) * CHUNK_SIZE).as_vec3();
            let high = ((center + reach + 1) * CHUNK_SIZE).as_vec3();
            let counted = |distance: f32| {
                if distance < uncounted_from {
                    distance
                } else {
                    f32::INFINITY
                }
            };
            // Under the bedrock nothing is seen: a box reaching it has no bottom
            let below = if low.y > 0.0 {
                counted(middle.y - low.y)
            } else {
                f32::INFINITY
            };
            [
                (Vec3::NEG_X, middle.x - low.x),
                (Vec3::X, high.x - middle.x),
                (Vec3::NEG_Z, middle.z - low.z),
                (Vec3::Z, high.z - middle.z),
                (Vec3::NEG_Y, below),
                (Vec3::Y, counted(high.y - middle.y)),
            ]
            .into_iter()
            .filter(|(outward, _)| outward.dot(twin.normal()) > -0.5)
            .map(|(outward, distance)| {
                let off = (forward.angle_between(outward) - half_diagonal).max(0.0);
                if off < FRAC_PI_2 {
                    (distance - OPENING_REACH).max(0.0) / off.cos()
                } else {
                    f32::INFINITY
                }
            })
            .fold(f32::INFINITY, f32::min)
        })
        .fold(0.0, f32::max)
}

/// The fog of the camera of a portal whose player's camera, of fog `main`, stands `behind` blocks
/// from the portal, which puts the portal camera as far from the twin: it ends `reach` blocks
/// behind the twin, where the chunks held around it end (`held_reach`), and starts as the
/// player's does over its render distance. Never farther than the player's: a player sees no
/// farther through a portal than around itself, and under water the fog of the water, between
/// the eye and the portal, stays as near.
pub(super) fn portal_fog(main: &DistanceFog, behind: f32, reach: f32) -> DistanceFog {
    let falloff = match main.falloff {
        FogFalloff::Linear { start, end } => FogFalloff::Linear {
            start: start.min(behind + FOG_START * reach),
            end: end.min(behind + reach),
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
    use shared::world::{FaceDirection, PortalColor};

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

    /// A twin on the west face of a wall at x = 20, opening onto the chunk (1, 0, 0): the
    /// chunks sent around it reach x = -48 to the west, z = -64 and 80 to the sides.
    fn twin() -> Portal {
        Portal {
            owner: 1,
            color: PortalColor::Orange,
            anchor: IVec3::new(20, 10, 5),
            face: FaceDirection::MinusX,
            up: FaceDirection::PlusY,
        }
    }

    /// How far the middle of the twin is from z = 144, the south edge of the player's chunks
    /// around the chunk (0, 0, 0) with a render distance of 8.
    fn to_the_south_edge() -> f32 {
        144.0 - twin().frame().translation.z
    }

    /// The half-diagonal of the view of the player's camera on a 16:9 screen.
    fn wide() -> f32 {
        half_diagonal(&PerspectiveProjection {
            fov: 60f32.to_radians(),
            aspect_ratio: 16.0 / 9.0,
            ..default()
        })
    }

    fn assert_near(a: f32, b: f32) {
        assert!((a - b).abs() < 1e-3, "{a} against {b}");
    }

    #[test]
    fn the_half_diagonal_of_a_view_reaches_its_corners() {
        // tan 30° · √(1 + (16/9)²) = tan 49.66°
        assert!((wide().to_degrees() - 49.66).abs() < 0.01, "{}", wide());
    }

    #[test]
    fn through_a_far_twin_the_chunks_end_where_those_sent_around_it_end() {
        let twin = twin();
        assert_eq!(twin.chunk(), IVec3::new(1, 0, 0));
        // Looking west out of it, the west edge of the chunks sent, 68 blocks away, comes
        // first: the sides, some 70 and 75 blocks away, are only seen askew
        assert_near(
            held_reach(&twin, Vec3::NEG_X, wide(), None),
            68.0 - OPENING_REACH,
        );
        // The player is too far for its own chunks to be around the twin
        let far = Some((IVec3::new(20, 0, 0), 8));
        assert_near(
            held_reach(&twin, Vec3::NEG_X, wide(), far),
            68.0 - OPENING_REACH,
        );
    }

    #[test]
    fn through_a_close_twin_the_chunks_are_the_players_own() {
        // The player's own chunks, within its render distance of 8 of the chunk (0, 0, 0),
        // reach x = -128 to the west, z = -128 and 144 to the sides, which the view sees no more
        // directly than 40° off
        let own = Some((IVec3::ZERO, 8));
        let reach = held_reach(&twin(), Vec3::NEG_X, wide(), own);
        assert_near(reach, 148.0 - OPENING_REACH);
        // The player's fog reaches no farther: the portal camera gets it as it is
        let main = linear(SKY_COLOR, 0.6 * 128.0, 128.0);
        assert_eq!(
            falloff(&portal_fog(&main, 0.05, reach)),
            (0.6 * 128.0, 128.0)
        );
    }

    #[test]
    fn a_side_of_the_chunks_counts_as_much_as_the_view_can_turn_to_it() {
        let own = Some((IVec3::ZERO, 8));
        // Looking out of the twin 60° to the south, the wide view reaches straight south
        let askew = Vec3::new(-0.5, 0.0, 3f32.sqrt() / 2.0);
        assert_near(
            held_reach(&twin(), askew, wide(), own),
            to_the_south_edge() - OPENING_REACH,
        );
        // A view 10° wide sees the south no more directly than 20° off
        let narrow = 10f32.to_radians();
        assert_near(
            held_reach(&twin(), askew, narrow, own),
            (to_the_south_edge() - OPENING_REACH) / 20f32.to_radians().cos(),
        );
    }

    #[test]
    fn the_edge_of_the_chunks_behind_the_twin_is_never_seen() {
        // The player's chunks end 12 blocks behind the twin, at x = 32: a view all but along
        // the twin could turn to that edge, but nothing behind the twin is drawn
        let own = Some((IVec3::new(-7, 0, 0), 8));
        let along = Vec3::new(-0.05, 0.0, 1.0).normalize();
        assert_near(
            held_reach(&twin(), along, wide(), own),
            to_the_south_edge() - OPENING_REACH,
        );
    }

    /// A twin on a ceiling at `y`, opening down.
    fn ceiling(y: i32) -> Portal {
        Portal {
            owner: 1,
            color: PortalColor::Orange,
            anchor: IVec3::new(0, y, 0),
            face: FaceDirection::MinusY,
            up: FaceDirection::PlusZ,
        }
    }

    #[test]
    fn looking_down_out_of_a_far_ceiling_twin_the_chunks_end_at_the_bottom_of_those_sent() {
        // Its plane y = 150, opening onto the chunk (0, 9, 0): the chunks sent around it reach
        // y = 80 below, 70 blocks under its middle
        let ceiling = ceiling(150);
        assert_eq!(ceiling.chunk(), IVec3::new(0, 9, 0));
        assert_near(ceiling.frame().translation.y, 150.0);
        // The bottom, straight ahead, comes before the sides, some 65 blocks away but askew
        assert_near(
            held_reach(&ceiling, Vec3::NEG_Y, wide(), None),
            70.0 - OPENING_REACH,
        );
    }

    #[test]
    fn chunks_down_to_the_bedrock_have_no_bottom_to_see() {
        // Its plane y = 40: the chunks sent reach y = -32, under the bedrock at y = 0, which
        // closes the world. Only the sides count, 64 and more blocks away, seen 40° off at most
        let reach = held_reach(&ceiling(40), Vec3::NEG_Y, wide(), None);
        assert!(
            reach > (64.0 - OPENING_REACH) / 40f32.to_radians().cos(),
            "{reach}"
        );
    }

    #[test]
    fn the_bottom_of_the_players_chunks_counts_where_it_is_nearer_than_around_the_player() {
        // A render distance of 8 around the chunk (0, 10, 0) reaches y = 32: 118 blocks under
        // the twin, nearer than the 128 the player's own fog takes for granted, and farther
        // than the chunks sent
        let deep = Some((IVec3::new(0, 10, 0), 8));
        assert_near(
            held_reach(&ceiling(150), Vec3::NEG_Y, wide(), deep),
            118.0 - OPENING_REACH,
        );
        // Around the chunk (0, 14, 0), it reaches y = 96, 54 blocks under: the chunks sent
        // reach farther
        let shallow = Some((IVec3::new(0, 14, 0), 8));
        assert_near(
            held_reach(&ceiling(150), Vec3::NEG_Y, wide(), shallow),
            70.0 - OPENING_REACH,
        );
    }

    #[test]
    fn the_top_and_bottom_of_the_players_chunks_clamp_no_level_view() {
        // A render distance of 12 reaches 8 chunks up and down, 128 blocks: the twin in the
        // player's chunk has them 132.5 and 139.5 blocks away, which the player's own fog does
        // not count either. The west edge, 212 blocks away, is the reach
        let own = Some((IVec3::ZERO, 12));
        assert_near(
            held_reach(&twin(), Vec3::NEG_X, wide(), own),
            212.0 - OPENING_REACH,
        );
    }

    #[test]
    fn a_portal_cameras_fog_ends_where_the_chunks_held_end_behind_the_twin() {
        let main = linear(SKY_COLOR, 0.6 * 384.0, 384.0);
        let fog = portal_fog(&main, 5.0, 68.0);
        assert_eq!(fog.color, SKY_COLOR);
        assert_eq!(falloff(&fog), (5.0 + FOG_START * 68.0, 5.0 + 68.0));
    }

    #[test]
    fn under_water_or_with_a_short_render_distance_a_portal_cameras_fog_is_the_players() {
        let water = linear(Color::srgb_u8(30, 62, 130), 2.0, 16.0);
        assert_eq!(falloff(&portal_fog(&water, 5.0, 68.0)), (2.0, 16.0));
        let short = linear(SKY_COLOR, 0.6 * 32.0, 32.0);
        assert_eq!(falloff(&portal_fog(&short, 5.0, 68.0)), (0.6 * 32.0, 32.0));
    }
}
