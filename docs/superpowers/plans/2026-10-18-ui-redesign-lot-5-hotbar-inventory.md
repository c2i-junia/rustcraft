# UI Redesign Lot 5 (Hotbar and Inventory) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Redesign the HUD hotbar and the inventory as one themed panel that grows upward from the hotbar, with slots and tooltips, Shift+click, the 1–9 keys, trackpad scrolling, the selected item's name, the keyboard, and a debug overlay that stays hidden, below the dialogs and quiet while the player types.

**Architecture:** Two new widgets, `ui/widgets/slot.rs` and `ui/widgets/tooltip.rs`, carry the look (sunken slot, count with shadow, accent selection, tooltip by the cursor). The HUD builds one tree: a panel whose bottom row is the `Hotbar` and whose upper section (`InventoryRoot`, title, crafting grid, 3×9 backpack) is shown while the inventory is open. Slots are found under the cursor through picking (`Hovered`), not the legacy `Interaction`. Every change to the inventory is still an `InventoryAction` pushed to `CurrentFrameInputs` before `GameSet::Simulation`, predicted by the client and applied by the server with the same `shared` code; two actions are added there (Shift+click, number-key swap).

**Tech Stack:** Rust nightly, Bevy `=0.19.1` (`bevy::picking::hover::Hovered`, `Pickable`, `bevy::input_focus`, `TextShadow`, `ComputedStackIndex`), `accesskit` 0.24, lavapipe for goldens and e2e.

**Spec:** `docs/superpowers/specs/2026-10-18-ui-redesign-design.md` (only "Lot 5 — Hotbar and inventory", and the "HUD", "Inventory" and "Debug" paragraphs of "Screens"; "Visual system", "Architecture", "Testing" and the anti-goals bind every task). Defects to fix: `docs/superpowers/specs/2026-10-18-ui-e2e-tests-design.md`, "Non-goals" (the HUD, inventory and debug lines). API reference: `docs/superpowers/specs/2026-10-18-bevy-ui-0.19-notes.md` ("notes §n"). Conventions and products of Lot 0: `docs/superpowers/plans/2026-10-18-ui-redesign-lot-0-foundations.md`.

## Global Constraints

- English for code, comments, docs and commit messages; Conventional Commits (`feat(client): …`, `feat(shared): …`, `fix(client): …`, `test(client): …`, `docs: …`).
- Match the surrounding style: doc comments in full sentences, tests named as sentences, comment density like the neighbouring code.
- Every test command runs under `timeout` (e.g. `timeout 300 cargo test -q -p client e2e::inventory`). The full suite `timeout 900 cargo test --workspace --no-fail-fast` is green at the end of every task.
- `cargo fmt --all -- --check` and `timeout 900 cargo clippy --workspace --all-targets -- -D warnings` are clean at every commit.
- The dev profile builds the client with Cranelift, which runs no destructor while a panic unwinds: a failing test's harness is never dropped. Do not rely on `Drop` in a failing path.
- Every e2e test is `#[test]` + `#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]`. Widget probes live in `client/src/e2e/widgets.rs` (start from `main_menu()`, probes under a `GlobalZIndex(1)` root, driven only through `click`/`hover`/`press`/`type_text`). In-game tests go in the new `client/src/e2e/inventory.rs` (panel, clicks, keyboard) and `client/src/e2e/hud.rs` (hotbar keys, wheel, item name, debug overlay); each `enter_world` starts an integrated server, so **one journey per behaviour group**, chaining its checks, keeps the suite under ~2 minutes.
- Never a bare `git stash`; set work aside with a WIP commit.
- `bevy::prelude::Button` is the legacy widget; slots do not use it, nor `Interaction`, nor `FocusPolicy`. The hit test of a slot is `bevy::picking::hover::Hovered`. Nodes that sit under the cursor without being targets (the held stack, the tooltip, the reticle, the item name) carry `Pickable::IGNORE`.
- Put the widget's role (`AccessibilityNode(accesskit::Node::new(Role::GridCell))`, `Role::Tooltip`) **before** `AccessibleLabel` in a bundle (notes §5).
- A widget no screen uses yet allows `dead_code` on the item with a comment naming its first user; the allowance is removed in the task that uses it. Never crate-wide.
- **Inventory authority.** The UI never writes `Res<Inventory>`. A click, Shift+click, number key or keyboard action becomes an `InventoryAction` pushed to `CurrentFrameInputs.0.inventory_actions` by a system in the HUD chain that runs `.after(GameSet::UiMode).before(GameSet::Simulation)`; `predict_inventory_system` predicts it and the server applies it through the same `shared::players::inventory::apply_inventory_action`. `InventoryAction` is append-only: new variants go last, `PROTOCOL_ID` is bumped once (4 → 5), and the pin test in `shared/src/messages/mod.rs` names them.
- The systems that change `Hotbar::selected` run `.before(GameSet::Input)`, because `update_frame_inputs_system` sends the selected slot to the server in that set.
- `InventoryRoot`'s `Visibility` stays the open/closed state of the inventory (read by `set_ui_mode` through `UiDialog`, by the pause menu's Escape, and by the crafting table in `player/interactions.rs`).
- Gameplay keys act only while `*ui_mode == UIMode::Closed`, the pattern already used by movement, F5, R, O and P. The digits 1–9 are hard-wired (`KeyCode::Digit1..=Digit9`), not added to `KeyMap`: the controls screen is redesigned in Lot 1.
- Pixel art: item icons are `ICON_SIZE = 32.0` logical px (twice their 16 px texture), so they are 32, 48, 64 and 96 physical px at UiScale 1, 1.5, 2 and 3. A node placed from the cursor divides `Window::cursor_position()` by `UiScale` (UI `Val::Px` is scaled by it).
- Goldens: the four inventory goldens change **only in Task 3**, deliberately, after a human look at the PNGs; they hold nothing time-dependent (no item name, no tooltip, no cursor) and are captured after their fonts are loaded. Any other golden that changes is a bug.

## Review Focus

