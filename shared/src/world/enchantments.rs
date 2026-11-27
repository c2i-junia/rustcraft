//! The enchantments: what a piece of equipment may carry, how far, for how many levels of
//! experience, and what it does. They are part of the type of the stack (`ItemType::Enchanted`),
//! with its wear, so that they follow it wherever it goes. The engraving table puts them there
//! (`players::enchanting`); only the server does.

use serde::{Deserialize, Serialize};

use super::{ItemId, ItemName, ItemStack, ItemType, ToolKind};

/// The levels each enchantment has.
pub const MAX_ENCHANTMENT_LEVEL: u32 = 3;
/// What efficiency adds to the speed of a tool at each level, in percent.
const EFFICIENCY_PER_LEVEL: u32 = 50;
/// What durability adds to the durability of an item at each level, in percent.
const DURABILITY_PER_LEVEL: u32 = 50;
/// The points of health sharpness and bow power add to a hit at each level.
const DAMAGE_PER_LEVEL: u32 = 1;
/// The percentage of a hit each level of protection of a piece worn absorbs.
pub const PROTECTION_PER_LEVEL: u32 = 1;

/// Append only, at most 8: stacks and inputs store the position of the variant. Bump
/// `PROTOCOL_ID` with any change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Enchantment {
    Efficiency,
    Durability,
    Sharpness,
    Protection,
    BowPower,
}

impl Enchantment {
    pub const ALL: [Enchantment; 5] = [
        Self::Efficiency,
        Self::Sharpness,
        Self::BowPower,
        Self::Protection,
        Self::Durability,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::Efficiency => "Efficiency",
            Self::Durability => "Durability",
            Self::Sharpness => "Sharpness",
            Self::Protection => "Protection",
            Self::BowPower => "Bow Power",
        }
    }

    /// Its name and its level, as a player reads them: "Efficiency II".
    pub fn title(self, level: u32) -> String {
        let numeral = ["", " I", " II", " III"];
        let numeral = numeral.get(level as usize).copied().unwrap_or_default();
        format!("{}{numeral}", self.name())
    }

    /// The number its effect is measured by at `level`: percent of speed, of durability or
    /// absorbed, or points of damage.
    pub fn amount(self, level: u32) -> u32 {
        level
            * match self {
                Self::Efficiency => EFFICIENCY_PER_LEVEL,
                Self::Durability => DURABILITY_PER_LEVEL,
                Self::Sharpness | Self::BowPower => DAMAGE_PER_LEVEL,
                Self::Protection => PROTECTION_PER_LEVEL,
            }
    }

    /// What it does at `level`, in a few words.
    pub fn effect(self, level: u32) -> String {
        let n = self.amount(level);
        match self {
            Self::Efficiency => format!("+{n} % mining speed"),
            Self::Durability => format!("+{n} % durability"),
            Self::Sharpness => format!("+{n} damage"),
            Self::Protection => format!("+{n} % absorbed"),
            Self::BowPower => format!("+{n} arrow damage"),
        }
    }

    /// Whether `item` may carry it.
    pub fn fits(self, item: ItemId) -> bool {
        let tool = item.tool().map(|tool| tool.kind);
        match self {
            Self::Efficiency => matches!(
                tool,
                Some(ToolKind::Pickaxe | ToolKind::Axe | ToolKind::Shovel | ToolKind::Hoe)
            ),
            Self::Durability => item.durability().is_some(),
            Self::Sharpness => {
                item == ItemId::Spear || matches!(tool, Some(ToolKind::Sword | ToolKind::Axe))
            }
            Self::Protection => item.armour().is_some(),
            Self::BowPower => item == ItemId::Bow,
        }
    }
}

/// The levels of experience it takes to bring an enchantment to `level`.
pub fn enchantment_cost(level: u32) -> u32 {
    match level {
        0 => 0,
        1 => 3,
        2 => 5,
        _ => 8,
    }
}

/// The level of each enchantment of an item, 2 bits each.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Enchantments(u16);

impl Enchantments {
    pub fn level(self, enchantment: Enchantment) -> u32 {
        u32::from(self.0 >> (2 * enchantment as u16)) & MAX_ENCHANTMENT_LEVEL
    }

    /// These, with `enchantment` at `level`.
    pub fn with(self, enchantment: Enchantment, level: u32) -> Self {
        let shift = 2 * enchantment as u16;
        let level = level.min(MAX_ENCHANTMENT_LEVEL) as u16;
        Self(self.0 & !(3 << shift) | level << shift)
    }
}

impl ItemId {
    /// The enchantments the item may carry, in the order they are listed.
    pub fn enchantments(self) -> impl Iterator<Item = Enchantment> {
        Enchantment::ALL
            .into_iter()
            .filter(move |enchantment| enchantment.fits(self))
    }

    pub fn is_enchantable(self) -> bool {
        self.enchantments().next().is_some()
    }
}

impl ItemType {
    /// The wear of an item that wears.
    pub fn wear(self) -> Option<u32> {
        match self {
            Self::Tool { wear } | Self::Enchanted { wear, .. } | Self::Forged { wear, .. } => {
                Some(wear)
            }
            Self::Generic
            | Self::Block(_)
            | Self::Map { .. }
            | Self::Treasure { .. }
            | Self::Banner { .. } => None,
        }
    }

