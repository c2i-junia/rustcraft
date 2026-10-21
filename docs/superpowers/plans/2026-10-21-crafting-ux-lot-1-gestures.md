# Crafting UX Lot 1 (The Minecraft gestures) Implementation Plan

**Goal:** The gestures a seasoned Minecraft player crafts with: a drag of the held stack over
slots splits it evenly (left button) or drops one item per slot (right button), Shift+click on
the result crafts again and again into the inventory, a number key over the result crafts into
that hotbar slot, a double click gathers the stacks of the held item onto the cursor, and a
Shift+double click moves every stack of that item across. To see: at a crafting table, pick up
17 planks and drag them over the ring of eight cells around the centre: each cell shows 2 in
the accent colour while the button is down, and a chest appears as the result on release.

**Spec:** `docs/superpowers/specs/2026-10-21-crafting-ux-design.md`, "Global constraints",
"Slot numbering" and "Task 1 — Lot 1: the Minecraft gestures", and its "Decisions".

## Global Constraints

- English for code, comments, docs and commits; Conventional Commits, no trailer.
- Doc comments in full sentences, tests named as sentences, sparse comments.
- TDD: each test is written and seen failing (not compiling counts) before the code that
  makes it pass.
- `InventoryAction` and `InventoryRefusal` are append only and `InventoryAction` stays `Copy`;
  `PROTOCOL_ID` goes from 24 to 25 once, here, for the whole branch.
- Every rule lives in `shared/src/players/inventory.rs`; the client only sends actions, and its
  preview calls the shared `drag_plan`.
- One action per gesture: none needs more than one slot of `MAX_INVENTORY_ACTIONS_PER_INPUT`.
- Goldens unchanged: nothing on screen changes outside a drag.
- `cargo test -p shared` / `cargo test -p client <filter>` while working; `just check` (fmt,
  clippy `-D warnings`, every test) at the end, under `timeout 1500`.

## Decisions (where the spec is silent)

1. **`drag_plan`** takes what `apply_inventory_action` takes: `drag_plan(inventory, slots,
   one_each, in_reach, contents) -> Result<DragPlan, InventoryRefusal>`, with
   `DragPlan { slots: Vec<(u32, ItemStack)>, held: Option<ItemStack> }`: the new stack of each
   slot the drag puts items into, ascending, and what stays on the cursor. The action applies
   the same private computation on the inventory with the open chest or furnace lent. A slot
   that holds the item keeps its own stack's type; an empty one takes the held stack's.
2. **More eligible slots than items** (split): the first `n` ascending get `n / n` = 1 each.
   Every slot of a plan gets at least one item.
3. **A `Drag` over one slot** is applied by the server like any other (the whole stack, or one
   item, clamped to the room): only the client turns a one-slot release into a plain click.
