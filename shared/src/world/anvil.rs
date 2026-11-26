//! The anvil: what it does to the items of its two slots, for how many levels of experience. It
//! repairs an item with the material it is made of, merges two worn items of the same kind, their
//! durabilities and enchantments together, and renames them. Each repair or merge makes the next
//! one dearer, as in Minecraft. `players::anvil` applies it; only the server does.

use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::{Ingredient, ItemId, ItemStack, ItemTag, ItemType, ToolTier};
use crate::players::armour::ArmourMaterial;

/// The most characters a name given at an anvil has.
pub const MAX_ITEM_NAME_CHARS: usize = 24;
/// The bytes the longest name takes in UTF-8, each of its characters a 2-byte accented letter.
const NAME_BYTES: usize = 2 * MAX_ITEM_NAME_CHARS;

/// The most levels a work at the anvil may cost: beyond, it is too dear, as in Minecraft.
pub const MAX_ANVIL_COST: u32 = 39;
/// What a merge adds to the durabilities of the two items put together, in percent of the
/// durability of the item.
const MERGE_BONUS_PERCENT: u32 = 12;
/// How many units of its material repair an item from nothing to new.
const UNITS_PER_REPAIR: u32 = 4;
/// What a merge costs before the enchantments it brings.
const MERGE_COST: u32 = 2;
/// What giving a name costs.
const RENAME_COST: u32 = 1;
/// The repairs counted at most for the price of the next one.
const MAX_COUNTED_REPAIRS: u8 = 6;

/// A name given to an item at an anvil: up to `MAX_ITEM_NAME_CHARS` characters, without control
/// characters nor spaces at its ends. Held inline, so that the type of a stack stays `Copy`;
/// saved and sent as a string.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct ItemName {
    len: u8,
    bytes: [u8; NAME_BYTES],
}

impl Default for ItemName {
    fn default() -> Self {
        Self {
            len: 0,
            bytes: [0; NAME_BYTES],
        }
    }
}

impl ItemName {
    /// `text`, trimmed, without its control characters, cut after `MAX_ITEM_NAME_CHARS`
    /// characters or at the last whole character that fits.
    pub fn new(text: &str) -> Self {
        let mut name = Self::default();
        let mut len = 0;
        for c in text
            .trim()
            .chars()
            .filter(|c| !c.is_control())
            .take(MAX_ITEM_NAME_CHARS)
        {
            let size = c.len_utf8();
            if len + size > NAME_BYTES {
                break;
            }
            c.encode_utf8(&mut name.bytes[len..len + size]);
            len += size;
        }
        // The cut may leave spaces at the end
        let kept = std::str::from_utf8(&name.bytes[..len])
            .unwrap_or_default()
            .trim_end()
            .len();
        name.bytes[kept..].fill(0);
        name.len = kept as u8;
        name
    }

    pub fn as_str(&self) -> &str {
        std::str::from_utf8(&self.bytes[..usize::from(self.len)]).unwrap_or_default()
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}

impl fmt::Debug for ItemName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self.as_str(), f)
    }
}

impl Serialize for ItemName {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for ItemName {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        Ok(Self::new(&text))
    }
}

/// What repairs `item` at an anvil: the material it is made of. `None` for what does not wear.
pub fn repair_material(item: ItemId) -> Option<Ingredient> {
    let tag = Ingredient::Tag;
    let one = Ingredient::Item;
    if let Some(armour) = item.armour() {
        return Some(one(match armour.material {
            ArmourMaterial::Leather => ItemId::Leather,
            ArmourMaterial::Bronze => ItemId::BronzeIngot,
            ArmourMaterial::Iron => ItemId::IronIngot,
            ArmourMaterial::Diamond => ItemId::Diamond,
        }));
    }
    if let Some(tool) = item.tool() {
        return Some(match tool.tier {
            ToolTier::Wood => tag(ItemTag::Planks),
            ToolTier::Stone => tag(ItemTag::Cobble),
            ToolTier::Iron => one(ItemId::IronIngot),
            ToolTier::Gold => one(ItemId::GoldIngot),
            ToolTier::Diamond => one(ItemId::Diamond),
            ToolTier::Bronze => one(ItemId::BronzeIngot),
        });
    }
    match item {
        ItemId::FlintAndSteel => Some(one(ItemId::IronIngot)),
        ItemId::Bow => Some(one(ItemId::Rope)),
        _ => None,
    }
}

