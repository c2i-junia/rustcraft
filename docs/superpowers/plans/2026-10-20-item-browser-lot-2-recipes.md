# Item Browser Lot 2 (Recipes and uses) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The item browser becomes the recipe book: a recipe view in the same panel (the recipes or the uses of an item, as cards with their 3 × 3 grid, their output, the station or why it cannot be made now, the ingredients the player lacks tinted red), a history walked back by its button and Escape, R / U over any item slot, and, in Survival, a click on a catalog item opening its recipes (right click: its uses), the green `Craftable` chip, the `OK` dot on what can be crafted now and the tooltip line `You have the ingredients`. Nothing comes out of the catalog yet, and nothing is sent to the server.

**Architecture:** `shared::world::crafting` exposes its recipe table read-only (`RecipeView`, `recipes()`, `recipes_for`, `uses_of`, `fits`) and counts what an inventory holds for a recipe (`missing`, `craftable`: the inventory slots and the crafting grid, not the chest, not the floating stack). The client keeps the view and its history in `ItemBrowserState`; the pure texts of the view live in `item_browser/recipes.rs`; the panel gains a `RecipePanel` beside the catalog's parts, shown instead of them while a view is open. The cards are rebuilt only when the view changes; their red tints and reasons are restyled when the inventory changes. A `CraftableNow` resource (the game mode and the items craftable now) is recomputed when the inventory or the mode changes, and drives the `Craftable` filter, the dots and the tooltip notes. No `InventoryAction`, no protocol change.

**Tech Stack:** Rust nightly, Bevy `=0.19.1` (`bevy::ui_widgets::{Button, Activate}`, `bevy::input_focus`, `bevy::picking::hover::Hovered`, `bevy::ui::{Checked, InteractionDisabled}`), `accesskit` 0.24, lavapipe for goldens and e2e.

**Spec:** `docs/superpowers/specs/2026-10-20-item-browser-design.md`, lot 2 ("Recipes and uses"): "Recipe view"; the Craftable chip, the `OK` square and the Survival tooltip line of "What the player sees"; the rows "Click on a catalog item" and "Right click" (Survival column), `R` / `U` and `Esc` of "Interactions"; "Shared" (`RecipeView`, `recipes`, `recipes_for`, `uses_of`, `fits`, `craftable`, `missing`). The look: the approved mockup (`renderRecipe`, `recipeCard`, `missing`, `craftableNow`). API reference: `docs/superpowers/specs/2026-10-18-bevy-ui-0.19-notes.md`.

## Global Constraints

The "Global Constraints" of `docs/superpowers/plans/2026-10-18-ui-redesign-lot-5-hotbar-inventory.md` and of `docs/superpowers/plans/2026-10-20-item-browser-lot-1-catalog.md` apply, with these readings for this lot:

- English for code, comments, docs, commit messages and game texts; Conventional Commits.
- Tests named as sentences; doc comments in full sentences; comment density like the neighbouring code.
- Every test command runs under `timeout`. `cargo fmt --all -- --check` and `timeout 900 cargo clippy --workspace --all-targets -- -D warnings` are clean at every commit; the suite is green at the end of every task.
- E2e tests in `client/src/e2e/item_browser.rs`, `#[test]` + `#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]`, driven only through `click`/`right_click`/`hover`/`press`/`type_text`/`resize`. **Two journeys**: recipes and uses in Survival; Creative, R / U and the drawer.
- No `bevy::prelude::Button`, `Interaction` or `FocusPolicy`: the back button, the tab chips, the Craftable chip and `+ Fill grid` are `bevy::ui_widgets::Button`; the slots of the cards are `widgets::slot` (hit test `Hovered`). Decorations (dashes, tints, dots, the arrow) carry `Pickable::IGNORE`.
- The role before `AccessibleLabel` in every bundle.
- **Inventory authority.** The browser never writes `Res<Inventory>` and sends no `InventoryAction` in this lot.
- Bevy's Tab navigation ignores visibility: every focusable of the hidden view (catalog or recipe view) has `TabIndex(-1)`.
- Goldens: the browser and open-inventory goldens of lot 1 change **only in Task 3**, deliberately, because the Craftable chip, the `OK` dots and the footer appear; `hotbar.png` and every menu golden must not change. The new recipe-view goldens come in Task 4. Each new or changed PNG is looked at before it is accepted.

