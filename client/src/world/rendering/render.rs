use crate::player::CurrentPlayerMarker;
use std::iter;
use std::sync::Arc;
use std::{
    collections::{HashMap, HashSet},
    time::{Duration, Instant},
};

use bevy::{
    asset::Assets,
    light::NotShadowCaster,
    math::IVec3,
    prelude::*,
    tasks::{block_on, futures_lite::future, AsyncComputeTaskPool, Task},
};
use shared::world::{
    global_block_to_chunk_pos, vertical_render_distance, within_render_distance,
    world_position_to_chunk_position, BlockData, BlockTransparency, ChunkMap, SIX_OFFSETS,
};

use crate::{
    input::{data::GameAction, keyboard::is_action_just_pressed},
    state::{GameState, KeyMap},
    ui::hud::UIMode,
    world::{self, MaterialResource, WorldRenderRequestUpdateEvent},
};

use crate::world::{
    group_of, group_origin, BlockChanged, ChunkLoadEvent, ChunkRender, ClientWorldMap,
    RenderDistance, ServerRenderDistanceLimit, GROUP_WIDTH,
};

use super::meshing::ChunkMeshResponse;

#[derive(Debug)]
pub struct MeshingTask {
    pub group: IVec2,
    pub mesh_request_ts: Instant,
    pub thread: Task<ChunkMeshResponse>,
}

/// The most group meshes being made at once: enough to keep the threads of the task pool busy,
/// few enough that looking at each of them in every frame costs nothing.
const MESHES_IN_FLIGHT: usize = 128;

/// The group meshes being made, and the groups waiting to be meshed once one is done.
#[derive(Debug, Default)]
pub struct QueuedMeshes {
    pub meshes: Vec<MeshingTask>,
    /// In no order, each once: the nearest to the player are taken first.
    pending: Vec<IVec2>,
    pending_set: HashSet<IVec2>,
}

impl QueuedMeshes {
    /// Asks for a new mesh of each of `groups`: a mesh of one of them still being made is
    /// dropped, which cancels it, as it would be outdated.
    fn request(&mut self, groups: Vec<IVec2>) {
        let remeshed: HashSet<IVec2> = groups.iter().copied().collect();
        self.meshes.retain(|task| !remeshed.contains(&task.group));
        for group in groups {
            if self.pending_set.insert(group) {
                self.pending.push(group);
            }
        }
    }

    /// Takes the waiting groups nearest to `center` that the meshes being made leave room for,
    /// nearest first.
    fn take_nearest(&mut self, center: IVec2) -> Vec<IVec2> {
        let room = MESHES_IN_FLIGHT.saturating_sub(self.meshes.len());
        let count = room.min(self.pending.len());
        if count == 0 {
            return Vec::new();
        }
        if count < self.pending.len() {
            self.pending
                .select_nth_unstable_by_key(count - 1, |group| group.distance_squared(center));
        }
        let mut taken: Vec<IVec2> = self.pending.drain(..count).collect();
        for group in &taken {
            self.pending_set.remove(group);
        }
        taken.sort_by_key(|group| group.distance_squared(center));
        taken
    }
}

/// Shows `new_meshes` for the chunk: the chunk entities are kept and only their meshes
/// replaced, so that the chunk never disappears for a frame. The solid and the liquid faces
/// have an entity each, and only while they have faces. A solid entity spawned now casts
/// shadows if `casts_shadow` (`shadow_casters_system`).
pub(super) fn update_chunk(
    chunk: &mut ChunkRender,
    origin: IVec3,
    material_resource: &MaterialResource,
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    new_meshes: ChunkMeshResponse,
    casts_shadow: bool,
) {
    #[cfg(test)]
    {
        let triangles = |mesh: &Option<Mesh>| {
            mesh.as_ref()
                .and_then(Mesh::indices)
                .map_or(0, |indices| indices.len() / 3)
        };
        chunk.triangles = [
            triangles(&new_meshes.solid_mesh),
            triangles(&new_meshes.liquid_mesh),
        ];
    }
    let mut show = |entity: &mut Option<Entity>, mesh, material, casts_shadow| {
        show_mesh(
            entity,
            mesh,
            (material, casts_shadow),
            origin,
            material_resource,
            commands,
            meshes,
        )
    };
    show(
        &mut chunk.entity,
        new_meshes.solid_mesh,
        world::GlobalMaterial::Blocks,
        casts_shadow,
    );
    // The light goes through a liquid: it does not darken the ground below
    show(
        &mut chunk.liquid_entity,
        new_meshes.liquid_mesh,
        world::GlobalMaterial::Liquids,
        false,
    );
}

