//! The mounts the players ride (`shared::players::riding`): a player gets on with the right
//! button, and its moves carry the mount, which the server puts where the rider's simulation
//! says; the passenger of a boat is carried along with it.

use bevy::prelude::*;
use rand::Rng;
use shared::{
    players::riding::{dismount, seats, Riding},
    world::{HorseTraits, MobAction, MobId, MobKind, ServerMob},
};

use crate::world::data::ServerWorldMap;

/// The seat of `mob` a player named `name` may take, if any: a boat's first free one; the only
/// one of a horse grown up, saddled and tamed by that player.
pub fn free_seat(mob: &ServerMob, name: &str) -> Option<u8> {
    match mob.kind {
        MobKind::Horse => (!mob.is_young()
            && mob.saddled
            && mob.owner.as_deref() == Some(name)
            && mob.riders[0].is_none())
        .then_some(0),
        MobKind::Boat => {
            (0..seats(MobKind::Boat)).find(|&seat| mob.riders[seat as usize].is_none())
        }
        _ => None,
    }
}

/// The heading of `mob`, as `Riding::heading`.
fn heading_of(mob: &ServerMob) -> f32 {
    let ahead = mob.rotation * Vec3::Z;
    ahead.x.atan2(ahead.z)
}

/// Puts `player` on the free seat of `mob`, whose id is `id`, if it has one.
pub fn mount(
    player: &mut shared::players::Player,
    id: MobId,
    mob: &mut ServerMob,
    rng: &mut impl Rng,
) {
    let Some(seat) = free_seat(mob, &player.name) else {
        return;
    };
    let heading = heading_of(mob);
    let riding = match mob.kind {
        MobKind::Horse => {
            let traits = *mob.horse.get_or_insert_with(|| HorseTraits::roll(rng));
            Riding::horse(id, &traits, heading)
        }
        _ => Riding::boat(id, seat, heading),
    };
    mob.riders[seat as usize] = Some(player.id);
    player.riding = Some(riding);
    player.position = mob.position + riding.seat_offset();
    player.velocity = Vec3::ZERO;
    player.fall_distance = 0.0;
    player.on_ground = mob.on_ground;
    player.is_sneaking = false;
    player.is_sprinting = false;
    player.is_flying = false;
}

/// What a ridden mob looks like it does, going at `speed` blocks/s: a horse walks, or runs when
/// it gallops; a boat stays still.
fn ridden_action(riding: &Riding, speed: f32) -> MobAction {
    if riding.kind != MobKind::Horse || speed < 0.2 {
        MobAction::Idle
    } else if speed > riding.speed * 1.2 {
        MobAction::Flee
    } else {
        MobAction::Walk
    }
}

/// Once the inputs of the tick moved the players: the driver of each mount puts it where its
/// simulation says, each passenger is carried by its boat, and a rider whose mount is gone, or
/// taken, or who died, gets off. The seats of the mobs (`ServerMob::riders`) are those of the
/// riders.
pub fn riding_system(mut world_map: ResMut<ServerWorldMap>) {
    let ServerWorldMap {
        players,
        mobs,
        changed_mobs,
        chunks,
        ..
    } = world_map.as_mut();
    for mob in mobs.values_mut() {
        mob.riders = [None; 2];
    }
    // In the order of the ids, that the same rider keeps a seat two claim
    let mut ids: Vec<_> = players.keys().copied().collect();
    ids.sort_unstable();
    for player_id in ids {
        let player = players.get_mut(&player_id).expect("a player of the map");
        let Some(mut riding) = player.riding else {
            continue;
        };
        let seat = riding.seat as usize;
        let mob = mobs.get_mut(&riding.mob).filter(|mob| {
            mob.kind == riding.kind && seat < mob.riders.len() && mob.riders[seat].is_none()
        });
        let Some(mob) = mob.filter(|_| !player.is_dead()) else {
            dismount(player, chunks);
            continue;
        };
        mob.riders[seat] = Some(player_id);
        if seat == 0 {
            let before = (mob.position, mob.rotation, mob.action);
            mob.position = player.position - riding.seat_offset();
            mob.rotation = Quat::from_rotation_y(riding.heading);
            mob.velocity = player.velocity;
            mob.on_ground = player.on_ground;
            mob.action = ridden_action(&riding, player.velocity.with_y(0.0).length());
            mob.target = shared::world::MobTarget::None;
            if before != (mob.position, mob.rotation, mob.action) {
                changed_mobs.insert(riding.mob);
            }
        } else {
            // The passenger is carried, facing where the boat goes
            riding.heading = heading_of(mob);
            player.riding = Some(riding);
            player.position = mob.position + riding.seat_offset();
            player.velocity = Vec3::ZERO;
            player.fall_distance = 0.0;
            player.on_ground = mob.on_ground;
        }
    }
}

/// Whether a player drives `mob`: the mob's behaviour leaves it to its rider.
pub fn is_driven(mob: &ServerMob) -> bool {
    mob.riders[0].is_some()
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::players::riding::forward;

    #[test]
    fn a_horse_takes_its_owner_once_saddled_and_a_boat_two_riders() {
        let mut horse = ServerMob::new(MobKind::Horse, Vec3::ZERO);
        horse.owner = Some("Ann".into());
        assert_eq!(free_seat(&horse, "Ann"), None);
        horse.saddled = true;
        assert_eq!(free_seat(&horse, "Ann"), Some(0));
        assert_eq!(free_seat(&horse, "Bob"), None);
        horse.riders[0] = Some(1);
        assert_eq!(free_seat(&horse, "Ann"), None);

        let mut boat = ServerMob::new(MobKind::Boat, Vec3::ZERO);
        assert_eq!(free_seat(&boat, "Bob"), Some(0));
        boat.riders[0] = Some(1);
        assert_eq!(free_seat(&boat, "Bob"), Some(1));
        boat.riders[1] = Some(2);
        assert_eq!(free_seat(&boat, "Bob"), None);
    }

    #[test]
    fn a_mob_turned_to_a_heading_gives_it_back() {
        let mut mob = ServerMob::new(MobKind::Boat, Vec3::ZERO);
        mob.rotation = Quat::from_rotation_y(1.0);
        assert!((heading_of(&mob) - 1.0).abs() < 1e-5);
        assert!((mob.rotation * Vec3::Z - forward(1.0)).length() < 1e-5);
    }
}
