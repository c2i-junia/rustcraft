use bevy::prelude::*;
use bevy_renet::{renet::ClientId, RenetServer};
use shared::{
    messages::{PlayerFrameInput, PlayerId, PlayerUpdateEvent},
    players::{blocks::CallerType, simulation::simulate_player_actions, Player},
    world::{ChunkMap, WorldSeed},
};

use crate::{
    network::extensions::SendGameMessageExtension,
    world::{data::ServerWorldMap, generation::generate_chunk},
};

use super::broadcast_world::get_all_active_chunks;

#[derive(Message, Debug)]
pub struct PlayerInputsEvent {
    pub client_id: ClientId,
    pub input: PlayerFrameInput,
}

/// Generates the chunks within `radius` of every player. A generated chunk is dirty: it has to
/// be saved, as inserting it is not logged as a block change.
fn generate_missing_chunks(
    players: &std::collections::HashMap<PlayerId, Player>,
    radius: impl Fn(PlayerId) -> i32,
    chunks: &mut ChunkMap,
    dirty_chunks: &mut std::collections::HashSet<IVec3>,
    seed: u32,
) {
    for c in get_all_active_chunks(players, radius) {
        if !chunks.is_chunk_loaded(c) {
            info!("Generated chunk: {:?}", c);
            chunks.insert_chunk(c, generate_chunk(c, seed));
            dirty_chunks.insert(c);
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

    generate_missing_chunks(
        players,
        |id| render_distances.get(id),
        chunks,
        dirty_chunks,
        seed.0,
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
        // the player's are enough: the rest of the render distance waits for the next tick
        generate_missing_chunks(players, |_| 1, chunks, dirty_chunks, seed.0);
    }
}

/// Sends every player's state to every client, once the tick has simulated the inputs and
/// given the players the items they picked up.
pub fn broadcast_player_updates_system(
    world_map: Res<ServerWorldMap>,
    mut server: ResMut<RenetServer>,
) {
    for player in world_map.players.values() {
        server.broadcast_game_message(shared::messages::ServerToClientMessage::PlayerUpdate(
            PlayerUpdateEvent {
                id: player.id,
                movement: player.movement_state(),
                orientation: player.camera_transform.rotation,
                last_ack_time: player.last_input_processed,
                inventory: player.inventory.clone(),
            },
        ));
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

        generate_missing_chunks(&players, |_| 2, &mut chunks, &mut dirty, 1);

        assert!(!dirty.contains(&IVec3::ZERO));
        assert!(dirty.contains(&IVec3::X));
        assert_eq!(
            dirty.len() + 1,
            get_all_active_chunks(&players, |_| 2).len()
        );
        assert!(dirty.iter().all(|&c| chunks.is_chunk_loaded(c)));
    }
}
