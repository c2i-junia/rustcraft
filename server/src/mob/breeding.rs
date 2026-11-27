//! The animals' lives beyond the day: a player feeds them their food (`MobKind::eats`), two fed
//! adults of a kind near each other give birth to a young one, which grows up in
//! `GROW_TICKS`; and a wolf or a cat fed its meat may be tamed, after which it follows its
//! owner (`follow_owner`) and is the only kind of its kind that breeds.

use std::collections::{HashMap, HashSet};

use bevy::math::Vec3;
use rand::Rng;
use shared::{
    messages::PlayerId,
    players::{progress::Achievement, Player},
    world::{
        HorseTraits, ItemId, MobAction, MobId, MobKind, MobTarget, ServerMob, BREED_WAIT_TICKS,
        FED_YOUNG_SPEED_UP, GROW_TICKS, LOVE_TICKS, TAME_CHANCE,
    },
};

/// How far an animal that wants a mate looks for one, in blocks.
const MATE_SEARCH: f32 = 8.0;
/// The living player nearest to a birth, at most this far, is who had the two breed, in blocks.
const BREEDER_DISTANCE: f32 = 16.0;
/// Two animals that want a mate this close to each other give birth, in blocks.
const MATE_DISTANCE: f32 = 1.5;
/// A tamed animal farther than this from its owner walks to it, in blocks.
pub const FOLLOW_START: f32 = 6.0;
/// A tamed animal following its owner stops this close to it, in blocks.
pub const FOLLOW_STOP: f32 = 2.5;
/// A tamed animal farther than this from its owner, who stands on the ground, is brought next
/// to it, in blocks: it would never catch up.
pub const FOLLOW_TELEPORT: f32 = 24.0;

/// How long a young animal with no room to grow up waits before it tries again, in ticks.
const GROW_RETRY_TICKS: u32 = 20;
/// How far it moves to find room at most, against what its box grows by on a side, and in how many
/// steps along each axis.
const GROW_REACH: f32 = 1.1;
const GROW_STEPS: i32 = 6;

/// The points of health a steed or a tamed wolf gets back by eating `item`: a roast more than
/// raw meat, a carrot or an apple more than rye.
fn heal(item: ItemId) -> u32 {
    match item {
        ItemId::Carrot | ItemId::Apple => 4,
        ItemId::RoastMutton
        | ItemId::RoastPork
        | ItemId::RoastBeef
        | ItemId::RoastChicken
        | ItemId::RoastRabbit => 6,
        ItemId::RawMutton
        | ItemId::RawPork
        | ItemId::RawBeef
        | ItemId::RawChicken
        | ItemId::RawRabbit => 3,
        _ => 2,
    }
}

/// The kind of the young of `a` and `b`, if they may have one: theirs, or a mule for a horse
/// and a donkey. A mule has none.
pub fn young_kind(a: MobKind, b: MobKind) -> Option<MobKind> {
    match (a, b) {
        (MobKind::Mule, _) | (_, MobKind::Mule) => None,
        (MobKind::Horse, MobKind::Donkey) | (MobKind::Donkey, MobKind::Horse) => {
            Some(MobKind::Mule)
        }
        (a, b) => (a == b).then_some(a),
    }
}

/// What a player named `player` does by feeding `item` to `mob`. Returns whether the animal ate
/// it, which takes one from the player's hand:
/// - a wolf or a cat not tamed is tamed by `player` one time in three (`TAME_CHANCE`), and eats
///   either way;
/// - a young animal grows up a tenth of its time sooner;
/// - an adult that may breed (a tamed one, for the wolf and the cat) wants a mate
///   (`LOVE_TICKS`); one that already does, or gave birth not long ago, does not eat.
pub fn feed(mob: &mut ServerMob, player: &str, item: ItemId, rng: &mut impl Rng) -> bool {
    if !mob.kind.eats(item) {
        return false;
    }
    if mob.kind.is_tameable() && mob.owner.is_none() {
        if rng.gen_bool(TAME_CHANCE) {
            mob.owner = Some(player.to_string());
            // It no longer flees from whoever struck it
            if mob.action == MobAction::Flee {
                (mob.action, mob.target, mob.action_ticks) = (MobAction::Idle, MobTarget::None, 1);
            }
        }
        return true;
    }
    // A tamed steed or wolf that was hurt eats to heal
    if (mob.kind.is_steed() || mob.kind == MobKind::Wolf) && mob.damage > 0 {
        mob.damage = mob.damage.saturating_sub(heal(item));
        return true;
    }
    if mob.is_young() {
        let spared = (GROW_TICKS as f32 * FED_YOUNG_SPEED_UP) as u32;
        mob.growing = mob.growing.saturating_sub(spared).max(1);
        return true;
    }
    // A mule never breeds
    if mob.in_love > 0 || mob.breed_wait > 0 || mob.kind == MobKind::Mule {
        return false;
    }
    mob.in_love = LOVE_TICKS;
    true
}

