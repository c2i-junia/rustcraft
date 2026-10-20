# Item Browser Lot 1 (Catalog, read-only) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Show every item of the game in a panel titled `Items` beside the open inventory: a search field, category chips, a scrolling grid of slots with tooltips, an empty state and a footer; docked right with 8 to 4 columns, or a drawer below 4; driven by the keyboard as well as the mouse, and never acting on a game key while the player types in it. Nothing comes out of the catalog yet.

**Architecture:** `shared` gains `ItemId::category()` and `ItemCategory`. The client gains `ui/hud/item_browser/`, a sibling of `inventory/`: pure functions (`catalog.rs`: order, matching, texts; `layout.rs`: columns, widths and where the inventory panel goes) and the panel (`setup.rs`, `display.rs`) built from the existing widgets (`slot`, `text_field`, `tooltip`, `list::spawn_scroll_list`, `focus::ArrowNav`, `hints::key_cap`, the `ui_widgets` button). `HudBottom` becomes a node over the whole window holding the inventory panel (still at the bottom, centred) and the browser (absolute, right), and takes the modal `TabGroup` of `InventoryDialog`, so that Tab walks both. The browser's state (`ItemBrowserState`: query, category, drawer) is a resource that outlives the game screen; the grid is filtered when it changes, not every frame. No `InventoryAction`, no protocol change.

**Tech Stack:** Rust nightly, Bevy `=0.19.1` (`bevy::ui_widgets::{Button, ScrollArea, ScrollIntoView, Activate}`, `bevy::input_focus`, `bevy::picking::hover::Hovered`, `UiTransform`), `accesskit` 0.24, lavapipe for goldens and e2e.

**Spec:** `docs/superpowers/specs/2026-10-20-item-browser-design.md`, lot 1 ("Catalog, read-only"): "What the player sees" without the Craftable chip, the `OK` square, the bookmarks and the recipe view; "Layout by window size"; the rows `/`, `Ctrl+F`, Arrows/Enter, `B`, `Esc` and "Enter in the search" of "Interactions" (a click on a catalog item only focuses it); "Shared" (`category()` only); "Client". The look and the sizes: the approved mockup (`INV_W`, `bw(cols)`, 8..4 columns, the drawer). API reference: `docs/superpowers/specs/2026-10-18-bevy-ui-0.19-notes.md` ("notes §n").

## Global Constraints

The "Global Constraints" of `docs/superpowers/plans/2026-10-18-ui-redesign-lot-5-hotbar-inventory.md` apply verbatim, with these readings for this lot:

- English for code, comments, docs and commit messages, and for the game texts; Conventional Commits (`feat(shared): …`, `feat(client): …`, `test(client): …`, `docs: …`).
- Tests are named as sentences; doc comments in full sentences; comment density like the neighbouring code.
- Every test command runs under `timeout`. `cargo fmt --all -- --check` and `timeout 900 cargo clippy --workspace --all-targets -- -D warnings` are clean at every commit; the full suite is green at the end of every task.
- Cranelift runs no destructor while a panic unwinds: nothing relies on `Drop` in a failing path.
- Every e2e test is `#[test]` + `#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]`, drives the client only through `click`/`hover`/`press`/`type_text`/`resize`, and goes in `client/src/e2e/item_browser.rs`. Each `enter_world` starts an integrated server: **three journeys** (search and chips, keyboard, layout and drawer), each chaining its checks.
- No `bevy::prelude::Button`, `Interaction` or `FocusPolicy`. Catalog slots are `widgets::slot` (hit test `Hovered`); the chips and the drawer handle are `bevy::ui_widgets::Button` (`UiButton`) observed with `On<Activate>`. Nodes under the cursor that are not targets carry `Pickable::IGNORE`.
- The role before `AccessibleLabel` in every bundle (notes §5).
- **Inventory authority.** The browser never writes `Res<Inventory>` and sends no `InventoryAction` in this lot.
- `InventoryRoot`'s `Visibility` stays the open/closed state of the inventory; the browser shows exactly while it is `Visible`.
- Bevy's Tab navigation ignores visibility: every focusable of the browser has `TabIndex(-1)` while it is not shown (inventory closed, drawer closed, slot filtered out), as `sync_inventory_focus` does for the inventory.
- Widths are logical px (`Val::Px`, multiplied by `UiScale`); a window size is divided by `UiScale`.
- Goldens: the four open-inventory goldens (`inventory_2x2_grid`, `inventory_leftovers_beyond_the_grid`, `crafting_table_3x3_grid`, `chest`) change **only in Task 4**, deliberately, because the panel slides left for the browser at 1024 px; `hotbar.png` and every menu golden must not change. Each new or changed PNG is looked at before it is accepted.

