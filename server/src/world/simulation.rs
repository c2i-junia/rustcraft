use bevy::{
    prelude::*,
    utils::{hashbrown::HashMap, HashSet},
};
use bevy_renet::renet::{ClientId, RenetServer};
use shared::{
    messages::{NetworkAction, PlayerFrameInput, PlayerUpdateEvent},
    players::movement::simulate_player_movement,
    world::{ServerWorldMap, WorldSeed},
};

use crate::{network::extensions::SendGameMessageExtension, world::generation::generate_chunk};

use super::broadcast_world::get_all_active_chunks;

#[derive(Event, Debug)]
pub struct PlayerInputsEvent {
    pub client_id: ClientId,
    pub input: PlayerFrameInput,
}

pub fn handle_player_inputs_system(
    mut events: EventReader<PlayerInputsEvent>,
    mut world_map: ResMut<ServerWorldMap>,
    mut server: ResMut<RenetServer>,
    seed: Res<WorldSeed>,
) {
    let world_map = world_map.as_mut();
    let players = &mut world_map.players;
    let chunks = &mut world_map.chunks;

    let active_chunks = get_all_active_chunks(players, 1);
    for c in active_chunks {
        let chunk = chunks.map.get(&c);

        if chunk.is_none() {
            let chunk = generate_chunk(c, seed.0);
            info!("Generated chunk: {:?}", c);
            chunks.map.insert(c, chunk);
        }
    }

    let mut player_actions = HashMap::<u64, HashSet<NetworkAction>>::new();
    for client_id in players.keys() {
        player_actions.insert(*client_id, HashSet::new());
    }

    for ev in events.read() {
        // info!(
        //     "Processing player inputs for client_id: {} at t={}",
        //     ev.client_id, ev.input.time_ms
        // );
        let player = players.get_mut(&ev.client_id).unwrap();
        // info!(
        //     "Received player inputs: {:?} at t={}",
        //     ev.input.inputs, ev.input.time_ms
        // );

        // let initial = player.position;

        simulate_player_movement(player, chunks, &ev.input.clone());

        // let end = player.position;
        // if initial != end {
        //     info!(
        //         "Player moved: {:?} -> {:?} | {:?}",
        //         initial, end, ev.input.position
        //     );
        // }

        player.last_input_processed = ev.input.time_ms;
    }

    for player in players.values() {
        server.broadcast_game_message(shared::messages::ServerToClientMessage::PlayerUpdate(
            PlayerUpdateEvent {
                id: player.id,
                position: player.position,
                orientation: player.camera_transform.rotation,
                last_ack_time: player.last_input_processed,
            },
        ));
        // info!("Server last ack time: {:?}", player.last_input_processed);
    }
}
