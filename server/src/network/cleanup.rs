use bevy_ecs::message::MessageWriter;
use shared::{messages::PlayerId, GameFolderPaths};

use crate::world::{
    data::ServerWorldMap,
    save::{save_player, SaveRequestEvent},
};

pub fn cleanup_all_players_from_world(world_map: &mut ServerWorldMap) {
    for p in world_map.players.values_mut() {
        p.last_input_processed = 0;
    }
    world_map.sent_chunks.clear();
}

pub fn cleanup_player_from_world(
    world_map: &mut ServerWorldMap,
    player_id: &PlayerId,
    game_folder_paths: &GameFolderPaths,
    save_event_writer: &mut MessageWriter<SaveRequestEvent>,
) {
    // The player must be written before it is dropped from the map
    if let Some(player) = world_map.players.remove(player_id) {
        save_player(&world_map.name, &player, game_folder_paths);
        save_event_writer.write(SaveRequestEvent::World);
    }

    world_map.sent_chunks.remove(player_id);
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::math::IVec3;
    use std::collections::HashSet;

    fn world_with_two_clients() -> ServerWorldMap {
        let mut world_map = ServerWorldMap::default();
        for id in [1, 2] {
            world_map
                .sent_chunks
                .insert(id, HashSet::from([IVec3::ZERO, IVec3::X]));
        }
        world_map
    }

    #[test]
    fn cleaning_up_every_player_forgets_what_was_sent() {
        let mut world_map = world_with_two_clients();
        cleanup_all_players_from_world(&mut world_map);
        assert!(world_map.sent_chunks.is_empty());
    }

    #[test]
    fn a_disconnecting_player_is_forgotten_by_the_chunk_bookkeeping() {
        use bevy::ecs::{message::Messages, system::RunSystemOnce};
        use bevy::prelude::{Res, ResMut, World};

        let mut world = World::new();
        world.insert_resource(world_with_two_clients());
        world.init_resource::<Messages<SaveRequestEvent>>();
        world.insert_resource(GameFolderPaths {
            game_folder_path: "unused".into(),
            assets_folder_path: "unused".into(),
        });
        world
            .run_system_once(
                |mut world_map: ResMut<ServerWorldMap>,
                 paths: Res<GameFolderPaths>,
                 mut writer: MessageWriter<SaveRequestEvent>| {
                    cleanup_player_from_world(&mut world_map, &1, &paths, &mut writer);
                },
            )
            .unwrap();

        let world_map = world.resource::<ServerWorldMap>();
        assert!(!world_map.sent_chunks.contains_key(&1));
        assert!(world_map.sent_chunks.contains_key(&2));
    }
}
