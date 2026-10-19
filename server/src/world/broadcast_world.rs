use crate::init::ServerTime;
use crate::network::extensions::SendGameMessageExtension;
use crate::world::data::ServerWorldMap;
use bevy::math::IVec3;
use bevy::prelude::*;
use bevy_ecs::system::ResMut;
use bevy_renet::{renet::ClientId, RenetServer};
use shared::messages::mob::MobUpdateEvent;
use shared::messages::{BlockUpdate, PlayerId, ServerToClientMessage, WorldUpdate};
use shared::players::Player;
use shared::world::{
    global_block_to_chunk_pos, world_position_to_chunk_position, Chunk, ChunkMap, MobId, ServerMob,
};
use shared::{BROADCAST_CHUNK_RADIUS, CHUNK_SIZE};
use std::collections::{HashMap, HashSet};

/// Most chunks sent to one client by one broadcast.
const MAX_CHUNKS_PER_BROADCAST: usize = 10;

pub fn broadcast_world_state(
    mut server: ResMut<RenetServer>,
    time: Res<ServerTime>,
    mut world_map: ResMut<ServerWorldMap>,
) {
    let ts = shared::utils::unix_time_ms();

    let world_map = world_map.as_mut();

    let players = &world_map.players;
    let chunks = &mut world_map.chunks;

    let changed = chunks.take_changed_blocks();
    // The only drain of the change log is also what tells the save which chunks to write. A
    // position the server only restated (`touch_block`) marks its chunk dirty too, which costs
    // one needless chunk write and is not worth telling apart.
    world_map
        .dirty_chunks
        .extend(changed.iter().map(global_block_to_chunk_pos));

    let active_chunks = get_all_active_chunks(players, BROADCAST_CHUNK_RADIUS);

    for client in server.clients_id() {
        let Some(player) = players.get(&client) else {
            continue;
        };

        send_nearby_mobs(&mut server, client, player.position, &world_map.mobs);

        let (block_updates, new_map) = client_world_messages(
            chunks,
            &changed,
            &active_chunks,
            world_map.sent_chunks.entry(player.id).or_default(),
        );

        if !block_updates.is_empty() {
            server.send_game_message(client, ServerToClientMessage::BlockUpdates(block_updates));
        }
        if !new_map.is_empty() {
            let update = WorldUpdate {
                tick: time.0,
                time: ts,
                new_map,
                mobs: world_map.mobs.clone(),
            };
            server.send_game_message(client, ServerToClientMessage::WorldUpdate(update));
        }
    }
}

/// Sends `client` the state of the mobs within broadcast distance of its player.
fn send_nearby_mobs(
    server: &mut RenetServer,
    client: ClientId,
    player_position: Vec3,
    mobs: &HashMap<MobId, ServerMob>,
) {
    let reach = (BROADCAST_CHUNK_RADIUS * CHUNK_SIZE) as f32;
    for (id, mob) in mobs {
        if mob.position.distance(player_position) < reach {
            server.send_game_message(
                client,
                ServerToClientMessage::MobUpdate(MobUpdateEvent {
                    id: *id,
                    mob: mob.clone(),
                }),
            );
        }
    }
}

