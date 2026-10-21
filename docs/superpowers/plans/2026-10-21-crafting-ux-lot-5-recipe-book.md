# Crafting UX Lot 5 (Express recipe book) Implementation Plan

**Goal:** The item browser fills the grid without a detour through the recipe view: Ctrl+click
on an item slot of the browser (catalog, bookmarks, Recent) fills the grid with the first recipe
of that item that can be made now (Ctrl+Shift: as many rounds as possible), or shakes the slot
with the refused thud. A card whose only problem is missing ingredients gets an enabled
`+ Fill grid` that lays the recipe over the grid as a ghost, red where the inventory lacks the
ingredient. A `Recent` row under the bookmarks holds the last 9 distinct items crafted in the
session. Fixes C9, C10, C11. To see: with 1 stick in the inventory, open the recipes of the
torch and press `+ Fill grid`: the coal shows faint on a red cell, the stick faint beside it;
Ctrl+click on Stick in the catalog with planks in the inventory: the grid takes two planks.

**Spec:** `docs/superpowers/specs/2026-10-21-crafting-ux-design.md`, "Global constraints",
"Task 5 — Lot 5: express recipe book", and its "Decisions" (lots 1 to 4 bind this lot); the
item browser's `docs/superpowers/specs/2026-10-20-item-browser-design.md` (bookmarks row,
tooltips, keys).

## Global constraints

- English for code, comments, docs and commits; Conventional Commits, no trailer.
- Doc comments in full sentences, tests named as sentences, sparse comments.
- TDD: each test is written and seen failing (not compiling counts) before the code that
  makes it pass. Every commit compiles and its tests pass.
- No new wire action: `FillCraftingGrid` exists. `PROTOCOL_ID` stays 25.
- Every rule stays in `shared`: the client's preview (which recipe a Ctrl+click fills, which
  cells of a ghost lack their ingredient) calls the one shared entry point that the action
  applies, so that it cannot disagree with the server.
- `click_inventory_cells_system` (14 of 16 params) is not touched.
- `cargo test -p shared <filter>`, `-p client <filter>` while working, under `timeout`;
  `timeout 1500 just check` at the end.

## Decisions (where the spec is silent)

1. **One entry point.** `shared::players::inventory::plan_grid_fill(inventory, recipe, times,
   size) -> Result<GridFill, InventoryRefusal>` puts the grid back into a copy of the
   inventory and plans the fill on it: `GridFill { recipe, emptied, plan: Option<FillPlan>,
   lacking: Vec<bool> }`. `Err` for `UnknownRecipe`, `DoesNotFit`, `GridNotEmptied`, in that
   order (today's); `plan` is `None` when one round is not there (the action's
   `MissingIngredients`); `lacking` says, cell by cell of the recipe, row after row, which cells
   one round leaves without their ingredient, served as `plan_fill` serves them
   (`shared::world::lacking_cells`, the same serving code as `plan_fill`). `fill_crafting_grid`
   calls it, and only applies the plan.
2. **What was crafted.** `InventoryActionOutcome` gets `crafted: Option<ItemId>`: for an
   accepted `TakeCraftResult`, `CraftAll` or `CraftToHotbar`, the item the grid made before the
   action. `simulate_inventory_actions` fills it; the server ignores it, as `held_*`.
3. **Ctrl+click.** A left press with either Control key held on a hovered `CatalogSlot` (the
   grid, a bookmark, a recent item), in both modes, with the inventory open: the first recipe of
   `recipes_for(item)` that `fills_now` (no chest nor furnace open in place of the grid, and
   `plan_grid_fill(.., 1, grid_in_use)` has a plan) is sent as `FillCraftingGrid { recipe,
   times: 1 }`, `u32::MAX` with Shift. None: `ActionRefused { slot: Some(slot) }`, nothing sent.
   Ctrl+left on the browser does nothing else: it opens no view (Survival), takes nothing and
   destroys nothing (Creative), on an item, the list or the trash. Ctrl with the right button,
   Enter or Space acts as without Ctrl. A card's slots ignore Ctrl.
