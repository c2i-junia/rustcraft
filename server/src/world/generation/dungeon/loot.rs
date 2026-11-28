//! What the containers of the dungeons hold, better the deeper: each item with its chance, its
//! fewest and its most. A new item is added at the end of a table, as the draws of an entry
//! come from its rank.

use shared::world::structure_loot;

use super::super::structure::LootEntry;
use super::DungeonKind;

pub fn table(kind: DungeonKind) -> &'static [LootEntry] {
    match kind {
        DungeonKind::Ruin => structure_loot::RUIN,
        DungeonKind::Crypt => structure_loot::CRYPT,
        DungeonKind::Sanctuary => structure_loot::SANCTUARY,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::world::ItemId;

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
