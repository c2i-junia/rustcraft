# Item Browser — design

The proposal the user accepted on 2026-10-20 ("c'est parfait, implémente ça"): an item browser
in the spirit of TooManyItems, JEI and REI, always open beside the inventory. In Creative it
hands out items; in Survival it is the recipe book and can fill the crafting grid. The mockup and
the proposal: https://claude.ai/artifact/CsaHciZNLAyS4gsVWGChRp. The default choices of the
proposal were all accepted; they are recorded under "Decisions".

## What the player sees

A panel titled `Items`, docked to the right edge of the window, from `SPACE_L` below its top to
`SPACE_L` above its bottom, shown whenever the inventory panel is open (inventory, crafting table
or chest). It uses the theme of the inventory: same panel bevel, same 50 px slots
(`widgets::slot`), same tooltips.

- **Header**: the title, and the number of items shown over the total (`12 / 33`).
- **Search field** (`widgets::text_field`), placeholder `Search items`, with a `/` key cap. It
  filters at every keystroke. Each word of the query must match: a word matches an item when a
  word of its name starts with it, or when the name contains it; a word starting with `#` matches
  the item's category (`#tools`, `#wood`, a prefix is enough). Case is ignored. The query is kept
  while the inventory is closed, for the session.
- **Category chips**, which wrap onto as many rows as they need (Decision 25): All, Blocks, Wood, Nature, Ores, Materials,
  Stations, Tools; in Survival a green `Craftable` chip keeps only the items the player can craft
  now (some recipe whose ingredients the inventory and the grid hold, whatever the grid size).
- **Bookmarks**: a `Bookmarks` label and a row of slots, shown only when there is one.
- **Grid** of every matching item, in category order then `ItemId::ALL` order, one item per slot
  (no count). It scrolls with the wheel and the trackpad (`widgets::scroll`), no pages. In
  Survival, a small `OK` square in the top-right corner of a slot marks an item craftable now.
  When nothing matches: `No item matches "<query>"` (and ` in <Category>`), `Esc clears the search.`
- **Footer**: in Creative a trash slot and `Drop a stack here, or on the list, to destroy it.`;
  in Survival the key caps `LMB recipes  RMB uses  A bookmark`.
- **Tooltip** on any item slot of the panel: the name, the category (and `Tool`), in Survival
  `You have the ingredients` when craftable now, in Creative on catalog slots
  `LMB 64  RMB 1  Shift to inventory` (1 for a tool), and `R Recipes  U Uses  A Bookmark`.

### Recipe view

Replaces the list in the same panel (title `Recipes` or `Uses`):

- a back button `<- Items` (or `<- <previous item>`), with an `Esc` cap;
- the item's icon, name and category (`· bookmarked` when it is);
- two chips `Recipes N` and `Uses M`;
- a scrolling column of recipe cards: a 3 × 3 grid of 40 px slots (empty cells dashed), `->`,
  the output slot with its count; under it, the station (`Inventory 2×2` or `Crafting Table 3×3`)
  or, in `DANGER`, why it cannot be made now (`Missing 3 Oak Planks`, `Needs a Crafting Table`),
  and the `+ Fill grid` button, disabled with that reason;
- ingredients the player lacks are tinted red;
- an item with no recipe: `<Item> is gathered, not crafted.` and, when it has uses,
  ` It goes into N recipes: see Uses.`; no use: `No recipe uses <Item> yet.`
- clicking an ingredient or output opens its recipes (right click: its uses), pushing the
  current view on a history that the back button and Esc pop.

### Layout by window size

The free width beside the inventory panel decides the columns: 8 at most, 4 at least. The
inventory panel stays centred when there is room, otherwise it slides left to leave room for the
browser, never closer than `SPACE_L` to the left edge. Below 4 columns the browser becomes a
drawer over the right edge (at most 6 columns), closed by default, with a handle (`B` cap,
`Items` written vertically) at mid-height; `B`, the handle, `/`, or opening a recipe open it; Esc
or a click outside closes it. Widths are logical px (after `UiScale`).