/// What the anvil does with the items of its slots.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ForgeWork {
    /// Repaired with units of its material.
    Repair,
    /// Merged with another of its kind.
    Merge,
    /// Only renamed.
    Rename,
}

/// A work the anvil may do: the item it gives back, its price in levels before the creative
/// mode is considered, and how many items it takes from the second slot.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Forging {
    pub work: ForgeWork,
    pub result: ItemStack,
    pub cost: u32,
    pub used: u32,
}

/// Why the anvil does nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ForgeRefusal {
    /// The first slot holds nothing the anvil works: only what wears.
    NothingToWork,
    /// The second slot holds neither the material of the item nor another of its kind.
    WrongMaterial,
    /// The work would change nothing: the item is whole, its name the same.
    Unchanged,
    /// The work would cost these levels, beyond `MAX_ANVIL_COST`.
    TooExpensive(u32),
}

/// The levels the repairs of an item add to the price of its next work: 0, 1, 3, 7...
pub fn repair_penalty(repairs: u8) -> u32 {
    (1u32 << repairs.min(MAX_COUNTED_REPAIRS)) - 1
}

/// What the anvil does with `left`, the item of its first slot, `right`, what its second slot
/// holds, and `name`, the name asked for: empty for the name of the kind of item.
pub fn forge(
    left: &ItemStack,
    right: Option<&ItemStack>,
    name: ItemName,
) -> Result<Forging, ForgeRefusal> {
    let item = left.item_id;
    let base = item.durability().ok_or(ForgeRefusal::NothingToWork)?;
    let kind = left.item_type;
    let mut wear = kind.wear().unwrap_or(0);
    let mut enchantments = kind.enchantments();
    let mut repairs = kind.repairs();
    let mut penalty = repair_penalty(repairs);
    let mut cost = 0;
    let mut used = 0;
    let mut work = ForgeWork::Rename;
    let mut changed = false;
    match right {
        None => {}
        Some(other) if other.item_id == item => {
            work = ForgeWork::Merge;
            let theirs = other.item_type;
            penalty = penalty.max(repair_penalty(theirs.repairs()));
            for enchantment in item.enchantments() {
                let mine = enchantments.level(enchantment);
                let their = theirs.enchantments().level(enchantment);
                let level = if mine == their && mine > 0 {
                    mine + 1
                } else {
                    mine.max(their)
                }
                .min(super::MAX_ENCHANTMENT_LEVEL);
                if their > 0 {
                    cost += level;
                }
                if level != mine {
                    enchantments = enchantments.with(enchantment, level);
                    changed = true;
                }
            }
            let merged = ItemStack {
                item_type: ItemType::worked(wear, enchantments, 0, name),
                ..*left
            };
            let durability = merged.durability().unwrap_or(base);
            let mine_left = durability.saturating_sub(wear);
            let their_left = other
                .durability()
                .unwrap_or(base)
                .saturating_sub(theirs.wear().unwrap_or(0));
            let bonus = base * MERGE_BONUS_PERCENT / 100;
            let left_after = (mine_left + their_left + bonus).min(durability);
            if left_after > mine_left {
                wear = durability - left_after;
                changed = true;
            }
            if !changed {
                return Err(ForgeRefusal::Unchanged);
            }
            cost += MERGE_COST;
            used = 1;
            repairs = repairs.max(theirs.repairs()).saturating_add(1);
        }
        Some(material) => {
            if !repair_material(item).is_some_and(|wanted| wanted.matches(material.item_id)) {
                return Err(ForgeRefusal::WrongMaterial);
            }
            if wear == 0 {
                return Err(ForgeRefusal::Unchanged);
            }
            work = ForgeWork::Repair;
            let unit = base.div_ceil(UNITS_PER_REPAIR);
            used = material.nb.min(wear.div_ceil(unit));
            wear = wear.saturating_sub(used * unit);
            cost += used;
            changed = true;
            repairs = repairs.saturating_add(1);
        }
    }
    if name != kind.name() {
        cost += RENAME_COST;
        changed = true;
    }
    if !changed {
        return Err(ForgeRefusal::Unchanged);
    }
    cost += penalty;
    if cost > MAX_ANVIL_COST {
        return Err(ForgeRefusal::TooExpensive(cost));
    }
    let result = ItemStack {
        item_type: ItemType::worked(wear, enchantments, repairs, name),
        ..*left
    };
    Ok(Forging {
        work,
        result,
        cost,
        used,
    })
}

