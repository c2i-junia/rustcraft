//! Plays animations from a skinned glTF.

use std::time::Duration;

use bevy::{
    animation::{AnimationEvent, AnimationTargetId},
    asset::AssetMut,
    color::palettes::css::WHITE,
    prelude::*,
};
use rand::{thread_rng, Rng};

use super::MobRoot;

const FOX_PATH: &str = "models/animated/Fox.glb";

#[derive(Resource, Default)]
pub struct Animations {
    animations: Vec<AnimationNodeIndex>,
    graph: Handle<AnimationGraph>,
}

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
            translation.reject_from_normalized(Vec3::Y),
            rng.gen_range(0.2..0.6),
            size,
            Vec3::new(horizontal.x, vertical, horizontal.y) * 10.0,
        ));
    }
}

pub fn setup_fox(
    id: u128,
    spawn_pos: Vec3,
    commands: &mut Commands,
    asset_server: &Res<AssetServer>,
    graphs: &mut ResMut<Assets<AnimationGraph>>,
) {
    // Build the animation graph
    let (graph, node_indices) = AnimationGraph::from_clips([
        asset_server.load(GltfAssetLabel::Animation(2).from_asset(FOX_PATH)),
        asset_server.load(GltfAssetLabel::Animation(1).from_asset(FOX_PATH)),
        asset_server.load(GltfAssetLabel::Animation(0).from_asset(FOX_PATH)),
    ]);

    // Insert a resource with the current scene information
    let graph_handle = graphs.add(graph);
    commands.insert_resource(Animations {
        animations: node_indices,
        graph: graph_handle,
    });

    // Fox
    let fox = commands
        .spawn((
            WorldAssetRoot(asset_server.load(GltfAssetLabel::Scene(0).from_asset(FOX_PATH))),
            Transform::from_translation(spawn_pos).with_scale(Vec3::splat(0.01)),
            MobRoot { id },
        ))
        .id();

    info!("Spawned fox: {:?}", fox);
}

// An `AnimationPlayer` is automatically added to the scene when it's ready.
// When the player is added, start the animation.
pub fn setup_fox_once_loaded(
    mut commands: Commands,
    animations: Res<Animations>,
    feet: Res<FoxFeetTargets>,
    graphs: Res<Assets<AnimationGraph>>,
    mut clips: ResMut<Assets<AnimationClip>>,
    mut players: Query<(Entity, &mut AnimationPlayer), Added<AnimationPlayer>>,
    mut events_registered: Local<bool>,
) {
    let Some(&running) = animations.animations.first() else {
        return;
    };

    for (entity, mut player) in &mut players {
        info!("setup_fox_once_loaded called with entity: {:?}", entity);

        // The clip is shared by all foxes, so its events must be added only once.
        // Otherwise each new fox adds them again and every fox fires all of them.
        if !*events_registered {
            if let Some(running_animation) = clip_of(running, &animations, &graphs, &mut clips) {
                // Send `OnStep` events once the fox feet hits the ground in the running
                // animation. The time of an event is its frame divided by the frame rate of
                // the animation: frame 15 at 24 fps is 15 / 24 = 0.625.
                running_animation.add_event_to_target(feet.front_left, 0.625, OnStep);
                running_animation.add_event_to_target(feet.front_right, 0.5, OnStep);
                running_animation.add_event_to_target(feet.back_left, 0.0, OnStep);
                running_animation.add_event_to_target(feet.back_right, 0.125, OnStep);
                *events_registered = true;
            }
        }

        let mut transitions = AnimationTransitions::new();

        // Make sure to start the animation via the `AnimationTransitions`
        // component. The `AnimationTransitions` component wants to manage all
        // the animations and will get confused if the animations are started
        // directly via the `AnimationPlayer`.
        transitions
            .play(&mut player, running, Duration::ZERO)
            .repeat();
        commands
            .entity(entity)
            .insert(AnimationGraphHandle(animations.graph.clone()))
            .insert(transitions);
    }
}

/// The animation clip played by `node` of the fox graph, once the model is loaded.
fn clip_of<'a>(
    node: AnimationNodeIndex,
    animations: &Animations,
    graphs: &Assets<AnimationGraph>,
    clips: &'a mut Assets<AnimationClip>,
) -> Option<&'a mut AnimationClip> {
    let graph = graphs.get(&animations.graph)?;
    let AnimationNodeType::Clip(handle) = &graph.get(node)?.node_type else {
        return None;
    };
    clips.get_mut(handle).map(AssetMut::into_inner)
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
            mesh: world.resource_mut::<Assets<Mesh>>().add(Sphere::new(10.0)),
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
