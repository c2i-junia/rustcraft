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
    chunk_distance, global_block_to_chunk_pos, world_position_to_chunk_position, Chunk, ChunkMap,
    MobId, ServerMob,
};
use shared::{CHUNK_SIZE, UNLOAD_MARGIN};
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

    for client in server.clients_id() {
        let Some(player) = players.get(&client) else {
            continue;
        };

        let render_distance = world_map.render_distances.get(player.id);
        send_nearby_mobs(
            &mut server,
            client,
            player.position,
            render_distance,
            &world_map.mobs,
        );

        let (unloads, block_updates, new_map) = world_messages_around(
            chunks,
            &changed,
            world_position_to_chunk_position(player.position),
            render_distance,
            world_map.sent_chunks.entry(player.id).or_default(),
        );

        // One channel carries the three, in this order: a chunk is unloaded before it can be
        // sent again
        if !unloads.is_empty() {
            server.send_game_message(client, ServerToClientMessage::ChunkUnloads(unloads));
        }
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

/// Sends `client` the state of the mobs within the render distance of its player.
fn send_nearby_mobs(
    server: &mut RenetServer,
    client: ClientId,
    player_position: Vec3,
    render_distance: i32,
    mobs: &HashMap<MobId, ServerMob>,
) {
    let reach = (render_distance * CHUNK_SIZE) as f32;
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

/// What the client of a player in `player_chunk` must be sent: the chunks it must forget, then
/// the updates of its chunks, then the chunks within `render_distance` it lacks. The unloads
/// come first, so that no update is sent for a chunk the client is told to forget.
fn world_messages_around(
    chunks: &ChunkMap,
    changed: &[IVec3],
    player_chunk: IVec3,
    render_distance: i32,
    sent: &mut HashSet<IVec3>,
) -> (Vec<IVec3>, Vec<BlockUpdate>, HashMap<IVec3, Chunk>) {
    let unloads = take_chunks_to_unload(player_chunk, render_distance, sent);
    let nearby = get_player_nearby_chunks_coords(player_chunk, render_distance);
    let (updates, new_chunks) = client_world_messages(chunks, changed, &nearby, sent);
    (unloads, updates, new_chunks)
}

/// The chunks of `sent` more than `render_distance + UNLOAD_MARGIN` chunks from
/// `player_chunk`, in no order. They are removed from `sent`, so that they are sent again once
/// the player comes back. The chunks of the margin are kept but never sent first: walking to
/// and fro across a chunk border neither unloads nor sends anything. A lowered render
/// distance unloads the same way.
fn take_chunks_to_unload(
    player_chunk: IVec3,
    render_distance: i32,
    sent: &mut HashSet<IVec3>,
) -> Vec<IVec3> {
    let mut unloads = Vec::new();
    sent.retain(|&chunk| {
        let kept = chunk_distance(chunk, player_chunk) <= render_distance + UNLOAD_MARGIN;
        if !kept {
            unloads.push(chunk);
        }
        kept
    });
    unloads
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

/// The chunks within the `radius` of each player, each listed once, in the order the players
/// reach them.
pub fn get_all_active_chunks(
    players: &HashMap<PlayerId, Player>,
    radius: impl Fn(PlayerId) -> i32,
) -> Vec<IVec3> {
    let mut seen = HashSet::new();
    players
        .iter()
        .flat_map(|(&id, player)| {
            let chunk = world_position_to_chunk_position(player.position);
            get_player_nearby_chunks_coords(chunk, radius(id))
        })
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

    /// Loaded chunks at x in `min..=max`, on the line y = z = 0.
    fn chunks_between(min: i32, max: i32) -> ChunkMap {
        ChunkMap::with_loaded_region(
            IVec3::new(min * CHUNK_SIZE, 0, 0),
            IVec3::new((max + 1) * CHUNK_SIZE - 1, 0, 0),
        )
    }

    /// The x of each chunk, sorted.
    fn xs<'a>(chunks: impl IntoIterator<Item = &'a IVec3>) -> Vec<i32> {
        let mut xs: Vec<i32> = chunks.into_iter().map(|chunk| chunk.x).collect();
        xs.sort();
        xs
    }

    /// The render distance of the players of these tests.
    const RENDER_DISTANCE: i32 = 2;
    /// The farthest a chunk a client has may be from its player.
    const KEPT: i32 = RENDER_DISTANCE + UNLOAD_MARGIN;

    /// What the client of a player in the chunk `(x, 0, 0)` is sent when no block changed.
    fn walk_to(
        chunks: &ChunkMap,
        x: i32,
        sent: &mut HashSet<IVec3>,
    ) -> (Vec<IVec3>, HashMap<IVec3, Chunk>) {
        let (unloads, updates, new_chunks) =
            world_messages_around(chunks, &[], IVec3::new(x, 0, 0), RENDER_DISTANCE, sent);
        assert!(updates.is_empty());
        (unloads, new_chunks)
    }

    #[test]
    fn the_chunks_beyond_the_margin_are_unloaded_and_forgotten() {
        let kept = [
            IVec3::ZERO,
            IVec3::new(KEPT, 0, 0),
            IVec3::new(KEPT, KEPT, -KEPT),
        ];
        let far = [IVec3::new(KEPT + 1, 0, 0), IVec3::new(0, -KEPT - 1, 0)];
        let mut sent: HashSet<IVec3> = kept.into_iter().chain(far).collect();

        let mut unloads = take_chunks_to_unload(IVec3::ZERO, RENDER_DISTANCE, &mut sent);

        unloads.sort_by_key(|c| (c.x, c.y, c.z));
        assert_eq!(unloads, vec![far[1], far[0]]);
        assert_eq!(sent, HashSet::from(kept));
    }

    #[test]
    fn only_the_render_distance_is_sent_and_the_margin_is_kept_but_never_sent_first() {
        let chunks = chunks_between(-KEPT - 2, KEPT + 2);
        let mut sent = HashSet::new();

        let (unloads, new_chunks) = walk_to(&chunks, 0, &mut sent);
        assert!(unloads.is_empty());
        assert_eq!(
            xs(new_chunks.keys()),
            (-RENDER_DISTANCE..=RENDER_DISTANCE).collect::<Vec<_>>()
        );

        // one chunk east: the westmost chunk falls in the margin and stays
        let (unloads, new_chunks) = walk_to(&chunks, 1, &mut sent);
        assert!(unloads.is_empty());
        assert_eq!(xs(new_chunks.keys()), vec![RENDER_DISTANCE + 1]);

        // one more: it is now beyond the margin
        let (unloads, new_chunks) = walk_to(&chunks, 2, &mut sent);
        assert_eq!(xs(&unloads), vec![-RENDER_DISTANCE]);
        assert_eq!(xs(new_chunks.keys()), vec![RENDER_DISTANCE + 2]);
        assert!(!sent.contains(&IVec3::new(-RENDER_DISTANCE, 0, 0)));
    }

    #[test]
    fn walking_to_and_fro_across_a_border_neither_unloads_nor_sends() {
        let chunks = chunks_between(-KEPT - 2, KEPT + 2);
        let mut sent = HashSet::new();
        walk_to(&chunks, 0, &mut sent);
        walk_to(&chunks, 1, &mut sent);

        for x in [0, 1, 0, 1, 0] {
            let (unloads, new_chunks) = walk_to(&chunks, x, &mut sent);
            assert!(unloads.is_empty() && new_chunks.is_empty(), "at {x}");
        }
    }

    #[test]
    fn an_unloaded_chunk_is_sent_again_when_the_player_comes_back() {
        let chunks = chunks_between(-KEPT - 2, KEPT + 2);
        let west = IVec3::new(-RENDER_DISTANCE, 0, 0);
        let mut sent = HashSet::new();
        walk_to(&chunks, 0, &mut sent);

        let (unloads, _) = walk_to(&chunks, UNLOAD_MARGIN + 1, &mut sent);
        assert_eq!(unloads, vec![west]);

        let (unloads, new_chunks) = walk_to(&chunks, 0, &mut sent);
        assert!(new_chunks.contains_key(&west));
        // the chunks the walk east brought are in the margin again, or beyond it
        assert!(unloads.iter().all(|c| c.x > KEPT));
    }

    #[test]
    fn a_walking_client_holds_what_the_server_thinks_and_no_update_misses_its_chunk() {
        let mut chunks = chunks_between(-KEPT - 4, KEPT + 4);
        let mut sent = HashSet::new();
        let mut client = ChunkMap::default();

        for (step, x) in [0, 1, 2, 3, 2, 1, 0, -1, -2, -1, 0].into_iter().enumerate() {
            // a block changes in every chunk of the row, those about to be unloaded included
            for chunk_x in -KEPT - 4..=KEPT + 4 {
                let pos = IVec3::new(chunk_x * CHUNK_SIZE + step as i32 % CHUNK_SIZE, 0, 0);
                chunks.set_block(pos, stone());
            }
            let changed = chunks.take_changed_blocks();
            let (unloads, updates, new_chunks) = world_messages_around(
                &chunks,
                &changed,
                IVec3::new(x, 0, 0),
                RENDER_DISTANCE,
                &mut sent,
            );

            for chunk in unloads {
                assert!(client.remove_chunk(chunk).is_some(), "step {step}: {chunk}");
            }
            for update in &updates {
                let chunk = global_block_to_chunk_pos(&update.position);
                assert!(client.is_chunk_loaded(chunk), "step {step}: {chunk}");
            }
            deliver(&mut client, (updates, new_chunks));

            let held: HashSet<IVec3> = client.chunk_positions().collect();
            assert_eq!(held, sent, "step {step}");
            for chunk in &held {
                assert_eq!(client.chunk(*chunk), chunks.chunk(*chunk), "step {step}");
                assert!(chunk_distance(*chunk, IVec3::new(x, 0, 0)) <= KEPT);
            }
        }
    }

    #[test]
    fn a_lowered_render_distance_unloads_and_a_raised_one_sends_more() {
        let chunks = chunks_between(-6, 6);
        let mut sent = HashSet::new();
        let around = |distance, sent: &mut HashSet<IVec3>| {
            let (unloads, _, new_chunks) =
                world_messages_around(&chunks, &[], IVec3::ZERO, distance, sent);
            (xs(&unloads), xs(new_chunks.keys()))
        };
        around(4, &mut sent);

        // down to 1: the margin keeps the chunks at 2
        let (unloads, new_chunks) = around(1, &mut sent);
        assert_eq!(unloads, vec![-4, -3, 3, 4]);
        assert!(new_chunks.is_empty());

        // up to 3: the chunks at 2 are still there, those at 3 come again
        let (unloads, new_chunks) = around(3, &mut sent);
        assert!(unloads.is_empty());
        assert_eq!(new_chunks, vec![-3, 3]);
    }

    /// The messages of the chunk channel the server queued for `client`.
    fn received_world_messages(
        server: &mut RenetServer,
        client: ClientId,
    ) -> Vec<ServerToClientMessage> {
        let mut connection = bevy_renet::renet::RenetClient::new(shared::get_shared_renet_config());
        for packet in server.get_packets_to_send(client).unwrap() {
            connection.process_packet(&packet);
        }
        std::iter::from_fn(|| connection.receive_message(shared::STC_CHUNK_DATA_CHANNEL))
            .map(|payload| shared::payload_to_game_message(&payload).unwrap())
            .collect()
    }

    #[test]
    fn each_client_is_sent_the_chunks_within_its_own_render_distance_of_its_own_player() {
        let far = 10;
        // client: (chunk of its player, render distance)
        let clients = [(1, (0, 2)), (2, (far, 1))];
        let mut players = HashMap::new();
        let mut render_distances = crate::world::data::RenderDistances::default();
        for (id, (chunk_x, distance)) in clients {
            render_distances.set(id, distance, 6);
            let position = Vec3::new((chunk_x * CHUNK_SIZE) as f32 + 0.5, 0.5, 0.5);
            players.insert(
                id,
                Player::new(id, "p".into(), position, Transform::default()),
            );
        }
        let mut server = RenetServer::new(shared::get_shared_renet_config());
        server.add_connection(1);
        server.add_connection(2);
        let mut app = App::new();
        app.insert_resource(server)
            .insert_resource(ServerTime(0))
            .insert_resource(ServerWorldMap {
                chunks: chunks_between(-KEPT, far + KEPT),
                players,
                render_distances,
                ..default()
            });

        app.add_systems(Update, broadcast_world_state);
        app.update();

        let mut server = app.world_mut().resource_mut::<RenetServer>();
        for (client, (center, distance)) in clients {
            let chunks: Vec<IVec3> = received_world_messages(&mut server, client)
                .into_iter()
                .flat_map(|message| match message {
                    ServerToClientMessage::WorldUpdate(update) => {
                        update.new_map.into_keys().collect::<Vec<_>>()
                    }
                    other => panic!("client {client}: unexpected {other:?}"),
                })
                .collect();
            let distance = distance as i32;
            let expected: Vec<i32> = (center - distance..=center + distance).collect();
            assert_eq!(xs(&chunks), expected, "client {client}");
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
        let active = get_all_active_chunks(&players, |_| 1);
        let unique: HashSet<&IVec3> = active.iter().collect();
        assert_eq!(active.len(), unique.len());
        // chunk columns -1..=1 and 0..=2 around the two players, three chunks high and deep
        assert_eq!(unique.len(), 4 * 9);
    }
}
