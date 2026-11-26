//! The inventory panel, whose bottom row is the hotbar.

use bevy::prelude::Component;
use shared::world::ItemId;

mod anvil;
mod armour;
mod display;
mod drop;
mod engraving;
mod feedback;
mod furnace;
mod gestures;
mod ghost;
#[cfg(test)]
mod goldens;
mod setup;
mod trade;

pub use anvil::*;
pub use armour::*;
pub use display::*;
pub use drop::*;
pub use engraving::*;
pub use feedback::*;
pub use furnace::*;
pub use gestures::*;
pub use ghost::*;
pub use setup::*;
pub use trade::*;

/// The name of an item as a player reads it, in the language of the game.
pub fn item_name(item: ItemId) -> String {
    crate::i18n::item_name(item).to_string()
}

/// The name of a stack as a player reads it: the one an anvil gave it, else that of its item.
pub fn stack_name(stack: &shared::world::ItemStack) -> String {
    match stack.custom_name() {
        Some(name) => name.to_string(),
        None => item_name(stack.item_id),
    }
}

/// The node over the whole window holding the panel, at the bottom and centred, and the item
/// browser.
#[derive(Component)]
pub struct HudBottom;

/// The upper part of the panel, above the hotbar: shown while the inventory is open.
#[derive(Component)]
pub struct InventoryRoot;

/// The panel holding the inventory and the hotbar. It is drawn only while the inventory is open.
#[derive(Component)]
pub struct InventoryDialog;

/// A slot of the player's inventory, in the hotbar or in the inventory panel.
#[derive(Component)]
pub struct InventoryCell {
    pub id: u32,
}

/// The title of the inventory panel, which names the crafting table while one is open.
#[derive(Component)]
pub struct InventoryTitle;

/// A cell of the crafting grid, shown when the grid in use reaches it.
#[derive(Component)]
pub struct CraftingGridCell {
    pub row: u32,
    pub col: u32,
}

/// The crafting grid, its arrow and its result: hidden while a chest, a furnace, an
/// engraving table, an anvil or a trade is open.
#[derive(Component)]
pub struct CraftingRow;

/// The slots, the name and the work of the open anvil: shown while one is open.
#[derive(Component)]
pub struct AnvilRow;

/// The slot and the enchantments of the open engraving table: shown while one is open.
#[derive(Component)]
pub struct EngravingRow;

/// The offers of the open trade: shown while one is open.
#[derive(Component)]
pub struct TradeGrid;

/// The 3x9 slots of the open chest, above the backpack: shown while a chest is open.
#[derive(Component)]
pub struct ChestGrid;

/// The slots, the flame and the arrow of the open furnace, above the backpack: shown while a
/// furnace is open.
#[derive(Component)]
pub struct FurnaceRow;

/// The cell showing what the crafting grid makes; a click on it takes it.
#[derive(Component)]
pub struct CraftResultCell;

/// Shows the stack held by the cursor, `Inventory::floating`.
#[derive(Component)]
pub struct FloatingStack;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn item_names_are_split_into_words() {
        for (item, name) in [
            (ItemId::Dirt, "Dirt"),
            (ItemId::OakPlanks, "Oak Planks"),
            (ItemId::CraftingTable, "Crafting Table"),
            (ItemId::TallGrass, "Tall Grass"),
        ] {
            assert_eq!(item_name(item), name);
        }
    }
}
