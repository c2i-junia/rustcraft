//! The tamed animals beside their owners. A wolf fights for its owner: it hunts what its owner
//! strikes (`rally`) and what goes at its owner (`guard`), on foot along the way it finds
//! (`hunt`), bites in reach (`bite`), and the hostile it bites strikes back. A tamed cat keeps
//! the hostiles that fear it (`fears_cats`) away from around it (`hostile::shun_cats`). A
//! steed takes half the blows of the hostiles at its rider, which its armour lessens
//! (`bear_mount_blows`). A wolf or a cat sits, or follows again, at a right click of its owner
//! (`toggle_sit`).

use std::collections::{HashMap, HashSet};

use bevy::math::Vec3;
use rand::Rng;
use shared::{
    messages::PlayerId,
    players::Player,
    world::{
        armoured_damage, hostiles::distance_to_box, MobAction, MobId, MobKind, MobTarget,
        ServerMob,
    },
};

use super::path::Pathing;
use crate::world::dropped_items::DroppedItems;

/// A hostile that fears the cats keeps this far from a tamed one, in blocks: on the
/// horizontal plane for the Gust Owl, which circles high over its prey.
pub const CAT_FEAR_DISTANCE: f32 = 8.0;
/// The points of health a wolf's bite takes.
pub const WOLF_BITE: u32 = 4;
/// How near the box of its prey the centre of a wolf bites, in blocks.
pub const BITE_REACH: f32 = 1.0;
/// Ticks between two bites of a wolf.
pub const BITE_COOLDOWN: u32 = 20;
/// How fast a wolf runs at its prey, in blocks/s.
pub const HUNT_SPEED: f32 = 4.5;
/// A wolf gives up a prey this far from it, or from its owner, in blocks.
pub const FORGET_DISTANCE: f32 = 24.0;
/// A wolf takes as its prey a hostile that goes at its owner this close to the owner, in blocks.
pub const GUARD_DISTANCE: f32 = 12.0;
/// The wolves of a player this close to what it strikes hunt it, in blocks.
pub const RALLY_DISTANCE: f32 = 24.0;
/// Ticks a bite shows (`MobAction::Attack`).
const BITE_SHOW_TICKS: u32 = 10;
/// Horizontal and upward speed of a mob a wolf bites, away from it.
const BITE_PUSH: f32 = 3.0;
const BITE_LIFT: f32 = 3.0;

/// Whether hostiles of `kind` fear the tamed cats: the Gnawer, a cave rat, and the Gust Owl, a
/// bird.
pub fn fears_cats(kind: MobKind) -> bool {
    matches!(kind, MobKind::Gnawer | MobKind::GustOwl)
}

/// The nearest of the tamed cats at `cats` within `CAT_FEAR_DISTANCE` of `position`, on the
/// horizontal plane when `flat`.
pub fn nearest_cat(cats: &[Vec3], position: Vec3, flat: bool) -> Option<Vec3> {
    let distance = |cat: &Vec3| {
        let between = *cat - position;
        if flat {
            between.with_y(0.0).length()
        } else {
            between.length()
        }
    };
    cats.iter()
        .copied()
        .filter(|cat| distance(cat) < CAT_FEAR_DISTANCE)
        .min_by(|a, b| distance(a).total_cmp(&distance(b)))
}

/// What the mobs of a step know of one another: where the tamed cats are, the box of each mob,
/// and the bites of the wolves, which `apply_bites` lands once every mob moved.
#[derive(Default)]
pub struct Pack {
    pub cats: Vec<Vec3>,
    /// The centre and the size of each mob.
    pub boxes: HashMap<MobId, (Vec3, Vec3)>,
    /// The wolf that bites, and its prey.
    pub bites: Vec<(MobId, MobId)>,
}

impl Pack {
    /// What `mobs` are before their step.
    pub fn survey(mobs: &HashMap<MobId, ServerMob>) -> Self {
        Self {
            cats: mobs
                .values()
                .filter(|mob| mob.kind == MobKind::Cat && mob.owner.is_some())
                .map(|mob| mob.position)
                .collect(),
            boxes: mobs
                .iter()
                .map(|(&id, mob)| {
                    let size = Vec3::new(mob.width, mob.height, mob.depth);
                    (id, (mob.position, size))
                })
                .collect(),
            bites: Vec::new(),
        }
    }
}

/// Whether a wolf may hunt `prey`: neither a tamed animal, a villager, a projectile, a vehicle
/// nor a boss, which fights in its lair.
pub fn may_hunt(prey: &ServerMob) -> bool {
    !(prey.kind.is_animal() && prey.owner.is_some())
        && !prey.kind.is_villager()
        && !prey.kind.is_projectile()
        && !prey.kind.is_vehicle()
        && !prey.kind.is_boss()
}