## Review Focus

1. Typing in the search never acts on a game key: `e` does not close the inventory, a digit over a slot swaps nothing, `b` does not close the drawer, Escape clears the field before anything else. (Task 6: `game_keys_wait_while_the_search_has_the_focus`, e2e `the_keyboard_reaches_the_item_browser_and_typing_in_it_moves_nothing`)
2. The closed hotbar stays centred; the open panel slides left only as far as the browser needs, never closer than `SPACE_L` to the left edge, and never under the browser. (Task 3 unit tests, Task 4 e2e)
3. Tab and the arrows reach the browser but a hidden part of it (closed drawer, filtered slot, closed inventory) is never focused. (Task 6)
4. Escape in the drawer closes the drawer, not the inventory; the pause menu leaves it alone. (Task 5)
5. A click on the inventory while the drawer is open closes the drawer; a click inside it does not. (Task 5)

## Files

- `shared/src/world/items.rs` — `ItemCategory`, `ItemId::category()` (Task 1).
- `client/src/ui/hud/item_browser/{mod,catalog,layout,setup,display,goldens}.rs` (new) — the browser (Tasks 2–6).
- `client/src/ui/hud/mod.rs` — the module, its setup and systems.
- `client/src/ui/hud/inventory/{setup,display,goldens}.rs` — `HudBottom` over the window, the modal group moved to it, the slide of the panel, the focus kept in the browser, the game keys while typing.
- `client/src/ui/menus/pause.rs` — Escape left to the open drawer.
- `client/src/ui/golden.rs` — `UiGolden::with_size`, `UiGolden::frames`.
- `client/src/e2e/{item_browser (new),inventory,mod}.rs`, `client/tests/golden/*.png`, `README.md`, `docs/ARCHITECTURE.md`, the spec's "Decisions".

---

### Task 1: Item categories in shared

**Files:**
- Modify: `shared/src/world/items.rs`

**Interfaces:**
- Produces:
  ```rust
  #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
  pub enum ItemCategory { Blocks, Wood, Nature, Ores, Materials, Stations, Tools }
  impl ItemCategory {
      pub const ALL: [ItemCategory; 7];   // in this order: the order of the chips and of the catalog
      pub fn name(self) -> &'static str;  // "Blocks", "Wood", …
  }
  impl ItemId { pub fn category(self) -> ItemCategory; } // an exhaustive match, no wildcard
  ```
  The categories of the mockup: Blocks = Dirt, Grass, Stone, Cobblestone, Gravel, Sand, Glass, Ice, Snow, Bedrock; Wood = Oak/Spruce Log and Planks; Nature = Oak Leaves, Cactus, Dandelion, Poppy, Tall Grass; Ores = Coal/Iron/Gold Ore; Materials = Coal, Stick, Snowball; Stations = Crafting Table, Furnace, Chest; Tools = the five wooden tools.

- [ ] **Step 1: Write the failing tests** (tests module of `items.rs`)
  - `every_item_has_a_category` — every `ItemId::ALL` item's category is in `ItemCategory::ALL`, and every category holds at least one item.
  - `items_fall_in_the_categories_of_the_browser` — `Dirt → Blocks`, `OakPlanks → Wood`, `Poppy → Nature`, `IronOre → Ores`, `Stick → Materials`, `Chest → Stations`, `WoodenSword → Tools`; the items of `Tools` are exactly those with a `tool()`.
  - `categories_have_names` — `ItemCategory::ALL.map(name) == ["Blocks", "Wood", "Nature", "Ores", "Materials", "Stations", "Tools"]`.
