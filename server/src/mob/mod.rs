pub mod behavior;

use bevy::prelude::*;
use shared::world::{MobAction, MobKind, MobTarget, ServerMob, ServerWorldMap, WorldMap};
use ulid::Ulid;

use crate::init::ServerTime;

fn create_new_mob_id() -> u128 {
    Ulid::new().0
}

pub fn manage_mob_spawning_system(mut world_map: ResMut<ServerWorldMap>, time: Res<ServerTime>) {
    if time.0 == 100 && !world_map.players.is_empty() {
        debug!("Should spawn mob");

        let id = create_new_mob_id();
        info!(
            "Heigt : {}",
            world_map
                .chunks
                .get_heigh_ground(Vec3::new(0.0, 0.0, 0.0))
                .to_string()
        );
        let position = Vec3::new(
            0.0,
            world_map.chunks.get_heigh_ground(Vec3::new(0.0, 0.0, 0.0)) as f32 + 2.0,
            0.0,
        );

        let mob = ServerMob {
            kind: MobKind::Fox,
            position,
            target: MobTarget::Player(*world_map.players.keys().next().unwrap()),
            action: MobAction::Walk,
            rotation: Quat::IDENTITY,
            height: 1.0,
            width: 1.0,
            deepth: 1.5,
            on_ground: true,
            velocity: Vec3::ZERO,
        };

        info!("Spawning new mob on server: {:?}", mob);

        world_map.mobs.insert(id, mob);
    }
}