impl ItemStack {
    /// The name an anvil gave the item, if any.
    pub fn custom_name(&self) -> Option<&str> {
        match &self.item_type {
            ItemType::Forged { name, .. } if !name.is_empty() => Some(name.as_str()),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::Enchantment;

    fn worn(item_id: ItemId, wear: u32) -> ItemStack {
        ItemStack {
            item_id,
            item_type: ItemType::Tool { wear },
            nb: 1,
        }
    }

    fn some(item_id: ItemId, nb: u32) -> ItemStack {
        ItemStack {
            item_id,
            item_type: item_id.get_default_type(),
            nb,
        }
    }

    #[test]
    fn a_name_is_trimmed_cut_and_kept_whole_through_a_save() {
        assert_eq!(ItemName::new("  Excalibur \n").as_str(), "Excalibur");
        assert!(ItemName::new("   ").is_empty());
        let long = "é".repeat(40);
        assert_eq!(
            ItemName::new(&long).as_str().chars().count(),
            MAX_ITEM_NAME_CHARS
        );
        let sword = ItemStack {
            item_id: ItemId::IronSword,
            item_type: ItemType::worked(3, Default::default(), 2, ItemName::new("Épée de Léa")),
            nb: 1,
        };
        assert_eq!(sword.custom_name(), Some("Épée de Léa"));
        let text = ron::to_string(&sword).unwrap();
        assert_eq!(ron::from_str::<ItemStack>(&text).unwrap(), sword);
        let bytes = crate::game_message_to_payload(sword);
        assert_eq!(
            crate::payload_to_game_message::<ItemStack>(&bytes).unwrap(),
            sword
        );
    }

    #[test]
    fn the_material_repairs_a_quarter_a_unit_and_is_taken_only_as_needed() {
        // An iron pickaxe wears out after 250: a quarter is 63
        let pickaxe = worn(ItemId::IronPickaxe, 200);
        let forging = forge(
            &pickaxe,
            Some(&some(ItemId::IronIngot, 10)),
            ItemName::default(),
        );
        let forging = forging.unwrap();
        assert_eq!(forging.work, ForgeWork::Repair);
        assert_eq!(forging.used, 4);
        assert_eq!(forging.cost, 4);
        assert_eq!(forging.result.item_type.wear(), Some(0));
        assert_eq!(forging.result.item_type.repairs(), 1);
        let one = forge(
            &pickaxe,
            Some(&some(ItemId::IronIngot, 1)),
            ItemName::default(),
        );
        assert_eq!(one.unwrap().result.item_type.wear(), Some(137));
        // Any planks for wood, the gloomrock for stone, nothing else
        let axe = worn(ItemId::WoodenAxe, 30);
        assert!(forge(
            &axe,
            Some(&some(ItemId::SprucePlanks, 1)),
            ItemName::default()
        )
        .is_ok());
        let stone = worn(ItemId::StoneSword, 30);
        assert!(forge(
            &stone,
            Some(&some(ItemId::CobbledGloomrock, 1)),
            ItemName::default()
        )
        .is_ok());
        assert_eq!(
            forge(
                &pickaxe,
                Some(&some(ItemId::GoldIngot, 1)),
                ItemName::default()
            ),
            Err(ForgeRefusal::WrongMaterial)
        );
        assert_eq!(
            forge(
                &worn(ItemId::IronPickaxe, 0),
                Some(&some(ItemId::IronIngot, 1)),
                ItemName::default()
            ),
            Err(ForgeRefusal::Unchanged)
        );
        assert_eq!(
            forge(&some(ItemId::Dirt, 3), None, ItemName::new("Terre")),
            Err(ForgeRefusal::NothingToWork)
        );
    }

    #[test]
    fn a_merge_adds_the_durabilities_with_a_bonus_and_combines_the_enchantments() {
        let mut mine = worn(ItemId::IronSword, 200);
        mine = mine.enchanted(Enchantment::Sharpness).unwrap();
        let mut theirs = worn(ItemId::IronSword, 300);
        theirs = theirs.enchanted(Enchantment::Sharpness).unwrap();
        theirs = theirs.enchanted(Enchantment::Durability).unwrap();
        let forging = forge(&mine, Some(&theirs), ItemName::default()).unwrap();
        assert_eq!(forging.work, ForgeWork::Merge);
        assert_eq!(forging.used, 1);
        let result = forging.result;
        // Two levels of sharpness make the next one; durability comes along
        assert_eq!(result.enchantment_level(Enchantment::Sharpness), 2);
        assert_eq!(result.enchantment_level(Enchantment::Durability), 1);
        // 375 with durability I: 175 left of it, 75 of the other, 30 of bonus
        assert_eq!(result.durability(), Some(375));
        assert_eq!(result.item_type.wear(), Some(375 - (175 + 75 + 30)));
        // 2, and the levels of the two enchantments the other brings
        assert_eq!(forging.cost, 2 + 2 + 1);
        // Two whole swords with nothing to give each other
        let whole = worn(ItemId::IronSword, 0);
        assert_eq!(
            forge(&whole, Some(&whole), ItemName::default()),
            Err(ForgeRefusal::Unchanged)
        );
    }

    #[test]
    fn each_repair_makes_the_next_dearer_until_it_is_too_dear() {
        let mut pickaxe = worn(ItemId::DiamondPickaxe, 1500);
        let mut costs = Vec::new();
        loop {
            let ingot = some(ItemId::Diamond, 1);
            match forge(&pickaxe, Some(&ingot), ItemName::default()) {
                Ok(forging) => {
                    costs.push(forging.cost);
                    pickaxe = forging.result;
                    pickaxe.item_type = pickaxe.item_type.with_wear(1500);
                }
                Err(refusal) => {
                    assert_eq!(refusal, ForgeRefusal::TooExpensive(64));
                    break;
                }
            }
        }
        assert_eq!(costs, [1, 2, 4, 8, 16, 32]);
    }

    #[test]
    fn a_name_costs_a_level_and_an_empty_one_gives_back_the_name_of_the_kind() {
        let axe = worn(ItemId::BronzeAxe, 10);
        let named = forge(&axe, None, ItemName::new("Hache")).unwrap();
        assert_eq!(
            (named.work, named.cost, named.used),
            (ForgeWork::Rename, 1, 0)
        );
        assert_eq!(named.result.custom_name(), Some("Hache"));
        // A rename alone adds no repair
        assert_eq!(named.result.item_type.repairs(), 0);
        assert_eq!(
            forge(&named.result, None, ItemName::new("Hache")),
            Err(ForgeRefusal::Unchanged)
        );
        let unnamed = forge(&named.result, None, ItemName::default()).unwrap();
        assert_eq!(unnamed.result.item_type, ItemType::Tool { wear: 10 });
    }
}