/// Shows `mesh` drawn with `material` on `entity`, spawning it or despawning it as needed. An
/// entity spawned casts shadows if `casts_shadow`.
fn show_mesh(
    entity: &mut Option<Entity>,
    mesh: Option<Mesh>,
    (material, casts_shadow): (world::GlobalMaterial, bool),
    origin: IVec3,
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
                // The block textures and materials are created together: nothing is meshed
                // before
                return;
            };
            let mut new_entity = commands.spawn((
                Transform::from_translation(origin.as_vec3()),
                Visibility::Visible,
                DespawnOnExit(GameState::Game),
                Mesh3d(meshes.add(mesh)),
                MeshMaterial3d(handle.clone()),
                bevy::camera::visibility::NoCpuCulling,
            ));
            if !casts_shadow {
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

/// Whether a block that went from `before` to `after` is drawn the same: only the hits it took
/// changed, and its cracks stay at the same stage.
fn drawn_the_same(before: Option<BlockData>, after: Option<BlockData>) -> bool {
    let (Some(before), Some(after)) = (before, after) else {
        return false;
    };
    let cracks =
        |block: &BlockData| (block.breaking_progress > 0).then(|| block.get_breaking_level());
    let unhit = |block: BlockData| BlockData {
        breaking_progress: 0,
        ..block
    };
    unhit(before) == unhit(after) && cracks(&before) == cracks(&after)
}

/// Asks for a new mesh of every chunk that a block changed since the last call: by the server,
/// or by this client's own prediction. A hit that leaves the cracks of a block at the same stage
/// changes nothing on screen: mining a block would otherwise mesh its chunk again on each hit.
pub fn request_remesh_of_changed_blocks_system(
    mut world_map: ResMut<ClientWorldMap>,
    mut block_changes: MessageReader<BlockChanged>,
    mut ev_render: MessageWriter<WorldRenderRequestUpdateEvent>,
) {
    // A position is left out only when every change told of it looks the same: a change
    // nobody told of is meshed
    let mut drawn_again = HashSet::new();
    let mut unchanged_look = HashSet::new();
    for change in block_changes.read() {
        if drawn_the_same(change.before, change.after) {
            unchanged_look.insert(change.position);
        } else {
            drawn_again.insert(change.position);
        }
    }
    let chunks_to_remesh: HashSet<IVec3> = world_map
        .blocks
        .take_changed_blocks()
        .into_iter()
        .filter(|pos| !unchanged_look.contains(pos) || drawn_again.contains(pos))
        .flat_map(chunks_touched_by_block)
        .collect();
    for chunk_pos in chunks_to_remesh {
        ev_render.write(WorldRenderRequestUpdateEvent::ChunkToReload(chunk_pos));
    }
}

/// Whether the group `group` is to be meshed: some of its chunks are loaded, and have blocks
/// whose faces may show, or it shows faces it has to clear. Most chunks a player receives are
/// air, with nothing to show, or buried in the ground.
fn needs_mesh(world_map: &ClientWorldMap, group: IVec2) -> bool {
    world_map.render.get(&group).is_some_and(|render| {
        render.entity.is_some()
            || render.liquid_entity.is_some()
            || chunks_to_mesh(world_map, group).next().is_some()
    })
}

/// The loaded chunks of the group `group` whose faces may show.
fn chunks_to_mesh(world_map: &ClientWorldMap, group: IVec2) -> impl Iterator<Item = IVec3> + '_ {
    world_map
        .render
        .get(&group)
        .into_iter()
        .flat_map(|render| render.chunks.iter().copied())
        .filter(|chunk_pos| {
            world_map
                .blocks
                .chunk(*chunk_pos)
                .is_some_and(|chunk| !chunk.is_empty())
                && !is_buried(&world_map.blocks, *chunk_pos)
        })
}

