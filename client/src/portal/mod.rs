//! The portals, drawn. An open portal shows what its camera sees from behind its twin; a lone
//! one shows its colour. Seen again in that view, it shows the view of its camera of the next
//! level, down to `PORTAL_RECURSION` levels, the last showing its colour. Each portal is a box
//! one block deep behind the face of its wall, drawn by its inner faces, so that the near plane
//! of the player's camera never cuts it while the player passes, and a glowing rim on the wall
//! around it.
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
    hide_portals_beyond_the_fog, place_portal_cameras, portal_image, resize_portal_images,
    show_chunks_to_new_views, spawn_portal_camera, PortalViewsStarted,
};
use material::{portal_material_plugin, PortalLook, PortalMaterial};

/// The layer of the surfaces that show the view through a portal to the player's camera, its
/// only. The views seen by the portal cameras are on layers of their own (`view_layer`).
pub const SURFACE_LAYER: usize = 1;
/// The layer of the local player's body in first person: the portal cameras' only, so that the
/// player sees itself through a portal but not from inside its body.
pub const BODY_LAYER: usize = 2;
/// The layer of the chunks the player has only for what a portal shows, beyond its own reach
/// (`world::rendering::portal_chunks_layers_system`): the portal cameras' only.
pub const PORTAL_CHUNKS_LAYER: usize = 3;
/// The first of the layers of each portal (`stand_in_layer`, `view_layer`).
const FIRST_PORTAL_LAYER: usize = 4;
/// How many levels of portals are seen through portals: through a portal, the portal seen
/// again shows its view, and that view the portal once more, in its colour.
pub const PORTAL_RECURSION: usize = 3;

/// How many levels of recursion are drawn, from 1 to `PORTAL_RECURSION`: fewer only to compare.
#[derive(Resource, Clone, Copy, Debug)]
pub struct PortalRecursion(pub usize);

impl Default for PortalRecursion {
    fn default() -> Self {
        Self(PORTAL_RECURSION)
    }
}

/// The layer of the stand-in of the portal drawn in `slot`, in its colour.
pub(crate) fn stand_in_layer(slot: usize) -> usize {
    FIRST_PORTAL_LAYER + PORTAL_RECURSION * slot
}

/// The layer of the surface of the portal drawn in `slot` that its own camera of `level`
/// (below `PORTAL_RECURSION`) sees: the view of the level below.
pub(crate) fn view_layer(slot: usize, level: usize) -> usize {
    stand_in_layer(slot) + level
}

/// What a portal camera sees of its own portal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum OwnView {
    /// Its view one level deeper, which the camera of `level + 1` draws: the camera of `level`
    /// sees it on `view_layer(slot, level)`.
    Deeper(usize),
    /// Its colour: at the deepest level drawn, or when the next level does not draw.
    Colour,
    /// Nothing: the portal is beyond the camera's fog, where its colour, unfogged, would show.
    Hidden,
}

/// The layers a camera of the portal drawn in `slot` sees, `slots` being those of every portal
/// drawn: the world, the body, the chunks kept for the portals, every other portal in its
/// colour, and its own portal as `own` says. None may read the image it draws, which wgpu
/// forbids: its own portal seen at `level` shows the image of `level + 1`.
pub(crate) fn camera_layers(
    slot: usize,
    own: OwnView,
    slots: impl IntoIterator<Item = usize>,
) -> RenderLayers {
    let mut layers = vec![0, BODY_LAYER, PORTAL_CHUNKS_LAYER];
    layers.extend(
        slots
            .into_iter()
            .filter(|other| *other != slot)
            .map(stand_in_layer),
    );
    match own {
        OwnView::Deeper(level) => layers.push(view_layer(slot, level)),
        OwnView::Colour => layers.push(stand_in_layer(slot)),
        OwnView::Hidden => {}
    }
    RenderLayers::from_layers(&layers)
}

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
    /// Which of the layers of the portals are its own (`stand_in_layer`, `view_layer`).
    slot: usize,
    /// The frame of the portal, parent of its surfaces, its stand-in and its rim.
    root: Entity,
    /// What an open portal shows.
    views: Option<PortalViews>,
}

/// The views of an open portal, one per level: level 1 is what the player sees through it,
/// level 2 the portal seen again in that view, and so on.
struct PortalViews {
    /// The camera of each level, from level 1.
    cameras: [Entity; PORTAL_RECURSION],
    /// Two images, which the levels draw into in turns (`image`).
    images: [Handle<Image>; 2],
    /// The material of the surface showing each level, from level 1.
    materials: [Handle<PortalMaterial>; PORTAL_RECURSION],
}

