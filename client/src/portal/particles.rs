//! The sparks a shot bursts into: small particles thrown out, which slow down and shrink to
//! nothing.

use bevy::prelude::*;

use crate::state::GameState;

pub fn simulate_particles(
    mut commands: Commands,
    mut query: Query<(Entity, &mut Transform, &mut Particle)>,
    time: Res<Time>,
) {
    for (entity, mut transform, mut particle) in &mut query {
        if particle.lifetime_timer.tick(time.delta()).just_finished() {
            commands.entity(entity).despawn();
        } else {
            transform.translation += particle.velocity * time.delta_secs();
            transform.scale =
                Vec3::splat(particle.size.lerp(0.0, particle.lifetime_timer.fraction()));
            particle
                .velocity
                .smooth_nudge(&Vec3::ZERO, 4.0, time.delta_secs());
        }
    }
}

/// Spawns a particle of `size` at `translation`, thrown at `velocity`, which slows down and
/// shrinks to nothing in `lifetime` seconds, or goes when the game is left.
pub(crate) fn spawn_particle<M: Material>(
    mesh: Handle<Mesh>,
    material: Handle<M>,
    translation: Vec3,
    lifetime: f32,
    size: f32,
    velocity: Vec3,
) -> impl Command {
    spawn_turned_particle(
        mesh,
        material,
        Transform::from_translation(translation),
        lifetime,
        size,
        velocity,
    )
}

/// A particle as `spawn_particle` makes, placed and turned as `transform` says.
pub(crate) fn spawn_turned_particle<M: Material>(
    mesh: Handle<Mesh>,
    material: Handle<M>,
    transform: Transform,
    lifetime: f32,
    size: f32,
    velocity: Vec3,
) -> impl Command {
    move |world: &mut World| {
        world.spawn((
            Particle {
                lifetime_timer: Timer::from_seconds(lifetime, TimerMode::Once),
                size,
                velocity,
            },
            Mesh3d(mesh),
            MeshMaterial3d(material),
            transform.with_scale(Vec3::splat(size)),
            // Frozen once the game is left, as nothing moves it then
            DespawnOnExit(GameState::Game),
        ));
    }
}

#[derive(Component)]
pub struct Particle {
    lifetime_timer: Timer,
    size: f32,
    velocity: Vec3,
}