/// What one client must be sent: the current state of the changed blocks in the chunks it
/// already has, then the chunks it does not have yet. The updates are computed before the new
/// chunks are recorded in `sent`: a chunk sent now already holds the changes, and every chunk
/// the client has gets its changes as updates, so the client never misses one and is never
/// sent an update for a chunk it lacks.
fn client_world_messages(
    chunks: &ChunkMap,
    changed: &[IVec3],
    active_chunks: &[IVec3],
    sent: &mut HashSet<IVec3>,
) -> (Vec<BlockUpdate>, HashMap<IVec3, Chunk>) {
    let updates = changed
        .iter()
        .filter(|pos| sent.contains(&global_block_to_chunk_pos(pos)))
        .map(|&position| BlockUpdate {
            position,
            block: chunks.get_block(position).copied(),
        })
        .collect();
    let new_chunks = get_world_map_chunks_to_send(chunks, active_chunks, sent);
    (updates, new_chunks)
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

/// The chunks within `radius` chunks of a player, each listed once, in the order the players
/// reach them.
pub fn get_all_active_chunks(players: &HashMap<PlayerId, Player>, radius: i32) -> Vec<IVec3> {
    let mut seen = HashSet::new();
    players
        .values()
        .map(|player| world_position_to_chunk_position(player.position))
        .flat_map(|chunk| get_player_nearby_chunks_coords(chunk, radius))
        .filter(|chunk| seen.insert(*chunk))
        .collect()
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

    /// What a client holds: the chunks and updates it received, applied in order.
    fn deliver(
        client: &mut ChunkMap,
        (updates, new_chunks): (Vec<BlockUpdate>, HashMap<IVec3, Chunk>),
    ) {
        for update in updates {
            match update.block {
                Some(block) => client.set_block(update.position, block),
                None => {
                    client.remove_block(update.position);
                }
            }
        }
        for (pos, chunk) in new_chunks {
            client.insert_chunk(pos, chunk);
        }
    }

    #[test]
    fn a_changed_block_is_sent_as_an_update_to_every_client_that_has_its_chunk() {
        let mut chunks = row_of_chunks(2);
        let active = active_row(2);
        let mut sent_chunks: HashMap<PlayerId, HashSet<IVec3>> = HashMap::new();
        for client in [1, 2, 3] {
            get_world_map_chunks_to_send(&chunks, &active, sent_chunks.entry(client).or_default());
        }

        let pos = IVec3::new(CHUNK_SIZE + 4, 2, 2);
        chunks.set_block(pos, stone());
        let changed = chunks.take_changed_blocks();

        for client in [1, 2, 3] {
            let (updates, new_chunks) = client_world_messages(
                &chunks,
                &changed,
                &active,
                sent_chunks.entry(client).or_default(),
            );
            assert_eq!(
                updates,
                vec![BlockUpdate {
                    position: pos,
                    block: Some(stone())
                }],
                "client {client}"
            );
            assert!(new_chunks.is_empty(), "client {client}");
        }

        // nothing changed since: nothing to send
        let changed = chunks.take_changed_blocks();
        let (updates, new_chunks) = client_world_messages(
            &chunks,
            &changed,
            &active,
            sent_chunks.entry(1).or_default(),
        );
        assert!(updates.is_empty() && new_chunks.is_empty());
    }

    #[test]
    fn a_removed_block_is_sent_as_air() {
        let mut chunks = row_of_chunks(1);
        let active = active_row(1);
        let mut sent = HashSet::new();
        get_world_map_chunks_to_send(&chunks, &active, &mut sent);
        let pos = IVec3::new(1, 2, 3);
        chunks.set_block(pos, stone());
        chunks.take_changed_blocks();
        chunks.remove_block(pos);

        let changed = chunks.take_changed_blocks();
        let (updates, _) = client_world_messages(&chunks, &changed, &active, &mut sent);

        assert_eq!(
            updates,
            vec![BlockUpdate {
                position: pos,
                block: None
            }]
        );
    }

    #[test]
    fn a_touched_block_is_restated_with_its_true_value() {
        let mut chunks = row_of_chunks(1);
        let active = active_row(1);
        let mut sent = HashSet::new();
        get_world_map_chunks_to_send(&chunks, &active, &mut sent);

        chunks.touch_block(IVec3::new(5, 5, 5));
        let changed = chunks.take_changed_blocks();
        let (updates, _) = client_world_messages(&chunks, &changed, &active, &mut sent);

        assert_eq!(
            updates,
            vec![BlockUpdate {
                position: IVec3::new(5, 5, 5),
                block: None
            }]
        );
    }

    #[test]
    fn changes_in_chunks_the_client_lacks_are_not_sent_as_updates() {
        let mut chunks = row_of_chunks(2);
        let mut sent = HashSet::from([IVec3::ZERO]);
        chunks.set_block(IVec3::new(1, 0, 0), stone());
        chunks.set_block(IVec3::new(CHUNK_SIZE + 1, 0, 0), stone());

        let changed = chunks.take_changed_blocks();
        let (updates, new_chunks) =
            client_world_messages(&chunks, &changed, &active_row(2), &mut sent);

        let positions: Vec<IVec3> = updates.iter().map(|u| u.position).collect();
        assert_eq!(positions, vec![IVec3::new(1, 0, 0)]);
        // the chunk it lacks comes whole, with the change in it
        assert_eq!(
            new_chunks.keys().copied().collect::<Vec<_>>(),
            vec![IVec3::X]
        );
        assert_eq!(new_chunks[&IVec3::X].iter().count(), 1);
    }

    #[test]
    fn a_chunk_held_back_by_the_limit_is_sent_whole_with_the_change_and_no_update() {
        let mut chunks = row_of_chunks(12);
        let active = active_row(12);
        let mut sent = HashSet::new();
        let first = get_world_map_chunks_to_send(&chunks, &active, &mut sent);
        assert!(!first.contains_key(&IVec3::new(11, 0, 0)));

        // chunk 11 is still waiting for its turn when one of its blocks changes
        let pos = IVec3::new(11 * CHUNK_SIZE + 1, 2, 3);
        chunks.set_block(pos, stone());
        let changed = chunks.take_changed_blocks();
        let (updates, new_chunks) = client_world_messages(&chunks, &changed, &active, &mut sent);

        assert!(updates.is_empty());
        assert_eq!(
            new_chunks[&IVec3::new(11, 0, 0)].get(IVec3::new(1, 2, 3)),
            Some(&stone())
        );
    }

    #[test]
    fn a_broadcast_marks_the_chunks_of_the_changed_blocks_dirty() {
        let mut app = App::new();
        app.insert_resource(RenetServer::new(shared::get_shared_renet_config()))
            .insert_resource(ServerTime(0))
            .insert_resource(ServerWorldMap {
                chunks: row_of_chunks(3),
                ..default()
            });
        {
            let mut map = app.world_mut().resource_mut::<ServerWorldMap>();
            map.chunks.set_block(IVec3::new(1, 2, 3), stone());
            map.chunks
                .set_block(IVec3::new(2 * CHUNK_SIZE + 1, 0, 0), stone());
            assert!(map.dirty_chunks.is_empty());
        }

        app.add_systems(Update, broadcast_world_state);
        app.update();

        let map = app.world().resource::<ServerWorldMap>();
        assert_eq!(
            map.dirty_chunks,
            HashSet::from([IVec3::ZERO, IVec3::new(2, 0, 0)])
        );
    }

    #[test]
    fn a_broadcast_without_clients_empties_the_change_log() {
        let mut app = App::new();
        app.insert_resource(RenetServer::new(shared::get_shared_renet_config()))
            .insert_resource(ServerTime(0))
            .insert_resource(ServerWorldMap {
                chunks: row_of_chunks(1),
                ..default()
            });
        app.world_mut()
            .resource_mut::<ServerWorldMap>()
            .chunks
            .set_block(IVec3::new(1, 2, 3), stone());

        app.add_systems(Update, broadcast_world_state);
        app.update();

        let mut map = app.world_mut().resource_mut::<ServerWorldMap>();
        assert!(map.chunks.take_changed_blocks().is_empty());
    }

    #[test]
    fn a_client_ends_with_the_true_blocks_whatever_the_interleaving() {
        // The client receives the chunk at the first broadcast; the block changes before,
        // during, or after it, for every broadcast index.
        for change_at in 0..4 {
            let mut chunks = row_of_chunks(1);
            let active = active_row(1);
            let mut sent = HashSet::new();
            let mut client = ChunkMap::default();
            let pos = IVec3::new(2, 3, 4);

            for broadcast in 0..4 {
                if broadcast == change_at {
                    chunks.set_block(pos, stone());
                }
                let changed = chunks.take_changed_blocks();
                // the chunk is only sent at the second broadcast: it is not active before
                let active_now: &[IVec3] = if broadcast >= 1 { &active } else { &[] };
                deliver(
                    &mut client,
                    client_world_messages(&chunks, &changed, active_now, &mut sent),
                );
                if broadcast >= 1 {
                    assert_eq!(
                        client.get_block(pos),
                        chunks.get_block(pos),
                        "change at {change_at}, after broadcast {broadcast}"
                    );
                }
            }
        }
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
