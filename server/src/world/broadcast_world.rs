use crate::init::ServerTime;
use crate::network::extensions::SendGameMessageExtension;
use crate::world::data::ServerWorldMap;
use bevy::math::IVec3;
use bevy::prelude::*;
use bevy_ecs::system::ResMut;
use bevy_renet::RenetServer;
use shared::messages::mob::MobUpdateEvent;
use shared::messages::{PlayerId, ServerToClientMessage, WorldUpdate};
use shared::players::Player;
use shared::world::{global_block_to_chunk_pos, world_position_to_chunk_position, Chunk, ChunkMap};
use shared::CHUNK_SIZE;
use std::collections::{HashMap, HashSet};

pub const BROADCAST_RENDER_DISTANCE: i32 = 1;

/// Most chunks sent to one client by one broadcast.
const MAX_CHUNKS_PER_BROADCAST: usize = 10;

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
    let players = &world_map.players;
    let chunks = &mut world_map.chunks;

    forget_changed_chunks(chunks, &mut world_map.sent_chunks);

    let active_chunks = get_all_active_chunks(players, BROADCAST_RENDER_DISTANCE);

    for client in server.clients_id().iter_mut() {
        let player = players.get(client);
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
            new_map: get_world_map_chunks_to_send(
                chunks,
                &active_chunks,
                world_map.sent_chunks.entry(player.id).or_default(),
            ),
            mobs: mobs.clone(),
        };

        if msg.new_map.is_empty() {
            continue;
        }

        let message = ServerToClientMessage::WorldUpdate(msg);

        server.send_game_message(*client, message);
    }
}

/// Makes every chunk that holds a block changed since the last call to be sent again to the
/// clients that had it.
fn forget_changed_chunks(
    chunks: &mut ChunkMap,
    sent_chunks: &mut HashMap<PlayerId, HashSet<IVec3>>,
) {
    let changed_chunks: HashSet<IVec3> = chunks
        .take_changed_blocks()
        .iter()
        .map(global_block_to_chunk_pos)
        .collect();
    for sent in sent_chunks.values_mut() {
        sent.retain(|chunk_pos| !changed_chunks.contains(chunk_pos));
    }
}