## Review Focus

1. Escape goes, in order, to the search field, the recipe history, the drawer, then the inventory; the pause menu never opens over a view. (Task 4 unit test `escape_belongs_to_the_view_then_to_the_drawer`, e2e)
2. `missing` and `craftable` count the inventory and the grid, never the chest or the floating stack. (Task 1)
3. The view rebuilds its cards only when it changes, not every frame; the tints follow the inventory. (Task 4)
4. A hidden view never takes Tab; a focus in cards that are rebuilt goes to the back button, a focus in the view that closes goes back to the item in the catalog. (Task 4)
5. In Creative a click on a catalog item still only focuses it (lot 3 decides); R / U work in both modes. (Task 5)

## Files

- `shared/src/world/crafting.rs` — the recipe API, `missing`, `craftable` (Task 1).
- `client/src/ui/hud/item_browser/{mod,recipes (new),setup,display,goldens}.rs` — the state and history, the texts, the view, the keys (Tasks 2–5).
- `client/src/ui/widgets/tooltip.rs` — a fourth line, `note`, in the OK colour (Task 3).
- `client/src/ui/hud/inventory/display.rs` — `stack_in` and the grid in use shared with the browser (Task 5).
- `client/src/ui/menus/pause.rs` — `browser_takes_escape` (Task 4).
- `client/src/e2e/item_browser.rs`, `client/tests/golden/*.png`, `README.md`, `docs/ARCHITECTURE.md`, the spec's "Decisions".

---

### Task 1: The recipe API in shared

**Files:**
- Modify: `shared/src/world/crafting.rs`

**Interfaces:**
- Produces:
  ```rust
  #[derive(Debug, Clone, Copy, PartialEq)]
  pub struct RecipeView { pub index: u16, pub width: usize, pub height: usize,
                          pub cells: &'static [Option<ItemId>], pub output: ItemStack }
  impl RecipeView {
      pub fn fits(&self, grid_size: u32) -> bool;          // width and height ≤ grid_size
      pub fn ingredients(&self) -> Vec<(ItemId, u32)>;      // one per cell, in the order of first appearance
  }
  pub fn recipes() -> impl Iterator<Item = RecipeView>;     // RECIPES order, index = position
  pub fn recipes_for(item: ItemId) -> impl Iterator<Item = RecipeView>;
  pub fn uses_of(item: ItemId) -> impl Iterator<Item = RecipeView>;
  pub fn missing(inventory: &Inventory, recipe: &RecipeView) -> Vec<(ItemId, u32)>;
  pub fn craftable(inventory: &Inventory, recipe: &RecipeView) -> bool;
  ```
  `missing` counts the slots `0..MAX_INVENTORY_SLOTS` and the nine crafting grid slots; a chest slot or the floating stack never counts.

- [ ] **Step 1: Write the failing tests**
  - `every_recipe_is_listed_with_its_index_and_size` — 19 recipes, indices 0..19, the furnace 3×3, the stick 1×2, `cells.len() == width × height`, outputs as `match_recipe` makes them.
  - `recipes_for_and_uses_of_find_the_recipes_of_an_item` — Wooden Pickaxe: 2 recipes; Stick: 2 recipes, 10 uses; Oak Planks: 1 recipe, 8 uses; Cobblestone: none, 1 use; Bedrock: none, none.
  - `a_recipe_fits_a_grid_as_large_as_it` — sticks fit 2; the pickaxe fits 3, not 2; the axe (2×3) fits 3, not 2.
  - `the_ingredients_of_a_recipe_are_counted_by_cell` — the pickaxe: `[(OakPlanks, 3), (Stick, 2)]`.
  - `missing_counts_the_inventory_and_the_grid` — 1 oak plank in the hotbar and 2 in the grid, 1 stick in the backpack: the pickaxe misses `[(Stick, 1)]`; one more stick: nothing, `craftable`.
  - `the_chest_and_the_floating_stack_do_not_count` — planks and sticks in chest slots and on the cursor: everything missing.
