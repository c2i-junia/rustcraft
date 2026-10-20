//! How mobs move, once per fixed step: gravity, then walking towards their target or
//! fleeing from it.

use bevy::{
    math::{bounding::Aabb3d, ops::atan2, Quat, Vec3},
    time::{Fixed, Time},
};
use bevy_ecs::system::{Res, ResMut};
use shared::{
    players::{
        constants::{GRAVITY, JUMP_VELOCITY, MAX_UPWARD_VELOCITY, SPEED},
        movement::is_terrain_loaded,
    },
    world::{ChunkMap, MobAction, MobTarget, ServerMob},
};

use crate::world::data::ServerWorldMap;

/// A walking mob that comes this close to its target starts fleeing from it.
const ARRIVAL_DISTANCE: f32 = 0.5;
/// A fleeing mob stops once it is this far from its target.
const SAFE_DISTANCE: f32 = 15.0;
/// Speed of a fleeing mob, in blocks per second.
const FLEE_SPEED: f32 = 1.0;

pub fn mob_behavior_system(mut world_map: ResMut<ServerWorldMap>, time: Res<Time<Fixed>>) {
    let delta = time.delta_secs();
    let ServerWorldMap {
        mobs,
        chunks,
        players,
        ..
    } = world_map.as_mut();

    for mob in mobs.values_mut() {
        // A mob whose position is not a number cannot be simulated: it stays as it is
        if !mob.position.is_finite() || !mob.velocity.is_finite() {
            continue;
        }
        let target = match mob.target {
            MobTarget::Position(position) => position,
            MobTarget::None => continue,
            // A mob does not chase a player that is not connected
            MobTarget::Player(id) => match players.get(&id) {
                Some(player) => player.position,
                None => continue,
            },
        };

        // A mob far from every player stays where it is: the chunks around it may be unloaded,
        // and it would fall through them
        if !is_terrain_loaded(mob.position, chunks) {
            continue;
        }
        let towards_target = (target - mob.position).normalize_or_zero();
        apply_gravity(mob, chunks, delta);
        match mob.action {
            MobAction::Walk => {
                walk(mob, chunks, towards_target, delta);
                if mob.position.distance(target) < ARRIVAL_DISTANCE {
                    mob.action = MobAction::Flee;
                }
            }
            MobAction::Flee => {
                if mob.position.distance(target) < SAFE_DISTANCE {
                    flee(mob, chunks, target, delta);
                }
            }
        }
    }
}

/// Whether `mob` would be inside a block at `position`.
fn collides_at(mob: &ServerMob, chunks: &ChunkMap, position: Vec3) -> bool {
    chunks.check_collision_box(&Aabb3d::new(
        position,
        Vec3::new(mob.width, mob.height, mob.depth) / 2.0,
    ))
}

/// Same vertical motion as a player: fall while in the air, land on the first block met. Half
/// the gravity is added before the move and half after it, so that a jump rises as high as a
/// player's at the 20 steps per second of the mobs.
fn apply_gravity(mob: &mut ServerMob, chunks: &ChunkMap, delta: f32) {
    if !mob.on_ground {
        mob.velocity.y += GRAVITY * delta / 2.0;
    }
    let new_y = mob.position.y + mob.velocity.y * delta;
    mob.velocity.y = mob.velocity.y.min(MAX_UPWARD_VELOCITY);

    if collides_at(mob, chunks, mob.position.with_y(new_y)) {
        mob.on_ground = true;
        mob.velocity.y = 0.0;
    } else {
        mob.position.y = new_y;
        mob.on_ground = false;
        mob.velocity.y += GRAVITY * delta / 2.0;
    }
}

fn jump(mob: &mut ServerMob) {
    mob.velocity.y += JUMP_VELOCITY;
    mob.on_ground = false;
    mob.velocity.x = 0.0;
    mob.velocity.z = 0.0;
}

/// One step along `direction`. A mob blocked while it was moving jumps; one blocked from a
/// standstill slides along the obstacle on one axis, and jumps if both are blocked.
fn walk(mob: &mut ServerMob, chunks: &ChunkMap, direction: Vec3, delta: f32) {
    let step = Vec3::new(direction.x, 0.0, direction.z) * (SPEED * delta);
    let along_x = Vec3::new(step.x, 0.0, 0.0);
    let along_z = Vec3::new(0.0, 0.0, step.z);

    if !collides_at(mob, chunks, mob.position + step) {
        mob.position += step;
        mob.velocity.x = step.x;
        mob.velocity.z = step.z;
    } else if mob.on_ground && mob.velocity.x != 0.0 && mob.velocity.z != 0.0 {
        jump(mob);
    } else if mob.on_ground {
        if !collides_at(mob, chunks, mob.position + along_x) {
            mob.position += along_x;
            mob.velocity.x = step.x;
        } else if !collides_at(mob, chunks, mob.position + along_z) {
            mob.position += along_z;
            mob.velocity.z = step.z;
        } else {
            jump(mob);
        }
    }

    mob.rotation = Quat::from_rotation_y(atan2(direction.x, direction.z));
}

