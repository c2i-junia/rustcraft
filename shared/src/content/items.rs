//! The properties of an item, as the table of `Content` holds them.

use serde::{Deserialize, Serialize};

use super::content;
use crate::players::effects::EffectKind;
use crate::world::{ItemCategory, ItemId, ItemType, MobKind, Tool};

/// The name an item is shown under, in each language, as its file or that of its block writes
/// it: `name: (en: "Amber", fr: "Ambre")`. A language it leaves out takes the name of the
/// language files (`item.<Ident>`).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct DisplayName {
    pub en: Option<String>,
    pub fr: Option<String>,
}

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
    /// Whether the item only holds the place of one the files no longer define, or never did
    /// in this process (`Content::hold_item_places`).
    pub placeholder: bool,
}

impl ItemProps {
    /// What an item is that a world holds but the content no longer defines: it places
    /// nothing and does nothing. Its name is kept, so that the world saves it back as it was.
    pub const PLACEHOLDER: ItemProps = ItemProps {
        max_stack: 64,
        attack_damage: 1,
        food_value: None,
        food_effect: None,
        eaten_remains: None,
        crafting_remains: None,
        horse_armour: None,
        spawned_mob: None,
        tool: None,
        durability: None,
        category: ItemCategory::Blocks,
        default_type: ItemType::Generic,
        placeholder: true,
    };
}

impl ItemId {
    /// Every item of the content of the process, by number: those of the engine first.
    pub fn all() -> impl Iterator<Item = ItemId> {
        (0..content().items.len()).map(|index| ItemId::from_index(index as u16))
    }

    /// The name of the item in the content of the process, as the files and the commands name
    /// it: `oak_log`. `None` for a number the content does not define.
    pub fn content_name(self) -> Option<&'static str> {
        content().item_names.get(self.index()).map(String::as_str)
    }

    /// The name the content files give the item to be shown under, if they give one.
    pub fn display_name(self) -> Option<&'static DisplayName> {
        u16::try_from(self.index())
            .ok()
            .and_then(|index| content().item_display.get(&index))
    }

    /// Whether the item only holds the place of one the content files do not define.
    pub fn is_placeholder(self) -> bool {
        content().item(self).placeholder
    }

    /// The properties of the item in the content of the process.
    #[inline]
    pub fn props(self) -> &'static ItemProps {
        content().item(self)
    }
}
