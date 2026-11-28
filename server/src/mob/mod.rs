pub mod arrow;
pub mod behavior;
pub mod boss;
pub mod breeding;
pub mod cinder;
pub mod combat;
pub mod companions;
pub mod fishing;
mod float;
pub mod guards;
pub mod hostile;
pub mod leash;
pub mod pack;
pub mod path;
pub mod riding;
pub mod spawning;
mod swim;
pub mod villager;
pub mod wildlife;

use std::collections::{HashMap, HashSet};

use bevy::math::bounding::Aabb3d;
use bevy::prelude::*;
use shared::world::{global_block_to_chunk_pos, ChunkMap, MobId, ServerMob};
use ulid::Ulid;

use crate::init::ServerTime;
use crate::world::data::ServerWorldMap;

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
    // Each steed is born with its coat and its gifts
    if mob.kind.is_steed() && mob.horse.is_none() {
        mob.horse = Some(shared::world::HorseTraits::of_kind(
            mob.kind,
            &mut rand::thread_rng(),
        ));
    }
    let id = create_new_mob_id();
    mobs.insert(id, mob);
    changed_mobs.insert(id);
    id
}

/// Ticks between two fits of the boxes of the mobs to their kinds (`fit_boxes_system`).
const FIT_INTERVAL_TICKS: u64 = 20;

/// Gives each mob the box of its kind (`ServerMob::fit_kind_box`) where it stands, once that box
/// is free of blocks in loaded chunks: after a reload of the content changed a size, or after
/// a world saved with an older one was loaded. A mob whose new box does not fit keeps its own,
/// and is tried again. The projectiles keep theirs.
pub fn fit_boxes_system(mut world_map: ResMut<ServerWorldMap>, time: Res<ServerTime>) {
    if !time.0.is_multiple_of(FIT_INTERVAL_TICKS) {
        return;
    }
    let ServerWorldMap {
        chunks,
        mobs,
        changed_mobs,
        ..
    } = world_map.as_mut();
    for (id, mob) in mobs.iter_mut() {
        if !mob.kind.is_projectile()
            && mob.fit_kind_box(|centre, half| is_free(chunks, centre, half))
        {
            changed_mobs.insert(*id);
        }
    }
}

/// Whether the box of `centre` and `half` lies in loaded chunks and overlaps no block.
fn is_free(chunks: &ChunkMap, centre: Vec3, half: Vec3) -> bool {
    [centre - half, centre + half]
        .iter()
        .all(|corner| chunks.is_chunk_loaded(global_block_to_chunk_pos(&corner.floor().as_ivec3())))
        && !chunks.check_collision_box(&Aabb3d::new(centre, half))
}
