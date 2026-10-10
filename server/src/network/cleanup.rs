use bevy_ecs::event::EventWriter;
use shared::{messages::PlayerId, world::ServerWorldMap, GameFolderPaths};

use crate::world::save::{save_player, SaveRequestEvent};

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
    game_folder_paths: &GameFolderPaths,
    save_event_writer: &mut EventWriter<SaveRequestEvent>,
) {
    // The player must be written before it is dropped from the map
    if let Some(player) = world_map.players.remove(player_id) {
        save_player(&world_map.name, &player, game_folder_paths);
        save_event_writer.write(SaveRequestEvent::World);
    }

    for (_, chunk) in world_map.chunks.map.iter_mut() {
        chunk.sent_to_clients.retain(|&id| id != *player_id);
    }
}
