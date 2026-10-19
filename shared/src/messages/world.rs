use std::collections::HashMap;

use crate::messages::PlayerId;
use crate::world::{BlockData, Chunk, DroppedItemId, ItemStack, MobId, ServerMob};
use bevy::math::{IVec3, Vec3};
use bevy::prelude::Message;
use serde::{Deserialize, Serialize};

/// WorldUpdate is a message sent from the server to the client to update the client's world state.
/// Only chunks which have been updated since the last message are sent.
#[derive(Default, Serialize, Deserialize, Debug, Clone)]
pub struct WorldUpdate {
    pub tick: u64,
    pub time: u64,
    pub new_map: HashMap<IVec3, Chunk>,
    pub mobs: HashMap<MobId, ServerMob>,
}

/// The current state of one block, sent by the server after the block changed.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct BlockUpdate {
    /// Global block position.
    pub position: IVec3,
    /// The block now at `position`, `None` for air.
    pub block: Option<BlockData>,
}

/// What a client is told of a dropped item: enough to draw it and to move it until the next
/// update.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct DroppedItemState {
    pub stack: ItemStack,
    pub position: Vec3,
    pub velocity: Vec3,
}

/// Why a client no longer has a dropped item. Append only, like the other enums of the wire.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq)]
pub enum ItemRemoval {
    /// It vanished, fell out of the world, or left the chunks the client has.
    Gone,
    PickedUp(PlayerId),
    /// It was added to the stack of the other item.
    Merged(DroppedItemId),
}

/// The dropped items that appeared or changed near a player, and the ones it no longer has. A
/// client applies the upserts before the removals: an item may merge into one it just learnt of.
#[derive(Message, Serialize, Deserialize, Debug, Clone, Default, PartialEq)]
pub struct DroppedItemsUpdate {
    pub upserts: Vec<(DroppedItemId, DroppedItemState)>,
    pub removals: Vec<(DroppedItemId, ItemRemoval)>,
}

impl DroppedItemsUpdate {
    pub fn is_empty(&self) -> bool {
        self.upserts.is_empty() && self.removals.is_empty()
    }
}
