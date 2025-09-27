use bevy::{
    math::{bounding::Aabb3d, ops::atan2, Quat, Vec3},
    time::{Fixed, Time},
};
use bevy_ecs::system::{Res, ResMut};
use shared::{
    players::constants::{GRAVITY, JUMP_VELOCITY, SPEED},
    world::{MobAction, MobTarget, ServerWorldMap, WorldMap},
};

pub fn mob_behavior_system(mut world_map: ResMut<ServerWorldMap>, delta: Res<Time<Fixed>>) {
    let mut mobs = world_map.mobs.clone();

    for (_mob_id, mob) in mobs.iter_mut() {
        //log::info!("Mob is at position: {:?}", mob.position);
        if (mob.position.x.is_nan() || mob.position.y.is_nan() || mob.position.z.is_nan())
            || (mob.velocity.x.is_nan() || mob.velocity.y.is_nan() || mob.velocity.z.is_nan())
        {
            //log::error!("Mob has NaN position or velocity");
            // TODO: FIX mob position
            return;
        }
        let target = match mob.target {
            MobTarget::Position(pos) => pos,
            MobTarget::None => continue,
            MobTarget::Player(id) => {
                if let Some(player) = world_map.players.get(&id) {
                    player.position
                } else {
                    mob.position
                }
            }
            MobTarget::Mob(id) => world_map.mobs.get(&id).unwrap().position,
        };

        // same gravity management as the player
        let dir = (target - mob.position).normalize();
        let delta = delta.delta_secs();
        if !mob.on_ground {
            mob.velocity.y += GRAVITY * delta;
        }

        let new_y = mob.position.y + mob.velocity.y;
        let new_vec = &Vec3::new(mob.position.x, new_y, mob.position.z);
        let max_velocity = 0.9;
        if mob.velocity.y > max_velocity {
            mob.velocity.y = max_velocity;
        }
        //log::debug!("New vec: {:?}", new_vec);
        //log::debug!(
        //    "Mob AABB: {:?}",
        //    Aabb3d::new(*new_vec, Vec3::new(mob.width, mob.height, mob.deepth) / 2.0)
        //);
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
            MobAction::Walk | MobAction::Attack => {
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
                    mob.velocity.y += JUMP_VELOCITY * delta;
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
                        mob.velocity.y += JUMP_VELOCITY * delta;
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
            _ => {}
        }
    }

    world_map.mobs = mobs;
}
