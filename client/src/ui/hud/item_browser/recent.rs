//! The items crafted lately: a row of the item browser under the bookmarks, newest first, kept
//! for the world being played and not saved.

use bevy::prelude::*;
use shared::world::ItemId;

use super::bookmarks::Shelf;
use super::{RecentBar, RecentRow, RecentSlot};
use crate::player::InventoryActionPredicted;

/// How many items the row of recent crafts holds.
pub const RECENT_CRAFTS: usize = 9;

/// The last items the local player crafted in the world being played, newest first, each once.
#[derive(Resource, Default, Clone, PartialEq, Debug)]
pub struct RecentCrafts {
    pub items: Vec<ItemId>,
}

impl RecentCrafts {
    /// `item` was crafted: it goes first, out of its former place, and the oldest beyond
    /// `RECENT_CRAFTS` go.
    pub fn crafted(&mut self, item: ItemId) {
        self.items.retain(|&recent| recent != item);
        self.items.insert(0, item);
        self.items.truncate(RECENT_CRAFTS);
    }
}

impl Shelf for RecentCrafts {
    type Bar = RecentBar;
    type Row = RecentRow;
    type Slot = RecentSlot;

    fn items(&self) -> &[ItemId] {
        &self.items
    }
}

/// Keeps the item of each craft action the prediction accepted, once: the replays of the
/// reconciliation report nothing.
pub fn remember_crafts(
    mut predicted: MessageReader<InventoryActionPredicted>,
    mut recent: ResMut<RecentCrafts>,
) {
    for InventoryActionPredicted(outcome) in predicted.read() {
        if let (Ok(()), Some(item)) = (outcome.result, outcome.crafted) {
            recent.crafted(item);
        }
    }
}

/// A world entered starts with no recent craft: they are not saved.
pub fn forget_recent_crafts(mut recent: ResMut<RecentCrafts>) {
    *recent = RecentCrafts::default();
}

#[cfg(test)]
mod tests {
    use shared::players::inventory::{InventoryAction, InventoryActionOutcome, InventoryRefusal};

    use super::*;

    #[test]
    fn recent_keeps_nine_distinct_items_newest_first() {
        let mut recent = RecentCrafts::default();
        recent.crafted(ItemId::Stick);
        recent.crafted(ItemId::Torch);
        recent.crafted(ItemId::Stick);
        assert_eq!(recent.items, [ItemId::Stick, ItemId::Torch]);
        // The oldest go beyond nine
        let crafted = &ItemId::ALL[..12];
        for &item in crafted {
            recent.crafted(item);
        }
        assert_eq!(RECENT_CRAFTS, 9);
        let newest: Vec<ItemId> = crafted.iter().rev().take(9).copied().collect();
        assert_eq!(recent.items, newest);
    }

    fn outcome(
        action: InventoryAction,
        result: Result<(), InventoryRefusal>,
        crafted: Option<ItemId>,
    ) -> InventoryActionPredicted {
        InventoryActionPredicted(InventoryActionOutcome {
            action,
            held_before: None,
            held_after: None,
            result,
            thrown: None,
            crafted,
        })
    }

    #[test]
    fn an_accepted_craft_goes_into_recent() {
        let mut app = App::new();
        app.add_message::<InventoryActionPredicted>()
            .init_resource::<RecentCrafts>()
            .add_systems(Update, remember_crafts);
        for predicted in [
            outcome(InventoryAction::CraftAll, Ok(()), Some(ItemId::OakPlanks)),
            outcome(
                InventoryAction::TakeCraftResult,
                Err(InventoryRefusal::NoRecipe),
                None,
            ),
            outcome(InventoryAction::LeftClickSlot(3), Ok(()), None),
            outcome(
                InventoryAction::CraftToHotbar(2),
                Ok(()),
                Some(ItemId::Stick),
            ),
        ] {
            app.world_mut().write_message(predicted);
        }
        app.update();
        assert_eq!(
            app.world().resource::<RecentCrafts>().items,
            [ItemId::Stick, ItemId::OakPlanks]
        );
        // Read once
        app.update();
        assert_eq!(app.world().resource::<RecentCrafts>().items.len(), 2);
    }

    #[test]
    fn entering_a_world_empties_recent() {
        let mut app = App::new();
        app.insert_resource(RecentCrafts {
            items: vec![ItemId::Chest],
        })
        .add_systems(Update, forget_recent_crafts);
        app.update();
        assert!(app.world().resource::<RecentCrafts>().items.is_empty());
    }
}