1. Trackpads send many small pixel deltas, sometimes reversing: the selection moves one slot per 20 px accumulated and a reversal drops the remainder, so it neither stalls nor jumps. (Task 5: `reversing_the_wheel_drops_the_remainder`)
2. At UiScale 1.5 (1920×1080) the held stack is centred on the cursor at slot size, and a click places it in the slot under the cursor, not on the held stack itself. (Task 3: `the_held_stack_follows_the_cursor_and_stray_clicks_do_nothing`)
3. A slot near the right or bottom edge of the window keeps its tooltip inside the window. (Task 2: `the_tooltip_stays_inside_the_window`)
4. Tab pressed during gameplay, inventory closed, puts no focus or ring on the hotbar and leaves the game keys working. (Task 6: `tab_in_gameplay_leaves_the_hotbar_unfocused`)
5. A number key with the inventory open over no slot sends nothing (neither a swap nor a selection), and over a slot while holding a stack changes nothing. (Task 4: `a_number_key_over_nothing_sends_nothing`; Task 1: `a_number_key_is_refused_while_the_cursor_holds_a_stack`)
6. A mouse button pressed to close a UI (a click outside the chat, "Resume" in the pause menu) is still held for a few frames after `UIMode` becomes `Closed`: it must not hit or place a block. (Task 7: `the_click_that_closes_a_ui_does_not_reach_the_world`)

## Files

- `shared/src/players/inventory.rs`, `shared/src/constants.rs`, `shared/src/messages/mod.rs` — the two new actions (Task 1).
- `client/src/ui/widgets/slot.rs`, `client/src/ui/widgets/tooltip.rs` (new) — the widgets (Task 2).
- `client/src/ui/hud/inventory/{mod,setup,display,goldens}.rs` — the one panel and its actions (Tasks 3, 4, 6).
- `client/src/ui/hud/hotbar.rs`, `client/src/ui/hud/reticle.rs` — the HUD hotbar systems, item name, reticle (Task 5).
- `client/src/ui/hud/debug/{mod,setup}.rs`, `client/src/ui/hud/mod.rs`, `client/src/main.rs` — the debug overlay (Task 7).
- `client/src/e2e/{inventory,hud}.rs` (new), `client/src/e2e/{game,input,widgets,review}.rs`, `client/src/ui/golden.rs`, `client/tests/golden/*.png`, `README.md`, `docs/ARCHITECTURE.md`.

---

### Task 1: Shift+click and number-key swap in the shared inventory model

**Files:**
- Modify: `shared/src/players/inventory.rs` (actions, refusal, tests), `shared/src/constants.rs` (`PROTOCOL_ID`, `HOTBAR_SLOTS`), `shared/src/messages/mod.rs` (pin test), `client/src/constants.rs` (`MAX_HOTBAR_SLOTS = shared::HOTBAR_SLOTS`)

**Interfaces:**
- Produces: `pub const HOTBAR_SLOTS: u32 = 9;` (shared, re-exported like `MAX_INVENTORY_SLOTS`); `InventoryAction::QuickMoveSlot(u32)` (variant 5) and `InventoryAction::SwapWithHotbar { slot: u32, hotbar: u32 }` (variant 6); `InventoryRefusal::HoldingAStack`; `PROTOCOL_ID = 5`.

- [ ] **Step 1: Write the failing tests** (in the tests module of `inventory.rs`, through the existing `apply` helper; `stack(item, nb)` as there)

```rust
fn shift_click_moves_a_hotbar_stack_into_the_backpack()   // slot 0 dirt 5 → slot 9 dirt 5, slot 0 empty
fn shift_click_moves_a_backpack_stack_into_the_hotbar()   // slot 20 dirt 5 → slot 0
fn shift_click_tops_up_stacks_of_the_same_item_first()    // slot 3 dirt 60, source slot 20 dirt 10 → slot 3 = 64, slot 0 = 6, slot 20 empty
fn shift_click_keeps_what_does_not_fit()                  // slots 0..9 stone 64, slot 20 dirt 5 → slot 20 still dirt 5
fn shift_click_on_a_grid_cell_puts_it_back_into_the_inventory() // grid_slot(0, 0) planks 4 → in a slot < MAX_INVENTORY_SLOTS, cell empty
fn shift_click_leaves_the_held_stack_alone()              // floating stick 3 before and after
fn a_number_key_swaps_a_slot_with_that_hotbar_slot()      // slot 12 dirt 5, slot 2 stick 3, SwapWithHotbar { slot: 12, hotbar: 2 } → 12 stick 3, 2 dirt 5; and with slot 4 empty → 12 empty, 4 dirt
fn a_number_key_is_refused_while_the_cursor_holds_a_stack() // floating dirt 1 → Err(HoldingAStack), slots unchanged
fn a_number_key_on_a_cell_beyond_the_grid_is_refused()     // no table, grid_slot(2, 2) → Err(InvalidSlot(grid_slot(2, 2)))
fn a_number_key_beyond_the_hotbar_is_refused()             // hotbar: 9 → Err(InvalidSlot(9))
```

In `messages/mod.rs` `network_actions_and_mob_enums_are_pinned`: `(InventoryAction::QuickMoveSlot(0), 5)`, `(InventoryAction::SwapWithHotbar { slot: 0, hotbar: 0 }, 6)`.

- [ ] **Step 2: Run them to verify they fail**

Run: `timeout 300 cargo test -q -p shared inventory messages`
Expected: compile error on the new variants.

- [ ] **Step 3: Implement in `apply_inventory_action`**

