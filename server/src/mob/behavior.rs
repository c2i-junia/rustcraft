use bevy::{
    math::{bounding::Aabb3d, ops::atan2, Quat, Vec3},
    time::{Fixed, Time},
};
use bevy_ecs::system::{Res, ResMut};
use shared::{
    players::constants::{GRAVITY, JUMP_VELOCITY, MAX_UPWARD_VELOCITY, SPEED},
    world::{MobAction, MobTarget, ServerWorldMap, WorldMap},
};

pub fn mob_behavior_system(mut world_map: ResMut<ServerWorldMap>, delta: Res<Time<Fixed>>) {
    let mut mobs = world_map.mobs.clone();

    for mob in mobs.values_mut() {
        if !mob.position.is_finite() || !mob.velocity.is_finite() {
            // TODO: FIX mob position
            continue;
        }
        let target = match mob.target {
            MobTarget::Position(pos) => pos,
            MobTarget::None => continue,
            MobTarget::Player(id) => {
                // A mob does not chase a player that is not connected
                let Some(player) = world_map.players.get(&id) else {
                    continue;
                };
                player.position
            }
        };

        // same gravity management as the player
        let dir = (target - mob.position).normalize_or_zero();
        let delta = delta.delta_secs();
        if !mob.on_ground {
            mob.velocity.y += GRAVITY * delta;
        }

        let new_y = mob.position.y + mob.velocity.y * delta;
        let new_vec = &Vec3::new(mob.position.x, new_y, mob.position.z);
        if mob.velocity.y > MAX_UPWARD_VELOCITY {
            mob.velocity.y = MAX_UPWARD_VELOCITY;
        }
        if world_map.chunks.check_collision_box(&Aabb3d::new(
            *new_vec,
            Vec3::new(mob.width, mob.height, mob.deepth) / 2.0,
        )) {
            mob.on_ground = true;
            mob.velocity.y = 0.0;
        } else {
            mob.position.y = new_y;
            mob.on_ground = false;
        }

        match mob.action {
            MobAction::Walk => {
                let speed = SPEED * delta;
                let new_x = mob.position.x + dir.x * speed;
                let new_z = mob.position.z + dir.z * speed;
                let new_vec = &Vec3::new(new_x, mob.position.y, new_z);
                //try to move the target
                if !world_map.chunks.check_collision_box(&Aabb3d::new(
                    *new_vec,
                    Vec3::new(mob.width, mob.height, mob.deepth) / 2.0,
                )) {
                    mob.position.x = new_x;
                    mob.position.z = new_z;
                    mob.velocity.x = dir.x * speed;
                    mob.velocity.z = dir.z * speed;
                }
                // If it can't move, try to jump (only if on ground and if it moved before)
                else if mob.on_ground && mob.velocity.x != 0.0 && mob.velocity.z != 0.0 {
                    mob.velocity.y += JUMP_VELOCITY;
                    mob.on_ground = false;
                    mob.velocity.x = 0.0;
                    mob.velocity.z = 0.0;
                } else if mob.on_ground {
                    // Try to move in the other direction
                    // Check if it can move in the x direction
                    if !world_map.chunks.check_collision_box(&Aabb3d::new(
                        Vec3::new(
                            mob.position.x + dir.x * speed,
                            mob.position.y,
                            mob.position.z,
                        ),
                        Vec3::new(mob.width, mob.height, mob.deepth) / 2.0,
                    )) {
                        mob.position.x += dir.x * speed;
                        mob.velocity.x = dir.x * speed;
                    // Check if it can move in the z direction
                    } else if !world_map.chunks.check_collision_box(&Aabb3d::new(
                        Vec3::new(
                            mob.position.x,
                            mob.position.y,
                            mob.position.z + dir.z * speed,
                        ),
                        Vec3::new(mob.width, mob.height, mob.deepth) / 2.0,
                    )) {
                        mob.position.z += dir.z * speed;
                        mob.velocity.z = dir.z * speed;
                    //Try to jump (can improve this)
                    } else {
                        mob.velocity.y += JUMP_VELOCITY;
                        mob.on_ground = false;
                        mob.velocity.x = 0.0;
                        mob.velocity.z = 0.0;
                    }
                }

                mob.rotation = Quat::from_rotation_y(atan2(dir.x, dir.z));

                // If reached destination, start idling
                if mob.position.distance(target) < 0.5 {
                    mob.action = MobAction::Flee;
                }
            }
            MobAction::Flee => {
                if mob.position.distance(target) < 15.0 {
                    mob.position -= dir * delta;
                    mob.rotation = Quat::from_rotation_y(atan2(-dir.x, -dir.z));
                }
            }
        }
    }

    world_map.mobs = mobs;
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;
    use bevy::math::Vec3;
    use bevy::prelude::World;
    use shared::world::{MobKind, ServerMob};
    use std::time::Duration;

    fn mob_at(position: Vec3, target: MobTarget) -> ServerMob {
        ServerMob {
            kind: MobKind::Fox,
            target,
            action: MobAction::Walk,
            position,
            rotation: Quat::IDENTITY,
            height: 1.0,
            width: 1.0,
            on_ground: false,
            velocity: Vec3::ZERO,
            deepth: 1.0,
        }
    }

    fn run_behavior(mobs: Vec<ServerMob>) -> ServerWorldMap {
        let mut world = World::new();
        let mut map = ServerWorldMap::default();
        map.mobs = mobs
            .into_iter()
            .enumerate()
            .map(|(i, m)| (i as u128, m))
            .collect();
        world.insert_resource(map);
        let mut time = Time::<Fixed>::default();
        time.advance_by(Duration::from_millis(15));
        world.insert_resource(time);
        world.run_system_once(mob_behavior_system).unwrap();
        world.remove_resource::<ServerWorldMap>().unwrap()
    }

    #[test]
    fn a_mob_with_a_nan_position_does_not_stop_the_other_mobs() {
        let broken = mob_at(Vec3::NAN, MobTarget::Position(Vec3::ZERO));
        let healthy_position = Vec3::new(0.0, 50.0, 0.0);
        let healthy = mob_at(healthy_position, MobTarget::Position(Vec3::X * 10.0));
        // The healthy mob comes second: the NaN one used to end the whole loop.
        let map = run_behavior(vec![broken, healthy]);
        assert_ne!(map.mobs[&1].position, healthy_position);
        assert!(map.mobs[&1].position.is_finite());
    }

    #[test]
    fn a_mob_targeting_an_absent_player_is_left_alone() {
        let position = Vec3::new(0.0, 50.0, 0.0);
        let map = run_behavior(vec![mob_at(position, MobTarget::Player(99))]);
        assert_eq!(map.mobs[&0].position, position);
        assert_eq!(map.mobs[&0].velocity, Vec3::ZERO);
    }

    #[test]
    fn a_mob_on_its_target_keeps_a_finite_state() {
        let position = Vec3::new(0.0, 50.0, 0.0);
        let map = run_behavior(vec![mob_at(position, MobTarget::Position(position))]);
        assert!(map.mobs[&0].position.is_finite());
        assert!(map.mobs[&0].rotation.is_finite());
    }
}