/// `young` as an adult, moved off the walls its bigger box would be in, if it finds room there: in
/// one, it would never move again.
fn grown_up(young: &ServerMob, free: &impl Fn(&ServerMob, Vec3) -> bool) -> Option<ServerMob> {
    let mut adult = young.clone();
    adult.grow_up();
    // Its box grows by this much on each side, up to which it may have touched a wall
    let grows = (adult.width - young.width) / 2.0;
    let mut offsets: Vec<Vec3> = (-GROW_STEPS..=GROW_STEPS)
        .flat_map(|x| (-GROW_STEPS..=GROW_STEPS).map(move |z| Vec3::new(x as f32, 0.0, z as f32)))
        .collect();
    offsets.sort_by(|a, b| a.length_squared().total_cmp(&b.length_squared()));
    let spot = offsets
        .into_iter()
        .map(|offset| adult.position + offset * (grows * GROW_REACH / GROW_STEPS as f32))
        .find(|&spot| free(&adult, spot))?;
    adult.position = spot;
    Some(adult)
}

/// The players who had the animals breed that gave birth at `births` earn it.
pub fn credit_births(births: Vec<Vec3>, players: &mut HashMap<PlayerId, Player>) {
    for birth in births {
        let breeder = players
            .values_mut()
            .filter(|player| !player.is_dead())
            .map(|player| (player.position.distance(birth), player))
            .filter(|(distance, _)| *distance <= BREEDER_DISTANCE)
            .min_by(|a, b| a.0.total_cmp(&b.0));
        if let Some((_, player)) = breeder {
            player.earn(Achievement::Animal);
        }
    }
}

/// Whether `a` and `b` may have a young one together: of the same kind, both adults wanting a
/// mate, and of the same owner when tamed.
fn are_mates(a: &ServerMob, b: &ServerMob) -> bool {
    young_kind(a.kind, b.kind).is_some()
        && a.in_love > 0
        && b.in_love > 0
        && !a.is_young()
        && !b.is_young()
        && (!a.kind.is_tameable() || a.owner.is_some() && a.owner == b.owner)
}

/// One tick in this many, an adult hen lays an egg: every 5 minutes or so.
pub const EGG_ODDS: u32 = 5 * 60 * shared::TICKS_PER_SECOND as u32;

/// Where the adult hens in loaded terrain lay an egg in this tick: under them.
pub fn lay_eggs(
    mobs: &HashMap<MobId, ServerMob>,
    loaded: impl Fn(Vec3) -> bool,
    rng: &mut impl Rng,
) -> Vec<Vec3> {
    mobs.values()
        .filter(|mob| mob.kind == MobKind::Chicken && !mob.is_young())
        .filter(|mob| loaded(mob.position) && rng.gen_ratio(1, EGG_ODDS))
        .map(|mob| mob.position - Vec3::Y * (mob.height / 2.0 - 0.15))
        .collect()
}