## Interactions

| Gesture | Creative | Survival |
| --- | --- | --- |
| Click on a catalog item (grid or bookmark) | A full stack on the cursor (64, 1 for a tool) | Opens its recipes |
| Right click | One item; each further right click on the same item adds one | Opens its uses |
| Shift+click | The full stack goes into the inventory | As a click |
| Click on the list (or the trash) holding a stack | Destroys the held stack; a toast says what | Nothing |
| `R` / `U` / `A` over any item slot (catalog, inventory, grid, result, chest, recipe) | Recipes / uses / bookmark toggle | same |
| `/`, `Ctrl+F` | Focus the search (leaving the recipe view, opening the drawer) | same |
| Arrows, Enter | Move through the grid and act as the click; Tab reaches the panel like the rest of the UI | same |
| `B` | Opens or closes the drawer (compact only) | same |
| `Esc` | Clears the search, then pops the recipe history, then closes the drawer, then closes the inventory (the existing behaviour) | same |
| Enter in the search | Acts as a click on the first result (Shift as Shift+click) | same |

While the search field has the focus, the game's keys (E, 1–9, R, U, A, B, movement) are not
acted on: typing never closes the inventory or swaps a slot.

## How it works

### Shared

- `ItemId::category(self) -> ItemCategory` (`Blocks`, `Wood`, `Nature`, `Ores`, `Materials`,
  `Stations`, `Tools`) with `ItemCategory::ALL` and `name()`; a test checks that every item of
  `ItemId::ALL` has one (exhaustive `match`, no wildcard).
- `crafting.rs` exposes the recipe table read-only: `pub struct RecipeView { pub index: u16,
  pub width: usize, pub height: usize, pub cells: &'static [Option<ItemId>], pub output:
  ItemStack }`, `pub fn recipes() -> impl Iterator<Item = RecipeView>`, `recipes_for(item)`,
  `uses_of(item)` and `RecipeView::fits(grid_size)`. The index is the position in `RECIPES`,
  stable for a protocol version.
- `fn craftable(inventory, recipe) -> bool` and `fn missing(inventory, recipe) -> Vec<(ItemId, u32)>`
  count the inventory slots and the crafting grid (not the chest, not the floating stack).
- Three new `InventoryAction` variants, appended last, `PROTOCOL_ID` bumped once (18 → 19), the
  pin test in `shared/src/messages/mod.rs` naming them:
  - `TakeFromCatalog { item: ItemId, count: u32, to_inventory: bool }`: Creative only. With
    `to_inventory`, the stack is added to the inventory (hotbar first, as a pick-up); else it goes
    to the cursor: refused if the cursor holds another item, added to it (up to the max stack)
    if it holds the same item. `count` is clamped to `1..=max_stack`. A tool comes new.
  - `DestroyFloating`: Creative only; the cursor stack disappears.
  - `FillCraftingGrid { recipe: u16, times: u32 }`: puts the grid back into the inventory, then
    moves the recipe's ingredients from the inventory into the grid, at its top-left corner,
    `times` times at most (clamped to what the inventory holds and to the max stack); refused
    when the recipe does not fit the current grid size or the inventory lacks one round of it.
    Allowed in both modes.
  `simulate_inventory_actions` refuses the Creative-only actions with
  `InventoryRefusal::NotInCreative` unless `player.game_mode.is_creative()`;
  `apply_inventory_action` receives the game mode (or a `creative: bool`).

### Client

A module `client/src/ui/hud/item_browser/` beside `inventory/`, built from the existing widgets
(`slot`, `text_field`, `tooltip`, `scroll`, `focus::ArrowNav`, `hints::key_cap`, `toast`). It
never writes `Res<Inventory>`: every change is an `InventoryAction` pushed to
`CurrentFrameInputs`, predicted by the client and applied by the server, like the inventory's
own clicks. Its state (`ItemBrowserState`: query, category, craftable filter, view and history,
drawer) is a resource; the list is rebuilt when it, the inventory or the game mode changes, not
every frame.

