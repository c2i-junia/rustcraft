//! The inventory dialog and the cells it shares with the hotbar.

use bevy::prelude::Component;

mod display;
mod setup;

pub use display::*;
pub use setup::*;

// Marker for Inventory root
#[derive(Component)]
pub struct InventoryRoot;

/// Main inventory dialog
#[derive(Component)]
pub struct InventoryDialog;

/// A slot of the player's inventory, in the hotbar or in the inventory dialog.
#[derive(Component)]
pub struct InventoryCell {
    pub id: u32,
}

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

/// The text showing how many items a cell or the floating stack holds.
#[derive(Component)]
pub struct StackCount;

/// The image of the item a cell or the floating stack holds.
#[derive(Component)]
pub struct StackIcon;
