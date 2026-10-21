# Forge Lot 2 (The furnace dialog, client) Implementation Plan

**Goal:** A right click on a furnace, lit or out, opens the inventory with a furnace section where
the chest section goes, laid out as Minecraft's: the input slot, the flame under it, the fuel
slot under the flame, an arrow pointing right to a larger output slot. The flame burns down with
the fuel, the arrow fills as the input cooks. Clicks, right clicks, shift-clicks, number keys
and the arrows of the keyboard work on its three slots, as on the others. A placed furnace shows
the pack's front toward its player, its sides, top and bottom. To see: place a furnace, right
click it, shift-click sand then coal: the flame lights, the arrow fills, and glass comes out.

**Spec:** `docs/superpowers/specs/2026-10-21-forge-design.md`, "Lot 2 — the furnace dialog,
client", and its "Decisions" (5 to 15 from lot 1).

## Global Constraints

- English for code, comments, docs and commits; Conventional Commits, no trailer.
- Doc comments in full sentences, tests named as sentences, sparse comments.
- TDD: each test is written and seen failing (not compiling counts) before the code that
  makes it pass.
- No change to the wire, the save or the shared rules: lot 1 made them. The dialog reads the
  furnace the server sends (`ClientWorldMap::blocks.furnace(inventory.open_furnace?)`) and sends
  the inventory actions the other slots send; the client predicts them with the shared rules.
- The dialog follows the chest section of the inventory panel: the same slots, bevels, spacing,
  font and title; it fits at 800x600 and stays crisp at every UI scale (sprites drawn at twice
  their texels, like the item icons).
- `just test-fast` while working; `timeout 1500 just check` at the end.

## Decisions (where the spec is silent)

1. **Textures** (the ruling given for this lot, over lot 1's decision 5): the pack's model,
   `models/block/furnace.json` and `furnace_on.json`: `furnace_front` (`Furnace.png`, also the
   item's icon) or `furnace_front_on` (`LitFurnace.png`) on the front only, `furnace_side`
   (`FurnaceSide.png`) on the other three sides, `furnace_top` (`FurnaceTop.png`) on top. The
   model gives the bottom its own `furnace_bottom` (`FurnaceBottom.png`): it is taken, as
   `CLAUDE.md` asks to follow the pack's model.
2. **Sprites.** The flame and the arrow are the pack's `gui/sprites/container/furnace/
   lit_progress.png` (14x14) and `burn_progress.png` (24x16), drawn at twice their texels, like
   the item icons. The pack's empty flame and arrow are painted into its stone background
   (`gui/container/furnace.png`), which does not fit our dark panel: the empty ones are
   silhouettes of the lit ones, white, tinted with `BEVEL_HI` by the dialog.
3. **Progress, as Minecraft draws it.** The flame shows `ceil(13 * burn_left / burn_total) + 1`
   of its 14 rows from the bottom while lit, none while out; the arrow
   `ceil(24 * cook / COOK_TICKS)` of its 24 columns from the left. Whole texels only, so that
   the sprites stay crisp. No smoothing: the server sends the open furnace with every player
   update, 20 times a second, a step of the arrow every 8 ticks at most; the dialog draws the
   last one, never ahead of it.
4. **Layout.** The furnace section is centred in the panel, Minecraft's place: a column of the
   input slot, the flame and the fuel slot (`SPACE_XS` apart), then the arrow and the output
   slot, `SPACE_L` apart like the crafting row. The output slot is `SLOT_SIZE + 2 * SPACE_S`
   (66 px) wide, Minecraft's larger result slot, with its icon still 32 px.
5. **Title** `Furnace`, the chest's place for the title.
6. **Hints.** With a furnace open, an inventory slot whose item smelts or burns says
   "Shift+click: move to the furnace"; another one has no hint, since a shift-click leaves it
   where it is (lot 1, decision 11). The furnace slots say "Shift+click: move to the
   inventory".
7. **Accessibility.** The flame and the arrow are `Role::ProgressIndicator`, labelled "Fuel" and
   "Smelting", their value in percent.
8. **Item browser.** Like a chest, an open furnace takes the place of the crafting grid: a recipe
   that fits the grid says "Not beside a furnace", and `+ Fill grid` is disabled.
9. **Sound.** No crackle of the lit furnace: the audio plays one-shot sounds only, without
   emitters that last at a place; it is not cheap. Left for later.

## Tasks

### Task 1: The furnace takes the pack's model (`client`, `data`)
- [x] Tests (voxel.rs): the furnace's faces are `FurnaceTop`, `FurnaceBottom`, `Furnace` at the
  front, `FurnaceSide` on the three other sides; the lit furnace the same with `LitFurnace` at
  the front. See them fail.
- [x] The textures from the pack (16x16 checked), the shapes, the credits in
  `docs/ressources.txt`. See them pass. Look at a placed furnace facing its player.
  Commit `feat: the furnace shows its front to its player, as the pack's model`.

### Task 2: The furnace section of the inventory panel (`client`)
- [x] Tests (inventory/furnace.rs): the rows of the flame and the columns of the arrow by
  Minecraft's formulas (out, full, half, last tick, a furnace with `burn_total` 0). Tests
  (display.rs): a furnace cell shows what the open furnace holds; the furnace takes the place
  of the crafting row and names the dialog, and leaves it on closing; the hints of the
  inventory slots and of the furnace slots; the setup builds the three furnace cells; the
  flame and the arrow show the synced furnace. See them fail.
- [x] `FurnaceRow`, `FurnaceFlame`, `FurnaceArrow`, `spawn_furnace_row`,
  `display_furnace_system`, `stack_in` for the furnace slots, the title and the rows in
  `display_crafting_system`, the hints; the sprites. See them pass.
- [x] Goldens: the dialog empty, and mid-smelt (lit, half the arrow). Look at them.
  Commit `feat(client): the furnace dialog`.

### Task 3: The item browser beside a furnace (`client`)
- [x] Tests: `recipe_problem` says "Not beside a furnace"; `+ Fill grid` is disabled with a
  furnace open. See them fail; make them pass. Commit `feat(client): the item browser knows the
  furnace takes the place of the grid`.

### Task 4: A player smelts (e2e)
- [x] `client/src/e2e/furnace.rs`: place a furnace, right click it: the dialog, empty; shift-click
  sand and coal from the inventory into the input and the fuel; the flame lights and the arrow
  fills; the glass comes out, and a click takes it; a click holding dirt on the output leaves
  it; a shift-click on dirt moves nothing; the keyboard reaches the furnace slots and Enter puts
  the held stack in. The review test opens a furnace at the three resolutions.
- [x] Run them, look at the review snapshots, fix what is ugly. Commit `test(client): a player
  smelts sand in a furnace`.

### Task 5: Closing
- [x] `CHANGELOG.txt` (the furnace a player can use); spec decisions appended; `just check`;
  commit `docs: decisions of the furnace dialog (lot 2)`.