- [ ] **Step 2: Run them to verify they fail** — `timeout 300 cargo test -q -p shared items` → compile error.
- [ ] **Step 3: Implement** as in Interfaces.
- [ ] **Step 4: Run the tests and the suite** → PASS.
- [ ] **Step 5: Commit** — `git commit -m "feat(shared): every item has a category, for the item browser"`

---

### Task 2: The catalog — order, search and texts

**Files:**
- Create: `client/src/ui/hud/item_browser/mod.rs` (module docs, `pub mod catalog;`), `client/src/ui/hud/item_browser/catalog.rs`
- Modify: `client/src/ui/hud/mod.rs` (`pub mod item_browser;`)

**Interfaces:**
- Consumes: `ItemId::category`, `ItemCategory`, `crate::ui::hud::item_name`.
- Produces, in `catalog.rs`:
  ```rust
  pub fn catalog() -> Vec<ItemId>;                       // by category (ItemCategory::ALL order), then ItemId::ALL order
  pub fn matches(query: &str, item: ItemId) -> bool;     // the search rules of the spec
  pub fn shown_in(query: &str, category: Option<ItemCategory>, item: ItemId) -> bool; // None = All
  pub fn count_label(shown: usize, total: usize) -> String;            // "12 / 33"
  pub fn no_match_message(query: &str, category: Option<ItemCategory>) -> String; // `No item matches "zzz" in Wood.`
  pub const NO_MATCH_HINT: &str = "Esc clears the search.";
  pub fn category_detail(item: ItemId) -> String;         // "Wood", "Tools · Tool"
  ```
  Matching: the query is lowercased and split on whitespace; every word must match; a word starting with `#` matches when the category name, lowercased, starts with the rest (`#` alone matches all); any other word matches when the lowercased display name contains it (which covers "a word of the name starts with it").
  These are `#[allow(dead_code)]` with "first used by the item browser panel (Task 4)" until Task 4.

- [ ] **Step 1: Write the failing tests** (tests module of `catalog.rs`)
  - `the_catalog_lists_every_item_once_by_category` — 33 items, no duplicate, categories non-decreasing in `ItemCategory::ALL` order, `Dirt` first, `WoodenSword` last; within Wood: Oak Log, Oak Planks, Spruce Log, Spruce Planks (`ItemId::ALL` order).
  - `each_word_of_the_search_must_match_the_name` — `"sp pl"` → only Spruce Planks; `"oak"` → Oak Log, Oak Planks, Oak Leaves; `"PLANKS"` → the two planks; `"ore"` → the three ores; `""` and `"  "` → all 33; `"zzz"` → none.
  - `a_hash_word_matches_the_category` — `"#tools"` → the 5 tools; `"#wo"` → the 4 wood items; `"#"` → all; `"#ores coal"` → Coal Ore only; `"#tools planks"` → none.
  - `a_category_keeps_only_its_items` — `shown_in("", Some(Wood), OakLog)`, `!shown_in("", Some(Wood), Dirt)`, `shown_in("oak", None, OakLeaves)`, `!shown_in("oak", Some(Wood), OakLeaves)`.
  - `the_texts_of_the_panel` — `count_label(12, 33) == "12 / 33"`; `no_match_message("zzz", None) == "No item matches \"zzz\"."`; `no_match_message("zzz", Some(Wood)) == "No item matches \"zzz\" in Wood."`; `category_detail(OakLog) == "Wood"`; `category_detail(WoodenPickaxe) == "Tools · Tool"`.
- [ ] **Step 2: Run them to verify they fail** — `timeout 300 cargo test -q -p client item_browser::catalog` → compile error.
- [ ] **Step 3: Implement** as in Interfaces.
- [ ] **Step 4: Run the tests and the suite** → PASS.
- [ ] **Step 5: Commit** — `git commit -m "feat(client): the item catalog, its order and its search"`