Bookmarks are kept by the client in `item_browser.ron` in the game folder, one list per world:
keyed by the world's name in solo and by the server address in multiplayer.

## Lots

1. **Catalog, read-only**: categories (shared), panel, search, chips, grid, tooltips, adaptive
   columns and drawer, keyboard. Client and shared only, no protocol change.
2. **Recipes and uses**: recipe API (shared), recipe view, R/U everywhere, history, missing
   ingredients, Craftable chip and dot. No protocol change.
3. **Creative and Fill grid**: the three actions, `PROTOCOL_ID` 18 → 19, take / one / to
   inventory / destroy on list and trash, `+ Fill grid` (Shift: as many as possible) in the 2×2
   inventory and the 3×3 table. Server session tests prove a Survival player is refused.
4. **Bookmarks**: `A`, the bookmark row, `item_browser.ron`.

## Decisions

1. Visible in Survival too, as a recipe book; nothing comes out of the catalog in Survival.
2. Always open, docked right; a drawer only when fewer than 4 columns fit.
3. In Creative a click on the list while holding a stack destroys it, as in TooManyItems, with
   a toast naming what was destroyed.
4. Fill grid is server-side: the server moves the ingredients itself.
5. Displayed name `Items`; `Item Browser` in code and docs. English only, like the rest of the UI.
6. Lots 3 and 4 of the proposal (Creative, Fill grid) share one protocol bump.
7. (Lot 1) The whole open inventory panel, hotbar row included, slides left when the docked
   browser needs room: "opening never moves the hotbar" now holds only where the panel stays
   centred, as in a drawer; the closed hotbar stays centred. The hotbar keeps its size, height
   and pixels.
8. (Lot 1) At 854×480 the formula gives 4 columns, not the 5 the mockup's prose says.
9. (Lot 1) Escape outside the search field does not clear the query, as the mockup did: it
   closes the drawer, then the inventory. In the field, it clears the query, then leaves the
   field. The query is kept for the session.
10. (Lot 1) The footer is `/ search  ↑↓ ←→ browse` in both modes, and the tooltip has no hint
    line, until the actions of the later lots exist.
11. (Lot 1) The `/` key cap sits at the right end of the search field: the placeholder of
    `widgets::text_field` sits at its left.
12. (Lot 1) 800×600 is a drawer: fewer than 4 columns fit beside the panel there.
13. (Lot 1) Over the open inventory, `/` focuses the search instead of doing nothing; the chat
    still opens on `/` only in the game.
14. (Lot 2) `+ Fill grid` is on every card, disabled (muted, out of Tab) until the server can
    fill the grid in lot 3, with the reason or the station beside it: the cards keep their size
    when it gets enabled, rather than a button appearing.
15. (Lot 2) In Creative, a click on a catalog item still only focuses it: lot 3 gives it its
    Creative meaning. R / U work in both modes, and a click on a slot of a card opens its recipes
    (right click: its uses) in both modes.
16. (Lot 2) The footer is `LMB recipes  RMB uses  / search` in Survival and
    `R recipes  U uses  / search` in Creative; `A bookmark` comes with lot 4, and the arrows'
    cap of lot 1 left for room.
17. (Lot 2) The key line of the tooltip is the text `R Recipes  U Uses` (tooltips have no key
    caps), on the slots of the catalog and of the cards; the inventory's slots keep their
    Shift+click hint. `You have the ingredients` is a fourth line of the tooltip widget, `note`,
    in the OK green.
18. (Lot 2) The stations read `Inventory 2x2` and `Crafting Table 3x3`: the game font has no `×`.
19. (Lot 2) `Needs a Crafting Table` follows the grid in use: at a crafting table, a 3×3 recipe
    whose ingredients are there shows its station, not a reason.