impl PortalViews {
    /// The image the camera of `level` draws into, and the surface showing it reads. Each level
    /// reads the image of the level below and draws into the other one: the deepest level draws
    /// first, and level 1 draws over the image the deepest one drew once level 2 has read it.
    fn image(&self, level: usize) -> &Handle<Image> {
        &self.images[level % 2]
    }
}

/// The portals drawn, by owner and colour.
#[derive(Resource, Default)]
pub struct DrawnPortals(HashMap<(PlayerId, PortalColor), DrawnPortal>);

/// The camera of a portal, which sees what the portal shows.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct PortalCamera {
    pub owner: PlayerId,
    pub color: PortalColor,
    /// 1 for the view through the portal, 2 for the portal seen again in that view, and so on.
    pub level: usize,
}

/// A portal was drawn open, its centre at `at`: it was placed with its twin there, its twin
/// was placed, or it moved.
#[derive(Message, Clone, Copy, Debug, PartialEq)]
pub struct PortalOpened {
    pub at: Vec3,
}

/// Whether the client map has had portals since the game started. The first portals it gets,
/// when the player joins, are not heard opening: they were open already.
#[derive(Resource, Default)]
struct PortalsSeen(bool);

/// The portals drawn in the game, after the client map's.
pub fn portal_plugin(app: &mut App) {
    app.add_plugins(portal_material_plugin);
    portal_systems(app);
}

