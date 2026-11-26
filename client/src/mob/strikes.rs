//! What a special blow looks like (`melee::Strike`): a critical one bursts into golden sparks
//! over the mob, a sweep leaves a pale arc across it, as seen from the eye.

use bevy::prelude::*;
use rand::Rng;
use shared::players::melee::Strike;

use super::MobHurt;
use crate::portal::particles::spawn_particle;
use crate::portal::PortalCamera;

/// Sparks of a critical blow.
const CRITICAL_SPARKS: usize = 12;
/// Specks of the arc of a sweep.
const SWEEP_SPECKS: usize = 11;
/// How wide the arc of a sweep spreads to each side, in radians.
const SWEEP_SPREAD: f32 = 1.1;
/// How far from the middle of the mob the arc runs, in blocks.
const SWEEP_RADIUS: f32 = 1.2;
const CRITICAL_GOLD: Color = Color::srgb(1.0, 0.82, 0.25);
const SWEEP_WHITE: Color = Color::srgb(0.92, 0.94, 1.0);

/// The mesh and the two materials of the sparks, made with the first.
#[derive(Resource, Default)]
pub struct StrikeSparks(
    Option<(
        Handle<Mesh>,
        Handle<StandardMaterial>,
        Handle<StandardMaterial>,
    )>,
);

/// Where the specks of the arc of a sweep on a mob at `at`, seen from `eye`, start, and how they
/// fly: across the mob, from one side to the other, flat, bending around it.
pub fn sweep_arc(at: Vec3, eye: Vec3) -> Vec<(Vec3, Vec3)> {
    let toward = (at - eye)
        .with_y(0.0)
        .try_normalize()
        .unwrap_or(Vec3::NEG_Z);
    let centre = at - toward * SWEEP_RADIUS * 0.5;
    (0..SWEEP_SPECKS)
        .map(|i| {
            let t = i as f32 / (SWEEP_SPECKS - 1) as f32 * 2.0 - 1.0;
            let direction = Quat::from_rotation_y(t * SWEEP_SPREAD) * toward;
            let tangent = Vec3::Y.cross(direction);
            (centre + direction * SWEEP_RADIUS, tangent * 2.0)
        })
        .collect()
}

pub fn show_strikes(
    mut commands: Commands,
    mut hurts: MessageReader<MobHurt>,
    mut sparks: ResMut<StrikeSparks>,
    meshes: Option<ResMut<Assets<Mesh>>>,
    materials: Option<ResMut<Assets<StandardMaterial>>>,
    cameras: Query<&GlobalTransform, (With<Camera3d>, Without<PortalCamera>)>,
) {
    let (Some(mut meshes), Some(mut materials)) = (meshes, materials) else {
        hurts.clear();
        return;
    };
    let eye = cameras.iter().next().map(GlobalTransform::translation);
    let mut rng = rand::thread_rng();
    for hurt in hurts.read() {
        let Some(strike) = hurt.struck else {
            continue;
        };
        let (mesh, gold, white) = sparks
            .0
            .get_or_insert_with(|| {
                let unlit = |color: Color| StandardMaterial {
                    base_color: color,
                    emissive: color.to_linear() * 2.0,
                    unlit: true,
                    ..default()
                };
                (
                    meshes.add(Cuboid::from_length(1.0)),
                    materials.add(unlit(CRITICAL_GOLD)),
                    materials.add(unlit(SWEEP_WHITE)),
                )
            })
            .clone();
        let size = hurt.kind.size();
        match strike {
            Strike::Critical => {
                let top = hurt.at + Vec3::Y * size.y * 0.3;
                for _ in 0..CRITICAL_SPARKS {
                    let out = Vec3::new(
                        rng.gen_range(-1.0..1.0),
                        rng.gen_range(0.2..1.2),
                        rng.gen_range(-1.0..1.0),
                    );
                    commands.queue(spawn_particle(
                        mesh.clone(),
                        gold.clone(),
                        top,
                        rng.gen_range(0.4..0.7),
                        0.09,
                        out * 4.0,
                    ));
                }
            }
            Strike::Sweep => {
                let eye = eye.unwrap_or(hurt.at + Vec3::Z * 3.0);
                for (at, velocity) in sweep_arc(hurt.at, eye) {
                    commands.queue(spawn_particle(
                        mesh.clone(),
                        white.clone(),
                        at,
                        0.35,
                        0.12,
                        velocity,
                    ));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_arc_of_a_sweep_crosses_the_mob_from_one_side_to_the_other() {
        let at = Vec3::new(0.0, 1.0, -3.0);
        let arc = sweep_arc(at, Vec3::new(0.0, 1.6, 0.0));
        let (first, last) = (arc[0].0, arc[arc.len() - 1].0);
        // From one side of the mob to the other, at its height
        assert!(first.x * last.x < 0.0, "{first} {last}");
        assert!(arc.iter().all(|(p, _)| p.y == at.y && p.distance(at) < 2.0));
    }
}
