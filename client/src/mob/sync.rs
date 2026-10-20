//! The server's updates of the mobs, turned into entities that move smoothly between them and
//! play the clip of what they do.

use std::collections::{HashMap, HashSet};
use std::time::Duration;

use bevy::prelude::*;
use shared::world::{MobAction, MobId, MobKind, ServerMob};

use super::{
    models::{clip_for, model, MobAnimations, MobClip},
    MobRoot,
};
use crate::state::GameState;

/// How fast a mob drawn catches up with where the server says it is, per second: the server
/// moves it 20 times a second, the screen many more.
const CATCH_UP_RATE: f32 = 15.0;
/// A mob farther than this from where it is drawn jumps there, in blocks.
const JUMP_DISTANCE: f32 = 4.0;
/// How long a mob takes to go from one clip to the next.
const CLIP_TRANSITION: Duration = Duration::from_millis(250);

/// The mobs the server told of and not drawn yet, and those it took back. They are kept here
/// from the loading screen on, when no mob is drawn yet.
#[derive(Resource, Default)]
pub struct ClientMobs {
    updated: HashMap<MobId, ServerMob>,
    removed: HashSet<MobId>,
}

impl ClientMobs {
    pub fn update(&mut self, id: MobId, mob: ServerMob) {
        self.removed.remove(&id);
        self.updated.insert(id, mob);
    }

    pub fn remove(&mut self, id: MobId) {
        self.updated.remove(&id);
        self.removed.insert(id);
    }
}

/// The entity of each mob drawn.
#[derive(Resource, Default)]
pub struct MobEntities(HashMap<MobId, Entity>);

/// A mob drawn, and where the server last said it is and what it does.
#[derive(Component, Debug)]
pub struct Mob {
    pub kind: MobKind,
    pub action: MobAction,
    position: Vec3,
    rotation: Quat,
}

/// The animation player of a mob's model: the mob it belongs to, and the clip it plays.
#[derive(Component)]
pub struct MobAnimator {
    mob: Entity,
    playing: MobClip,
}

pub fn apply_mob_updates(
    mut commands: Commands,
    mut client_mobs: ResMut<ClientMobs>,
    mut entities: ResMut<MobEntities>,
    mut mobs: Query<&mut Mob>,
    asset_server: Res<AssetServer>,
) {
    for id in std::mem::take(&mut client_mobs.removed) {
        if let Some(entity) = entities.0.remove(&id) {
            commands.entity(entity).despawn();
        }
    }
    for (id, server) in std::mem::take(&mut client_mobs.updated) {
        if let Some(mut mob) = entities.0.get(&id).and_then(|&e| mobs.get_mut(e).ok()) {
            mob.action = server.action;
            mob.position = server.position;
            mob.rotation = server.rotation;
            continue;
        }
        let entity = spawn_mob(&mut commands, &server, &asset_server);
        entities.0.insert(id, entity);
    }
}

/// The entity of a mob: its position is the middle of its hitbox, its model stands on the
/// bottom of it.
fn spawn_mob(commands: &mut Commands, server: &ServerMob, asset_server: &AssetServer) -> Entity {
    let model = model(server.kind);
    let scene = asset_server.load(GltfAssetLabel::Scene(0).from_asset(model.path));
    commands
        .spawn((
            MobRoot,
            Mob {
                kind: server.kind,
                action: server.action,
                position: server.position,
                rotation: server.rotation,
            },
            Transform::from_translation(server.position).with_rotation(server.rotation),
            Visibility::default(),
            DespawnOnExit(GameState::Game),
            children![(
                WorldAssetRoot(scene),
                Transform::from_xyz(0.0, -server.kind.size().y / 2.0, 0.0)
                    .with_scale(Vec3::splat(model.scale)),
            )],
        ))
        .id()
}

/// Moves each mob drawn toward where the server last said it is.
pub fn smooth_mob_motion(time: Res<Time>, mut mobs: Query<(&Mob, &mut Transform)>) {
    let catch_up = 1.0 - (-CATCH_UP_RATE * time.delta_secs()).exp();
    for (mob, mut transform) in &mut mobs {
        if transform.translation.distance(mob.position) > JUMP_DISTANCE {
            transform.translation = mob.position;
        } else {
            transform.translation = transform.translation.lerp(mob.position, catch_up);
        }
        transform.rotation = transform.rotation.slerp(mob.rotation, catch_up);
    }
}

/// Gives the animation player of a model just loaded the graph of its mob, and starts the clip
/// of what the mob does.
pub fn setup_mob_animators(
    mut commands: Commands,
    mut players: Query<(Entity, &mut AnimationPlayer), Added<AnimationPlayer>>,
    parents: Query<&ChildOf>,
    mobs: Query<&Mob>,
    mut animations: ResMut<MobAnimations>,
    asset_server: Res<AssetServer>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
) {
    for (entity, mut player) in &mut players {
        let Some((mob_entity, mob)) = parents
            .iter_ancestors(entity)
            .find_map(|ancestor| mobs.get(ancestor).ok().map(|mob| (ancestor, mob)))
        else {
            continue;
        };
        let kind_animations = animations.of(mob.kind, &asset_server, &mut graphs);
        let graph = kind_animations.graph.clone();
        let clip = clip_for(mob.kind, mob.action);
        let node = kind_animations.nodes[&clip];

        let mut transitions = AnimationTransitions::new();
        // The clips start through the transitions, which would be confused by a clip started
        // on the player directly
        transitions.play(&mut player, node, Duration::ZERO).repeat();
        commands.entity(entity).insert((
            AnimationGraphHandle(graph),
            transitions,
            MobAnimator {
                mob: mob_entity,
                playing: clip,
            },
        ));
    }
}

/// Plays the clip of what each mob does now.
pub fn play_mob_clips(
    mut animators: Query<(
        &mut MobAnimator,
        &mut AnimationPlayer,
        &mut AnimationTransitions,
    )>,
    mobs: Query<&Mob>,
    animations: Res<MobAnimations>,
) {
    for (mut animator, mut player, mut transitions) in &mut animators {
        let Ok(mob) = mobs.get(animator.mob) else {
            continue;
        };
        let clip = clip_for(mob.kind, mob.action);
        if clip == animator.playing {
            continue;
        }
        let Some(kind_animations) = animations.get(mob.kind) else {
            continue;
        };
        transitions
            .play(&mut player, kind_animations.nodes[&clip], CLIP_TRANSITION)
            .repeat();
        animator.playing = clip;
    }
}

/// Forgets the mobs of a session that ended: their entities are despawned with the game.
pub fn forget_mobs(mut client_mobs: ResMut<ClientMobs>, mut entities: ResMut<MobEntities>) {
    *client_mobs = ClientMobs::default();
    entities.0.clear();
}
