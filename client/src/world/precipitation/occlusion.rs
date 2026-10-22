//! Where the rain and the snow stop: for each column around the camera, the height of the top
//! of its highest block that is not air nor a decoration, which the shaders of the drops read
//! to hide what falls under it. Leaves, glass and cacti stop it; water stops it at its surface;
//! grass, flowers and torches let it through to the ground.
//!
//! The heights lie in a texture of one texel per column, wrapped around like the climate map:
//! the column `(x, z)` is at `(x, z)` modulo its size. It holds the chunk columns within
//! `REACH` of the camera's. A chunk column is scanned again when one of its chunks comes or
//! goes and when it enters the map as the camera moves; a single column when one of its blocks
//! changes, in the frame it does. The image changes only then.

use bevy::asset::RenderAssetUsages;
use bevy::platform::collections::{HashMap, HashSet};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use shared::world::liquid::surface_height;
use shared::world::{BlockTransparency, Chunk, ChunkMap, MAX_VERTICAL_RENDER_DISTANCE};
use shared::CHUNK_SIZE;

use crate::world::{BlockChanged, ChunkLoadEvent, ClientWorldMap};

/// The chunk columns held around the camera's, along x and z: the drops fall at most 48 blocks
/// away, and the map reaches at least 64 blocks from the camera whatever its place in its chunk.
pub const REACH: i32 = 4;

/// Chunk columns along each side of the map.
const MAP_CHUNKS: i32 = 2 * REACH + 1;

/// Texels along each side of the occlusion map.
pub const OCCLUSION_MAP_SIZE: u32 = (MAP_CHUNKS * CHUNK_SIZE) as u32;

/// The height of a column not known yet, which stops everything: nothing falls where the world
/// is not loaded.
pub const UNKNOWN: f32 = 1e6;

/// The height of a column with no block in the chunks held: everything falls through it.
pub const OPEN: f32 = -1e6;

/// The chunk columns scanned again in a frame at most, the nearest first: entering a world
/// brings the whole map at once.
const COLUMNS_PER_FRAME: usize = 12;

/// The chunks of a column looked for above and below the camera's: those the server sends, and
/// a few more being unloaded.
const VERTICAL_REACH: i32 = MAX_VERTICAL_RENDER_DISTANCE + 2;

/// Whether `block` stops the rain: any block but the decorations, which are no shelter.
fn stops_rain(block: shared::world::BlockId) -> bool {
    block.get_visibility() != BlockTransparency::Decoration
}

/// The chunks of the chunk column `column` held in `map` from `top` down to `bottom`, with their
/// heights, but those of air: they are found once for the 256 columns.
fn column_chunks(map: &ChunkMap, column: IVec2, top: i32, bottom: i32) -> Vec<(i32, &Chunk)> {
    (bottom..=top)
        .rev()
        .filter_map(|y| Some((y, map.chunk(IVec3::new(column.x, y, column.y))?)))
        .filter(|(_, chunk)| chunk.uniform() != Some(None))
        .collect()
}

/// The height at which the precipitation stops at the column `local` (x, z in its chunk),
/// looking down through `chunks` (`column_chunks`): the top of its highest block that stops it,
/// or the surface of its water; `OPEN` for none.
fn stop_height(chunks: &[(i32, &Chunk)], local: IVec2) -> f32 {
    for &(chunk_y, chunk) in chunks {
        for local_y in (0..CHUNK_SIZE).rev() {
            let Some(block) = chunk.get(IVec3::new(local.x, local_y, local.y)) else {
                continue;
            };
            if !stops_rain(block.id) {
                continue;
            }
            let y = (chunk_y * CHUNK_SIZE + local_y) as f32;
            if block.id.get_visibility() == BlockTransparency::Liquid {
                return y + surface_height(block.level);
            }
            return y + 1.;
        }
    }
    OPEN
}

/// The index in the data of the occlusion map of the column `(x, z)`, wrapped around.
fn texel_index(x: i32, z: i32) -> usize {
    let size = OCCLUSION_MAP_SIZE as i32;
    (z.rem_euclid(size) * size + x.rem_euclid(size)) as usize
}