- `QuickMoveSlot(slot)`: `check_slot` as for a click. Source in `0..HOTBAR_SLOTS` → target range `HOTBAR_SLOTS..MAX_INVENTORY_SLOTS`; source in the backpack → `0..HOTBAR_SLOTS`; a crafting grid cell (usable, or a leftover beyond it) → `put_back` (the whole inventory). Within a range: first top up stacks of the same item in slot order, then fill empty slots in slot order (`fn add_to_range(inventory: &mut Inventory, stack: ItemStack, range: Range<u32>) -> u32`, returning what did not fit). What does not fit stays in the source. The floating stack is untouched; an empty source is `Ok(())`.
- `SwapWithHotbar { slot, hotbar }`: `hotbar >= HOTBAR_SLOTS` → `InvalidSlot(hotbar)`; a held stack → `HoldingAStack`; `slot` must be `< MAX_INVENTORY_SLOTS` or in the usable grid, else `InvalidSlot(slot)`; exchange the two entries of `inner` (no merging); `slot == hotbar` is `Ok(())`.
- Bump `PROTOCOL_ID` to 5. The server needs no change: it runs `simulate_inventory_actions` too.

- [ ] **Step 4: Run the tests and the suite** → PASS; full suite green.

- [ ] **Step 5: Commit** — `git commit -m "feat(shared): Shift+click and number-key swap inventory actions"`

---

### Task 2: The slot and tooltip widgets

**Files:**
- Create: `client/src/ui/widgets/slot.rs`, `client/src/ui/widgets/tooltip.rs`
- Modify: `client/src/ui/widgets/mod.rs` (modules and plugins), `client/src/e2e/widgets.rs`

**Interfaces:**
- Consumes: `theme::*`.
- Produces, in `slot.rs`:
  ```rust
  pub const SLOT_SIZE: f32 = 50.0;
  /// Twice the 16 px item textures: a whole multiple of them at every UiScale step.
  pub const ICON_SIZE: f32 = 32.0;
  #[derive(Component)] #[require(SlotSelected, Hovered)] pub struct Slot;
  #[derive(Component, Default, Clone, Copy, PartialEq, Eq, Debug)] pub struct SlotSelected(pub bool);
  #[derive(Component)] pub struct SlotIcon;
  #[derive(Component)] pub struct SlotCount;
  pub fn slot_node() -> Node;   // 50×50, border BEVEL_WIDTH, centred content; callers override grid placement with `..slot_node()`
  pub fn slot(label: &str, number: Option<u32>, icon: ImageNode, font: Handle<Font>) -> impl Bundle;
  pub fn slot_icon(icon: ImageNode) -> impl Bundle;      // (SlotIcon, icon, Node 32×32, Visibility::Hidden, Pickable::IGNORE)
  pub fn slot_count(font: Handle<Font>) -> impl Bundle;  // (SlotCount, Text "", FONT_S, TEXT, TextShadow { offset: Vec2::splat(2.0), color: BEVEL_LO }, absolute bottom-right, Pickable::IGNORE)
  pub fn count_label(nb: u32) -> String;                 // "" for 0 and 1
  pub(crate) fn style_slots(...);                        // pub(crate): the goldens run it
  ```
  `slot()` = `(Slot, AccessibilityNode(Node::new(Role::GridCell)), AccessibleLabel::new(label), slot_node(), BackgroundColor(SLOT), sunken bevel, children![slot_icon(icon), slot_count(font.clone()), number])`, where `number` is the digit in `MUTED`, `FONT_S`, absolute top-left, only when `Some`. `style_slots` (in `PostUpdate`, on `Changed<Hovered>` or `Changed<SlotSelected>`) applies `fn slot_colors(hovered: bool, selected: bool) -> (Color, BorderColor)`: background `SLOT`, `RAISE_HOVER` when hovered; border sunken (top/left `BEVEL_LO`, bottom/right `BEVEL_HI`), all `ACCENT` when selected.
- Produces, in `tooltip.rs`:
  ```rust
  #[derive(Component, Clone, Default, PartialEq, Debug)] #[require(Hovered)]
  pub struct Tooltip { pub title: String, pub detail: String, pub hint: String }  // an empty title shows none
  #[derive(Component)] pub struct TooltipRoot;
  fn tooltip_position(cursor: Vec2, size: Vec2, window: Vec2) -> Vec2;  // logical UI px
  ```
  `tooltip::plugin` spawns one `TooltipRoot` at `Startup` (`Role::Tooltip`, `GlobalZIndex(100)`, `Pickable::IGNORE`, `Display::None`, `PANEL` background, raised bevel, padding `SPACE_S`, column of three `Text`s: title `TEXT`, detail and hint `MUTED`, `FONT_S`, an empty line `Display::None`), and a system in `Update` that shows the `Tooltip` of the first hovered entity (`Hovered(true)`, visible) at `tooltip_position(cursor / UiScale, size, window / UiScale)`, or hides the root. Offset `TOOLTIP_OFFSET = 12.0` right and below the cursor; flipped to the left / above when it would leave the window, then clamped to it.

- [ ] **Step 1: Write the failing tests**

Unit, `slot.rs`:
- `a_count_shows_from_two_items` — `count_label(0) == ""`, `(1) == ""`, `(2) == "2"`, `(64) == "64"`.
- `a_slot_is_sunken_lightens_on_hover_and_takes_the_accent_when_selected` — `slot_colors(false, false)` = (`SLOT`, sunken bevel); `(true, false).0 == RAISE_HOVER`; `(false, true).1 == BorderColor::all(ACCENT)`.
- `icons_are_whole_multiples_of_their_texels_at_every_scale` — for `s` in `[1.0, 1.5, 2.0, 3.0]`: `(ICON_SIZE * s) % 16.0 == 0.0`; and `SLOT_SIZE - ICON_SIZE - 2.0 * BEVEL_WIDTH` is even (the icon is centred on whole pixels).
- `a_slot_has_the_grid_cell_role_and_its_label` — spawn `slot("Hotbar slot 1", Some(1), ImageNode::default(), Handle::default())` in a bare `World`: `Role::GridCell`, label `"Hotbar slot 1"`.

Unit, `tooltip.rs`: `the_tooltip_stays_inside_the_window` — window (800, 600), size (100, 40): cursor (10, 10) → (22, 22); cursor (790, 300) → (678, 312); cursor (790, 590) → (678, 538); cursor (50, 590) → (62, 538).

