//! The leads (`ServerMob::leash`). A right click with a lead ties an animal to the hand of the
//! player (`combat::interact`), which it follows; a right click on a fence post ties to it the
//! animals the player leads (`tie_to_post`), which then stay about it, as in a pen. A lead
//! pulled too far snaps, and one tied to a post that is gone falls (`leash_system`): either
//! way the lead is dropped where the animal is.

use std::collections::{HashMap, HashSet};

use bevy::prelude::*;
use shared::{
    messages::{NetworkAction, PlayerFrameInput, PlayerId},
    players::{blocks::aim, Player},
    world::{BlockId, ChunkMap, ItemId, Leash, MobAction, MobId, MobTarget, ServerMob},
};

use super::path::Pathing;
use crate::world::data::ServerWorldMap;

/// A leashed animal farther than this from what holds its lead is pulled toward it, in blocks.
pub const LEASH_SLACK: f32 = 4.0;
/// A lead pulled farther than this snaps, in blocks.
pub const LEASH_BREAK: f32 = 12.0;
/// How fast an animal pulled by its lead goes, in blocks/s.
const PULL_SPEED: f32 = 4.0;
/// How far from the player a fence post is tied to, in blocks.
const POST_REACH: f32 = 5.0;

/// Where the lead of the mob is held: the hands of a living player, or the top of a post.
pub fn holder(leash: &Leash, players: &HashMap<PlayerId, Player>) -> Option<Vec3> {
    match leash {
        Leash::Player(name) => players
            .values()
            .find(|player| &player.name == name && !player.is_dead())
            .map(|player| player.position),
        Leash::Post(cell) => Some(cell.as_vec3() + Vec3::new(0.5, 0.5, 0.5)),
    }
}

/// A leashed mob farther than `LEASH_SLACK` from what holds its lead walks back to it along the
/// way it finds. Returns whether it was pulled, which is all it did this step.
pub fn pull(
    mob: &mut ServerMob,
    players: &HashMap<PlayerId, Player>,
    chunks: &ChunkMap,
    pathing: &mut Pathing,
    delta: f32,
) -> bool {
    let Some(at) = mob.leash.as_ref().and_then(|leash| holder(leash, players)) else {
        return false;
    };
    if mob.position.with_y(0.0).distance(at.with_y(0.0)) <= LEASH_SLACK {
        return false;
    }
    mob.action = MobAction::Walk;
    mob.target = MobTarget::Position(at);
    mob.action_ticks = super::behavior::WALK_TICKS;
    super::hostile::chase_to(mob, chunks, (at, 1.0), PULL_SPEED, delta, pathing);
    true
}

/// Whether the lead of `mob` holds: its player there and near enough, its post still a fence.
fn holds(mob: &ServerMob, players: &HashMap<PlayerId, Player>, chunks: &ChunkMap) -> bool {
    let Some(leash) = &mob.leash else {
        return true;
    };
    if let Leash::Post(cell) = leash {
        let loaded = chunks.get_block(*cell).is_some()
            || !shared::players::movement::is_terrain_loaded(mob.position, chunks);
        let fence = chunks
            .get_block(*cell)
            .is_some_and(|block| block.id == BlockId::Fence);
        // Far from the players, the chunk of the post may be gone: the lead waits
        if !loaded {
            return true;
        }
        if !fence && chunks.get_block(*cell).is_some() {
            return false;
        }
    }
    holder(leash, players).is_some_and(|at| at.distance(mob.position) <= LEASH_BREAK)
}

/// The leads that no longer hold (`holds`) fall, each dropped where its animal is.
pub fn leash_system(mut world_map: ResMut<ServerWorldMap>) {
    let ServerWorldMap {
        mobs,
        players,
        chunks,
        changed_mobs,
        dropped_items,
        ..
    } = world_map.as_mut();
    for at in snap(mobs, players, chunks, changed_mobs) {
        super::combat::drop_loot(dropped_items, vec![(ItemId::Lead, 1)], at);
    }
}