---

### Task 3: The layout by window size

**Files:**
- Create: `client/src/ui/hud/item_browser/layout.rs`

**Interfaces:**
- Consumes: `SLOT_SIZE`, `SPACE_*`, `BEVEL_WIDTH`, `list::{RING_ROOM, SCROLLBAR_WIDTH}`.
- Produces:
  ```rust
  pub const MAX_COLUMNS: u32 = 8;
  pub const MIN_COLUMNS: u32 = 4;
  pub const MAX_DRAWER_COLUMNS: u32 = 6;
  /// Around the grid: the panel's padding and bevel, the room of the focus ring, the scrollbar and its gap: 42.
  pub const BROWSER_CHROME: f32;
  pub fn browser_width(columns: u32) -> f32;              // columns × 50 + (columns − 1) × 4 + 42
  #[derive(Resource, Clone, Copy, PartialEq, Debug, Default)]
  pub struct BrowserLayout { pub columns: u32, pub compact: bool, pub width: f32, pub inventory_left: f32 }
  pub fn browser_layout(window: Vec2, inventory_width: f32) -> BrowserLayout;
  ```
  As the mockup: `free = W − inventory_width − 3 × SPACE_L`; `columns = min(8, floor((free − 42 + 4) / 54))`. With 4 or more: docked, `width = browser_width(columns)`, `inventory_left = max(SPACE_L, min((W − inventory_width) / 2, W − width − 2 × SPACE_L − inventory_width))`. Else compact: `columns = clamp(floor((W − 2 × SPACE_L − 42 + 4) / 54), 1, 6)`, the inventory centred. Allowed `dead_code` until Task 4.

- [ ] **Step 1: Write the failing tests** (the inventory panel is 502 px wide: 9 slots, 8 gaps, padding and bevel)
  - `a_wide_window_docks_eight_columns_and_slides_the_inventory_left` — 1280×720: 8 columns, not compact, width 470, `inventory_left` 276 (so 16 px between the panel and the browser, whose left is 794).
  - `a_very_wide_window_keeps_the_inventory_centred` — 1920 wide: 8 columns, `inventory_left == (1920 − 502) / 2`.
  - `a_small_window_docks_four_columns` — 854×480: 4 columns, width 254, `inventory_left` 66.
  - `below_four_columns_the_browser_is_a_drawer` — 800×600 and 640×480: compact, 6 columns, width 362, inventory centred.
  - `the_docked_browser_never_covers_the_inventory` — for every width 400..=3000: compact, or 4..=8 columns with `inventory_left ≥ SPACE_L` and `inventory_left + 502 + SPACE_L ≤ W − SPACE_L − width`; compact → `width ≤ W` and columns ≤ 6.
- [ ] **Step 2: Run them to verify they fail** — `timeout 300 cargo test -q -p client item_browser::layout` → compile error.
- [ ] **Step 3: Implement** as in Interfaces.
- [ ] **Step 4: Run the tests and the suite** → PASS.
- [ ] **Step 5: Commit** — `git commit -m "feat(client): the columns of the item browser follow the width of the window"`

---

### Task 4: The panel docked beside the inventory

**Files:**
- Create: `client/src/ui/hud/item_browser/{setup,display,goldens}.rs`, `client/src/e2e/item_browser.rs`
- Modify: `client/src/ui/hud/item_browser/mod.rs`, `client/src/ui/hud/mod.rs`, `client/src/ui/hud/inventory/setup.rs` (`HudBottom` over the window), `client/src/ui/hud/inventory/display.rs` (the focus may sit in the browser), `client/src/ui/hud/inventory/goldens.rs`, `client/src/ui/golden.rs`, `client/src/e2e/{inventory,mod}.rs`, `client/tests/golden/*.png`

