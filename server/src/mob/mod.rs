pub mod behavior;

use bevy::prelude::*;
use shared::world::{MobAction, MobKind, MobTarget, ServerMob};
use ulid::Ulid;

use crate::init::ServerTime;
use crate::world::data::ServerWorldMap;

fn create_new_mob_id() -> u128 {
    Ulid::new().0
}

/// The tick at which the one mob of a world, a fox, appears: mobs are still a prototype.
const FOX_SPAWN_TICK: u64 = 100;
/// How far above the ground a mob is created, so that it lands instead of starting in a block.
const SPAWN_HEIGHT_ABOVE_GROUND: f32 = 2.0;

pub fn manage_mob_spawning_system(mut world_map: ResMut<ServerWorldMap>, time: Res<ServerTime>) {
    if time.0 != FOX_SPAWN_TICK {
        return;
    }
    // The fox chases whoever is connected at that tick
    let Some(&target) = world_map.players.keys().next() else {
        return;
    };

    let ground = world_map.chunks.get_height_ground(Vec3::ZERO) as f32;
    let mob = ServerMob {
        kind: MobKind::Fox,
        position: Vec3::new(0.0, ground + SPAWN_HEIGHT_ABOVE_GROUND, 0.0),
        target: MobTarget::Player(target),
        action: MobAction::Walk,
        rotation: Quat::IDENTITY,
        height: 1.0,
        width: 1.0,
        depth: 1.5,
        on_ground: true,
        velocity: Vec3::ZERO,
    };
    info!("Spawning new mob on server: {:?}", mob);
    world_map.mobs.insert(create_new_mob_id(), mob);
}
