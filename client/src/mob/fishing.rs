//! What a fishing float looks like beyond its model (`MobKind::Bobber`): the line from the rod
//! of its angler, sagging a little, and the bubbles that burst around it when a fish bites.

use bevy::prelude::*;
use rand::Rng;
use shared::players::Player;
use shared::world::MobKind;

use super::Mob;
use crate::portal::particles::spawn_particle;

/// A fish bit the float at `at`.
#[derive(Message, Debug, Clone, Copy)]
pub struct BobberBite {
    pub at: Vec3,
}

/// The mesh and the material of the bubbles, made with the first.
#[derive(Resource, Default)]
pub struct Bubbles(Option<(Handle<Mesh>, Handle<StandardMaterial>)>);

/// Bubbles of a bite.
const BUBBLES: usize = 10;
const BUBBLE_WHITE: Color = Color::srgba(0.85, 0.93, 1.0, 0.9);
/// How far the line sags at its middle, for each block of its length.
const SAG: f32 = 0.06;
/// The segments the line is drawn in.
const LINE_SEGMENTS: usize = 12;
const LINE_COLOUR: Color = Color::srgb(0.12, 0.12, 0.12);

/// Bursts bubbles around each float a fish bites.
pub fn show_bites(
    mut commands: Commands,
    mut bites: MessageReader<BobberBite>,
    mut bubbles: ResMut<Bubbles>,
    meshes: Option<ResMut<Assets<Mesh>>>,
    materials: Option<ResMut<Assets<StandardMaterial>>>,
) {
    let (Some(mut meshes), Some(mut materials)) = (meshes, materials) else {
        bites.clear();
        return;
    };
    let mut rng = rand::thread_rng();
    for bite in bites.read() {
        let (mesh, material) = bubbles
            .0
            .get_or_insert_with(|| {
                (
                    meshes.add(Cuboid::from_length(1.0)),
                    materials.add(StandardMaterial {
                        base_color: BUBBLE_WHITE,
                        unlit: true,
                        alpha_mode: AlphaMode::Blend,
                        ..default()
                    }),
                )
            })
            .clone();
        for _ in 0..BUBBLES {
            let out = Vec3::new(
                rng.gen_range(-1.0..1.0),
                rng.gen_range(0.6..1.6),
                rng.gen_range(-1.0..1.0),
            );
            commands.queue(spawn_particle(
                mesh.clone(),
                material.clone(),
                bite.at + Vec3::new(out.x * 0.2, 0.15, out.z * 0.2),
                rng.gen_range(0.3..0.6),
                rng.gen_range(0.05..0.09),
                out * 1.5,
            ));
        }
    }
}

/// The point at `t`, from 0 to 1, of a line from `from` to `to` sagging by `SAG` a block.
pub fn line_point(from: Vec3, to: Vec3, t: f32) -> Vec3 {
    let sag = SAG * from.distance(to) * 4.0 * t * (1.0 - t);
    from.lerp(to, t) - Vec3::Y * sag
}

/// Draws the line of each float from the rod of its angler.
pub fn draw_lines(mut gizmos: Gizmos, mobs: Query<(&Mob, &Transform)>, players: Query<&Player>) {
    for (mob, transform) in &mobs {
        if mob.kind != MobKind::Bobber {
            continue;
        }
        let Some(angler) = mob
            .angler
            .as_deref()
            .and_then(|name| players.iter().find(|player| player.name == name))
        else {
            continue;
        };
        let forward = angler.camera_transform.forward().as_vec3();
        let tip = angler.position + Vec3::Y * 0.7 + forward * 0.8;
        let float = transform.translation + Vec3::Y * (MobKind::Bobber.size().y / 2.0);
        let points =
            (0..=LINE_SEGMENTS).map(|i| line_point(tip, float, i as f32 / LINE_SEGMENTS as f32));
        gizmos.linestrip(points, LINE_COLOUR);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_line_runs_from_the_rod_to_the_float_and_sags_between() {
        let (from, to) = (Vec3::new(0.0, 2.0, 0.0), Vec3::new(10.0, 0.0, 0.0));
        assert_eq!(line_point(from, to, 0.0), from);
        assert_eq!(line_point(from, to, 1.0), to);
        assert!(line_point(from, to, 0.5).y < 1.0);
    }
}
