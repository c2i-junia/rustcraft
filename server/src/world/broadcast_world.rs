use crate::init::ServerTime;
use crate::network::extensions::SendGameMessageExtension;
use crate::world::data::{SentChunks, ServerWorldMap};
use crate::world::dropped_items::dropped_items_update;
use crate::world::pool::server_pool;
use bevy::math::IVec3;
use bevy::prelude::*;
use bevy_ecs::system::ResMut;
use bevy_renet::RenetServer;
use shared::messages::mob::MobUpdateEvent;
use shared::messages::{BlockUpdate, PlayerId, ServerToClientMessage, WorldUpdate};
use shared::players::Player;
use shared::world::{
    global_block_to_chunk_pos, render_distance_offsets, world_position_to_chunk_position, Chunk,
    ChunkMap, ChunkReach, MobId, Portals, ServerMob,
};
use shared::{game_message_to_payload, CHUNK_SIZE, STC_CHUNK_DATA_CHANNEL, UNLOAD_MARGIN};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

/// Most chunks sent to one client by one broadcast.
const MAX_CHUNKS_PER_BROADCAST: usize = 1024;
/// Most of them that the chunks around the twins of the portals take (`ChunkReach`), the
/// player's own taking the rest: neither starves while the other is being sent.
const PORTAL_CHUNKS_PER_BROADCAST: usize = MAX_CHUNKS_PER_BROADCAST / 4;

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
    // The contents of a chest and a furnace only reach the clients through the player
    // updates, but their chunk must be saved with them
    let changed_contents = [
        chunks.take_changed_containers(),
        chunks.take_changed_furnaces(),
    ];
    world_map.dirty_chunks.extend(
        changed_contents
            .iter()
            .flatten()
            .map(global_block_to_chunk_pos),
    );
    let (changed_items, removed_items) = world_map.dropped_items.take_changes();
    let changed_mobs = std::mem::take(&mut world_map.changed_mobs);

    for client in server.clients_id() {
        let Some(player) = players.get(&client) else {
            continue;
        };

        let render_distance = world_map.render_distances.get(player.id);
        let reach = ChunkReach::of_player(player, render_distance, chunks.portals());
        for message in mob_messages(
            player.position,
            &reach,
            &world_map.mobs,
            &changed_mobs,
            world_map.sent_mobs.entry(player.id).or_default(),
        ) {
            server.send_game_message(client, message);
        }

        let sent_chunks = world_map.sent_chunks.entry(player.id).or_default();
        let (unloads, block_updates, new_map) =
            world_messages_around(chunks, &changed, &reach, sent_chunks);
        // A client is told of the items in the chunks it has, those sent now included
        let items = dropped_items_update(
            &world_map.dropped_items,
            &changed_items,
            &removed_items,
            &sent_chunks.chunks,
            world_map.sent_items.entry(player.id).or_default(),
        );
        if !items.is_empty() {
            server.send_game_message(client, ServerToClientMessage::DroppedItems(items));
        }

        // One channel carries the three, in this order: a chunk is unloaded before it can be
        // sent again
        if !unloads.is_empty() {
            server.send_game_message(client, ServerToClientMessage::ChunkUnloads(unloads));
        }
        if !block_updates.is_empty() {
            server.send_game_message(client, ServerToClientMessage::BlockUpdates(block_updates));
        }
        for payload in encode_world_updates(new_map, &reach, time.0, ts) {
            server.send_message(client, STC_CHUNK_DATA_CHANNEL, payload);
        }
    }
}

/// The most chunks in one `WorldUpdate`: the new chunks of a tick are split into messages of
/// this many, encoded in parallel, which the client decodes in parallel too.
const CHUNKS_PER_MESSAGE: usize = 32;

/// The messages that carry `new_map` to a client whose player is in `player_chunk`, encoded,
/// nearest chunks first. Each message copies its chunks and is encoded on its own task. They
/// carry no mobs: those are sent on their own (`mob_messages`).
fn encode_world_updates(
    new_map: HashMap<IVec3, &Chunk>,
    reach: &ChunkReach,
    tick: u64,
    time: u64,
) -> Vec<Vec<u8>> {
    let mut chunks: Vec<(IVec3, &Chunk)> = new_map.into_iter().collect();
    // The player's own chunks first, nearest to it first, then those around the twins, nearest
    // to one first
    let twins = reach.twins();
    chunks.sort_by_key(|&(pos, _)| {
        let distance = |center: IVec3| (pos - center).length_squared();
        if reach.within(pos, 0) {
            (false, distance(reach.center))
        } else {
            let to_twin = twins.iter().map(|&twin| distance(twin)).min();
            (true, to_twin.unwrap_or_else(|| distance(reach.center)))
        }
    });
    let encode = |_: usize, chunks: &[(IVec3, &Chunk)]| {
        game_message_to_payload(ServerToClientMessage::WorldUpdate(WorldUpdate {
            tick,
            time,
            new_map: chunks
                .iter()
                .map(|&(pos, chunk)| (pos, chunk.clone()))
                .collect(),
            mobs: HashMap::new(),
        }))
    };
    match chunks.len() {
        0 => Vec::new(),
        1..=CHUNKS_PER_MESSAGE => vec![encode(0, &chunks)],
        _ => server_pool().scope(|scope| {
            for (index, part) in chunks.chunks(CHUNKS_PER_MESSAGE).enumerate() {
                scope.spawn(async move { encode(index, part) });
            }
        }),
    }
}

/// What the client of a player at `player_position` must be told of the mobs, given the mobs
/// it holds, `sent`, which this updates: a mob that came within reach is sent, one within
/// reach is sent again when it changed this tick (`changed`), and the client forgets one that
/// left its reach or the world. Within reach is within the render distance of `reach`, or in
/// the chunks around the twin of a portal it sees (`ChunkReach::keeps`): a mob there is seen
/// through the portal, and one that walks through it stays known.
fn mob_messages(
    player_position: Vec3,
    reach: &ChunkReach,
    mobs: &HashMap<MobId, ServerMob>,
    changed: &HashSet<MobId>,
    sent: &mut HashSet<MobId>,
) -> Vec<ServerToClientMessage> {
    let distance = (reach.distance * CHUNK_SIZE) as f32;
    let keeps = reach.kept(0);
    let mut messages = Vec::new();
    for (id, mob) in mobs {
        let chunk = world_position_to_chunk_position(mob.position);
        let within = mob.position.distance(player_position) < distance
            || (keeps.contains(chunk) && !reach.within(chunk, 0));
        let known = sent.contains(id);
        if within && (!known || changed.contains(id)) {
            sent.insert(*id);
            messages.push(ServerToClientMessage::MobUpdate(MobUpdateEvent {
                id: *id,
                mob: mob.clone(),
            }));
        } else if !within && known {
            sent.remove(id);
            messages.push(ServerToClientMessage::MobRemoved(*id));
        }
    }
    sent.retain(|id| {
        let exists = mobs.contains_key(id);
        if !exists {
            messages.push(ServerToClientMessage::MobRemoved(*id));
        }
        exists
    });
    messages
}

