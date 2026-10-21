# Crafting UX Lot 2 (Feedback) Implementation Plan

**Goal:** Every inventory action the player makes is heard, and a refused one is seen: a soft
sound when a stack goes onto the cursor or leaves it into slots, a woody knock when a craft
succeeds, a dull thud when an action is refused, and the slot it targeted (the result cell for
the craft actions) shakes 3 px left and right for 0.24 s with a `DANGER` border. The result's
tooltip names its keys: `Shift+click: craft all  1-9: to the hotbar`. To see: open the
inventory with an empty grid and click the result: it shakes red and thuds.

**Spec:** `docs/superpowers/specs/2026-10-21-crafting-ux-design.md`, "Global constraints",
"Task 2 — Lot 2: feedback", and its "Decisions" (those of lot 1 bind this lot).

## Global Constraints

- English for code, comments, docs and commits; Conventional Commits, no trailer.
- Doc comments in full sentences, tests named as sentences, sparse comments.
- TDD: each test is written and seen failing (not compiling counts) before the code that
  makes it pass.
- The server's behaviour does not change: it ignores what the simulation now reports.
- The wire does not change: no new action, no `PROTOCOL_ID` bump.
- Goldens unchanged: nothing they show changes (no golden shows the result's tooltip, and no
  slot shakes in them).
- `cargo test -p shared` / `cargo test -p client <filter>` while working; `just check` (fmt,
  clippy `-D warnings`, every test) at the end, under `timeout 1500`.

## How prediction works here (what "once per action" means)

- `update_frame_inputs_system` and the HUD systems fill `CurrentFrameInputs`; in
  `GameSet::Simulation`, `predict_inventory_system` applies the frame's inventory actions once
  to the local player, then `player_movement_system` queues the frame for the server and clears
  them. A frame that is not simulated keeps its actions for the next one, which predicts them
  then: each action is predicted live exactly once.
- In `GameSet::Reconciliation`, `update_players_system` takes the inventory of every server
  update and replays on it the inventory actions of every input the server has not
  acknowledged yet: the same action is re-simulated at each update until it is acknowledged.
- So the feedback comes from `predict_inventory_system` only, as `LocalPlayerMoved` comes
  from the live movement only; the replays report nothing.

## Decisions (where the spec is silent)

1. **Outcomes.** `simulate_inventory_actions` returns a `Vec<InventoryActionOutcome>`, one per
   action it applied (up to `MAX_INVENTORY_ACTIONS_PER_INPUT`), in order:
   `InventoryActionOutcome { action, held_before, held_after, result }`, the floating stack
   before and after it and `Result<(), InventoryRefusal>`. `InventoryRefusal` becomes
   `Clone, Copy`. The server and the replays drop it.
2. **Report.** `predict_inventory_system` writes one `InventoryActionPredicted(outcome)`
   message per outcome. The replays of `update_players_system` write none, and a server
   correction plays nothing: the feedback is the client's prediction, as the spec asks.
3. **Sounds of an accepted action.** The craft actions (`TakeCraftResult`, `CraftAll`,
   `CraftToHotbar`) play `Craft`. The cursor actions (`LeftClickSlot`, `RightClickSlot`,
   `Drag`, `CollectAll`, `TakeFromCatalog`) compare the floating stack: more of the same
   item, or a stack where there was none or another item (a swap) is `ItemPick`; fewer, or
   none where there was one, is `ItemPlace`; unchanged is silent. Every other action is
   silent when accepted: it moves nothing through the cursor (`QuickMoveSlot`,
   `SwapWithHotbar`, `MoveAllOfType`, `FillCraftingGrid`), or it is not a gesture on a slot
   (`Close`, the `Open*` actions, `DestroyFloating`).
4. **Refusals.** Every refusal of a predicted action is reported as `ActionRefused { slot }`,
   a message of `ui::widgets::slot`: the slot, when it has one, shakes, and the audio plays
   `Refused`. The targets: the slot of `LeftClickSlot`, `RightClickSlot`, `QuickMoveSlot`,
   `SwapWithHotbar` and `MoveAllOfType`; every slot of a `Drag`; the result cell for the craft
   actions; no slot for the others (the sound alone). A target with no cell on screen gives
   `slot: None`. `ActionRefused` is the one door to a refused slot's feedback: lot 5 writes it
   for an item browser slot.
5. **One sound of each kind per frame.** The audio system gathers the sounds of the frame's
   predicted outcomes and refusals, and writes each kind once.
6. **The shake** is `RefusalShake`, on any `Slot`: its `UiTransform` moves
   `round(3 px * sin(2 pi t / 80 ms))` (three left-right periods in 0.24 s, whole pixels, no
   new layout), and its border is `DANGER`, over the hover and selection looks, until 0.24 s
   of `Time<Real>` have passed; then the transform goes back to none and the border to the
   slot's look. A slot refused again while it shakes starts again.
