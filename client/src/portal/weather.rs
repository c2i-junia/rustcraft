//! The rain and the snow through the portals. A portal camera draws the drops around its own eye,
//! behind the twin, and they show beyond the twin's plane (`precipitation/render.rs`). They stop
//! under the roofs around the twin, which the player's occlusion map, around the player, does not
//! hold for a twin far away: each twin seen through gets a map of its own. And they fall as the
//! twin opens: none in a view through a twin under water, rain in a view out of the water.

use std::collections::HashMap;

use bevy::prelude::*;
use shared::messages::PlayerId;
use shared::world::{Portal, PortalColor};

use super::PortalCamera;
use crate::camera::eye_under_water;
use crate::world::precipitation::occlusion::{write_heights, OcclusionMap};
use crate::world::precipitation::render::{UnderWaterView, ViewOcclusion};
use crate::world::{BlockChanged, ChunkLoadEvent, ClientWorldMap};

/// The occlusion map around the twin of each portal whose cameras there are, by owner and colour
/// of the portal.
#[derive(Resource, Default)]
pub struct TwinOcclusion(HashMap<(PlayerId, PortalColor), OcclusionMap>);

/// Where a view out of `twin` starts: the middle of its opening, in front of its wall.
fn opening(twin: &Portal) -> Vec3 {
    Vec3::from(twin.frame().translation) + twin.normal() * 0.5
}

/// Keeps a map of where the drops stop around the twin of each portal with cameras, as the
/// player's is kept (`update_occlusion_map_system`), and gives the portal cameras theirs, and
/// whether their twin opens under water.
#[allow(clippy::too_many_arguments)]
pub(super) fn portal_cameras_under_the_weather(
    world_map: Res<ClientWorldMap>,
    cameras: Query<(
        Entity,
        &PortalCamera,
        Option<&ViewOcclusion>,
        Has<UnderWaterView>,
    )>,
    mut maps: ResMut<TwinOcclusion>,
    mut chunk_loads: MessageReader<ChunkLoadEvent>,
    mut block_changes: MessageReader<BlockChanged>,
    mut images: ResMut<Assets<Image>>,
    mut writes: Local<Vec<(i32, i32, f32)>>,
    mut commands: Commands,
) {
    let loads: Vec<IVec3> = chunk_loads
        .read()
        .map(|load| match *load {
            ChunkLoadEvent::Loaded(chunk) | ChunkLoadEvent::Unloaded(chunk) => chunk,
        })
        .collect();
    let changes: Vec<IVec3> = block_changes.read().map(|change| change.position).collect();
    let portals = world_map.blocks.portals();
    let twin_of = |camera: &PortalCamera| {
        portals
            .get(camera.owner, camera.color)
            .and_then(|portal| portals.twin(portal))
    };
    let mut kept = Vec::new();
    for (entity, camera, own, marked) in &cameras {
        let key = (camera.owner, camera.color);
        let Some(twin) = twin_of(camera) else {
            commands
                .entity(entity)
                .remove::<(ViewOcclusion, UnderWaterView)>();
            continue;
        };
        if !kept.contains(&key) {
            kept.push(key);
            let map = maps
                .0
                .entry(key)
                .or_insert_with(|| OcclusionMap::new(&mut images));
            writes.clear();
            map.update(
                &world_map.blocks,
                opening(twin),
                loads.iter().copied(),
                changes.iter().copied(),
                &mut writes,
            );
            write_heights(&mut images, &map.image, &writes);
        }
        let image = &maps.0[&key].image;
        if own.is_none_or(|own| own.0 != *image) {
            commands.entity(entity).insert(ViewOcclusion(image.clone()));
        }
        let under = eye_under_water(&world_map.blocks, opening(twin));
        if under && !marked {
            commands.entity(entity).insert(UnderWaterView);
        } else if !under && marked {
            commands.entity(entity).remove::<UnderWaterView>();
        }
    }
    maps.0.retain(|key, _| kept.contains(key));
}

