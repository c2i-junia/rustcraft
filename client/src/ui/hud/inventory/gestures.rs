//! The gestures of the mouse on the cells of the open inventory that last longer than a press: a
//! drag of the held stack over cells, sent once its button is released, the double clicks, and
//! the Mouse Tweaks sweep of Shift+drag, which quick-moves every cell it enters, and its wheel,
//! which moves one item a notch.

use std::time::Duration;

use bevy::input::mouse::MouseWheel;
use bevy::picking::hover::Hovered;
use bevy::prelude::*;
use shared::players::inventory::{drag_plan, DragPlan, InventoryAction, OpenContents};
use shared::players::Inventory;
use shared::world::{ItemId, ItemStack};

use super::{stack_in, InventoryCell, InventoryRoot};
use crate::network::buffered_client::CurrentFrameInputs;
use crate::ui::settings::{InterfaceSettings, InventoryTweaksChoice};
use crate::ui::widgets::scroll::scroll_steps;
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

/// A sweep of the pointer with Shift and the left button down, from a Shift+press on a cell with
/// an empty cursor: every cell it enters is quick-moved, once.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShiftDrag {
    /// The slots quick-moved, bit `n` for slot `n`: the one pressed, and those entered since.
    pub moved: u128,
    /// The slots holding a stack at the press, as the client saw them, noted in the frame of the
    /// press, before the prediction applies it: a stack the sweep moved into a cell stays there.
    stacks: Option<u128>,
}

impl ShiftDrag {
    fn new(slot: u32) -> Self {
        ShiftDrag {
            moved: 1 << slot,
            stacks: None,
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
    /// The Shift+drag whose button is down.
    pub shift_drag: Option<ShiftDrag>,
    last_press: Option<LastPress>,
    /// The part of a notch of the wheel the trackpad made over a stack, `scroll_steps`'.
    wheel: f32,
}

impl CellGestures {
    /// What a press of `button` on the cell of `slot`, at `now`, sends at once, the cursor
    /// holding `held` and the slot `in_slot`. With a held stack it starts a drag and sends
    /// nothing, unless it ends a double click; Shift turns the left button into a move of the
    /// whole stack, which starts a Shift+drag when the cursor is empty.
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
            if held.is_none() {
                self.shift_drag = Some(ShiftDrag::new(slot));
            }
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

/// While a Shift+drag's button and Shift stay down, quick-moves each cell the pointer enters that
/// held a stack at the press and still holds one, once; a cell sent forgets the press, which is
/// then no first click of a double click. The Shift+drag ends when the button or Shift is
/// released, the right button is pressed, the cursor holds a stack, Escape is pressed or the
/// inventory is hidden, and at once while the inventory tweaks are off.
#[allow(clippy::too_many_arguments)]
pub fn shift_drag_inventory_cells_system(
    mouse_input: Res<ButtonInput<MouseButton>>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    settings: Res<InterfaceSettings>,
    inventory_root: Query<&Visibility, With<InventoryRoot>>,
    cells: Query<(&Hovered, &InventoryCell)>,
    inventory: Res<Inventory>,
    world_map: Res<ClientWorldMap>,
    mut gestures: ResMut<CellGestures>,
    mut frame_inputs: ResMut<CurrentFrameInputs>,
) {
    let Some(mut drag) = gestures.shift_drag else {
        return;
    };
    let is_open = matches!(inventory_root.single(), Ok(&Visibility::Visible));
    let goes_on = is_open
        && settings.inventory_tweaks == InventoryTweaksChoice::On
        && mouse_input.pressed(MouseButton::Left)
        && !mouse_input.just_pressed(MouseButton::Right)
        && keyboard_input.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight])
        && !keyboard_input.just_pressed(KeyCode::Escape)
        && inventory.floating.is_none();
    if !goes_on {
        gestures.shift_drag = None;
        return;
    }
    let holds_a_stack = |slot| stack_in(slot, &inventory, &world_map).is_some();
    let stacks = *drag.stacks.get_or_insert_with(|| {
        cells
            .iter()
            .filter(|(_, cell)| holds_a_stack(cell.id))
            .fold(0, |stacks, (_, cell)| stacks | 1 << cell.id)
    });
    for (hovered, cell) in &cells {
        let bit = 1 << cell.id;
        if hovered.0 && drag.moved & bit == 0 && stacks & bit != 0 && holds_a_stack(cell.id) {
            drag.moved |= bit;
            frame_inputs
                .0
                .inventory_actions
                .push(InventoryAction::QuickMoveSlot(cell.id));
            gestures.forget_press();
        }
    }
    gestures.shift_drag = Some(drag);
}

/// Sends a notch of the wheel over a cell holding a stack, the cursor empty, as the move of one
/// item of it, either way the wheel turns: at most as many a frame as the stack holds items. The
/// part of a notch made elsewhere is dropped. Nothing while the inventory is hidden or the
/// inventory tweaks are off.
#[allow(clippy::too_many_arguments)]
pub fn wheel_inventory_cells_system(
    mut scroll: MessageReader<MouseWheel>,
    settings: Res<InterfaceSettings>,
    inventory_root: Query<&Visibility, With<InventoryRoot>>,
    cells: Query<(&Hovered, &InventoryCell)>,
    inventory: Res<Inventory>,
    world_map: Res<ClientWorldMap>,
    mut gestures: ResMut<CellGestures>,
    mut frame_inputs: ResMut<CurrentFrameInputs>,
) {
    let is_open = matches!(inventory_root.single(), Ok(&Visibility::Visible));
    let target = cells
        .iter()
        .find(|(hovered, _)| hovered.0)
        .map(|(_, cell)| cell.id)
        .filter(|_| {
            is_open
                && settings.inventory_tweaks == InventoryTweaksChoice::On
                && inventory.floating.is_none()
        })
        .and_then(|slot| Some((slot, stack_in(slot, &inventory, &world_map)?)));
    let Some((slot, stack)) = target else {
        scroll.clear();
        gestures.wheel = 0.;
        return;
    };
    let remainder = &mut gestures.wheel;
    let notches: u32 = scroll
        .read()
        .map(|event| scroll_steps(remainder, event.unit, event.y).unsigned_abs())
        .sum();
    let moves = std::iter::repeat_n(
        InventoryAction::QuickMoveOne(slot),
        notches.min(stack.nb) as usize,
    );
    frame_inputs.0.inventory_actions.extend(moves);
}