**Interfaces:**
- Produces, in `item_browser/mod.rs`:
  ```rust
  #[derive(Resource, Default, Clone, PartialEq, Debug)]
  pub struct ItemBrowserState { pub query: String, pub category: Option<ItemCategory>, pub drawer_open: bool }
  #[derive(Component)] pub struct ItemBrowser;          // the panel
  #[derive(Component)] pub struct ItemSearchField;      // the input of the search field
  #[derive(Component)] pub struct ItemCount;            // "12 / 33"
  #[derive(Component)] pub struct CategoryChip(pub Option<ItemCategory>);
  #[derive(Component)] pub struct CatalogGrid;
  #[derive(Component)] pub struct CatalogSlot(pub ItemId);
  #[derive(Component)] pub struct NoMatch;              // the empty state
  pub const ITEM_BROWSER_TITLE: &str = "Items";
  pub const SEARCH_LABEL: &str = "Search items";
  pub fn item_browser_plugin(app: &mut App);
  ```
- Produces, in `setup.rs`: `pub fn setup_item_browser(commands, materials, asset_server, state: Res<ItemBrowserState>, bottom: Single<Entity, With<HudBottom>>)`, run after `setup_inventory`. The tree, a child of `HudBottom`:
  `ItemBrowser` (`AccessibilityNode(Role::Group)`, label `Items`, absolute, `top`/`bottom`/`right` `SPACE_L`, column, `row_gap SPACE_S`, padding `SPACE_S`, bevel, `PANEL`, raised bevel, `Hovered`, `ArrowNav`, `Visibility::Hidden`) → [header row (`justify_content: SpaceBetween`, `align_items: Baseline`): title `FONT_L` `TEXT`, `ItemCount` `FONT_S` `MUTED`; the search field (`spawn_text_field`, label and placeholder `Search items`, `FONT_S`, 32 characters) with a `/` key cap at its right end; the chips row (`Role::Toolbar` "Categories", `ScrollArea`, `Overflow::scroll_x()`, padding `RING_ROOM`, `column_gap SPACE_XS`): `All` then one chip per `ItemCategory::ALL`; `spawn_scroll_list(WheelStep::Px(SLOT_SIZE + SPACE_XS))`, its well unpainted and without bevel, holding `CatalogGrid` (`Role::Grid` "Items", `Display::Grid`, columns `RepeatedGridTrack::px(n, SLOT_SIZE)`, gap `SPACE_XS`, vertical padding `RING_ROOM`) with one `(CatalogSlot(item), Tooltip, slot(item_name(item), None, icon, font))` per `catalog()` item, and the `NoMatch` column (two texts, `MUTED`, `Display::None`); the footer (top border `BEVEL_WIDTH` `BEVEL_LO`, `padding_top SPACE_S`, wrapping row): key cap `/` + `search`, key cap `↑↓←→` + `browse`]. A chip is `(UiButton, AccessibleLabel, CategoryChip, TabIndex(0), FocusRing, Hovered, Node { padding: (SPACE_S, 2), border BEVEL_WIDTH }, raised bevel, RAISE, text FONT_S MUTED)`.
  `HudBottom` becomes absolute over the whole window (`left/right/top/bottom: 0`, column, `justify_content: FlexEnd`, `align_items: Center`, `padding.bottom = HOTBAR_BOTTOM + SPACE_S`): the panel stays exactly where it was.
- Produces, in `display.rs`:
  - `read_search` — `ItemSearchField`'s `Changed<EditableText>` → `state.query`.
  - `choose_category` (observer `On<Activate>` on each chip) → `state.category = chip.0`.
  - `layout_item_browser` — `BrowserLayout` from the logical size of `HudBottom` (`ComputedNode`, which is the window) and of `InventoryDialog`; `set_if_neq` the resource.
  - `filter_catalog` (run when `ItemBrowserState` changes or a `CatalogSlot` is added) — each slot `Display::Grid`/`None` by `shown_in`; `ItemCount` text; `NoMatch` shown, with `no_match_message`, when none is.
  - `show_item_browser` (every frame, `set_if_neq`) — the panel's `Visibility` (inventory open and docked, or drawer open), width and right (`SPACE_L` docked, `0` compact), the grid's columns, the `InventoryDialog`'s `left` (`inventory_left − (W − inventory_width) / 2` while open and docked, `0` otherwise), and the `TabIndex` of every focusable of the panel (`0` only while shown, and for a slot only while it matches).
  - `show_catalog_icons` (`Added<CatalogSlot>`) — the slot's icon visible on the item's sprite.
  - `style_chips` — the chosen chip `ACCENT` with `PANEL` text, `Checked` (a11y toggled) and the accent bevel (`#F6CF86` / `#8A6524`, as the mockup); the others `RAISE` (`RAISE_HOVER` hovered) with `MUTED` (`TEXT` hovered) text and the raised bevel.
  - Catalog tooltips at setup: `Tooltip { title: item_name, detail: category_detail, hint: "" }`.
  - `scroll_focused_into_view` (observer `On<FocusGained>`) — a focused chip or catalog slot triggers `ScrollIntoView`.
