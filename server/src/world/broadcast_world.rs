use crate::init::ServerTime;
use crate::network::extensions::SendGameMessageExtension;
use bevy::math::IVec3;
use bevy::prelude::*;
use bevy_ecs::system::ResMut;
use bevy_renet::renet::RenetServer;
use shared::messages::mob::MobUpdateEvent;
use shared::messages::{ItemStackUpdateEvent, PlayerId, ServerToClientMessage, WorldUpdate};
use shared::players::Player;
use shared::world::{
    world_position_to_chunk_position, ServerChunk, ServerChunkWorldMap, ServerWorldMap,
};
use shared::CHUNK_SIZE;
use std::collections::HashMap;

pub const BROADCAST_RENDER_DISTANCE: i32 = 1;

pub fn broadcast_world_state(
    mut server: ResMut<RenetServer>,
    time: Res<ServerTime>,
    mut world_map: ResMut<ServerWorldMap>,
) {
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;

    let world_map = world_map.as_mut();

    let mobs = world_map.mobs.clone();
    let players = &mut world_map.players;
    let chunks = &mut world_map.chunks;

    for client in server.clients_id().iter_mut() {
        let player = players.get_mut(client);
        let player = match player {
            Some(p) => p.clone(),
            None => continue,
        };

        for (id, mob) in mobs.iter() {
            if mob.position.distance(player.position)
                < (BROADCAST_RENDER_DISTANCE * CHUNK_SIZE) as f32
            {
                server.send_game_message(
                    *client,
                    ServerToClientMessage::MobUpdate(MobUpdateEvent {
                        id: *id,
                        mob: mob.clone(),
                    }),
                );
            }
        }

        let msg = WorldUpdate {
            tick: time.0,
            time: ts,
            new_map: get_world_map_chunks_to_send(chunks, players, &player),
            mobs: mobs.clone(),
            item_stacks: get_items_stacks(),
            player_events: vec![],
        };

        if msg.new_map.is_empty() {
            continue;
        }

        let message = ServerToClientMessage::WorldUpdate(msg);

        server.send_game_message(*client, message);
    }
}

fn get_world_map_chunks_to_send(
    chunks: &mut ServerChunkWorldMap,
    players: &HashMap<PlayerId, Player>,
    player: &Player,
) -> HashMap<IVec3, ServerChunk> {
    // Send only chunks in render distance
    let mut map: HashMap<IVec3, ServerChunk> = HashMap::new();

    let active_chunks = get_all_active_chunks(players, BROADCAST_RENDER_DISTANCE);
    for c in active_chunks {
        if map.len() >= 10 {
            break;
        }

        let chunk = chunks.map.get_mut(&c);

        // If chunk already exists, transmit it to client
        if let Some(chunk) = chunk {
            if chunk.sent_to_clients.contains(&player.id) {
                continue;
            }

            map.insert(c, chunk.clone());
            chunk.sent_to_clients.push(player.id);
        }
    }

    map
}

fn get_items_stacks() -> Vec<ItemStackUpdateEvent> {
    // TODO: Update later by requiring less data (does not need to borrow a full ServerWorldMap)
    vec![]
    // world_map
    //     .item_stacks
    //     .iter()
    //     .map(|stack| ItemStackUpdateEvent {
    //         id: stack.id,
    //         data: if stack.despawned {
    //             None
    //         } else {
    //             Some((stack.stack, stack.pos))
    //         },
    //     })
    //     .collect()
}

pub fn get_all_active_chunks(players: &HashMap<PlayerId, Player>, radius: i32) -> Vec<IVec3> {
    let player_chunks: Vec<IVec3> = players
        .values()
        .map(|v| world_position_to_chunk_position(v.position))
        .flat_map(|v| get_player_nearby_chunks_coords(v, radius))
        .collect();

    let mut chunks: Vec<IVec3> = Vec::new();

    for c in player_chunks {
        if !chunks.contains(&c) {
            chunks.push(c);
        }
    }

    chunks
}

fn get_player_nearby_chunks_coords(
    player_chunk_position: IVec3,
    render_distance: i32,
) -> Vec<IVec3> {
    let mut chunks: Vec<IVec3> = Vec::new();
    for x in -render_distance..=render_distance {
        for y in -render_distance..=render_distance {
            for z in -render_distance..=render_distance {
                chunks.push(player_chunk_position + IVec3::new(x, y, z));
            }
        }
    }

    // let's sort by distance to player
    chunks.sort_by_key(|&c| (c - player_chunk_position).length_squared());

    chunks
}