/// What the client of a player of reach `reach` must be sent: the chunks it must forget, then
/// the updates of its chunks, then the chunks within the reach it lacks: its own, within the
/// render distance, and those around the twins of the portals, which take at most
/// `PORTAL_CHUNKS_PER_BROADCAST`. The unloads come first, so that no update is sent for a chunk
/// the client is told to forget.
///
/// While the reach stays the same, the chunks of its own a client holds all of, nearest first,
/// are not looked at again (`SentChunks::progress`): with a large render distance, they are
/// hundreds of thousands. Nothing is unloaded then, as nothing went out of reach. The same goes
/// for the chunks around the twins (`SentChunks::portal_progress`), listed once per reach: the
/// render distance of an approached twin is thousands.
pub(super) fn world_messages_around<'a>(
    chunks: &'a ChunkMap,
    changed: &[IVec3],
    reach: &ChunkReach,
    sent: &mut SentChunks,
) -> (Vec<IVec3>, Vec<BlockUpdate>, HashMap<IVec3, &'a Chunk>) {
    let (unloads, held) = match &sent.progress {
        Some((then, held)) if then == reach => (Vec::new(), *held),
        _ => (take_chunks_to_unload(reach, &mut sent.chunks), 0),
    };
    let (portal_chunks, portal_held) = match sent.portal_progress.take() {
        Some((then, list, held)) if then == *reach => (list, held),
        _ => (Arc::from(reach.portal_chunks()), 0),
    };
    let offsets = render_distance_offsets(reach.distance);
    let lacking = || offsets[held..].iter().map(|&offset| reach.center + offset);
    let portal_lacking = || portal_chunks[portal_held..].iter().copied();
    let updates = block_updates(chunks, changed, &sent.chunks);
    let mut new_chunks = HashMap::new();
    let portal_varied = add_chunks_to_send(
        chunks,
        portal_lacking(),
        &mut sent.chunks,
        PORTAL_CHUNKS_PER_BROADCAST,
        &mut new_chunks,
    );
    add_chunks_to_send(
        chunks,
        lacking(),
        &mut sent.chunks,
        MAX_CHUNKS_PER_BROADCAST - portal_varied,
        &mut new_chunks,
    );
    let now_held = lacking()
        .take_while(|chunk| sent.chunks.contains(chunk))
        .count();
    sent.progress = Some((reach.clone(), held + now_held));
    let portal_now_held = portal_lacking()
        .take_while(|chunk| sent.chunks.contains(chunk))
        .count();
    sent.portal_progress = Some((reach.clone(), portal_chunks, portal_held + portal_now_held));
    (unloads, updates, new_chunks)
}

/// The chunks of `sent` more than `UNLOAD_MARGIN` chunks beyond `reach` (`ChunkReach::keeps`),
/// in no order. They are removed from `sent`, so that they are sent again once the player comes
/// back. The chunks of the margin are kept but never sent first: walking to and fro across a
/// chunk border neither unloads nor sends anything. A lowered render distance, or a portal
/// that closed or moved, unloads the same way.
fn take_chunks_to_unload(reach: &ChunkReach, sent: &mut HashSet<IVec3>) -> Vec<IVec3> {
    let mut unloads = Vec::new();
    let keeps = reach.kept(UNLOAD_MARGIN);
    sent.retain(|&chunk| {
        let kept = keeps.contains(chunk);
        if !kept {
            unloads.push(chunk);
        }
        kept
    });
    unloads
}

/// The current state of the blocks of `changed` in the chunks of `sent`.
fn block_updates(chunks: &ChunkMap, changed: &[IVec3], sent: &HashSet<IVec3>) -> Vec<BlockUpdate> {
    changed
        .iter()
        .filter(|pos| sent.contains(&global_block_to_chunk_pos(pos)))
        .map(|&position| BlockUpdate {
            position,
            block: chunks.get_block(position).copied(),
        })
        .collect()
}

/// Adds to `map` the chunks of `active_chunks`, in their order, that a client has not received
/// yet, at most `most` of varied cells: a chunk of one kind of cell, such as air or stone,
/// weighs a few bytes on the wire, so those among them do not count. They are recorded in
/// `sent`. Says how many varied ones it added.
fn add_chunks_to_send<'a>(
    chunks: &'a ChunkMap,
    active_chunks: impl IntoIterator<Item = IVec3>,
    sent: &mut HashSet<IVec3>,
    most: usize,
    map: &mut HashMap<IVec3, &'a Chunk>,
) -> usize {
    let mut varied = 0;

    for c in active_chunks {
        if varied >= most {
            break;
        }

        // Only chunks that exist and that the client has not received
        let Some(chunk) = chunks.chunk(c) else {
            continue;
        };
        if sent.insert(c) {
            if chunk.uniform().is_none() {
                varied += 1;
            }
            map.insert(c, chunk);
        }
    }

    varied
}

/// The chunks within the `radius` of each player, each listed once, in the order the players
/// reach them.
#[cfg(test)]
pub fn get_all_active_chunks<'a>(
    players: impl IntoIterator<Item = (&'a PlayerId, &'a Player)>,
    radius: impl Fn(PlayerId) -> i32,
) -> Vec<IVec3> {
    active_chunks(players, radius, &Default::default()).collect()
}

/// The chunks within the `radius` of each player, each once, in the order the players reach
/// them, as they are asked for: the hundreds of thousands of a large render distance are not
/// all listed when the first few are enough. Then those around the twins of the open portals
/// within the radius of each player (`behind_portals`).
pub fn active_chunks<'a>(
    players: impl IntoIterator<Item = (&'a PlayerId, &'a Player)>,
    radius: impl Fn(PlayerId) -> i32,
    portals: &Portals,
) -> impl Iterator<Item = IVec3> {
    let reaches: Vec<ChunkReach> = players
        .into_iter()
        .map(|(&id, player)| ChunkReach::of_player(player, radius(id), portals))
        .collect();
    let behind = behind_portals(&reaches);
    active_chunks_of(&reaches, behind)
}

