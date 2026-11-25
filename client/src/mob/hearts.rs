//! The hearts that rise over an animal that wants a mate, now and then, and burst over one as it
//! is tamed: small red particles (`portal::particles`), a heart of 7 by 6 pixels turned to the
//! eye.

use bevy::prelude::*;
use rand::Rng;

use super::Mob;
use crate::portal::particles::spawn_turned_particle;
use crate::portal::PortalCamera;

/// Seconds between two hearts over an animal that wants a mate.
const LOVE_PERIOD: f32 = 0.6;
/// Hearts that burst over an animal as it is tamed.
const TAME_BURST: usize = 7;
/// Side of a heart, in blocks.
const HEART_SIZE: f32 = 0.3;
/// Seconds a heart lasts.
const HEART_LIFETIME: f32 = 1.2;
/// How fast a heart rises at first, in blocks per second.
const HEART_RISE: f32 = 1.5;
const HEART_RED: Color = Color::srgb(0.92, 0.14, 0.24);

/// The pixels of a heart, top row first.
const HEART: [&str; 6] = [
    ".XX.XX.", //
    "XXXXXXX", //
    "XXXXXXX", //
    ".XXXXX.", //
    "..XXX..", //
    "...X...",
];

/// A heart one block wide, a pixel thick, facing +Z, centred on the origin.
fn heart_mesh() -> Mesh {
    let pixel = 1.0 / HEART[0].len() as f32;
    let mut mesh: Option<Mesh> = None;
    for (row, line) in HEART.iter().enumerate() {
        for (column, cell) in line.chars().enumerate() {
            if cell != 'X' {
                continue;
            }
            let centre = Vec3::new(
                (column as f32 + 0.5) * pixel - 0.5,
                (HEART.len() as f32 / 2.0 - row as f32 - 0.5) * pixel,
                0.0,
            );
            let cube = Mesh::from(Cuboid::from_length(pixel)).translated_by(centre);
            match &mut mesh {
                Some(mesh) => {
                    mesh.merge(&cube).expect("cuboids share their attributes");
                }
                None => mesh = Some(cube),
            }
        }
    }
    mesh.expect("a heart has pixels")
}

/// The mesh and the material of the hearts, made with the first.
#[derive(Resource, Default)]
pub struct Hearts(Option<(Handle<Mesh>, Handle<StandardMaterial>)>);

/// What the hearts of a mob know of it: when the next rises, and whether it was tamed when
/// last seen.
#[derive(Component)]
pub struct HeartTimer {
    next: f32,
    tamed: bool,
}

/// How many hearts rise over `mob` after `seconds` more: one every `LOVE_PERIOD` while it wants
/// a mate, and `TAME_BURST` as it is tamed. A mob first seen tamed was tamed before.
fn hearts_due(mob: &Mob, timer: &mut HeartTimer, seconds: f32) -> usize {
    let mut due = 0;
    if mob.tamed && !timer.tamed {
        due += TAME_BURST;
    }
    timer.tamed = mob.tamed;
    if mob.in_love {
        timer.next -= seconds;
        if timer.next <= 0.0 {
            timer.next += LOVE_PERIOD;
            due += 1;
        }
    } else {
        timer.next = 0.0;
    }
    due
}

pub fn show_hearts(
    mut commands: Commands,
    time: Res<Time>,
    mut hearts: ResMut<Hearts>,
    meshes: Option<ResMut<Assets<Mesh>>>,
    materials: Option<ResMut<Assets<StandardMaterial>>>,
    mut mobs: Query<(Entity, &Mob, &Transform, Option<&mut HeartTimer>)>,
    cameras: Query<&GlobalTransform, (With<Camera3d>, Without<PortalCamera>)>,
) {
    let eye = cameras.iter().next().map(GlobalTransform::translation);
    let (Some(mut meshes), Some(mut materials)) = (meshes, materials) else {
        return;
    };
    let (mesh, material) = hearts
        .0
        .get_or_insert_with(|| {
            let red = HEART_RED.to_linear();
            (
                meshes.add(heart_mesh()),
                materials.add(StandardMaterial {
                    base_color: HEART_RED,
                    emissive: red * 2.0,
                    unlit: true,
                    ..default()
                }),
            )
        })
        .clone();
    let mut rng = rand::thread_rng();
    for (entity, mob, transform, timer) in &mut mobs {
        let Some(mut timer) = timer else {
            commands.entity(entity).insert(HeartTimer {
                next: 0.0,
                tamed: mob.tamed,
            });
            continue;
        };
        let top = transform.translation + Vec3::Y * (mob.kind.size().y * mob.scale / 2.0 + 0.2);
        for _ in 0..hearts_due(mob, &mut timer, time.delta_secs()) {
            let offset = Vec3::new(rng.gen_range(-0.3..0.3), 0.0, rng.gen_range(-0.3..0.3));
            let at = top + offset;
            // Its face, +Z, toward the eye
            let towards = eye.map_or(Vec3::Z, |eye| (eye - at).with_y(0.0));
            let turn = Quat::from_rotation_y(towards.x.atan2(towards.z));
            commands.queue(spawn_turned_particle(
                mesh.clone(),
                material.clone(),
                Transform::from_translation(at).with_rotation(turn),
                HEART_LIFETIME,
                HEART_SIZE,
                Vec3::Y * HEART_RISE + offset,
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::world::{MobKind, ServerMob};

    fn cat(in_love: u32, owner: Option<&str>) -> Mob {
        Mob::of(&ServerMob {
            in_love,
            owner: owner.map(str::to_string),
            ..ServerMob::new(MobKind::Cat, Vec3::ZERO)
        })
    }

    #[test]
    fn hearts_rise_now_and_then_over_an_animal_that_wants_a_mate() {
        let mob = cat(100, None);
        let mut timer = HeartTimer {
            next: 0.0,
            tamed: false,
        };
        let total: usize = (0..58).map(|_| hearts_due(&mob, &mut timer, 0.05)).sum();
        // 2.9 seconds: one at once, then one every 0.6 s
        assert_eq!(total, 5);
        assert_eq!(hearts_due(&cat(0, None), &mut timer, 1.0), 0);
    }

    #[test]
    fn hearts_burst_once_as_an_animal_is_tamed_but_not_over_one_seen_tamed() {
        let mut timer = HeartTimer {
            next: 0.0,
            tamed: false,
        };
        let tamed = cat(0, Some("Ann"));
        assert_eq!(hearts_due(&tamed, &mut timer, 0.05), TAME_BURST);
        assert_eq!(hearts_due(&tamed, &mut timer, 0.05), 0);
    }
}