/// Whether `mob` is a wolf of `owner` ready to fight: tamed by it, grown up and standing.
fn fights_for(mob: &ServerMob, owner: &str) -> bool {
    mob.kind == MobKind::Wolf
        && mob.owner.as_deref() == Some(owner)
        && !mob.sitting
        && !mob.is_young()
}

/// The player named `owner` struck the mob `prey`: its wolves near it hunt it.
pub fn rally(mobs: &mut HashMap<MobId, ServerMob>, owner: &str, prey: MobId) {
    let Some(target) = mobs.get(&prey).filter(|mob| may_hunt(mob)) else {
        return;
    };
    let at = target.position;
    for (id, mob) in mobs.iter_mut() {
        if *id != prey && fights_for(mob, owner) && mob.position.distance(at) <= RALLY_DISTANCE {
            mob.prey = Some(prey);
        }
    }
}

/// Each wolf without a prey takes as its prey the nearest hostile that goes at its owner within
/// `GUARD_DISTANCE` of it.
pub fn guard(mobs: &mut HashMap<MobId, ServerMob>, players: &HashMap<PlayerId, Player>) {
    let threats: Vec<(MobId, PlayerId, Vec3)> = mobs
        .iter()
        .filter(|(_, mob)| mob.kind.is_hostile() && may_hunt(mob))
        .filter_map(|(&id, mob)| match mob.target {
            MobTarget::Player(player) => Some((id, player, mob.position)),
            _ => None,
        })
        .collect();
    if threats.is_empty() {
        return;
    }
    for mob in mobs.values_mut() {
        if mob.kind != MobKind::Wolf || mob.prey.is_some() || mob.sitting || mob.is_young() {
            continue;
        }
        let Some(owner) = super::breeding::owner_of(players, mob) else {
            continue;
        };
        mob.prey = threats
            .iter()
            .filter(|(_, hunted, at)| {
                *hunted == owner.id && at.distance(owner.position) <= GUARD_DISTANCE
            })
            .min_by(|a, b| {
                let to = |at: &Vec3| at.distance(mob.position);
                to(&a.2).total_cmp(&to(&b.2))
            })
            .map(|(id, _, _)| *id);
    }
}

/// How far the centre of `mob` is from the box at `target`.
fn reach_of(mob: &ServerMob, (centre, size): (Vec3, Vec3)) -> f32 {
    distance_to_box(mob.position, centre, size / 2.0)
}

/// What a wolf `id` with a prey does this step: it forgets a prey gone, or too far from it or
/// from its owner at `owner`; it runs at it along the way it finds (`hostile::chase_to`), and
/// bites it in reach once its last bite is far enough behind (`Pack::bites`). Returns whether
/// it hunted, which is all it did this step.
pub fn hunt(
    mob: &mut ServerMob,
    id: MobId,
    owner: Option<Vec3>,
    pack: &mut Pack,
    chunks: &shared::world::ChunkMap,
    pathing: &mut Pathing,
    delta: f32,
) -> bool {
    let Some(prey) = mob.prey else {
        return false;
    };
    let target = pack.boxes.get(&prey).copied().filter(|&(at, _)| {
        prey != id
            && at.distance(mob.position) <= FORGET_DISTANCE
            && owner.is_some_and(|owner| owner.distance(at) <= FORGET_DISTANCE)
    });
    let Some(target) = target.filter(|_| !mob.sitting) else {
        mob.prey = None;
        if matches!(mob.action, MobAction::Chase | MobAction::Attack) {
            (mob.action, mob.target, mob.action_ticks) = (MobAction::Idle, MobTarget::None, 1);
        }
        return false;
    };
    let towards = (target.0 - mob.position).with_y(0.0);
    if towards.length_squared() > 1e-6 {
        mob.rotation =
            bevy::math::Quat::from_rotation_y(bevy::math::ops::atan2(towards.x, towards.z));
    }
    mob.target = MobTarget::Position(target.0);
    if reach_of(mob, target) <= BITE_REACH {
        mob.velocity.x = 0.0;
        mob.velocity.z = 0.0;
        if mob.attack_wait == 0 {
            pack.bites.push((id, prey));
            mob.attack_wait = BITE_COOLDOWN;
            mob.action = MobAction::Attack;
            mob.action_ticks = BITE_SHOW_TICKS;
        } else if mob.action != MobAction::Attack || mob.action_ticks <= 1 {
            mob.action = MobAction::Idle;
        } else {
            mob.action_ticks -= 1;
        }
        return true;
    }
    mob.action = MobAction::Chase;
    super::hostile::chase_to(
        mob,
        chunks,
        (target.0, target.1.y),
        HUNT_SPEED,
        delta,
        pathing,
    );
    true
}

