# Item Browser Lot 3 (Creative and Fill grid) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Items come out of the catalog in Creative (a click puts a full stack on the cursor, a right click one more, Shift+click a full stack into the inventory, a click on the list or the trash destroys the held stack with a toast), and `+ Fill grid` on a recipe card moves the ingredients into the crafting grid in both modes (Shift: as many rounds as possible). The server applies all of it and refuses the Creative actions of a Survival player. The row of chips wraps so that `Craftable` is always in view.

**Architecture:** `shared::players::inventory` gains three `InventoryAction` variants, appended last (`TakeFromCatalog`, `DestroyFloating`, `FillCraftingGrid`), and their refusals; `apply_inventory_action` takes `creative: bool`, which `simulate_inventory_actions` reads from `player.game_mode`, on the server as on the client (whose `Player` follows the server's game mode), so that prediction and authority agree. `PROTOCOL_ID` 18 → 19. The client's `item_browser/creative.rs` holds the pure rules of a gesture on the catalog and the texts; `catalog_actions` turns the clicks, Enter / Space on the focused slot, Enter in the search, the list and the trash into `InventoryAction`s pushed to `CurrentFrameInputs` before the simulation. Observers (`Submit` of the search, `Activate` of `+ Fill grid`) only leave a request in `BrowserRequests`, which `catalog_actions` sends in `Update`: an observer runs outside the frame's input window. The browser still never writes `Res<Inventory>`.

**Tech Stack:** Rust nightly, Bevy `=0.19.1` (`bevy::ui_widgets::{Button, Activate}`, `bevy::input_focus`, `bevy::picking::hover::Hovered`, `bevy::ui::InteractionDisabled`), `accesskit` 0.24, lavapipe for goldens and e2e.

**Spec:** `docs/superpowers/specs/2026-10-20-item-browser-design.md`, lot 3 ("Creative and Fill grid"): the Creative footer and tooltip line of "What the player sees"; the Creative column of "Interactions", "Enter in the search"; the three actions of "Shared"; the server test of "Testing". The look and the gestures: the approved mockup (`clickCatalog`, `destroyCursor`, `fillGrid`, the trash of `renderBrowser`, `.fill`). API reference: `docs/superpowers/specs/2026-10-18-bevy-ui-0.19-notes.md`.

## Global Constraints

The "Global Constraints" of `docs/superpowers/plans/2026-10-18-ui-redesign-lot-5-hotbar-inventory.md` and of the plans of lots 1 and 2 apply, with these readings for this lot:

- English for code, comments, docs, commit messages and game texts; Conventional Commits.
- Tests named as sentences; doc comments in full sentences; comment density like the neighbouring code.
- Every test command runs under `timeout`. `cargo fmt --all -- --check` and `timeout 900 cargo clippy --workspace --all-targets -- -D warnings` are clean at every commit; the suite is green at the end of every task.
- E2e tests in `client/src/e2e/item_browser.rs`, `#[test]` + `#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]`, driven only through `click`/`right_click`/`hover`/`press`/`key_down`/`type_text`/`resize`. **Two new journeys**: Creative; Fill grid.
- No `bevy::prelude::Button`, `Interaction` or `FocusPolicy`: `+ Fill grid` stays a `bevy::ui_widgets::Button`; the trash is a `widgets::slot` (hit test `Hovered`). Decorations carry `Pickable::IGNORE`.
- The role before `AccessibleLabel` in every bundle.
- **Inventory authority.** The browser never writes `Res<Inventory>`: every change is an `InventoryAction` pushed to `CurrentFrameInputs`, predicted by the client and applied by the server.
- **Protocol.** The three variants go at the end of `InventoryAction`; `PROTOCOL_ID` is bumped once, 18 → 19; `network_actions_and_mob_enums_are_pinned` names them.
- Goldens: the browser and open-inventory goldens that show the row of chips change **only in Task 3**, because it wraps; `item_browser_uses_854x480` changes **only in Task 5**, because `+ Fill grid` gets enabled there; the new `item_browser_creative_1280x720` comes in Task 4. `hotbar.png` and every menu golden must not change. Each new or changed PNG is looked at before it is accepted.

## Review Focus

1. A Survival player's `TakeFromCatalog` and `DestroyFloating` change nothing, on the server (session test) and in the prediction (the same `simulate_inventory_actions`). (Tasks 1, 2)
2. `FillCraftingGrid` is atomic: refused, it changes nothing, not even the grid it would have put back. (Task 1)
3. Prediction and server agree: the client's `Player::game_mode` follows the server's, and the same function applies the actions.
4. Observers never push to `CurrentFrameInputs`: a request waits for `catalog_actions`, before the simulation. (Tasks 4, 5)
5. In Survival a click on the list holding a stack does nothing; the Survival gestures of lot 2 are unchanged. (Task 4)

## Files

- `shared/src/players/inventory.rs`, `shared/src/constants.rs`, `shared/src/messages/mod.rs`, `shared/src/players/game_mode.rs` (Task 1).
- `server/tests/session.rs` (Task 2).
- `client/src/ui/hud/item_browser/{mod,setup,display,view,goldens,creative (new)}.rs`, `client/src/e2e/item_browser.rs`, `client/tests/golden/*.png` (Tasks 3–5).
- `README.md`, `docs/ARCHITECTURE.md`, the spec's "Decisions" (Task 6).

---

### Task 1: The three actions in shared

**Files:**
- Modify: `shared/src/players/inventory.rs`, `shared/src/players/game_mode.rs`, `shared/src/constants.rs`, `shared/src/messages/mod.rs`

**Interfaces:**
- Produces:
  ```rust
  // appended to InventoryAction
  TakeFromCatalog { item: ItemId, count: u32, to_inventory: bool },
  DestroyFloating,
  FillCraftingGrid { recipe: u16, times: u32 },
  // appended to InventoryRefusal
  NotInCreative, UnknownRecipe(u16), DoesNotFit, MissingIngredients, GridNotEmptied,
  impl GameMode { pub fn is_creative(self) -> bool }
  pub fn apply_inventory_action(inventory, action, in_reach, chest, creative: bool) -> Result<(), InventoryRefusal>;
  ```
  - `TakeFromCatalog`: Creative only; `count` clamped to `1..=max_stack`; a new stack (`get_default_type`: a tool unworn). `to_inventory`: `add_item_to_inventory` (hotbar first), what does not fit is lost. Else to the cursor: `CursorFull` if it holds another item, topped up to the max stack if the same.
  - `DestroyFloating`: Creative only; the cursor is emptied.
  - `FillCraftingGrid`: both modes. On a copy of the inventory: the nine cells go back into the inventory (`GridNotEmptied` if one does not fit); `UnknownRecipe` past the table; `DoesNotFit` when the recipe is larger than the grid in use; `times` = min(`times`, per ingredient ⌊held ÷ cells⌋, the max stack of every ingredient) — `MissingIngredients` at 0; each cell of the recipe, unmirrored, at the top-left corner, gets `times` items taken from the inventory, the backpack's last slots first, the hotbar last. Only a success replaces the inventory.
  - `simulate_inventory_actions` passes `player.game_mode.is_creative()`.

- [ ] **Step 1: Write the failing tests** (`inventory.rs`)
  - `taking_from_the_catalog_needs_creative` — Survival: `NotInCreative` for both Creative actions, nothing changed; `simulate_inventory_actions` on a Survival `Player` changes nothing, on a Creative one takes.
  - `taking_from_the_catalog_clamps_the_count` — 0 → 1, 100 dirt → 64, a pickaxe ×5 → 1 unworn.
  - `taking_onto_a_cursor_holding_another_item_is_refused` — `CursorFull`.
  - `taking_onto_a_cursor_holding_the_same_item_tops_it_up` — 63 + 1 → 64; 64 + 1 → 64.
  - `taking_to_the_inventory_fills_the_hotbar_first` — a dirt in slot 20: 64 dirt top up slot 20 then the first free hotbar slot; the cursor untouched.
  - `destroying_the_floating_stack_empties_the_cursor`.
  - `filling_the_grid_returns_the_grid_first` — a stone in a grid cell goes back to the inventory.
  - `filling_the_grid_puts_the_recipe_at_its_top_left_corner_unmirrored` — the pickaxe at a table: planks in row 0, sticks in (1,1), (2,1); `craft_result` is the pickaxe.
  - `filling_the_grid_needs_a_recipe_that_fits_the_grid_in_use` — the pickaxe without a table: `DoesNotFit`; sticks fit 2×2.
  - `filling_the_grid_clamps_the_rounds_to_the_inventory_and_the_max_stack` — 7 planks, sticks ×`u32::MAX` → 3 per cell, 1 plank left; 200 logs → 64.
  - `filling_the_grid_without_one_round_is_refused_and_changes_nothing` — `MissingIngredients`, the grid as it was.
  - `filling_the_grid_works_in_survival`.
  - `filling_the_grid_with_an_unknown_recipe_is_refused`.
  - `messages/mod.rs`: the pin test names the three variants (8, 9, 10).
- [ ] **Step 2: Run them to verify they fail** — `timeout 600 cargo test -q -p shared inventory` → compile error.
- [ ] **Step 3: Implement** as in Interfaces; `PROTOCOL_ID` 19.
- [ ] **Step 4: Run the tests and the suite** → PASS.
- [ ] **Step 5: Commit** — `git commit -m "feat(shared): take from the catalog, destroy the held stack and fill the crafting grid"`

---

### Task 2: The server refuses a Survival player

**Files:**
- Modify: `server/tests/session.rs`

- [ ] **Step 1: Write the tests**
  - `a_survival_player_cannot_take_from_the_catalog_or_destroy_its_stack` — a Survival player holding dirt on its cursor sends `TakeFromCatalog` ×2 and `DestroyFloating`: the inventory acknowledged is unchanged; `/gamemode creative`, the same actions: the stack is destroyed, 64 stone in the inventory.
  - `a_survival_player_fills_the_grid` — one oak log: `FillCraftingGrid` of the planks: the log in the grid; `TakeCraftResult` → 4 planks on the cursor.
- [ ] **Step 2: Run them** — `timeout 600 cargo test -q -p server --test session` → PASS (Task 1 did the work; they guard it).
- [ ] **Step 3: Commit** — `git commit -m "test(server): a survival player takes nothing from the catalog, and fills the grid"`

---

### Task 3: Every chip in view

**Files:**
- Modify: `client/src/ui/hud/item_browser/{setup,display}.rs`, `client/tests/golden/*.png`

- The row of chips wraps (`flex_wrap: Wrap`, `row_gap: SPACE_XS`), no sideways scroll (`ScrollArea` and `wheel_sideways` go). The Craftable chip, off, keeps its green text (`OK`), so that it reads apart from the categories.
- [ ] **Step 1: Write the failing e2e check** in `recipes_and_uses_open_from_the_browser_and_the_history_walks_back`: the Craftable chip lies inside the browser at 1280×720.
- [ ] **Step 2: Implement.**
- [ ] **Step 3: Goldens** — read every `.actual.png` (two rows of chips, nothing clipped at 854×480), then `UPDATE_GOLDEN=1`.
- [ ] **Step 4: Suite** → green.
- [ ] **Step 5: Commit** — `git commit -m "fix(client): the chips of the item browser wrap, so that Craftable is always in view"`

---

### Task 4: Creative — take, one more, to the inventory, destroy

**Files:**
- Create: `client/src/ui/hud/item_browser/creative.rs`
- Modify: `client/src/ui/hud/item_browser/{mod,setup,display,view,goldens}.rs`, `client/src/e2e/item_browser.rs`

**Interfaces:**
- Produces (pure, `creative.rs`):
  ```rust
  #[derive(Clone, Copy, PartialEq, Eq, Debug)] pub enum Gesture { Click, RightClick, ShiftClick }
  pub fn creative_action(item: ItemId, gesture: Gesture, held: Option<ItemStack>) -> InventoryAction;
      // held and not (RightClick on the same item) → DestroyFloating; Click → max stack on the cursor;
      // RightClick → 1; ShiftClick → max stack to the inventory
  pub fn creative_hint(item: ItemId) -> String;      // "LMB 64  RMB 1  Shift to inventory"
  pub fn destroyed_message(stack: ItemStack) -> String; // "Destroyed 64 Dirt"
  pub const TRASH_HINT: &str = "Drop a stack here, or on the list, to destroy it.";
  ```
- Produces (panel):
  ```rust
  #[derive(Component)] pub struct CatalogList;  // the scroll frame of the catalog, with Hovered
  #[derive(Component)] pub struct TrashSlot;    // the trash of the Creative footer
  #[derive(Resource, Default)] pub struct BrowserRequests { pub submit: Option<bool>, pub fill: Option<(u16, bool)> } // shift
  pub fn catalog_actions(..);  // Update, after UiMode, before GameSet::Simulation
  ```
  - `catalog_actions`: a press on a catalog slot (Shift: `ShiftClick`), Enter / Space on the focused one while its ring shows (Shift+Enter), Enter in the search on the first shown slot: in Creative `creative_action` pushed, a `DestroyFloating` with its toast; in Survival, Enter in the search opens the recipes of the first result (the other Survival gestures stay in `click_items`). A press on the list or the trash, or Enter / Space on the focused trash, holding a stack in Creative: `DestroyFloating` and the toast. Nothing in Survival.
  - `submit_search` (observer) leaves `BrowserRequests::submit`; `show_view` moves a focus left in any `CatalogPart` (the search too) to the back button.
  - The Creative footer: the trash slot (an `X` in `DANGER`) and `TRASH_HINT`; the catalog's tooltips in Creative: `creative_hint` over `ITEM_KEYS_HINT`, on two lines.
- [ ] **Step 1: Write the failing tests**
  - `creative.rs`: `a_click_takes_a_full_stack_and_a_right_click_one`, `holding_a_stack_a_click_on_the_catalog_destroys_it`, `a_right_click_on_the_held_item_adds_one`, `shift_click_sends_a_full_stack_to_the_inventory`, `the_texts_of_creative` (64 / 1 for a tool, `Destroyed 64 Dirt`).
  - Golden `the_creative_panel_at_1280x720` → `item_browser_creative_1280x720` (the trash footer).
  - E2e `creative_takes_from_the_catalog_and_the_list_destroys_the_held_stack`:
    `/gamemode creative`; E; hover Dirt → `LMB 64  RMB 1  Shift to inventory`; click Dirt → cursor 64 Dirt; click the list → toast `Destroyed 64 Dirt`, cursor empty; right click Stone ×3 → 3 Stone; right click Dirt → destroyed; Shift+click Wooden Pickaxe → slot 0 holds it, cursor empty; click Dirt, click the trash → destroyed; `/` type `oak l` Enter → 64 Oak Log on the cursor; put it in cell 5 (cursor empty); `/gamemode survival`; click a cell to hold a stack, click the list → nothing, still held.
  - Update `r_and_u_work_over_any_slot_in_both_modes_and_open_the_drawer` (a Creative click now takes) and `the_keyboard_reaches_the_item_browser_and_typing_in_it_moves_nothing` (Enter opens the recipes of the first result in Survival; ↓ still goes to it).
- [ ] **Step 2: Run them to verify they fail.**
- [ ] **Step 3: Implement** as in Interfaces.
- [ ] **Step 4: Golden** — read the new PNG before accepting it; the others unchanged.
- [ ] **Step 5: Suite** → green.
- [ ] **Step 6: Commit** — `git commit -m "feat(client): in Creative the item browser hands out stacks and destroys the held one"`

---

### Task 5: `+ Fill grid`

**Files:**
- Modify: `client/src/ui/hud/item_browser/{mod,view,display,goldens}.rs`, `client/src/e2e/item_browser.rs`, `client/tests/golden/item_browser_uses_854x480.png`

**Interfaces:**
- `FillGridButton(pub u16)`; `style_recipe_cards` enables it (no `InteractionDisabled`, a `RecipeStop`, green as the mockup's `.fill`) when `recipe_problem` is `None` with the grid in use and no chest is open (then the note says `Not beside a chest`); disabled, it is muted and out of Tab, as in lot 2.
- `fill_grid` (observer `On<Activate>`) leaves `BrowserRequests::fill = Some((recipe, shift))`; `catalog_actions` sends `FillCraftingGrid { recipe, times: if shift { u32::MAX } else { 1 } }`.
- [ ] **Step 1: Write the failing tests**
  - `view.rs`: `fill_grid_is_enabled_when_the_recipe_fits_and_one_round_is_there` — 5 planks: the stick card enabled, the pickaxe card not (`Missing 2 Stick`); a chest open: the stick card disabled, `Not beside a chest`.
  - E2e `fill_grid_moves_a_recipe_into_the_grid`: seeded 1 oak log, 6 oak planks, Survival; E; click Stick → its recipes; click `+ Fill grid` of the oak card → 1 plank in cells (0,0), (1,0) and the result shows Stick; Shift+click it → 3 each; click the result → 4 sticks on the cursor.
- [ ] **Step 2: Run them to verify they fail.**
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Golden** — `item_browser_uses_854x480` changes (the Stick and Crafting Table cards' buttons are enabled): read the PNG, accept.
- [ ] **Step 5: Suite** → green.
- [ ] **Step 6: Commit** — `git commit -m "feat(client): + Fill grid moves a recipe into the crafting grid"`

---

### Task 6: Docs and the final check

**Files:**
- Modify: `README.md` (Controls: Creative take / one / to inventory / destroy, Fill grid, Enter in the search; Features), `docs/ARCHITECTURE.md` (the three actions and the Creative check; the browser hands out items), the spec's "Decisions".

- [ ] **Step 1: Docs** as above.
- [ ] **Step 2: Full check** — `cargo fmt --all -- --check`; `timeout 900 cargo clippy --workspace --all-targets -- -D warnings`; `timeout 1500 cargo test --workspace --no-fail-fast` green; no `.actual.png` / `.diff.png` left.
- [ ] **Step 3: Commit** — `git commit -m "docs: the Creative item browser and Fill grid in the README, the architecture and the decisions"`