/// One step away from `threat`, on the horizontal plane only: gravity handles the vertical
/// axis. The mob stays in place when a block is in the way.
fn flee(mob: &mut ServerMob, chunks: &ChunkMap, threat: Vec3, delta: f32) {
    let away = (mob.position - threat).with_y(0.0).normalize_or_zero();
    let fled = mob.position + away * (FLEE_SPEED * delta);
    if !collides_at(mob, chunks, fled) {
        mob.position = fled;
    }
    if away != Vec3::ZERO {
        mob.rotation = Quat::from_rotation_y(atan2(away.x, away.z));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;
    use bevy::math::IVec3;
    use bevy::math::Vec3;
    use bevy::prelude::World;
    use shared::world::{BlockId, ChunkMap, MobKind, ServerMob};
    use shared::TICKS_PER_SECOND;
    use std::time::Duration;

    /// What `Time<Fixed>` is on the server: the duration of a tick.
    fn tick() -> Duration {
        Duration::from_secs_f64(1.0 / TICKS_PER_SECOND as f64)
    }

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
            depth: 1.0,
        }
    }

    /// Runs a step of `mobs`, in the air of loaded chunks up to y = 63.
    fn run_behavior(mobs: Vec<ServerMob>) -> ServerWorldMap {
        let mut world = World::new();
        let mut map = ServerWorldMap {
            chunks: ChunkMap::with_loaded_region(IVec3::splat(-32), IVec3::splat(63)),
            ..Default::default()
        };
        map.mobs = mobs
            .into_iter()
            .enumerate()
            .map(|(i, m)| (i as u128, m))
            .collect();
        world.insert_resource(map);
        let mut time = Time::<Fixed>::default();
        time.advance_by(tick());
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
    fn a_mob_over_chunks_that_are_not_loaded_stays_still() {
        let position = Vec3::new(0.0, 50.0, 0.0);
        let mob = mob_at(position, MobTarget::Position(Vec3::X * 10.0));
        let mut world = World::new();
        world.insert_resource(ServerWorldMap {
            mobs: [(0, mob)].into(),
            ..Default::default()
        });
        let mut time = Time::<Fixed>::default();
        time.advance_by(tick());
        world.insert_resource(time);
        world.run_system_once(mob_behavior_system).unwrap();
        let map = world.resource::<ServerWorldMap>();
        assert_eq!(map.mobs[&0].position, position);
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

    /// Flat stone ground (top face at y = 1) with an optional wall of stone at x = -2.
    fn ground_world(with_wall: bool) -> ServerWorldMap {
        let mut map = ServerWorldMap {
            // The chunks below the ground too: a mob is not moved without them
            chunks: ChunkMap::with_loaded_region(IVec3::new(-10, -16, -10), IVec3::new(9, 3, 9)),
            ..Default::default()
        };
        map.chunks
            .fill_box(IVec3::new(-10, 0, -10), IVec3::new(9, 0, 9), BlockId::Stone);
        if with_wall {
            map.chunks
                .fill_box(IVec3::new(-2, 1, -10), IVec3::new(-2, 3, 9), BlockId::Stone);
        }
        map
    }

    fn fleeing_fox_at_origin(target: Vec3) -> ServerMob {
        let mut fox = mob_at(Vec3::new(0.5, 1.5, 0.5), MobTarget::Position(target));
        fox.action = MobAction::Flee;
        fox.on_ground = true;
        fox
    }

    fn run_seconds(mut map: ServerWorldMap, fox: ServerMob, seconds: u64) -> ServerWorldMap {
        map.mobs.insert(0, fox);
        let mut world = World::new();
        world.insert_resource(map);
        let mut time = Time::<Fixed>::default();
        time.advance_by(tick());
        world.insert_resource(time);
        for _ in 0..seconds * TICKS_PER_SECOND {
            world.run_system_once(mob_behavior_system).unwrap();
        }
        world.remove_resource::<ServerWorldMap>().unwrap()
    }

    #[test]
    fn a_fleeing_mob_does_not_sink_when_its_target_is_above() {
        for target in [Vec3::new(0.5, 9.0, 0.5), Vec3::new(3.5, 8.0, 0.5)] {
            let map = run_seconds(ground_world(false), fleeing_fox_at_origin(target), 5);
            let fox = &map.mobs[&0];
            assert!(
                (fox.position.y - 1.5).abs() < 0.05,
                "target {target}: y = {}",
                fox.position.y
            );
        }
    }

    #[test]
    fn a_fleeing_mob_does_not_enter_a_wall() {
        let target = Vec3::new(5.5, 1.5, 0.5);
        let map = run_seconds(ground_world(true), fleeing_fox_at_origin(target), 10);
        let fox = map.mobs[&0].clone();
        let hitbox = Aabb3d::new(
            fox.position,
            Vec3::new(fox.width, fox.height, fox.depth) / 2.0,
        );
        assert!(
            !map.chunks.check_collision_box(&hitbox),
            "fox at {}",
            fox.position
        );
        assert!(
            fox.position.x < 0.4 && fox.position.x > -0.6,
            "the fox should have run up to the wall and stopped: {}",
            fox.position
        );
    }

    #[test]
    fn a_fleeing_mob_runs_away_horizontally_on_open_ground() {
        let target = Vec3::new(3.5, 1.5, 4.5);
        let map = run_seconds(ground_world(false), fleeing_fox_at_origin(target), 2);
        let moved = map.mobs[&0].position - Vec3::new(0.5, 1.5, 0.5);
        // about one block per second for two seconds, along the direction away from the target
        let away = Vec3::new(-3.0, 0.0, -4.0).normalize();
        assert!((moved.dot(away) - 2.0).abs() < 0.2, "moved {moved}");
        assert!(moved.y.abs() < 0.05);
    }
}
