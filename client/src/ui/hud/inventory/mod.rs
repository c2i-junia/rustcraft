//! The inventory panel, whose bottom row is the hotbar.

use bevy::prelude::Component;
use shared::world::ItemId;

mod display;
mod drop;
mod feedback;
mod furnace;
mod gestures;
mod ghost;
#[cfg(test)]
mod goldens;
mod setup;

pub use display::*;
pub use drop::*;
pub use feedback::*;
pub use furnace::*;
pub use gestures::*;
pub use ghost::*;
pub use setup::*;

/// The name of an item as a player reads it: its identifier split into words.
pub fn item_name(item: ItemId) -> String {
    let mut name = String::new();
    for letter in format!("{item:?}").chars() {
        if letter.is_uppercase() && !name.is_empty() {
            name.push(' ');
        }
        name.push(letter);
    }
    name
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

/// The crafting grid, its arrow and its result: hidden while a chest or a furnace is open.
#[derive(Component)]
pub struct CraftingRow;

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