/// The chunks around the twins of the open portals within the reach of each of `reaches`
/// (`ChunkReach::portal_chunks`) that none has within its render distance, each once: up to
/// thousands around an approached twin (`MAX_APPROACHED_EXTRA_CHUNKS`), to be listed once per
/// reach, not on every tick.
pub fn behind_portals(reaches: &[ChunkReach]) -> Arc<[IVec3]> {
    // A single player's are each once, and not within its render distance already
    let mut seen = (reaches.len() > 1).then(HashSet::new);
    reaches
        .iter()
        .flat_map(ChunkReach::portal_chunks)
        .filter(|&chunk| match seen.as_mut() {
            Some(seen) => !reaches.iter().any(|reach| reach.within(chunk, 0)) && seen.insert(chunk),
            None => true,
        })
        .collect()
}

/// `active_chunks`, of the reaches `reaches`, with the chunks `behind_portals` of them.
pub fn active_chunks_of(
    reaches: &[ChunkReach],
    behind_portals: Arc<[IVec3]>,
) -> impl Iterator<Item = IVec3> {
    let behind = (0..behind_portals.len()).map(move |i| behind_portals[i]);
    let around: Vec<(IVec3, std::sync::Arc<[IVec3]>)> = reaches
        .iter()
        .map(|reach| (reach.center, render_distance_offsets(reach.distance)))
        .collect();
    // A single player reaches each chunk once
    let mut seen = (around.len() > 1).then(HashSet::new);
    around
        .into_iter()
        .flat_map(|(chunk, offsets)| (0..offsets.len()).map(move |i| chunk + offsets[i]))
        .filter(move |chunk| seen.as_mut().is_none_or(|seen| seen.insert(*chunk)))
        .chain(behind)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_renet::renet::ClientId;
    use shared::world::{
        chunk_distance, BlockData, BlockDirection, BlockId, FaceDirection, Portal, PortalColor,
        PORTAL_VIEW_RADIUS,
    };

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

    /// The chunks of `active_chunks` a client has not received yet, as `world_messages_around`
    /// adds them: at most `MAX_CHUNKS_PER_BROADCAST` of varied cells (`add_chunks_to_send`).
    fn get_world_map_chunks_to_send<'a>(
        chunks: &'a ChunkMap,
        active_chunks: impl IntoIterator<Item = IVec3>,
        sent: &mut HashSet<IVec3>,
    ) -> HashMap<IVec3, &'a Chunk> {
        let mut map = HashMap::new();
        add_chunks_to_send(
            chunks,
            active_chunks,
            sent,
            MAX_CHUNKS_PER_BROADCAST,
            &mut map,
        );
        map
    }

    /// What one client must be sent, in the order of `world_messages_around`: the updates of
    /// the changed blocks in the chunks it has, computed before the new chunks are recorded in
    /// `sent`, then the chunks it lacks. A chunk sent now already holds the changes, so the
    /// client never misses one and is never sent an update for a chunk it lacks.
    fn client_world_messages<'a>(
        chunks: &'a ChunkMap,
        changed: &[IVec3],
        active_chunks: impl IntoIterator<Item = IVec3>,
        sent: &mut HashSet<IVec3>,
    ) -> (Vec<BlockUpdate>, HashMap<IVec3, &'a Chunk>) {
        let updates = block_updates(chunks, changed, sent);
        let new_chunks = get_world_map_chunks_to_send(chunks, active_chunks, sent);
        (updates, new_chunks)
    }

    #[test]
    fn a_chunk_is_sent_once_to_a_client() {
        let chunks = row_of_chunks(3);
        let mut sent = HashSet::new();

        let first = get_world_map_chunks_to_send(&chunks, active_row(3), &mut sent);
        let second = get_world_map_chunks_to_send(&chunks, active_row(3), &mut sent);

        assert_eq!(first.len(), 3);
        assert!(second.is_empty());
    }

    #[test]
    fn chunks_that_do_not_exist_are_skipped_and_sent_once_they_do() {
        let mut chunks = row_of_chunks(1);
        let mut sent = HashSet::new();

        let first = get_world_map_chunks_to_send(&chunks, active_row(2), &mut sent);
        assert_eq!(first.keys().copied().collect::<Vec<_>>(), vec![IVec3::ZERO]);

        chunks.insert_chunk(IVec3::X, Chunk::new());
        let second = get_world_map_chunks_to_send(&chunks, active_row(2), &mut sent);
        assert_eq!(second.keys().copied().collect::<Vec<_>>(), vec![IVec3::X]);
    }

    /// `row_of_chunks`, with a block in each chunk.
    fn row_of_full_chunks(count: i32) -> ChunkMap {
        let mut chunks = row_of_chunks(count);
        for x in 0..count {
            chunks.set_block(IVec3::new(x * CHUNK_SIZE, 0, 0), stone());
        }
        // As generated: no change logged
        chunks.take_changed_blocks();
        chunks
    }

    #[test]
    fn at_most_the_limit_of_chunks_are_sent_per_call_nearest_first() {
        const MAX: i32 = MAX_CHUNKS_PER_BROADCAST as i32;
        let chunks = row_of_full_chunks(2 * MAX + 5);
        let mut sent = HashSet::new();

        let first = get_world_map_chunks_to_send(&chunks, active_row(2 * MAX + 5), &mut sent);
        let second = get_world_map_chunks_to_send(&chunks, active_row(2 * MAX + 5), &mut sent);

        assert_eq!(first.len(), MAX_CHUNKS_PER_BROADCAST);
        assert!((0..MAX).all(|x| first.contains_key(&IVec3::new(x, 0, 0))));
        assert_eq!(second.len(), MAX_CHUNKS_PER_BROADCAST);
        assert!((MAX..2 * MAX).all(|x| second.contains_key(&IVec3::new(x, 0, 0))));
    }

    #[test]
    fn chunks_of_one_kind_of_cell_do_not_count_towards_the_limit() {
        const MAX: i32 = MAX_CHUNKS_PER_BROADCAST as i32;
        // Chunks of air or of stone only at even x, chunks with a block at odd x
        let mut chunks = row_of_chunks(4 * MAX);
        for x in (1..4 * MAX).step_by(2) {
            chunks.set_block(IVec3::new(x * CHUNK_SIZE, 0, 0), stone());
        }
        for x in (0..4 * MAX).step_by(4) {
            chunks.insert_chunk(IVec3::new(x, 0, 0), Chunk::filled(stone()));
        }
        let mut sent = HashSet::new();

        let first = get_world_map_chunks_to_send(&chunks, active_row(4 * MAX), &mut sent);

        // The limit of chunks with a block, at x = 1 to 2 * MAX - 1, and the others among them
        assert_eq!(
            first.values().filter(|c| c.uniform().is_none()).count(),
            MAX_CHUNKS_PER_BROADCAST
        );
        assert_eq!(xs(first.keys()), (0..2 * MAX).collect::<Vec<_>>());
    }

    /// What a client holds: the chunks and updates it received, applied in order.
    fn deliver(
        client: &mut ChunkMap,
        (updates, new_chunks): (Vec<BlockUpdate>, HashMap<IVec3, &Chunk>),
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
            client.insert_chunk(pos, chunk.clone());
        }
    }

    #[test]
    fn a_changed_block_is_sent_as_an_update_to_every_client_that_has_its_chunk() {
        let mut chunks = row_of_chunks(2);
        let active = active_row(2);
        let mut sent_chunks: HashMap<PlayerId, HashSet<IVec3>> = HashMap::new();
        for client in [1, 2, 3] {
            get_world_map_chunks_to_send(
                &chunks,
                active.iter().copied(),
                sent_chunks.entry(client).or_default(),
            );
        }

        let pos = IVec3::new(CHUNK_SIZE + 4, 2, 2);
        chunks.set_block(pos, stone());
        let changed = chunks.take_changed_blocks();

        for client in [1, 2, 3] {
            let (updates, new_chunks) = client_world_messages(
                &chunks,
                &changed,
                active.iter().copied(),
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
            active.iter().copied(),
            sent_chunks.entry(1).or_default(),
        );
        assert!(updates.is_empty() && new_chunks.is_empty());
    }

    #[test]
    fn a_removed_block_is_sent_as_air() {
        let mut chunks = row_of_chunks(1);
        let active = active_row(1);
        let mut sent = HashSet::new();
        get_world_map_chunks_to_send(&chunks, active.iter().copied(), &mut sent);
        let pos = IVec3::new(1, 2, 3);
        chunks.set_block(pos, stone());
        chunks.take_changed_blocks();
        chunks.remove_block(pos);

        let changed = chunks.take_changed_blocks();
        let (updates, _) =
            client_world_messages(&chunks, &changed, active.iter().copied(), &mut sent);

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
        get_world_map_chunks_to_send(&chunks, active.iter().copied(), &mut sent);

        chunks.touch_block(IVec3::new(5, 5, 5));
        let changed = chunks.take_changed_blocks();
        let (updates, _) =
            client_world_messages(&chunks, &changed, active.iter().copied(), &mut sent);

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
            client_world_messages(&chunks, &changed, active_row(2), &mut sent);

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
        const LAST: i32 = MAX_CHUNKS_PER_BROADCAST as i32 + 1;
        let mut chunks = row_of_full_chunks(LAST + 1);
        let active = active_row(LAST + 1);
        let mut sent = HashSet::new();
        let first = get_world_map_chunks_to_send(&chunks, active.iter().copied(), &mut sent);
        assert!(!first.contains_key(&IVec3::new(LAST, 0, 0)));

        // the last chunk is still waiting for its turn when one of its blocks changes
        let pos = IVec3::new(LAST * CHUNK_SIZE + 1, 2, 3);
        chunks.set_block(pos, stone());
        let changed = chunks.take_changed_blocks();
        let (updates, new_chunks) =
            client_world_messages(&chunks, &changed, active.iter().copied(), &mut sent);

        assert!(updates.is_empty());
        assert_eq!(
            new_chunks[&IVec3::new(LAST, 0, 0)].get(IVec3::new(1, 2, 3)),
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
    fn a_broadcast_marks_the_chunk_of_a_changed_chest_dirty_and_sends_no_update() {
        let mut app = App::new();
        app.insert_resource(RenetServer::new(shared::get_shared_renet_config()))
            .insert_resource(ServerTime(0))
            .insert_resource(ServerWorldMap {
                chunks: row_of_chunks(3),
                ..default()
            });
        let chest = IVec3::new(CHUNK_SIZE + 1, 0, 0);
        {
            let mut map = app.world_mut().resource_mut::<ServerWorldMap>();
            map.chunks
                .set_block(chest, BlockData::new(BlockId::Chest, BlockDirection::Front));
            map.chunks
                .insert_container(chest, shared::world::Container::default());
            map.chunks.take_changed_blocks();
            map.chunks.container_mut(chest);
        }

        app.add_systems(Update, broadcast_world_state);
        app.update();

        let mut map = app.world_mut().resource_mut::<ServerWorldMap>();
        assert_eq!(map.dirty_chunks, HashSet::from([IVec3::X]));
        assert!(map.chunks.take_changed_containers().is_empty());
    }

    #[test]
    fn a_broadcast_marks_the_chunk_of_a_changed_furnace_dirty() {
        let mut app = App::new();
        app.insert_resource(RenetServer::new(shared::get_shared_renet_config()))
            .insert_resource(ServerTime(0))
            .insert_resource(ServerWorldMap {
                chunks: row_of_chunks(3),
                ..default()
            });
        let furnace = IVec3::new(2 * CHUNK_SIZE + 1, 0, 0);
        {
            let mut map = app.world_mut().resource_mut::<ServerWorldMap>();
            map.chunks.set_block(
                furnace,
                BlockData::new(BlockId::Furnace, BlockDirection::Front),
            );
            map.chunks
                .insert_furnace(furnace, shared::world::Furnace::default());
            map.chunks.take_changed_blocks();
            map.chunks.furnace_mut(furnace);
        }

        app.add_systems(Update, broadcast_world_state);
        app.update();

        let mut map = app.world_mut().resource_mut::<ServerWorldMap>();
        assert_eq!(map.dirty_chunks, HashSet::from([IVec3::new(2, 0, 0)]));
        assert!(map.chunks.take_changed_furnaces().is_empty());
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
                    client_world_messages(&chunks, &changed, active_now.iter().copied(), &mut sent),
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
    #[test]
    fn new_chunks_are_split_into_messages_nearest_first() {
        let air = Chunk::new();
        let new_map: HashMap<IVec3, &Chunk> =
            (0..70).map(|x| (IVec3::new(x, 0, 0), &air)).collect();

        let reach = ChunkReach::new(IVec3::ZERO, 70, &Default::default());
        let payloads = encode_world_updates(new_map, &reach, 3, 4);

        let updates: Vec<WorldUpdate> = payloads
            .iter()
            .map(
                |payload| match shared::payload_to_game_message(payload).unwrap() {
                    ServerToClientMessage::WorldUpdate(update) => update,
                    other => panic!("not a world update: {other:?}"),
                },
            )
            .collect();
        let sizes: Vec<usize> = updates.iter().map(|u| u.new_map.len()).collect();
        assert_eq!(
            sizes,
            [
                CHUNKS_PER_MESSAGE,
                CHUNKS_PER_MESSAGE,
                70 - 2 * CHUNKS_PER_MESSAGE
            ]
        );
        assert_eq!(xs(updates[0].new_map.keys()), (0..32).collect::<Vec<_>>());
        assert!(updates.iter().all(|u| u.mobs.is_empty()));
    }

    /// The ids of the mobs `messages` send, and of those they remove.
    fn sent_and_removed(messages: &[ServerToClientMessage]) -> (Vec<MobId>, Vec<MobId>) {
        let (mut sent, mut removed) = (Vec::new(), Vec::new());
        for message in messages {
            match message {
                ServerToClientMessage::MobUpdate(update) => sent.push(update.id),
                ServerToClientMessage::MobRemoved(id) => removed.push(*id),
                other => panic!("not about a mob: {other:?}"),
            }
        }
        sent.sort();
        removed.sort();
        (sent, removed)
    }

    #[test]
    fn a_mob_is_sent_when_it_comes_within_reach_or_changes_and_removed_when_it_leaves() {
        let sheep = |x: f32| {
            shared::world::ServerMob::new(shared::world::MobKind::Sheep, Vec3::new(x, 0.0, 0.0))
        };
        let reach = (RENDER_DISTANCE * CHUNK_SIZE) as f32;
        let mut mobs = HashMap::from([(1, sheep(5.0)), (2, sheep(reach + 5.0))]);
        let mut sent = HashSet::new();
        let messages = |mobs: &HashMap<MobId, ServerMob>, changed: &[MobId], sent: &mut _| {
            let changed = changed.iter().copied().collect();
            let reach = ChunkReach::new(IVec3::ZERO, RENDER_DISTANCE, &Portals::default());
            sent_and_removed(&mob_messages(Vec3::ZERO, &reach, mobs, &changed, sent))
        };

        // the near one is sent, once, then again only when it changes
        assert_eq!(messages(&mobs, &[1, 2], &mut sent), (vec![1], vec![]));
        assert_eq!(messages(&mobs, &[], &mut sent), (vec![], vec![]));
        assert_eq!(messages(&mobs, &[1], &mut sent), (vec![1], vec![]));
        // the far one comes within reach, the near one leaves it
        mobs.insert(1, sheep(reach + 1.0));
        mobs.insert(2, sheep(1.0));
        assert_eq!(messages(&mobs, &[1, 2], &mut sent), (vec![2], vec![1]));
        // a mob gone from the world is removed too
        mobs.remove(&2);
        assert_eq!(messages(&mobs, &[], &mut sent), (vec![], vec![2]));
        assert!(sent.is_empty());
    }

    #[test]
    fn a_mob_by_the_far_twin_of_a_portal_within_reach_is_sent_and_removed_once_it_closes() {
        let mut chunks = ChunkMap::default();
        open_pair(&mut chunks, 1);
        let by_twin = (FAR_TWIN * CHUNK_SIZE).as_vec3() + Vec3::new(4.5, 5.0, 9.0);
        let mobs = HashMap::from([(
            1,
            shared::world::ServerMob::new(shared::world::MobKind::Sheep, by_twin),
        )]);
        let mut sent = HashSet::new();
        let messages = |chunks: &ChunkMap, sent: &mut _| {
            let reach = ChunkReach::new(IVec3::ZERO, RENDER_DISTANCE, chunks.portals());
            sent_and_removed(&mob_messages(
                Vec3::ZERO,
                &reach,
                &mobs,
                &HashSet::new(),
                sent,
            ))
        };
        assert_eq!(messages(&chunks, &mut sent), (vec![1], vec![]));
        assert_eq!(messages(&chunks, &mut sent), (vec![], vec![]));
        chunks.portals_mut().remove(1, PortalColor::Orange);
        assert_eq!(messages(&chunks, &mut sent), (vec![], vec![1]));
    }

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
    fn walk_to<'a>(
        chunks: &'a ChunkMap,
        x: i32,
        sent: &mut SentChunks,
    ) -> (Vec<IVec3>, HashMap<IVec3, &'a Chunk>) {
        let reach = ChunkReach::new(IVec3::new(x, 0, 0), RENDER_DISTANCE, chunks.portals());
        let (unloads, updates, new_chunks) = world_messages_around(chunks, &[], &reach, sent);
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

        let reach = ChunkReach::new(IVec3::ZERO, RENDER_DISTANCE, &Default::default());
        let mut unloads = take_chunks_to_unload(&reach, &mut sent);

        unloads.sort_by_key(|c| (c.x, c.y, c.z));
        assert_eq!(unloads, vec![far[1], far[0]]);
        assert_eq!(sent, HashSet::from(kept));
    }

    #[test]
    fn only_the_render_distance_is_sent_and_the_margin_is_kept_but_never_sent_first() {
        let chunks = chunks_between(-KEPT - 2, KEPT + 2);
        let mut sent = SentChunks::default();

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
        assert!(!sent.chunks.contains(&IVec3::new(-RENDER_DISTANCE, 0, 0)));
    }

    #[test]
    fn walking_to_and_fro_across_a_border_neither_unloads_nor_sends() {
        let chunks = chunks_between(-KEPT - 2, KEPT + 2);
        let mut sent = SentChunks::default();
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
        let mut sent = SentChunks::default();
        walk_to(&chunks, 0, &mut sent);

        let (unloads, _) = walk_to(&chunks, UNLOAD_MARGIN + 1, &mut sent);
        assert_eq!(unloads, vec![west]);

        let (unloads, new_chunks) = walk_to(&chunks, 0, &mut sent);
        assert!(new_chunks.contains_key(&west));
        // the chunks the walk east brought are in the margin again, or beyond it
        assert!(unloads.iter().all(|c| c.x > KEPT));
    }

    #[test]
    fn a_client_holding_its_render_distance_is_only_sent_updates_until_its_player_moves() {
        // Every chunk within 1 of the origin but one, which is generated later
        let missing = IVec3::new(1, 1, 1);
        let mut chunks = ChunkMap::default();
        for x in -2..=2 {
            for y in -2..=2 {
                for z in -2..=2 {
                    if IVec3::new(x, y, z) != missing {
                        chunks.insert_chunk(IVec3::new(x, y, z), Chunk::new());
                    }
                }
            }
        }
        let mut sent = SentChunks::default();
        // The positions of the new chunks
        let mut around = |chunks: &ChunkMap, changed: &[IVec3], player_chunk| {
            let reach = ChunkReach::new(player_chunk, 1, chunks.portals());
            let (unloads, updates, new_chunks) =
                world_messages_around(chunks, changed, &reach, &mut sent);
            (unloads, updates, new_chunks.into_keys().collect::<Vec<_>>())
        };

        assert_eq!(around(&chunks, &[], IVec3::ZERO).2.len(), 26);
        chunks.insert_chunk(missing, Chunk::new());
        let (_, _, new_chunks) = around(&chunks, &[], IVec3::ZERO);
        assert_eq!(new_chunks, vec![missing]);

        // Complete: a change still comes as an update
        let pos = IVec3::new(3, 2, 1);
        chunks.set_block(pos, stone());
        let changed = chunks.take_changed_blocks();
        let (unloads, updates, new_chunks) = around(&chunks, &changed, IVec3::ZERO);
        assert!(unloads.is_empty() && new_chunks.is_empty());
        assert_eq!(updates.len(), 1);

        // Another chunk: the chunks a step further are sent
        let (_, _, new_chunks) = around(&chunks, &[], IVec3::NEG_X);
        assert_eq!(new_chunks.len(), 9);
    }

    #[test]
    fn a_walking_client_holds_what_the_server_thinks_and_no_update_misses_its_chunk() {
        let mut chunks = chunks_between(-KEPT - 4, KEPT + 4);
        let mut sent = SentChunks::default();
        let mut client = ChunkMap::default();

        for (step, x) in [0, 1, 2, 3, 2, 1, 0, -1, -2, -1, 0].into_iter().enumerate() {
            // a block changes in every chunk of the row, those about to be unloaded included
            for chunk_x in -KEPT - 4..=KEPT + 4 {
                let pos = IVec3::new(chunk_x * CHUNK_SIZE + step as i32 % CHUNK_SIZE, 0, 0);
                chunks.set_block(pos, stone());
            }
            let changed = chunks.take_changed_blocks();
            let reach = ChunkReach::new(IVec3::new(x, 0, 0), RENDER_DISTANCE, chunks.portals());
            let (unloads, updates, new_chunks) =
                world_messages_around(&chunks, &changed, &reach, &mut sent);

            for chunk in unloads {
                assert!(client.remove_chunk(chunk).is_some(), "step {step}: {chunk}");
            }
            for update in &updates {
                let chunk = global_block_to_chunk_pos(&update.position);
                assert!(client.is_chunk_loaded(chunk), "step {step}: {chunk}");
            }
            deliver(&mut client, (updates, new_chunks));

            let held: HashSet<IVec3> = client.chunk_positions().collect();
            assert_eq!(held, sent.chunks, "step {step}");
            for chunk in &held {
                assert_eq!(client.chunk(*chunk), chunks.chunk(*chunk), "step {step}");
                assert!(chunk_distance(*chunk, IVec3::new(x, 0, 0)) <= KEPT);
            }
        }
    }

    #[test]
    fn a_lowered_render_distance_unloads_and_a_raised_one_sends_more() {
        let chunks = chunks_between(-6, 6);
        let mut sent = SentChunks::default();
        let around = |distance, sent: &mut SentChunks| {
            let reach = ChunkReach::new(IVec3::ZERO, distance, chunks.portals());
            let (unloads, _, new_chunks) = world_messages_around(&chunks, &[], &reach, sent);
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

    /// Where the portal pair of the tests opens: the blue portal onto the chunk `(x, 0, 0)`, the
    /// orange one onto `FAR_TWIN`.
    fn open_pair(chunks: &mut ChunkMap, x: i32) {
        for (color, chunk) in [
            (PortalColor::Blue, IVec3::new(x, 0, 0)),
            (PortalColor::Orange, FAR_TWIN),
        ] {
            chunks.portals_mut().insert(Portal {
                owner: 1,
                color,
                anchor: chunk * CHUNK_SIZE + IVec3::new(4, 4, 4),
                face: FaceDirection::PlusZ,
                up: FaceDirection::PlusY,
            });
        }
    }

    /// The chunk the orange portal of the tests opens onto, far from the players.
    const FAR_TWIN: IVec3 = IVec3::new(40, 0, 0);

    /// The chunks along x around `FAR_TWIN`, which the blue portal shows.
    fn around_twin() -> Vec<i32> {
        (FAR_TWIN.x - PORTAL_VIEW_RADIUS..=FAR_TWIN.x + PORTAL_VIEW_RADIUS).collect()
    }

    /// The x of the chunks of `chunks` around `FAR_TWIN`, sorted.
    fn far_xs<'a>(chunks: impl IntoIterator<Item = &'a IVec3>) -> Vec<i32> {
        xs(chunks.into_iter().filter(|chunk| chunk.x > KEPT + 1))
    }

    #[test]
    fn a_far_twins_chunks_are_sent_once_its_portal_is_within_reach_and_unloaded_after() {
        let mut chunks = chunks_between(-KEPT - 2, FAR_TWIN.x + PORTAL_VIEW_RADIUS + 2);
        // The blue portal opens onto the chunk at 4, two chunks beyond the render distance
        open_pair(&mut chunks, RENDER_DISTANCE + 2);
        let mut sent = SentChunks::default();

        let (_, new_chunks) = walk_to(&chunks, 0, &mut sent);
        assert!(far_xs(new_chunks.keys()).is_empty());

        // Within the render distance: what it shows is sent, past the player's own chunks
        let (unloads, new_chunks) = walk_to(&chunks, 2, &mut sent);
        assert!(far_xs(&unloads).is_empty());
        assert_eq!(far_xs(new_chunks.keys()), around_twin());

        // Within the margin: kept, nothing sent again
        let (unloads, new_chunks) = walk_to(&chunks, 1, &mut sent);
        assert!(far_xs(&unloads).is_empty() && far_xs(new_chunks.keys()).is_empty());

        // Beyond it: unloaded
        let (unloads, _) = walk_to(&chunks, 0, &mut sent);
        assert_eq!(far_xs(&unloads), around_twin());
        assert!(far_xs(&sent.chunks).is_empty());
    }

    #[test]
    fn a_portal_that_closes_unloads_the_chunks_around_its_twin() {
        let mut chunks = chunks_between(-KEPT - 2, FAR_TWIN.x + PORTAL_VIEW_RADIUS + 2);
        open_pair(&mut chunks, 1);
        let mut sent = SentChunks::default();
        let (_, new_chunks) = walk_to(&chunks, 0, &mut sent);
        assert_eq!(far_xs(new_chunks.keys()), around_twin());

        chunks.portals_mut().remove(1, PortalColor::Orange);
        let (unloads, new_chunks) = walk_to(&chunks, 0, &mut sent);
        assert_eq!(far_xs(&unloads), around_twin());
        assert!(new_chunks.is_empty());
    }

    #[test]
    fn the_chunks_around_a_twin_take_at_most_a_quarter_of_a_broadcast_and_come_after_the_own() {
        // Every chunk within 6 of the origin and within the view radius of the twin, each with a
        // block: far more than a broadcast
        let distance = 6;
        let mut chunks = ChunkMap::default();
        let cube = |center: IVec3, radius: i32| {
            (-radius..=radius).flat_map(move |x| {
                (-radius..=radius).flat_map(move |y| {
                    (-radius..=radius).map(move |z| center + IVec3::new(x, y, z))
                })
            })
        };
        for chunk in cube(IVec3::ZERO, distance).chain(cube(FAR_TWIN, PORTAL_VIEW_RADIUS)) {
            let mut filled = Chunk::new();
            filled.set(IVec3::ZERO, stone());
            chunks.insert_chunk(chunk, filled);
        }
        open_pair(&mut chunks, 1);
        let reach = ChunkReach::new(IVec3::ZERO, distance, chunks.portals());
        let mut sent = SentChunks::default();

        let (_, _, new_chunks) = world_messages_around(&chunks, &[], &reach, &mut sent);
        let far = new_chunks.keys().filter(|c| c.x > distance).count();
        assert_eq!(far, MAX_CHUNKS_PER_BROADCAST / 4);
        assert_eq!(new_chunks.len(), MAX_CHUNKS_PER_BROADCAST);

        // On the wire, the player's own chunks first, nearest to it first
        let payloads = encode_world_updates(new_chunks, &reach, 0, 0);
        let order: Vec<IVec3> = payloads
            .iter()
            .flat_map(
                |payload| match shared::payload_to_game_message(payload).unwrap() {
                    ServerToClientMessage::WorldUpdate(update) => {
                        let mut chunks: Vec<IVec3> = update.new_map.into_keys().collect();
                        // Within a message, the order is lost: sorted again the same way
                        chunks
                            .sort_by_key(|c| (c.x > distance, (*c - IVec3::ZERO).length_squared()));
                        chunks
                    }
                    other => panic!("not a world update: {other:?}"),
                },
            )
            .collect();
        let first_far = order.iter().position(|c| c.x > distance).unwrap();
        assert!(order[first_far..].iter().all(|c| c.x > distance));
        assert_eq!(order[0], IVec3::ZERO);

        // Until the view is all sent, every broadcast carries a quarter of it
        let mut far_total = far;
        while far_total < 9 * 9 * 9 {
            let (_, _, new_chunks) = world_messages_around(&chunks, &[], &reach, &mut sent);
            let far = new_chunks.keys().filter(|c| c.x > distance).count();
            assert!(far > 0 && far <= MAX_CHUNKS_PER_BROADCAST / 4);
            far_total += far;
        }
        assert_eq!(far_total, 9 * 9 * 9);
    }

    /// Chunks with a block each, those within `radius` of each of `centers`.
    fn varied_cubes(centers: &[(IVec3, i32)]) -> ChunkMap {
        let mut chunks = ChunkMap::default();
        for &(center, radius) in centers {
            for chunk in shared::world::render_distance_offsets(radius)
                .iter()
                .map(|offset| center + *offset)
            {
                let mut filled = Chunk::new();
                filled.set(IVec3::ZERO, stone());
                chunks.insert_chunk(chunk, filled);
            }
        }
        chunks
    }

    #[test]
    fn an_approached_twins_render_distance_comes_nearest_first_within_a_quarter_of_each_broadcast()
    {
        use shared::world::render_distance_offsets;
        let distance = 6;
        let mut chunks = varied_cubes(&[(IVec3::ZERO, distance), (FAR_TWIN, distance)]);
        // The blue portal right by the player
        open_pair(&mut chunks, 1);
        let reach = ChunkReach::new(IVec3::ZERO, distance, chunks.portals());
        let order = reach.portal_chunks();
        let area = render_distance_offsets(distance).len();
        assert_eq!(order.len(), area);
        let mut sent = SentChunks::default();

        let (_, _, new_chunks) = world_messages_around(&chunks, &[], &reach, &mut sent);
        let far: HashSet<IVec3> = new_chunks
            .keys()
            .filter(|c| c.x > distance)
            .copied()
            .collect();
        // The player's own are not starved, and the nearest to the twin come first
        assert_eq!(far.len(), PORTAL_CHUNKS_PER_BROADCAST);
        assert_eq!(new_chunks.len(), MAX_CHUNKS_PER_BROADCAST);
        assert_eq!(
            far,
            order[..PORTAL_CHUNKS_PER_BROADCAST]
                .iter()
                .copied()
                .collect()
        );

        let mut far_total = far.len();
        let mut broadcasts = 1;
        while far_total < area {
            let (_, _, new_chunks) = world_messages_around(&chunks, &[], &reach, &mut sent);
            let far = new_chunks.keys().filter(|c| c.x > distance).count();
            assert!(far > 0 && far <= PORTAL_CHUNKS_PER_BROADCAST);
            far_total += far;
            broadcasts += 1;
        }
        assert_eq!(far_total, area);
        assert_eq!(broadcasts, area.div_ceil(PORTAL_CHUNKS_PER_BROADCAST));
        // Nothing more, and the list is no longer looked at
        let (_, _, new_chunks) = world_messages_around(&chunks, &[], &reach, &mut sent);
        assert!(new_chunks.is_empty());
        let (_, list, held) = sent.portal_progress.as_ref().unwrap();
        assert_eq!(*held, list.len());
    }

    #[test]
    fn a_twin_beyond_the_approach_has_only_its_view_radius_sent() {
        let distance = 6;
        let chunks = {
            let mut chunks = varied_cubes(&[(IVec3::ZERO, distance), (FAR_TWIN, distance)]);
            open_pair(&mut chunks, shared::world::PORTAL_APPROACH_RADIUS + 1);
            chunks
        };
        let reach = ChunkReach::new(IVec3::ZERO, distance, chunks.portals());
        let mut sent = SentChunks::default();
        for _ in 0..20 {
            world_messages_around(&chunks, &[], &reach, &mut sent);
        }
        let far = sent.chunks.iter().filter(|c| c.x > distance).count();
        assert_eq!(far, 9 * 9 * 9);
    }

    #[test]
    fn a_player_in_a_flying_sprint_has_the_render_distance_of_a_twin_sent_from_farther() {
        let distance = 6;
        let chunks = {
            let mut chunks = varied_cubes(&[(IVec3::ZERO, distance), (FAR_TWIN, distance)]);
            open_pair(&mut chunks, 5);
            chunks
        };
        let mut player = Player::new(1, "PlayerOne".into(), Vec3::splat(8.0), default());
        let far_sent = |player: &Player| {
            let reach = ChunkReach::of_player(player, distance, chunks.portals());
            let mut sent = SentChunks::default();
            for _ in 0..40 {
                world_messages_around(&chunks, &[], &reach, &mut sent);
            }
            sent.chunks.iter().filter(|c| c.x > distance).count()
        };
        // On foot, 5 chunks from the portal: the view radius
        assert_eq!(far_sent(&player), 9 * 9 * 9);
        // In a flying sprint, the whole render distance
        player.is_flying = true;
        player.is_sprinting = true;
        assert_eq!(far_sent(&player), render_distance_offsets(distance).len());
    }

    #[test]
    fn releasing_a_sprint_or_landing_by_a_portal_unloads_nothing_of_its_twin() {
        let distance = 6;
        let chunks = {
            let mut chunks = varied_cubes(&[(IVec3::ZERO, distance), (FAR_TWIN, distance)]);
            open_pair(&mut chunks, 5);
            chunks
        };
        let mut player = Player::new(1, "PlayerOne".into(), Vec3::splat(8.0), default());
        player.is_flying = true;
        player.is_sprinting = true;
        let mut sent = SentChunks::default();
        let far_unloaded = |player: &Player, sent: &mut SentChunks| {
            let reach = ChunkReach::of_player(player, distance, chunks.portals());
            let mut unloaded = 0;
            for _ in 0..40 {
                let (unloads, _, _) = world_messages_around(&chunks, &[], &reach, sent);
                unloaded += unloads.iter().filter(|c| c.x > distance).count();
            }
            (
                unloaded,
                sent.chunks.iter().filter(|c| c.x > distance).count(),
            )
        };
        let area = render_distance_offsets(distance).len();
        assert_eq!(far_unloaded(&player, &mut sent), (0, area));
        // Sprint released, then landed: the portal is no longer approached, nothing goes
        player.is_sprinting = false;
        assert_eq!(far_unloaded(&player, &mut sent), (0, area));
        player.is_flying = false;
        assert_eq!(far_unloaded(&player, &mut sent), (0, area));
    }

    #[test]
    fn the_render_distance_of_a_twin_unloads_once_left_past_the_margin_but_its_view_radius() {
        use shared::world::{MAX_PORTAL_APPROACH_RADIUS, PORTAL_VIEW_RADIUS};
        // Past the widest approach and the margin, the portal still within the render distance
        let distance = MAX_PORTAL_APPROACH_RADIUS + 2;
        let mut chunks = ChunkMap::default();
        for chunk in render_distance_offsets(distance + 2)
            .iter()
            .flat_map(|offset| [*offset, FAR_TWIN + *offset])
        {
            chunks.insert_chunk(chunk, Chunk::new());
        }
        open_pair(&mut chunks, 0);
        let mut sent = SentChunks::default();
        let walk = |x: i32, sent: &mut SentChunks| {
            let reach = ChunkReach::new(IVec3::new(x, 0, 0), distance, chunks.portals());
            let (unloads, _, _) = world_messages_around(&chunks, &[], &reach, sent);
            let far = |c: &&IVec3| c.x > 20;
            (
                unloads.iter().filter(far).count(),
                sent.chunks.iter().filter(far).count(),
            )
        };
        let area = render_distance_offsets(distance).len();
        let view = render_distance_offsets(PORTAL_VIEW_RADIUS).len();
        assert_eq!(walk(0, &mut sent), (0, area));
        // Within the widest approach and the margin: nothing goes, whatever the player's speed
        assert_eq!(
            walk(-MAX_PORTAL_APPROACH_RADIUS - UNLOAD_MARGIN, &mut sent),
            (0, area)
        );
        // Past them: all but the view radius and the margin
        let around = PORTAL_VIEW_RADIUS + UNLOAD_MARGIN;
        let kept =
            (2 * around + 1) as usize * (2 * around + 1) as usize * (2 * around + 1) as usize;
        let (unloaded, held) = walk(-MAX_PORTAL_APPROACH_RADIUS - UNLOAD_MARGIN - 1, &mut sent);
        assert_eq!((unloaded, held), (area - kept, kept));
        assert!(kept > view);
        // Back by the portal: the area comes again
        walk(0, &mut sent);
        assert_eq!(walk(0, &mut sent), (0, area));
    }

    #[test]
    fn a_client_is_sent_the_chunks_around_the_twin_of_a_portal_in_reach() {
        let mut chunks = chunks_between(-KEPT, FAR_TWIN.x + PORTAL_VIEW_RADIUS + 2);
        open_pair(&mut chunks, 1);
        let mut players = HashMap::new();
        let mut render_distances = crate::world::data::RenderDistances::default();
        render_distances.set(1, RENDER_DISTANCE as u32, 6);
        players.insert(
            1,
            Player::new(1, "p".into(), Vec3::splat(0.5), Transform::default()),
        );
        let mut server = RenetServer::new(shared::get_shared_renet_config());
        server.add_connection(1);
        let mut app = App::new();
        app.insert_resource(server)
            .insert_resource(ServerTime(0))
            .insert_resource(ServerWorldMap {
                chunks,
                players,
                render_distances,
                ..default()
            });

        app.add_systems(Update, broadcast_world_state);
        app.update();

        let mut server = app.world_mut().resource_mut::<RenetServer>();
        let chunks: Vec<IVec3> = received_world_messages(&mut server, 1)
            .into_iter()
            .flat_map(|message| match message {
                ServerToClientMessage::WorldUpdate(update) => {
                    update.new_map.into_keys().collect::<Vec<_>>()
                }
                other => panic!("unexpected {other:?}"),
            })
            .collect();
        let mut expected: Vec<i32> = (-RENDER_DISTANCE..=RENDER_DISTANCE).collect();
        expected.extend(around_twin());
        assert_eq!(xs(&chunks), expected);
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
