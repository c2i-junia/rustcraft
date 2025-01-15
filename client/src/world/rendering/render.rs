use crate::world::FirstChunkReceived;
use std::collections::HashSet;
use std::sync::Arc;

use bevy::{
    asset::Assets,
    math::IVec3,
    prelude::*,
    tasks::{block_on, futures_lite::future, AsyncComputeTaskPool, Task},
};
use bevy_mod_raycast::deferred::RaycastMesh;
use shared::{
    world::{global_block_to_chunk_pos, SIX_OFFSETS},
    CHUNK_SIZE,
};

use crate::{
    camera::BlockRaycastSet,
    world::{self, MaterialResource, QueuedEvents, WorldRenderRequestUpdateEvent},
    GameState,
};

use crate::world::ClientWorldMap;

#[derive(Debug, Default, Resource)]
pub struct QueuedMeshes {
    pub meshes: Vec<Task<(IVec3, Mesh)>>,
}

fn update_chunk(
    chunk_pos: &IVec3,
    material_resource: &MaterialResource,
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    world_map: &mut ClientWorldMap,
    new_mesh: Mesh,
) {
    let chunk = world_map.map.get_mut(chunk_pos).unwrap();
    let texture = material_resource
        .global_materials
        .get(&world::GlobalMaterial::Blocks)
        .unwrap();

    if chunk.entity.is_some() {
        commands.entity(chunk.entity.unwrap()).despawn_recursive();
        chunk.entity = None;
    }

    if chunk.entity.is_none() {
        let chunk_t = Transform::from_xyz(
            (chunk_pos.x * CHUNK_SIZE) as f32,
            (chunk_pos.y * CHUNK_SIZE) as f32,
            (chunk_pos.z * CHUNK_SIZE) as f32,
        );

        let new_entity = commands
            .spawn((
                StateScoped(GameState::Game),
                Mesh3d(meshes.add(new_mesh)),
                MeshMaterial3d(texture.clone()),
                GlobalTransform::from(chunk_t),
                Transform::from(chunk_t),
                RaycastMesh::<BlockRaycastSet>::default(),
            ))
            .id();

        let ch = world_map.map.get_mut(chunk_pos).unwrap();
        ch.entity = Some(new_entity);
    }
    // debug!("ClientChunk updated : len={}", chunk.map.len());
}

pub fn world_render_system(
    mut world_map: ResMut<ClientWorldMap>,
    material_resource: Res<MaterialResource>,
    mut ev_render: EventReader<WorldRenderRequestUpdateEvent>,
    mut queued_events: Local<QueuedEvents>,
    mut queued_meshes: Local<QueuedMeshes>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut commands: Commands,
    mut first_chunk_received: ResMut<FirstChunkReceived>,
) {
    for event in ev_render.read() {
        queued_events.events.insert(*event);
    }

    if material_resource.blocks.is_none() {
        // Wait until the texture is ready
        return;
    }

    let pool = AsyncComputeTaskPool::get();

    let events = queued_events.events.clone();

    if !events.is_empty() {
        let map_ptr = Arc::new(world_map.clone());

        let uvs = Arc::new(material_resource.blocks.as_ref().unwrap().uvs.clone());

        let mut chunks_to_reload: HashSet<IVec3> = HashSet::new();

        // Using a set so same chunks are not reloaded multiple times
        // Accumulate chunks to render
        for event in &events {
            let target_chunk_pos = match event {
                WorldRenderRequestUpdateEvent::ChunkToReload(pos) => pos,
                WorldRenderRequestUpdateEvent::BlockToReload(pos) => {
                    // Temporary shortcut
                    &global_block_to_chunk_pos(pos)
                }
            };

            chunks_to_reload.insert(*target_chunk_pos);
            for offset in &SIX_OFFSETS {
                chunks_to_reload.insert(*target_chunk_pos + *offset);
            }
        }

        for pos in chunks_to_reload {
            if let Some(chunk) = world_map.map.get(&pos) {
                // If chunk is empty, ignore it
                if chunk.map.is_empty() {
                    continue;
                }

                // Define variables to move to the thread
                let map_clone = Arc::clone(&map_ptr);
                let uvs_clone = Arc::clone(&uvs);
                let ch = chunk.clone();
                let t = pool.spawn(async move {
                    (
                        pos,
                        world::meshing::generate_chunk_mesh(&map_clone, &ch, &pos, &uvs_clone),
                    )
                });
                queued_meshes.meshes.push(t);
            }
        }
        first_chunk_received.0 = true;
    }

    // Iterate through queued meshes to see if they are completed
    queued_meshes.meshes.retain_mut(|task| {
        // If completed, then use the mesh to update the chunk and delete it from the meshing queue
        if let Some((chunk_pos, new_mesh)) = block_on(future::poll_once(task)) {
            // Update the corresponding chunk
            if world_map.map.contains_key(&chunk_pos) {
                update_chunk(
                    &chunk_pos,
                    &material_resource,
                    &mut commands,
                    &mut meshes,
                    &mut world_map,
                    new_mesh,
                );
            }
            false
        } else {
            // Else, keep the task until it is done
            true
        }
    });

    queued_events.events.clear();
}