/// The mob takes `damage` points of health and is thrown back along `push`; a steed's armour
/// spares it a part. Returns whether it dies.
pub fn take_blow(mob: &mut ServerMob, damage: u32, push: Vec3) -> bool {
    let points = mob.horse_armour.and_then(|armour| armour.horse_armour());
    let damage = points.map_or(damage, |points| armoured_damage(damage, points));
    mob.damage += damage;
    mob.hurt_ticks = super::combat::MOB_INVULNERABILITY_TICKS;
    mob.struck = None;
    mob.velocity = push.with_y(0.0) + Vec3::Y * BITE_LIFT;
    mob.on_ground = false;
    mob.damage >= mob.max_health()
}

/// What a mob killed by a wolf leaves: its loot and its experience where it was. The owner of
/// the wolf, if there, is credited with it.
fn fall(
    mob: ServerMob,
    owner: Option<&mut Player>,
    items: &mut DroppedItems,
    rng: &mut impl Rng,
) {
    if let Some(owner) = owner {
        owner.beat(mob.kind);
    }
    super::combat::drop_loot(items, super::combat::loot(&mob, rng), mob.position);
    items.drop_orb(mob.experience(), mob.position, rng);
}

/// Lands the bites of the step: each prey not hurt a moment ago takes `WOLF_BITE` and is
/// thrown back; an animal flees from the wolf, a hostile that strikes in reach strikes the
/// wolf back. A mob killed is taken out of the world with its loot (`fall`).
pub fn apply_bites(
    bites: Vec<(MobId, MobId)>,
    mobs: &mut HashMap<MobId, ServerMob>,
    changed: &mut HashSet<MobId>,
    players: &mut HashMap<PlayerId, Player>,
    items: &mut DroppedItems,
    rng: &mut impl Rng,
) {
    for (wolf_id, prey_id) in bites {
        let Some(wolf) = mobs.get(&wolf_id) else {
            continue;
        };
        let (wolf_at, wolf_size, owner) = (
            wolf.position,
            Vec3::new(wolf.width, wolf.height, wolf.depth),
            wolf.owner.clone(),
        );
        let Some(prey) = mobs.get_mut(&prey_id) else {
            continue;
        };
        if prey.hurt_ticks > 0 {
            continue;
        }
        let away = (prey.position - wolf_at).with_y(0.0).normalize_or_zero();
        let dies = take_blow(prey, WOLF_BITE, away * BITE_PUSH);
        changed.insert(prey_id);
        if prey.kind.is_animal() {
            prey.action = MobAction::Flee;
            prey.action_ticks = super::combat::FLEE_TICKS;
            prey.target = MobTarget::Position(wolf_at);
        }
        // The hostile strikes back, if the wolf is in its reach
        let mut back = 0;
        if let (false, Some(strike)) = (dies, prey.kind.strike()) {
            let reach = distance_to_box(prey.position, wolf_at, wolf_size / 2.0);
            if prey.kind.is_hostile() && prey.attack_wait == 0 && reach <= strike.reach.max(1.0) {
                back = strike.damage;
                prey.attack_wait = strike.cooldown.max(10);
                prey.action = MobAction::Attack;
                prey.action_ticks = BITE_SHOW_TICKS;
            }
        }
        if dies {
            let dead = mobs.remove(&prey_id).expect("the prey bitten");
            changed.remove(&prey_id);
            let owner = owner
                .as_deref()
                .and_then(|name| players.values_mut().find(|player| player.name == name));
            fall(dead, owner, items, rng);
            if let Some(wolf) = mobs.get_mut(&wolf_id) {
                wolf.prey = None;
            }
            continue;
        }
        if back > 0 {
            let Some(wolf) = mobs.get_mut(&wolf_id) else {
                continue;
            };
            if take_blow(wolf, back, -away * BITE_PUSH) {
                mobs.remove(&wolf_id);
                changed.remove(&wolf_id);
            } else {
                changed.insert(wolf_id);
            }
        }
    }
}