- `sync_inventory_focus` treats a focus inside `ItemBrowser` as inside the dialog (kept while open, cleared on close).
- `UiGolden::with_size(width, height)` (and `new()` = `with_size(WIDTH, HEIGHT)`), `UiGolden::frames(n)`.

- [ ] **Step 1: Write the failing tests**

Unit, `item_browser/display.rs` (a bare `World` with the resources the systems read):
- `the_search_and_the_category_filter_the_slots_and_the_count` — spawn the catalog's slots, `ItemCount` and `NoMatch`; state query `"oak"` → three slots `Display::Grid`, count `"3 / 33"`, `NoMatch` hidden; category Wood → 2; query `"zzz"` → none, `NoMatch` shown with `No item matches "zzz" in Wood.`.
- `the_browser_shows_only_while_the_inventory_is_open` — `show_item_browser` with `InventoryRoot` hidden → panel `Hidden`, every `TabIndex` −1, `InventoryDialog.left == 0`; visible, docked 1280 layout → `Inherited`, `TabIndex(0)` on the field, the chips and the matching slots only, `left == px(276 − 389)`.

Goldens, `item_browser/goldens.rs` (the open inventory, a few stacks, every item in the catalog): `the_item_browser_docks_eight_columns_at_1280x720` → `item_browser_1280x720`; `the_item_browser_docks_four_columns_at_854x480` → `item_browser_854x480`. The inventory goldens (`inventory/goldens.rs`) run the browser too.

E2e, `e2e/item_browser.rs`:
```rust
fn the_item_browser_lists_searches_and_filters_the_items()
    // default 1280×720, seeded (0, Dirt, 5)
    press(E) → L::text("Items") visible, "33 / 33", 33 visible CatalogSlot, 8 distinct x among the first 8;
    the browser rect inside the window, right edge 16 px from it, and not intersecting InventoryDialog;
    click the search field; type_text("sp pl") → "1 / 33", only the Spruce Planks slot visible;
    clear (Escape), type_text("#tools") → "5 / 33";
    type_text("zzz") → L::text("No item matches \"#toolszzz\".") and "Esc clears the search." visible;
    Escape → "33 / 33";
    click the chip "Wood" → "4 / 33", the chip has Checked; click "All" → "33 / 33";
    hover the Oak Log slot → "Oak Log" and "Wood" within TooltipRoot; the Wooden Pickaxe → "Tools · Tool";
    click the Oak Log slot → it has the focus, the inventory unchanged (slot 0 still Dirt 5, nothing held);
    press(E) closes; the browser is hidden; press(E) → the query of the field kept ("" here: set "oak" before closing, then "3 / 33" after reopening).
```
`the_open_inventory_leaves_the_hotbar_alone_and_fits_the_window` (`e2e/inventory.rs`) becomes `the_open_inventory_keeps_its_hotbar_whole_and_fits_beside_the_item_browser`: at 800×600 (drawer) the hotbar does not move; at 1280×720 and 1366×768 it moves left with the panel, by the same amount for every cell, keeping its size and its height, and its pixels are the same at its new place; the panel and the visible browser lie inside the window and do not intersect.

