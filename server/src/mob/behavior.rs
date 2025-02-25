use bevy::{
    math::{ops::atan2, Quat},
    time::{Fixed, Time},
};
use bevy_ecs::system::{Res, ResMut};
use shared::world::{MobAction, MobTarget, ServerWorldMap};

pub fn mob_behavior_system(mut world_map: ResMut<ServerWorldMap>, delta: Res<Time<Fixed>>) {
    let mut mobs = world_map.mobs.clone();

    for (_mob_id, mob) in mobs.iter_mut() {
        let target = match mob.target {
            MobTarget::Position(pos) => pos,
            MobTarget::None => continue,
            MobTarget::Player(id) => world_map.players.get(&id).unwrap().position,
            MobTarget::Mob(id) => world_map.mobs.get(&id).unwrap().position,
        };

        let dir = (target - mob.position).normalize();
        let velocity = 2.0 * delta.delta_secs();

        match mob.action {
            MobAction::Walk | MobAction::Attack => {
                mob.position += dir * velocity;
                mob.rotation = Quat::from_rotation_y(atan2(dir.x, dir.z));

                // If reached destination, start idling
                if mob.position.distance(target) < 0.5 {
                    mob.action = MobAction::Flee;
                }
            }
            MobAction::Flee => {
                if mob.position.distance(target) < 15.0 {
                    mob.position -= dir * velocity;
                    mob.rotation = Quat::from_rotation_y(atan2(-dir.x, -dir.z));
                }
            }
            _ => {}
        }
    }

    world_map.mobs = mobs;
}