E2e, `e2e/widgets.rs` (probes: a `slot("Probe slot", Some(3), ImageNode::default(), Handle::default())` and a 100×40 node with `Tooltip { title: "Dirt", detail: "5 items", hint: "Shift+click: move to the hotbar" }`, plus a plain probe "Elsewhere"):
- `hovering_a_slot_lightens_it` — `hover(&L::role(Role::GridCell, "Probe slot"))` → `BackgroundColor(RAISE_HOVER)`; insert `SlotSelected(true)`, one frame → border `ACCENT`.
- `hovering_an_element_with_a_tooltip_shows_it_by_the_cursor` — hover the tooltip probe → `L::text("Dirt").within(L::marker::<TooltipRoot>())`, `"5 items"` and the hint visible; the root's top-left is right and below the probe's centre; hover "Elsewhere" → `!is_visible(&L::marker::<TooltipRoot>())`.
- `an_empty_tooltip_shows_nothing` — `Tooltip::default()` on the probe, hover → root not visible.

- [ ] **Step 2: Run them to verify they fail** — `timeout 300 cargo test -q -p client -- ui::widgets::slot ui::widgets::tooltip e2e::widgets` → compile error.

- [ ] **Step 3: Implement** the two files and add their plugins to `widgets_plugin`. `slot`, `slot_icon`, `slot_count`, `count_label`, `SLOT_SIZE`, `ICON_SIZE`: `#[allow(dead_code)]` with "first used by the inventory panel (Lot 5, Task 3)".

- [ ] **Step 4: Run the tests and the suite** → PASS, full suite green, no golden changed.

- [ ] **Step 5: Commit** — `git commit -m "feat(client): slot and tooltip widgets"`

---

### Task 3: One panel — the inventory grows upward from the hotbar

**Files:**
- Modify: `client/src/ui/hud/inventory/setup.rs` (rewrite), `client/src/ui/hud/inventory/display.rs` (systems and unit tests), `client/src/ui/hud/inventory/mod.rs` (markers), `client/src/ui/hud/inventory/goldens.rs`, `client/src/ui/hud/hotbar.rs` (drop `setup_hotbar`), `client/src/ui/hud/mod.rs` (setup and system lists), `client/src/constants.rs` (drop `HOTBAR_CELL_SIZE`, `HOTBAR_PADDING`, `HOTBAR_BORDER`), `client/src/ui/widgets/slot.rs` (drop the `dead_code` allowances), `client/src/ui/golden.rs` (`wait_for_fonts`), `client/src/e2e/input.rs` (`hover_at`), `client/src/e2e/game.rs` (locators, `slot`/`floating` become `pub(super)`), `client/src/e2e/mod.rs`, `client/tests/golden/{hotbar,inventory_2x2_grid,inventory_leftovers_beyond_the_grid,crafting_table_3x3_grid}.png`
- Create: `client/src/e2e/inventory.rs`

**Interfaces:**
- Consumes: `slot`, `slot_node`, `slot_icon`, `slot_count`, `SlotSelected`, `SlotIcon`, `SlotCount`, `count_label`, `style_slots`, `SLOT_SIZE`; `load_font`.
- Produces: `pub fn setup_inventory(commands, materials: Res<MaterialResource>, asset_server: Res<AssetServer>)` spawning the whole tree:
  `HudBottom` (absolute, `left: 0`, `right: 0`, `bottom: px(HOTBAR_BOTTOM)` with `const HOTBAR_BOTTOM: f32 = SPACE_L`, `justify_content: Center`, `Pickable::IGNORE`, `DespawnOnExit(Game)`, `GlobalZIndex(1)`) → `InventoryDialog` (the panel: column, padding `SPACE_S`, `row_gap: SPACE_M`, border `BEVEL_WIDTH`) → [`InventoryRoot` (`UiDialog`, `Visibility::Hidden`; column: `InventoryTitle`, crafting row, 3×9 backpack grid of slots 9..36 with gap `SPACE_XS`), `Hotbar { selected: 0 }` (row of slots 0..9, gap `SPACE_XS`, numbers 1–9)]. `FloatingStack` is its own root (`GlobalZIndex(3)`, `Pickable::IGNORE`, `SLOT_SIZE`×`SLOT_SIZE`, `slot_icon` centred, `slot_count`). Slot labels: `"Hotbar slot {n}"` (1–9), `"Backpack slot {n}"` (1–27), `"Crafting grid {row}, {col}"` (1-based), `"Crafting result"`. `StackIcon` / `StackCount` are replaced by `SlotIcon` / `SlotCount`; `CELL_BORDER_COLOR` goes.
- Produces: `pub fn style_inventory_panel(...)` — the panel has a `PANEL` background and a raised bevel while `InventoryRoot` is visible, `Color::NONE` for both while hidden. `display_inventory_system` sets `SlotSelected` on the selected hotbar cell whether the inventory is open or not, shows `count_label(nb)`, and places `FloatingStack` centred on the cursor (`cursor / UiScale - SLOT_SIZE / 2`). `click_inventory_cells_system` reads `&Hovered` instead of `&Interaction`.
- Produces (harness): `Harness::hover_at(&mut self, position: Vec2)` — moves the cursor to a physical point for a frame, no click.

- [ ] **Step 1: Write the failing tests**

Unit, `display.rs` (the `Hud` helper spawns cells with `Hovered(false)` and `SlotSelected::default()`; `click` inserts `Hovered(true)` instead of `Interaction::Pressed`; a `ButtonInput<KeyCode>` and `UiScale` resource are added):
- `a_cell_shows_the_count_of_its_slot_and_nothing_when_empty` gains: a stack of 1 shows `""`.
- `the_selected_hotbar_cell_is_outlined_while_the_inventory_is_closed` becomes `the_selected_hotbar_cell_stays_selected_when_the_inventory_opens` — `SlotSelected(true)` on cell 0 and `false` on cell 1, with the root hidden and visible.
- `the_panel_has_its_background_only_while_open` — run `style_inventory_panel`: hidden → `BackgroundColor(Color::NONE)`; visible → `BackgroundColor(PANEL)`.