/// The systems of `portal_plugin`, without the rendering of the material.
fn portal_systems(app: &mut App) {
    app.init_resource::<DrawnPortals>()
        .init_resource::<PortalRecursion>()
        .add_message::<PortalOpened>()
        .init_resource::<PortalsSeen>()
        .init_resource::<PortalViewsStarted>()
        .add_systems(
            PostUpdate,
            (
                sync_portals,
                hide_portals_beyond_the_fog,
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
            |mut drawn: ResMut<DrawnPortals>, mut seen: ResMut<PortalsSeen>| {
                // Their entities go with the game
                drawn.0.clear();
                seen.0 = false;
            },
        );
}

/// Draws the portals of the client map as they are: a portal that appeared, moved, or opened or
/// closed is drawn anew, one that is gone is no longer drawn, and the wall cells of both are
/// meshed again, the face a portal covers being left out. A portal drawn open is heard
/// (`PortalOpened`), but for the first portals of the game, open before the player joined.
fn sync_portals(
    world_map: Res<ClientWorldMap>,
    mut drawn: ResMut<DrawnPortals>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<PortalMaterial>>,
    mut images: ResMut<Assets<Image>>,
    mut remesh: MessageWriter<WorldRenderRequestUpdateEvent>,
    mut opened: MessageWriter<PortalOpened>,
    mut seen: ResMut<PortalsSeen>,
    mut commands: Commands,
) {
    let portals = world_map.blocks.portals();
    let heard = seen.0;
    seen.0 |= !portals.is_empty();
    let mut walls = Vec::new();
    drawn.0.retain(|(owner, color), drawn| {
        let current = portals.get(*owner, *color);
        let unchanged = current.is_some_and(|portal| {
            *portal == drawn.portal && portals.twin(portal).is_some() == drawn.open
        });
        if !unchanged {
            commands.entity(drawn.root).despawn();
            for camera in drawn.views.iter().flat_map(|views| views.cameras) {
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
        let slot = (0..)
            .find(|slot| drawn.0.values().all(|drawn| drawn.slot != *slot))
            .unwrap();
        let open = portals.twin(portal).is_some();
        let color = portal_color(portal.color);
        let box_mesh = meshes.add(portal_box());
        let surface = |material: Handle<PortalMaterial>, layer: usize| {
            (
                Mesh3d(box_mesh.clone()),
                MeshMaterial3d(material),
                BOX_TRANSFORM,
                RenderLayers::layer(layer),
            )
        };
        let stand_in = materials.add(PortalMaterial::new(PortalLook::Color, portal, color, None));
        let rim = materials.add(PortalMaterial::new(PortalLook::Rim, portal, color, None));
        let root = commands
            .spawn((
                Transform::from_matrix(Mat4::from(portal.frame())),
                Visibility::default(),
                DespawnOnExit(GameState::Game),
                children![
                    surface(stand_in.clone(), stand_in_layer(slot)),
                    (Mesh3d(meshes.add(rim_mesh())), MeshMaterial3d(rim)),
                ],
            ))
            .id();
        let views = if open {
            if heard {
                opened.write(PortalOpened {
                    at: portal.frame().translation.into(),
                });
            }
            let images = [(); 2].map(|_| images.add(portal_image(size)));
            let shown = std::array::from_fn(|index| {
                let image = images[(index + 1) % 2].clone();
                materials.add(PortalMaterial::new(
                    PortalLook::View,
                    portal,
                    color,
                    Some(image),
                ))
            });
            let cameras = std::array::from_fn(|index| {
                let level = index + 1;
                spawn_portal_camera(&mut commands, portal, level, images[level % 2].clone())
            });
            let views = PortalViews {
                cameras,
                images,
                materials: shown,
            };
            // Level 1 for the player's camera, each deeper level for the camera of its portal
            // one level up
            commands.entity(root).with_children(|children| {
                children.spawn(surface(views.materials[0].clone(), SURFACE_LAYER));
                for level in 2..=PORTAL_RECURSION {
                    children.spawn(surface(
                        views.materials[level - 1].clone(),
                        view_layer(slot, level - 1),
                    ));
                }
            });
            Some(views)
        } else {
            commands
                .entity(root)
                .with_child(surface(stand_in, SURFACE_LAYER));
            None
        };
        drawn.0.insert(
            (portal.owner, portal.color),
            DrawnPortal {
                portal: *portal,
                open,
                slot,
                root,
                views,
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
    use bevy::{
        camera::RenderTarget, core_pipeline::tonemapping::Tonemapping, state::app::StatesPlugin,
        window::PrimaryWindow,
    };
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
        let mut app = unstarted_app();
        app.update();
        app
    }

    /// `app`, before its first frame, in which it enters the game.
    fn unstarted_app() -> App {
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
            .query::<(&RenderLayers, &MeshMaterial3d<PortalMaterial>)>()
            .iter(world)
            .filter(|(layers, _)| layers.intersects(&RenderLayers::layer(SURFACE_LAYER)))
            .map(|(_, material)| material.0.clone())
            .collect();
        let materials = world.resource::<Assets<PortalMaterial>>();
        let color_of = |material: &PortalMaterial| {
            [PortalColor::Blue, PortalColor::Orange]
                .into_iter()
                .find(|color| {
                    material.params.color == LinearRgba::from(portal_color(*color)).to_vec4()
                })
                .unwrap()
        };
        let mut looks: Vec<_> = surfaces
            .iter()
            .map(|material| {
                let material = materials.get(material).unwrap();
                (color_of(material), material.look)
            })
            .collect();
        looks.sort_by_key(|(color, _)| *color as u8);
        looks
    }

    fn camera_of(app: &mut App, color: PortalColor) -> Option<(Transform, Camera, Projection)> {
        let world = app.world_mut();
        world
            .query::<(&PortalCamera, &Transform, &Camera, &Projection)>()
            .iter(world)
            .find(|(camera, ..)| camera.color == color && camera.level == 1)
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
    fn an_open_pair_gets_two_surfaces_and_cameras() {
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
        // One per level of each
        assert_eq!(cameras(&mut app), 2 * PORTAL_RECURSION);
    }

    #[test]
    fn a_lone_portal_shows_its_colour_and_has_no_camera() {
        let mut app = app();
        insert(&mut app, blue());
        app.update();
        assert_eq!(surfaces(&mut app), [(PortalColor::Blue, PortalLook::Color)]);
        assert_eq!(cameras(&mut app), 0);
    }

    /// The planes of the materials of the boxes drawn in a colour, and of the rims.
    fn colour_planes(app: &mut App) -> Vec<Vec4> {
        let world = app.world_mut();
        let handles: Vec<_> = world
            .query::<&MeshMaterial3d<PortalMaterial>>()
            .iter(world)
            .map(|material| material.0.clone())
            .collect();
        let materials = world.resource::<Assets<PortalMaterial>>();
        handles
            .iter()
            .filter_map(|handle| materials.get(handle))
            .filter(|material| material.look != PortalLook::View)
            .map(|material| material.params.plane)
            .collect()
    }

    #[test]
    fn the_colours_and_the_rims_know_the_plane_of_their_portal() {
        // The blue portal faces +Z, its plane z = 1; the orange one -X, its plane x = 20
        let blue_plane = Vec4::new(0.0, 0.0, 1.0, -1.0);
        let orange_plane = Vec4::new(-1.0, 0.0, 0.0, 20.0);
        let mut app = app();
        insert(&mut app, blue());
        app.update();
        // The surface of the lone portal, its stand-in and its rim
        assert_eq!(colour_planes(&mut app), [blue_plane; 3]);

        insert(&mut app, orange());
        app.update();
        let mut planes = colour_planes(&mut app);
        planes.sort_by(|a, b| a.x.total_cmp(&b.x));
        // The stand-ins and the rims, the surfaces showing the view
        assert_eq!(planes, [orange_plane, orange_plane, blue_plane, blue_plane]);
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

    /// The fog of the camera of the blue portal, by its colour.
    fn fog_of_blue_camera(app: &mut App) -> Option<Color> {
        let world = app.world_mut();
        world
            .query::<(&PortalCamera, Option<&DistanceFog>)>()
            .iter(world)
            .find(|(camera, _)| camera.color == PortalColor::Blue && camera.level == 1)
            .and_then(|(_, fog)| fog.map(|fog| fog.color))
    }

    fn set_player_fog(app: &mut App, color: Color) {
        let world = app.world_mut();
        let camera = world
            .query_filtered::<Entity, With<CameraController>>()
            .single(world)
            .unwrap();
        world
            .entity_mut(camera)
            .insert(DistanceFog { color, ..default() });
    }

    #[test]
    fn a_portal_camera_turned_on_has_the_fog_of_the_player_camera() {
        let mut app = app();
        set_player_fog(&mut app, Color::WHITE);
        insert(&mut app, blue());
        insert(&mut app, orange());
        app.update();
        app.update();
        assert_eq!(fog_of_blue_camera(&mut app), Some(Color::WHITE));

        // The fog changes, under water say, while the player looks away from the portal
        place_player_camera(
            &mut app,
            Transform::from_xyz(1.0, 11.5, 6.0).looking_to(Vec3::Z, Vec3::Y),
        );
        app.update();
        set_player_fog(&mut app, Color::BLACK);
        app.update();
        place_player_camera(&mut app, Transform::from_xyz(1.0, 11.5, 6.0));
        app.update();
        app.update();
        assert_eq!(fog_of_blue_camera(&mut app), Some(Color::BLACK));
    }

    #[test]
    fn a_portal_cameras_fog_follows_the_distance_to_the_portal() {
        let mut app = app();
        let world = app.world_mut();
        let camera = world
            .query_filtered::<Entity, With<CameraController>>()
            .single(world)
            .unwrap();
        world.entity_mut(camera).insert(DistanceFog {
            falloff: FogFalloff::Linear {
                start: 200.0,
                end: 384.0,
            },
            ..default()
        });
        insert(&mut app, blue());
        insert(&mut app, orange());
        let end_of_blue_fog = |app: &mut App| {
            let world = app.world_mut();
            let fog = world
                .query::<(&PortalCamera, &DistanceFog)>()
                .iter(world)
                .find(|(camera, _)| camera.color == PortalColor::Blue && camera.level == 1)
                .map(|(_, fog)| fog.falloff.clone());
            match fog {
                Some(FogFalloff::Linear { end, .. }) => end,
                other => panic!("{other:?}"),
            }
        };
        let view = (shared::world::PORTAL_VIEW_RADIUS * shared::CHUNK_SIZE) as f32;
        app.update();
        app.update();
        // Five blocks from the middle of the blue portal
        assert!((end_of_blue_fog(&mut app) - (5.0 + view)).abs() < 1e-3);

        // Three blocks farther
        place_player_camera(&mut app, Transform::from_xyz(1.0, 11.5, 9.0));
        app.update();
        assert!((end_of_blue_fog(&mut app) - (8.0 + view)).abs() < 1e-3);
    }

    #[test]
    fn a_chunk_that_changed_layer_the_frame_before_a_view_starts_reaches_that_view() {
        use bevy::camera::visibility::NoCpuCulling;
        use camera::CpuCulledForAFrame;
        let mut app = app();
        let cpu_culled = |app: &App, entity| {
            app.world().get::<NoCpuCulling>(entity).is_none()
                && app.world().get::<CpuCulledForAFrame>(entity).is_some()
        };
        // Frame N: a group changed layer, so it is culled on the CPU for a frame; another is
        // culled on the GPU only
        let changed = app.world_mut().spawn(CpuCulledForAFrame).id();
        let steady = app.world_mut().spawn(NoCpuCulling).id();

        // Frame N + 1: a portal camera starts
        insert(&mut app, blue());
        insert(&mut app, orange());
        app.update();
        assert!(camera_of(&mut app, PortalColor::Blue).unwrap().1.is_active);
        // Both are culled on the CPU through the first frame of the new view
        assert!(cpu_culled(&app, changed), "the changed group");
        assert!(cpu_culled(&app, steady), "the steady group");

        // Then both are culled on the GPU again
        app.update();
        assert!(!cpu_culled(&app, changed) && !cpu_culled(&app, steady));
    }

    #[test]
    fn a_portal_beyond_the_players_fog_is_neither_drawn_nor_seen_through() {
        let mut app = app();
        let world = app.world_mut();
        let camera = world
            .query_filtered::<Entity, With<CameraController>>()
            .single(world)
            .unwrap();
        world.entity_mut(camera).insert(DistanceFog {
            falloff: FogFalloff::Linear {
                start: 12.0,
                end: 20.0,
            },
            ..default()
        });
        insert(&mut app, blue());
        insert(&mut app, orange());
        app.update();
        app.update();
        let root_shown = |app: &App, color| {
            let drawn = &app.world().resource::<DrawnPortals>().0[&(OWNER, color)];
            app.world().get::<Visibility>(drawn.root) != Some(&Visibility::Hidden)
        };
        // Five blocks from the blue portal
        assert!(camera_of(&mut app, PortalColor::Blue).unwrap().1.is_active);
        assert!(root_shown(&app, PortalColor::Blue));

        // Thirty blocks from it, looking at it: beyond the fog, which ends at 20
        place_player_camera(&mut app, Transform::from_xyz(1.0, 11.5, 31.0));
        app.update();
        assert!(!camera_of(&mut app, PortalColor::Blue).unwrap().1.is_active);
        assert!(!root_shown(&app, PortalColor::Blue));

        // Back
        place_player_camera(&mut app, Transform::from_xyz(1.0, 11.5, 6.0));
        app.update();
        assert!(camera_of(&mut app, PortalColor::Blue).unwrap().1.is_active);
        assert!(root_shown(&app, PortalColor::Blue));
    }

    #[test]
    fn the_image_of_a_portal_camera_follows_the_size_of_the_window() {
        let mut app = app();
        let window = app
            .world_mut()
            .spawn((Window::default(), PrimaryWindow))
            .id();
        // The size of every image of the blue portal, which its views read as its cameras
        // draw them
        let size = |app: &mut App| {
            let images = targets(app, PortalColor::Blue);
            assert_eq!(images.len(), PORTAL_RECURSION);
            let slot = app.world().resource::<DrawnPortals>().0[&(OWNER, PortalColor::Blue)].slot;
            let mut read = views_on(app, SURFACE_LAYER);
            read.retain(|image| images.contains(image));
            assert_eq!(read, [images[0]]);
            assert_eq!(views_on(app, view_layer(slot, 1)), [images[1]]);
            assert_eq!(views_on(app, view_layer(slot, 2)), [images[2]]);
            let sizes: Vec<_> = images
                .iter()
                .map(|image| {
                    app.world()
                        .resource::<Assets<Image>>()
                        .get(*image)
                        .unwrap()
                        .size()
                })
                .collect();
            assert!(sizes.iter().all(|size| *size == sizes[0]), "{sizes:?}");
            sizes[0]
        };
        insert(&mut app, blue());
        insert(&mut app, orange());
        app.update();
        app.update();
        let first = app.world().get::<Window>(window).unwrap().physical_size();
        assert_eq!(size(&mut app), first);

        app.world_mut()
            .get_mut::<Window>(window)
            .unwrap()
            .resolution
            .set_physical_resolution(640, 360);
        app.update();
        assert_eq!(size(&mut app), UVec2::new(640, 360));
    }

    /// An orange portal facing the blue one, 15 blocks south of it.
    fn facing_orange() -> Portal {
        Portal {
            owner: OWNER,
            color: PortalColor::Orange,
            anchor: IVec3::new(1, 10, 16),
            face: FaceDirection::MinusZ,
        }
    }

    /// The cameras of the portal of colour `color` that draw: their level, their order and
    /// where they stand, by level.
    fn drawing(app: &mut App, color: PortalColor) -> Vec<(usize, isize, Vec3)> {
        let world = app.world_mut();
        let mut drawing: Vec<_> = world
            .query::<(&PortalCamera, &Camera, &Transform)>()
            .iter(world)
            .filter(|(camera, view, _)| camera.color == color && view.is_active)
            .map(|(camera, view, transform)| (camera.level, view.order, transform.translation))
            .collect();
        drawing.sort_by_key(|(level, ..)| *level);
        drawing
    }

    #[test]
    fn facing_portals_are_seen_through_themselves_down_to_three_levels() {
        let mut app = app();
        set_player_fog(&mut app, Color::WHITE);
        let world = app.world_mut();
        let camera = world
            .query_filtered::<Entity, With<CameraController>>()
            .single(world)
            .unwrap();
        world.entity_mut(camera).insert(DistanceFog {
            falloff: FogFalloff::Linear {
                start: 200.0,
                end: 384.0,
            },
            ..default()
        });
        insert(&mut app, blue());
        insert(&mut app, facing_orange());
        app.update();
        app.update();

        // Through the blue portal, the player's camera stands 15 blocks further south at each
        // level, and the deepest level draws first
        let blue = drawing(&mut app, PortalColor::Blue);
        let levels: Vec<_> = blue
            .iter()
            .map(|(level, order, _)| (*level, *order))
            .collect();
        assert_eq!(levels, [(1, -1), (2, -2), (3, -3)]);
        for (level, _, translation) in &blue {
            let expected = Vec3::new(1.0, 11.5, 6.0 + 15.0 * *level as f32);
            assert!(
                translation.distance(expected) < 1e-3,
                "{level}: {translation}"
            );
        }
        // The orange portal is behind the player
        assert!(drawing(&mut app, PortalColor::Orange).is_empty());

        // The fog of the second level ends the view radius behind the blue portal, as seen
        // from the first level: 20 blocks from it
        let world = app.world_mut();
        let end = world
            .query::<(&PortalCamera, &DistanceFog)>()
            .iter(world)
            .find(|(camera, _)| camera.color == PortalColor::Blue && camera.level == 2)
            .map(|(_, fog)| fog.falloff.clone());
        let view = (shared::world::PORTAL_VIEW_RADIUS * shared::CHUNK_SIZE) as f32;
        match end {
            Some(FogFalloff::Linear { end, .. }) => assert!((end - (20.0 + view)).abs() < 1e-3),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn with_a_recursion_of_one_only_the_view_through_the_portal_is_drawn() {
        let mut app = app();
        app.insert_resource(PortalRecursion(1));
        insert(&mut app, blue());
        insert(&mut app, facing_orange());
        app.update();
        let levels: Vec<_> = drawing(&mut app, PortalColor::Blue)
            .iter()
            .map(|(level, ..)| *level)
            .collect();
        assert_eq!(levels, [1]);
    }

    #[test]
    fn a_portal_behind_the_plane_of_its_twin_is_not_seen_through_it() {
        let mut app = app();
        // Fifteen blocks in front of the blue portal
        place_player_camera(&mut app, Transform::from_xyz(1.0, 11.5, 16.0));
        // The orange portal faces north, 11 blocks north of the blue one: its camera stands 15
        // blocks south of it, 4 blocks in front of the blue portal, which it would see but for
        // the orange portal's plane
        insert(&mut app, blue());
        insert(
            &mut app,
            Portal {
                owner: OWNER,
                color: PortalColor::Orange,
                anchor: IVec3::new(4, 10, -10),
                face: FaceDirection::MinusZ,
            },
        );
        app.update();
        let drawn = drawing(&mut app, PortalColor::Blue);
        assert_eq!(drawn.len(), 1, "{drawn:?}");
        assert!(drawn[0].2.distance(Vec3::new(4.0, 11.5, 5.0)) < 1e-3);
        // Its frustum alone would see it: Bevy's leaves the oblique near plane out
        let world = app.world_mut();
        let (transform, projection) = world
            .query::<(&PortalCamera, &Transform, &Projection)>()
            .iter(world)
            .find(|(camera, ..)| camera.color == PortalColor::Blue && camera.level == 1)
            .map(|(_, transform, projection)| (*transform, projection.clone()))
            .unwrap();
        assert!(camera::sees(&transform, &projection, &blue()));
    }

    #[test]
    fn a_portal_camera_sees_the_other_portals_in_their_colour_and_its_own_as_told() {
        let has = |layers: &RenderLayers, layer| layers.intersects(&RenderLayers::layer(layer));
        let slots = [0, 1, 2];
        let first = camera_layers(1, OwnView::Deeper(1), slots);
        for layer in [0, BODY_LAYER, PORTAL_CHUNKS_LAYER, view_layer(1, 1)] {
            assert!(has(&first, layer), "{layer}");
        }
        assert!(has(&first, stand_in_layer(0)) && has(&first, stand_in_layer(2)));
        // Neither the player's surfaces, nor its own portal's colour or second level
        for layer in [SURFACE_LAYER, stand_in_layer(1), view_layer(1, 2)] {
            assert!(!has(&first, layer), "{layer}");
        }
        let second = camera_layers(1, OwnView::Deeper(2), slots);
        assert!(has(&second, view_layer(1, 2)) && !has(&second, view_layer(1, 1)));
        let colour = camera_layers(1, OwnView::Colour, slots);
        assert!(has(&colour, stand_in_layer(1)));
        assert!(!has(&colour, view_layer(1, 1)) && !has(&colour, view_layer(1, 2)));
        let hidden = camera_layers(1, OwnView::Hidden, slots);
        assert!(!has(&hidden, stand_in_layer(1)) && !has(&hidden, view_layer(1, 1)));
        assert!(has(&hidden, stand_in_layer(0)));
    }

    /// The layers of the camera of `level` of the portal of colour `color`, and the slot of the
    /// portal.
    fn layers_of(app: &mut App, color: PortalColor, level: usize) -> (RenderLayers, usize) {
        let slot = app.world().resource::<DrawnPortals>().0[&(OWNER, color)].slot;
        let world = app.world_mut();
        let layers = world
            .query::<(&PortalCamera, &RenderLayers)>()
            .iter(world)
            .find(|(camera, _)| camera.color == color && camera.level == level)
            .map(|(_, layers)| layers.clone())
            .unwrap();
        (layers, slot)
    }

    fn sees_layer(layers: &RenderLayers, layer: usize) -> bool {
        layers.intersects(&RenderLayers::layer(layer))
    }

    #[test]
    fn a_portal_camera_sees_its_portal_one_level_deeper_or_in_its_colour_at_the_last_level() {
        let mut app = app();
        insert(&mut app, blue());
        insert(&mut app, facing_orange());
        app.update();
        let (first, slot) = layers_of(&mut app, PortalColor::Blue, 1);
        assert!(sees_layer(&first, view_layer(slot, 1)));
        assert!(!sees_layer(&first, stand_in_layer(slot)));
        let (last, _) = layers_of(&mut app, PortalColor::Blue, PORTAL_RECURSION);
        assert!(sees_layer(&last, stand_in_layer(slot)));

        app.insert_resource(PortalRecursion(1));
        app.update();
        let (first, _) = layers_of(&mut app, PortalColor::Blue, 1);
        assert!(sees_layer(&first, stand_in_layer(slot)));
        assert!(!sees_layer(&first, view_layer(slot, 1)));
    }

    #[test]
    fn a_portal_beyond_the_fog_of_a_portal_camera_is_not_drawn_by_it() {
        let mut app = app();
        let world = app.world_mut();
        let camera = world
            .query_filtered::<Entity, With<CameraController>>()
            .single(world)
            .unwrap();
        world.entity_mut(camera).insert(DistanceFog {
            falloff: FogFalloff::Linear {
                start: 10.0,
                end: 15.0,
            },
            ..default()
        });
        insert(&mut app, blue());
        insert(&mut app, facing_orange());
        app.update();
        app.update();
        // The blue portal is 5 blocks from the player, and 20 from the camera of the first level
        let levels: Vec<_> = drawing(&mut app, PortalColor::Blue)
            .iter()
            .map(|(level, ..)| *level)
            .collect();
        assert_eq!(levels, [1]);
        // Which then draws neither the next level, which it does not draw, nor its colour on
        // its fog
        let (first, slot) = layers_of(&mut app, PortalColor::Blue, 1);
        assert!(!sees_layer(&first, view_layer(slot, 1)));
        assert!(!sees_layer(&first, stand_in_layer(slot)));
    }

    /// The image each camera of the portal of colour `color` draws into, by level.
    fn targets(app: &mut App, color: PortalColor) -> Vec<AssetId<Image>> {
        let world = app.world_mut();
        let mut targets: Vec<_> = world
            .query::<(&PortalCamera, &RenderTarget)>()
            .iter(world)
            .filter(|(camera, _)| camera.color == color)
            .map(|(camera, target)| {
                let RenderTarget::Image(image) = target else {
                    panic!("a portal camera draws into an image");
                };
                (camera.level, image.handle.id())
            })
            .collect();
        targets.sort_by_key(|(level, _)| *level);
        targets.into_iter().map(|(_, image)| image).collect()
    }

    /// The images read by the views drawn on `layer`.
    fn views_on(app: &mut App, layer: usize) -> Vec<AssetId<Image>> {
        let world = app.world_mut();
        let handles: Vec<_> = world
            .query::<(&RenderLayers, &MeshMaterial3d<PortalMaterial>)>()
            .iter(world)
            .filter(|(layers, _)| layers.intersects(&RenderLayers::layer(layer)))
            .map(|(_, material)| material.0.clone())
            .collect();
        let materials = world.resource::<Assets<PortalMaterial>>();
        handles
            .iter()
            .filter_map(|handle| materials.get(handle))
            .filter(|material| material.look == PortalLook::View)
            .filter_map(|material| material.image.as_ref().map(Handle::id))
            .collect()
    }

    #[test]
    fn each_level_draws_into_the_image_the_level_above_reads_and_two_images_take_turns() {
        let mut app = app();
        insert(&mut app, blue());
        insert(&mut app, facing_orange());
        app.update();
        let images = targets(&mut app, PortalColor::Blue);
        assert_eq!(images.len(), PORTAL_RECURSION);
        assert_ne!(images[0], images[1]);
        assert_eq!(images[0], images[2]);
        let slot = app.world().resource::<DrawnPortals>().0[&(OWNER, PortalColor::Blue)].slot;
        // The player's camera reads the first level, the first level the second, and so on
        let mut read = views_on(&mut app, SURFACE_LAYER);
        read.retain(|image| images.contains(image));
        assert_eq!(read, [images[0]]);
        assert_eq!(views_on(&mut app, view_layer(slot, 1)), [images[1]]);
        assert_eq!(views_on(&mut app, view_layer(slot, 2)), [images[2]]);
    }

    /// Where the portals opened during one more frame.
    fn opened(app: &mut App) -> Vec<Vec3> {
        app.world_mut()
            .resource_mut::<Messages<PortalOpened>>()
            .clear();
        app.update();
        let messages = app.world().resource::<Messages<PortalOpened>>();
        messages
            .get_cursor()
            .read(messages)
            .map(|opened| opened.at)
            .collect()
    }

    #[test]
    fn a_portal_drawn_open_is_heard_once() {
        let mut app = app();
        insert(&mut app, blue());
        // Alone, it does not open
        assert!(opened(&mut app).is_empty());
        // With its twin, both open
        insert(&mut app, orange());
        let mut at = opened(&mut app);
        at.sort_by(|a, b| a.x.total_cmp(&b.x));
        assert_eq!(at, [Vec3::new(1.0, 11.5, 1.0), Vec3::new(20.0, 11.5, 6.0)]);
        assert!(opened(&mut app).is_empty());
        // Moved, the blue one opens again where it is
        let moved = Portal {
            anchor: IVec3::new(0, 20, 0),
            ..blue()
        };
        insert(&mut app, moved);
        assert_eq!(opened(&mut app), [Vec3::from(moved.frame().translation)]);
    }

    #[test]
    fn the_portals_open_when_the_player_joins_are_not_heard() {
        let mut app = unstarted_app();
        // The portals of the world, sent when the player joins
        insert(&mut app, blue());
        insert(&mut app, orange());
        assert!(opened(&mut app).is_empty());
        assert_eq!(cameras(&mut app), 2 * PORTAL_RECURSION);

        // A pair opened later is heard
        app.world_mut()
            .resource_mut::<ClientWorldMap>()
            .blocks
            .portals_mut()
            .remove(OWNER, PortalColor::Orange);
        assert!(opened(&mut app).is_empty());
        insert(&mut app, orange());
        assert_eq!(opened(&mut app).len(), 2);
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
