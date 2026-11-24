//! What the chests and barrels of a village hold: drawn from the position of each, so that a
//! chest holds the same whichever chunk around it is generated first.

use bevy::prelude::*;
use shared::world::{Container, ItemId, ItemStack, CHEST_SLOTS};

use super::super::PositionRandom;
use super::pieces::PieceKind;

/// Each draw of a container uses its own salt from here.
const SALT_LOOT: u64 = 0x2000;

/// The items a container of a piece may hold: each with its chance and its fewest and most.
fn table(kind: PieceKind) -> &'static [(ItemId, f32, u32, u32)] {
    match kind {
        PieceKind::Forge => &[
            (ItemId::BronzeIngot, 0.7, 1, 4),
            (ItemId::CopperIngot, 0.5, 1, 5),
            (ItemId::TinIngot, 0.4, 1, 3),
            (ItemId::IronIngot, 0.25, 1, 3),
            (ItemId::Coal, 0.6, 2, 8),
            (ItemId::BronzePickaxe, 0.15, 1, 1),
            (ItemId::BronzeSword, 0.15, 1, 1),
            (ItemId::BronzeAxe, 0.1, 1, 1),
            (ItemId::Flint, 0.3, 1, 3),
        ],
        PieceKind::Granary => &[
            (ItemId::Rye, 0.9, 4, 16),
            (ItemId::RyeBread, 0.6, 1, 4),
            (ItemId::Berries, 0.4, 2, 8),
            (ItemId::Fiber, 0.3, 2, 6),
        ],
        _ => &[
            (ItemId::RyeBread, 0.6, 1, 3),
            (ItemId::Berries, 0.4, 1, 5),
            (ItemId::Torch, 0.4, 2, 6),
            (ItemId::Stick, 0.3, 2, 6),
            (ItemId::Wool, 0.2, 1, 3),
            (ItemId::Rope, 0.2, 1, 2),
            (ItemId::WoodenHoe, 0.1, 1, 1),
        ],
    }
}

/// What the container at `pos`, in a piece of `kind`, holds in a world of `seed`: a few stacks
/// of its table, in slots drawn at random.
pub fn container(seed: u32, pos: IVec3, kind: PieceKind) -> Container {
    let random = PositionRandom {
        seed,
        coords: [pos.x, pos.y, pos.z],
    };
    let mut container = Container::default();
    for (i, &(item, chance, fewest, most)) in table(kind).iter().enumerate() {
        let salt = SALT_LOOT + 4 * i as u64;
        if random.unit(salt) >= chance {
            continue;
        }
        let nb = fewest + random.below(salt + 1, (most - fewest + 1) as u64) as u32;
        // the first free slot from the one drawn
        let start = random.below(salt + 2, CHEST_SLOTS as u64) as usize;
        let slots = CHEST_SLOTS as usize;
        if let Some(slot) = (0..slots)
            .map(|k| (start + k) % slots)
            .find(|&slot| container.slots[slot].is_none())
        {
            container.slots[slot] = Some(ItemStack {
                item_id: item,
                item_type: item.get_default_type(),
                nb,
            });
        }
    }
    container
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_container_holds_the_same_at_the_same_place() {
        let at = IVec3::new(120, 70, -40);
        assert_eq!(
            container(3, at, PieceKind::Forge),
            container(3, at, PieceKind::Forge)
        );
        let filled = (0..50)
            .filter(|x| !container(3, at + IVec3::X * x, PieceKind::Granary).is_empty())
            .count();
        assert!(filled > 40, "{filled}");
    }

    #[test]
    fn the_stacks_fit_their_items() {
        for kind in [PieceKind::Forge, PieceKind::Granary, PieceKind::SmallHouse] {
            for &(item, _, fewest, most) in table(kind) {
                assert!(fewest >= 1 && most >= fewest, "{item:?}");
                assert!(most <= item.get_max_stack(), "{item:?}");
            }
        }
    }
}
