//! What the villagers trade, against coins or for them. The offers of a profession are fixed
//! and known to both sides; the server counts what each villager has left, restocked every
//! dawn, and applies the trades.

use bevy::math::IVec3;
use serde::{Deserialize, Serialize};

use crate::players::Inventory;
use crate::world::{ItemId, ItemStack, MobId, MobKind};
use crate::{DAY_DURATION_IN_TICKS, MAX_INVENTORY_SLOTS};

/// A trade ends once the player is farther than this from the villager, in blocks, between
/// their middles.
pub const TRADE_DISTANCE: f32 = 5.0;

/// What a villager gives.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Goods {
    Items(ItemId, u32),
    /// Where the nearest dungeon is, told in the chat.
    Directions,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Offer {
    /// What the player pays.
    pub cost: (ItemId, u32),
    pub goods: Goods,
    /// How many times a day a villager makes it.
    pub stock: u8,
}

/// Why a trade is not made.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TradeRefusal {
    SoldOut,
    CannotPay,
    NoRoom,
}

const fn buys(item: ItemId, nb: u32, coins: u32, stock: u8) -> Offer {
    Offer {
        cost: (item, nb),
        goods: Goods::Items(ItemId::Coin, coins),
        stock,
    }
}

const fn sells(item: ItemId, nb: u32, coins: u32, stock: u8) -> Offer {
    Offer {
        cost: (ItemId::Coin, coins),
        goods: Goods::Items(item, nb),
        stock,
    }
}

const FARMER: [Offer; 6] = [
    buys(ItemId::Rye, 12, 1, 8),
    buys(ItemId::Carrot, 10, 1, 8),
    buys(ItemId::Potato, 10, 1, 8),
    sells(ItemId::RyeBread, 3, 1, 8),
    sells(ItemId::BakedPotato, 4, 1, 8),
    sells(ItemId::RoastBeef, 3, 2, 4),
];
const SMITH: [Offer; 6] = [
    buys(ItemId::Coal, 8, 1, 8),
    buys(ItemId::IronIngot, 3, 2, 6),
    sells(ItemId::IronPickaxe, 1, 6, 2),
    sells(ItemId::IronSword, 1, 5, 2),
    sells(ItemId::BronzeHelmet, 1, 6, 1),
    sells(ItemId::IronChestplate, 1, 14, 1),
];
const CARTOGRAPHER: [Offer; 6] = [
    buys(ItemId::Fiber, 12, 1, 6),
    buys(ItemId::Feather, 4, 1, 6),
    buys(ItemId::Leather, 3, 1, 6),
    sells(ItemId::Torch, 8, 1, 6),
    sells(ItemId::LeatherBoots, 1, 3, 1),
    Offer {
        cost: (ItemId::Coin, 5),
        goods: Goods::Directions,
        stock: 2,
    },
];

/// The most offers a profession has.
pub const MAX_OFFERS: usize = 6;

/// The offers of a villager of `kind`; none of any other mob.
pub fn offers(kind: MobKind) -> &'static [Offer] {
    match kind {
        MobKind::Farmer => &FARMER,
        MobKind::Smith => &SMITH,
        MobKind::Cartographer => &CARTOGRAPHER,
        _ => &[],
    }
}

/// How many of `item` the slots of `inventory` hold.
pub fn held_count(inventory: &Inventory, item: ItemId) -> u32 {
    (0..MAX_INVENTORY_SLOTS)
        .filter_map(|slot| inventory.inner.get(&slot))
        .filter(|stack| stack.item_id == item)
        .map(|stack| stack.nb)
        .sum()
}

impl Offer {
    /// `inventory` once the cost is paid and the goods received.
    pub fn applied_to(&self, inventory: &Inventory) -> Result<Inventory, TradeRefusal> {
        let (item, mut left) = self.cost;
        if held_count(inventory, item) < left {
            return Err(TradeRefusal::CannotPay);
        }
        let mut after = inventory.clone();
        // The last slots first: the hotbar is kept as long as it can be
        for slot in (0..MAX_INVENTORY_SLOTS).rev() {
            let Some(stack) = after.inner.get_mut(&slot).filter(|s| s.item_id == item) else {
                continue;
            };
            let taken = stack.nb.min(left);
            stack.nb -= taken;
            left -= taken;
            if stack.nb == 0 {
                after.inner.remove(&slot);
            }
            if left == 0 {
                break;
            }
        }
        if let Goods::Items(item_id, nb) = self.goods {
            let stack = ItemStack {
                item_id,
                item_type: item_id.get_default_type(),
                nb,
            };
            if after.add_item_to_inventory(stack) > 0 {
                return Err(TradeRefusal::NoRoom);
            }
        }
        Ok(after)
    }
}

