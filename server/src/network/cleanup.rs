use bevy_ecs::message::MessageWriter;
use shared::messages::PlayerId;

use crate::world::{
    data::ServerWorldMap,
    save::{queue_player_save, SaveRequestEvent},
};

pub fn cleanup_all_players_from_world(world_map: &mut ServerWorldMap) {
    for p in world_map.players.values_mut() {
        p.last_input_processed = 0;
    }
    world_map.sent_chunks.clear();
    world_map.sent_items.clear();
    world_map.sent_mobs.clear();
    world_map.render_distances = Default::default();
}

pub fn cleanup_player_from_world(
    world_map: &mut ServerWorldMap,
    player_id: &PlayerId,
    save_event_writer: &mut MessageWriter<SaveRequestEvent>,
) {
    // The player must be queued before it is dropped from the map
    if let Some(player) = world_map.players.remove(player_id) {
        queue_player_save(&mut world_map.players_to_save, &world_map.name, &player);
        save_event_writer.write(SaveRequestEvent::World);
    }

    world_map.sent_chunks.remove(player_id);
    world_map.sent_items.remove(player_id);
    world_map.sent_mobs.remove(player_id);
    world_map.render_distances.remove(*player_id);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::data::SentChunks;
    use bevy::math::IVec3;
    use std::collections::HashSet;

    fn world_with_two_clients() -> ServerWorldMap {
        let mut world_map = ServerWorldMap::default();
        for id in [1, 2] {
            world_map.sent_chunks.insert(
                id,
                SentChunks {
                    chunks: HashSet::from([IVec3::ZERO, IVec3::X]),
                    progress: None,
                },
            );
            world_map.render_distances.set(id, 4, 6);
        }
        world_map
    }

    #[test]
    fn cleaning_up_every_player_forgets_what_was_sent() {
        let mut world_map = world_with_two_clients();
        cleanup_all_players_from_world(&mut world_map);
        assert!(world_map.sent_chunks.is_empty());
        assert_eq!(world_map.render_distances.get(1), 1);
    }

    #[test]
    fn a_disconnecting_player_is_forgotten_by_the_chunk_bookkeeping() {
        use bevy::ecs::{message::Messages, system::RunSystemOnce};
        use bevy::prelude::{ResMut, World};

        let mut world = World::new();
        world.insert_resource(world_with_two_clients());
        world.init_resource::<Messages<SaveRequestEvent>>();
        world
            .run_system_once(
                |mut world_map: ResMut<ServerWorldMap>,
                 mut writer: MessageWriter<SaveRequestEvent>| {
                    cleanup_player_from_world(&mut world_map, &1, &mut writer);
                },
            )
            .unwrap();

        let world_map = world.resource::<ServerWorldMap>();
        assert!(!world_map.sent_chunks.contains_key(&1));
        assert!(world_map.sent_chunks.contains_key(&2));
        assert_eq!(world_map.render_distances.get(1), 1);
        assert_eq!(world_map.render_distances.get(2), 4);
    }
}