7. **The files** are `sounds/ui/item_pick.ogg`, `ui/item_place.ogg`, `ui/craft.ogg`,
   `ui/refused.ogg`, made by `build_item_pick`, `build_item_place`, `build_craft`,
   `build_refused` in `scripts/generate_sounds.py`, listed in `main` with the others. The Ogg
   stream serial differs at each run, so only the four new files are committed.
8. **Hint.** `HINT_RESULT` is the spec's text; the result's tooltip keeps its title and
   count, as it does today.

## Tasks

### Task 1: The simulation reports its outcomes (`shared`)
- [ ] Tests: `simulate_inventory_actions` returns, for a pick-up then a refused craft, the
  action, the held stack before and after, and the result of each; it reports no action
  beyond `MAX_INVENTORY_ACTIONS_PER_INPUT`. See them fail.
- [ ] `InventoryActionOutcome`, the return value, `InventoryRefusal: Clone, Copy`. See them
  pass. Commit `feat(shared): the inventory simulation reports what each action did`.

### Task 2: The client hears of its own predictions (`client/src/player`)
- [ ] Tests: `predict_inventory_system` writes one `InventoryActionPredicted` per action of the
  frame; `update_players_system` replaying unacknowledged actions writes none. See them fail.
- [ ] `InventoryActionPredicted`, written by `predict_inventory_system`, registered by the
  player plugin. See them pass. Commit
  `feat(client): the prediction reports the outcome of each inventory action`.

### Task 3: A refused slot shakes (`client/src/ui/widgets/slot.rs`)
- [ ] Tests: `shake_offset` stays within 3 whole px, goes both ways and is 0 from 0.24 s on;
  an `ActionRefused` on a slot moves it and turns its border `DANGER`; after 0.24 s it is back
  in place with its look (hovered or selected); a second refusal restarts the shake;
  `slot: None` shakes nothing. See them fail.
- [ ] `ActionRefused`, `RefusalShake`, `SHAKE_AMPLITUDE`, `SHAKE_DURATION`, `shake_offset`,
  `start_refusal_shakes`, `shake_refused_slots`, chained after `style_slots` before the
  layout. See them pass. Commit `feat(client): a refused slot shakes with a red border`.

### Task 4: The refused inventory actions (`client/src/ui/hud/inventory/feedback.rs`)
- [ ] Tests: the targets of each refused action (cells, every slot of a drag, the result,
  none); a refused `TakeCraftResult` reports the result cell, a refused click its cell, a
  refused open no slot, an accepted action nothing. See them fail.
- [ ] `RefusedTarget`, `refused_targets`, `report_refused_actions`, after
  `GameSet::Simulation` and before `GameSet::Reconciliation`. See them pass. Commit
  `feat(client): a refused inventory action shakes the slot it targeted`.

### Task 5: The sounds (`client/src/audio`, `scripts/generate_sounds.py`, `data/sounds/ui`)
- [ ] Tests: `inventory_sound` for every case of decision 3; the system writes one sound of
  each kind per frame, `Refused` for an `ActionRefused`; the new sounds are on the Interface
  channel; `every_file_of_every_sound_is_in_the_data_folder` with the new variants; the new
  files decode, pick and place last under 120 ms, craft and refused under 300 ms, none clips
  (peak under 0 dBFS) and none is silent (peak over -20 dBFS). See them fail.
- [ ] `Sound::ItemPick`, `ItemPlace`, `Craft`, `Refused`, their files, the four builders, the
  generated files, `audio/inventory.rs` with `inventory_sound` and `inventory_sounds`. See them
  pass. Commit `feat(client): the sounds of picking, placing, crafting and refusals`.

### Task 6: The result's hint (`display.rs`)
- [ ] Test: the result's tooltip is `Oak Planks`, `4 items`,
  `Shift+click: craft all  1-9: to the hotbar`. See it fail.
- [ ] `HINT_RESULT`. See it pass. Commit `feat(client): the result's tooltip names its keys`.

### Task 7: e2e (`client/src/e2e/feedback.rs`)
- [ ] A click on the result of an empty grid shakes it red and plays `sounds/ui/refused.ogg`,
  and it is back in place 0.24 s later; picking a stack up plays `item_pick.ogg`, putting it
  down `item_place.ogg`; Shift+click on the result of a log plays `craft.ogg`. See them fail
  on the code before tasks 3 to 5 where they can, then pass. Commit
  `test(client): e2e of the feedback of the inventory`.

### Task 8: Closing
- [ ] Spec decisions appended; `timeout 1500 just check`; commit
  `docs: decisions of the feedback (lot 2)`.
