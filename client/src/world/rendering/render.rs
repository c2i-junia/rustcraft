use crate::player::CurrentPlayerMarker;
use std::iter;
use std::sync::Arc;
use std::{
    collections::{HashMap, HashSet},
    time::Instant,
};

use bevy::{
    asset::Assets,
    light::NotShadowCaster,
    math::IVec3,
    prelude::*,
    tasks::{block_on, futures_lite::future, AsyncComputeTaskPool, Task},
};
use shared::{
    world::{global_block_to_chunk_pos, world_position_to_chunk_position, ChunkMap},
    CHUNK_SIZE,
};

use crate::{
    input::{data::GameAction, keyboard::is_action_just_pressed},
    state::{GameState, KeyMap},
    ui::hud::UIMode,
    world::{self, MaterialResource, WorldRenderRequestUpdateEvent},
};

use crate::world::{ChunkRender, ClientWorldMap};

use super::meshing::ChunkMeshResponse;

#[derive(Debug)]
pub struct MeshingTask {
    pub chunk_pos: IVec3,
    pub mesh_request_ts: Instant,
    pub thread: Task<ChunkMeshResponse>,
}

#[derive(Debug, Default)]
pub struct QueuedMeshes {
    pub meshes: Vec<MeshingTask>,
}

/// Shows `new_meshes` for the chunk: the chunk entities are kept and only their meshes
/// replaced, so that the chunk never disappears for a frame. The solid and the liquid faces
/// have an entity each, and only while they have faces.
pub(super) fn update_chunk(
    chunk: &mut ChunkRender,
    chunk_pos: &IVec3,
    material_resource: &MaterialResource,
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    new_meshes: ChunkMeshResponse,
) {
    let mut show = |entity: &mut Option<Entity>, mesh, material| {
        show_mesh(
            entity,
            mesh,
            material,
            chunk_pos,
            material_resource,
            commands,
            meshes,
        )
    };
    show(
        &mut chunk.entity,
        new_meshes.solid_mesh,
        world::GlobalMaterial::Blocks,
    );
    show(
        &mut chunk.liquid_entity,
        new_meshes.liquid_mesh,
        world::GlobalMaterial::Liquids,
    );
}

/// Shows `mesh` drawn with `material` on `entity`, spawning it or despawning it as needed.
fn show_mesh(
    entity: &mut Option<Entity>,
    mesh: Option<Mesh>,
    material: world::GlobalMaterial,
    chunk_pos: &IVec3,
    material_resource: &MaterialResource,
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
) {
    match (mesh, *entity) {
        (Some(mesh), Some(entity)) => {
            commands.entity(entity).insert(Mesh3d(meshes.add(mesh)));
        }
        (Some(mesh), None) => {
            let Some(handle) = material_resource.chunk_materials.get(&material) else {
                // The atlas and the block materials are created together: nothing is meshed
                // before
                return;
            };
            let mut new_entity = commands.spawn((
                Transform::from_translation((*chunk_pos * CHUNK_SIZE).as_vec3()),
                Visibility::Visible,
                DespawnOnExit(GameState::Game),
                Mesh3d(meshes.add(mesh)),
                MeshMaterial3d(handle.clone()),
            ));
            // The light goes through a liquid: it does not darken the ground below
            if material == world::GlobalMaterial::Liquids {
                new_entity.insert(NotShadowCaster);
            }
            *entity = Some(new_entity.id());
        }
        (None, Some(old_entity)) => {
            commands.entity(old_entity).despawn();
            *entity = None;
        }
        (None, None) => {}
    }
}

/// Where the blocks that the mesh of a block reads lie around it: below and on its sides, for
/// the faces they hide, and the cells that share the corners of its top, with the cells above
/// them, for the surface of a liquid. The same offsets give the chunks a chunk mesh reads.
fn read_by_mesh() -> impl Iterator<Item = IVec3> {
    let around_top = (-1..=1)
        .flat_map(|x| (0..=1).flat_map(move |y| (-1..=1).map(move |z| IVec3::new(x, y, z))));
    around_top.chain(iter::once(IVec3::NEG_Y))
}