E2e, `e2e/inventory.rs`:
```rust
fn the_open_inventory_leaves_the_hotbar_alone_and_fits_the_window()
    // world seeded with (0, Dirt, 64), (10, Sand, 33) and (grid_slot(2, 2), Cobblestone, 1):
    // the leftover cell gives the crafting row its 3×3 height
    for (w, h_) in [(800, 600), (1280, 720), (1366, 768)] {
        h.resize(w, h_); h.hover_at(top-right corner); h.frames(2);
        let before = rects of the 9 cells within Hotbar (UiGlobalTransform + ComputedNode) and their pixels in h.screenshot();
        h.press(KeyE); h.frames(2);
        assert_eq!(rects after, before);                       // not moved
        assert every pixel inside each hotbar cell (inset 2 px) within 16 per channel of before;  // not covered, not dimmed
        assert every visible InventoryCell / CraftResultCell rect and the InventoryDialog rect lie inside the window;
        assert every cell outside the Hotbar ends above the top of the hotbar cells;
        h.press(KeyE);
    }
fn the_held_stack_follows_the_cursor_and_stray_clicks_do_nothing()
    // Options { width: 1920, height: 1080 } (UiScale 1.5); seeded (0, Stick, 3), (10, Dirt, 5)
    E; click cell 10 → wait_until floating == Dirt 5;
    hover cell 20 → FloatingStack centre within 1 px of center_of(cell 20), size == cell 20's ComputedNode size;
    the SlotIcon within FloatingStack and within cell 0 are 48×48 physical;
    click_at(midpoint between the bottom of cell 27 and the top of cell 0) → after 10 frames floating still Dirt 5, slot 0 still Stick 3, slot 27 empty;
    click_at(window corner (10, 10)) → still held, InventoryRoot visible;
    click cell 20 → wait_until slot 20 == Dirt 5 and floating None.
```
`the_inventory_opens_closes_and_moves_stacks` (`e2e/game.rs`) keeps its journey: `StackIcon` / `StackCount` become `SlotIcon` / `SlotCount`, and the result count of one crafting table is now `""` (no count for 1).

- [ ] **Step 2: Run them to verify they fail**

Run: `timeout 300 cargo test -q -p client -- hud::inventory e2e::inventory`
Expected: compile errors, then the e2e tests fail on the moved/dimmed hotbar and the 20×20 held stack.

- [ ] **Step 3: Implement**

- `setup_inventory` builds the tree above; `hud_plugin` runs it alone at `OnEnter(Game)` (`setup_hotbar` is deleted; `Hotbar` stays in `hotbar.rs`). Crafting grid cells are `(InventoryCell, CraftingGridCell, slot(..))` with `.insert(Node { grid_row, grid_column, ..slot_node() })`, placed by hand as today. The arrow `"->"` becomes `Text` in `MUTED`.
- `style_inventory_panel` runs in the HUD chain after `display_inventory_system`.
- `UiGolden::wait_for_fonts(&mut self)` runs frames until every font of a `TextFont` is loaded; `assert_matches` calls it first. `assert_inventory_matches` runs `setup_inventory`, sets the root visible if `open`, then `display_inventory_system`, `display_crafting_system`, `style_inventory_panel`, `style_slots`; it inits any resource those systems read that `UiGolden` lacks.

- [ ] **Step 4: Run the tests; regenerate the four goldens on purpose**

Run the unit and e2e tests → PASS except the four inventory goldens. Read each `client/tests/golden/<name>.actual.png`: one panel; the hotbar as its bottom row with numbers 1–9 and an accent border on slot 1; sunken slots on the slot colour; counts bottom-right with a shadow, none on a stack of 1; icons 32 px and crisp; in `hotbar.png`, only the hotbar row (no panel background). Then `UPDATE_GOLDEN=1 timeout 300 cargo test -q -p client -- hud::inventory::goldens`, rerun without it → PASS, no `.actual.png` / `.diff.png` left. No menu golden changes. Full suite green.

- [ ] **Step 5: Commit** (the report lists the changed locators of `e2e/game.rs` and the four regenerated goldens)

`git commit -m "feat(client): one inventory panel that grows upward from the hotbar"`

---

### Task 4: Shift+click, the number keys over a slot, and tooltips

**Files:**
- Modify: `client/src/ui/hud/inventory/display.rs`, `client/src/ui/hud/inventory/mod.rs` (`item_name`), `client/src/ui/hud/hotbar.rs` (`HOTBAR_KEYS`), `client/src/ui/hud/inventory/setup.rs` (a `Tooltip` on each cell), `client/src/e2e/inventory.rs`, `README.md` (Inventory rows of the controls table)

**Interfaces:**
- Consumes: `InventoryAction::{QuickMoveSlot, SwapWithHotbar}`, `Tooltip`.
- Produces: `pub const HOTBAR_KEYS: [KeyCode; 9]` (`Digit1..=Digit9`) and `pub fn hotbar_key_index(input: &ButtonInput<KeyCode>) -> Option<u32>` (the first just-pressed digit) in `hotbar.rs`; `pub fn item_name(item: ItemId) -> String` in `inventory/mod.rs`; hints in `display.rs`: `HINT_HOTBAR = "Shift+click: move to the backpack"`, `HINT_BACKPACK = "Shift+click: move to the hotbar"`, `HINT_GRID = "Shift+click: move to the inventory"`, `HINT_RESULT = "Click: craft"`; `fn count_detail(nb: u32) -> String` (`"1 item"`, `"5 items"`).

- [ ] **Step 1: Write the failing tests**

Unit, `inventory/mod.rs`: `item_names_are_split_into_words` — `Dirt → "Dirt"`, `OakPlanks → "Oak Planks"`, `CraftingTable → "Crafting Table"`, `TallGrass → "Tall Grass"`.

