//! The furniture on the server (`shared::world::decor`): what the item frames and the flower
//! pots show, and the patterns of the banners, kept with the signs in `places.ron` and sent to
//! every player when they change.
//!
//! A click on an empty frame puts one of the item held in it, and on a frame that shows one turns
//! it by an eighth. A click on an empty pot with a plant plants one, and on a pot that holds one
//! takes it back. A frame or a pot broken drops what it shows; a banner broken drops itself with
//! its patterns.

use bevy::prelude::*;
use bevy_renet::RenetServer;
use shared::messages::ServerToClientMessage;
use shared::players::blocks::BrokenBlock;
use shared::world::{
    global_block_to_chunk_pos, DecorKind, Decorations, DroppedItem, ItemStack, ItemType, Shown,
};

use super::data::ServerWorldMap;
use super::home::Places;
use crate::init::ServerLobby;
use crate::network::extensions::SendGameMessageExtension;

/// The eighths of a turn an item frame goes round in.
const FRAME_TURNS: u8 = 8;

/// The banner `broken` drops with its patterns, which leave the places, if it had some and was
/// harvested.
pub fn broken_banner(places: &mut Places, broken: &BrokenBlock) -> Option<DroppedItem> {
    let decor = broken
        .block
        .decor()
        .filter(|d| d.kind == DecorKind::Banner)?;
    let patterns = places.banners.remove(&broken.position)?;
    if !broken.harvested {
        return None;
    }
    let stack = ItemStack {
        item_id: decor.item,
        item_type: ItemType::Banner { patterns },
        nb: 1,
    };
    let centre = broken.position.as_vec3() + Vec3::splat(0.5);
    Some(DroppedItem::new(stack, centre, Vec3::Y * 2.0))
}

/// A stack of one plain `item`.
fn one(item: shared::world::ItemId) -> ItemStack {
    ItemStack {
        item_id: item,
        item_type: item.get_default_type(),
        nb: 1,
    }
}

/// Puts the items held in the frames and the pots clicked or takes the plants back, turns the
/// items framed, drops what the frames and pots broken showed, and sends the decorations to
/// whoever has not had them as they are.
pub fn decor_system(
    mut world_map: ResMut<ServerWorldMap>,
    mut server: ResMut<RenetServer>,
    lobby: Res<ServerLobby>,
) {
    let world_map = world_map.as_mut();
    let mut changed = std::mem::take(&mut world_map.home.decorations_changed);

    for (id, at) in std::mem::take(&mut world_map.home.displayed) {
        let kind = world_map
            .chunks
            .get_block(at)
            .and_then(|b| b.id.decor_kind());
        let Some(player) = world_map.players.get_mut(&id) else {
            continue;
        };
        let slot = player.hotbar_slot;
        let held = player.inventory.inner.get(&slot).copied();
        let displays = &mut world_map.places.displays;
        match kind {
            Some(DecorKind::ItemFrame) => {
                if let Some(shown) = displays.get_mut(&at) {
                    shown.turn = (shown.turn + 1) % FRAME_TURNS;
                    changed = true;
                } else if let Some(stack) =
                    held.filter(|stack| stack.item_type == stack.item_id.get_default_type())
                {
                    // only a plain item: a frame keeps no wear, enchantment or map
                    player.inventory.remove_item_from_stack(slot, 1);
                    displays.insert(
                        at,
                        Shown {
                            item: stack.item_id,
                            turn: 0,
                        },
                    );
                    changed = true;
                }
            }
            Some(DecorKind::FlowerPot) => {
                if let Some(shown) = displays.remove(&at) {
                    let left = player.inventory.add_item_to_inventory(one(shown.item));
                    if left > 0 {
                        let centre = at.as_vec3() + Vec3::new(0.5, 0.6, 0.5);
                        world_map.dropped_items.spawn(DroppedItem::new(
                            one(shown.item),
                            centre,
                            Vec3::Y * 2.0,
                        ));
                    }
                    changed = true;
                } else if let Some(stack) = held.filter(|stack| stack.item_id.fits_flower_pot()) {
                    player.inventory.remove_item_from_stack(slot, 1);
                    displays.insert(
                        at,
                        Shown {
                            item: stack.item_id,
                            turn: 0,
                        },
                    );
                    changed = true;
                }
            }
            _ => {}
        }
    }

    // A frame or a pot broken drops what it showed; a banner gone takes its patterns with it
    let chunks = &world_map.chunks;
    let gone_from = |at: IVec3, kinds: &[DecorKind]| {
        chunks.is_chunk_loaded(global_block_to_chunk_pos(&at))
            && !chunks
                .get_block(at)
                .and_then(|b| b.id.decor_kind())
                .is_some_and(|kind| kinds.contains(&kind))
    };
    let broken: Vec<IVec3> = world_map
        .places
        .displays
        .keys()
        .copied()
        .filter(|&at| gone_from(at, &[DecorKind::ItemFrame, DecorKind::FlowerPot]))
        .collect();
    let faded: Vec<IVec3> = world_map
        .places
        .banners
        .keys()
        .copied()
        .filter(|&at| gone_from(at, &[DecorKind::Banner]))
        .collect();
    for at in broken {
        if let Some(shown) = world_map.places.displays.remove(&at) {
            let centre = at.as_vec3() + Vec3::splat(0.5);
            world_map
                .dropped_items
                .spawn(DroppedItem::new(one(shown.item), centre, Vec3::Y * 2.0));
            changed = true;
        }
    }
    for at in faded {
        world_map.places.banners.remove(&at);
        changed = true;
    }

    let home = &mut world_map.home;
    if changed {
        world_map.places.save(world_map.saved.as_deref());
        home.sent_decorations.clear();
    }
    home.sent_decorations
        .retain(|id| lobby.players.contains_key(id));
    let unsent: Vec<_> = lobby
        .players
        .keys()
        .copied()
        .filter(|id| !home.sent_decorations.contains(id))
        .collect();
    if unsent.is_empty() {
        return;
    }
    let decorations = Decorations {
        displays: world_map
            .places
            .displays
            .iter()
            .map(|(&at, &shown)| (at, shown))
            .collect(),
        banners: world_map
            .places
            .banners
            .iter()
            .map(|(&at, &patterns)| (at, patterns))
            .collect(),
    };
    for id in unsent {
        home.sent_decorations.insert(id);
        server.send_game_message(id, ServerToClientMessage::Decorations(decorations.clone()));
    }
}