- [ ] **Step 2: Run them to verify they fail** — `timeout 600 cargo test -q -p client -- item_browser hud::inventory e2e::inventory` → compile errors, then the e2e finds no `Items`.
- [ ] **Step 3: Implement** as in Interfaces. `item_browser_plugin` inits `ItemBrowserState` and `BrowserLayout`, adds `setup_item_browser` at `OnEnter(Game)` after `setup_inventory`, and the systems in `Update` after `style_inventory_panel`, in the game state. Remove the Task 2–3 `dead_code` allowances.
- [ ] **Step 4: Goldens** — run the goldens; read every new PNG and every `.actual.png` of the four open-inventory goldens: the inventory slid to 20 px from the left at 1024×768, the browser on the right with 8 columns, its search, chips and footer; `hotbar.png` unchanged. Then `UPDATE_GOLDEN=1 timeout 600 cargo test -q -p client -- golden item_browser::goldens`, rerun → PASS, no `.actual.png` left.
- [ ] **Step 5: Run the suite** → green.
- [ ] **Step 6: Commit** — `git commit -m "feat(client): the item browser lists, searches and filters every item beside the inventory"` (the message says why the four inventory goldens changed).

---

### Task 5: The drawer below four columns

**Files:**
- Modify: `client/src/ui/hud/item_browser/{mod,setup,display,goldens}.rs`, `client/src/ui/menus/pause.rs`, `client/src/e2e/item_browser.rs`, `client/tests/golden/*.png`

**Interfaces:**
- Produces: `#[derive(Component)] pub struct DrawerHandle;` — a `UiButton` labelled `Items` (absolute, `right: 0`, `top: 50%` translated −50 %, 34 px wide, `PANEL`, raised bevel without the right edge, column: key cap `B`, then `Items` turned a quarter, `UiTransform`), shown while the inventory is open, the layout compact and the drawer closed; `On<Activate>` opens the drawer.
  `pub fn drawer_takes_escape(state, layout, inventory_root) -> bool` (the drawer open in compact with the inventory open), a run condition of `toggle_pause_menu`, which becomes `pub(crate)`.
  `drawer_keys` (Update, `.after(toggle_pause_menu)`, not while `UIMode::Typing`): `B` toggles the drawer in compact; Escape closes it; a mouse press with neither the panel nor the handle hovered closes it. Closing the drawer clears a focus inside the panel. `ItemBrowserState::drawer_open` is reset when the inventory closes.
  In compact, the panel is at `right: 0`, `width = browser_width(columns)`.

- [ ] **Step 1: Write the failing tests**

Unit, `display.rs`: `escape_belongs_to_the_open_drawer` — `drawer_takes_escape` false docked, false compact closed, true compact open, false with the inventory hidden.

Goldens: `the_drawer_is_a_handle_at_640x480` → `item_browser_drawer_closed_640x480`; `the_open_drawer_covers_the_right_of_the_inventory_at_640x480` → `item_browser_drawer_open_640x480`.

E2e:
```rust
fn the_item_browser_becomes_a_drawer_in_a_small_window()
    // starts at 1280×720
    press(E); resize(854, 480) → 4 columns, docked; resize(640, 480) → the panel hidden, the handle visible, at the right edge, mid-height;
    press(B) → the panel shows, flush right, 6 columns; the handle hidden;
    Escape → closed, the inventory still open (InventoryRoot visible, no PauseMenu);
    click the handle → open; click an inventory cell (cell 0) → closed;
    press(B); click inside the panel (the title) → still open;
    type_on(Slash, '/') → the search has the focus (drawer open); Escape (clears / leaves) ×2, then Escape → closed;
    resize(1280, 720) → docked, the handle hidden.
```
- [ ] **Step 2: Run them to verify they fail** — `timeout 600 cargo test -q -p client -- item_browser` → the handle is never found.
- [ ] **Step 3: Implement** as in Interfaces.
- [ ] **Step 4: Goldens** — read both new PNGs (the handle with its `B` cap and the turned `Items`; the open drawer flush right over the inventory) before committing them.
- [ ] **Step 5: Run the suite** → green.
- [ ] **Step 6: Commit** — `git commit -m "feat(client): below four columns the item browser is a drawer, opened by B or its handle"`

