//! What the player sees of a refused inventory action: the slot it targeted shakes.

use bevy::prelude::*;
use shared::players::inventory::InventoryAction;

use super::{CraftResultCell, InventoryCell};
use crate::player::InventoryActionPredicted;
use crate::ui::widgets::slot::ActionRefused;

/// A slot of the panel an inventory action targets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RefusedTarget {
    /// The cell of this slot.
    Cell(u32),
    /// The result of the crafting grid.
    Result,
}

/// The slots that shake when `action` is refused: the slot it acts on, every slot of a drag,
/// the result for the craft actions, and none for the actions on no slot.
pub fn refused_targets(action: InventoryAction) -> Vec<RefusedTarget> {
    match action {
        InventoryAction::LeftClickSlot(slot)
        | InventoryAction::RightClickSlot(slot)
        | InventoryAction::QuickMoveSlot(slot)
        | InventoryAction::QuickMoveOne(slot)
        | InventoryAction::SwapWithHotbar { slot, .. }
        | InventoryAction::MoveAllOfType { slot, .. }
        | InventoryAction::DropSlot { slot, .. } => vec![RefusedTarget::Cell(slot)],
        InventoryAction::Drag { slots, .. } => (0..u128::BITS)
            .filter(|slot| slots & 1 << slot != 0)
            .map(RefusedTarget::Cell)
            .collect(),
        InventoryAction::TakeCraftResult
        | InventoryAction::CraftAll
        | InventoryAction::CraftToHotbar(_) => vec![RefusedTarget::Result],
        InventoryAction::Close
        | InventoryAction::OpenCraftingTable(_)
        | InventoryAction::OpenChest(_)
        | InventoryAction::OpenFurnace(_)
        | InventoryAction::TakeFromCatalog { .. }
        | InventoryAction::DestroyFloating
        | InventoryAction::FillCraftingGrid { .. }
        | InventoryAction::CollectAll
        | InventoryAction::DropFloating { .. } => Vec::new(),
    }
}

/// Reports each inventory action the prediction refused, with the cells it targeted: one
/// refusal per cell, or one of no slot when it targeted none, or a slot no cell shows.
pub fn report_refused_actions(
    mut predicted: MessageReader<InventoryActionPredicted>,
    cells: Query<(Entity, &InventoryCell)>,
    result: Query<Entity, With<CraftResultCell>>,
    mut refusals: MessageWriter<ActionRefused>,
) {
    for InventoryActionPredicted(outcome) in predicted.read() {
        if outcome.result.is_ok() {
            continue;
        }
        let targets = refused_targets(outcome.action);
        if targets.is_empty() {
            refusals.write(ActionRefused { slot: None });
        }
        for target in targets {
            let slot = match target {
                RefusedTarget::Cell(id) => cells
                    .iter()
                    .find(|(_, cell)| cell.id == id)
                    .map(|(entity, _)| entity),
                RefusedTarget::Result => result.iter().next(),
            };
            refusals.write(ActionRefused { slot });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::players::inventory::{InventoryActionOutcome, InventoryRefusal};
    use shared::world::ItemId;

    #[test]
    fn a_refused_action_targets_its_slot_the_slots_of_its_drag_or_the_result() {
        use InventoryAction as A;
        use RefusedTarget::{Cell, Result};
        let cases = [
            (A::LeftClickSlot(3), vec![Cell(3)]),
            (A::RightClickSlot(4), vec![Cell(4)]),
            (A::QuickMoveSlot(5), vec![Cell(5)]),
            (A::QuickMoveOne(9), vec![Cell(9)]),
            (A::SwapWithHotbar { slot: 6, hotbar: 0 }, vec![Cell(6)]),
            (
                A::MoveAllOfType {
                    slot: 7,
                    item: ItemId::Dirt,
                },
                vec![Cell(7)],
            ),
            (
                A::Drag {
                    slots: 1 << 2 | 1 << 40 | 1 << 127,
                    one_each: false,
                },
                vec![Cell(2), Cell(40), Cell(127)],
            ),
            (A::TakeCraftResult, vec![Result]),
            (A::CraftAll, vec![Result]),
            (A::CraftToHotbar(9), vec![Result]),
            (A::DropSlot { slot: 8, all: true }, vec![Cell(8)]),
            (A::DropFloating { all: false }, vec![]),
            (A::OpenChest(IVec3::ZERO), vec![]),
            (A::OpenCraftingTable(IVec3::ZERO), vec![]),
            (A::OpenFurnace(IVec3::ZERO), vec![]),
            (A::Close, vec![]),
            (A::CollectAll, vec![]),
            (A::DestroyFloating, vec![]),
            (
                A::FillCraftingGrid {
                    recipe: 0,
                    times: 1,
                },
                vec![],
            ),
            (
                A::TakeFromCatalog {
                    item: ItemId::Dirt,
                    count: 1,
                    to_inventory: false,
                },
                vec![],
            ),
        ];
        for (action, targets) in cases {
            assert_eq!(refused_targets(action), targets, "{action:?}");
        }
    }

    fn predicted(
        action: InventoryAction,
        result: Result<(), InventoryRefusal>,
    ) -> InventoryActionPredicted {
        InventoryActionPredicted(InventoryActionOutcome {
            action,
            held_before: None,
            held_after: None,
            result,
            thrown: None,
            crafted: None,
        })
    }

    #[test]
    fn each_refused_action_reports_the_cells_it_targeted() {
        let mut app = App::new();
        app.add_message::<InventoryActionPredicted>()
            .add_message::<ActionRefused>()
            .add_systems(Update, report_refused_actions);
        let cell = |app: &mut App, id| app.world_mut().spawn(InventoryCell { id }).id();
        let (zero, one, two, three) = (
            cell(&mut app, 0),
            cell(&mut app, 1),
            cell(&mut app, 2),
            cell(&mut app, 3),
        );
        let result = app.world_mut().spawn(CraftResultCell).id();
        let refused = Err(InventoryRefusal::NoRoom);
        app.world_mut().write_message_batch([
            predicted(InventoryAction::TakeCraftResult, refused),
            predicted(InventoryAction::LeftClickSlot(0), Ok(())),
            predicted(InventoryAction::LeftClickSlot(3), refused),
            predicted(InventoryAction::OpenChest(IVec3::ZERO), refused),
            predicted(
                InventoryAction::Drag {
                    slots: 1 << 1 | 1 << 2,
                    one_each: true,
                },
                refused,
            ),
            // No cell on screen holds this slot: the refusal is heard all the same
            predicted(InventoryAction::RightClickSlot(50), refused),
        ]);

        app.update();

        let reported: Vec<_> = app
            .world_mut()
            .resource_mut::<Messages<ActionRefused>>()
            .drain()
            .map(|refused| refused.slot)
            .collect();
        assert_eq!(
            reported,
            [Some(result), Some(three), None, Some(one), Some(two), None]
        );
        assert!(!reported.contains(&Some(zero)));
    }
}
