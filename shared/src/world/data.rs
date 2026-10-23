use bevy_ecs::resource::Resource;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;

use super::{ItemId, ItemType};

#[derive(Resource, Clone, Copy, Serialize, Deserialize, Default)]
pub struct WorldSeed(pub u32);

#[derive(Debug, Clone, Serialize, Deserialize, Copy, Default, PartialEq)]
pub struct ItemStack {
    pub item_id: ItemId,
    pub item_type: ItemType,
    pub nb: u32,
}

/// Global trait for all numerical enums serving as unique IDs for certain
/// types of elements in the game. Example : ItemId, BlockId...
/// Used in texture atlases and such
pub trait GameElementId: std::hash::Hash + Eq + PartialEq + Copy + Clone + Default + Debug {}