---

### Task 6: The keyboard, and game keys while typing

**Files:**
- Modify: `client/src/ui/hud/item_browser/{display,setup}.rs`, `client/src/ui/hud/inventory/{setup,display}.rs` (modal group on `HudBottom`; `E` and the keys of the cells wait while typing), `client/src/e2e/{item_browser,inventory}.rs`

**Interfaces:**
- `TabGroup::modal()` moves from `InventoryDialog` to `HudBottom`: Tab walks the inventory then the browser and the handle, and stays inside.
- `focus_search` (Update, inventory open, not typing): `Ctrl+F` pressed, or the character `/` released after a press made with the inventory open (as `open_chat_on_slash`, so that the `/` is not typed): opens the drawer in compact, focuses `ItemSearchField`, shows the ring.
- The search field: `On<Submit>` (Enter) and ArrowDown focus the first shown catalog slot with its ring; `On<Cancel>` (Escape) clears a non-empty query, else lets go of the focus (the inventory takes it back).
- `toggle_inventory_system` does nothing while `UIMode::Typing`; `click_inventory_cells_system` ignores Enter, Space and the digits while `UIMode::Typing`.

- [ ] **Step 1: Write the failing tests**

Unit, `inventory/display.rs`: `game_keys_wait_while_the_search_has_the_focus` — `UIMode::Typing`, inventory open: `E` → still visible; cell 12 hovered and `Digit3`, Enter, Space → no action; a left click on cell 12 still sends `LeftClickSlot(12)`.

E2e:
```rust
fn the_keyboard_reaches_the_item_browser_and_typing_in_it_moves_nothing()
    // seeded (0, Dirt, 5), (12, Sand, 3)
    press(E); type_on(Slash, '/') → the focus on the field, which holds "" (no slash typed);
    hover cell 12; type_text("e3b ") → the field holds "e3b ", the inventory open, slot 12 still Sand 3, slot 2 empty;
    Escape → field empty and still focused; Escape → focus back on a cell, the inventory open;
    ctrl(KeyF) → the field focused; type_text("planks"); Enter → focus on the Oak Planks slot, its ring ACCENT, its tooltip shown;
    ArrowRight → Spruce Planks; ArrowLeft → Oak Planks;
    50 × Tab → the focus always inside HudBottom; at least once on a catalog slot or a chip and once on an inventory cell;
    press(E) → closed, focus None.
```
`the_keyboard_alone_moves_stacks_in_the_inventory` (`e2e/inventory.rs`): "The panel keeps Tab to itself" now checks the focus stays inside `HudBottom` (the inventory or the item browser).

- [ ] **Step 2: Run them to verify they fail** — `timeout 600 cargo test -q -p client -- hud::inventory item_browser e2e::inventory` → `e` closes the inventory, `/` focuses nothing.
- [ ] **Step 3: Implement** as in Interfaces.
- [ ] **Step 4: Run the suite** → green; goldens unchanged.
- [ ] **Step 5: Commit** — `git commit -m "feat(client): the item browser with the keyboard, and the game keys wait while the search has the focus"`

---

### Task 7: Docs and the final check

**Files:**
- Modify: `README.md` (Controls: an "Item browser" category: Search `/` or Ctrl+F, First result Enter or ↓, Clear the search Escape, Open / close the drawer B; Features: the item browser), `docs/ARCHITECTURE.md` (the HUD paragraph: `ui/hud/item_browser/`), the spec's "Decisions" (the deviations of this lot).

- [ ] **Step 1: Docs** as above.
- [ ] **Step 2: Full check** — `cargo fmt --all -- --check`; `timeout 900 cargo clippy --workspace --all-targets -- -D warnings`; `timeout 1500 cargo test --workspace --no-fail-fast` green; `git status` shows no `.actual.png` / `.diff.png`.
- [ ] **Step 3: Commit** — `git commit -m "docs: the item browser in the README and the architecture"`