/// The chunk that holds the block at `block_pos`, then the neighbouring chunks whose meshes
/// read it: a block on a face of its chunk touches the neighbour across that face, and a block
/// on an edge or a corner the chunks around it too, as a liquid next to it slopes towards it.
fn chunks_touched_by_block(block_pos: IVec3) -> Vec<IVec3> {
    let chunk_pos = global_block_to_chunk_pos(&block_pos);
    let mut touched = vec![chunk_pos];
    for offset in read_by_mesh() {
        let reader = global_block_to_chunk_pos(&(block_pos - offset));
        if !touched.contains(&reader) {
            touched.push(reader);
        }
    }
    touched
}

/// Orders `chunks` by increasing distance to the chunk `center`, so that the chunks the player
/// sees are meshed first.
fn sort_nearest_first(chunks: &mut [IVec3], center: IVec3) {
    chunks.sort_by_key(|chunk| chunk.distance_squared(center));
}

/// Asks for a new mesh of every chunk that a block changed since the last call: by the server,
/// or by this client's own prediction.
pub fn request_remesh_of_changed_blocks_system(
    mut world_map: ResMut<ClientWorldMap>,
    mut ev_render: MessageWriter<WorldRenderRequestUpdateEvent>,
) {
    let chunks_to_remesh: HashSet<IVec3> = world_map
        .blocks
        .take_changed_blocks()
        .into_iter()
        .flat_map(chunks_touched_by_block)
        .collect();
    for chunk_pos in chunks_to_remesh {
        ev_render.write(WorldRenderRequestUpdateEvent::ChunkToReload(chunk_pos));
    }
}

/// Meshes the chunks that were asked for, nearest to the player first, on the task pool, and
/// shows each mesh once its task is done.
pub fn world_render_system(
    mut world_map: ResMut<ClientWorldMap>,
    material_resource: Res<MaterialResource>,
    mut ev_render: MessageReader<WorldRenderRequestUpdateEvent>,
    mut requested_chunks: Local<HashSet<IVec3>>,
    mut queued_meshes: Local<QueuedMeshes>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut commands: Commands,
    player: Query<&Transform, With<CurrentPlayerMarker>>,
) {
    // A set, so that a chunk asked for several times is meshed once
    requested_chunks.extend(
        ev_render
            .read()
            .map(|WorldRenderRequestUpdateEvent::ChunkToReload(chunk_pos)| *chunk_pos),
    );

    // The requests wait until the textures are ready
    let Some(block_atlas) = material_resource.blocks.as_ref() else {
        return;
    };

    if !requested_chunks.is_empty() {
        let mut chunks: Vec<IVec3> = requested_chunks.drain().collect();
        let player_position = player.single().map_or(Vec3::ZERO, |t| t.translation);
        sort_nearest_first(
            &mut chunks,
            world_position_to_chunk_position(player_position),
        );
        queued_meshes.meshes.extend(spawn_meshing_tasks(
            &world_map.blocks,
            &block_atlas.tiles,
            chunks,
        ));
    }

    queued_meshes.meshes.retain_mut(|task| {
        let Some(chunk) = world_map.render.get_mut(&task.chunk_pos) else {
            // The chunk is gone: nobody wants this mesh
            return false;
        };
        // A mesh requested before the one on display must not replace it
        if !chunk.is_newer_than_displayed(task.mesh_request_ts) {
            return false;
        }
        let Some(new_meshes) = block_on(future::poll_once(&mut task.thread)) else {
            // Keep the task until it is done
            return true;
        };
        chunk.last_mesh_ts = task.mesh_request_ts;
        update_chunk(
            chunk,
            &task.chunk_pos,
            &material_resource,
            &mut commands,
            &mut meshes,
            new_meshes,
        );
        false
    });
}

