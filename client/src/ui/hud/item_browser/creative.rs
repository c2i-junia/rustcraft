//! What the gestures on the catalog do in Creative, where items come out of it, and the texts
//! that say so. Pure: `catalog_actions` sends what they decide.

use shared::players::inventory::InventoryAction;
use shared::world::{ItemId, ItemStack};

use crate::ui::hud::item_name;

/// What the footer says in Creative, beside the trash.
pub const TRASH_HINT: &str = "Drop a stack here, or on the list, to destroy it.";
/// The accessible label of the trash.
pub const TRASH_LABEL: &str = "Trash";

/// A gesture on an item of the catalog: a click (or Enter), a right click (or Space), or
/// Shift with the click.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Gesture {
    Click,
    RightClick,
    ShiftClick,
}

/// What `gesture` on `item` of the catalog does in Creative, the cursor holding `held`: holding
/// a stack, the catalog takes it back and destroys it, unless a right click asks for one more of
/// the same item; else a click puts a full stack on the cursor, a right click one item, and
/// Shift+click a full stack into the inventory.
pub fn creative_action(item: ItemId, gesture: Gesture, held: Option<ItemStack>) -> InventoryAction {
    let one_more = gesture == Gesture::RightClick && held.is_some_and(|held| held.item_id == item);
    if held.is_some() && !one_more {
        return InventoryAction::DestroyFloating;
    }
    let (count, to_inventory) = match gesture {
        Gesture::Click => (item.get_max_stack(), false),
        Gesture::RightClick => (1, false),
        Gesture::ShiftClick => (item.get_max_stack(), true),
    };
    InventoryAction::TakeFromCatalog {
        item,
        count,
        to_inventory,
    }
}

/// The line of the tooltip of a catalog item in Creative: what each gesture takes.
pub fn creative_hint(item: ItemId) -> String {
    format!("LMB {}  RMB 1  Shift to inventory", item.get_max_stack())
}

/// The toast saying that `stack` was destroyed.
pub fn destroyed_message(stack: ItemStack) -> String {
    format!("Destroyed {} {}", stack.nb, item_name(stack.item_id))
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

    fn take(item: ItemId, count: u32, to_inventory: bool) -> InventoryAction {
        InventoryAction::TakeFromCatalog {
            item,
            count,
            to_inventory,
        }
    }

    #[test]
    fn a_click_takes_a_full_stack_and_a_right_click_one() {
        assert_eq!(
            creative_action(ItemId::Dirt, Gesture::Click, None),
            take(ItemId::Dirt, 64, false)
        );
        assert_eq!(
            creative_action(ItemId::WoodenPickaxe, Gesture::Click, None),
            take(ItemId::WoodenPickaxe, 1, false)
        );
        assert_eq!(
            creative_action(ItemId::Dirt, Gesture::RightClick, None),
            take(ItemId::Dirt, 1, false)
        );
    }

    #[test]
    fn holding_a_stack_a_click_on_the_catalog_destroys_it() {
        let held = Some(stack(ItemId::Dirt, 5));
        for (item, gesture) in [
            (ItemId::Dirt, Gesture::Click),
            (ItemId::Dirt, Gesture::ShiftClick),
            (ItemId::Stone, Gesture::Click),
            (ItemId::Stone, Gesture::RightClick),
        ] {
            assert_eq!(
                creative_action(item, gesture, held),
                InventoryAction::DestroyFloating,
                "{item:?} {gesture:?}"
            );
        }
    }

    #[test]
    fn a_right_click_on_the_held_item_adds_one() {
        assert_eq!(
            creative_action(
                ItemId::Dirt,
                Gesture::RightClick,
                Some(stack(ItemId::Dirt, 5))
            ),
            take(ItemId::Dirt, 1, false)
        );
    }

    #[test]
    fn shift_click_sends_a_full_stack_to_the_inventory() {
        assert_eq!(
            creative_action(ItemId::Sand, Gesture::ShiftClick, None),
            take(ItemId::Sand, 64, true)
        );
        assert_eq!(
            creative_action(ItemId::WoodenAxe, Gesture::ShiftClick, None),
            take(ItemId::WoodenAxe, 1, true)
        );
    }

    #[test]
    fn the_texts_of_creative() {
        assert_eq!(
            creative_hint(ItemId::Dirt),
            "LMB 64  RMB 1  Shift to inventory"
        );
        assert_eq!(
            creative_hint(ItemId::WoodenSword),
            "LMB 1  RMB 1  Shift to inventory"
        );
        assert_eq!(
            destroyed_message(stack(ItemId::Dirt, 64)),
            "Destroyed 64 Dirt"
        );
    }
}
