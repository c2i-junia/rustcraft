use bevy_ecs::event::EventWriter;
use shared::{messages::PlayerId, world::ServerWorldMap};

use crate::world::save::SaveRequestEvent;

pub fn cleanup_all_players_from_world(world_map: &mut ServerWorldMap) {
    for p in world_map.players.values_mut() {
        p.last_input_processed = 0;
    }
    for (_, chunk) in world_map.chunks.map.iter_mut() {
        chunk.sent_to_clients.clear();
    }
}

pub fn cleanup_player_from_world(
    world_map: &mut ServerWorldMap,
    player_id: &PlayerId,
    save_event_writer: &mut EventWriter<SaveRequestEvent>,
) {
    if world_map.players.remove(player_id).is_some() {
        save_event_writer.write(SaveRequestEvent::Player(*player_id));
    }

    for (_, chunk) in world_map.chunks.map.iter_mut() {
        chunk.sent_to_clients.retain(|&id| id != *player_id);
    }
}
