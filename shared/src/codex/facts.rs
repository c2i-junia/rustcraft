//! Where an item comes from, read across the tables of the game: the blocks whose breaking
//! gives it, the creatures that drop it, the villagers that trade it.

use crate::world::trade::{offers, Goods, Offer};
use crate::world::{BlockId, ItemId, Loot, MobKind};

/// A block whose breaking may give an item: the share of its breakings that do, and how many.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BlockSource {
    pub block: BlockId,
    pub chance: f64,
    pub count: u32,
}

/// The blocks whose breaking may give `item`, but for the block of the item itself, which drops
/// itself and is no news.
pub fn block_sources(item: ItemId) -> Vec<BlockSource> {
    BlockId::all()
        .filter_map(|block| {
            let table = block.get_drop_table();
            let total: u32 = table.iter().map(|(chance, _, _)| chance).sum();
            let &(chance, _, count) = table.iter().find(|(_, i, _)| *i == item)?;
            let own = format!("{block:?}") == format!("{item:?}");
            (!own && total > 0).then_some(BlockSource {
                block,
                chance: f64::from(chance) / f64::from(total),
                count,
            })
        })
        .collect()
}

/// The creatures whose loot holds `item`, with its line.
pub fn mob_sources(item: ItemId) -> Vec<(MobKind, Loot)> {
    MobKind::ALL
        .iter()
        .flat_map(|&kind| {
            kind.loot()
                .iter()
                .filter(move |l| l.item == item)
                .map(move |l| (kind, *l))
        })
        .collect()
}

/// The offers of villagers that sell `item` or take it as their price.
pub fn trades_with(item: ItemId) -> Vec<(MobKind, Offer)> {
    MobKind::ALL
        .iter()
        .flat_map(|&kind| {
            offers(kind)
                .iter()
                .filter(move |o| {
                    o.cost.0 == item || matches!(o.goods, Goods::Items(i, _) if i == item)
                })
                .map(move |o| (kind, *o))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sources_of_an_item_are_found_across_the_tables() {
        let gravel = block_sources(ItemId::Flint);
        assert!(gravel
            .iter()
            .any(|s| s.block == BlockId::Gravel && (s.chance - 0.1).abs() < 1e-9));
        assert!(block_sources(ItemId::Gravel)
            .iter()
            .all(|s| s.block != BlockId::Gravel));
        assert!(mob_sources(ItemId::Wool)
            .iter()
            .any(|(kind, _)| *kind == MobKind::Sheep));
        assert!(trades_with(ItemId::IronSword)
            .iter()
            .any(|(kind, _)| *kind == MobKind::Smith));
    }
}
