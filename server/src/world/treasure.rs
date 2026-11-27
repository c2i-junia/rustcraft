//! The treasure maps found in the wrecks (`generation::ocean`): a player who holds one has the
//! place of its buried chest marked on its maps, as a red cross named "X".

use bevy::prelude::*;
use shared::world::{ItemId, ItemType};
use shared::MAX_INVENTORY_SLOTS;

use super::data::ServerWorldMap;
use crate::init::ServerTime;

/// The ticks between two looks at what the players hold.
const LOOK_EVERY: u64 = 20;
/// The name and the colour (`MARKER_COLORS`) of the marker of a treasure.
pub const TREASURE_MARKER: &str = "X";
const TREASURE_COLOR: u8 = 0;

/// Marks on the maps of each player the treasures of the treasure maps it holds, once each.
pub fn treasure_maps_system(mut world_map: ResMut<ServerWorldMap>, time: Res<ServerTime>) {
    if time.0 % LOOK_EVERY != 0 {
        return;
    }
    for player in world_map.players.values_mut() {
        let treasures: Vec<IVec3> = (0..MAX_INVENTORY_SLOTS)
            .filter_map(|slot| player.inventory.inner.get(&slot))
            .chain(player.inventory.floating.as_ref())
            .filter_map(|stack| match (stack.item_id, stack.item_type) {
                (ItemId::TreasureMap, ItemType::Treasure { at }) => Some(at),
                _ => None,
            })
            .collect();
        for at in treasures {
            let marked = player
                .waypoints
                .markers
                .iter()
                .any(|marker| marker.at == at);
            if !marked {
                player.waypoints.mark(TREASURE_MARKER, TREASURE_COLOR, at);
            }
        }
    }
}