20. (Lot 2) Closing the inventory leaves the recipe view and empties its history, as it closes
    the drawer; only the query outlives it. `/` and Ctrl+F leave the view too.
21. (Lot 2) R / U act on the focused slot while its ring shows, else on the hovered one, as the
    number keys of the inventory do (the mockup took the hovered one first).
22. (Lot 2) With the Craftable filter and an empty search, nothing craftable reads
    `No craftable item.` (` in <Category>`) and `Gather more, or turn Craftable off.`, not
    `No item matches "".`.
23. (Lot 2) The gaps inside a card are 4 px, not the mockup's 12, so that a card fits the four
    columns of 854×480. The Craftable chip ends the row of chips, which scrolls sideways: at
    1280×720 it is out of view until the row scrolls.
24. (Lot 2) A focus on cards that are rebuilt, or on the catalog the view hides, moves to the
    back button; back on the catalog, it returns to the slot of the item that was shown.
25. (Lot 3) The chips wrap instead of scrolling sideways, so that every chip, `Craftable`
    included, is in view without scrolling: two rows at 1280×720, three in four columns, with
    4 px rather than 8 beside their labels. The Craftable chip keeps green text while off, to
    read apart from the categories. This replaces the end of Decision 23.
26. (Lot 3) Holding a stack in Creative, any gesture on a catalog item destroys the stack, but a
    right click on the item held, which adds one, as the mockup's `clickCatalog`. The server's
    refusal of `TakeFromCatalog` onto another item (`CursorFull`) is only a safety net.
27. (Lot 3) "The list" is the scroll list of the catalog, its slots and the room around them,
    and the trash: a click on the search, the chips, the title, the footer's text or the recipe
    view never destroys. Enter or Space on the focused trash destroys too.
28. (Lot 3) Only a destruction shows a toast; the mockup's `64 Dirt added` and `Grid filled`
    are left out, the inventory and the grid showing what happened. With `to_inventory`, what
    does not fit is lost without a word, as with `/give`.
29. (Lot 3) `FillCraftingGrid` is all or nothing: refused with `UnknownRecipe`, `DoesNotFit`,
    `MissingIngredients`, or `GridNotEmptied` when the inventory has no room for the grid it
    puts back. The ingredients are taken from the end of the backpack first, the hotbar last, so
    that the hotbar keeps its stacks longest; a cell keeps the type of the stack it came from.
    Shift+click sends `times: u32::MAX`, which the server clamps.
30. (Lot 3) While a chest is open, the chest takes the place of the grid in the panel: `+ Fill
    grid` is disabled with `Not beside a chest`. The server still allows it (on the 2×2 grid,
    which the closing gives back).
31. (Lot 3) Enter in the search acts as a click on the first result, as the spec says: in Survival
    it opens its recipes and the focus goes to the back button. Lot 1's Enter, which focused the
    first result, is now only ↓.
32. (Lot 3) The Creative footer is the trash and its sentence only, as the mockup; the keys of
    Decision 16's Creative footer are in the tooltips. In Creative a catalog tooltip's hint is
    two lines: `LMB 64  RMB 1  Shift to inventory`, then `R Recipes  U Uses`.
33. (Lot 3) The observers of the panel (`Submit` of the search, `Activate` of `+ Fill grid`) only
    leave a request in `BrowserRequests`: `catalog_actions` sends it in `Update`, before the
    simulation, where the actions of the frame are gathered and predicted.

## Testing

TDD. Shared logic (categories, search matching, recipe API, craftable/missing, the three actions,
Creative refusal) in unit tests; the server refusal in `server/tests/session.rs`; the panel in
e2e tests (`client/src/e2e/item_browser.rs`, one journey per behaviour group) and goldens at
1280×720 and 854×480 (docked) and 640×480 (drawer). Existing inventory goldens change only where
the inventory panel moves left, after a look at the PNGs.

## Out of scope

Smelting recipes (the furnace has none yet), a cheat mode in Survival, mod/source filters,
pagination, a settings screen for the browser, server-wide bookmarks.