/// Takes off the leads that no longer hold, and returns where each was: the lead is dropped
/// there. Only those of loaded animals are looked at.
fn snap(
    mobs: &mut HashMap<MobId, ServerMob>,
    players: &HashMap<PlayerId, Player>,
    chunks: &ChunkMap,
    changed: &mut HashSet<MobId>,
) -> Vec<Vec3> {
    let mut snapped = Vec::new();
    for (id, mob) in mobs.iter_mut() {
        if mob.leash.is_none() || !shared::players::movement::is_terrain_loaded(mob.position, chunks)
        {
            continue;
        }
        // A player who left takes the lead off: it waits for it on the ground
        if !holds(mob, players, chunks) {
            mob.leash = None;
            changed.insert(*id);
            snapped.push(mob.position);
        }
    }
    snapped
}

/// A right click of `player` on a fence post within reach ties to it the animals it leads.
/// Returns the input without the click when it tied one, as `combat::interact` does.
pub fn tie_to_post(
    player: &Player,
    input: &PlayerFrameInput,
    mobs: &mut HashMap<MobId, ServerMob>,
    changed: &mut HashSet<MobId>,
    chunks: &ChunkMap,
) -> Option<PlayerFrameInput> {
    if !input.is_pressed(NetworkAction::RightClick) || player.is_dead() {
        return None;
    }
    let led = Leash::Player(player.name.clone());
    if !mobs.values().any(|mob| mob.leash.as_ref() == Some(&led)) {
        return None;
    }
    let target = aim(
        chunks,
        &input.camera,
        &player.position,
        input.view_mode,
        None,
    )?;
    let post = target.position;
    let is_fence = chunks
        .get_block(post)
        .is_some_and(|block| block.id == BlockId::Fence);
    if !is_fence || (post.as_vec3() + 0.5).distance(player.position) > POST_REACH {
        return None;
    }
    for (id, mob) in mobs.iter_mut() {
        if mob.leash.as_ref() == Some(&led) {
            mob.leash = Some(Leash::Post(post));
            changed.insert(*id);
        }
    }
    let mut rest = input.clone();
    rest.inputs.remove(&NetworkAction::RightClick);
    Some(rest)
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::world::{BlockData, BlockDirection, MobKind};

    fn ann() -> HashMap<PlayerId, Player> {
        let mut ann = Player::default();
        ann.id = 1;
        ann.name = "Ann".into();
        HashMap::from([(1, ann)])
    }

    #[test]
    fn a_lead_pulled_too_far_snaps_and_one_held_close_holds() {
        let players = ann();
        let chunks = ChunkMap::default();
        let mut mobs = HashMap::new();
        let mut near = ServerMob::new(MobKind::Cow, Vec3::X * 3.0);
        near.leash = Some(Leash::Player("Ann".into()));
        let mut far = near.clone();
        far.position = Vec3::X * 20.0;
        assert!(holds(&near, &players, &chunks));
        assert!(!holds(&far, &players, &chunks));
        far.leash = Some(Leash::Player("Bob".into()));
        assert!(!holds(&far, &players, &chunks), "its player left");
        mobs.insert(1, near);
        assert_eq!(holder(mobs[&1].leash.as_ref().unwrap(), &players), Some(Vec3::ZERO));
    }

    #[test]
    fn a_lead_tied_to_a_post_falls_with_the_post() {
        let players = ann();
        let mut chunks = ChunkMap::with_loaded_region(IVec3::splat(-16), IVec3::splat(16));
        let post = IVec3::new(0, 0, 0);
        chunks.set_block(post, BlockData::new(BlockId::Fence, BlockDirection::Front));
        let mut cow = ServerMob::new(MobKind::Cow, Vec3::new(2.0, 0.5, 0.5));
        cow.leash = Some(Leash::Post(post));
        assert!(holds(&cow, &players, &chunks));
        chunks.set_block(post, BlockData::new(BlockId::Dirt, BlockDirection::Front));
        assert!(!holds(&cow, &players, &chunks));
    }
}