4. **Tooltip.** The key line of a catalog slot (grid, bookmark, recent) of an item that has a
   crafting recipe ends with `  Ctrl+click fill`: `R Recipes  U Uses  A Bookmark  Ctrl+click
   fill`. Items with no crafting recipe (Dirt, the ores) keep the line of today, as a Ctrl+click
   on them is always refused; the cards' slots keep theirs.
5. **The button.** `fill_button(inventory, recipe, grid_size) -> Option<FillButton>`:
   `Some(Fill)` when `recipe_problem` is `None` (today's enabled button), `Some(Ghost)` when the
   only problem is missing ingredients (the recipe fits the grid in use and no chest or furnace
   is open), else `None` (disabled). The ghost button looks as the fill button (green): it is
   enabled, and laying the ghost is what it does. Activated (`BrowserRequests::fill`), the
   decision is taken again on the inventory of that frame: `Fill` sends the action as today,
   `Ghost` lays the ghost (Shift changes nothing).
6. **The ghost.** `GridGhost(Option<Ghost>)`, client state in `ui::hud::inventory::ghost`:
   `Ghost { grid, cells }`, the nine grid stacks when it was laid and, for each cell of the
   recipe holding an ingredient whose grid slot is empty, `GhostCell { slot, ingredient,
   lacking }`, the recipe at the top left corner as the fill puts it. `ghost_of(inventory,
   recipe, size)` builds it from `plan_grid_fill(.., 1, size)` (`lacking`); `None` when that is
   refused. Each crafting grid cell gets two hidden children after its icon: a tint
   (`MISSING_TINT`, the cards' `DANGER` at 0.32, shown where `lacking`) and a ghost icon (the
   ingredient's item at 40 % alpha; a family shows its members in turn, `tag_member`, in step
   with the cards). `clear_grid_ghost` takes it away when the inventory is hidden or the nine
   grid stacks differ from those it was laid on; a fill sent by the browser, or another ghost,
   replaces it.
7. **Recent.** `RecentCrafts { items }`, newest first, at most `RECENT_CRAFTS = 9`, distinct:
   a craft moves its item to the front. Filled from `InventoryActionPredicted` outcomes with
   `crafted` (accepted only; the replays report nothing, so a craft counts once). Emptied on
   entering a world (`OnEnter(GameState::Game)`): a session is a world played, not saved. Its
   bar (`RecentBar`, label `Recent`) and row (`RecentRow`) are built as the bookmarks', right
   under them, shown while not empty and no view shows; its slots are `CatalogSlot`s with a
   `RecentSlot` marker, out of the search and the count as bookmarks are. `show_bookmarks`
   becomes the generic `show_shelf::<S>` over a `Shelf` (its items, bar, row and slot marker),
   run for `Bookmarks` and `RecentCrafts`.

## Steps

### 1. Shared: one entry point for the fill

- RED, in `shared/src/world/crafting.rs`: `lacking_cells_serves_one_round_cell_by_cell`
  (a pickaxe with 2 planks and 1 stick: the third planks cell and the second stick lack; a
  family takes any member; nothing lacks when one round is there).
- RED, in `shared/src/players/inventory.rs`: `planning_a_fill_puts_the_grid_back_first`
  (planks in the grid count, `emptied` holds them in the inventory, the grid empty),
  `planning_a_fill_is_refused_as_the_action_is` (unknown recipe, does not fit,
  `GridNotEmptied` with a full inventory, `plan: None` and its lacking cells without one
  round), `the_fill_applies_what_was_planned` (for a few inventories, `FillCraftingGrid` is
  accepted iff `plan_grid_fill` has a plan, and the grid then holds the plan's cells).
- GREEN: `lacking_cells`, the serving of `plan_rounds` shared with it; `GridFill`,
  `plan_grid_fill`; `fill_crafting_grid` applies it.
- Commit `feat(shared): one entry point plans a fill of the grid, put back first`.

### 2. Shared: what an action crafted

- RED: `an_outcome_names_what_a_craft_action_crafted` (TakeCraftResult, CraftAll,
  CraftToHotbar accepted: the item; refused: `None`; a click: `None`).
- GREEN: `crafted` in `InventoryActionOutcome`, filled by `simulate_inventory_actions`; the
  literals of the client's tests get `crafted: None`.
- Commit `feat(shared): an inventory outcome names what it crafted`.

### 3. Client: Ctrl+click fills the grid

- RED, in `client/src/ui/hud/item_browser/express.rs`: `a_ctrl_click_fills_the_grid_with_the
  _first_recipe_made_now`, `ctrl_shift_fills_as_many_rounds_as_possible`,
  `a_ctrl_click_with_nothing_to_make_shakes_the_slot_and_sends_nothing` (no recipe, missing
  ingredients, needs a table, a chest open), `a_ctrl_click_opens_no_view_and_takes_nothing`
  (Survival `click_items`, Creative `catalog_actions`); in `recipes.rs`
  `the_keys_of_a_catalog_slot_name_ctrl_click_when_the_item_is_crafted`; display tests of the
  tooltip updated.
- GREEN: `fills_now`, `express_fill` (`BrowserRequests::fill` handled there too, moved from
  `catalog_actions`), `catalog_keys_hint`, Ctrl in `click_items` and `catalog_actions`.
- Commit `feat(client): Ctrl+click on an item of the browser fills the grid`.

### 4. Client: the ghost recipe

- RED, `client/src/ui/hud/inventory/ghost.rs`: `a_ghost_shows_the_empty_cells_of_the_recipe`
  (`ghost_of`: cells, lacking, a cell holding items left out), `the_ghost_goes_when_the_grid
  _changes_or_the_inventory_closes`, `the_ghost_cells_show_faint_icons_on_red_where_lacking`
  (`show_grid_ghost` on a world of cells); in `express.rs` `fill_grid_on_a_card_missing
  _ingredients_lays_a_ghost_and_sends_nothing`, `a_fill_replaces_the_ghost`; in `view.rs` the
  button test: enabled for missing-only, disabled for a table or a chest; `recipes.rs`
  `fill_button` cases.
- GREEN: `GridGhost`, `ghost_of`, the cells' children in `setup_inventory`, `show_grid_ghost`,
  `clear_grid_ghost`, `fill_button` in `style_recipe_cards`, the ghost in `express_fill`.
- Commit `feat(client): + Fill grid lays a ghost of a recipe missing ingredients`.

### 5. Client: the Recent row

- RED, `client/src/ui/hud/item_browser/recent.rs`: `recent_keeps_nine_distinct_items_newest
  _first`, `an_accepted_craft_goes_into_recent` (outcomes: accepted crafted ones only),
  `entering_a_world_empties_recent`; `display.rs` `recent_slots_are_neither_searched_nor
  _counted`.
- GREEN: `RecentCrafts`, `remember_crafts`, the bar, `Shelf` and `show_shelf`.
- Commit `feat(client): a Recent row of the items crafted in the session`.

### 6. Goldens

- `the_recent_row_at_1280x720` (two bookmarks, three recents) and
  `a_ghost_of_the_torch_over_the_grid_at_1280x720` (1 stick, no coal: the coal cell red, the
  stick faint); `show_frames` runs `show_grid_ghost` and `show_shelf::<RecentCrafts>`. Run
  every golden: only the two new ones may appear; look at both PNGs.
- Commit `test(client): goldens of the Recent row and of a ghost recipe`.

### 7. E2E

- `client/src/e2e/item_browser.rs`: `ctrl_click_fills_the_grid_from_the_browser` (Survival:
  Ctrl+click Stick fills one round, Ctrl+Shift the most, no view opens; Ctrl+click Dirt shakes
  its slot and sends nothing; Creative: Ctrl+click fills too and takes nothing),
  `a_card_missing_ingredients_lays_a_ghost_until_the_grid_changes` (ghost shown, cleared by
  a click into the grid, laid again and cleared by closing the inventory, laid again and
  replaced by a Ctrl+click fill), `the_items_crafted_go_into_recent` (two crafts, the order,
  the same item once, Ctrl+click on a recent slot).
- Commit `test(client): e2e of the express recipe book`.

### 8. Decisions and check

- Append the rulings above to the spec's Decisions, as `(Lot 5)` items 38 and on.
- `timeout 1500 just check`; re-run alone any failure that looks unrelated.
- Commit `docs: decisions of the express recipe book (lot 5)`.
