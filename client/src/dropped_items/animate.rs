//! How a dropped item moves on screen: towards where the server says it is, turning and
//! bobbing, and flying to whoever picks it up.

use bevy::prelude::*;

use super::{DroppedItemRoot, DroppedItemSpin, FlyingTo};

/// How fast the item shown catches up with where the server says it is: the share of the gap
/// left after one second is `exp(-CATCH_UP_RATE)`.
const CATCH_UP_RATE: f32 = 20.0;
/// Longest time the item is moved on with the velocity of the last update, in seconds: the
/// server sends an update each tick while the item moves.
const MAX_EXTRAPOLATION_SECONDS: f32 = 0.1;
/// An item shown farther than this from where it should be, in blocks, jumps there.
const SNAP_DISTANCE: f32 = 2.0;
/// Turns per second.
const SPIN_SPEED: f32 = std::f32::consts::TAU / 4.0;
/// Height of a bob above the resting place, in blocks, and its speed in radians per second.
const BOB_HEIGHT: f32 = 0.1;
const BOB_SPEED: f32 = std::f32::consts::PI;

pub(super) fn follow_the_server(
    time: Res<Time>,
    mut items: Query<(&mut DroppedItemRoot, &mut Transform)>,
) {
    let delta = time.delta_secs();
    let catch_up = 1.0 - (-CATCH_UP_RATE * delta).exp();
    for (mut item, mut transform) in &mut items {
        item.since_update += delta;
        let expected =
            item.server_position + item.velocity * item.since_update.min(MAX_EXTRAPOLATION_SECONDS);
        if transform.translation.distance(expected) > SNAP_DISTANCE {
            transform.translation = expected;
        } else {
            transform.translation = transform.translation.lerp(expected, catch_up);
        }
    }
}

pub(super) fn spin_and_bob(time: Res<Time>, mut spins: Query<(&DroppedItemSpin, &mut Transform)>) {
    let now = time.elapsed_secs();
    for (spin, mut transform) in &mut spins {
        let t = now + spin.phase;
        transform.rotation = Quat::from_rotation_y(t * SPIN_SPEED);
        transform.translation.y = spin.lift + BOB_HEIGHT * ((t * BOB_SPEED).sin() + 1.0) / 2.0;
    }
}

pub(super) fn fly_to_target(
    mut commands: Commands,
    time: Res<Time>,
    mut flying: Query<(Entity, &mut FlyingTo, &mut Transform)>,
    targets: Query<&GlobalTransform>,
) {
    let delta = time.delta_secs();
    for (entity, mut flight, mut transform) in &mut flying {
        let Ok(target) = targets.get(flight.target) else {
            commands.entity(entity).despawn();
            continue;
        };
        if flight.seconds_left <= delta {
            commands.entity(entity).despawn();
            continue;
        }
        // Covers the share of the way left that this frame is of the time left
        let share = delta / flight.seconds_left;
        transform.translation = transform.translation.lerp(target.translation(), share);
        flight.seconds_left -= delta;
    }
}
