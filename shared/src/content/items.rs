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

impl ItemProps {
    /// The properties of `id` written in the code.
    pub fn builtin(id: ItemId) -> Self {
        Self {
            max_stack: id.builtin_max_stack(),
            attack_damage: id.builtin_attack_damage(),
            food_value: id.builtin_food_value(),
            food_effect: id.builtin_food_effect(),
            eaten_remains: id.builtin_eaten_remains(),
            crafting_remains: id.builtin_crafting_remains(),
            horse_armour: id.builtin_horse_armour(),
            spawned_mob: id.builtin_spawned_mob(),
            tool: id.builtin_tool(),
            durability: id.builtin_durability(),
            category: id.builtin_category(),
            default_type: id.builtin_default_type(),
        }
    }
}

impl ItemId {
    /// The properties of the item in the content of the process.
    #[inline]
    pub fn props(self) -> &'static ItemProps {
        content().item(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Each getter reads its own entry of the table: it says what its `builtin_*` says, for
    /// every item.
    #[test]
    fn the_getters_of_every_item_say_what_the_code_says() {
        for id in ItemId::ALL {
            assert_eq!(id.get_max_stack(), id.builtin_max_stack(), "{id:?}");
            assert_eq!(id.attack_damage(), id.builtin_attack_damage(), "{id:?}");
            assert_eq!(id.food_value(), id.builtin_food_value(), "{id:?}");
            assert_eq!(id.food_effect(), id.builtin_food_effect(), "{id:?}");
            assert_eq!(id.eaten_remains(), id.builtin_eaten_remains(), "{id:?}");
            assert_eq!(
                id.crafting_remains(),
                id.builtin_crafting_remains(),
                "{id:?}"
            );
            assert_eq!(id.horse_armour(), id.builtin_horse_armour(), "{id:?}");
            assert_eq!(id.spawned_mob(), id.builtin_spawned_mob(), "{id:?}");
            assert_eq!(id.tool(), id.builtin_tool(), "{id:?}");
            assert_eq!(id.durability(), id.builtin_durability(), "{id:?}");
            assert_eq!(id.category(), id.builtin_category(), "{id:?}");
            assert_eq!(id.get_default_type(), id.builtin_default_type(), "{id:?}");
        }
    }
}