Unit, `display.rs` (`Hud` helper; Shift is `ShiftLeft` or `ShiftRight` held in `ButtonInput<KeyCode>`):
- `shift_click_sends_a_quick_move` — Shift held, left click on cell 12 → `[QuickMoveSlot(12)]`; right click with Shift → `[RightClickSlot(12)]` (Shift only changes the left button).
- `a_number_key_over_a_slot_sends_a_swap` — cell 12 hovered, `Digit3` just pressed → `[SwapWithHotbar { slot: 12, hotbar: 2 }]`.
- `a_number_key_over_nothing_sends_nothing` — no cell hovered, `Digit3` → `[]`; inventory closed with cell 12 hovered → `[]`.
- `a_cell_s_tooltip_names_its_item_count_and_hint` — cell 12 with dirt 5 → `Tooltip { title: "Dirt", detail: "5 items", hint: HINT_BACKPACK }`; cell 0 → `HINT_HOTBAR`; a grid cell → `HINT_GRID`; the result cell with a recipe → title of the result, `HINT_RESULT`.
- `no_tooltip_on_an_empty_slot_while_holding_a_stack_or_while_closed` — each case → `Tooltip::default()`.

E2e, `e2e/inventory.rs`:
```rust
fn shift_click_number_keys_and_tooltips_in_the_open_inventory()
    // seeded (2, Stick, 3), (10, Dirt, 5); E
    hover cell 10 → L::text("Dirt") and L::text("5 items") and L::text(HINT_BACKPACK) within TooltipRoot;
    key_down(ShiftLeft); click cell 10; key_up(ShiftLeft) → wait_until slot 0 == Dirt 5, slot 10 empty;
    key_down(ShiftLeft); click cell 0; key_up(ShiftLeft) → wait_until slot 9 == Dirt 5;
    hover cell 9; press(Digit3) → wait_until slot 2 == Dirt 5 and slot 9 == Stick 3;
    press(KeyE) → TooltipRoot not visible.
```

- [ ] **Step 2: Run them to verify they fail** — `timeout 300 cargo test -q -p client -- hud::inventory e2e::inventory` → compile errors, then no quick move is sent.

- [ ] **Step 3: Implement**

- `click_inventory_cells_system`: a left press with Shift pushes `QuickMoveSlot(id)` (the result cell still pushes `TakeCraftResult`); while open, `hotbar_key_index` with a hovered cell pushes `SwapWithHotbar { slot: id, hotbar }` (the result cell ignores digits).
- `display_inventory_system` / `display_crafting_system` set each cell's `Tooltip` (`set_if_neq`): `Tooltip::default()` while closed, while `inventory.floating` is `Some`, or on an empty slot.
- README, Inventory rows: add "Move a stack between backpack and hotbar — Shift + Left Click" and "Swap with a hotbar slot — 1–9 over a slot".

- [ ] **Step 4: Run the tests and the suite** → PASS, full suite green, goldens unchanged (no hover in them).

- [ ] **Step 5: Commit** — `git commit -m "feat(client): Shift+click, number keys over a slot and tooltips in the inventory"`

---

### Task 5: The HUD hotbar — 1–9, trackpad scrolling, item name, reticle

**Files:**
- Modify: `client/src/ui/hud/hotbar.rs`, `client/src/ui/hud/inventory/display.rs` (the wheel system moves to `hotbar.rs`), `client/src/ui/hud/inventory/setup.rs` (the item name node), `client/src/ui/hud/reticle.rs`, `client/src/ui/hud/mod.rs`, `client/src/e2e/input.rs` (`scroll_pixels`), `client/src/e2e/hud.rs` (new), `client/src/e2e/mod.rs`, `README.md` (Hotbar rows)

