use std::collections::HashMap;

use crate::world::{ItemStack, ServerChunk, ServerMob};
use bevy::{
    math::{IVec3, Vec3},
    prelude::Event,
};
use serde::{Deserialize, Serialize};

use super::PlayerId;

#[derive(Default, Serialize, Deserialize, Debug, Clone)]
pub struct WorldUpdate {
    pub tick: u64,
    pub new_map: HashMap<IVec3, ServerChunk>,
    pub player_positions: HashMap<PlayerId, Vec3>,
    pub time: u64,
    pub mobs: Vec<ServerMob>,
    pub item_stacks: Vec<ItemStackUpdateEvent>,
}

#[derive(Debug, Default, Serialize, Deserialize, Clone, Event)]
pub struct ItemStackUpdateEvent {
    pub id: u128,
    /// `None` if the stack has been deleted, `Some` if it has been updated in any way (position, number of items...)
    pub data: Option<(ItemStack, Vec3)>,
}