/// Starts meshing each of `chunks` that is loaded, in this order. The tasks share one copy of
/// the chunks they read: theirs and those around each that `read_by_mesh` gives, as a mesh
/// depends on the neighbours of its chunk. Copying the whole world instead would stall the
/// frame once it is large.
pub(super) fn spawn_meshing_tasks(
    blocks: &ChunkMap,
    tiles: &HashMap<String, u32>,
    chunks: Vec<IVec3>,
) -> Vec<MeshingTask> {
    let pool = AsyncComputeTaskPool::get();
    let mut nearby = ChunkMap::default();
    for chunk_pos in chunks
        .iter()
        .flat_map(|chunk_pos| read_by_mesh().map(move |offset| *chunk_pos + offset))
    {
        if let Some(chunk) = blocks.chunk(chunk_pos) {
            if !nearby.is_chunk_loaded(chunk_pos) {
                nearby.insert_chunk(chunk_pos, chunk.clone());
            }
        }
    }
    let shared_blocks = Arc::new(nearby);
    let shared_tiles = Arc::new(tiles.clone());

    chunks
        .into_iter()
        .filter(|chunk_pos| blocks.is_chunk_loaded(*chunk_pos))
        .map(|chunk_pos| {
            let blocks = Arc::clone(&shared_blocks);
            let tiles = Arc::clone(&shared_tiles);
            MeshingTask {
                chunk_pos,
                mesh_request_ts: Instant::now(),
                thread: pool.spawn(async move {
                    let chunk = blocks.chunk(chunk_pos).expect("copied with its neighbours");
                    world::meshing::generate_chunk_mesh(&blocks, chunk, &chunk_pos, &tiles)
                }),
            }
        })
        .collect()
}

