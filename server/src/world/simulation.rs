use bevy::{
    platform::collections::{HashMap, HashSet},
    prelude::*,
};
use bevy_renet::{renet::ClientId, RenetServer};
use shared::{
    messages::{NetworkAction, PlayerFrameInput, PlayerId, PlayerUpdateEvent},
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

/// Generates the chunks within one chunk of every player. A generated chunk is dirty: it has
/// to be saved, as inserting it is not logged as a block change.
fn generate_missing_chunks(
    players: &std::collections::HashMap<PlayerId, Player>,
    chunks: &mut ChunkMap,
    dirty_chunks: &mut std::collections::HashSet<IVec3>,
    seed: u32,
) {
    for c in get_all_active_chunks(players, 1) {
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
    mut server: ResMut<RenetServer>,
    seed: Res<WorldSeed>,
) {
    let world_map = world_map.as_mut();
    let players = &mut world_map.players;
    let chunks = &mut world_map.chunks;
    let dirty_chunks = &mut world_map.dirty_chunks;

    generate_missing_chunks(players, chunks, dirty_chunks, seed.0);

    let mut player_actions = HashMap::<u64, HashSet<NetworkAction>>::new();
    for client_id in players.keys() {
        player_actions.insert(*client_id, HashSet::new());
    }

    for ev in events.read() {
        let Some(player) = players.get_mut(&ev.client_id) else {
            warn!("Ignoring inputs from unknown player {}", ev.client_id);
            continue;
        };

        simulate_player_actions(player, chunks, &ev.input.clone(), CallerType::Server);

        player.last_input_processed = ev.input.time_ms;

        // The simulation holds a player still while its chunk or the one below is missing, and
        // a falling player can cross a chunk within one batch of inputs
        generate_missing_chunks(players, chunks, dirty_chunks, seed.0);
    }

    for player in players.values() {
        server.broadcast_game_message(shared::messages::ServerToClientMessage::PlayerUpdate(
            PlayerUpdateEvent {
                id: player.id,
                position: player.position,
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

        generate_missing_chunks(&players, &mut chunks, &mut dirty, 1);

        assert!(!dirty.contains(&IVec3::ZERO));
        assert!(dirty.contains(&IVec3::X));
        assert_eq!(dirty.len() + 1, get_all_active_chunks(&players, 1).len());
        assert!(dirty.iter().all(|&c| chunks.is_chunk_loaded(c)));
    }
}
