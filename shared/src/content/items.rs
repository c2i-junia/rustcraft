//! The properties of an item, as the table of `Content` holds them.

use serde::{Deserialize, Serialize};

use super::content;
use crate::players::effects::EffectKind;
use crate::world::{ItemCategory, ItemId, ItemType, MobKind, Tool};

/// What an item is and does, as the getters of `ItemId` read it: see each getter.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ItemProps {
    /// `ItemId::get_max_stack`.
    pub max_stack: u32,
    /// `ItemId::attack_damage`.
    pub attack_damage: u32,
    /// `ItemId::food_value`.
    pub food_value: Option<(u8, f32)>,
    /// `ItemId::food_effect`.
    pub food_effect: Option<(EffectKind, u8, u32)>,
    /// `ItemId::eaten_remains`.
    pub eaten_remains: Option<ItemId>,
    /// `ItemId::crafting_remains`.
    pub crafting_remains: Option<ItemId>,
    /// `ItemId::horse_armour`.
    pub horse_armour: Option<u32>,
    /// `ItemId::spawned_mob`.
    pub spawned_mob: Option<MobKind>,
    /// `ItemId::tool`.
    pub tool: Option<Tool>,
    /// `ItemId::durability`.
    pub durability: Option<u32>,
    /// `ItemId::category`.
    pub category: ItemCategory,
    /// `ItemId::get_default_type`.
    pub default_type: ItemType,
}

impl ItemId {
    /// The properties of the item in the content of the process.
    #[inline]
    pub fn props(self) -> &'static ItemProps {
        content().item(self)
    }
}
