pub mod arrow;
pub mod behavior;
pub mod boss;
pub mod breeding;
pub mod cinder;
pub mod combat;
pub mod fishing;
mod float;
pub mod guards;
pub mod hostile;
pub mod path;
pub mod riding;
pub mod spawning;
mod swim;
pub mod villager;
pub mod wildlife;

use std::collections::{HashMap, HashSet};

use shared::world::{MobId, ServerMob};
use ulid::Ulid;

fn create_new_mob_id() -> u128 {
    Ulid::new().0
}

/// Adds `mob` to the world under a new id, which it returns. The clients that have it within
/// reach are sent it with the next broadcast.
pub(crate) fn add_mob(
    mobs: &mut HashMap<MobId, ServerMob>,
    changed_mobs: &mut HashSet<MobId>,
    mut mob: ServerMob,
) -> MobId {
    // Each horse is born with its coat and its gifts
    if mob.kind == shared::world::MobKind::Horse && mob.horse.is_none() {
        mob.horse = Some(shared::world::HorseTraits::roll(&mut rand::thread_rng()));
    }
    let id = create_new_mob_id();
    mobs.insert(id, mob);
    changed_mobs.insert(id);
    id
}
