//! The server's updates of the mobs, turned into entities that move smoothly between them and
//! play the clip of what they do.

use std::collections::{HashMap, HashSet};
use std::time::Duration;

use bevy::prelude::*;
use shared::constants::TICKS_PER_SECOND;
use shared::world::{Coat, ItemId, Leash, MobAction, MobId, MobKind, ServerMob, GROW_TICKS};

/// A young mob first seen this few ticks into its growth was just born, not come into sight.
const BORN_TICKS: u32 = 2 * TICKS_PER_SECOND as u32;

use super::{
    hurt::MobHurt,
    models::{clip_for, model_lift, scene_path, MobAnimations, MobClip},
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

impl MobEntities {
    /// The entity of the mob `id`, if it is drawn.
    pub fn get(&self, id: MobId) -> Option<Entity> {
        self.0.get(&id).copied()
    }
}

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
    /// How big the mob is drawn: smaller while young (`ServerMob::is_young`).
    pub scale: f32,
    /// Whether the mob wants a mate: hearts rise over it (`hearts::show_hearts`).
    pub in_love: bool,
    /// Whether the mob was tamed: hearts burst over it when it is.
    pub tamed: bool,
    /// Whether the mob was first seen just born (`audio::cues`): young, its growth hardly begun.
    pub born: bool,
    /// The coat of a horse, which its model was drawn with.
    pub coat: Option<Coat>,
    /// Whether a horse wears its saddle (`show_saddles`).
    pub saddled: bool,
    /// The points of health the mob has at most, for the mount's bar (`ui::hud::mount`).
    pub max_health: u32,
    /// How many blows the mob parried (`ServerMob::parries`): each change is heard.
    parries: u8,
    /// The name of the player whose rod a float hangs from: its line is drawn to it.
    pub angler: Option<String>,
    /// The id of the mob, which a chest on its back is opened by.
    pub id: MobId,
    /// The name of the player who tamed the animal.
    pub owner: Option<String>,
    /// Whether a tamed wolf or cat sits (`sitting_pose`).
    pub sitting: bool,
    /// The horse armour a horse wears (`show_gear`).
    pub horse_armour: Option<ItemId>,
    /// Whether a donkey or a mule carries a chest (`show_gear`).
    pub chested: bool,
    /// What the lead of the mob is tied to: it is drawn (`draw_leads`).
    pub leash: Option<Leash>,
    /// The splash potion a thrown potion is: its burst is of its colour.
    pub potion: Option<ItemId>,
}

/// The model of a mob, a child of its entity, scaled with the mob.
#[derive(Component)]
pub struct MobModel;

/// How big a mob of `server` is drawn, against an adult of its kind.
fn scale_of(server: &ServerMob) -> f32 {
    if server.is_young() {
        server.height / server.kind.size().y
    } else {
        1.0
    }
}

/// The transform of the model of a mob of `kind` drawn at `scale`: standing on the bottom of
/// its hitbox; one that sits leans back on its hind legs, its rump lowered.
fn model_transform(kind: MobKind, scale: f32, sitting: bool) -> Transform {
    let standing = Transform::from_xyz(0.0, -(kind.size().y / 2.0 + model_lift(kind)) * scale, 0.0)
        .with_scale(Vec3::splat(scale));
    if !sitting {
        return standing;
    }
    // The model faces -Z: raising its front turns it about +X, pivoting on its hind feet
    let size = kind.size() * scale;
    let hind = Vec3::new(0.0, standing.translation.y, size.z * 0.35);
    let tilt = Quat::from_rotation_x(SIT_TILT);
    let lowered = Vec3::Y * (size.y * 0.18);
    Transform {
        translation: hind + tilt * (standing.translation - hind) - lowered,
        rotation: tilt,
        scale: standing.scale,
    }
}

/// How far a sitting wolf or cat leans back, in radians.
const SIT_TILT: f32 = 0.45;

