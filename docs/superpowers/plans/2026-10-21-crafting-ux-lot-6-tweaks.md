# Crafting UX Lot 6 (Mouse Tweaks gestures) Implementation Plan

**Goal:** The two gestures of the Mouse Tweaks mod that seasoned players have in their hands:
Shift held, a press on a cell with an empty cursor quick-moves it, and while the button stays
down every cell the pointer enters is quick-moved too; over a cell holding a stack, with an
empty cursor, each notch of the wheel moves one item of it to where a shift-click would send
the stack. A switch `Inventory tweaks` on the Controls tab, on by default, turns both off. To
see: open a chest, hold Shift and sweep the pointer, button down, over a row of the backpack:
every stack goes into the chest; then hover a stack and roll the wheel: it goes one item a
notch.

**Spec:** `docs/superpowers/specs/2026-10-21-crafting-ux-design.md`, "Global constraints",
"Task 6 — Lot 6: Mouse Tweaks gestures", and its "Decisions" (lots 1 to 5 bind this lot,
notably 10 and 11 on Shift and the double clicks, 13 on where the gestures live, 17 and 18 on
the feedback, 28 on a slot action with a held stack, 35 on presses outside the panel).

## Global constraints

- English for code, comments, docs and commits; Conventional Commits, no trailer.
- Doc comments in full sentences, tests named as sentences, sparse comments.
- TDD: each test is written and seen failing (not compiling counts) before the code that
  makes it pass. Every commit compiles and its tests pass.
- `QuickMoveOne(u32)` is appended after `DropFloating`; `PROTOCOL_ID` stays 25. The pinned
  encoding gets 19.
- Every rule stays in `shared`: where one item goes, and when it is refused. The client only
  chooses which actions to send.
- `click_inventory_cells_system` (14 of 16 params) gets no new parameter: the shift-drag starts
  in `CellGestures::press` and goes on in a system of its own in `gestures.rs`, beside the wheel.
- `cargo test -p shared <filter>`, `-p client <filter>` while working, under `timeout`;
  `timeout 1500 just check` at the end.

## Decisions (where the spec is silent)

1. **QuickMoveOne rules.** `QuickMoveOne(slot)`: `slot` must pass `check_slot`, else
   `InvalidSlot`; the cursor must be empty, else `HoldingAStack` (as `DropSlot`, Decision 28);
   an empty slot is no refusal and moves nothing. Else one item of the stack goes where
   `QuickMoveSlot` would send the whole stack, by the same code: `quick_move_slot` takes a
   count (`u32::MAX` for the stack) and returns how many items it moved. When no slot there
   has room for the item (a full hotbar, a furnace for what neither smelts nor burns) it is
   refused with `NoRoom`, where `QuickMoveSlot` keeps the stack silently: a wheel notch that
   does nothing says why. The item keeps its stack's type.
2. **Feedback.** An accepted `QuickMoveOne` is silent, as `QuickMoveSlot` (no item goes through
   the cursor, Decision 17); a refused one shakes its cell with the refused thud (Decision 18).
3. **Shift+drag.** A Shift+left press on a cell, the cursor empty as the client sees it, starts
   a shift-drag (`CellGestures::shift_drag`); the press sends what it sent before
   (`QuickMoveSlot`, or `MoveAllOfType` for the second press of a Shift+double click). While the
   left button and Shift stay down, each cell the pointer enters is sent as `QuickMoveSlot`,
   once a drag, when it held a stack at the press and still holds one, as the client sees them:
   a stack the drag itself moved into a cell is not sent back. The cells holding a stack at the
   press are noted by the drag's system in the frame of the press, which runs before the
   prediction applies it. The drag ends when the left button or Shift is released, the right
   button is pressed, the cursor holds a stack, Escape is pressed or the inventory hides, or the
   switch is off. The result is not a cell: a Shift+press on it is `CraftAll` and starts
   nothing. A Shift+press with a held stack is the shift-click of today and starts nothing.
4. **With the Shift+double click.** The press of a shift-drag is lot 1's shift-click, recorded
   for the double click (Decision 10). A cell entered and sent forgets that press: a press
   after a sweep over other cells is a new Shift+press, not the second of a double click. The
   second press of a Shift+double click (`MoveAllOfType`) starts a shift-drag too; a cell it
   emptied is not sent again, since it holds nothing.
5. **Wheel.** Either direction: each notch over a cell holding a stack, the cursor empty and the
   inventory open, sends one `QuickMoveOne(slot)`. A notch is a line of the wheel, or
   `SCROLL_PIXELS_PER_STEP` of a trackpad, counted by the shared `scroll_steps`; the part of a
   notch left over is dropped when the wheel turns elsewhere than over a stack. At most as many
   actions a frame as the stack holds items. Pulling items into the slot (Mouse Tweaks' other
   direction) is not part of this lot. The browser's lists scroll by their own
   `Pointer<Scroll>` observers, and a cell is never hovered with them: the wheel over the
   browser keeps scrolling it. The hotbar's wheel keeps its `UIMode::Closed` gate (the open
   inventory is a `UiDialog`), which a test pins.
