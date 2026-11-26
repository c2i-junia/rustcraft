//! What the containers of the dungeons hold, better the deeper: each item with its chance, its
//! fewest and its most. A new item is added at the end of a table, as the draws of an entry
//! come from its rank.

use shared::world::ItemId;

use super::super::structure::LootEntry;
use super::DungeonKind;

pub fn table(kind: DungeonKind) -> &'static [LootEntry] {
    match kind {
        DungeonKind::Ruin => &[
            (ItemId::RyeBread, 0.6, 1, 3),
            (ItemId::Torch, 0.6, 2, 8),
            (ItemId::Rope, 0.4, 1, 3),
            (ItemId::Coal, 0.5, 2, 6),
            (ItemId::CopperIngot, 0.5, 1, 4),
            (ItemId::TinIngot, 0.3, 1, 3),
            (ItemId::BronzeIngot, 0.3, 1, 3),
            (ItemId::BronzeSword, 0.15, 1, 1),
            (ItemId::BronzePickaxe, 0.1, 1, 1),
            (ItemId::Spear, 0.15, 1, 1),
            (ItemId::FlintAndSteel, 0.1, 1, 1),
            (ItemId::Arrow, 0.35, 2, 8),
            (ItemId::Bow, 0.12, 1, 1),
            (ItemId::LeatherHelmet, 0.1, 1, 1),
            (ItemId::LeatherBoots, 0.1, 1, 1),
            (ItemId::BronzeHelmet, 0.05, 1, 1),
            (ItemId::Saddle, 0.15, 1, 1),
        ],
        DungeonKind::Crypt => &[
            (ItemId::Torch, 0.5, 4, 10),
            (ItemId::RyeBread, 0.4, 1, 3),
            (ItemId::Coal, 0.5, 3, 10),
            (ItemId::BronzeIngot, 0.5, 2, 5),
            (ItemId::IronIngot, 0.7, 2, 6),
            (ItemId::GoldIngot, 0.3, 1, 3),
            (ItemId::IronSword, 0.2, 1, 1),
            (ItemId::IronPickaxe, 0.15, 1, 1),
            (ItemId::IronAxe, 0.1, 1, 1),
            (ItemId::Lantern, 0.2, 1, 2),
            (ItemId::Diamond, 0.06, 1, 1),
            (ItemId::Arrow, 0.5, 4, 12),
            (ItemId::Bow, 0.2, 1, 1),
            (ItemId::BronzeChestplate, 0.12, 1, 1),
            (ItemId::IronHelmet, 0.1, 1, 1),
            (ItemId::IronBoots, 0.1, 1, 1),
            (ItemId::Saddle, 0.12, 1, 1),
        ],
        DungeonKind::Sanctuary => &[
            (ItemId::IronIngot, 0.8, 3, 8),
            (ItemId::GoldIngot, 0.8, 2, 6),
            (ItemId::Diamond, 0.55, 1, 3),
            (ItemId::GoldenSword, 0.15, 1, 1),
            (ItemId::DiamondSword, 0.12, 1, 1),
            (ItemId::DiamondPickaxe, 0.12, 1, 1),
            (ItemId::DiamondAxe, 0.08, 1, 1),
            (ItemId::Obsidian, 0.4, 2, 6),
            (ItemId::GoldBlock, 0.1, 1, 1),
            (ItemId::RoastBeef, 0.5, 1, 4),
            (ItemId::Arrow, 0.6, 8, 20),
            (ItemId::Bow, 0.25, 1, 1),
            (ItemId::IronChestplate, 0.15, 1, 1),
            (ItemId::IronLeggings, 0.15, 1, 1),
            (ItemId::DiamondHelmet, 0.06, 1, 1),
            (ItemId::DiamondBoots, 0.06, 1, 1),
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_loot_of_the_dungeons_fits_and_is_better_with_depth() {
        let diamonds = |kind| {
            table(kind)
                .iter()
                .filter(|entry| entry.0 == ItemId::Diamond)
                .map(|&(_, chance, fewest, most)| chance * (fewest + most) as f32 / 2.0)
                .sum::<f32>()
        };
        let [ruin, crypt, sanctuary] = DungeonKind::ALL.map(diamonds);
        assert!(ruin < crypt && crypt < sanctuary);
        for kind in DungeonKind::ALL {
            for &(item, chance, fewest, most) in table(kind) {
                assert!((0.0..=1.0).contains(&chance) && fewest >= 1 && most >= fewest);
                assert!(most <= item.get_max_stack(), "{item:?}");
            }
        }
    }
}