/// Whether every face of the chunk at `chunk_pos` is hidden: it is filled with one opaque block,
/// and so are the six chunks around it.
fn is_buried(blocks: &ChunkMap, chunk_pos: IVec3) -> bool {
    let opaque = |pos: IVec3| {
        blocks
            .chunk(pos)
            .and_then(|chunk| chunk.uniform())
            .flatten()
            .is_some_and(|block| block.id.get_visibility() == BlockTransparency::Solid)
    };
    opaque(chunk_pos) && SIX_OFFSETS.iter().all(|offset| opaque(chunk_pos + *offset))
}

/// How long no chunk must arrive for the stream of chunks to be over: the server sends them at
/// every tick, 50 ms apart, while it has some.
const STREAM_PAUSE: Duration = Duration::from_millis(100);

/// The chunks asked for that wait for their neighbours, by group, and when the last chunk
/// arrived.
#[derive(Default)]
pub struct WaitingChunks {
    chunks: HashMap<IVec2, HashSet<IVec3>>,
    last_arrival: Option<Instant>,
}

impl WaitingChunks {
    /// Of `requested`, the chunks to mesh now: while chunks keep arriving, a chunk waits for
    /// the chunks within the render distance `distance` of the chunk `center` that its mesh
    /// depends on: its six neighbours, and those of its group and around it, as each of them
    /// would mesh the whole group again. Once no chunk arrived for `STREAM_PAUSE`, those that
    /// wait are meshed too: the player may have moved, or the server grant less.
    fn ready(
        &mut self,
        blocks: &ChunkMap,
        (center, distance): (IVec3, i32),
        requested: impl Iterator<Item = IVec3>,
        now: Instant,
    ) -> Vec<IVec3> {
        let streaming = self
            .last_arrival
            .is_some_and(|arrival| now.duration_since(arrival) < STREAM_PAUSE);
        let has_neighbours = |chunk: IVec3| {
            SIX_OFFSETS.iter().all(|offset| {
                let neighbour = chunk + *offset;
                blocks.is_chunk_loaded(neighbour)
                    || !within_render_distance(neighbour, center, distance)
            })
        };
        let mut complete: HashMap<IVec2, bool> = HashMap::new();
        let mut group_complete = |group: IVec2| {
            *complete
                .entry(group)
                .or_insert_with(|| group_is_complete(blocks, group, center, distance))
        };
        let mut ready = Vec::new();
        for chunk in requested {
            let group = group_of(chunk);
            if !streaming || (has_neighbours(chunk) && group_complete(group)) {
                // The whole group is meshed: the other chunks of it no longer wait
                self.chunks.remove(&group);
                ready.push(chunk);
            } else {
                self.chunks.entry(group).or_default().insert(chunk);
            }
        }
        // A waiting chunk is asked for again with each neighbour that arrives
        if !streaming {
            ready.extend(self.chunks.drain().flat_map(|(_, chunks)| chunks));
        }
        ready
    }
}

/// Whether every chunk within the render distance `distance` of the chunk `center` is loaded,
/// in the columns of the group `group` and in those along its sides, which its faces touch. The
/// columns at its corners touch none: a chunk arriving there does not ask for the group again.
fn group_is_complete(blocks: &ChunkMap, group: IVec2, center: IVec3, distance: i32) -> bool {
    let inside = group * GROUP_WIDTH..group * GROUP_WIDTH + IVec2::splat(GROUP_WIDTH);
    let first = inside.start - IVec2::ONE;
    let last = inside.end;
    let lowest = IVec2::new(center.x, center.z) - IVec2::splat(distance);
    let highest = IVec2::new(center.x, center.z) + IVec2::splat(distance);
    let (first, last) = (first.max(lowest), last.min(highest));
    let vertical = vertical_render_distance(distance);
    (first.x..=last.x).all(|x| {
        (first.y..=last.y).all(|z| {
            let corner = !(inside.start.x..inside.end.x).contains(&x)
                && !(inside.start.y..inside.end.y).contains(&z);
            corner
                || (center.y - vertical..=center.y + vertical)
                    .all(|y| blocks.is_chunk_loaded(IVec3::new(x, y, z)))
        })
    })
}

