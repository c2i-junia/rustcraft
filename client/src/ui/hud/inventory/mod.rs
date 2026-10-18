//! The inventory panel, whose bottom row is the hotbar.

use bevy::prelude::Component;

mod display;
#[cfg(test)]
mod goldens;
mod setup;

pub use display::*;
pub use setup::*;

/// The bottom of the screen, where the panel sits, centred.
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

/// The cell showing what the crafting grid makes; a click on it takes it.
#[derive(Component)]
pub struct CraftResultCell;

/// Shows the stack held by the cursor, `Inventory::floating`.
#[derive(Component)]
pub struct FloatingStack;