- [ ] **Step 2: Run them to verify they fail** — `timeout 300 cargo test -q -p shared crafting` → compile error.
- [ ] **Step 3: Implement** as in Interfaces.
- [ ] **Step 4: Run the tests and the suite** → PASS.
- [ ] **Step 5: Commit** — `git commit -m "feat(shared): the recipes, read-only, and what an inventory misses for one"`

---

### Task 2: The view, its history and its texts

**Files:**
- Create: `client/src/ui/hud/item_browser/recipes.rs`
- Modify: `client/src/ui/hud/item_browser/mod.rs`

**Interfaces:**
- Produces, in `mod.rs`:
  ```rust
  #[derive(Clone, Copy, PartialEq, Eq, Debug)] pub enum RecipeTab { Recipes, Uses }
  #[derive(Clone, Copy, PartialEq, Eq, Debug)] pub struct ItemView { pub item: ItemId, pub tab: RecipeTab }
  // ItemBrowserState gains: pub craftable: bool, pub view: Option<ItemView>, pub history: Vec<ItemView>
  impl ItemBrowserState {
      pub fn open(&mut self, view: ItemView, compact: bool); // pushes the open view (unless the same), opens the drawer in compact
      pub fn back(&mut self);                               // pops the history, to the list at its bottom
      pub fn leave_view(&mut self);                         // back to the list, the history emptied
  }
  ```
- Produces, in `recipes.rs` (pure):
  ```rust
  pub const CRAFTABLE_NOTE: &str = "You have the ingredients";
  pub const ITEM_KEYS_HINT: &str = "R Recipes  U Uses";
  pub fn view_title(tab: RecipeTab) -> &'static str;                 // "Recipes", "Uses"
  pub fn back_label(history: &[ItemView]) -> String;                 // "<- Items", "<- Stick"
  pub fn tab_label(tab: RecipeTab, count: usize) -> String;          // "Recipes 2", "Uses 10"
  pub fn station_label(recipe: &RecipeView) -> &'static str;         // "Inventory 2×2", "Crafting Table 3×3"
  pub fn recipe_problem(inventory: &Inventory, recipe: &RecipeView, grid_size: u32) -> Option<String>;
      // "Missing 3 Oak Planks, 2 Stick", else "Needs a Crafting Table" when it does not fit the grid in use
  pub fn empty_view_message(view: ItemView) -> String;
      // "Cobblestone is gathered, not crafted. It goes into 1 recipe: see Uses." / "No recipe uses Bedrock yet."
  pub fn craftable_now(inventory: &Inventory, item: ItemId) -> bool; // some recipe of it, whatever the grid size
  ```
  `recipes.rs` items are `#[allow(dead_code)]` with "first used by the recipe view (Task 4)" until Task 4.

- [ ] **Step 1: Write the failing tests**
  - `mod.rs`: `opening_views_pushes_the_history_and_back_pops_it` — open Pickaxe recipes: history empty; open Stick recipes: history `[Pickaxe]`; open Stick recipes again: unchanged; `back` → Pickaxe; `back` → `None`; `leave_view` empties; `open` in compact opens the drawer.
  - `recipes.rs`: `the_texts_of_the_view` (title, back, tab, station); `the_problem_of_a_recipe_is_what_is_missing_then_the_table` (an empty inventory: `Missing 3 Oak Planks, 2 Stick`; all of it with grid 2: `Needs a Crafting Table`; grid 3: `None`; sticks with grid 2: `None`); `an_item_without_recipe_or_use_says_so` (Cobblestone, Oak Log with 2 uses: "recipes", Bedrock, and Wooden Sword uses); `an_item_is_craftable_now_when_some_recipe_is` (one oak log → Oak Planks yes, Stick no).
