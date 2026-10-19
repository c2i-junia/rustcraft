use bevy::prelude::*;
use bevy_renet::{renet::ClientId, RenetServer};
use shared::{
    messages::{PlayerFrameInput, PlayerId, PlayerUpdateEvent},
    players::{blocks::CallerType, simulation::simulate_player_actions, Player},
    world::{Chunk, ChunkMap, WorldSeed},
};
use std::path::Path;

use crate::{
    network::extensions::SendGameMessageExtension,
    world::{
        chunk_files::{read_chunk, CHUNKS_DIR},
        container_files::{
            insert_with_chests, read_chunk_containers, ChunkContainers, CONTAINERS_DIR,
        },
        data::ServerWorldMap,
        generation::generate_chunk,
    },
};
use bevy::tasks::{ComputeTaskPool, TaskPool};

use super::broadcast_world::get_all_active_chunks;

#[derive(Message, Debug)]
pub struct PlayerInputsEvent {
    pub client_id: ClientId,
    pub input: PlayerFrameInput,
}

/// How many chunks the render distances of the players get loaded or generated per tick, at
/// most: a joining player would otherwise freeze the server for its whole render distance at
/// once.
pub(super) const GENERATED_CHUNKS_PER_TICK: usize = 1024;

/// Chunks made on one task of the pool: fewer are made on the tick's thread.
const CHUNKS_PER_TASK: usize = 16;

/// A chunk that was not loaded, read from its file or generated.
enum Made {
    Read(Chunk, ChunkContainers),
    Generated(Chunk),
}

/// The chunk at `chunk_pos`: from its file in the folder of the world `saved`, else generated.
fn make_chunk(chunk_pos: IVec3, saved: Option<&Path>, seed: u32) -> Made {
    if let Some(dir) = saved {
        if let Some(chunk) = read_chunk(&dir.join(CHUNKS_DIR), chunk_pos) {
            let chests = read_chunk_containers(&dir.join(CONTAINERS_DIR), chunk_pos);
            return Made::Read(chunk, chests);
        }
    }
    Made::Generated(generate_chunk(chunk_pos, seed))
}

/// Makes `chunks` on the threads of the compute pool, in their order.
fn make_chunks(chunks: &[IVec3], saved: Option<&Path>, seed: u32) -> Vec<Made> {
    if chunks.len() <= CHUNKS_PER_TASK {
        return chunks.iter().map(|&c| make_chunk(c, saved, seed)).collect();
    }
    ComputeTaskPool::get_or_init(TaskPool::default)
        .scope(|scope| {
            for batch in chunks.chunks(CHUNKS_PER_TASK) {
                scope.spawn(async move {
                    batch
                        .iter()
                        .map(|&c| make_chunk(c, saved, seed))
                        .collect::<Vec<_>>()
                });
            }
        })
        .into_iter()
        .flatten()
        .collect()
}

/// Loads the chunks within `radius` of each of `players`, at most `budget` of them, the nearest
/// to each player first: from their files in the folder of the world `saved`, else generated.
/// A generated chunk is dirty: it has to be saved, as inserting it is not logged as a block
/// change. A chunk read from its file is not.
pub(super) fn generate_missing_chunks<'a>(
    players: impl IntoIterator<Item = (&'a PlayerId, &'a Player)>,
    radius: impl Fn(PlayerId) -> i32,
    chunks: &mut ChunkMap,
    dirty_chunks: &mut std::collections::HashSet<IVec3>,
    saved: Option<&Path>,
    seed: u32,
    budget: usize,
) {
    let missing: Vec<IVec3> = get_all_active_chunks(players, radius)
        .into_iter()
        .filter(|c| !chunks.is_chunk_loaded(*c))
        .take(budget)
        .collect();
    if missing.is_empty() {
        return;
    }
    debug!("Loading {} chunks", missing.len());
    for (c, made) in missing.iter().zip(make_chunks(&missing, saved, seed)) {
        match made {
            Made::Read(chunk, chests) => insert_with_chests(chunks, *c, chunk, chests),
            Made::Generated(chunk) => {
                chunks.insert_chunk(*c, chunk);
                dirty_chunks.insert(*c);
            }
        }
    }
}

pub fn handle_player_inputs_system(
    mut events: MessageReader<PlayerInputsEvent>,
    mut world_map: ResMut<ServerWorldMap>,
    seed: Res<WorldSeed>,
) {
    let mut rng = rand::thread_rng();
    let world_map = world_map.as_mut();
    let players = &mut world_map.players;
    let chunks = &mut world_map.chunks;
    let dropped_items = &mut world_map.dropped_items;
    let dirty_chunks = &mut world_map.dirty_chunks;
    let render_distances = &world_map.render_distances;
    let saved = world_map.saved.as_deref();

    generate_missing_chunks(
        &*players,
        |id| render_distances.get(id),
        chunks,
        dirty_chunks,
        saved,
        seed.0,
        GENERATED_CHUNKS_PER_TICK,
    );

    for ev in events.read() {
        let Some(player) = players.get_mut(&ev.client_id) else {
            warn!("Ignoring inputs from unknown player {}", ev.client_id);
            continue;
        };

        let broken = simulate_player_actions(player, chunks, &ev.input, CallerType::Server);
        for block in &broken {
            dropped_items.drop_block_items(block, &mut rng);
        }

        player.last_input_processed = ev.input.time_ms;

        // The simulation holds a player still while its chunk or the one below is missing, and
        // a falling player can cross a chunk within one batch of inputs. The chunks next to
        // the player's are enough: the rest of the render distance waits for the next tick.
        // The other players did not move
        generate_missing_chunks(
            [(&ev.client_id, &*player)],
            |_| 1,
            chunks,
            dirty_chunks,
            saved,
            seed.0,
            usize::MAX,
        );
    }
}