4. **The client's drag:** the pressed slot always, then each slot entered that the plan with it
   puts items into, while the current plan puts items into fewer slots than the cursor holds
   items (both buttons, so that a slot entered never takes another's item away). On release:
   one slot is the plain click on it; two or more are a `Drag` of all of them, the pressed slot
   included even when the server skips it.
5. **The preview** shows from two slots on (a one-slot release is a plain click, which may
   swap). The planned counts use `count_label` (no `1`, as everywhere); the icon shows the
   item in an empty cell.
6. **`CraftAll`**: the same item is the same `ItemId` as the first round's result; a round
   fits when the slots `0..36` have room for the whole result, counted as
   `add_item_to_inventory` fills them (stacks of that item, then empty slots).
   `MAX_CRAFT_ALL_ROUNDS = 64`.
7. **`CollectAll`**: `NothingHeld` without a floating stack, `CursorFull` with a full one;
   nothing to gather is not a refusal (silent, as in Minecraft). The sources are the stacks of
   the same `ItemId`.
8. **`MoveAllOfType` zones**: a furnace slot's zone is the furnace's three slots; the grid's is
   its cells a click may reach (`check_slot`: leftovers beyond the grid with an empty cursor).
   `slot` itself must pass `check_slot`, else `InvalidSlot`. "A chest or a furnace open" means
   its contents are lent, as for `QuickMoveSlot`.
9. **`CraftToHotbar`** checks in this order: `InvalidSlot`, `NoRecipe`, `NoRoom`; the floating
   stack is left alone.
10. **Double click** (client, `Time<Real>`): "a press that picked a stack up" is a left press
    without Shift and with an empty cursor on a slot holding a stack, as the client sees it.
    A Shift+press on a slot holding a stack records its item for a Shift+double click. Any
    other press on a cell or on the result forgets the last press; a double click uses it up,
    so a third press is a single one.
11. **Result keys**: Shift changes only the left button and Enter, as on the cells: Shift+right
    click and Shift+Space take the result once. A number key goes to the result as it goes to
    a cell: the ringed result while the ring shows, the hovered one otherwise.
12. **Cancels**: the other button's press cancels the drag and does nothing else. A drag whose
    floating stack is gone (a correction from the server) ends too, sending nothing.
13. **Code**: the gesture state (`CellGestures`, a resource) and `drag_inventory_cells_system`
    live in `client/src/ui/hud/inventory/gestures.rs`, chained right after
    `click_inventory_cells_system`.
14. **e2e**: "a chest ring" is the ring of eight cells around the centre of the 3x3 grid, the
    chest's recipe. The double-click e2e runs its two clicks under
    `TimeUpdateStrategy::ManualDuration`, so that a slow frame under load cannot split them.

## Tasks

### Task 1: The wire (`shared`)
- [x] Tests: `Drag` 12, `CraftAll` 13, `CollectAll` 14, `MoveAllOfType` 15, `CraftToHotbar` 16
  in `network_actions_and_mob_enums_are_pinned`; `a_drag_s_slots_round_trip_on_the_wire`
  (bincode options, bit 127 set); `every_slot_is_below_128`. See them fail.
- [x] The five variants, the refusals `NothingHeld` and `NoRoom`, `PROTOCOL_ID` 25. Until
  tasks 2 to 4 give them their rules, the new variants share one arm in `apply_action` that
  refuses them (`NoRoom`). See them pass. Commit
  `feat(shared): the inventory actions of the Minecraft gestures on the wire`.

### Task 2: The drag (`shared`)
- [x] Tests: split with remainder; clamped to the room; more slots than items; ineligible slots
  skipped (another item, a full stack, beyond the grid, unreachable chest slots); the furnace's
  fuel slot takes only a fuel and its output nothing; one each, until the cursor is empty;
  nothing held refused; no eligible slot refused and unchanged; `drag_plan` returns what the
  action does, with a chest lent; a tool keeps its wear. See them fail.
- [x] `DragPlan`, `drag_plan`, the private `plan_drag`, the `lend` generic over its result.
  See them pass. Commit `feat(shared): a drag splits the held stack over slots`.

### Task 3: Crafting all and to the hotbar (`shared`)
- [x] Tests: `CraftAll` of 64 logs fills four stacks; stops on a full inventory (tops up
  first); stops when the result changes; stops at 64 rounds; leaves the floating stack alone;
  refused without a recipe and without room for one round. `CraftToHotbar`: into an empty
  slot, onto the same item; refused beyond the hotbar, without a recipe, onto another item or
  without room. See them fail.
- [x] `craft_all`, `MAX_CRAFT_ALL_ROUNDS`, `craft_to_hotbar`. See them pass. Commit
  `feat(shared): shift-click and number keys on the crafting result`.

### Task 4: Collecting and moving all of a type (`shared`)
- [x] Tests: `CollectAll` smallest stacks first then slot order, until a full stack; from an
  open chest too; never from the grid or a furnace; refused with nothing held or a full
  cursor. `MoveAllOfType`: hotbar zone, backpack zone, inventory to chest and chest to
  inventory, the grid back into the inventory, a furnace slot; other items stay; refused on an
  invalid slot. See them fail.
- [x] `collect_all`, `move_all_of_type`. See them pass. Commit
  `feat(shared): double clicks gather and move every stack of an item`.

### Task 5: The client's gestures (`client`)
- [x] Tests (display.rs): a press with a held stack sends nothing, its release on the same slot
  sends the click (both buttons); a drag over two slots sends `Drag` (split, one each); a slot
  that takes nothing does not join; a split stops at the held count; the other button,
  closing, Escape cancel; a press without a held stack acts at once; a double click sends
  `CollectAll`, too late it does not; a Shift+double click sends `MoveAllOfType` with the
  first item; Shift+click and Shift+Enter on the result send `CraftAll`; a number key over or
  focused on the result sends `CraftToHotbar`; the drag preview shows the planned counts in
  the accent colour and the floating stack what stays. See them fail.
- [x] `gestures.rs` (`CellGestures`, `CellDrag`, `DOUBLE_CLICK_WINDOW`,
  `drag_inventory_cells_system`, `planned_drag`), the press logic in
  `click_inventory_cells_system`, the preview in `display_inventory_system`, the system in the
  chain. See them pass. Commit `feat(client): drag, double click and result keys in the
  inventory`.

### Task 6: e2e (`client/src/e2e/gestures.rs`)
- [x] Drag-split 17 planks over the chest ring at a table (preview, then 2 each, 1 held, a
  chest made); right-drag 6 planks over the 2x2 grid (1 each, 2 held, a crafting table);
  Shift+click the result of 64 logs (256 planks in four stacks, the grid empty);
  double-click collect (smallest first, a full stack). See them fail on the code before
  task 5 where they can, then pass. Commit `test(client): e2e of the crafting gestures`.

### Task 7: Closing
- [x] Spec decisions appended; `timeout 1500 just check`; commit
  `docs: decisions of the crafting gestures (lot 1)`.