/// Meshes every loaded chunk again when the reload key is pressed.
pub fn chunk_force_reload_system(
    mut world_map: ResMut<ClientWorldMap>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    key_map: Res<KeyMap>,
    mut ev_writer: MessageWriter<WorldRenderRequestUpdateEvent>,
    mut commands: Commands,
    ui_mode: Res<UIMode>,
) {
    if *ui_mode == UIMode::Closed
        && is_action_just_pressed(GameAction::ReloadChunks, &keyboard_input, &key_map)
    {
        for (pos, chunk) in world_map.render.iter_mut() {
            // Despawn the chunk's entities
            for entity in [chunk.entity.take(), chunk.liquid_entity.take()]
                .into_iter()
                .flatten()
            {
                commands.entity(entity).despawn();
            }
            // Request a render for this chunk
            ev_writer.write(WorldRenderRequestUpdateEvent::ChunkToReload(*pos));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::world::CommandQueue;
    use bevy::mesh::PrimitiveTopology;

    fn sorted(mut chunks: Vec<IVec3>) -> Vec<IVec3> {
        chunks.sort_by_key(|c| (c.x, c.y, c.z));
        chunks
    }

    #[test]
    fn an_inner_block_touches_only_its_own_chunk() {
        assert_eq!(
            chunks_touched_by_block(IVec3::new(5, 6, 7)),
            vec![IVec3::ZERO]
        );
        assert_eq!(
            chunks_touched_by_block(IVec3::new(-5, 33, 20)),
            vec![IVec3::new(-1, 2, 1)]
        );
    }

    #[test]
    fn a_block_on_a_chunk_face_touches_the_neighbour_on_that_side() {
        let last = CHUNK_SIZE - 1;
        assert_eq!(
            chunks_touched_by_block(IVec3::new(0, 5, 5)),
            vec![IVec3::ZERO, IVec3::new(-1, 0, 0)]
        );
        assert_eq!(
            chunks_touched_by_block(IVec3::new(5, last, 5)),
            vec![IVec3::ZERO, IVec3::new(0, 1, 0)]
        );
        // The water below a block at the bottom of a chunk fills its cell under it
        assert_eq!(
            chunks_touched_by_block(IVec3::new(5, 0, 5)),
            vec![IVec3::ZERO, IVec3::new(0, -1, 0)]
        );
        assert_eq!(
            sorted(chunks_touched_by_block(IVec3::new(5, 5, CHUNK_SIZE))),
            vec![IVec3::new(0, 0, 0), IVec3::new(0, 0, 1)]
        );
    }

    #[test]
    fn a_block_on_a_vertical_edge_touches_the_chunk_across_the_edge_too() {
        let last = CHUNK_SIZE - 1;
        // A liquid across the edge shares a corner with the block
        assert_eq!(
            sorted(chunks_touched_by_block(IVec3::new(0, 5, last))),
            sorted(vec![
                IVec3::ZERO,
                IVec3::new(-1, 0, 0),
                IVec3::new(0, 0, 1),
                IVec3::new(-1, 0, 1),
            ])
        );
        // The corner of the chunk (-1, -1, -1) that faces the origin: the chunks of the cells
        // beside and above it read it, the chunks below it do not
        assert_eq!(
            sorted(chunks_touched_by_block(IVec3::splat(-1))),
            sorted(vec![
                IVec3::splat(-1),
                IVec3::new(0, -1, -1),
                IVec3::new(-1, -1, 0),
                IVec3::new(0, -1, 0),
                IVec3::new(-1, 0, -1),
            ])
        );
    }

    #[test]
    fn chunks_are_sorted_nearest_first() {
        let center = IVec3::new(2, 0, 0);
        let mut chunks = vec![
            IVec3::new(5, 0, 0),
            IVec3::new(2, 0, 0),
            IVec3::new(-3, 0, 0),
            IVec3::new(3, 1, 0),
            IVec3::new(1, 0, 0),
        ];
        sort_nearest_first(&mut chunks, center);
        let distances: Vec<i32> = chunks.iter().map(|c| c.distance_squared(center)).collect();
        assert_eq!(distances, vec![0, 1, 2, 9, 25]);
        assert_eq!(chunks[0], center);
    }

    #[test]
    fn a_meshing_task_sees_the_blocks_of_the_neighbouring_chunks() {
        use bevy::tasks::TaskPool;
        use shared::world::{BlockData, BlockDirection, BlockId};
        AsyncComputeTaskPool::get_or_init(TaskPool::default);
        let stone = BlockData::new(BlockId::Stone, BlockDirection::Front);
        let mut world = ChunkMap::default();
        for chunk_pos in [IVec3::ZERO, IVec3::X, IVec3::new(5, 0, 0)] {
            world.insert_chunk(chunk_pos, Default::default());
        }
        world.set_block(IVec3::new(CHUNK_SIZE - 1, 5, 5), stone);
        // Against the block above, in the next chunk
        world.set_block(IVec3::new(CHUNK_SIZE, 5, 5), stone);
        let tiles = HashMap::from([("_Default".to_string(), 0)]);

        let tasks = spawn_meshing_tasks(&world, &tiles, vec![IVec3::ZERO, IVec3::NEG_ONE]);

        assert_eq!(tasks.len(), 1, "a chunk that is not loaded is not meshed");
        let meshes = block_on(tasks.into_iter().next().unwrap().thread);
        let in_place = world::meshing::generate_chunk_mesh(
            &world,
            world.chunk(IVec3::ZERO).unwrap(),
            &IVec3::ZERO,
            &tiles,
        );
        assert_eq!(
            meshes.solid_mesh.unwrap().count_vertices(),
            in_place.solid_mesh.unwrap().count_vertices()
        );
    }

    fn empty_mesh() -> Option<Mesh> {
        Some(Mesh::new(
            PrimitiveTopology::TriangleList,
            Default::default(),
        ))
    }

    fn some_mesh() -> ChunkMeshResponse {
        ChunkMeshResponse {
            solid_mesh: empty_mesh(),
            liquid_mesh: None,
        }
    }

    /// Runs `update_chunk` on a fresh `chunk` state and applies its commands.
    fn show(
        world: &mut World,
        meshes: &mut Assets<Mesh>,
        chunk: &mut ChunkRender,
        response: ChunkMeshResponse,
    ) {
        let mut materials = MaterialResource::default();
        materials
            .chunk_materials
            .insert(world::GlobalMaterial::Blocks, Handle::default());
        materials
            .chunk_materials
            .insert(world::GlobalMaterial::Liquids, Handle::default());
        let mut queue = CommandQueue::default();
        let mut commands = Commands::new(&mut queue, world);
        update_chunk(
            chunk,
            &IVec3::new(1, -2, 3),
            &materials,
            &mut commands,
            meshes,
            response,
        );
        queue.apply(world);
    }

    #[test]
    fn a_new_mesh_replaces_the_mesh_of_the_same_entity() {
        let mut world = World::new();
        let mut meshes = Assets::<Mesh>::default();
        let mut chunk = ChunkRender::new();

        show(&mut world, &mut meshes, &mut chunk, some_mesh());
        let entity = chunk.entity.expect("the chunk has an entity");
        assert_eq!(
            world.get::<Transform>(entity).unwrap().translation,
            Vec3::new(16.0, -32.0, 48.0)
        );
        let first_mesh = world.get::<Mesh3d>(entity).unwrap().0.clone();

        show(&mut world, &mut meshes, &mut chunk, some_mesh());

        assert_eq!(chunk.entity, Some(entity));
        assert_ne!(world.get::<Mesh3d>(entity).unwrap().0, first_mesh);
        assert!(world
            .get::<MeshMaterial3d<world::ChunkMaterial>>(entity)
            .is_some());
        assert_eq!(world.query::<&Mesh3d>().iter(&world).count(), 1);
    }

    #[test]
    fn a_chunk_without_faces_has_no_entity_and_gets_one_back_with_a_mesh() {
        let mut world = World::new();
        let mut meshes = Assets::<Mesh>::default();
        let mut chunk = ChunkRender::new();
        show(&mut world, &mut meshes, &mut chunk, some_mesh());
        let entity = chunk.entity.unwrap();

        show(
            &mut world,
            &mut meshes,
            &mut chunk,
            ChunkMeshResponse::default(),
        );

        assert_eq!(chunk.entity, None);
        assert!(world.get_entity(entity).is_err());
        show(
            &mut world,
            &mut meshes,
            &mut chunk,
            ChunkMeshResponse::default(),
        );
        assert_eq!(chunk.entity, None);

        show(&mut world, &mut meshes, &mut chunk, some_mesh());
        assert!(chunk.entity.is_some());
    }

    #[test]
    fn the_liquid_faces_get_an_entity_of_their_own_that_casts_no_shadow() {
        let mut world = World::new();
        let mut meshes = Assets::<Mesh>::default();
        let mut chunk = ChunkRender::new();
        let both = || ChunkMeshResponse {
            solid_mesh: empty_mesh(),
            liquid_mesh: empty_mesh(),
        };

        show(&mut world, &mut meshes, &mut chunk, both());
        let solid = chunk.entity.unwrap();
        let liquid = chunk
            .liquid_entity
            .expect("the liquid faces have an entity");
        assert_ne!(solid, liquid);
        assert!(world.get::<NotShadowCaster>(liquid).is_some());
        assert!(world.get::<NotShadowCaster>(solid).is_none());

        // The water is gone, the ground stays
        show(&mut world, &mut meshes, &mut chunk, some_mesh());
        assert_eq!(chunk.entity, Some(solid));
        assert_eq!(chunk.liquid_entity, None);
        assert!(world.get_entity(liquid).is_err());
    }
}