/// Meshes the chunks that were asked for, nearest to the player first, on the task pool, and
/// shows each mesh once its task is done. While chunks arrive, a chunk waits for its neighbours
/// (`WaitingChunks`), and at most `MESHES_IN_FLIGHT` are meshed at once (`QueuedMeshes`).
#[allow(clippy::too_many_arguments)]
pub fn world_render_system(
    mut world_map: ResMut<ClientWorldMap>,
    material_resource: Res<MaterialResource>,
    mut ev_render: MessageReader<WorldRenderRequestUpdateEvent>,
    mut chunk_loads: MessageReader<ChunkLoadEvent>,
    mut requested_chunks: Local<HashSet<IVec3>>,
    mut waiting: Local<WaitingChunks>,
    mut queued_meshes: Local<QueuedMeshes>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut commands: Commands,
    player: Query<&Transform, With<CurrentPlayerMarker>>,
    render_distance: Res<RenderDistance>,
    server_limit: Option<Res<ServerRenderDistanceLimit>>,
) {
    // A set, so that a chunk asked for several times is meshed once
    requested_chunks.extend(
        ev_render
            .read()
            .map(|WorldRenderRequestUpdateEvent::ChunkToReload(chunk_pos)| *chunk_pos),
    );
    let now = Instant::now();
    for load in chunk_loads.read() {
        match load {
            ChunkLoadEvent::Loaded(_) => waiting.last_arrival = Some(now),
            // The other chunks of its group are drawn without it
            ChunkLoadEvent::Unloaded(chunk_pos) => {
                requested_chunks.insert(*chunk_pos);
            }
        }
    }

    // The requests wait until the textures are ready
    let Some(block_textures) = material_resource.blocks.as_ref() else {
        return;
    };

    let player_chunk =
        world_position_to_chunk_position(player.single().map_or(Vec3::ZERO, |t| t.translation));
    let distance = render_distance
        .distance
        .min(server_limit.map_or(u32::MAX, |limit| limit.0)) as i32;
    // Whether a group is to be meshed is asked once it is taken: each chunk that arrives asks for
    // its group seven times, with its neighbours
    let requested = requested_chunks
        .drain()
        .filter(|chunk_pos| world_map.render.contains_key(&group_of(*chunk_pos)))
        .collect::<Vec<_>>();
    let ready: HashSet<IVec2> = waiting
        .ready(
            &world_map.blocks,
            (player_chunk, distance),
            requested.into_iter(),
            now,
        )
        .into_iter()
        .map(group_of)
        .collect();
    queued_meshes.request(ready.into_iter().collect());
    let groups: Vec<(IVec2, Vec<IVec3>)> = queued_meshes
        .take_nearest(group_of(player_chunk))
        .into_iter()
        .filter(|group| needs_mesh(&world_map, *group))
        .map(|group| (group, chunks_to_mesh(&world_map, group).collect()))
        .collect();
    if !groups.is_empty() {
        queued_meshes.meshes.extend(spawn_meshing_tasks(
            &world_map.blocks,
            &block_textures.tiles,
            groups,
        ));
    }

    // The meshes shown in a frame go to the GPU before it is drawn
    let mut vertices_left = VERTICES_SHOWN_PER_FRAME;
    queued_meshes.meshes.retain_mut(|task| {
        if vertices_left == 0 {
            return true;
        }
        let Some(chunk) = world_map.render.get_mut(&task.group) else {
            // The group is gone: nobody wants this mesh
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
        let vertices = [&new_meshes.solid_mesh, &new_meshes.liquid_mesh]
            .into_iter()
            .flatten()
            .map(Mesh::count_vertices)
            .sum::<usize>();
        vertices_left = vertices_left.saturating_sub(vertices);
        #[cfg(test)]
        MESHES_SHOWN.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        update_chunk(
            chunk,
            group_origin(task.group),
            &material_resource,
            &mut commands,
            &mut meshes,
            new_meshes,
            casts_shadow(task.group, player_chunk),
        );
        false
    });
}

/// About the most vertices of the meshes shown in a frame, give or take a mesh: a group of
/// chunks far away may have a hundred thousand, and sending many to the GPU at once would stall
/// the frame.
const VERTICES_SHOWN_PER_FRAME: usize = 200_000;

/// How far from the player, in chunks along x and z, the chunks cast shadows: the shadows are
/// drawn within 150 blocks of the camera (`CascadeShadowConfig`), and a chunk farther than 192
/// only shades them with the sun on the horizon. The others are not tested against the
/// frusta of the shadow cascades in every frame.
pub const SHADOW_CASTER_DISTANCE: i32 = 12;

/// Whether the solid faces of the group `group` cast shadows, for a player in the chunk
/// `player_chunk`: some of its columns are within `SHADOW_CASTER_DISTANCE`.
fn casts_shadow(group: IVec2, player_chunk: IVec3) -> bool {
    let first = group * GROUP_WIDTH;
    let last = first + IVec2::splat(GROUP_WIDTH - 1);
    let column = player_chunk.xz();
    // How far the column of the player is from the nearest column of the group, along each axis
    let apart = (first - column).max(column - last).max(IVec2::ZERO);
    apart.max_element() <= SHADOW_CASTER_DISTANCE
}

/// Makes the groups within `SHADOW_CASTER_DISTANCE` of the player, and only them, cast shadows,
/// once the player is in another chunk. A group shown since was spawned as it should be.
pub fn shadow_casters_system(
    world_map: Res<ClientWorldMap>,
    player: Query<&Transform, With<CurrentPlayerMarker>>,
    mut last_chunk: Local<Option<IVec3>>,
    mut commands: Commands,
) {
    let Ok(player) = player.single() else {
        return;
    };
    let now = world_position_to_chunk_position(player.translation);
    let Some(before) = last_chunk.replace(now) else {
        return;
    };
    if before == now {
        return;
    }
    for (group, render) in &world_map.render {
        let Some(entity) = render.entity else {
            continue;
        };
        match (casts_shadow(*group, before), casts_shadow(*group, now)) {
            (false, true) => {
                commands.entity(entity).try_remove::<NotShadowCaster>();
            }
            (true, false) => {
                commands.entity(entity).try_insert(NotShadowCaster);
            }
            _ => {}
        }
    }
}

/// How many group meshes were started, for the benchmarks.
#[cfg(test)]
pub(crate) static MESHES_STARTED: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);