/// The chunks, nearest first, that a client has not received yet, at most
/// `MAX_CHUNKS_PER_BROADCAST`. They are recorded in `sent`.
fn get_world_map_chunks_to_send(
    chunks: &ChunkMap,
    active_chunks: &[IVec3],
    sent: &mut HashSet<IVec3>,
) -> HashMap<IVec3, Chunk> {
    let mut map: HashMap<IVec3, Chunk> = HashMap::new();

    for &c in active_chunks {
        if map.len() >= MAX_CHUNKS_PER_BROADCAST {
            break;
        }

        // Only chunks that exist and that the client has not received
        let Some(chunk) = chunks.chunk(c) else {
            continue;
        };
        if sent.insert(c) {
            map.insert(c, chunk.clone());
        }
    }

    map
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

#[cfg(test)]
mod tests {
    use super::*;
    use shared::world::{BlockData, BlockDirection, BlockId};

    fn stone() -> BlockData {
        BlockData::new(BlockId::Stone, BlockDirection::Front)
    }

    /// Loaded chunks at x in `0..count`, on the line y = z = 0.
    fn row_of_chunks(count: i32) -> ChunkMap {
        ChunkMap::with_loaded_region(IVec3::ZERO, IVec3::new(count * CHUNK_SIZE - 1, 0, 0))
    }

    fn active_row(count: i32) -> Vec<IVec3> {
        (0..count).map(|x| IVec3::new(x, 0, 0)).collect()
    }

    #[test]
    fn a_chunk_is_sent_once_to_a_client() {
        let chunks = row_of_chunks(3);
        let mut sent = HashSet::new();

        let first = get_world_map_chunks_to_send(&chunks, &active_row(3), &mut sent);
        let second = get_world_map_chunks_to_send(&chunks, &active_row(3), &mut sent);

        assert_eq!(first.len(), 3);
        assert!(second.is_empty());
    }

    #[test]
    fn chunks_that_do_not_exist_are_skipped_and_sent_once_they_do() {
        let mut chunks = row_of_chunks(1);
        let mut sent = HashSet::new();

        let first = get_world_map_chunks_to_send(&chunks, &active_row(2), &mut sent);
        assert_eq!(first.keys().copied().collect::<Vec<_>>(), vec![IVec3::ZERO]);

        chunks.insert_chunk(IVec3::X, Chunk::new());
        let second = get_world_map_chunks_to_send(&chunks, &active_row(2), &mut sent);
        assert_eq!(second.keys().copied().collect::<Vec<_>>(), vec![IVec3::X]);
    }

    #[test]
    fn at_most_ten_chunks_are_sent_per_call_nearest_first() {
        let chunks = row_of_chunks(25);
        let mut sent = HashSet::new();

        let first = get_world_map_chunks_to_send(&chunks, &active_row(25), &mut sent);
        let second = get_world_map_chunks_to_send(&chunks, &active_row(25), &mut sent);

        assert_eq!(first.len(), 10);
        assert!((0..10).all(|x| first.contains_key(&IVec3::new(x, 0, 0))));
        assert_eq!(second.len(), 10);
        assert!((10..20).all(|x| second.contains_key(&IVec3::new(x, 0, 0))));
    }

    #[test]
    fn a_changed_chunk_is_sent_again_to_every_client_that_had_it() {
        let mut chunks = row_of_chunks(2);
        let active = active_row(2);
        let mut sent_chunks: HashMap<PlayerId, HashSet<IVec3>> = HashMap::new();
        for client in [1, 2, 3] {
            get_world_map_chunks_to_send(&chunks, &active, sent_chunks.entry(client).or_default());
        }

        chunks.set_block(IVec3::new(CHUNK_SIZE + 4, 2, 2), stone());
        forget_changed_chunks(&mut chunks, &mut sent_chunks);

        for client in [1, 2, 3] {
            let resent = get_world_map_chunks_to_send(
                &chunks,
                &active,
                sent_chunks.entry(client).or_default(),
            );
            assert_eq!(
                resent.keys().copied().collect::<Vec<_>>(),
                vec![IVec3::X],
                "client {client}"
            );
            assert_eq!(resent[&IVec3::X].iter().count(), 1);
        }

        // nothing changed since: nothing to send
        forget_changed_chunks(&mut chunks, &mut sent_chunks);
        assert!(
            get_world_map_chunks_to_send(&chunks, &active, sent_chunks.entry(1).or_default())
                .is_empty()
        );
    }

    #[test]
    fn a_block_change_on_a_chunk_border_resends_only_its_own_chunk() {
        let mut chunks = row_of_chunks(2);
        let mut sent_chunks: HashMap<PlayerId, HashSet<IVec3>> = HashMap::new();
        get_world_map_chunks_to_send(&chunks, &active_row(2), sent_chunks.entry(1).or_default());

        chunks.set_block(IVec3::new(CHUNK_SIZE - 1, 0, 0), stone());
        forget_changed_chunks(&mut chunks, &mut sent_chunks);

        assert_eq!(sent_chunks[&1], HashSet::from([IVec3::X]));
    }

    #[test]
    fn active_chunks_of_several_players_have_no_duplicates() {
        let mut players = HashMap::new();
        for (id, x) in [(1, 0.0), (2, 20.0)] {
            players.insert(
                id,
                Player::new(
                    id,
                    "p".into(),
                    bevy::math::Vec3::new(x, 0.0, 0.0),
                    Transform::default(),
                ),
            );
        }
        let active = get_all_active_chunks(&players, 1);
        let unique: HashSet<&IVec3> = active.iter().collect();
        assert_eq!(active.len(), unique.len());
        // chunk columns -1..=1 and 0..=2 around the two players, three chunks high and deep
        assert_eq!(unique.len(), 4 * 9);
    }
}
