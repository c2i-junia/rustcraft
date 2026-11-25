//! The armour: the four pieces worn, what share of a hit they absorb and how they wear. The
//! pieces are stacks of the inventory, in its armour slots (`inventory::armour_slots`).

use serde::{Deserialize, Serialize};

use super::game_mode::DamageSource;
use super::inventory::ARMOUR_SLOT_START;
use super::{Inventory, Player};
use crate::world::{Enchantment, ItemId, PROTECTION_PER_LEVEL};

/// The percentage of a hit a point of armour absorbs.
pub const ABSORBED_PER_POINT: u32 = 4;
/// The points of the armour bar when it is full, and the most that count: 80 % of a hit.
pub const MAX_ARMOUR_POINTS: u32 = 20;
/// The most armour absorbs of a hit, its enchantments included, in percent.
const MAX_ABSORBED: u32 = 90;
/// The wear a hit adds to each piece worn.
const WEAR_PER_HIT: u32 = 1;

/// The pieces of a suit of armour, in the order of their slots.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ArmourPiece {
    Helmet,
    Chestplate,
    Leggings,
    Boots,
}

impl ArmourPiece {
    pub const ALL: [ArmourPiece; 4] = [Self::Helmet, Self::Chestplate, Self::Leggings, Self::Boots];

    /// The slot the piece is worn in, in the numbering of the inventory actions.
    pub fn slot(self) -> u32 {
        ARMOUR_SLOT_START + self as u32
    }

    /// The name of the piece as a player reads it.
    pub fn name(self) -> &'static str {
        match self {
            Self::Helmet => "Helmet",
            Self::Chestplate => "Chestplate",
            Self::Leggings => "Leggings",
            Self::Boots => "Boots",
        }
    }
}

/// What a piece of armour is made of.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ArmourMaterial {
    Leather,
    Bronze,
    Iron,
    Diamond,
}

impl ArmourMaterial {
    /// The points of each piece of a suit, in the order of `ArmourPiece::ALL`: 7 for leather,
    /// 11 for bronze, 15 for iron and 19 for diamond, which absorb 28, 44, 60 and 76 %.
    fn points(self) -> [u32; 4] {
        match self {
            Self::Leather => [1, 3, 2, 1],
            Self::Bronze => [2, 4, 3, 2],
            Self::Iron => [2, 6, 5, 2],
            Self::Diamond => [3, 7, 6, 3],
        }
    }

    /// How hard the material wears: a piece takes this times the hits of its shape.
    fn toughness(self) -> u32 {
        match self {
            Self::Leather => 6,
            Self::Bronze => 11,
            Self::Iron => 16,
            Self::Diamond => 32,
        }
    }
}

/// What an armour item is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Armour {
    pub piece: ArmourPiece,
    pub material: ArmourMaterial,
}

impl Armour {
    /// The points of armour the piece gives while it is worn.
    pub fn points(self) -> u32 {
        self.material.points()[self.piece as usize]
    }

    /// The hits the piece absorbs before it breaks.
    pub fn durability(self) -> u32 {
        let hits = match self.piece {
            ArmourPiece::Helmet => 11,
            ArmourPiece::Chestplate => 16,
            ArmourPiece::Leggings => 15,
            ArmourPiece::Boots => 13,
        };
        hits * self.material.toughness()
    }
}

impl ItemId {
    /// What the item is as a piece of armour, if it is one.
    pub fn armour(&self) -> Option<Armour> {
        use ArmourMaterial::*;
        use ArmourPiece::*;
        let (material, piece) = match *self {
            Self::LeatherHelmet => (Leather, Helmet),
            Self::LeatherChestplate => (Leather, Chestplate),
            Self::LeatherLeggings => (Leather, Leggings),
            Self::LeatherBoots => (Leather, Boots),
            Self::BronzeHelmet => (Bronze, Helmet),
            Self::BronzeChestplate => (Bronze, Chestplate),
            Self::BronzeLeggings => (Bronze, Leggings),
            Self::BronzeBoots => (Bronze, Boots),
            Self::IronHelmet => (Iron, Helmet),
            Self::IronChestplate => (Iron, Chestplate),
            Self::IronLeggings => (Iron, Leggings),
            Self::IronBoots => (Iron, Boots),
            Self::DiamondHelmet => (Diamond, Helmet),
            Self::DiamondChestplate => (Diamond, Chestplate),
            Self::DiamondLeggings => (Diamond, Leggings),
            Self::DiamondBoots => (Diamond, Boots),
            _ => return None,
        };
        Some(Armour { piece, material })
    }
}

/// The armour a player wears, as the others see it: one item per piece, in the order of
/// `ArmourPiece::ALL`.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct WornArmour(pub [Option<ItemId>; 4]);

impl Inventory {
    /// The armour in the armour slots.
    pub fn worn(&self) -> WornArmour {
        WornArmour(
            ArmourPiece::ALL.map(|piece| self.inner.get(&piece.slot()).map(|stack| stack.item_id)),
        )
    }

