//! What a chest holds. The contents live in the `ChunkMap`, at the position of the chest: the
//! server holds those of every chest, a client only those of the chest its player has open.

use serde::{Deserialize, Serialize};

use crate::world::ItemStack;

/// Number of slots of a chest: three rows of nine.
pub const CHEST_SLOTS: u32 = 27;

/// The stacks of a chest, slot after slot. A fixed array, so that a message or a file can never
/// announce more slots than a chest has.
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug, Default)]
pub struct Container {
    pub slots: [Option<ItemStack>; CHEST_SLOTS as usize],
}

impl Container {
    /// Whether every slot is empty.
    pub fn is_empty(&self) -> bool {
        self.slots.iter().all(Option::is_none)
    }

    /// The stacks the chest holds, in slot order.
    pub fn stacks(&self) -> impl Iterator<Item = ItemStack> + '_ {
        self.slots.iter().flatten().copied()
    }
}