/// What a villager remembers.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct Villager {
    /// The cell of its feet when it is at home; where it first stood, for one with no house.
    pub home: Option<IVec3>,
    /// How many times it made each offer since `day`.
    #[serde(default)]
    sold: Vec<u8>,
    /// The day it last restocked (`market_day`).
    #[serde(default)]
    day: u64,
    /// Whether it trades, and stands still meanwhile. Neither saved nor sent.
    #[serde(skip)]
    pub trading: bool,
    /// Whether it goes home or stays there: the night. Neither saved nor sent.
    #[serde(skip)]
    pub homeward: bool,
    /// The cells left to walk through, the next one last. Neither saved nor sent.
    #[serde(skip)]
    pub route: Vec<IVec3>,
}

/// The day of `tick` for the trades, which starts at sunrise.
pub fn market_day(tick: u64) -> u64 {
    (tick + DAY_DURATION_IN_TICKS / 2) / DAY_DURATION_IN_TICKS
}

impl Villager {
    /// A villager whose home is the cell `home`.
    pub fn living_at(home: IVec3) -> Self {
        Self {
            home: Some(home),
            ..Self::default()
        }
    }

    /// How many times it makes each offer of `kind` before it restocks, on the day `day`.
    pub fn left(&self, kind: MobKind, day: u64) -> Vec<u8> {
        let sold = |offer: usize| match self.day == day {
            true => self.sold.get(offer).copied().unwrap_or(0),
            false => 0,
        };
        let offers = offers(kind).iter().enumerate();
        offers
            .map(|(i, offer)| offer.stock.saturating_sub(sold(i)))
            .collect()
    }

    /// Makes the offer `index` of `kind` to the holder of `inventory`, on the day `day`:
    /// nothing changes when it is refused.
    pub fn trade(
        &mut self,
        kind: MobKind,
        index: usize,
        day: u64,
        inventory: &mut Inventory,
    ) -> Result<Goods, TradeRefusal> {
        let offer = offers(kind).get(index).ok_or(TradeRefusal::SoldOut)?;
        if self.left(kind, day)[index] == 0 {
            return Err(TradeRefusal::SoldOut);
        }
        *inventory = offer.applied_to(inventory)?;
        if self.day != day {
            self.day = day;
            self.sold.clear();
        }
        self.sold.resize(offers(kind).len(), 0);
        self.sold[index] += 1;
        Ok(offer.goods)
    }
}

/// A trade, as the client shows it.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct TradeView {
    pub mob: MobId,
    pub kind: MobKind,
    /// How many times the villager still makes each of its offers today.
    pub left: Vec<u8>,
    /// Whether the trade starts: an update of one the player has just closed opens nothing.
    pub opening: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stack(item_id: ItemId, nb: u32) -> ItemStack {
        ItemStack {
            item_id,
            item_type: item_id.get_default_type(),
            nb,
        }
    }

    #[test]
    fn a_trade_takes_its_cost_gives_its_goods_and_is_limited_until_the_next_day() {
        let mut villager = Villager::default();
        let mut inventory = Inventory::new();
        let farmer = MobKind::Farmer;
        let rye = offers(farmer)[0];
        assert_eq!(
            villager.trade(farmer, 0, 3, &mut inventory),
            Err(TradeRefusal::CannotPay)
        );
        inventory.inner.insert(0, stack(ItemId::Rye, 64));
        inventory.inner.insert(20, stack(ItemId::Rye, 64));
        for made in 1..=u32::from(rye.stock) {
            assert!(villager.trade(farmer, 0, 3, &mut inventory).is_ok());
            assert_eq!(held_count(&inventory, ItemId::Coin), made);
            assert_eq!(held_count(&inventory, ItemId::Rye), 128 - 12 * made);
        }
        let before = inventory.clone();
        assert_eq!(
            villager.trade(farmer, 0, 3, &mut inventory),
            Err(TradeRefusal::SoldOut)
        );
        assert_eq!(
            villager.trade(farmer, 99, 3, &mut inventory),
            Err(TradeRefusal::SoldOut)
        );
        assert_eq!(inventory, before);
        assert_eq!(villager.left(farmer, 4)[0], rye.stock);
        // No room for the bread, whose coin is not taken
        for slot in 0..MAX_INVENTORY_SLOTS {
            inventory.inner.insert(slot, stack(ItemId::Dirt, 64));
        }
        inventory.inner.insert(0, stack(ItemId::Coin, 2));
        let before = inventory.clone();
        assert_eq!(
            villager.trade(farmer, 3, 4, &mut inventory),
            Err(TradeRefusal::NoRoom)
        );
        assert_eq!(inventory, before);
        // The stock is saved, the rest of what it does is not
        let saved: Villager = ron::from_str(&ron::to_string(&villager).unwrap()).unwrap();
        assert_eq!(saved, villager);
    }

    #[test]
    fn every_profession_buys_and_sells() {
        for kind in MobKind::ALL {
            let offers = offers(kind);
            assert_eq!(offers.is_empty(), !kind.is_villager(), "{kind:?}");
            assert!(offers.len() <= MAX_OFFERS);
            if kind.is_villager() {
                assert!(offers.iter().any(|o| o.cost.0 == ItemId::Coin));
                assert!(offers.iter().any(|o| o.cost.0 != ItemId::Coin));
            }
        }
    }
}
