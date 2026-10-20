//! The portals, drawn. An open portal shows what its camera sees from behind its twin; a lone
//! one shows its colour. Each portal is a box one block deep behind the face of its wall, drawn
//! by its inner faces, so that the near plane of the player's camera never cuts it while the
//! player passes, and a glowing rim on the wall around it.
//!
//! The portals are the client map's (`ClientWorldMap::blocks.portals()`): what is drawn follows
//! them, frame after frame.

pub mod camera;
pub mod material;
pub mod projection;

use std::collections::HashMap;

use bevy::{
    asset::RenderAssetUsages,
    camera::visibility::RenderLayers,
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
    transform::TransformSystems,
};
use shared::messages::PlayerId;
use shared::world::{global_block_to_chunk_pos, Portal, PortalColor, PORTAL_HEIGHT, PORTAL_WIDTH};

use crate::state::{GameSet, GameState};
use crate::world::{ClientWorldMap, WorldRenderRequestUpdateEvent};
use camera::{
    place_portal_cameras, portal_image, resize_portal_images, show_chunks_to_new_views,
    spawn_portal_camera, PortalViewsStarted,
};
use material::{portal_material_plugin, PortalLook, PortalMaterial};

/// The layer of the surfaces that show the view through a portal: the main camera's only. A
/// portal camera never draws them, as none may sample the image it is drawing (wgpu forbids
/// it for its own), and this is one level of recursion: through a portal, portals show their
/// colour.
pub const SURFACE_LAYER: usize = 1;
/// The layer of the stand-ins of the portals, in their colour: the portal cameras' only.
pub const STAND_IN_LAYER: usize = 2;

/// How far the rim reaches out on the wall from the edge of the opening, in blocks.
const RIM_WIDTH: f32 = 0.22;
/// How far the box stops short of the wall cells, so that its faces never meet theirs.
const BOX_INSET: f32 = 0.002;

/// The colour of a portal on screen.
pub fn portal_color(color: PortalColor) -> Color {
    match color {
        PortalColor::Blue => Color::srgb_u8(0x2f, 0x8c, 0xff),
        PortalColor::Orange => Color::srgb_u8(0xff, 0x8a, 0x1f),
    }
}

/// What is drawn of a portal: its entities, as they were spawned for it.
struct DrawnPortal {
    portal: Portal,
    /// Whether the portal had a twin when it was drawn.
    open: bool,
    /// The frame of the portal, parent of its surface, its stand-in and its rim.
    root: Entity,
    /// The camera of an open portal.
    camera: Option<Entity>,
}

/// The portals drawn, by owner and colour.
#[derive(Resource, Default)]
pub struct DrawnPortals(HashMap<(PlayerId, PortalColor), DrawnPortal>);

/// The camera of a portal, which sees what the portal shows.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct PortalCamera {
    pub owner: PlayerId,
    pub color: PortalColor,
}

/// The surface of a portal that the main camera sees: the view through it, or its colour.
#[derive(Component)]
pub struct PortalSurface {
    pub owner: PlayerId,
    pub color: PortalColor,
}

/// The portals drawn in the game, after the client map's.
pub fn portal_plugin(app: &mut App) {
    app.add_plugins(portal_material_plugin);
    portal_systems(app);
}

/// The systems of `portal_plugin`, without the rendering of the material.
fn portal_systems(app: &mut App) {
    app.init_resource::<DrawnPortals>()
        .init_resource::<PortalViewsStarted>()
        .add_systems(
            PostUpdate,
            (
                sync_portals,
                resize_portal_images,
                place_portal_cameras,
                show_chunks_to_new_views,
            )
                .chain()
                .before(GameSet::Meshing)
                .before(TransformSystems::Propagate)
                .before(bevy::camera::CameraUpdateSystems)
                .run_if(in_state(GameState::Game)),
        )
        .add_systems(
            OnExit(GameState::Game),
            |mut drawn: ResMut<DrawnPortals>| {
                // Their entities go with the game
                drawn.0.clear();
            },
        );
}