/// One step of the animals' lives, after their moves: the young grow, the wish for a mate
/// fades, and two animals that want a mate walk to each other and give birth once close
/// (`MATE_DISTANCE`). A young one born of a tamed pair belongs to their owner. The mobs
/// changed are added to `changed`, the young ones born to the mobs. `free` tells whether a mob has
/// room at a position. Returns where they were born.
pub fn breed(
    mobs: &mut HashMap<MobId, ServerMob>,
    changed: &mut HashSet<MobId>,
    loaded: impl Fn(Vec3) -> bool,
    free: impl Fn(&ServerMob, Vec3) -> bool,
) -> Vec<Vec3> {
    let mut wanting = Vec::new();
    for (id, mob) in mobs.iter_mut() {
        if !mob.kind.is_animal() || !loaded(mob.position) {
            continue;
        }
        mob.breed_wait = mob.breed_wait.saturating_sub(1);
        if mob.growing > 0 {
            mob.growing -= 1;
            if mob.growing == 0 {
                match grown_up(mob, &free) {
                    Some(adult) => {
                        *mob = adult;
                        changed.insert(*id);
                    }
                    None => mob.growing = GROW_RETRY_TICKS,
                }
            }
        }
        if mob.in_love > 0 {
            mob.in_love -= 1;
            if mob.in_love == 0 {
                changed.insert(*id);
            } else {
                wanting.push(*id);
            }
        }
    }
    // Sorted, so that the same mobs pair up whatever the order of the map
    wanting.sort_unstable();
    let mut paired = HashSet::new();
    let mut born = Vec::new();
    for &id in &wanting {
        if paired.contains(&id) {
            continue;
        }
        let mob = &mobs[&id];
        let mate = wanting
            .iter()
            .filter(|&&other| other != id && !paired.contains(&other))
            .filter(|other| are_mates(mob, &mobs[other]))
            .map(|&other| (other, mobs[&other].position.distance(mob.position)))
            .filter(|&(_, distance)| distance <= MATE_SEARCH)
            .min_by(|a, b| a.1.total_cmp(&b.1).then(a.0.cmp(&b.0)));
        let Some((mate, distance)) = mate else {
            continue;
        };
        paired.insert(id);
        paired.insert(mate);
        let (here, there) = (mobs[&id].position, mobs[&mate].position);
        if distance <= MATE_DISTANCE {
            let kind = young_kind(mob.kind, mobs[&mate].kind).unwrap_or(mob.kind);
            let mut young = ServerMob::young(kind, (here + there) / 2.0);
            young.owner = mob.owner.clone();
            // A foal takes after its parents
            if let (Some(a), Some(b)) = (mob.horse, mobs[&mate].horse) {
                young.horse = Some(HorseTraits::inherit(&a, &b, &mut rand::thread_rng()));
            }
            young.rotation = mob.rotation;
            // Standing where its parents' feet are
            young.position.y = here.y - mob.height / 2.0 + young.height / 2.0;
            // Not in a wall or a fence between them
            if !free(&young, young.position) {
                young.position = here.with_y(young.position.y);
            }
            born.push(young);
            for parent in [id, mate] {
                let parent_mob = mobs.get_mut(&parent).expect("a mob wanting a mate");
                parent_mob.in_love = 0;
                parent_mob.breed_wait = BREED_WAIT_TICKS;
                changed.insert(parent);
            }
        } else {
            for (walker, to) in [(id, there), (mate, here)] {
                let walker_mob = mobs.get_mut(&walker).expect("a mob wanting a mate");
                // One hit flees, and one already near enough waits
                if matches!(
                    walker_mob.action,
                    MobAction::Idle | MobAction::Graze | MobAction::Walk
                ) {
                    walker_mob.action = MobAction::Walk;
                    walker_mob.target = MobTarget::Position(to);
                    walker_mob.action_ticks = walker_mob.action_ticks.max(2);
                }
            }
        }
    }
    let places = born.iter().map(|young| young.position).collect();
    for young in born {
        super::add_mob(mobs, changed, young);
    }
    places
}

