use crate::{player::CurrentPlayerMarker, world::FirstChunkReceived};
use std::sync::Arc;
use std::{collections::HashSet, time::Instant};

use bevy::{
    asset::Assets,
    math::IVec3,
    prelude::*,
    tasks::{block_on, futures_lite::future, AsyncComputeTaskPool, Task},
};
use shared::{
    world::{global_block_to_chunk_pos, SIX_OFFSETS},
    CHUNK_SIZE,
};

use crate::{
    world::{self, MaterialResource, QueuedEvents, WorldRenderRequestUpdateEvent},
    GameState,
};

use crate::world::{ClientChunk, ClientWorldMap};

use super::meshing::ChunkMeshResponse;

#[derive(Debug)]
pub struct MeshingTask {
    pub chunk_pos: IVec3,
    pub mesh_request_ts: Instant,
    pub thread: Task<ChunkMeshResponse>,
}

#[derive(Debug, Default, Resource)]
pub struct QueuedMeshes {
    pub meshes: Vec<MeshingTask>,
}

fn update_chunk(
    chunk: &mut ClientChunk,
    chunk_pos: &IVec3,
    material_resource: &MaterialResource,
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    new_meshes: ChunkMeshResponse,
) {
    // If the

    let solid_texture = material_resource
        .global_materials
        .get(&world::GlobalMaterial::Blocks)
        .unwrap();

    let liquid_texture = material_resource
        .global_materials
        .get(&world::GlobalMaterial::Liquids)
        .unwrap();

    if chunk.entity.is_some() {
        commands.entity(chunk.entity.unwrap()).despawn();
        chunk.entity = None;
    }

    if chunk.entity.is_none() {
        // Offset the chunk's position by half a block so that blocks are centered
        let chunk_t = Transform::from_xyz(
            (chunk_pos.x * CHUNK_SIZE) as f32,
            (chunk_pos.y * CHUNK_SIZE) as f32,
            (chunk_pos.z * CHUNK_SIZE) as f32,
        );

        let new_entity = commands
            .spawn((chunk_t, Visibility::Visible))
            .with_children(|root| {
                if let Some(new_solid_mesh) = new_meshes.solid_mesh {
                    root.spawn((
                        StateScoped(GameState::Game),
                        Mesh3d(meshes.add(new_solid_mesh)),
                        MeshMaterial3d(solid_texture.clone()),
                    ));
                }

                if let Some(new_liquid_mesh) = new_meshes.liquid_mesh {
                    root.spawn((
                        StateScoped(GameState::Game),
                        Mesh3d(meshes.add(new_liquid_mesh)),
                        MeshMaterial3d(liquid_texture.clone()),
                    ));
                }
            })
            .id();

        chunk.entity = Some(new_entity);
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
    player_pos: Query<&Transform, With<CurrentPlayerMarker>>,
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
        let start = std::time::Instant::now();
        let map_ptr = Arc::new(world_map.clone());
        let delta = start.elapsed();
        info!("cloning map for render, took {:?}", delta);

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

        let player_pos = player_pos
            .single()
            .expect("Player should exist")
            .translation;
        let player_pos = global_block_to_chunk_pos(&IVec3::new(
            player_pos.x as i32,
            player_pos.y as i32,
            player_pos.z as i32,
        ));

        let mut chunks_to_reload = Vec::from_iter(chunks_to_reload);

        chunks_to_reload.sort_by(|a, b| {
            (a.distance_squared(player_pos) - b.distance_squared(player_pos))
                .cmp(&a.distance_squared(player_pos))
        });

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
                    world::meshing::generate_chunk_mesh(&map_clone, &ch, &pos, &uvs_clone)
                });

                queued_meshes.meshes.push(MeshingTask {
                    chunk_pos: pos,
                    mesh_request_ts: Instant::now(),
                    thread: t,
                });
            }
        }
        first_chunk_received.0 = true;
    }

    // Iterate through queued meshes to see if they are completed
    queued_meshes.meshes.retain_mut(|task| {
        let MeshingTask {
            chunk_pos,
            mesh_request_ts,
            thread,
        } = task;

        if let Some(chunk) = world_map.map.get_mut(chunk_pos) {
            // If a later mesh has been completed before, we can drop this task
            if *mesh_request_ts < chunk.last_mesh_ts {
                false
            }
            // If completed, use the mesh to update the chunk and delete it from the meshing queue
            else if let Some(new_meshes) = block_on(future::poll_once(thread)) {
                // Update the corresponding chunk
                update_chunk(
                    chunk,
                    chunk_pos,
                    &material_resource,
                    &mut commands,
                    &mut meshes,
                    new_meshes,
                );
                false
            } else {
                // Else, keep the task until it is done
                true
            }
        } else {
            true
        }
    });

    queued_events.events.clear();
}
