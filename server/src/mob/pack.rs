//! The chests on the backs of the donkeys and the mules (`ServerMob::pack`). A chest put on a
//! tamed one (`combat::interact`) is opened, sneaking, as a chest is (`InventoryAction::OpenPack`):
//! while the server simulates the inputs of a player, it lends the pack that player opens or
//! has open, if its mob is the player's and within reach, to the map of the chunks
//! (`ChunkMap::packs`), and takes it back after (`lend`, `take_back`). It is saved with its
//! mob, and dropped where the mob dies.

use std::collections::{HashMap, HashSet};

use bevy::math::Vec3;
use shared::{
    messages::PlayerFrameInput,
    players::{inventory::InventoryAction, Player},
    world::{ChunkMap, DroppedItem, MobId, ServerMob},
};

use crate::world::dropped_items::DroppedItems;

/// How far from the centre of its mob a player uses a pack, in blocks.
pub const PACK_REACH: f32 = 5.0;

/// Whether `player` may use the pack of `mob`: its owner, within `PACK_REACH`.
pub fn may_open(player: &Player, mob: &ServerMob) -> bool {
    mob.pack.is_some()
        && mob.kind.carries_chest()
        && mob.owner.as_deref() == Some(player.name.as_str())
        && mob.position.distance(player.position) <= PACK_REACH
}

/// Lends the pack `player` opens with `input`, or has open, to `chunks`, if it may use it.
/// Returns the mob whose pack was lent, to `take_back`.
pub fn lend(
    player: &Player,
    input: &PlayerFrameInput,
    mobs: &HashMap<MobId, ServerMob>,
    chunks: &mut ChunkMap,
) -> Option<MobId> {
    let opened = input.inventory_actions.iter().rev().find_map(|action| match action {
        InventoryAction::OpenPack(id) => Some(*id),
        _ => None,
    });
    let id = opened.or(player.inventory.open_pack)?;
    let mob = mobs.get(&id).filter(|mob| may_open(player, mob))?;
    chunks.packs.insert(id, (**mob.pack.as_ref()?).clone());
    Some(id)
}

/// Takes the pack of `id` back from `chunks` into its mob, which changed if it holds something
/// else.
pub fn take_back(
    id: Option<MobId>,
    mobs: &mut HashMap<MobId, ServerMob>,
    changed: &mut HashSet<MobId>,
    chunks: &mut ChunkMap,
) {
    let Some(id) = id else {
        return;
    };
    let Some(contents) = chunks.packs.remove(&id) else {
        return;
    };
    if let Some(mob) = mobs.get_mut(&id) {
        if mob.pack.as_deref() != Some(&contents) {
            mob.pack = Some(Box::new(contents));
            changed.insert(id);
        }
    }
}

/// Drops what the pack of `mob`, dead, held where it was: the stacks as they are, enchanted or
/// worn. The chest itself is of its loot (`combat::loot`).
pub fn drop_pack(items: &mut DroppedItems, mob: &ServerMob) {
    let Some(pack) = &mob.pack else {
        return;
    };
    for stack in pack.stacks() {
        items.spawn(DroppedItem::new(stack, mob.position, Vec3::Y * 4.0));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::world::{Container, ItemId, ItemStack, MobKind};

    #[test]
    fn a_pack_is_lent_to_its_owner_near_it_and_taken_back_with_what_it_put_in() {
        let mut ann = Player::default();
        ann.name = "Ann".into();
        let mut donkey = ServerMob::new(MobKind::Donkey, Vec3::X * 2.0);
        donkey.owner = Some("Ann".into());
        donkey.pack = Some(Box::default());
        let mut mobs = HashMap::from([(9, donkey)]);
        let mut chunks = ChunkMap::default();
        let mut input = PlayerFrameInput::default();
        input.inventory_actions.push(InventoryAction::OpenPack(9));

        let mut bob = ann.clone();
        bob.name = "Bob".into();
        assert_eq!(lend(&bob, &input, &mobs, &mut chunks), None);
        let lent = lend(&ann, &input, &mobs, &mut chunks);
        assert_eq!(lent, Some(9));
        let mut held = Container::default();
        held.slots[0] = Some(ItemStack {
            item_id: ItemId::Stone,
            item_type: ItemId::Stone.get_default_type(),
            nb: 3,
        });
        chunks.packs.insert(9, held.clone());
        let mut changed = HashSet::new();
        take_back(lent, &mut mobs, &mut changed, &mut chunks);
        assert_eq!(mobs[&9].pack.as_deref(), Some(&held));
        assert!(changed.contains(&9));
        assert!(chunks.packs.is_empty());

        ann.position = Vec3::X * 20.0;
        assert_eq!(lend(&ann, &input, &mobs, &mut chunks), None, "out of reach");
    }
}