    /// The same item, worn by `wear`.
    pub fn with_wear(self, wear: u32) -> Self {
        match self {
            Self::Enchanted { enchantments, .. } => Self::Enchanted { wear, enchantments },
            Self::Tool { .. } => Self::Tool { wear },
            Self::Forged {
                enchantments,
                repairs,
                name,
                ..
            } => Self::Forged {
                wear,
                enchantments,
                repairs,
                name,
            },
            other => other,
        }
    }

    /// The enchantments the item carries, none for one that carries none.
    pub fn enchantments(self) -> Enchantments {
        match self {
            Self::Enchanted { enchantments, .. } | Self::Forged { enchantments, .. } => {
                enchantments
            }
            _ => Enchantments::default(),
        }
    }

    /// How many times the item was repaired or merged at an anvil.
    pub fn repairs(self) -> u8 {
        match self {
            Self::Forged { repairs, .. } => repairs,
            _ => 0,
        }
    }

    /// The name an anvil gave the item, empty for none.
    pub fn name(self) -> ItemName {
        match self {
            Self::Forged { name, .. } => name,
            _ => ItemName::default(),
        }
    }

    /// The type of an item that wears, with all of this: the simplest variant that holds it, so
    /// that an item never worked at an anvil keeps the variant it had.
    pub fn worked(wear: u32, enchantments: Enchantments, repairs: u8, name: ItemName) -> Self {
        if repairs > 0 || !name.is_empty() {
            Self::Forged {
                wear,
                enchantments,
                repairs,
                name,
            }
        } else if enchantments != Enchantments::default() {
            Self::Enchanted { wear, enchantments }
        } else {
            Self::Tool { wear }
        }
    }
}

/// `value` with `percent` per level added.
fn raised(value: u32, percent: u32, level: u32) -> u32 {
    value * (100 + percent * level) / 100
}

impl ItemStack {
    /// The level of `enchantment` the stack carries, 0 for none.
    pub fn enchantment_level(&self, enchantment: Enchantment) -> u32 {
        if !enchantment.fits(self.item_id) {
            return 0;
        }
        self.item_type.enchantments().level(enchantment)
    }

    /// The enchantments the stack carries, with their levels.
    pub fn enchantments(&self) -> impl Iterator<Item = (Enchantment, u32)> + '_ {
        self.item_id
            .enchantments()
            .map(|enchantment| (enchantment, self.enchantment_level(enchantment)))
            .filter(|&(_, level)| level > 0)
    }

    pub fn is_enchanted(&self) -> bool {
        self.enchantments().next().is_some()
    }

    /// The stack with `enchantment` one level higher, its wear kept: `None` when it does not
    /// take it, or no further.
    pub fn enchanted(&self, enchantment: Enchantment) -> Option<ItemStack> {
        let level = self.enchantment_level(enchantment) + 1;
        if !enchantment.fits(self.item_id) || level > MAX_ENCHANTMENT_LEVEL {
            return None;
        }
        let kind = self.item_type;
        let carried = kind.enchantments();
        Some(ItemStack {
            item_type: ItemType::worked(
                kind.wear().unwrap_or(0),
                carried.with(enchantment, level),
                kind.repairs(),
                kind.name(),
            ),
            ..*self
        })
    }

    /// The wear at which this very item breaks: `ItemId::durability`, raised by its enchantment.
    pub fn durability(&self) -> Option<u32> {
        let level = self.enchantment_level(Enchantment::Durability);
        let durability = self.item_id.durability()?;
        Some(raised(durability, DURABILITY_PER_LEVEL, level))
    }

    /// The mining hits a hit of this very tool counts for, `speed` without its enchantment.
    pub fn mining_speed(&self, speed: u8) -> u8 {
        let level = self.enchantment_level(Enchantment::Efficiency);
        let speed = raised(u32::from(speed), EFFICIENCY_PER_LEVEL, level);
        u8::try_from(speed).unwrap_or(u8::MAX)
    }

    /// The points of health its enchantments add to a blow.
    pub fn bonus_blow_damage(&self) -> u32 {
        self.enchantment_level(Enchantment::Sharpness) * DAMAGE_PER_LEVEL
    }

    /// The points of health its enchantments add to an arrow it shoots.
    pub fn bonus_arrow_damage(&self) -> u32 {
        self.enchantment_level(Enchantment::BowPower) * DAMAGE_PER_LEVEL
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_enchantment_rises_to_its_last_level_on_what_takes_it_and_keeps_the_wear() {
        let mut axe = ItemStack {
            item_id: ItemId::IronAxe,
            item_type: ItemType::Tool { wear: 40 },
            nb: 1,
        };
        assert_eq!(axe.enchanted(Enchantment::Protection), None);
        for level in 1..=MAX_ENCHANTMENT_LEVEL {
            axe = axe.enchanted(Enchantment::Efficiency).unwrap();
            axe = axe.enchanted(Enchantment::Durability).unwrap();
            assert_eq!(axe.enchantment_level(Enchantment::Efficiency), level);
        }
        assert_eq!(axe.enchanted(Enchantment::Efficiency), None);
        assert_eq!(axe.enchantment_level(Enchantment::Sharpness), 0);
        assert_eq!(axe.item_type.wear(), Some(40));
        assert_eq!(axe.durability(), Some(625));
        assert_eq!(axe.mining_speed(6), 15);
        // As a save holds it, in RON, and a chest file, in bincode
        let text = ron::to_string(&axe).unwrap();
        assert_eq!(ron::from_str::<ItemStack>(&text).unwrap(), axe);
        let bytes = crate::game_message_to_payload(axe);
        assert_eq!(
            crate::payload_to_game_message::<ItemStack>(&bytes).unwrap(),
            axe
        );
    }
}
