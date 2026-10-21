# Crafting UX Lot 4 (Dropping items) Implementation Plan

**Goal:** The player throws items out of the inventory as in Minecraft: Q over a slot of the
open inventory throws one item of it, Ctrl+Q its stack; a press outside the panel and the item
browser throws the held stack (left: all of it, right: one); Q in the game throws one item of
the selected hotbar slot, Ctrl+Q its stack. A thrown item lands ahead of the player and cannot
be picked up for 2 s. Fixes C5. To see: in a world with 10 dirt in the hotbar, press Q, then
Ctrl+Q: one dirt, then nine, fly ahead and stay on the ground.

**Spec:** `docs/superpowers/specs/2026-10-21-crafting-ux-design.md`, "Global constraints",
"Task 4 — Lot 4: dropping items", and its "Decisions" (lots 1 to 3 bind this lot).

## Global constraints

- English for code, comments, docs and commits; Conventional Commits, no trailer.
- Doc comments in full sentences, tests named as sentences, sparse comments.
- TDD: each test is written and seen failing (not compiling counts) before the code that
  makes it pass. Every commit compiles and its tests pass.
- `DropSlot` and `DropFloating` are appended after `CraftToHotbar`; `PROTOCOL_ID` stays 25
  (bumped once for the branch in lot 1). The pinned encodings get 17 and 18.
- Every rule stays in `shared`: what a drop takes and what it throws. The client predicts it;
  the server spawns what the outcome says was thrown.
- `cargo test -p shared <filter>`, `-p server`, `-p client` while working, under `timeout`;
  `timeout 1500 just check` at the end.

## Decisions (where the spec is silent)

1. **Rules.** `DropSlot { slot, all }`: `slot` must pass `check_slot` (inventory, usable grid,
   open chest or furnace, a leftover beyond the grid), else `InvalidSlot`; the cursor must be
   empty, else `HoldingAStack` (Minecraft throws from a slot only with an empty cursor). One
   item, or the whole stack with `all`, leaves the slot and is thrown, keeping its type (a
   tool keeps its wear). An empty slot is no refusal and throws nothing, as a click on an
   empty slot with nothing held. `DropFloating { all }`: `NothingHeld` without a floating
   stack; else one item of it, or all, is thrown.
2. **Thrown stacks.** `apply_inventory_action` returns `Result<Option<ItemStack>, _>`: `Ok`
   holds the stack the action threw out, if any. `InventoryActionOutcome` gets
   `thrown: Option<ItemStack>`, and `ActionsOutcome` gets `thrown: Vec<ItemStack>`, beside
   `dropped`: the server must tell what a player threw (the longer delay) from what it had no
   room for (`dropped`, which keeps its delay).
3. **Server.** `DroppedItem` gets `pickup_delay_ticks`, the age from which it can be picked
   up: `PICKUP_DELAY_TICKS` (10) by default, `THROWN_PICKUP_DELAY_TICKS` (40) for a thrown
   item. It is not saved (`serde(skip)`): a saved item takes the default delay on load, and
   the world files do not change. Two items that merge keep the longer of the two waits left.
   `DroppedItems::throw_from_player(stack, player)` throws as `drop_from_player` does, but at
   `THROWN_SPEED` and with the longer delay.
4. **Throw speed.** At today's `THROW_SPEED` (2 blocks/s) an item lands about a block ahead,
   inside the player's pickup reach (1.0 + half its width + half the item): a player standing
   still would get its thrown item back after 2 s. A thrown item flies at `THROWN_SPEED`,
   chosen so that it comes to rest out of reach (a test pins it), as in Minecraft where a
   thrown item lands a few blocks ahead. The overflow keeps `THROW_SPEED`.
5. **Sounds and shakes.** Both drops are silent when accepted, as in Minecraft (the item seen
   flying is the feedback): neither puts items into a slot nor picks up. A refused `DropSlot`
   shakes its cell; a refused `DropFloating` targets no slot (the refused sound alone).
6. **Key.** `GameAction::Drop`, default `KeyQ`, listed right after `ToggleInventory` (Interface:
   `Inventory`, `Drop item`, `Chat`, ...). Ctrl is either Control key, held, not a binding.
   The settings search counts become `of 25`.
7. **Open inventory.** In `click_inventory_cells_system`: Drop goes to the cell a number key
   goes to (the ringed cell while the ring shows, else the hovered one) and sends
   `DropSlot { slot, all: ctrl }` when that cell holds a stack as the client sees it. Q over
   the result does nothing (not asked). Not while typing.
8. **Press outside.** `throw_held_stack_system`, run before `click_inventory_cells_system` (a
   drag the press cancels must still be seen) and before `drawer_keys`: a press of the left
   (`all: true`) or right (`all: false`) button, inventory open, the cursor holding a stack,
   no drag under way, the pointer neither inside the panel (`InventoryDialog`'s rectangle: the
   panel is not pickable) nor over the item browser or its drawer handle (`Hovered`), sends
   `DropFloating`. A press outside while the compact drawer is open only closes the drawer.
   The trash and the list of the browser keep destroying in Creative (inside the browser).