    /// The points of the armour worn, as the armour bar shows them.
    pub fn armour_points(&self) -> u32 {
        let worn = self.worn().0.into_iter().flatten();
        let points: u32 = worn
            .filter_map(|item| item.armour())
            .map(|armour| armour.points())
            .sum();
        points.min(MAX_ARMOUR_POINTS)
    }

    /// The percentage of a hit the armour worn absorbs, its enchantments included.
    fn absorbed_percent(&self) -> u32 {
        let protection: u32 = ArmourPiece::ALL
            .iter()
            .filter_map(|piece| self.inner.get(&piece.slot()))
            .map(|stack| stack.enchantment_level(Enchantment::Protection))
            .sum();
        let absorbed =
            self.armour_points() * ABSORBED_PER_POINT + protection * PROTECTION_PER_LEVEL;
        absorbed.min(MAX_ABSORBED)
    }

    /// Adds `wear` to every piece worn: those worn out break.
    fn wear_armour(&mut self, wear: u32) {
        for piece in ArmourPiece::ALL {
            self.wear_tool(piece.slot(), wear);
        }
    }
}

impl Player {
    /// What the armour worn leaves of a hit of `amount` from `source`, and the wear of the
    /// pieces. As health goes by whole points and most hits take 2 or 3 of them, what is left
    /// is counted in hundredths and the rest is kept for the next hit (`armour_carry`): some
    /// hits take nothing, but over a fight the armour absorbs its exact share.
    pub(super) fn absorb(&mut self, amount: u32, source: DamageSource) -> u32 {
        let absorbed = self.inventory.absorbed_percent();
        if absorbed == 0 || !source.is_absorbed_by_armour() {
            return amount;
        }
        self.inventory.wear_armour(WEAR_PER_HIT);
        let left = amount.saturating_mul(100 - absorbed) + self.armour_carry;
        self.armour_carry = left % 100;
        left / 100
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::players::game_mode::MAX_HEALTH;
    use crate::world::{ItemStack, ItemType, MobKind};

    const IRON: [ItemId; 4] = [
        ItemId::IronHelmet,
        ItemId::IronChestplate,
        ItemId::IronLeggings,
        ItemId::IronBoots,
    ];

    fn wearing(suit: [ItemId; 4]) -> Player {
        let mut player = Player {
            health: MAX_HEALTH,
            ..Default::default()
        };
        for item_id in suit {
            let slot = item_id.armour().unwrap().piece.slot();
            let item_type = item_id.get_default_type();
            let stack = ItemStack {
                item_id,
                item_type,
                nb: 1,
            };
            player.inventory.inner.insert(slot, stack);
        }
        player
    }

    #[test]
    fn a_suit_absorbs_its_share_of_the_hits_it_stops_and_wears_out() {
        let mut player = wearing(IRON);
        assert_eq!(player.inventory.armour_points(), 15);
        // Ten hits of 3, 60 % of them absorbed: 12 points
        for _ in 0..10 {
            player.invulnerable_ticks = 0;
            assert!(player.hurt(3, DamageSource::Mob(MobKind::Stumpling)));
        }
        assert_eq!(player.health, MAX_HEALTH - 12);
        let worn = player.inventory.inner[&ArmourPiece::Boots.slot()].item_type;
        assert_eq!(worn, ItemType::Tool { wear: 10 });

        // Neither a fall nor hunger is absorbed, nor wears
        for source in [DamageSource::Fall, DamageSource::Starvation] {
            player.invulnerable_ticks = 0;
            player.health = MAX_HEALTH;
            player.hurt(5, source);
            assert_eq!(player.health, MAX_HEALTH - 5, "{source:?}");
        }
        assert_eq!(
            player.inventory.inner[&ArmourPiece::Boots.slot()].item_type,
            worn
        );

        // The helmet breaks first, and the rest still protects
        for _ in 10..ItemId::IronHelmet.durability().unwrap() {
            player.invulnerable_ticks = 0;
            player.health = MAX_HEALTH;
            player.hurt(3, DamageSource::Lava);
        }
        assert_eq!(
            player.inventory.worn().0,
            [None, Some(IRON[1]), Some(IRON[2]), Some(IRON[3])]
        );
        assert_eq!(player.inventory.armour_points(), 13);
    }

    #[test]
    fn the_suits_absorb_more_and_more_but_none_all_of_a_hit() {
        let points = |suffix: &str| {
            let suit = ItemId::ALL
                .into_iter()
                .filter(|item| format!("{item:?}").starts_with(suffix))
                .filter_map(|item| item.armour());
            suit.map(Armour::points).sum::<u32>()
        };
        let suits = ["Leather", "Bronze", "Iron", "Diamond"].map(points);
        assert_eq!(suits, [7, 11, 15, 19]);
        assert_eq!(
            ItemId::ALL
                .iter()
                .filter(|item| item.armour().is_some())
                .count(),
            16
        );
        const { assert!(MAX_ARMOUR_POINTS * ABSORBED_PER_POINT < 100) };
    }
}