/// Forgets the maps of the world left.
pub(super) fn forget_twin_occlusion(mut maps: ResMut<TwinOcclusion>) {
    maps.0.clear();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::precipitation::occlusion::height_at;
    use crate::world::precipitation::render::PrecipitationView;
    use shared::world::{BlockData, BlockDirection, BlockId, Chunk, FaceDirection};

    fn portal(color: PortalColor, anchor: IVec3) -> Portal {
        Portal {
            owner: 1,
            color,
            anchor,
            face: FaceDirection::PlusX,
            up: FaceDirection::PlusY,
        }
    }

    /// The blue portal near the origin, its twin 1000 blocks east, each on ground of stone at
    /// y = 63 over chunks 0 to 5 of the columns around it; a roof of planks at y = 70 just in
    /// front of the twin; the portal cameras of the blue portal, as the game spawns them.
    fn app() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<Image>()
            .add_message::<ChunkLoadEvent>()
            .add_message::<BlockChanged>()
            .init_resource::<ClientWorldMap>()
            .init_resource::<TwinOcclusion>()
            .add_systems(Update, portal_cameras_under_the_weather);
        let mut world_map = app.world_mut().resource_mut::<ClientWorldMap>();
        let map = &mut world_map.blocks;
        for origin in [IVec2::ZERO, IVec2::new(1000, 0)] {
            let centre = origin / 16;
            for cx in -4..=4 {
                for cz in -4..=4 {
                    for cy in 0..6 {
                        map.insert_chunk(
                            IVec3::new(centre.x + cx, cy, centre.y + cz),
                            Chunk::new(),
                        );
                    }
                }
            }
            for x in -8..8 {
                for z in -8..8 {
                    map.set_block(
                        IVec3::new(origin.x + x, 63, origin.y + z),
                        BlockData::new(BlockId::Stone, BlockDirection::Front),
                    );
                }
            }
        }
        map.set_block(
            IVec3::new(1002, 70, 2),
            BlockData::new(BlockId::OakPlanks, BlockDirection::Front),
        );
        map.portals_mut()
            .insert(portal(PortalColor::Blue, IVec3::new(0, 64, 0)));
        map.portals_mut()
            .insert(portal(PortalColor::Orange, IVec3::new(1000, 64, 0)));
        for level in 1..=3 {
            app.world_mut().spawn((
                PortalCamera {
                    owner: 1,
                    color: PortalColor::Blue,
                    level,
                },
                PrecipitationView,
            ));
        }
        app
    }

    fn views(app: &mut App) -> Vec<(Option<Handle<Image>>, bool)> {
        let world = app.world_mut();
        world
            .query_filtered::<(Option<&ViewOcclusion>, Has<UnderWaterView>), With<PortalCamera>>()
            .iter(world)
            .map(|(own, under)| (own.map(|own| own.0.clone()), under))
            .collect()
    }

    /// The portal cameras' drops stop under the roof by the twin, far from the player: they read
    /// a map around the twin, all of them the same, which holds it once scanned.
    #[test]
    fn the_portal_cameras_drops_stop_under_the_roofs_by_the_twin() {
        let mut app = app();
        // The 81 chunk columns of the map, 12 a frame
        for _ in 0..8 {
            app.update();
        }
        let views = views(&mut app);
        assert_eq!(views.len(), 3);
        let image = views[0].0.clone().expect("a map of its own");
        assert!(views.iter().all(|(own, _)| own.as_ref() == Some(&image)));
        let images = app.world().resource::<Assets<Image>>();
        let map = images.get(&image).unwrap();
        assert_eq!(height_at(map, 1002, 2), 71.);
        assert_eq!(height_at(map, 1004, 4), 64.);
    }

    /// A view out of a twin under water draws no drop, one out of a twin in the air draws them:
    /// by the twin, whatever the player's eye.
    #[test]
    fn a_portal_camera_draws_no_drop_out_of_a_twin_under_water() {
        let mut app = app();
        app.update();
        assert!(views(&mut app).iter().all(|(_, under)| !under));
        // The water before the twin's opening, up to its middle and over
        let mut world_map = app.world_mut().resource_mut::<ClientWorldMap>();
        for y in 64..=65 {
            world_map.blocks.set_block(
                IVec3::new(1001, y, 0),
                BlockData::new(BlockId::Water, BlockDirection::Front),
            );
        }
        app.update();
        app.update();
        assert!(views(&mut app).iter().all(|(_, under)| *under));
        // Its twin gone, a portal camera has neither
        app.world_mut()
            .resource_mut::<ClientWorldMap>()
            .blocks
            .portals_mut()
            .remove(1, PortalColor::Orange);
        app.update();
        app.update();
        assert!(views(&mut app)
            .iter()
            .all(|(own, under)| own.is_none() && !under));
        assert!(app.world().resource::<TwinOcclusion>().0.is_empty());
    }
}