9. **In game.** `drop_selected_item_system` (hotbar): `UIMode::Closed`, Drop just pressed, the
   selected hotbar slot holding a stack: `DropSlot { slot: selected, all: ctrl }`. After the
   number keys and the wheel, so that the slot selected in this frame is the one thrown from.

## Steps

### 1. Shared: the drop actions

- RED, in `shared/src/players/inventory.rs`:
  - `dropping_from_a_slot_throws_one_item_or_the_stack`;
  - `dropping_from_a_chest_furnace_output_or_a_leftover_beyond_the_grid`;
  - `dropping_from_a_slot_is_refused_out_of_reach_or_with_a_held_stack`;
  - `dropping_from_an_empty_slot_throws_nothing`;
  - `dropping_the_held_stack_throws_one_item_or_all` and `..._without_one_is_refused`;
  - `a_thrown_tool_keeps_its_wear`;
  - `the_simulation_reports_what_each_action_did` extended with `thrown`.
  In `shared/src/players/simulation.rs`: `what_a_player_throws_is_in_the_outcome`.
  In `shared/src/messages/mod.rs`: the pinned indices 17 and 18.
  In `client/src/audio/inventory.rs`: `drops_are_silent`; in `feedback.rs`:
  `a_refused_drop_shakes_its_slot_or_none`.
- GREEN: the variants, the rules, `thrown` in both outcomes, and the arms of the client's two
  matches (they must cover every variant for the workspace to build).
- Commit `feat(shared): the inventory actions that throw items`.

### 2. Server: thrown items

- RED, in `server/src/world/dropped_items.rs`: `a_thrown_item_waits_two_seconds_before_it_can_be_picked_up`,
  `a_thrown_item_comes_to_rest_out_of_the_reach_of_its_thrower` (steps over `floor()`),
  `merged_items_keep_the_longer_wait`; the overflow test asserts the 10-tick delay still. In
  `shared/src/world/dropped_items.rs`: `can_be_picked_up` honours the item's delay.
- GREEN: `pickup_delay_ticks`, `THROWN_PICKUP_DELAY_TICKS`, `THROWN_SPEED`,
  `throw_from_player`, the merge, and `handle_player_inputs_system` throwing `outcome.thrown`.
- Commit `feat(server): a thrown item flies ahead and waits 2 s to be picked up`.

### 3. Client: the Drop action

- RED: `data.rs` `every_listed_action...` (25, `Drop item`, Interface); `keyboard.rs`
  `drop_is_q_by_default` and that an old keybindings file without `Drop` loads Q.
- GREEN: `GameAction::Drop` appended, its default, name, category, place in `LISTED`. The
  settings e2e counts `of 25`. Run the Controls golden: regenerate only if it changed, and look.
- Commit `feat(client): a Drop key, Q by default`.

### 4. Client: Q in the open inventory

- RED in `display.rs`: `drop_over_a_slot_throws_one_item_and_ctrl_drop_its_stack` (hovered,
  then the ringed cell winning), `drop_over_an_empty_slot_or_nothing_sends_nothing`,
  `drop_waits_while_the_player_types`.
- GREEN: in `click_inventory_cells_system`; `KeyMap` in the test `Hud`.
- Commit `feat(client): Q over a slot of the open inventory throws from it`.

### 5. Client: a press outside the panel

- RED in the new `client/src/ui/hud/inventory/drop.rs`: left outside throws all, right one;
  inside the panel, over the browser or its handle, without a held stack, during a drag, with
  the inventory closed, with the compact drawer open: nothing.
- GREEN: `throw_held_stack_system` in the HUD chain before `click_inventory_cells_system`,
  `.before(drawer_keys)`. The e2e `the_floating_stack...` (inventory.rs) no longer asserts
  that a click outside keeps the stack: that part moves to the new e2e.
- Commit `feat(client): a press outside the inventory throws the held stack`.

### 6. Client: Q in the game

- RED in `drop.rs`: `drop_in_game_throws_one_item_of_the_selected_slot_and_ctrl_drop_its_stack`,
  nothing for an empty slot, nothing with a dialog open or while typing.
- GREEN: `drop_selected_item_system` after the hotbar selection systems, before
  `GameSet::Input`.
- Commit `feat(client): Q in the game throws from the selected hotbar slot`.

### 7. e2e

- `client/src/e2e/drop.rs`:
  - `q_in_game_throws_an_item_ahead_that_is_not_picked_up_at_once`: 10 dirt in slot 0; Q: the
    slot holds 9 and one item appears, ahead of the player (along the camera's forward), and
    is still on the ground a second later with the slot at 9; Ctrl+Q: the slot empties.
  - `a_click_outside_the_panel_throws_the_held_stack`: pick up a stack, right click outside:
    one item thrown; left click outside: the rest; the inventory stays open.
  - `q_over_a_slot_of_the_open_inventory_throws_from_it`.
- Commit `test(client): e2e of dropping items`.

### 8. Wrap-up

- Append the rulings to the spec's Decisions, `(Lot 4)`, from 28.
- `timeout 1500 just check`; re-run a failing test alone before calling it a flake.
- Commit `docs: decisions of dropping items (lot 4)`; write the report.
