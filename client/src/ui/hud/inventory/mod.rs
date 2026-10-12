//! The inventory dialog and the cells it shares with the hotbar.

use bevy::prelude::Component;
use shared::world::ItemStack;

mod display;
pub mod items;
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

/// The current selected stack, not considered in the player's inventory
#[derive(Component)]
pub struct FloatingStack {
    pub items: Option<ItemStack>,
}

/// The text showing how many items a cell or the floating stack holds.
#[derive(Component)]
pub struct StackCount;

/// The image of the item a cell or the floating stack holds.
#[derive(Component)]
pub struct StackIcon;