**Interfaces:**
- Consumes: `HOTBAR_KEYS`, `hotbar_key_index`, `item_name`, `Hotbar`.
- Produces, in `hotbar.rs`:
  ```rust
  const SCROLL_PIXELS_PER_SLOT: f32 = 20.0;
  pub const ITEM_NAME_DURATION: Duration = Duration::from_secs(2);
  #[derive(Component, Default)] pub struct HotbarItemName { pub hide_at: Option<Duration> }
  fn scroll_steps(remainder: &mut f32, unit: MouseScrollUnit, y: f32) -> i32;
  pub fn select_hotbar_slot_with_keys(...);   // Closed only
  pub fn select_hotbar_slot_with_wheel(...);  // Closed only; Local<f32> remainder, reset when not Closed
  pub fn show_selected_item_name(...);        // on a change of `selected` (Local<Option<u32>>; the first frame shows nothing)
  ```
  `HotbarItemName` is a `Text` (`FONT_M`, `TEXT`, `TextShadow`, `Pickable::IGNORE`, `Visibility::Hidden`) spawned by `setup_inventory`, absolute above the `Hotbar` row (bottom at the row's top, `SPACE_S` apart, centred).
  `Harness::scroll_pixels(&mut self, pixels: f32)` writes a `MouseScrollUnit::Pixel` wheel (both messages, like `scroll`).

- [ ] **Step 1: Write the failing tests**

Unit, `hotbar.rs`:
- `line_scrolling_moves_one_slot_per_line` — `scroll_steps(&mut 0., Line, -1.) == 1`, `(Line, 2.) == -2`.
- `pixel_scrolling_accumulates_into_steps` — four `(Pixel, -5.)` → `0, 0, 0, 1`; then `(Pixel, -45.)` → 2, remainder 5 px.
- `reversing_the_wheel_drops_the_remainder` — `(Pixel, -15.)` → 0; `(Pixel, 10.)` → 0 with the remainder now `+10` px worth, not `-5`; `(Pixel, 10.)` → -1.
- `the_item_name_shows_for_two_seconds_when_the_selection_changes` — `World` with `Time<()>`, `Inventory` (slot 2 planks 4), a `Hotbar`, a `HotbarItemName`, `InventoryRoot` hidden: run once (no show); set `selected = 2`, run → text `"Oak Planks"`, `Visibility::Inherited`; `advance_by(1.9 s)`, run → shown; `advance_by(0.2 s)`, run → `Hidden`.
- `the_item_name_stays_hidden_on_an_empty_slot_or_with_the_inventory_open`.

Unit, `reticle.rs`: `the_reticle_has_a_dark_outline_and_lets_clicks_through` — `spawn_reticle` in a `World`: a 22×4 and a 4×22 node in `BEVEL_LO` come before (child order) the 20×2 and 2×20 white bars; every reticle node has `Pickable::IGNORE`.

E2e, `e2e/hud.rs`:
```rust
fn number_keys_the_trackpad_and_the_item_name_drive_the_hotbar()
    // seeded (2, OakPlanks, 4)
    press(Digit3) → selected 2; L::text("Oak Planks") visible;
    press(Digit9) → selected 8; "Oak Planks" not visible (empty slot hides it);
    4 × scroll_pixels(-5.) → selected 0 (wraps); scroll(-1.) → 1;
    press(KeyE); press(Digit5) → selected still 1; press(KeyE).
```
`the_wheel_moves_the_hotbar_selection` (`e2e/game.rs`) is unchanged and still passes; `the_hud_shows_and_f3_toggles_the_debug_overlay` still counts two reticle bars.

- [ ] **Step 2: Run them to verify they fail** — `timeout 300 cargo test -q -p client -- hud::hotbar hud::reticle e2e::hud` → compile errors, then Digit3 selects nothing and the pixel scroll never moves.

- [ ] **Step 3: Implement**

- `hud_plugin`: the three hotbar systems join the HUD chain, `.before(GameSet::Input)`; `select_hotbar_slot_system` is replaced by `select_hotbar_slot_with_wheel` (selection `-= steps`, `rem_euclid(MAX_HOTBAR_SLOTS)`). A hide system for the name, `Update`, compares `Time::elapsed()` with `hide_at`.
- `scroll_steps`: add `y` (lines) or `y / SCROLL_PIXELS_PER_SLOT` (pixels) to the remainder, first zeroing it when the signs differ; return the remainder's whole part (`trunc`) and keep the fraction.
- README: Hotbar rows "Select a slot — 1–9", "Next / previous slot — Mouse wheel".

- [ ] **Step 4: Run the tests and the suite** → PASS, full suite green, goldens unchanged (the name starts hidden).

- [ ] **Step 5: Commit** — `git commit -m "feat(client): number keys, trackpad scrolling and the item name on the hotbar, and an outlined reticle"`

---

### Task 6: The inventory with the keyboard alone

**Files:**
- Modify: `client/src/ui/widgets/slot.rs` (`TabIndex`, `FocusRing`), `client/src/ui/hud/inventory/setup.rs` (`ArrowNav`, `TabGroup::modal()` on `InventoryDialog`), `client/src/ui/hud/inventory/display.rs`, `client/src/ui/hud/mod.rs`, `client/src/e2e/inventory.rs`, `README.md`

**Interfaces:**
- Consumes: `FocusRing`, `ArrowNav`, `InputFocus`, `InputFocusVisible`, `HOTBAR_KEYS`.
- Produces: `slot()` adds `TabIndex(0)` and `FocusRing`; `pub fn sync_inventory_focus(...)` — on opening, `InputFocus` = the selected hotbar cell and `InputFocusVisible(false)`; while closed, every `InventoryCell` and the result cell have `TabIndex(-1)` (`0` while open); on closing, the focus is cleared if it is inside `InventoryDialog`. Keys on the focused cell, read in `click_inventory_cells_system` while open: Enter = left click (`TakeCraftResult` on the result), Shift+Enter = `QuickMoveSlot`, Space = right click, a digit = `SwapWithHotbar` when no cell is hovered. `FloatingStack` sits centred on the focused cell while `InputFocusVisible` is true, on the cursor otherwise.

- [ ] **Step 1: Write the failing tests**

Unit, `display.rs`: `enter_space_and_shift_enter_act_on_the_focused_cell` — focus cell 12, Enter → `LeftClickSlot(12)`; Space → `RightClickSlot(12)`; Shift+Enter → `QuickMoveSlot(12)`; `Digit2` with nothing hovered → `SwapWithHotbar { slot: 12, hotbar: 1 }`; with cell 15 hovered → `{ slot: 15, .. }` (the cursor wins).

E2e, `e2e/inventory.rs`:
```rust
fn the_keyboard_alone_moves_stacks_in_the_inventory()
    // seeded (10, Dirt, 5)
    press(KeyE) → InputFocus is cell 0, no ring (Outline.color NONE on every cell);
    ArrowUp ×3, ArrowRight → focus cell 10, its Outline.color == ACCENT;
    Enter → wait_until floating Dirt 5; FloatingStack centre == center_of(cell 10) ± 1;
    ArrowDown ×3 → cell 1; Enter → wait_until slot 1 == Dirt 5;
    Shift+Enter → wait_until slot 9 == Dirt 5;
    Tab ×50 → InputFocus still inside InventoryDialog (modal);
    press(KeyE) → InputFocus is None; press(KeyE) → the inventory opens again.
fn tab_in_gameplay_leaves_the_hotbar_unfocused()
    press(Tab) → InputFocus is not a cell, no cell Outline is ACCENT, UIMode::Closed;
    press(KeyE) → InventoryRoot visible.
```

- [ ] **Step 2: Run them to verify they fail** — `timeout 300 cargo test -q -p client -- hud::inventory e2e::inventory` → the focus is never on a cell.

- [ ] **Step 3: Implement** as in Interfaces; `sync_inventory_focus` runs in the HUD chain after `toggle_inventory_system` and `close_inventory_system`. README Inventory rows: "Move between slots — Arrows", "Take / place — Enter", "Take half / place one — Space", "Move to the backpack / hotbar — Shift+Enter".

- [ ] **Step 4: Run the tests and the suite** → PASS; full suite green; the goldens are unchanged (the ring is `Color::NONE`).

- [ ] **Step 5: Commit** — `git commit -m "feat(client): the inventory with the keyboard alone"`

---

### Task 7: The debug overlay, and game keys while typing

**Files:**
- Modify: `client/src/ui/hud/debug/setup.rs`, `client/src/ui/hud/debug/mod.rs`, `client/src/ui/hud/mod.rs`, `client/src/main.rs`, `client/src/player/interactions.rs`, `client/src/e2e/game.rs` (`the_hud_shows_and_f3_toggles_the_debug_overlay`, `typing_in_the_chat_keeps_the_game_keys_quiet`), `README.md` (F3 row: "Toggle debug overlay")

**Interfaces:**
- Produces: `HudRoot` spawned with `Visibility::Hidden` and `GlobalZIndex(-1)` (below the HUD panel `1`, the held stack `3`, the pause menu `5` and the chat), text in `fonts/FiraMono-Medium.ttf` at `FONT_S` on `PANEL`; `pub fn in_gameplay(ui_mode: Res<UIMode>) -> bool` (`== UIMode::Closed`) as the run condition of `toggle_hud_system`, `toggle_chunk_debug_mode_system`, `toggle_raycast_debug_mode_system`, `toggle_wireframe_system` (they run `.after(GameSet::UiMode)`); `pub fn debug_overlay_shown(overlay: Query<&Visibility, With<HudRoot>>) -> bool`; in `main.rs`, `add_debug_inspector` and its imports, and `pub mod inspector`, under `#[cfg(debug_assertions)]`, with `inspector_ui.run_if(debug_overlay_shown)`.

- [ ] **Step 1: Write the failing tests**

Unit, `debug/setup.rs`: `the_inspector_runs_only_while_the_debug_overlay_is_shown` — no `HudRoot` → false; hidden → false; visible → true.

E2e, `e2e/game.rs`:
- `the_hud_shows_and_f3_toggles_the_debug_overlay` — the overlay starts hidden; F3 shows it, F3 hides it (the journey is kept, its first assertion is now fixed). Then, overlay shown: Escape → `ComputedStackIndex` of `HudRoot` < that of `PauseMenu`; Escape; E → < `InventoryDialog`; E; T → < `ChatRoot`; Escape.
- `typing_in_the_chat_keeps_the_game_keys_quiet` gains, right after its `type_text("e")`: `type_text("3")`, `press(F3)`, `press(F6)`, `scroll(-1.)` → `Hotbar::selected == 0`, `HudRoot` hidden, `WireframeConfig::global == false`, the field holds `"e3"`. And at its end (inventory open): `press(KeyE)` closes it; Escape opens the pause menu; `press(F3)`, `press(F6)` → still hidden and `false`; Escape; `press(F3)` → the overlay shows (gameplay).

- `the_click_that_closes_a_ui_does_not_reach_the_world` — in a seeded world, aim at a block (as the existing interaction tests do); `press(KeyT)`, then press the right mouse button over the hotbar and hold it 6 frames before releasing (a local helper writing the press, frames, then the release) → the chat closes and the block count in front of the player is unchanged (no block placed); same with Escape → click "Resume" held 6 frames → nothing placed. Implement it in the player interaction systems (`client/src/player/interactions.rs`): a mouse button pressed while `UIMode != Closed` is ignored until it is released (`just_pressed` in gameplay starts a new action).

- [ ] **Step 2: Run them to verify they fail** — `timeout 300 cargo test -q -p client -- hud::debug e2e::game` → the overlay starts shown; F3 toggles it while typing; the held click places a block.

- [ ] **Step 3: Implement** as in Interfaces. Check the release build: `timeout 1800 cargo clippy --release -p client -- -D warnings` is clean (no unused egui import).

- [ ] **Step 4: Run the tests and the suite** → PASS, full suite green.

- [ ] **Step 5: Commit** — `git commit -m "fix(client): the debug overlay starts hidden, under the dialogs, and its keys wait while typing"`

---

### Task 8: Review snapshots at three resolutions, and docs

**Files:**
- Modify: `client/src/e2e/review.rs`, `docs/ARCHITECTURE.md` ("UI widgets", "Known limits" untouched), plus whatever the review finds in the files of Tasks 3–7

- [ ] **Step 1: Extend the review journey** — in the game part of `every_screen_at_three_resolutions`, after `press(KeyE)`: `hover(&L::cell(10))` then `snapshot_in_game("inventory")` (tooltip and hover); `click(&L::cell(11))`, `hover(&L::cell(20))`, `snapshot_in_game("inventory_held")`, `click(&L::cell(11))`; and `press(Digit3)` before `snapshot_in_game("hud")` (item name). Seed `(grid_slot(2, 2), Cobblestone, 1)` too, for the 3×3 height.

- [ ] **Step 2: Run and look**

Run: `UI_SNAPSHOTS=1 timeout 600 cargo test -q -p client review -- --ignored`
Read every `target/ui-snapshots/every_screen_at_three_resolutions/{hud,inventory,inventory_held,chat,pause}_<w>x<h>.png` at 800×600, 1280×720 and 1920×1080. Check: the hotbar is at the same place in `hud` and `inventory`; the panel fits 800×600; icons crisp and evenly sized (48 px at 1920×1080); counts bottom-right with a shadow; slot 1 (or the selected one) with the accent border; the tooltip inside the window; the held stack centred on the cursor; no debug overlay; the reticle outlined; the item name above the hotbar in `hud`. Fix any defect in its task's file with a test first, as in the earlier tasks.

- [ ] **Step 3: Docs** — ARCHITECTURE "UI widgets": `slot.rs` (an item slot: icon at twice its texture, count, number, selection) and `tooltip.rs` (one tooltip by the cursor, for any hovered entity with a `Tooltip`); one sentence: the inventory is one panel whose bottom row is the hotbar, and every change to it is an `InventoryAction` predicted by the client and applied by the server.

- [ ] **Step 4: Full suite** — `timeout 900 cargo test --workspace --no-fail-fast` green; fmt and clippy clean; `git status` shows no `.actual.png` / `.diff.png`.

- [ ] **Step 5: Commit** — `git commit -m "test(client): review snapshots of the hotbar and the inventory, and docs of the slot and tooltip widgets"`
