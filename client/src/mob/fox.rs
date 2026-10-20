//! The fox: the dust its feet raise when it runs.

use bevy::{
    animation::{AnimationEvent, AnimationTargetId},
    asset::AssetMut,
    color::palettes::css::WHITE,
    prelude::*,
};
use rand::{thread_rng, Rng};
use shared::world::MobKind;

use super::models::{MobAnimations, MobClip};

/// The fox model is in centimetres: its dust is sized and thrown at the same scale.
const FOX_SCALE: f32 = 0.01;

#[derive(AnimationEvent, Reflect, Clone)]
pub struct OnStep;

pub fn observe_on_step(
    trigger: On<OnStep>,
    particle: Res<ParticleAssets>,
    mut commands: Commands,
    transforms: Query<&GlobalTransform>,
) {
    // The foot may be gone already: a fox is despawned with its bones
    let Ok(transform) = transforms.get(trigger.trigger().target) else {
        return;
    };
    let translation = transform.translation();
    let mut rng = thread_rng();
    // Spawn a bunch of particles.
    for _ in 0..14 {
        let horizontal =
            Vec2::from_angle(rng.gen_range(0.0..std::f32::consts::TAU)) * rng.gen_range(8.0..12.0);
        let vertical = rng.gen_range(0.0..4.0);
        let size = rng.gen_range(0.2..1.0);
        commands.queue(spawn_particle(
            particle.mesh.clone(),
            particle.material.clone(),
            translation,
            rng.gen_range(0.2..0.6),
            size,
            Vec3::new(horizontal.x, vertical, horizontal.y) * (10.0 * FOX_SCALE),
        ));
    }
}

/// Adds the `OnStep` events to the run of the fox once it is loaded. The clip is shared by all
/// foxes: its events are added once, or each fox would fire them all.
pub fn add_fox_step_events(
    animations: Res<MobAnimations>,
    graphs: Res<Assets<AnimationGraph>>,
    mut clips: ResMut<Assets<AnimationClip>>,
    feet: Res<FoxFeetTargets>,
    mut added: Local<bool>,
) {
    if *added {
        return;
    }
    let Some(fox) = animations.get(MobKind::Fox) else {
        return;
    };
    let run = fox.nodes[&MobClip::Run];
    let Some(AnimationNodeType::Clip(handle)) = graphs
        .get(&fox.graph)
        .and_then(|g| g.get(run))
        .map(|node| &node.node_type)
    else {
        return;
    };
    let Some(running) = clips.get_mut(handle).map(AssetMut::into_inner) else {
        return;
    };
    // A foot hits the ground at these times: the frame divided by the frame rate of the
    // animation, frame 15 at 24 fps is 15 / 24 = 0.625.
    running.add_event_to_target(feet.front_left, 0.625, OnStep);
    running.add_event_to_target(feet.front_right, 0.5, OnStep);
    running.add_event_to_target(feet.back_left, 0.0, OnStep);
    running.add_event_to_target(feet.back_right, 0.125, OnStep);
    *added = true;
}

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

fn spawn_particle<M: Material>(
    mesh: Handle<Mesh>,
    material: Handle<M>,
    translation: Vec3,
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
            Transform {
                translation,
                scale: Vec3::splat(size),
                ..default()
            },
        ));
    }
}

#[derive(Component)]
pub struct Particle {
    lifetime_timer: Timer,
    size: f32,
    velocity: Vec3,
}

#[derive(Resource)]
pub struct ParticleAssets {
    mesh: Handle<Mesh>,
    material: Handle<StandardMaterial>,
}

impl FromWorld for ParticleAssets {
    fn from_world(world: &mut World) -> Self {
        Self {
            mesh: world
                .resource_mut::<Assets<Mesh>>()
                .add(Sphere::new(10.0 * FOX_SCALE)),
            material: world
                .resource_mut::<Assets<StandardMaterial>>()
                .add(StandardMaterial {
                    base_color: WHITE.into(),
                    ..default()
                }),
        }
    }
}

#[derive(Resource)]
pub struct FoxFeetTargets {
    front_right: AnimationTargetId,
    front_left: AnimationTargetId,
    back_left: AnimationTargetId,
    back_right: AnimationTargetId,
}

impl Default for FoxFeetTargets {
    fn default() -> Self {
        // Get the id's of the feet and store them in a resource.
        let hip_node = ["root", "_rootJoint", "b_Root_00", "b_Hip_01"];
        let front_left_foot = hip_node.iter().chain(
            [
                "b_Spine01_02",
                "b_Spine02_03",
                "b_LeftUpperArm_09",
                "b_LeftForeArm_010",
                "b_LeftHand_011",
            ]
            .iter(),
        );
        let front_right_foot = hip_node.iter().chain(
            [
                "b_Spine01_02",
                "b_Spine02_03",
                "b_RightUpperArm_06",
                "b_RightForeArm_07",
                "b_RightHand_08",
            ]
            .iter(),
        );
        let back_left_foot = hip_node.iter().chain(
            [
                "b_LeftLeg01_015",
                "b_LeftLeg02_016",
                "b_LeftFoot01_017",
                "b_LeftFoot02_018",
            ]
            .iter(),
        );
        let back_right_foot = hip_node.iter().chain(
            [
                "b_RightLeg01_019",
                "b_RightLeg02_020",
                "b_RightFoot01_021",
                "b_RightFoot02_022",
            ]
            .iter(),
        );
        Self {
            front_left: AnimationTargetId::from_iter(front_left_foot),
            front_right: AnimationTargetId::from_iter(front_right_foot),
            back_left: AnimationTargetId::from_iter(back_left_foot),
            back_right: AnimationTargetId::from_iter(back_right_foot),
        }
    }
}