/// Draws the portals of the client map as they are: a portal that appeared, moved, or opened or
/// closed is drawn anew, one that is gone is no longer drawn, and the wall cells of both are
/// meshed again, the face a portal covers being left out.
fn sync_portals(
    world_map: Res<ClientWorldMap>,
    mut drawn: ResMut<DrawnPortals>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<PortalMaterial>>,
    mut images: ResMut<Assets<Image>>,
    mut remesh: MessageWriter<WorldRenderRequestUpdateEvent>,
    mut commands: Commands,
) {
    let portals = world_map.blocks.portals();
    let mut walls = Vec::new();
    drawn.0.retain(|(owner, color), drawn| {
        let current = portals.get(*owner, *color);
        let unchanged = current.is_some_and(|portal| {
            *portal == drawn.portal && portals.twin(portal).is_some() == drawn.open
        });
        if !unchanged {
            commands.entity(drawn.root).despawn();
            if let Some(camera) = drawn.camera {
                commands.entity(camera).despawn();
            }
            walls.extend(drawn.portal.cells());
        }
        unchanged
    });

    let size = windows
        .single()
        .map_or(UVec2::ONE, |window| window.physical_size().max(UVec2::ONE));
    for portal in portals.iter() {
        if drawn.0.contains_key(&(portal.owner, portal.color)) {
            continue;
        }
        let open = portals.twin(portal).is_some();
        let color = portal_color(portal.color);
        let (surface_look, image) = if open {
            (PortalLook::View, Some(images.add(portal_image(size))))
        } else {
            (PortalLook::Color, None)
        };
        let surface_material =
            materials.add(PortalMaterial::new(surface_look, color, image.clone()));
        let stand_in = materials.add(PortalMaterial::new(PortalLook::Color, color, None));
        let rim = materials.add(PortalMaterial::new(PortalLook::Rim, color, None));
        let box_mesh = meshes.add(portal_box());
        let root = commands
            .spawn((
                Transform::from_matrix(Mat4::from(portal.frame())),
                Visibility::default(),
                DespawnOnExit(GameState::Game),
                children![
                    (
                        Mesh3d(box_mesh.clone()),
                        MeshMaterial3d(surface_material),
                        BOX_TRANSFORM,
                        RenderLayers::layer(SURFACE_LAYER),
                        PortalSurface {
                            owner: portal.owner,
                            color: portal.color,
                        },
                    ),
                    (
                        Mesh3d(box_mesh),
                        MeshMaterial3d(stand_in),
                        BOX_TRANSFORM,
                        RenderLayers::layer(STAND_IN_LAYER),
                    ),
                    (Mesh3d(meshes.add(rim_mesh())), MeshMaterial3d(rim)),
                ],
            ))
            .id();
        let camera = image.map(|image| spawn_portal_camera(&mut commands, portal, image));
        drawn.0.insert(
            (portal.owner, portal.color),
            DrawnPortal {
                portal: *portal,
                open,
                root,
                camera,
            },
        );
        walls.extend(portal.cells());
    }

    let mut chunks: Vec<IVec3> = walls.iter().map(global_block_to_chunk_pos).collect();
    chunks.sort_by_key(|chunk| chunk.to_array());
    chunks.dedup();
    remesh.write_batch(
        chunks
            .into_iter()
            .map(WorldRenderRequestUpdateEvent::ChunkToReload),
    );
}

/// Where the box of a portal is in its frame: behind the face of the wall.
const BOX_TRANSFORM: Transform = Transform::from_xyz(0.0, 0.0, -0.5);

/// The box of a portal, one block deep, a little smaller than its wall cells.
fn portal_box() -> Mesh {
    Cuboid::new(
        PORTAL_WIDTH as f32 - 2.0 * BOX_INSET,
        PORTAL_HEIGHT as f32 - 2.0 * BOX_INSET,
        1.0 - 2.0 * BOX_INSET,
    )
    .into()
}

/// The rim of a portal: a ring on the face of the wall around the opening, a hair in front of
/// it. Its `UV_0.x` goes from 0 at the opening to 1 at its outer edge.
fn rim_mesh() -> Mesh {
    let half = Vec2::new(PORTAL_WIDTH as f32, PORTAL_HEIGHT as f32) / 2.0;
    let corners = [
        Vec2::new(-1.0, -1.0),
        Vec2::new(1.0, -1.0),
        Vec2::new(1.0, 1.0),
        Vec2::new(-1.0, 1.0),
    ];
    let mut positions = Vec::new();
    let mut uvs = Vec::new();
    for (size, edge) in [(half, 0.0), (half + RIM_WIDTH, 1.0)] {
        for corner in corners {
            positions.push((corner * size).extend(0.01).to_array());
            uvs.push([edge, 0.0]);
        }
    }
    let indices = (0..4u16)
        .flat_map(|k| {
            let n = (k + 1) % 4;
            [k, 4 + k, 4 + n, k, 4 + n, n]
        })
        .collect();
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0, 0.0, 1.0]; 8])
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
    .with_inserted_indices(Indices::U16(indices))
}

