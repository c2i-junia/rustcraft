# Crafting UX — design

Date: 2026-10-21. Branch `feat/crafting-ux`, worktree `feat-crafting-ux/`, from main `b2a3bc5c`.

The user asked for crafting to be as fast as Minecraft for seasoned players ("quand on craft un
item, sur la table de craft, on peut juste laisser appuyer pour dessiner le craft"). The audit
artifact (https://claude.ai/artifact/3E35cwmk6xkgvfSmxtLo4s) listed 14 findings, C1–C14, and
six lots. The user approved all of it and delegated every decision, including the four open
questions, which take the artifact's recommendations:

- Q1: Shift+click on the result puts what it crafts into the inventory, not the cursor.
- Q2: dropping items (lot 4) is part of this branch.
- Q3: wood species mix in a recipe, as in Minecraft.
- Q4: the Mouse Tweaks gestures (lot 6) are on by default, with a switch in Controls.

Later rulings are appended under **Decisions**.

## Global constraints

- Every rule of what an action does lives in `shared` (`shared/src/players/inventory.rs` and
  `shared/src/world/crafting.rs`), applied by the client as a prediction and by the server as
  the authority, exactly as the clicks of today. The client never decides an outcome on its own;
  previews (drag counts, ghosts) call the same shared functions.
- New `InventoryAction` variants are **appended** to the enum, never inserted, and the enum
  stays `Copy`. `PROTOCOL_ID` goes from 24 to 25 **once, in lot 1**, for the whole branch: later
  lots that change the wire (new variants, renumbered recipes) do not bump again. The pinned
  encoding tests are extended for every new variant.
- No action may need more than one slot of `MAX_INVENTORY_ACTIONS_PER_INPUT` (32) per gesture.
- UI text is English, in the game font (no `×`, no `·`: use `x` and two spaces). The pixel-craft
  style of the redesigned UI (`client/src/ui/theme.rs`) is kept.
- Test first. Shared rules get unit tests in `shared`; client systems get unit tests like those
  of `client/src/ui/hud/inventory/display.rs`; every new gesture gets an e2e test in
  `client/src/e2e/`; goldens are regenerated only where the UI changed on purpose, and the
  change is looked at.
- `just check` (fmt, clippy `-D warnings`, the whole workspace test suite) is green at the end
  of every lot. Run test commands under `timeout`. Known flakes under load are listed in the
  memory of the repo's sessions; a failure is re-run alone before it is called a flake.
- Commits: Conventional Commits, in English, no attribution or co-author trailer.
- Doc comments and comments follow the house style: plain sentences saying what a thing is or
  does, as in the files around.

## Slot numbering (unchanged)

Inventory `0..36` (hotbar `0..9`), crafting grid `36..45`, chest `45..72`, furnace input, fuel,
output `72, 73, 74`. Every slot number is below 128, which `Drag` relies on (a test pins it).

## Task 1 — Lot 1: the Minecraft gestures

Fixes C1, C2, C3, C4, C6, C7.

### Shared

Appended variants, in this order:

```rust
/// Release of a drag over at least two slots with a held stack: `slots` has bit `n` set for
/// slot `n`. `one_each`: right button, one item per slot; else left button, the stack split
/// evenly.
Drag { slots: u128, one_each: bool },
/// Shift+click on the result: crafts again and again into the inventory.
CraftAll,
/// Double click with a held stack: the stacks of its item gather onto the cursor.
CollectAll,
/// Shift+double click on a slot that held `item`: every other stack of `item` on that side
/// moves across, as a shift-click each.
MoveAllOfType { slot: u32, item: ItemId },
/// Number key over the result: crafts once into this hotbar slot.
CraftToHotbar(u32),
```

Rules:

- **Drag.** Needs a floating stack, else refused (`NothingHeld`). The eligible slots are those
  set in `slots` that a click may reach (`check_slot`), that take the held item (`takes`), and
  that are empty or hold the same item with room; the others are skipped. Split: with `n` held
  and `k` eligible slots, each gets `n / k` (integer division), clamped to its room, the slots
  taken in ascending order and at most `n` of them; the rest stays on the cursor. One each: one
  item per eligible slot, ascending, until the cursor is empty. A drag with no eligible slot
  changes nothing and is refused (`NoRoom`). A public pure function
  `drag_plan(inventory, slots, one_each, ...) -> what each slot would hold and what stays held`
  is the single computation, used by the action and by the client preview.
- **CraftAll.** Rounds while the grid makes the same item as the first round, at most 64
  rounds: each round checks that the whole result fits in the inventory (hotbar and backpack,
  topping up stacks first, in the order of `add_item_to_inventory`), else stops; then adds it
  and takes one item from each cell, as `TakeCraftResult`. The floating stack is left alone,
  whatever it holds. No round at all is refused (`NoRecipe` or `NoRoom`).
- **CollectAll.** Needs a floating stack with room. Takes from the inventory slots, and from an
  open chest's slots, the stacks of the held item: the smallest stacks first, then by slot
  order, until the cursor holds a full stack. Never from the crafting grid or a furnace.
- **MoveAllOfType.** Every stack of `item` in the zone of `slot` is quick-moved, as
  `QuickMoveSlot` does, in slot order. The zone: with a chest or furnace open, the player's
  whole inventory (`0..36`) or the chest, the side `slot` is on; without one, the hotbar or the
  backpack, the one `slot` is in. From the grid, the grid.
- **CraftToHotbar(h).** `h` must be below 9 (`InvalidSlot`). Crafts once, as `TakeCraftResult`,
  but into hotbar slot `h`, which must be empty or hold the same item with room for the whole
  result, else refused (`NoRoom`).
- New refusals, appended to `InventoryRefusal`: `NothingHeld`, `NoRoom`.

### Client

In `click_inventory_cells_system` (or systems beside it, split if it grows):

- A press **with a floating stack** on a slot starts a drag: nothing is sent yet. While the same
  button stays down, every slot the pointer enters joins the drag if `drag_plan` would put
  something in it, up to the held count for a split. On release: one slot only → the plain
  `LeftClickSlot` / `RightClickSlot` on it, sent then; two or more → `Drag`. Pressing the other
  button, closing the inventory or Escape cancels the drag: nothing is sent.
- A press **without a floating stack** acts at once, as today (pick up, right-click half).
- While dragging, the cells of the drag show the counts `drag_plan` gives them, in the accent
  colour, and the floating stack shows what stays held.
- **Double click:** a second left press on the same slot within `DOUBLE_CLICK_WINDOW` = 250 ms
  of a press that picked a stack up sends `CollectAll` instead of starting a drag. With Shift,
  a second press within the window on the same slot after a shift-click sends
  `MoveAllOfType { slot, item }`, `item` being what the first shift-click moved.
- **Result:** Shift+click (and Shift+Enter on the focused result) sends `CraftAll`; a number key
  over (or focused on) the result sends `CraftToHotbar`.
- The keyboard paths of today keep working; drags are mouse only.

Tests: shared unit tests for every rule above (split with remainder, room clamps, more slots
than items, ineligible slots skipped, furnace fuel/output, one each, CraftAll stopping on a full
inventory and on a changed result and at 64, CollectAll order and limits, MoveAllOfType zones,
CraftToHotbar refusals, the pinned encodings, slots below 128); client unit tests for the drag
state machine, the double click and the result keys; e2e: drag-split planks over a chest ring,
right-drag, shift-click the result of 64 logs, double-click collect.

## Task 2 — Lot 2: feedback

Fixes C12, C13, C14.

- The client learns the outcome of each of its own predicted inventory actions (Ok or the
  refusal), e.g. by `simulate_inventory_actions` returning them or emitting a message; the
  server's behaviour is unchanged.
- **Sounds**, new `Sound` variants on the Interface channel: `ItemPick` (a stack goes onto the
  cursor), `ItemPlace` (items leave the cursor into slots, a drag included), `Craft` (any craft
  action succeeded), `Refused` (any refusal of an action the player made). One sound per action
  at most; when one input carries several actions, at most one sound of each kind per frame.
  The files are made by `scripts/generate_sounds.py`, in the style of the existing effects:
  short (< 120 ms) and soft for pick/place, a woody knock for craft, a dull low thud for
  refused.
- **Refusal:** the slot the refused action targeted (the result cell for the craft actions)
  shakes 3 px left and right for 0.24 s and its border turns `DANGER` for that time.
- **Hints:** `HINT_RESULT` becomes `"Shift+click: craft all  1-9: to the hotbar"`, and the
  tooltip of the result keeps its title and count.

Tests: unit tests mapping outcomes to sounds and to the shake, the sound files exist (the
existing `every_file_of_every_sound_is_in_the_data_folder`), the tooltip text.

## Task 3 — Lot 3: ingredient families

Fixes C8.

- A recipe cell is an `Ingredient`: an item, or `Ingredient::Tag(ItemTag)`. `ItemTag::Planks`
  is every planks item (Oak, Spruce, Jungle today; a future species joins by being listed once).
- `RECIPES` collapses every recipe repeated per species into one with `Planks` cells: sticks,
  crafting table, chest, the five wooden tools. Planks from logs stay one recipe per log.
  Recipe indices change (no new bump, see the global constraints).
- Matching: a `Planks` cell takes any planks; species mix freely (the test
  `mixed_planks_make_nothing` becomes the opposite). Mirroring and placement anywhere are kept.
- `RecipeView` exposes ingredients as `Ingredient`s; `missing`, `craftable`, `recipe_problem`
  count every member of a tag together, and name a tag by its name: `Missing 3 Planks`.
- **Fill grid:** the number of rounds is the largest `r` ≤ `times`, ≤ the max stack, for which
  every cell can get `r` items of one item; a tag cell takes the member the inventory holds the
  most of (after the cells already served), so one species is used whenever it is enough.
- **Item browser:** one card per recipe; a tag cell's icon cycles through the members, one per
  second, all tag cells of a card in step; its tooltip names the tag (`Any Planks`). The uses
  of any planks item list the tag recipes.

Tests: matching with mixed species, every former recipe still made by each species, fill with
one species and with a mix, missing/craftable counts, the cards of the wooden pickaxe (one),
cycling icon, goldens of the recipe views regenerated.

## Task 4 — Lot 4: dropping items

Fixes C5.

- New `GameAction::Drop`, appended, default `KeyQ` (a physical key, as every default), listed
  under Interface on the Controls tab as "Drop item".
- Appended actions:
  - `DropSlot { slot: u32, all: bool }`: one item, or the whole stack, of a slot the player may
    take from (inventory, usable grid, open chest or furnace slots, leftovers beyond the grid).
  - `DropFloating { all: bool }`: one item, or all, of the stack on the cursor.
- The dropped stacks reach the server's `outcome.dropped`, which already throws them ahead of the
  player (`drop_from_player`). An item thrown by a player cannot be picked up for 40 ticks (2 s),
  as Minecraft; the items that overflowed keep their current delay.
- Client: inventory open, Drop over the hovered slot (or the focused one while the ring shows)
  sends `DropSlot { all: false }`, Ctrl+Drop `all: true`; a press of either button outside the
  inventory panel and the item browser with a floating stack sends `DropFloating` (left: all,
  right: one). The Creative destruction over the browser's list is unchanged. Inventory closed,
  in game: Drop drops one item of the selected hotbar slot, Ctrl+Drop its stack. Not while
  typing.

Tests: shared rules and refusals, the pinned encodings, the server spawns the thrown item with
the longer delay, e2e: Q in game throws an item that lies ahead and is not picked up at once,
click outside the panel throws the held stack.

## Task 5 — Lot 5: express recipe book

Fixes C9, C10, C11.

- **Ctrl+click** on an item slot of the browser (catalog, bookmarks, Recent), in both game
  modes, fills the grid with the first recipe of that item that can be made now with the grid
  in use (`FillCraftingGrid`, `times` 1; with Ctrl+Shift, `u32::MAX`), without opening the
  recipe view. When none can, the slot is refused (lot 2's shake and sound) and nothing else
  happens. The browser's tooltip key line adds `Ctrl+click fill`.
- **Ghost recipe:** on a card whose only problem is missing ingredients, `+ Fill grid` is
  enabled; it sends nothing and lays the recipe over the grid as a ghost: each empty cell of
  the recipe shows its ingredient's icon at 40 % opacity, on a `DANGER`-tinted slot where the
  inventory lacks it. The ghost is client state only, and goes away when the grid changes, the
  inventory closes, or another fill or ghost replaces it.
- **Recent:** a row labelled `Recent` under the bookmarks, shown when not empty, holding the
  last 9 distinct items the player crafted in this session (any craft action the prediction
  accepted), newest first. Not saved. Its slots behave as bookmark slots do (R, U, A, clicks,
  Ctrl+click).

Tests: Ctrl+click fills / is refused, Ctrl+Shift max, ghost shown and cleared on each trigger,
Recent order, distinctness, cap, goldens of the browser with a Recent row and of a ghost.

## Task 6 — Lot 6: Mouse Tweaks gestures

- **Shift+drag:** a Shift+press on a slot with an empty cursor quick-moves it at once, and while
  the button stays down every slot the pointer enters is quick-moved too (one `QuickMoveSlot`
  per slot entered, never twice the same slot in one drag).
- **Wheel:** over a slot holding a stack, with an empty cursor, a wheel notch moves one item of
  it to where a shift-click would send the stack. Appended `QuickMoveOne(u32)`. The wheel's
  hotbar selection never fires while the inventory is open.
- A switch `Inventory tweaks` on the Controls tab, on by default, saved with the other
  settings, turns both off.

Tests: shared `QuickMoveOne` rules, client shift-drag and wheel, the switch saved and honoured,
an e2e of shift-drag into a chest.

## Final

After lot 6: a whole-branch review, then the branch waits for the user's go before any merge
or push.

## Decisions

(Appended during implementation.)

1. (Lot 1) `drag_plan` takes what `apply_inventory_action` takes:
   `drag_plan(inventory, slots, one_each, in_reach, contents) -> Result<DragPlan, InventoryRefusal>`,
   `DragPlan { slots: Vec<(u32, ItemStack)>, held: Option<ItemStack> }`: the new stack of each
   slot the drag puts items into, ascending, and what stays held. The action runs the same
   private computation on the inventory with the open chest or furnace lent; the client calls
   it with the chest or furnace the server sent. A slot that holds the item keeps its stack's
   type; an empty one takes the held stack's (a tool keeps its wear).
2. (Lot 1) A split over more eligible slots than held items puts one item in each of the first
   `n` slots, ascending (`n / n`): every slot of a plan gets at least one item.
3. (Lot 1) The server applies a `Drag` over a single slot like any other; only the client turns
   a one-slot release into the plain click.
4. (Lot 1) The client's drag holds the pressed slot, then each slot entered that the plan with
   it puts items into, while the plan puts items into fewer slots than the cursor holds items,
   for both buttons: a slot entered never takes another's item away. A release with two slots
   or more sends all of them, the pressed slot included even when the server skips it.
5. (Lot 1) The preview shows from two slots on, since a one-slot release is a click (which may
   swap). Its counts use `count_label`, so a single item shows its icon and no `1`.
6. (Lot 1) `CraftAll`: "the same item" is the same `ItemId`; a round fits when the slots
   `0..36` have room for the whole result, counted as `add_item_to_inventory` fills them.
   `MAX_CRAFT_ALL_ROUNDS = 64`, which only a stack larger than 64 in a cell could reach.
7. (Lot 1) `CollectAll`: `NothingHeld` without a floating stack, `CursorFull` with a full one;
   nothing to gather is no refusal (silent, as in Minecraft). Its sources are the stacks of the
   same `ItemId`.
8. (Lot 1) `MoveAllOfType`: a furnace slot's zone is the furnace's three slots; the grid's zone
   is its cells a click may reach (`check_slot`, so leftovers beyond the grid with an empty
   cursor). `slot` must pass `check_slot`, else `InvalidSlot`. "A chest or a furnace open" means
   its contents are lent, as for `QuickMoveSlot`.
9. (Lot 1) `CraftToHotbar` checks `InvalidSlot`, then `NoRecipe`, then `NoRoom`, and leaves the
   floating stack alone.
10. (Lot 1) Double clicks use `Time<Real>`. "A press that picked a stack up" is a left press
    without Shift, with an empty cursor, on a slot holding a stack, as the client sees it; a
    Shift+press on a slot holding a stack records its item for the Shift+double click. Any
    other press on a cell, on the result or elsewhere forgets the last press, and a double click
    uses it up: a third press is a single one.
11. (Lot 1) Shift changes only the left button and Enter, as on the cells: Shift+right click and
    Shift+Space on the result take it once. A number key goes to the result as to a cell: the
    ringed result while the ring shows, the hovered one otherwise.
12. (Lot 1) The other button's press cancels the drag and does nothing else. A drag whose held
    stack is gone (a correction from the server) ends too, sending nothing.
13. (Lot 1) The gestures live in `client/src/ui/hud/inventory/gestures.rs`: the `CellGestures`
    resource and `drag_inventory_cells_system`, chained right after
    `click_inventory_cells_system`.
14. (Lot 1) The e2e "chest ring" is the ring of eight cells around the centre of the 3x3 grid,
    the chest's recipe. The double-click e2e runs its two clicks under
    `TimeUpdateStrategy::ManualDuration`, so that a slow frame cannot split them.
15. (Lot 2) `simulate_inventory_actions` returns a `Vec<InventoryActionOutcome>`, one per
    action it applied (up to `MAX_INVENTORY_ACTIONS_PER_INPUT`), in order:
    `InventoryActionOutcome { action, held_before, held_after, result }`, the floating stack
    before and after the action and its `Result<(), InventoryRefusal>` (now `Clone, Copy`). The
    server and the client's replays drop it: the server behaves as before.
16. (Lot 2) Once per action: the client applies each action live once, in
    `predict_inventory_system`, but `update_players_system` replays every unacknowledged
    input's actions at each server update that carries an inventory. Only the live prediction
    reports, as `InventoryActionPredicted(outcome)` (like `LocalPlayerMoved` for the
    movement); the replays report nothing, and a server correction plays nothing either.
17. (Lot 2) Sounds of an accepted action: the craft actions (`TakeCraftResult`, `CraftAll`,
    `CraftToHotbar`) play `Craft`, whatever the cursor does. The cursor actions
    (`LeftClickSlot`, `RightClickSlot`, `Drag`, `CollectAll`, `TakeFromCatalog`) compare the
    floating stack: a stack where there was none, another item (a swap) or more of the same is
    `ItemPick`; none where there was one, or fewer, is `ItemPlace`; unchanged is silent. The
    other accepted actions are silent: they move nothing through the cursor (`QuickMoveSlot`,
    `SwapWithHotbar`, `MoveAllOfType`, `FillCraftingGrid`) or are no gesture on a slot
    (`Close`, the `Open*` actions, `DestroyFloating`).
18. (Lot 2) A refusal is `ActionRefused { slot: Option<Entity> }`, a message of
    `ui::widgets::slot`: the audio plays `Refused` for it, and the slot, when it is a `Slot`,
    shakes. It is the one door to a refusal's feedback: lot 5 writes it for the item browser
    slot of a refused Ctrl+click. For a predicted refusal, `report_refused_actions` writes one
    per targeted cell: the slot of `LeftClickSlot`, `RightClickSlot`, `QuickMoveSlot`,
    `SwapWithHotbar` and `MoveAllOfType`, every slot of a `Drag`, the result for the craft
    actions; an action on no slot, or on a slot no cell shows, gives `slot: None` (the sound
    alone).
19. (Lot 2) One sound of each kind per frame: `inventory_sounds` gathers the sounds of the
    frame's predicted outcomes and refusals and writes each kind once, in the order first met.
20. (Lot 2) The shake (`RefusalShake`, on any `Slot`) moves the slot's `UiTransform` by
    `round(3 px * sin(2 pi t / 80 ms))`, three left-right periods in the 0.24 s, in whole
    logical pixels and without a new layout; its border is `DANGER` over the hover and
    selection looks. It runs on `Time<Real>`, starts again when the slot is refused again, and
    at its end the slot goes back in place with its look.
21. (Lot 2) The files are `sounds/ui/item_pick.ogg` and `item_place.ogg` (90 ms, a soft tick
    rising, and a soft tap falling, at -27 LUFS of the script's measure), `craft.ogg` (a
    240 ms knock of the wood contact, -21) and `refused.ogg` (a 200 ms thump from 150 to 70 Hz
    under 900 Hz, -22), made by the script's `build_item_pick`, `build_item_place`,
    `build_craft` and `build_refused`. The Ogg stream serial changes at each run of the script,
    so only the new files are committed. A test decodes them: pick and place under 120 ms,
    craft and refused under 300 ms, each peak between -20 and 0 dBFS.