- [ ] **Step 2: Run them to verify they fail** — `timeout 300 cargo test -q -p client item_browser` → compile error.
- [ ] **Step 3: Implement** as in Interfaces.
- [ ] **Step 4: Run the tests and the suite** → PASS.
- [ ] **Step 5: Commit** — `git commit -m "feat(client): the views of the item browser, their history and their texts"`

---

### Task 3: Craftable now — the chip, the dot, the tooltip and the footer

**Files:**
- Modify: `client/src/ui/widgets/tooltip.rs` (a `note` line, `OK`, between the detail and the hint), every `Tooltip { .. }` literal; `client/src/ui/hud/item_browser/{mod,setup,display,goldens}.rs`; `client/tests/golden/*.png`

**Interfaces:**
- Produces:
  ```rust
  #[derive(Resource, Default, Clone, PartialEq, Debug)]
  pub struct CraftableNow { pub survival: bool, pub items: Vec<ItemId> } // items empty in Creative
  #[derive(Component)] pub struct CraftableChip;   // a UiButton after the category chips, Survival only
  #[derive(Component)] pub struct CraftableDot;    // the 8 px OK square in a catalog slot's top-right corner
  #[derive(Component)] pub struct ModeFooter(pub bool); // the footer of Survival (true) or Creative
  ```
  - `update_craftable_now` — the local player's mode (`Survival` without a player, as a new one) and `craftable_now` for every item, `set_if_neq`.
  - `filter_catalog` (also on `CraftableNow` changed) — keeps only `CraftableNow::items` while `state.craftable` in Survival; the empty state says `No craftable item.` (` in Wood`) when the search is empty.
  - `mark_craftable_items` (on `CraftableNow` changed or slots added) — the dot of each catalog slot; the `note` of the tooltip of the catalog and recipe slots.
  - `toggle_craftable` (observer `On<Activate>`), `style_chips` styles the Craftable chip green (`OK`, `#AEE09C` / `#3F7330`) while on, and hides it in Creative.
  - The footer: Survival `LMB recipes  RMB uses  / search`; Creative `R recipes  U uses  / search`.
  - The catalog tooltips: hint `ITEM_KEYS_HINT`.

- [ ] **Step 1: Write the failing tests** (`display.rs`)
  - `the_craftable_chip_keeps_what_can_be_crafted_now` — an inventory of one oak log, Survival: `CraftableNow.items == [OakPlanks]`; `state.craftable` → `1 / 33`; Creative → 33 shown, the chip hidden.
  - `a_craftable_item_shows_a_dot_and_says_so_in_its_tooltip` — the Oak Planks slot's dot shown and note `You have the ingredients`, the Stick's hidden and empty; Creative: none.
  - `tooltip.rs`: `the_note_shows_between_the_detail_and_the_hint`.
- [ ] **Step 2: Run them to verify they fail.**
- [ ] **Step 3: Implement** as in Interfaces.
- [ ] **Step 4: Goldens** — the four browser goldens and the four open-inventory goldens change: read every `.actual.png` (the Craftable chip at the end of the chips, the dots on Oak Planks and Furnace, the new footer); then `UPDATE_GOLDEN=1`, rerun → PASS. `hotbar.png` unchanged.
- [ ] **Step 5: Run the suite** → green.
- [ ] **Step 6: Commit** — `git commit -m "feat(client): in Survival the item browser shows what can be crafted now"` (the message says why the goldens changed).

---