#[cfg(test)]
mod tests {
    use bevy::{core_pipeline::tonemapping::Tonemapping, state::app::StatesPlugin};
    use shared::world::FaceDirection;

    use super::*;
    use crate::camera::CameraController;

    const OWNER: PlayerId = 7;

    fn blue() -> Portal {
        Portal {
            owner: OWNER,
            color: PortalColor::Blue,
            anchor: IVec3::new(0, 10, 0),
            face: FaceDirection::PlusZ,
        }
    }

    fn orange() -> Portal {
        Portal {
            owner: OWNER,
            color: PortalColor::Orange,
            anchor: IVec3::new(20, 10, 5),
            face: FaceDirection::MinusX,
        }
    }

    /// The portal systems in a game, with the player's camera five blocks in front of the blue
    /// portal, looking at it.
    fn app() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, StatesPlugin, AssetPlugin::default()))
            .init_asset::<Mesh>()
            .init_asset::<Image>()
            .init_asset::<PortalMaterial>()
            .init_state::<GameState>()
            .init_resource::<ClientWorldMap>()
            .add_message::<WorldRenderRequestUpdateEvent>();
        portal_systems(&mut app);
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Game);
        app.world_mut().spawn((
            Camera::default(),
            Tonemapping::AcesFitted,
            Projection::Perspective(PerspectiveProjection {
                fov: 60f32.to_radians(),
                aspect_ratio: 16.0 / 9.0,
                ..default()
            }),
            Transform::from_xyz(1.0, 11.5, 6.0),
            CameraController::default(),
        ));
        app.update();
        app
    }

    fn insert(app: &mut App, portal: Portal) {
        app.world_mut()
            .resource_mut::<ClientWorldMap>()
            .blocks
            .portals_mut()
            .insert(portal);
    }

    /// The look of the surface of each portal drawn.
    fn surfaces(app: &mut App) -> Vec<(PortalColor, PortalLook)> {
        let world = app.world_mut();
        let surfaces: Vec<_> = world
            .query::<(&PortalSurface, &MeshMaterial3d<PortalMaterial>)>()
            .iter(world)
            .map(|(surface, material)| (surface.color, material.0.clone()))
            .collect();
        let materials = world.resource::<Assets<PortalMaterial>>();
        let mut looks: Vec<_> = surfaces
            .into_iter()
            .map(|(color, material)| (color, materials.get(&material).unwrap().look))
            .collect();
        looks.sort_by_key(|(color, _)| *color as u8);
        looks
    }

    fn camera_of(app: &mut App, color: PortalColor) -> Option<(Transform, Camera, Projection)> {
        let world = app.world_mut();
        world
            .query::<(&PortalCamera, &Transform, &Camera, &Projection)>()
            .iter(world)
            .find(|(camera, ..)| camera.color == color)
            .map(|(_, transform, camera, projection)| {
                (*transform, camera.clone(), projection.clone())
            })
    }

    fn cameras(app: &mut App) -> usize {
        let world = app.world_mut();
        world.query::<&PortalCamera>().iter(world).count()
    }

    fn place_player_camera(app: &mut App, transform: Transform) {
        let world = app.world_mut();
        let mut camera = world
            .query_filtered::<&mut Transform, With<CameraController>>()
            .single_mut(world)
            .unwrap();
        *camera = transform;
    }

    #[test]
    fn an_open_pair_gets_two_surfaces_and_two_cameras() {
        let mut app = app();
        insert(&mut app, blue());
        insert(&mut app, orange());
        app.update();
        assert_eq!(
            surfaces(&mut app),
            [
                (PortalColor::Blue, PortalLook::View),
                (PortalColor::Orange, PortalLook::View)
            ]
        );
        assert_eq!(cameras(&mut app), 2);
    }

    #[test]
    fn a_lone_portal_shows_its_colour_and_has_no_camera() {
        let mut app = app();
        insert(&mut app, blue());
        app.update();
        assert_eq!(surfaces(&mut app), [(PortalColor::Blue, PortalLook::Color)]);
        assert_eq!(cameras(&mut app), 0);
    }

    #[test]
    fn removing_the_twin_closes_the_portal() {
        let mut app = app();
        insert(&mut app, blue());
        insert(&mut app, orange());
        app.update();
        app.world_mut()
            .resource_mut::<ClientWorldMap>()
            .blocks
            .portals_mut()
            .remove(OWNER, PortalColor::Orange);
        app.update();
        assert_eq!(surfaces(&mut app), [(PortalColor::Blue, PortalLook::Color)]);
        assert_eq!(cameras(&mut app), 0);
    }

    #[test]
    fn the_portal_camera_follows_through_the_portal() {
        let mut app = app();
        insert(&mut app, blue());
        insert(&mut app, orange());
        app.update();

        let (transform, camera, projection) = camera_of(&mut app, PortalColor::Blue).unwrap();
        assert!(camera.is_active);
        assert_eq!(camera.order, -1);
        // Its image is shown as it is, among what the main camera tonemapped
        let world = app.world_mut();
        let tonemapping = world
            .query_filtered::<&Tonemapping, With<PortalCamera>>()
            .iter(world)
            .next()
            .copied();
        assert_eq!(tonemapping, Some(Tonemapping::AcesFitted));
        // Five blocks in front of the blue portal is five blocks behind the orange one, and
        // looking into the blue one is looking out of the orange one
        assert!(
            transform.translation.distance(Vec3::new(25.0, 11.5, 6.0)) < 1e-4,
            "{}",
            transform.translation
        );
        assert!(transform.forward().distance(Vec3::NEG_X) < 1e-4);
        let Projection::Perspective(projection) = projection else {
            panic!("a perspective");
        };
        assert_ne!(
            projection.near_clip_plane,
            PerspectiveProjection::default().near_clip_plane
        );
    }

    #[test]
    fn a_portal_off_screen_or_seen_from_behind_has_its_camera_inactive() {
        let mut app = app();
        insert(&mut app, blue());
        insert(&mut app, orange());
        app.update();
        // Out of the view of the player, to its side
        assert!(
            !camera_of(&mut app, PortalColor::Orange)
                .unwrap()
                .1
                .is_active
        );

        // Turned away
        place_player_camera(
            &mut app,
            Transform::from_xyz(1.0, 11.5, 6.0).looking_to(Vec3::Z, Vec3::Y),
        );
        app.update();
        assert!(!camera_of(&mut app, PortalColor::Blue).unwrap().1.is_active);

        // Behind the wall, looking at the back of the portal
        place_player_camera(
            &mut app,
            Transform::from_xyz(1.0, 11.5, -4.0).looking_to(Vec3::Z, Vec3::Y),
        );
        app.update();
        assert!(!camera_of(&mut app, PortalColor::Blue).unwrap().1.is_active);

        // Back in front of it
        place_player_camera(&mut app, Transform::from_xyz(1.0, 11.5, 6.0));
        app.update();
        assert!(camera_of(&mut app, PortalColor::Blue).unwrap().1.is_active);
    }

    #[test]
    fn the_wall_cells_are_meshed_again_when_the_portals_change() {
        let mut app = app();
        // The chunks asked for during one more frame
        let requested = |app: &mut App| {
            app.world_mut()
                .resource_mut::<Messages<WorldRenderRequestUpdateEvent>>()
                .clear();
            app.update();
            let messages = app
                .world()
                .resource::<Messages<WorldRenderRequestUpdateEvent>>();
            messages
                .get_cursor()
                .read(messages)
                .map(|WorldRenderRequestUpdateEvent::ChunkToReload(chunk)| *chunk)
                .collect::<Vec<_>>()
        };
        insert(&mut app, blue());
        assert_eq!(requested(&mut app), [IVec3::new(0, 0, 0)]);

        // Moved to the next chunk up: both walls change
        insert(
            &mut app,
            Portal {
                anchor: IVec3::new(0, 20, 0),
                ..blue()
            },
        );
        let mut chunks = requested(&mut app);
        chunks.sort_by_key(|chunk| chunk.to_array());
        assert_eq!(chunks, [IVec3::new(0, 0, 0), IVec3::new(0, 1, 0)]);

        assert!(requested(&mut app).is_empty());
    }
}