/// Sends every player's state to every client, once the tick has simulated the inputs and
/// given the players the items they picked up. A player is sent its own inventory and open
/// chest; the others only see it move.
pub fn broadcast_player_updates_system(
    world_map: Res<ServerWorldMap>,
    mut server: ResMut<RenetServer>,
) {
    for player in world_map.players.values() {
        let update = |inventory, open_chest| {
            shared::messages::ServerToClientMessage::PlayerUpdate(PlayerUpdateEvent {
                id: player.id,
                movement: player.movement_state(),
                orientation: player.camera_transform.rotation,
                last_ack_time: player.last_input_processed,
                inventory,
                open_chest,
            })
        };
        let open_chest = player
            .inventory
            .open_chest
            .and_then(|chest| world_map.chunks.container(chest).cloned().map(Box::new));
        server.broadcast_game_message_except(player.id, update(None, None));
        server.send_game_message(
            player.id,
            update(Some(player.inventory.clone()), open_chest),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_chunks_are_dirty_and_existing_ones_are_not() {
        let mut players = std::collections::HashMap::new();
        players.insert(
            1,
            Player::new(1, "p".into(), Vec3::new(8., 8., 8.), Transform::default()),
        );
        let mut chunks = ChunkMap::default();
        chunks.insert_chunk(IVec3::ZERO, shared::world::Chunk::new());
        let mut dirty = std::collections::HashSet::new();

        generate_missing_chunks(
            &players,
            |_| 2,
            &mut chunks,
            &mut dirty,
            None,
            1,
            usize::MAX,
        );

        assert!(!dirty.contains(&IVec3::ZERO));
        assert!(dirty.contains(&IVec3::X));
        assert_eq!(
            dirty.len() + 1,
            get_all_active_chunks(&players, |_| 2).len()
        );
        assert!(dirty.iter().all(|&c| chunks.is_chunk_loaded(c)));
    }

    #[test]
    fn a_tick_generates_its_budget_of_chunks_nearest_first() {
        let mut players = std::collections::HashMap::new();
        players.insert(
            1,
            Player::new(1, "p".into(), Vec3::new(8., 8., 8.), Transform::default()),
        );
        let mut chunks = ChunkMap::default();
        let mut dirty = std::collections::HashSet::new();

        generate_missing_chunks(&players, |_| 2, &mut chunks, &mut dirty, None, 1, 7);
        // The chunk of the player and its six face neighbours
        assert_eq!(dirty.len(), 7);
        assert!(dirty.iter().all(|c| c.abs().element_sum() <= 1));

        generate_missing_chunks(&players, |_| 2, &mut chunks, &mut dirty, None, 1, 7);
        assert_eq!(
            dirty.len(),
            14,
            "the next tick goes on where the last stopped"
        );
        assert!(dirty.iter().all(|c| c.abs().max_element() <= 1));
    }

    #[test]
    fn a_saved_chunk_is_read_from_its_file_with_its_chests_and_is_not_dirty() {
        use crate::world::chunk_files::write_chunk;
        use crate::world::container_files::write_containers;
        use shared::world::{BlockData, BlockDirection, BlockId, Container};

        let dir = tempfile::tempdir().unwrap();
        let mut saved = Chunk::new();
        saved.set(
            IVec3::new(1, 2, 3),
            BlockData::new(BlockId::Chest, BlockDirection::Front),
        );
        let chunks_dir = dir.path().join(CHUNKS_DIR);
        let containers_dir = dir.path().join(CONTAINERS_DIR);
        std::fs::create_dir_all(&chunks_dir).unwrap();
        std::fs::create_dir_all(&containers_dir).unwrap();
        write_chunk(&chunks_dir, IVec3::ZERO, &saved).unwrap();
        let mut held = Container::default();
        held.slots[3] = Some(shared::world::ItemStack {
            item_id: shared::world::ItemId::Dirt,
            item_type: shared::world::ItemId::Dirt.get_default_type(),
            nb: 5,
        });
        write_containers(
            &containers_dir,
            IVec3::ZERO,
            &vec![(IVec3::new(1, 2, 3), held.clone())],
        )
        .unwrap();

        let mut players = std::collections::HashMap::new();
        players.insert(
            1,
            Player::new(1, "p".into(), Vec3::new(8., 8., 8.), Transform::default()),
        );
        let mut chunks = ChunkMap::default();
        let mut dirty = std::collections::HashSet::new();
        generate_missing_chunks(
            &players,
            |_| 1,
            &mut chunks,
            &mut dirty,
            Some(dir.path()),
            1,
            usize::MAX,
        );

        assert_eq!(chunks.chunk(IVec3::ZERO), Some(&saved));
        assert_eq!(chunks.container(IVec3::new(1, 2, 3)), Some(&held));
        assert!(
            !dirty.contains(&IVec3::ZERO),
            "a chunk read is saved already"
        );
        assert_eq!(dirty.len(), 26, "the chunks without a file are generated");
    }
}