### Task 4: The recipe view

**Files:**
- Modify: `client/src/ui/hud/item_browser/{mod,recipes,setup,display,goldens}.rs`, `client/src/ui/menus/pause.rs`, `client/tests/golden/*.png`

**Interfaces:**
- Produces:
  ```rust
  #[derive(Component)] pub struct BrowserTitle;          // "Items", "Recipes", "Uses"
  #[derive(Component)] pub struct CatalogPart;           // the search, the chips, the grid: hidden in a view
  #[derive(Component)] pub struct RecipePanel;           // the view
  #[derive(Component)] pub struct BackButton;            // "<- Items", with an Esc cap
  #[derive(Component)] pub struct ViewedItem;            // the icon; ViewedItemName, ViewedItemCategory texts
  #[derive(Component)] pub struct RecipeTabChip(pub RecipeTab);
  #[derive(Component)] pub struct RecipeCards;           // the column of cards in its scroll list
  #[derive(Component)] pub struct RecipeCard(pub u16);
  #[derive(Component)] pub struct RecipeSlot { pub item: ItemId, pub recipe: Option<u16>, pub count: u32 } // recipe: Some for an ingredient
  #[derive(Component)] pub struct MissingTint;           // DANGER at 32 % over a missing ingredient
  #[derive(Component)] pub struct RecipeNote(pub u16);   // the station, or the reason in DANGER
  #[derive(Component)] pub struct FillGridButton;        // "+ Fill grid", disabled in this lot
  pub fn browser_takes_escape(..) -> bool;               // replaces drawer_takes_escape
  ```
  - `show_view` (on `ItemBrowserState` changed): `CatalogPart`s and `RecipePanel` displayed by `state.view`; the title; the count empty in a view; the back label; the icon, name and category; the tab chips' labels and `Checked`; the cards rebuilt when the view differs from the one built (a card: the 3 × 3 grid of 40 px slots, gap 3, empty cells dashed; `->`; the output slot with its count; under them the note and `+ Fill grid`, `InteractionDisabled`, out of Tab), or the empty message. A focus in the rebuilt cards goes to the back button; a focus in the view that closes goes to the item's catalog slot when shown, else is let go of.
  - `style_recipe_cards` (on `Inventory` changed or cards added) — tints and notes from `missing` and the grid in use.
  - `show_item_browser` gives Tab to the catalog's stops or to the view's, whichever shows.
  - `click_items` — a left / right press on a card slot opens its recipes / uses (pushing), on a catalog slot too in Survival; Enter / Space on the focused one, ring showing.
  - `view_keys` (after `drawer_keys`) — Escape pops the history while the view shows; `drawer_keys` leaves Escape to an open view. Closing the inventory leaves the view.
  - Observers: the back button → `back`; a tab chip → that tab, same item, no push.

- [ ] **Step 1: Write the failing tests**
  - `display.rs`: `escape_belongs_to_the_view_then_to_the_drawer` — docked: no view false, view true; compact drawer closed with a view false, open true; inventory hidden false.
  - `display.rs`: `a_view_hides_the_catalog_and_builds_its_cards` (a world with the panel parts: Wooden Pickaxe recipes → `CatalogPart` hidden, title `Recipes`, 2 `RecipeCard`s, 2 × 5 ingredient slots; `back` → catalog shown, no card).
  - `display.rs`: `missing_ingredients_are_tinted_and_named` — 5 oak planks: in the oak card, the sticks tinted, not the planks, note `Missing 2 Stick` in `DANGER`; 2 sticks more → `Needs a Crafting Table`; `open_table` set → `Crafting Table 3×3` in `MUTED`.
  - Goldens: `the_recipes_of_the_wooden_pickaxe_at_1280x720` → `item_browser_recipes_1280x720` (5 oak planks: the sticks red, the spruce card all red, history `[Stick]` so the back button says `<- Stick`); `the_uses_of_oak_planks_at_854x480` → `item_browser_uses_854x480` (cards fit four columns).