impl Mob {
    /// A mob first seen as `server` says it is, which is no hurt.
    pub fn of(id: MobId, server: &ServerMob) -> Self {
        Self {
            kind: server.kind,
            action: server.action,
            position: server.position,
            rotation: server.rotation,
            passes: server.passes,
            damage: server.damage,
            hurt_seconds: None,
            scale: scale_of(server),
            in_love: server.in_love > 0,
            tamed: server.owner.is_some() && server.kind.is_animal(),
            born: server.is_young() && server.growing + BORN_TICKS >= GROW_TICKS,
            coat: server.horse.map(|traits| traits.coat),
            saddled: server.saddled,
            max_health: server.max_health(),
            parries: server.parries,
            angler: server
                .owner
                .clone()
                .filter(|_| server.kind == MobKind::Bobber),
            id,
            owner: server.owner.clone(),
            sitting: server.sitting,
            horse_armour: server.horse_armour,
            chested: server.pack.is_some(),
            leash: server.leash.clone(),
            potion: server.potion,
        }
    }

    /// The points of health the server said the mob lost.
    pub fn damage(&self) -> u32 {
        self.damage
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
    mut sounds: MessageWriter<crate::audio::PlaySound>,
    mut bites: MessageWriter<super::fishing::BobberBite>,
    mut bursts: MessageWriter<super::potions::PotionBurst>,
) {
    use crate::audio::{PlaySound, Sound};
    for id in std::mem::take(&mut client_mobs.removed) {
        if let Some(entity) = entities.0.remove(&id) {
            // A float drawn in winds the reel
            if let Ok((mob, transform)) = mobs.get(entity) {
                if mob.kind == MobKind::Bobber {
                    sounds.write(PlaySound::at(Sound::FishingReel, transform.translation));
                }
                // A splash potion bursts where it ends
                if mob.kind == MobKind::ThrownPotion {
                    sounds.write(PlaySound::at(Sound::PotionShatter, transform.translation));
                    bursts.write(super::potions::PotionBurst {
                        at: transform.translation,
                        potion: mob.potion,
                    });
                }
            }
            commands.entity(entity).despawn();
        }
    }
    for (id, server) in std::mem::take(&mut client_mobs.updated) {
        if let Some((mut mob, mut transform)) =
            entities.0.get(&id).and_then(|&e| mobs.get_mut(e).ok())
        {
            if mob.parries != server.parries {
                mob.parries = server.parries;
                sounds.write(crate::audio::PlaySound::at(
                    crate::audio::Sound::ShieldBlock,
                    server.position,
                ));
            }
            // A fish bites: the float plunges with a plop and bubbles
            if server.kind == MobKind::Bobber
                && server.action == MobAction::Attack
                && mob.action != MobAction::Attack
            {
                sounds.write(PlaySound::at(Sound::FishingBite, server.position));
                bites.write(super::fishing::BobberBite {
                    at: server.position,
                });
            }
            if take_update(&mut mob, &mut transform, &server) {
                hurts.write(MobHurt {
                    kind: server.kind,
                    at: server.position,
                    struck: server.struck,
                });
            }
            continue;
        }
        if server.kind == MobKind::Bobber {
            sounds.write(PlaySound::at(Sound::FishingCast, server.position));
        }
        let entity = spawn_mob(&mut commands, id, &server, &asset_server);
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
    mob.scale = scale_of(server);
    mob.in_love = server.in_love > 0;
    mob.tamed = server.owner.is_some() && server.kind.is_animal();
    mob.saddled = server.saddled;
    mob.owner.clone_from(&server.owner);
    mob.sitting = server.sitting;
    mob.horse_armour = server.horse_armour;
    mob.chested = server.pack.is_some();
    mob.leash.clone_from(&server.leash);
    mob.max_health = server.max_health();
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
fn spawn_mob(
    commands: &mut Commands,
    id: MobId,
    server: &ServerMob,
    asset_server: &AssetServer,
) -> Entity {
    let path = scene_path(server.kind, server.horse.map(|traits| traits.coat));
    let scene = asset_server.load(GltfAssetLabel::Scene(0).from_asset(path));
    commands
        .spawn((
            MobRoot,
            Mob::of(id, server),
            Transform::from_translation(server.position).with_rotation(server.rotation),
            Visibility::default(),
            DespawnOnExit(GameState::Game),
            children![(
                WorldAssetRoot(scene),
                MobModel,
                model_transform(server.kind, scale_of(server), server.sitting),
            )],
        ))
        .id()
}

/// Scales the model of each mob that grew up, or was first drawn young.
pub fn scale_mob_models(
    mobs: Query<(&Mob, &Children), Changed<Mob>>,
    mut models: Query<&mut Transform, With<MobModel>>,
) {
    for (mob, children) in &mobs {
        for &child in children {
            if let Ok(mut transform) = models.get_mut(child) {
                let wanted = model_transform(mob.kind, mob.scale, mob.sitting);
                if *transform != wanted {
                    *transform = wanted;
                }
            }
        }
    }
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

/// Whether the part `name` of the model of `mob` shows: the saddle of a saddled steed, the
/// horse armour a horse wears, the chest a donkey or a mule carries; every other part always.
/// They are parts of the models of the steeds (`scripts/generate_animals.py`).
pub fn shows_part(mob: &Mob, name: &str) -> bool {
    match name {
        "saddle" => mob.saddled,
        "chest" => mob.chested,
        _ => match name.strip_prefix("armour_") {
            Some(material) => mob
                .horse_armour
                .is_some_and(|armour| armour_part(armour) == material),
            None => true,
        },
    }
}

/// The name of the part of the horse's model that shows `armour`, after `armour_`.
fn armour_part(armour: ItemId) -> &'static str {
    match armour {
        ItemId::LeatherHorseArmour => "leather",
        ItemId::IronHorseArmour => "iron",
        ItemId::GoldHorseArmour => "gold",
        _ => "diamond",
    }
}

/// Shows the gear of each steed (`shows_part`) and hides the rest.
pub fn show_saddles(
    mobs: Query<(Entity, &Mob)>,
    children: Query<&Children>,
    mut parts: Query<(&Name, &mut Visibility), Without<Mob>>,
) {
    for (entity, mob) in &mobs {
        if !mob.kind.is_steed() {
            continue;
        }
        for descendant in children.iter_descendants(entity) {
            if let Ok((name, mut visibility)) = parts.get_mut(descendant) {
                let wanted = if shows_part(mob, name.as_str()) {
                    Visibility::Inherited
                } else {
                    Visibility::Hidden
                };
                visibility.set_if_neq(wanted);
            }
        }
    }
}

/// Puts the mount the local player drives under it, where its prediction says, rather than
/// where the server last said: the mount would trail behind its rider.
pub fn place_ridden_mount(
    local: Query<&shared::players::Player, With<crate::player::CurrentPlayerMarker>>,
    entities: Res<MobEntities>,
    mut mobs: Query<&mut Transform, With<Mob>>,
) {
    let Ok(player) = local.single() else {
        return;
    };
    let Some(riding) = player.riding.filter(|riding| riding.seat == 0) else {
        return;
    };
    let Some(mut transform) = entities.get(riding.mob).and_then(|e| mobs.get_mut(e).ok()) else {
        return;
    };
    transform.translation = player.position - riding.seat_offset();
    transform.rotation = Quat::from_rotation_y(riding.heading);
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
        let mut mob = Mob::of(0, &server);
        let mut transform = Transform::default();
        assert!(!take_update(&mut mob, &mut transform, &server));
        server.damage = 3;
        assert!(take_update(&mut mob, &mut transform, &server));
        // The same damage told again, as a moving mob is, is no new hurt
        assert!(!take_update(&mut mob, &mut transform, &server));
        // A mob first seen hurt is not flashing
        assert_eq!(Mob::of(0, &server).hurt_seconds, None);
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
            scale: 1.0,
            in_love: false,
            tamed: false,
            born: false,
            coat: None,
            saddled: false,
            max_health: 8,
            parries: 0,
            angler: None,
            id: 0,
            owner: None,
            sitting: false,
            horse_armour: None,
            chested: false,
            leash: None,
            potion: None,
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
