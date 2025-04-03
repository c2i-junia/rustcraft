use shared::{messages::PlayerId, world::ServerWorldMap};

pub fn cleanup_all_players_from_world(world_map: &mut ServerWorldMap) {
    for p in world_map.players.values_mut() {
        p.last_input_processed = 0;
    }
    for (_, chunk) in world_map.chunks.map.iter_mut() {
        chunk.sent_to_clients.clear();
    }
}

pub fn cleanup_player_from_world(world_map: &mut ServerWorldMap, player_id: &PlayerId) {
    world_map.players.remove(player_id);
    for (_, chunk) in world_map.chunks.map.iter_mut() {
        chunk.sent_to_clients.retain(|&id| id != *player_id);
    }
}
