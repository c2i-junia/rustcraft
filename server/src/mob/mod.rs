pub mod behavior;

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
    mob: ServerMob,
) -> MobId {
    let id = create_new_mob_id();
    mobs.insert(id, mob);
    changed_mobs.insert(id);
    id
}
