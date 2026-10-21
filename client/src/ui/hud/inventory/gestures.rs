//! The gestures of the mouse on the cells of the open inventory that last longer than a press: a
//! drag of the held stack over cells, sent once its button is released, and the double clicks.

use std::time::Duration;

use bevy::picking::hover::Hovered;
use bevy::prelude::*;
use shared::players::inventory::{drag_plan, DragPlan, InventoryAction, OpenContents};
use shared::players::Inventory;
use shared::world::{ItemId, ItemStack};

use super::{InventoryCell, InventoryRoot};
use crate::network::buffered_client::CurrentFrameInputs;
use crate::world::ClientWorldMap;

/// How soon after a press a second one on the same cell makes a double click.
pub const DOUBLE_CLICK_WINDOW: Duration = Duration::from_millis(250);

/// A drag of the stack held by the cursor, from a press on a cell until its button is released.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CellDrag {
    /// The left button splits the stack, the right one drops one item per slot.
    pub button: MouseButton,
    /// The slot pressed, which a release over no other one clicks.
    pub start: u32,
    /// The slots of the drag, bit `n` for slot `n`: the one pressed, and those entered since that
    /// the drag puts items into.
    pub slots: u128,
}

impl CellDrag {
    pub fn one_each(&self) -> bool {
        self.button == MouseButton::Right
    }

    /// Takes `slot` in when the drag with it puts items into it, while the drag puts items into
    /// fewer slots than the cursor holds, `held`: no slot entered takes another's item away.
    fn join(&mut self, slot: u32, held: u32, plan: impl Fn(u128) -> Option<DragPlan>) {
        let with = self.slots | 1 << slot;
        if with == self.slots {
            return;
        }
        let filled = plan(self.slots).map_or(0, |plan| plan.slots.len());
        let fills_it = plan(with).is_some_and(|plan| plan.slots.iter().any(|(s, _)| *s == slot));
        if filled < held as usize && fills_it {
            self.slots = with;
        }
    }

    /// What its release sends: the plain click over a single slot, else the drag.
    fn release(&self) -> InventoryAction {
        match (self.slots.count_ones(), self.button) {
            (1, MouseButton::Left) => InventoryAction::LeftClickSlot(self.start),
            (1, _) => InventoryAction::RightClickSlot(self.start),
            _ => InventoryAction::Drag {
                slots: self.slots,
                one_each: self.one_each(),
            },
        }
    }
}

/// What a left press on a cell did, which a second one soon after on the same cell completes.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Pressed {
    /// It picked a stack up: a double click gathers its item.
    PickedUp,
    /// Shift+click, which moved this item: a Shift+double click moves all of it.
    ShiftMoved(ItemId),
}

#[derive(Clone, Copy, Debug)]
struct LastPress {
    slot: u32,
    at: Duration,
    pressed: Pressed,
}

/// The gestures under way on the cells of the open inventory.
#[derive(Resource, Default, Debug)]
pub struct CellGestures {
    /// The drag whose button is down.
    pub drag: Option<CellDrag>,
    last_press: Option<LastPress>,
}

impl CellGestures {
    /// What a press of `button` on the cell of `slot`, at `now`, sends at once, the cursor
    /// holding `held` and the slot `in_slot`. With a held stack it starts a drag and sends
    /// nothing, unless it ends a double click; Shift turns the left button into a move of the
    /// whole stack.
    pub fn press(
        &mut self,
        slot: u32,
        button: MouseButton,
        shift: bool,
        held: Option<ItemStack>,
        in_slot: Option<ItemStack>,
        now: Duration,
    ) -> Option<InventoryAction> {
        let last = self
            .last_press
            .take()
            .filter(|last| last.slot == slot && now.saturating_sub(last.at) <= DOUBLE_CLICK_WINDOW)
            .map(|last| last.pressed);
        let left = button == MouseButton::Left;
        let remember = |pressed| LastPress {
            slot,
            at: now,
            pressed,
        };
        if left && shift {
            if let Some(Pressed::ShiftMoved(item)) = last {
                return Some(InventoryAction::MoveAllOfType { slot, item });
            }
            self.last_press = in_slot.map(|stack| remember(Pressed::ShiftMoved(stack.item_id)));
            return Some(InventoryAction::QuickMoveSlot(slot));
        }
        if held.is_some() {
            if left && last == Some(Pressed::PickedUp) {
                return Some(InventoryAction::CollectAll);
            }
            self.drag = Some(CellDrag {
                button,
                start: slot,
                slots: 1 << slot,
            });
            return None;
        }
        if !left {
            return Some(InventoryAction::RightClickSlot(slot));
        }
        if in_slot.is_some() {
            self.last_press = Some(remember(Pressed::PickedUp));
        }
        Some(InventoryAction::LeftClickSlot(slot))
    }

    /// Forgets the last press on a cell: another press came between.
    pub fn forget_press(&mut self) {
        self.last_press = None;
    }
}

/// What a drag over `slots` would leave, with the open chest or furnace the server sent: the
/// shared `drag_plan`, as the prediction will apply it.
pub(crate) fn planned_drag(
    inventory: &Inventory,
    world_map: &ClientWorldMap,
    slots: u128,
    one_each: bool,
) -> Option<DragPlan> {
    let mut chest = inventory
        .open_chest
        .and_then(|pos| world_map.blocks.container(pos))
        .cloned();
    let mut furnace = inventory
        .open_furnace
        .and_then(|pos| world_map.blocks.furnace(pos))
        .cloned();
    let contents = match (&mut chest, &mut furnace) {
        (Some(chest), _) => Some(OpenContents::Chest(chest)),
        (None, Some(furnace)) => Some(OpenContents::Furnace(furnace)),
        (None, None) => None,
    };
    // The server checks that the table is within reach; the panel follows its updates
    drag_plan(inventory, slots, one_each, |_, _| true, contents).ok()
}

/// While a drag's button is down, takes in the cells the pointer enters, and sends the drag once
/// it is released. Closing the inventory, Escape, or the loss of the held stack cancels it.
pub fn drag_inventory_cells_system(
    mouse_input: Res<ButtonInput<MouseButton>>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    inventory_root: Query<&Visibility, With<InventoryRoot>>,
    cells: Query<(&Hovered, &InventoryCell)>,
    inventory: Res<Inventory>,
    world_map: Res<ClientWorldMap>,
    mut gestures: ResMut<CellGestures>,
    mut frame_inputs: ResMut<CurrentFrameInputs>,
) {
    let Some(mut drag) = gestures.drag else {
        return;
    };
    let is_open = matches!(inventory_root.single(), Ok(&Visibility::Visible));
    let held = inventory
        .floating
        .filter(|_| is_open && !keyboard_input.just_pressed(KeyCode::Escape));
    let Some(held) = held else {
        gestures.drag = None;
        return;
    };
    let one_each = drag.one_each();
    for (hovered, cell) in &cells {
        if hovered.0 {
            drag.join(cell.id, held.nb, |slots| {
                planned_drag(&inventory, &world_map, slots, one_each)
            });
        }
    }
    if mouse_input.pressed(drag.button) {
        gestures.drag = Some(drag);
    } else {
        gestures.drag = None;
        frame_inputs.0.inventory_actions.push(drag.release());
    }
}