/// The height at which the precipitation stops at the column `(x, z)` on `image`, the
/// occlusion map: `UNKNOWN` for a column not known yet, `OPEN` for one with no block.
pub fn height_at(image: &Image, x: i32, z: i32) -> f32 {
    let start = texel_index(x, z) * 4;
    image.data.as_ref().map_or(UNKNOWN, |data| {
        f32::from_le_bytes(data[start..start + 4].try_into().unwrap())
    })
}

/// The chunk column of a column.
fn chunk_column(x: i32, z: i32) -> IVec2 {
    IVec2::new(x.div_euclid(CHUNK_SIZE), z.div_euclid(CHUNK_SIZE))
}

/// A new occlusion map, where no column is known yet.
pub fn new_occlusion_map() -> Image {
    Image::new_fill(
        Extent3d {
            width: OCCLUSION_MAP_SIZE,
            height: OCCLUSION_MAP_SIZE,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &UNKNOWN.to_le_bytes(),
        TextureFormat::R32Float,
        RenderAssetUsages::default(),
    )
}

/// Writes `height` into `data`, the bytes of the occlusion map, at the column `(x, z)`.
fn write_height(data: &mut [u8], x: i32, z: i32, height: f32) {
    let start = texel_index(x, z) * 4;
    data[start..start + 4].copy_from_slice(&height.to_le_bytes());
}

/// The occlusion map, and what it holds.
#[derive(Resource)]
pub struct OcclusionMap {
    pub image: Handle<Image>,
    /// The chunk column whose columns each slot of the map holds, by slot, once scanned.
    slots: HashMap<IVec2, IVec2>,
    /// The chunk columns of the map to scan again.
    dirty: HashSet<IVec2>,
    /// The chunk column of the camera the map is around.
    centre: Option<IVec2>,
}

impl FromWorld for OcclusionMap {
    fn from_world(world: &mut World) -> Self {
        let image = world
            .resource_mut::<Assets<Image>>()
            .add(new_occlusion_map());
        Self {
            image,
            slots: HashMap::new(),
            dirty: HashSet::new(),
            centre: None,
        }
    }
}

impl OcclusionMap {
    fn slot(column: IVec2) -> IVec2 {
        column.rem_euclid(IVec2::splat(MAP_CHUNKS))
    }

    /// Whether the chunk column `column` is on the map around `centre`.
    fn holds(centre: IVec2, column: IVec2) -> bool {
        let gap = (column - centre).abs();
        gap.x <= REACH && gap.y <= REACH
    }

    /// Forgets every column: they are scanned again, as for a new world.
    pub fn forget(&mut self) {
        self.slots.clear();
        self.dirty.clear();
        self.centre = None;
    }

    /// Brings the map up to date with the camera at `camera`, after the chunks `loads` came or
    /// went and the blocks at `changes` changed in `map`: pushes the heights to write to
    /// `writes`, as `(x, z, height)`.
    fn update(
        &mut self,
        map: &ChunkMap,
        camera: Vec3,
        loads: impl IntoIterator<Item = IVec3>,
        changes: impl IntoIterator<Item = IVec3>,
        writes: &mut Vec<(i32, i32, f32)>,
    ) {
        let eye = camera.floor().as_ivec3();
        let centre = chunk_column(eye.x, eye.z);
        let (top, bottom) = (
            eye.y.div_euclid(CHUNK_SIZE) + VERTICAL_REACH,
            eye.y.div_euclid(CHUNK_SIZE) - VERTICAL_REACH,
        );
        if self.centre != Some(centre) {
            self.centre = Some(centre);
            self.dirty.retain(|column| Self::holds(centre, *column));
            for dx in -REACH..=REACH {
                for dz in -REACH..=REACH {
                    let column = centre + IVec2::new(dx, dz);
                    if self.slots.get(&Self::slot(column)) != Some(&column) {
                        self.dirty.insert(column);
                    }
                }
            }
        }
        for chunk in loads {
            if Self::holds(centre, chunk.xz()) {
                self.dirty.insert(chunk.xz());
            }
        }
        for position in changes {
            let column = chunk_column(position.x, position.z);
            if !Self::holds(centre, column)
                || self.dirty.contains(&column)
                || self.slots.get(&Self::slot(column)) != Some(&column)
            {
                continue;
            }
            let chunks = column_chunks(map, column, top, bottom);
            let local = IVec2::new(
                position.x.rem_euclid(CHUNK_SIZE),
                position.z.rem_euclid(CHUNK_SIZE),
            );
            writes.push((position.x, position.z, stop_height(&chunks, local)));
        }

        let mut nearest: Vec<IVec2> = self.dirty.iter().copied().collect();
        nearest.sort_by_key(|column| (*column - centre).length_squared());
        for column in nearest.into_iter().take(COLUMNS_PER_FRAME) {
            self.dirty.remove(&column);
            self.slots.insert(Self::slot(column), column);
            let chunks = column_chunks(map, column, top, bottom);
            let known = !chunks.is_empty() || any_chunk(map, column, top, bottom);
            let origin = column * CHUNK_SIZE;
            for x in 0..CHUNK_SIZE {
                for z in 0..CHUNK_SIZE {
                    let height = if known {
                        stop_height(&chunks, IVec2::new(x, z))
                    } else {
                        UNKNOWN
                    };
                    writes.push((origin.x + x, origin.y + z, height));
                }
            }
        }
    }
}

/// Whether the chunk column `column` has any chunk held from `top` down to `bottom`.
fn any_chunk(map: &ChunkMap, column: IVec2, top: i32, bottom: i32) -> bool {
    (bottom..=top).any(|y| map.is_chunk_loaded(IVec3::new(column.x, y, column.y)))
}

/// Keeps the occlusion map up to date around the camera.
pub fn update_occlusion_map_system(
    mut occlusion: ResMut<OcclusionMap>,
    world_map: Res<ClientWorldMap>,
    cameras: Query<&GlobalTransform, With<Camera3d>>,
    mut chunk_loads: MessageReader<ChunkLoadEvent>,
    mut block_changes: MessageReader<BlockChanged>,
    mut images: ResMut<Assets<Image>>,
    mut writes: Local<Vec<(i32, i32, f32)>>,
) {
    let loads = chunk_loads.read().map(|load| match *load {
        ChunkLoadEvent::Loaded(chunk) | ChunkLoadEvent::Unloaded(chunk) => chunk,
    });
    let changes = block_changes.read().map(|change| change.position);
    let Some(camera) = cameras.iter().next() else {
        return;
    };
    writes.clear();
    occlusion.update(
        &world_map.blocks,
        camera.translation(),
        loads,
        changes,
        &mut writes,
    );
    if writes.is_empty() {
        return;
    }
    let Some(mut image) = images.get_mut(&occlusion.image) else {
        return;
    };
    let Some(data) = image.data.as_mut() else {
        return;
    };
    for &(x, z, height) in writes.iter() {
        write_height(data, x, z, height);
    }
}

/// Forgets the columns of the world left: the next one is scanned anew.
pub fn forget_occlusion_map(mut occlusion: ResMut<OcclusionMap>) {
    occlusion.forget();
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::world::{BlockData, BlockDirection, BlockId};

    fn block(id: BlockId) -> BlockData {
        BlockData::new(id, BlockDirection::Front)
    }

    /// A world of two chunk columns side by side at x = 0 and 1, z = 0: ground of stone up to
    /// 63 (chunks 0 to 3) under air (chunks 4 and 5).
    fn ground() -> ChunkMap {
        let mut map = ChunkMap::default();
        for cx in 0..2 {
            for cy in 0..6 {
                map.insert_chunk(IVec3::new(cx, cy, 0), Chunk::new());
            }
        }
        for x in 0..32 {
            for z in 0..16 {
                map.set_block(IVec3::new(x, 63, z), block(BlockId::Stone));
            }
        }
        map
    }

    fn stop_at(map: &ChunkMap, x: i32, z: i32) -> f32 {
        let column = chunk_column(x, z);
        let chunks = column_chunks(map, column, 10, -10);
        stop_height(
            &chunks,
            IVec2::new(x.rem_euclid(CHUNK_SIZE), z.rem_euclid(CHUNK_SIZE)),
        )
    }

    #[test]
    fn the_rain_stops_on_the_top_of_the_highest_block() {
        let mut map = ground();
        assert_eq!(stop_at(&map, 3, 4), 64.);
        // a roof well above the ground, in a chunk above
        map.set_block(IVec3::new(3, 70, 4), block(BlockId::OakPlanks));
        assert_eq!(stop_at(&map, 3, 4), 71.);
        // leaves and glass stop it as well
        for (x, id) in [(5, BlockId::OakLeaves), (6, BlockId::Glass)] {
            map.set_block(IVec3::new(x, 66, 4), block(id));
            assert_eq!(stop_at(&map, x, 4), 67., "{id:?}");
        }
    }

    #[test]
    fn grass_flowers_and_torches_let_the_rain_through_to_the_ground() {
        let mut map = ground();
        for (x, id) in [
            (1, BlockId::TallGrass),
            (2, BlockId::Poppy),
            (3, BlockId::Torch),
        ] {
            map.set_block(IVec3::new(x, 64, 0), block(id));
            assert_eq!(stop_at(&map, x, 0), 64., "{id:?}");
        }
    }

    #[test]
    fn water_stops_the_rain_at_its_surface() {
        let mut map = ground();
        map.set_block(IVec3::new(8, 64, 8), block(BlockId::Water));
        map.set_block(IVec3::new(8, 65, 8), block(BlockId::Water));
        assert_eq!(stop_at(&map, 8, 8), 65. + surface_height(0));
        assert!(stop_at(&map, 8, 8).fract() > 0.5);
    }

    #[test]
    fn a_column_of_air_lets_everything_through() {
        let mut map = ground();
        map.remove_block(IVec3::new(9, 63, 9));
        assert_eq!(stop_at(&map, 9, 9), OPEN);
    }

    #[test]
    fn each_column_has_its_texel_wrapped_around_also_west_and_north_of_the_origin() {
        let size = OCCLUSION_MAP_SIZE as i32;
        assert_eq!(size, 144);
        assert_eq!(texel_index(0, 0), 0);
        assert_eq!(texel_index(3, 1), (size + 3) as usize);
        assert_eq!(texel_index(-1, -1), texel_index(size - 1, size - 1));
        assert_eq!(
            texel_index(-878, 454),
            texel_index(-878 + size, 454 - 3 * size)
        );
        // the columns within the reach of the drops never share a texel
        let texels: HashSet<usize> = (-72..72)
            .flat_map(|x| (-72..72).map(move |z| texel_index(x - 500, z + 300)))
            .collect();
        assert_eq!(texels.len(), (size * size) as usize);
    }

    /// An app with `map` and a camera at `camera`, its occlusion map filled.
    fn app(map: ChunkMap, camera: Vec3) -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<Image>()
            .add_message::<ChunkLoadEvent>()
            .add_message::<BlockChanged>()
            .insert_resource(ClientWorldMap {
                blocks: map,
                ..default()
            })
            .init_resource::<OcclusionMap>()
            .add_systems(Update, update_occlusion_map_system);
        app.world_mut().spawn((
            Camera3d::default(),
            GlobalTransform::from_translation(camera),
        ));
        // the whole map, a few chunk columns a frame
        for _ in 0..8 {
            app.update();
        }
        app
    }

    fn image(app: &App) -> &Image {
        let handle = &app.world().resource::<OcclusionMap>().image;
        app.world().resource::<Assets<Image>>().get(handle).unwrap()
    }

    fn change(app: &mut App, position: IVec3, after: Option<BlockId>) {
        let world = app.world_mut();
        let mut map = world.resource_mut::<ClientWorldMap>();
        match after {
            Some(id) => map.blocks.set_block(position, block(id)),
            None => {
                map.blocks.remove_block(position);
            }
        }
        world.write_message(BlockChanged {
            position,
            before: None,
            after: after.map(block),
            by_local_player: true,
        });
    }

    #[test]
    fn the_map_holds_the_columns_around_the_camera_and_not_known_ones_stop_everything() {
        let app = app(ground(), Vec3::new(10., 70., 5.));
        let image = image(&app);
        assert_eq!(height_at(image, 3, 4), 64.);
        assert_eq!(height_at(image, 31, 15), 64.);
        // a chunk column not held
        assert_eq!(height_at(image, 40, 4), UNKNOWN);
        assert_eq!(height_at(image, -5, -5), UNKNOWN);
    }

    #[test]
    fn a_roof_placed_stops_the_rain_in_the_same_frame_and_a_block_broken_lets_it_through() {
        let mut app = app(ground(), Vec3::new(10., 70., 5.));
        change(&mut app, IVec3::new(12, 68, 6), Some(BlockId::OakPlanks));
        app.update();
        assert_eq!(height_at(image(&app), 12, 6), 69.);
        assert_eq!(height_at(image(&app), 13, 6), 64.);
        change(&mut app, IVec3::new(12, 68, 6), None);
        app.update();
        assert_eq!(height_at(image(&app), 12, 6), 64.);
    }

    #[test]
    fn a_chunk_that_arrives_is_scanned() {
        let mut app = app(ground(), Vec3::new(10., 70., 5.));
        assert_eq!(height_at(image(&app), 40, 4), UNKNOWN);
        let world = app.world_mut();
        let mut map = world.resource_mut::<ClientWorldMap>();
        let mut chunk = Chunk::new();
        chunk.set(IVec3::new(8, 2, 4), block(BlockId::Sand));
        map.blocks.insert_chunk(IVec3::new(2, 3, 0), chunk);
        world.write_message(ChunkLoadEvent::Loaded(IVec3::new(2, 3, 0)));
        app.update();
        assert_eq!(height_at(image(&app), 40, 4), 51.);
        assert_eq!(height_at(image(&app), 41, 4), OPEN);
    }

    #[test]
    fn the_map_follows_the_camera_and_takes_in_the_columns_it_comes_near() {
        let mut map = ground();
        // a chunk column far to the east, out of the map at first
        let far = 7 * CHUNK_SIZE;
        for cy in 0..6 {
            map.insert_chunk(IVec3::new(7, cy, 0), Chunk::new());
        }
        map.set_block(IVec3::new(far + 2, 80, 3), block(BlockId::Stone));
        let mut app = app(map, Vec3::new(10., 70., 5.));
        assert_eq!(height_at(image(&app), far + 2, 3), UNKNOWN);
        let world = app.world_mut();
        let mut cameras = world.query::<&mut GlobalTransform>();
        *cameras.single_mut(world).unwrap() =
            GlobalTransform::from_translation(Vec3::new(far as f32 - 20., 70., 5.));
        for _ in 0..5 {
            app.update();
        }
        assert_eq!(height_at(image(&app), far + 2, 3), 81.);
        // the columns still on the map are not scanned again
        assert_eq!(height_at(image(&app), 20, 4), 64.);
        // those of chunk column 0 gave their texels to chunk column 9, not held
        assert_eq!(height_at(image(&app), 3, 4), UNKNOWN);
    }

    #[test]
    fn the_image_changes_only_when_something_does() {
        let mut app = app(ground(), Vec3::new(10., 70., 5.));
        app.world_mut()
            .resource_mut::<Messages<AssetEvent<Image>>>()
            .clear();
        app.update();
        app.update();
        let events = app.world().resource::<Messages<AssetEvent<Image>>>();
        let modified = events
            .iter_current_update_messages()
            .filter(|event| matches!(event, AssetEvent::Modified { .. }))
            .count();
        assert_eq!(modified, 0);
    }
}