6. **The switch.** `InterfaceSettings::inventory_tweaks: InventoryTweaksChoice { Off, On }`,
   `On` by default, saved in `interface.ron` with the other settings at once when chosen (the
   keybindings file holds keys only); a file saved before it loads `On` (the struct's
   `serde(default)`). On the Controls tab, a row under the list of actions: the label
   `Inventory tweaks` and an `Off` / `On` segmented control, as the Minimap row of the
   Interface tab, then a muted note `Shift+drag: stacks  Wheel: one item`. Off, a
   Shift+press is the plain shift-click and the wheel does nothing in the inventory. The
   golden of the Controls tab changes: the list loses a row of height.

## Steps

### 0. Plan

- This file. Commit `docs: plan of the Mouse Tweaks gestures (lot 6)`.

### 1. Shared: QuickMoveOne

- RED, in `shared/src/players/inventory.rs`:
  `quick_move_one_moves_one_item_where_a_shift_click_sends_the_stack` (hotbar to backpack,
  topping a stack up first; backpack to hotbar; a grid cell to the inventory; the last item
  empties its slot), `quick_move_one_moves_between_the_chest_or_the_furnace_and_the_inventory`
  (inventory to chest and back; coal to the fuel, sand to the input, the output to the
  inventory), `quick_move_one_is_refused_out_of_reach_with_a_held_stack_or_without_room`
  (`InvalidSlot`, `HoldingAStack`, `NoRoom` for a full hotbar and for dirt by a furnace, nothing
  changed), `quick_move_one_on_an_empty_slot_moves_nothing`; in `shared/src/messages/mod.rs`
  the pinned index 19 and a round trip.
- GREEN: the variant, `quick_move_slot` with a count, `quick_move_one`; the client's two
  exhaustive matches (silent sound, its cell shakes) with their test tables.
- Commit `feat(shared): QuickMoveOne moves one item where a shift-click sends the stack`.

### 2. Client: the switch, saved

- RED, in `client/src/ui/settings.rs`: `the_inventory_tweaks_are_on_unless_saved_off` (no file,
  a file without the field, `(inventory_tweaks: Off)`), the round trip test with the field.
- GREEN: `InventoryTweaksChoice`, the field.
- Commit `feat(client): an inventory tweaks setting, on by default`.

### 3. Client: Shift+drag

- RED, in the tests of `display.rs` (the `Hud` harness, whose frame runs the new systems):
  `a_shift_drag_quick_moves_each_cell_entered_once`,
  `a_shift_drag_leaves_the_cells_empty_at_its_press`,
  `a_shift_drag_ends_with_the_button_shift_the_other_button_or_a_held_stack`,
  `a_shift_press_with_a_held_stack_starts_no_shift_drag`,
  `a_shift_drag_over_other_cells_is_no_first_click_of_a_double_click`,
  `the_second_press_of_a_shift_double_click_starts_a_shift_drag`,
  `with_the_tweaks_off_a_shift_press_is_a_shift_click`.
- GREEN: `ShiftDrag`, `CellGestures::shift_drag`, `press` starting it,
  `shift_drag_inventory_cells_system` chained after `drag_inventory_cells_system`.
- Commit `feat(client): Shift+drag quick-moves every cell the pointer enters`.

### 4. Client: the wheel

- RED: `a_wheel_notch_over_a_stack_moves_one_item_of_it` (lines both ways, trackpad pixels
  adding up, capped at the stack), `the_wheel_moves_nothing_from_an_empty_cell_with_a_held_stack
  _closed_or_with_the_tweaks_off`; in `hotbar.rs`
  `the_wheel_leaves_the_hotbar_selection_alone_while_a_dialog_is_open` (a pin: it passes at
  once).
- GREEN: `wheel_inventory_cells_system`, chained after the shift-drag.
- Commit `feat(client): a wheel notch over a stack moves one item of it`.

### 5. Client: the switch on the Controls tab

- RED, in `client/src/e2e/settings.rs`: `the_inventory_tweaks_switch_is_saved_and_honoured`
  (Controls tab: `On` checked; `Off` clicked is saved; in a world, a Shift+sweep moves only the
  pressed stack and the wheel moves nothing).
- GREEN: the row under the list (`spawn_controls_tab` takes the `InterfaceSettings`), the
  observer saving the choice.
- Regenerate the golden of the Controls tab (`UPDATE_GOLDEN=1`, that test alone), look at it;
  run the other settings e2e tests (the Tab order, the files left alone).
- Commit `feat(client): an Inventory tweaks switch on the Controls tab`.

### 6. E2E

- `client/src/e2e/gestures.rs`: `a_shift_drag_moves_every_stack_swept_into_the_chest` (three
  stacks swept, a cell entered twice, an empty cell, then the pointer up onto the chest cell the
  first stack went to: it stays there), `the_wheel_moves_one_item_a_notch_and_leaves_the
  _hotbar_selection` (three notches from a hotbar stack into the backpack, the selection
  unchanged, the wheel over the browser scrolls its list and moves nothing).
- Commit `test(client): e2e of the Mouse Tweaks gestures`.

### 7. Decisions and check

- Append the rulings above to the spec's Decisions, as `(Lot 6)` items 46 and on.
- `timeout 1500 just check`; re-run alone any failure that looks unrelated.
- Commit `docs: decisions of the Mouse Tweaks gestures (lot 6)`.
