use crate::init::{ServerTime, TICKS_PER_SECOND};
use crate::network::utils::format_bytes;
use crate::world::generation::generate_chunk;
use bevy::math::IVec3;
use bevy::prelude::*;
use bevy_ecs::system::ResMut;
use bevy_renet::renet::{ClientId, DefaultChannel, RenetServer};
use bincode::Options;
use shared::messages::{ItemStackUpdateEvent, ServerToClientMessage, WorldUpdate};
use shared::world::{chunk_in_radius, ServerChunk, ServerWorldMap};
use std::collections::HashMap;

use shared::world::data::WorldSeed;

#[derive(Event, Debug)]
pub struct WorldUpdateRequestEvent {
    pub client: ClientId,
    pub chunks: Vec<IVec3>,
    pub render_distance: u32,
    pub player_chunk_position: IVec3,
}

pub fn send_world_update(
    mut server: ResMut<RenetServer>,
    ticker: Res<ServerTime>,
    seed: Res<WorldSeed>,
    mut world_map: ResMut<ServerWorldMap>,
    mut ev_update: EventReader<WorldUpdateRequestEvent>,
) {
    let mut chunks_to_update_count = 0;
    for event in ev_update.read() {
        let payload = bincode::options()
            .serialize(&ServerToClientMessage::WorldUpdate(WorldUpdate {
                tick: ticker.0,
                player_positions: world_map.player_positions.clone(),
                new_map: {
                    let mut map: HashMap<IVec3, ServerChunk> = HashMap::new();
                    for c in event.chunks.iter() {
                        if chunk_in_radius(
                            &event.player_chunk_position,
                            c,
                            event.render_distance as i32,
                        ) {
                            let chunk = world_map.map.get(c);

                            // If chunk already exists, transmit it to client
                            if let Some(chunk) = chunk {
                                if chunk.map.is_empty() {
                                    continue;
                                }

                                chunks_to_update_count += 1;
                                map.insert(*c, chunk.clone());
                            } else {
                                // If chunk does not exists, generate it before transmitting it
                                let chunk = generate_chunk(*c, seed.0);

                                // If chunk is empty, do not create it to prevent unnecessary data transmission
                                if chunk.map.is_empty() {
                                    continue;
                                }

                                chunks_to_update_count += 1;
                                map.insert(*c, chunk.clone());
                                world_map.map.insert(*c, chunk);
                            }
                        }
                    }
                    trace!("Update event yippeee :D    len={}", map.len());
                    map
                },
                time: world_map.time,
                mobs: world_map.mobs.clone(),
                item_stacks: world_map
                    .item_stacks
                    .iter()
                    .map(|stack| ItemStackUpdateEvent {
                        id: stack.id,
                        data: if stack.despawned {
                            None
                        } else {
                            Some((stack.stack, stack.pos))
                        },
                    })
                    .collect(),
            }))
            .unwrap();

        info!(
            "Broadcasting world state, number of chunks = {}, payload size: {}",
            chunks_to_update_count,
            format_bytes(payload.len() as u64)
        );
        server.send_message(event.client, DefaultChannel::ReliableUnordered, payload);
    }
}

pub fn broadcast_world_state(
    mut server: ResMut<RenetServer>,
    ticker: Res<ServerTime>,
    mut world_map: ResMut<ServerWorldMap>,
) {
    if ticker.0 % (2 * TICKS_PER_SECOND) != 0 {
        return;
    }

    // Update time value in the "ServerWorldMap" ressource
    world_map.time = ticker.0;

    trace!("Broadcast world update");
    let payload = bincode::options()
        .serialize(&ServerToClientMessage::WorldUpdate(to_network(
            &mut world_map,
            ticker.0,
        )))
        .unwrap();
    server.broadcast_message(DefaultChannel::ReliableUnordered, payload);
}

fn to_network(world_map: &mut ServerWorldMap, tick: u64) -> WorldUpdate {
    WorldUpdate {
        tick,
        player_positions: world_map.player_positions.clone(),
        new_map: {
            let mut m: HashMap<IVec3, ServerChunk> = HashMap::new();
            // Only send chunks that must be updated
            for v in world_map.chunks_to_update.iter() {
                m.insert(*v, world_map.map.get(v).unwrap().clone());
            }
            // Chunks are up do date, clear the vector
            world_map.chunks_to_update.clear();
            m
        },
        time: world_map.time,
        mobs: world_map.mobs.clone(),
        item_stacks: world_map
            .item_stacks
            .iter()
            .map(|stack| ItemStackUpdateEvent {
                id: stack.id,
                data: if stack.despawned {
                    None
                } else {
                    Some((stack.stack, stack.pos))
                },
            })
            .collect(),
    }
}
