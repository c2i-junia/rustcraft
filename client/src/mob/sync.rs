//! The server's updates of the mobs, turned into entities that move smoothly between them and
//! play the clip of what they do.

use std::collections::{HashMap, HashSet};
use std::time::Duration;

use bevy::prelude::*;
use shared::world::{MobAction, MobId, MobKind, ServerMob};

use super::{
    hurt::MobHurt,
    models::{clip_for, model, model_lift, MobAnimations, MobClip},
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
    /// How many portals the server said the mob went through.
    passes: u32,
    /// The points of health the server said the mob lost.
    damage: u32,
    /// The time since the mob was last hurt, while it flashes red (`hurt::flash_hurt_mobs`).
    pub hurt_seconds: Option<f32>,
}

impl Mob {
    /// A mob first seen as `server` says it is, which is no hurt.
    pub fn of(server: &ServerMob) -> Self {
        Self {
            kind: server.kind,
            action: server.action,
            position: server.position,
            rotation: server.rotation,
            passes: server.passes,
            damage: server.damage,
            hurt_seconds: None,
        }
    }
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
    mut mobs: Query<(&mut Mob, &mut Transform)>,
    asset_server: Res<AssetServer>,
    mut hurts: MessageWriter<MobHurt>,
) {
    for id in std::mem::take(&mut client_mobs.removed) {
        if let Some(entity) = entities.0.remove(&id) {
            commands.entity(entity).despawn();
        }
    }
    for (id, server) in std::mem::take(&mut client_mobs.updated) {
        if let Some((mut mob, mut transform)) =
            entities.0.get(&id).and_then(|&e| mobs.get_mut(e).ok())
        {
            if take_update(&mut mob, &mut transform, &server) {
                hurts.write(MobHurt {
                    kind: server.kind,
                    at: server.position,
                });
            }
            continue;
        }
        let entity = spawn_mob(&mut commands, &server, &asset_server);
        entities.0.insert(id, entity);
    }
}

/// Takes the update `server` of the mob `mob`, drawn at `transform`: the mob drawn moves on
/// toward where the server says (`smooth_mob_motion`), but one that went through a portal
/// since the last update (`ServerMob::passes`) is put there at once, turned as it is, rather
/// than eased across the gap between the portals. Returns whether the mob was hurt: whether
/// its damage rose, which starts its red flash.
fn take_update(mob: &mut Mob, transform: &mut Transform, server: &ServerMob) -> bool {
    let hurt = server.damage > mob.damage;
    mob.damage = server.damage;
    if hurt {
        mob.hurt_seconds = Some(0.0);
    }
    mob.action = server.action;
    mob.position = server.position;
    mob.rotation = server.rotation;
    if mob.passes != server.passes {
        mob.passes = server.passes;
        transform.translation = server.position;
        transform.rotation = server.rotation;
    }
    hurt
}

/// The entity of a mob: its position is the middle of its hitbox, its model stands on the
/// bottom of it.
fn spawn_mob(commands: &mut Commands, server: &ServerMob, asset_server: &AssetServer) -> Entity {
    let model = model(server.kind);
    let scene = asset_server.load(GltfAssetLabel::Scene(0).from_asset(model.path));
    commands
        .spawn((
            MobRoot,
            Mob::of(server),
            Transform::from_translation(server.position).with_rotation(server.rotation),
            Visibility::default(),
            DespawnOnExit(GameState::Game),
            children![(
                WorldAssetRoot(scene),
                Transform::from_xyz(
                    0.0,
                    -server.kind.size().y / 2.0 - model_lift(server.kind),
                    0.0
                ),
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
        let active = transitions.play(&mut player, node, Duration::ZERO);
        if !clip.plays_once() {
            active.repeat();
        }
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
        let active = transitions.play(&mut player, kind_animations.nodes[&clip], CLIP_TRANSITION);
        if !clip.plays_once() {
            active.repeat();
        }
        animator.playing = clip;
    }
}

/// Forgets the mobs of a session that ended: their entities are despawned with the game.
pub fn forget_mobs(mut client_mobs: ResMut<ClientMobs>, mut entities: ResMut<MobEntities>) {
    *client_mobs = ClientMobs::default();
    entities.0.clear();
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::{FRAC_PI_2, PI};

    #[test]
    fn an_update_that_raises_the_damage_of_a_mob_is_a_hurt() {
        let mut server = ServerMob::new(MobKind::Pig, Vec3::ZERO);
        let mut mob = Mob::of(&server);
        let mut transform = Transform::default();
        assert!(!take_update(&mut mob, &mut transform, &server));
        server.damage = 3;
        assert!(take_update(&mut mob, &mut transform, &server));
        // The same damage told again, as a moving mob is, is no new hurt
        assert!(!take_update(&mut mob, &mut transform, &server));
        // A mob first seen hurt is not flashing
        assert_eq!(Mob::of(&server).hurt_seconds, None);
    }

    #[test]
    fn a_mob_that_went_through_a_portal_is_put_on_the_other_side_at_once() {
        let shown = Vec3::new(1.0, 1.625, -2.6);
        let mut mob = Mob {
            kind: MobKind::Sheep,
            action: MobAction::Walk,
            position: shown,
            rotation: Quat::from_rotation_y(PI),
            passes: 0,
            damage: 0,
            hurt_seconds: None,
        };
        let mut transform = Transform::from_translation(shown).with_rotation(mob.rotation);
        let mut server = ServerMob {
            action: MobAction::Walk,
            ..ServerMob::new(MobKind::Sheep, shown - Vec3::Z * 0.05)
        };

        // A step on the same side is eased
        take_update(&mut mob, &mut transform, &server);
        assert_eq!(transform.translation, shown);

        // Out of a portal two blocks away, closer than a mob ever jumps by itself, turned west
        server.position = Vec3::new(2.9, 1.625, -1.0);
        server.rotation = Quat::from_rotation_y(-FRAC_PI_2);
        server.passes = 1;
        take_update(&mut mob, &mut transform, &server);
        assert_eq!(transform.translation, server.position);
        assert_eq!(transform.rotation, server.rotation);
        assert_eq!(mob.passes, 1);
    }
}