/// The player named `name`, alive, if it is there.
pub fn owner_of<'a>(players: &'a HashMap<PlayerId, Player>, mob: &ServerMob) -> Option<&'a Player> {
    let name = mob.owner.as_deref()?;
    players
        .values()
        .find(|player| player.name == name && !player.is_dead())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_horse_and_a_donkey_have_a_mule_and_a_mule_has_none() {
        assert_eq!(
            young_kind(MobKind::Horse, MobKind::Donkey),
            Some(MobKind::Mule)
        );
        assert_eq!(
            young_kind(MobKind::Donkey, MobKind::Horse),
            Some(MobKind::Mule)
        );
        assert_eq!(
            young_kind(MobKind::Donkey, MobKind::Donkey),
            Some(MobKind::Donkey)
        );
        assert_eq!(young_kind(MobKind::Mule, MobKind::Mule), None);
        assert_eq!(young_kind(MobKind::Cow, MobKind::Horse), None);
        let mut wolf = ServerMob::new(MobKind::Wolf, Vec3::ZERO);
        wolf.owner = Some("Ann".into());
        wolf.damage = 5;
        let mut rng = rand::thread_rng();
        assert!(feed(&mut wolf, "Ann", ItemId::RoastBeef, &mut rng));
        assert_eq!(wolf.damage, 0);
    }
    use rand::{rngs::StdRng, SeedableRng};
    use shared::world::{MobKind, YOUNG_SCALE};

    fn cow_at(x: f32) -> ServerMob {
        ServerMob::new(MobKind::Cow, Vec3::new(x, 0.7, 0.0))
    }

    /// `ticks` steps of `breed`, every chunk loaded.
    fn run(mobs: &mut HashMap<MobId, ServerMob>, ticks: u32) -> HashSet<MobId> {
        let mut changed = HashSet::new();
        for _ in 0..ticks {
            breed(mobs, &mut changed, |_| true, |_, _| true);
        }
        changed
    }

    #[test]
    fn an_animal_eats_only_its_food_and_not_while_it_already_wants_a_mate() {
        let mut rng = StdRng::seed_from_u64(1);
        let mut cow = cow_at(0.0);
        assert!(!feed(&mut cow, "Ann", ItemId::Carrot, &mut rng));
        assert!(feed(&mut cow, "Ann", ItemId::Rye, &mut rng));
        assert_eq!(cow.in_love, LOVE_TICKS);
        assert!(!feed(&mut cow, "Ann", ItemId::Rye, &mut rng));
        let mut pig = ServerMob::new(MobKind::Pig, Vec3::ZERO);
        assert!(feed(&mut pig, "Ann", ItemId::Potato, &mut rng));
        let mut gnawer = ServerMob::new(MobKind::Gnawer, Vec3::ZERO);
        assert!(!feed(&mut gnawer, "Ann", ItemId::RawBeef, &mut rng));
    }

    #[test]
    fn two_fed_cows_walk_to_each_other_and_give_birth_to_a_young_one() {
        let mut rng = StdRng::seed_from_u64(2);
        let mut mobs = HashMap::from([(1, cow_at(0.0)), (2, cow_at(5.0))]);
        for cow in mobs.values_mut() {
            assert!(feed(cow, "Ann", ItemId::Rye, &mut rng));
        }
        run(&mut mobs, 1);
        assert!(matches!(mobs[&1].target, MobTarget::Position(p) if p.x == 5.0));
        assert!(matches!(mobs[&2].target, MobTarget::Position(p) if p.x == 0.0));
        assert_eq!(mobs[&1].action, MobAction::Walk);
        // Once close
        mobs.get_mut(&2).unwrap().position.x = 1.0;
        run(&mut mobs, 1);
        assert_eq!(mobs.len(), 3);
        let young = mobs.values().find(|mob| mob.is_young()).unwrap();
        assert_eq!(young.kind, MobKind::Cow);
        assert_eq!(young.height, MobKind::Cow.size().y * YOUNG_SCALE);
        assert!((young.position.x - 0.5).abs() < 1e-6);
        // Its feet where its parents' are
        let feet = |mob: &ServerMob| mob.position.y - mob.height / 2.0;
        assert!((feet(young) - feet(&mobs[&1])).abs() < 1e-6);
        for parent in [1, 2] {
            assert_eq!(mobs[&parent].in_love, 0);
            assert_eq!(mobs[&parent].breed_wait, BREED_WAIT_TICKS);
        }
        // Not again before a while
        assert!(!feed(
            mobs.get_mut(&1).unwrap(),
            "Ann",
            ItemId::Rye,
            &mut rng
        ));
    }

    #[test]
    fn animals_of_two_kinds_or_far_apart_have_no_young() {
        let mut rng = StdRng::seed_from_u64(3);
        let mut sheep = ServerMob::new(MobKind::Sheep, Vec3::new(1.0, 0.7, 0.0));
        let mut cow = cow_at(0.0);
        feed(&mut sheep, "Ann", ItemId::Rye, &mut rng);
        feed(&mut cow, "Ann", ItemId::Rye, &mut rng);
        let mut far = cow_at(MATE_SEARCH + 1.0);
        feed(&mut far, "Ann", ItemId::Rye, &mut rng);
        let mut mobs = HashMap::from([(1, sheep), (2, cow), (3, far)]);
        run(&mut mobs, LOVE_TICKS);
        assert_eq!(mobs.len(), 3);
        assert!(mobs.values().all(|mob| mob.in_love == 0));
    }

    #[test]
    fn a_young_one_grows_up_standing_where_it_stood_and_sooner_when_fed() {
        let mut rng = StdRng::seed_from_u64(4);
        let mut young = ServerMob::young(MobKind::Sheep, Vec3::new(0.0, 10.0, 0.0));
        let feet = young.position.y - young.height / 2.0;
        assert!(feed(&mut young, "Ann", ItemId::Rye, &mut rng));
        assert_eq!(young.in_love, 0, "a young one does not breed");
        let left = young.growing;
        assert_eq!(left, GROW_TICKS - GROW_TICKS / 10);
        let mut mobs = HashMap::from([(1, young)]);
        run(&mut mobs, left - 1);
        assert!(mobs[&1].is_young());
        let changed = run(&mut mobs, 1);
        let sheep = &mobs[&1];
        assert!(!sheep.is_young());
        assert!(changed.contains(&1));
        assert_eq!(sheep.height, MobKind::Sheep.size().y);
        assert!((sheep.position.y - sheep.height / 2.0 - feet).abs() < 1e-5);
    }

    #[test]
    fn a_wolf_is_tamed_by_its_meat_sooner_or_later_and_only_tamed_wolves_breed() {
        let mut rng = StdRng::seed_from_u64(5);
        let mut wolf = ServerMob::new(MobKind::Wolf, Vec3::ZERO);
        assert!(!feed(&mut wolf, "Ann", ItemId::Rye, &mut rng));
        let mut meals = 0;
        while wolf.owner.is_none() {
            assert!(feed(&mut wolf, "Ann", ItemId::RawBeef, &mut rng));
            assert_eq!(wolf.in_love, 0, "an untamed wolf does not breed");
            meals += 1;
            assert!(meals < 30);
        }
        assert_eq!(wolf.owner.as_deref(), Some("Ann"));
        // Tamed, the same meat makes it want a mate
        assert!(feed(&mut wolf, "Bob", ItemId::RoastBeef, &mut rng));
        assert_eq!(wolf.in_love, LOVE_TICKS);
        assert_eq!(wolf.owner.as_deref(), Some("Ann"));
    }

    #[test]
    fn the_young_of_two_tamed_cats_belongs_to_their_owner_and_cats_of_two_owners_do_not_mate() {
        let cat = |x: f32, owner: &str| ServerMob {
            owner: Some(owner.into()),
            in_love: LOVE_TICKS,
            ..ServerMob::new(MobKind::Cat, Vec3::new(x, 0.35, 0.0))
        };
        let mut mobs = HashMap::from([(1, cat(0.0, "Ann")), (2, cat(1.0, "Bob"))]);
        run(&mut mobs, 1);
        assert_eq!(mobs.len(), 2);
        let mut mobs = HashMap::from([(1, cat(0.0, "Ann")), (2, cat(1.0, "Ann"))]);
        run(&mut mobs, 1);
        let young = mobs.values().find(|mob| mob.is_young()).unwrap();
        assert_eq!(young.owner.as_deref(), Some("Ann"));
    }
}
