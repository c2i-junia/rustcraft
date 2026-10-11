use bevy::{
    platform::collections::{HashMap, HashSet},
    prelude::*,
};
use bevy_renet::{renet::ClientId, RenetServer};
use shared::{
    messages::{NetworkAction, PlayerFrameInput, PlayerId, PlayerUpdateEvent},
    players::{blocks::CallerType, simulation::simulate_player_actions, Player},
    world::{ServerChunkWorldMap, ServerWorldMap, WorldSeed},
};

use crate::{network::extensions::SendGameMessageExtension, world::generation::generate_chunk};

use super::broadcast_world::get_all_active_chunks;

#[derive(Message, Debug)]
pub struct PlayerInputsEvent {
    pub client_id: ClientId,
    pub input: PlayerFrameInput,
}

/// Generates the chunks within one chunk of every player.
fn generate_missing_chunks(
    players: &std::collections::HashMap<PlayerId, Player>,
    chunks: &mut ServerChunkWorldMap,
    seed: u32,
) {
    for c in get_all_active_chunks(players, 1) {
        chunks.map.entry(c).or_insert_with(|| {
            info!("Generated chunk: {:?}", c);
            generate_chunk(c, seed)
        });
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

    generate_missing_chunks(players, chunks, seed.0);

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
        generate_missing_chunks(players, chunks, seed.0);
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
