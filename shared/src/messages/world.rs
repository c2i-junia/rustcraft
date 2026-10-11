use std::collections::HashMap;

use crate::world::{BlockData, Chunk, MobId, ServerMob};
use bevy::math::IVec3;
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