/// The steeds take the blows of the hostiles their riders were spared (`Player::mount_blows`),
/// lessened by their armour; one killed so is taken out of the world with its loot, which
/// throws its rider off.
pub fn bear_mount_blows(
    players: &mut HashMap<PlayerId, Player>,
    mobs: &mut HashMap<MobId, ServerMob>,
    changed: &mut HashSet<MobId>,
    items: &mut DroppedItems,
    rng: &mut impl Rng,
) {
    for player in players.values_mut() {
        let blows = std::mem::take(&mut player.mount_blows);
        let Some(riding) = player.riding.filter(|_| blows > 0) else {
            continue;
        };
        let Some(steed) = mobs.get_mut(&riding.mob).filter(|mob| mob.kind.is_steed()) else {
            continue;
        };
        let points = steed.horse_armour.and_then(|armour| armour.horse_armour());
        steed.damage += points.map_or(blows, |points| armoured_damage(blows, points));
        steed.hurt_ticks = super::combat::MOB_INVULNERABILITY_TICKS;
        changed.insert(riding.mob);
        if steed.damage >= steed.max_health() {
            let dead = mobs.remove(&riding.mob).expect("the steed");
            changed.remove(&riding.mob);
            super::combat::drop_loot(items, super::combat::loot(&dead, rng), dead.position);
        }
    }
}

/// A right click of its owner on a tamed wolf or cat: it sits, or stands up to follow again.
/// A wolf that sits forgets its prey.
pub fn toggle_sit(mob: &mut ServerMob) {
    mob.sitting = !mob.sitting;
    mob.prey = None;
    (mob.action, mob.target, mob.action_ticks) = (MobAction::Idle, MobTarget::None, 1);
    mob.velocity.x = 0.0;
    mob.velocity.z = 0.0;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tamed(kind: MobKind, at: Vec3) -> ServerMob {
        ServerMob {
            owner: Some("Ann".into()),
            ..ServerMob::new(kind, at)
        }
    }

    #[test]
    fn the_wolves_of_a_player_hunt_what_it_strikes_but_not_a_tamed_animal() {
        let mut mobs = HashMap::new();
        mobs.insert(1, tamed(MobKind::Wolf, Vec3::ZERO));
        let sitting = ServerMob {
            sitting: true,
            ..tamed(MobKind::Wolf, Vec3::X)
        };
        mobs.insert(2, sitting);
        mobs.insert(3, ServerMob::new(MobKind::Gnawer, Vec3::Z * 5.0));
        mobs.insert(4, tamed(MobKind::Cat, Vec3::Z * 3.0));
        rally(&mut mobs, "Ann", 4);
        assert_eq!(mobs[&1].prey, None);
        rally(&mut mobs, "Bob", 3);
        assert_eq!(mobs[&1].prey, None);
        rally(&mut mobs, "Ann", 3);
        assert_eq!(mobs[&1].prey, Some(3));
        assert_eq!(mobs[&2].prey, None, "a wolf that sits stays");
    }

    #[test]
    fn a_wolf_guards_its_owner_from_a_hostile_that_hunts_it() {
        let mut players = HashMap::new();
        let mut ann = Player::default();
        ann.id = 7;
        ann.name = "Ann".into();
        players.insert(7, ann);
        let mut mobs = HashMap::new();
        mobs.insert(1, tamed(MobKind::Wolf, Vec3::X));
        let mut gnawer = ServerMob::new(MobKind::Gnawer, Vec3::Z * 6.0);
        gnawer.target = MobTarget::Player(8);
        mobs.insert(2, gnawer);
        guard(&mut mobs, &players);
        assert_eq!(mobs[&1].prey, None, "it hunts another player");
        mobs.get_mut(&2).unwrap().target = MobTarget::Player(7);
        guard(&mut mobs, &players);
        assert_eq!(mobs[&1].prey, Some(2));
    }

    #[test]
    fn the_rats_and_the_owls_fear_a_cat_near_them() {
        assert!(fears_cats(MobKind::Gnawer) && fears_cats(MobKind::GustOwl));
        assert!(!fears_cats(MobKind::Stumpling));
        let cats = [Vec3::new(0.0, 0.0, 0.0)];
        assert!(nearest_cat(&cats, Vec3::new(5.0, 0.0, 0.0), false).is_some());
        assert!(nearest_cat(&cats, Vec3::new(5.0, 9.0, 0.0), false).is_none());
        assert!(nearest_cat(&cats, Vec3::new(5.0, 9.0, 0.0), true).is_some());
        assert!(nearest_cat(&cats, Vec3::new(20.0, 0.0, 0.0), true).is_none());
    }

    #[test]
    fn horse_armour_spares_a_steed_a_part_of_its_blows() {
        assert_eq!(armoured_damage(10, 0), 10);
        assert_eq!(armoured_damage(10, 11), 6);
        assert_eq!(armoured_damage(1, 11), 1);
        assert_eq!(armoured_damage(0, 3), 0);
        let mut horse = ServerMob::new(MobKind::Horse, Vec3::ZERO);
        horse.horse_armour = Some(shared::world::ItemId::DiamondHorseArmour);
        take_blow(&mut horse, 10, Vec3::ZERO);
        assert_eq!(horse.damage, 6);
    }
}