- [ ] **Step 2: Run them to verify they fail.**
- [ ] **Step 3: Implement** as in Interfaces.
- [ ] **Step 4: Goldens** — read both new PNGs (cards, dashes, arrow, counts, tints, reasons, the disabled button; nothing clipped at 854) before accepting them; the other goldens unchanged.
- [ ] **Step 5: Run the suite** → green.
- [ ] **Step 6: Commit** — `git commit -m "feat(client): the recipe view of the item browser, its cards and its history"`

---

### Task 5: R / U over any item slot, and the journeys

**Files:**
- Modify: `client/src/ui/hud/item_browser/{display,mod}.rs`, `client/src/ui/hud/inventory/display.rs` (`stack_in`, `grid_in_use` made `pub(crate)`), `client/src/e2e/item_browser.rs`

**Interfaces:**
- `item_keys` (Update, not typing, inventory open): `R` / `U` open the recipes / uses of the item of the focused slot while the ring shows, else of the hovered one: catalog, card, inventory cell (hotbar, backpack, grid, chest) or the result.
- `focus_search` leaves the view.

- [ ] **Step 1: Write the failing e2e journeys**
```rust
fn recipes_and_uses_open_from_the_browser_and_the_history_walks_back()
    // seeded (0, OakPlanks, 5), (1, OakLog, 2), Survival
    press(E); hover Oak Planks → "You have the ingredients", "R Recipes  U Uses";
    click Craftable → only the craftable items, Oak Planks among them; click it again → 33;
    click Wooden Pickaxe → title "Recipes", "Recipes 2", "Uses 0", "<- Items", "Missing 2 Stick";
    click a Stick of a card → Stick's recipes, "<- Wooden Pickaxe"; right click Oak Planks of a card → "Uses";
    Escape → Stick; click the back button → Wooden Pickaxe; click "Uses 0" → "No recipe uses Wooden Pickaxe yet.";
    Escape → the catalog ("33 / 33"), the inventory open; right click Cobblestone → its uses;
    click "Recipes 0" → "Cobblestone is gathered, not crafted. It goes into 1 recipe: see Uses.";
    `/` → the catalog, the search focused; Escape ×2; Escape → the inventory closes.
fn r_and_u_work_over_any_slot_in_both_modes_and_open_the_drawer()
    // seeded (0, OakPlanks, 5); /gamemode creative
    press(E) → no Craftable chip; click Oak Log → no view; hover cell 0, R → recipes of Oak Planks;
    U → its uses, "<- Oak Planks"; resize(640, 480) after Escape ×2: hover cell 0, U → the drawer opens on the uses;
    Escape → the list, drawer open; Escape → drawer closed; Escape → inventory closed.
```
- [ ] **Step 2: Run them to verify they fail** — R does nothing.
- [ ] **Step 3: Implement** as in Interfaces.
- [ ] **Step 4: Run the suite** → green; goldens unchanged.
- [ ] **Step 5: Commit** — `git commit -m "feat(client): R and U open the recipes and the uses of any item slot"`

---

### Task 6: Docs and the final check

**Files:**
- Modify: `README.md` (Controls: Recipes `R` / LMB, Uses `U` / RMB, Back Escape; Features: the recipe book), `docs/ARCHITECTURE.md` (the recipe API and the view), the spec's "Decisions" (the deviations of this lot).

- [ ] **Step 1: Docs** as above.
- [ ] **Step 2: Full check** — `cargo fmt --all -- --check`; `timeout 900 cargo clippy --workspace --all-targets -- -D warnings`; `timeout 1500 cargo test --workspace --no-fail-fast` green; no `.actual.png` / `.diff.png` left.
- [ ] **Step 3: Commit** — `git commit -m "docs: the recipe view of the item browser in the README, the architecture and the decisions"`