/// How many group meshes were shown, for the benchmarks.
#[cfg(test)]
pub(crate) static MESHES_SHOWN: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);

/// Starts meshing each of `groups`, in this order: one mesh of the chunks given with each. The
/// tasks share one copy of the chunks they read: theirs and those around each that
/// `read_by_mesh` gives, as a mesh depends on the neighbours of its chunk. Copying the whole
/// world instead would stall the frame once it is large.
pub(super) fn spawn_meshing_tasks(
    blocks: &ChunkMap,
    tiles: &HashMap<String, u32>,
    groups: Vec<(IVec2, Vec<IVec3>)>,
) -> Vec<MeshingTask> {
    let pool = AsyncComputeTaskPool::get();
    let mut nearby = ChunkMap::default();
    for chunk_pos in groups
        .iter()
        .flat_map(|(_, chunks)| chunks)
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

    groups
        .into_iter()
        .map(|(group, chunks)| {
            #[cfg(test)]
            MESHES_STARTED.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let blocks = Arc::clone(&shared_blocks);
            let tiles = Arc::clone(&shared_tiles);
            MeshingTask {
                group,
                mesh_request_ts: Instant::now(),
                thread: pool.spawn(async move {
                    // The chunks of a group are meshed side by side: a block broken is shown
                    // as soon as when each chunk was drawn alone
                    let faces: Vec<_> = chunks
                        .into_iter()
                        .map(|chunk_pos| {
                            let blocks = Arc::clone(&blocks);
                            let tiles = Arc::clone(&tiles);
                            pool.spawn(async move {
                                world::meshing::chunk_faces_at(&blocks, chunk_pos, &tiles)
                                    .map(|faces| (chunk_pos, faces))
                            })
                        })
                        .collect();
                    let mut meshed = Vec::with_capacity(faces.len());
                    for task in faces {
                        meshed.extend(task.await);
                    }
                    world::meshing::group_mesh(meshed, group_origin(group))
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
        for chunk in world_map.render.values_mut() {
            // Despawn the group's entities
            for entity in [chunk.entity.take(), chunk.liquid_entity.take()]
                .into_iter()
                .flatten()
            {
                commands.entity(entity).despawn();
            }
            // Request a render for this group
            for pos in &chunk.chunks {
                ev_writer.write(WorldRenderRequestUpdateEvent::ChunkToReload(*pos));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::message::Messages;
    use bevy::ecs::system::RunSystemOnce;
    use bevy::ecs::world::CommandQueue;
    use bevy::mesh::PrimitiveTopology;
    use shared::world::{BlockData, BlockDirection, BlockId, Chunk};
    use shared::CHUNK_SIZE;

    /// The chunks a remesh is asked for after the block at (5, 5, 5) goes from `before` to
    /// `after`, as told by a `BlockChanged`.
    fn remeshed_after(before: BlockData, after: BlockData) -> usize {
        let mut world = World::new();
        let mut map = ClientWorldMap::default();
        map.blocks.insert_chunk(IVec3::ZERO, Chunk::new());
        map.blocks.set_block(IVec3::splat(5), before);
        map.blocks.take_changed_blocks();
        map.blocks.set_block(IVec3::splat(5), after);
        world.insert_resource(map);
        world.init_resource::<Messages<BlockChanged>>();
        world.init_resource::<Messages<WorldRenderRequestUpdateEvent>>();
        world.write_message(BlockChanged {
            position: IVec3::splat(5),
            before: Some(before),
            after: Some(after),
            by_local_player: true,
        });
        world
            .run_system_once(request_remesh_of_changed_blocks_system)
            .unwrap();
        world
            .resource_mut::<Messages<WorldRenderRequestUpdateEvent>>()
            .drain()
            .count()
    }

    /// How many meshes of its chunk mining each block takes, one hit at a time, before and
    /// after the hits that do not change the cracks were left out.
    #[test]
    #[ignore = "benchmark: run it with --ignored"]
    fn bench_mining_remeshes() {
        let (mut hits, mut meshes) = (0, 0);
        let mined = [
            BlockId::Dirt,
            BlockId::Grass,
            BlockId::Sand,
            BlockId::Stone,
            BlockId::Cobblestone,
            BlockId::OakLog,
            BlockId::OakPlanks,
            BlockId::OakLeaves,
        ];
        for id in mined {
            let break_time = id.get_break_time().unwrap();
            let hit = |progress| BlockData {
                breaking_progress: progress,
                ..BlockData::new(id, BlockDirection::Front)
            };
            // The last hit breaks the block, which always changes its chunk
            hits += break_time as usize;
            meshes += 1
                + (1..break_time)
                    .filter(|&progress| {
                        !drawn_the_same(Some(hit(progress - 1)), Some(hit(progress)))
                    })
                    .count();
        }
        println!(
            "mining {} kinds of block: {hits} meshes before, {meshes} now",
            mined.len()
        );
    }

    #[test]
    fn an_empty_group_is_meshed_only_to_clear_what_it_showed() {
        let mut map = ClientWorldMap::default();
        let empty = IVec3::new(0, 2, 0);
        let full = IVec3::X * GROUP_WIDTH;
        for pos in [empty, full] {
            map.blocks.insert_chunk(pos, Chunk::new());
            map.chunk_arrived(pos);
        }
        map.blocks.set_block(
            full * CHUNK_SIZE,
            BlockData::new(BlockId::Stone, BlockDirection::Front),
        );
        assert!(!needs_mesh(&map, group_of(empty)), "empty, nothing shown");
        assert_eq!(chunks_to_mesh(&map, group_of(empty)).count(), 0);
        assert!(needs_mesh(&map, group_of(full)), "a block");
        assert_eq!(
            chunks_to_mesh(&map, group_of(full)).collect::<Vec<_>>(),
            vec![full]
        );
        assert!(!needs_mesh(&map, IVec2::Y), "not loaded");

        // Its last block broken, a group clears the faces it shows
        let mut world = World::new();
        map.render.get_mut(&group_of(empty)).unwrap().entity = Some(world.spawn_empty().id());
        assert!(needs_mesh(&map, group_of(empty)));
    }

    #[test]
    fn a_hit_remeshes_only_when_the_cracks_of_the_block_change() {
        let hit = |progress| BlockData {
            breaking_progress: progress,
            ..BlockData::new(BlockId::Stone, BlockDirection::Front)
        };
        let level = |progress| hit(progress).get_breaking_level();
        // Two hits a stage, as stone takes 20
        assert_eq!((level(2), level(3), level(4)), (1, 1, 2));
        assert_eq!(remeshed_after(hit(2), hit(3)), 0, "same stage");
        assert_eq!(remeshed_after(hit(3), hit(4)), 1, "next stage");
        assert_eq!(remeshed_after(hit(0), hit(1)), 1, "the cracks appear");
        let turned = BlockData {
            direction: BlockDirection::Back,
            ..hit(3)
        };
        assert_eq!(remeshed_after(hit(2), turned), 1, "turned");
    }

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

        let tasks = spawn_meshing_tasks(&world, &tiles, vec![(IVec2::ZERO, vec![IVec3::ZERO])]);

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

    #[test]
    fn a_group_mesh_holds_the_faces_of_each_of_its_chunks_in_place() {
        use bevy::tasks::TaskPool;
        AsyncComputeTaskPool::get_or_init(TaskPool::default);
        let stone = BlockData::new(BlockId::Stone, BlockDirection::Front);
        let mut world = ChunkMap::default();
        // Two chunks of the group west of the origin: its first column, and its last one higher
        let chunks = [
            IVec3::new(-GROUP_WIDTH, 0, 0),
            IVec3::new(-1, 2, GROUP_WIDTH - 1),
        ];
        for chunk_pos in chunks {
            world.insert_chunk(chunk_pos, Default::default());
            world.set_block(chunk_pos * CHUNK_SIZE, stone);
        }
        let tiles = HashMap::from([("_Default".to_string(), 0)]);
        let group = group_of(chunks[0]);
        assert_eq!(group, group_of(chunks[1]));

        let tasks = spawn_meshing_tasks(&world, &tiles, vec![(group, chunks.to_vec())]);
        let mesh = block_on(tasks.into_iter().next().unwrap().thread)
            .solid_mesh
            .unwrap();

        // A lone block shows its six faces, four vertices each
        assert_eq!(mesh.count_vertices(), 2 * 6 * 4);
        let bounds = bevy::camera::primitives::MeshAabb::compute_aabb(&mesh).unwrap();
        let last_column = (GROUP_WIDTH - 1) * CHUNK_SIZE;
        assert_eq!(Vec3::from(bounds.min()), Vec3::ZERO);
        assert_eq!(
            Vec3::from(bounds.max()),
            Vec3::new(1., 0., 0.)
                + IVec3::new(last_column, 2 * CHUNK_SIZE + 1, last_column + 1).as_vec3()
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
            IVec3::new(1, -2, 3) * CHUNK_SIZE,
            &materials,
            &mut commands,
            meshes,
            response,
            true,
        );
        queue.apply(world);
    }

    #[test]
    fn only_the_groups_near_the_player_cast_shadows_as_it_moves() {
        let mut world = World::new();
        // Its nearest column is as far as shadows are cast
        let near = IVec2::X * SHADOW_CASTER_DISTANCE / GROUP_WIDTH;
        assert!(casts_shadow(near, IVec3::ZERO));
        let mut map = ClientWorldMap::default();
        let mut entity_at = |world: &mut World, pos: IVec2| {
            let entity = world.spawn_empty().id();
            map.render.insert(
                pos,
                ChunkRender {
                    entity: Some(entity),
                    ..ChunkRender::new()
                },
            );
            entity
        };
        let near_entity = entity_at(&mut world, near);
        let far_entity = entity_at(&mut world, near + IVec2::X);
        world.entity_mut(far_entity).insert(NotShadowCaster);
        world.insert_resource(map);
        let player = world
            .spawn((CurrentPlayerMarker {}, Transform::default()))
            .id();
        let mut system = IntoSystem::into_system(shadow_casters_system);
        system.initialize(&mut world);
        let casts = |world: &World, entity| world.get::<NotShadowCaster>(entity).is_none();

        system.run((), &mut world).unwrap();
        world.flush();
        assert!(casts(&world, near_entity) && !casts(&world, far_entity));

        // One group along x: the far group comes within reach, the near one stays
        world.get_mut::<Transform>(player).unwrap().translation.x =
            (GROUP_WIDTH * CHUNK_SIZE) as f32;
        system.run((), &mut world).unwrap();
        world.flush();
        assert!(casts(&world, near_entity) && casts(&world, far_entity));

        // Back and one more chunk the other way: both are out of reach
        world.get_mut::<Transform>(player).unwrap().translation.x = -1.;
        system.run((), &mut world).unwrap();
        world.flush();
        assert!(!casts(&world, near_entity) && !casts(&world, far_entity));
    }

    #[test]
    fn the_waiting_chunks_nearest_to_the_player_are_meshed_first_as_many_as_there_is_room() {
        let mut queued = QueuedMeshes::default();
        let far: Vec<IVec2> = (0..MESHES_IN_FLIGHT as i32 * 2)
            .map(|x| IVec2::new(x + 10, 0))
            .collect();
        queued.request(far.clone());
        // Asked for twice, waiting once
        queued.request(vec![IVec2::new(3, 0), IVec2::new(0, -2), IVec2::new(3, 0)]);

        let taken = queued.take_nearest(IVec2::ZERO);
        assert_eq!(taken.len(), MESHES_IN_FLIGHT);
        assert_eq!(&taken[..2], &[IVec2::new(0, -2), IVec2::new(3, 0)]);
        assert_eq!(&taken[2..], &far[..MESHES_IN_FLIGHT - 2]);
        // No mesh is being made in this test: there is room for as many again
        assert_eq!(queued.take_nearest(IVec2::ZERO).len(), MESHES_IN_FLIGHT);
        assert_eq!(queued.take_nearest(IVec2::ZERO).len(), 2);
        assert!(queued.take_nearest(IVec2::ZERO).is_empty());
    }

    #[test]
    fn while_chunks_arrive_a_chunk_waits_for_the_chunks_of_its_group_and_around_within_reach() {
        let mut blocks = ChunkMap::default();
        for x in -1..=1 {
            blocks.insert_chunk(IVec3::new(x, 0, 0), Chunk::new());
        }
        let start = Instant::now();
        let mut waiting = WaitingChunks {
            last_arrival: Some(start),
            ..default()
        };
        // Within reach: the cube from -1 to 1
        let reach = (IVec3::ZERO, 1);
        let requested = [IVec3::ZERO, IVec3::X];
        // The neighbours of the middle chunk above, below and on the sides are missing
        assert!(waiting
            .ready(&blocks, reach, requested.into_iter(), start)
            .is_empty());
        for offset in [IVec3::Y, IVec3::NEG_Y, IVec3::Z, IVec3::NEG_Z] {
            blocks.insert_chunk(offset, Chunk::new());
        }
        // Its neighbours are in, the rest of its group and around it is not
        assert!(waiting
            .ready(&blocks, reach, [IVec3::ZERO].into_iter(), start)
            .is_empty());
        for x in -1..=1 {
            for y in -1..=1 {
                for z in -1..=1 {
                    blocks.insert_chunk(IVec3::new(x, y, z), Chunk::new());
                }
            }
        }
        // Asked for again as the chunks arrive: its group is meshed, and the other chunk of the
        // group that waited no longer does, as it would mesh the group again
        assert_eq!(
            waiting.ready(&blocks, reach, [IVec3::ZERO].into_iter(), start),
            vec![IVec3::ZERO]
        );
        assert!(waiting.chunks.is_empty());

        // The stream pauses: a chunk that still waits is meshed as it is
        let lacking = IVec3::new(5, 0, 0);
        assert!(waiting
            .ready(&blocks, (lacking, 1), [lacking].into_iter(), start)
            .is_empty());
        let later = start + STREAM_PAUSE;
        assert_eq!(
            waiting.ready(&blocks, (lacking, 1), iter::empty(), later),
            vec![lacking]
        );
        assert!(waiting
            .ready(&blocks, reach, iter::empty(), later)
            .is_empty());
    }

    #[test]
    fn a_chunk_at_the_edge_of_the_render_distance_does_not_wait_for_what_lies_beyond() {
        let mut blocks = ChunkMap::default();
        blocks.insert_chunk(IVec3::ZERO, Chunk::new());
        let now = Instant::now();
        let mut waiting = WaitingChunks {
            last_arrival: Some(now),
            ..default()
        };
        assert_eq!(
            waiting.ready(&blocks, (IVec3::ZERO, 0), [IVec3::ZERO].into_iter(), now),
            vec![IVec3::ZERO]
        );
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
