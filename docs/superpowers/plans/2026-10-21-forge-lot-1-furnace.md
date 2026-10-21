# Forge Lot 1 (The furnace, shared and server) Implementation Plan

**Goal:** A furnace that works on the server: input, fuel and output slots, smelting over time
by Minecraft's rules, a lit furnace block that glows while it burns, opened and filled through
inventory actions, sent to the player who has it open, and kept in the save. No dialog yet (lot
2). To see: right click a furnace, shift-click sand then coal from the inventory (they go to
its unseen input and fuel slots): it lights, glows, and goes out once the sand is glass.

**Spec:** `docs/superpowers/specs/2026-10-21-forge-design.md`, "Lot 1 — the furnace, shared and
server", and its "Decisions".

## Global Constraints

- English for code, comments, docs and commits; Conventional Commits, no trailer.
- Doc comments in full sentences, tests named as sentences, sparse comments.
- TDD: each test is written and seen failing (not compiling counts) before the code that
  makes it pass.
- `BlockId`, `InventoryAction` and the messages are append only; `PROTOCOL_ID` goes from 23 to
  24 once, with the first change of the wire (the new block), for the whole branch.
- `FORMAT_VERSION` stays 5: a world without `furnaces/` has empty furnaces.
- The client keeps compiling and behaving: it carries the new field of `PlayerUpdateEvent`
  into its map, as it does the open chest; the dialog is lot 2.
- `just test-fast` while working; `just check` (fmt, clippy `-D warnings`, every test) at the
  end.

## Decisions (where the spec is silent)

1. **Textures.** Our `Furnace.png` (one opening, grey) and `FurnaceTop.png` are not the pack's
   front/side/top: the furnace shows its opening on its four sides. That look is kept. The lit
   furnace mirrors it: the pack's `block/furnace_front_on.png` as `blocks/LitFurnace.png` on
   its four sides, and `FurnaceTop` on its top and bottom (no copy of the file). It is 16x16
   and not animated in the pack.
2. **Facing.** The placement gives every block `Front` today. A furnace is turned so that its
   front looks at the player who placed it: toward the opposite of the main horizontal axis of
   the look. The client drew a turned whole block culled and lit by its unturned sides (only
   the vine and the cocoa were turned, whose faces are inset): it now culls and lights a side
   of a turned block by the side it is turned to. With decision 1 the four sides look alike.
3. **Usable.** `Furnace` and `LitFurnace` are usable (`is_usable`), so that a right click opens
   them instead of placing a block against them; the client sends `OpenFurnace` where it sends
   `OpenChest`. Until lot 2 that opens the plain inventory.
4. **Tick order**, Minecraft's (`AbstractFurnaceBlockEntity.serverTick`): `burn_left` drops by
   one first, then a furnace out that can smelt burns a fuel item, then it cooks. So a fuel of
   1600 ticks cooks 1600 ticks, 8 items, and the next fuel item lights on the tick the last
   one ran out, with no gap. A furnace that cannot smelt (no input, no result, output full)
   has `cook = 0`, as the spec says; one that could but burns nothing loses 2 per tick.
5. **`FurnaceChange`** is an enum: `Same`, `Changed`, `Toggled` (lit or out, which is a change
   too). The server sets the block from `Furnace::is_lit` whenever they disagree, which also
   mends a lit block whose furnace state was lost.
6. **Tables.** Smelting in lot 1: Sand → Glass, Cobblestone → Stone (the ores' ingots and the
   charcoal do not exist yet). Fuels: Coal 1600; the three logs, the three planks, the crafting
   table and the chest 300; the five wooden tools 200; the stick 100.
7. **Slot rules.** A slot that does not take the held item (the output, or the fuel slot for
   anything but a fuel): a left click takes from it what the cursor can hold when it holds the
   same item, else does nothing; a right click drops nothing; a number key is refused when the
   hotbar slot holds an item it does not take. The new `InventoryRefusal::NotTaken(slot)`
   says so. The furnace slots are 72 (input), 73 (fuel) and 74 (output): `furnace_slots()`,
   `FURNACE_INPUT_SLOT`, `FURNACE_FUEL_SLOT`, `FURNACE_OUTPUT_SLOT`.
8. **Storage in the map.** `ChunkMap` keeps the chests' contents and the furnaces in one
   generic store (`Held<T>`: values by position, and the positions changed), so the furnaces
   are not a copy of the chests. Its public methods for the chests keep their names; the
   furnaces get `furnace`, `furnace_mut`, `insert_furnace`, `take_furnace`,
   `furnaces_of_chunk`, `furnace_positions` and `take_changed_furnaces`. A block swap between
   `Furnace` and `LitFurnace` keeps the furnace.
9. **Files.** `container_files.rs` keeps its name and its functions, made generic over a
   `Stored` trait (`Container`, `Furnace`): folder, size bound, the blocks that hold one, how a
   map stores one. `ChunkContainers<T = Container>`. What the blocks of a chunk hold travels as
   `ChunkContents { chests, furnaces }` (read with the chunk, handed to the writer). The
   writer writes or removes each kind's file per chunk the same way.
10. **Breaking.** `BrokenBlock` gains `furnace: Option<Furnace>`, dropped with the block like
    `contents`.
11. **Ticking** runs in the `Simulate` set right after the players' inputs.
12. **Preview tools:** `LitFurnace` joins the palette of `worldgen_preview.py` (indexed by the
    variants). `CHANGELOG.txt` waits for lot 2, when a player can use the furnace.

## Tasks

### Task 1: The furnace model (`shared/src/world/furnace.rs`)
- [x] Tests: `smelting_result`, `fuel_ticks` (each entry, a non-fuel); tick rules: an idle
  furnace stays the same; it lights with fuel and a smeltable input and takes one fuel item;
  it does not light without input, with an input that has no result, or with the output full
  or of another item; it cooks one item in `COOK_TICKS`; a coal smelts 8 items and the next
  coal follows with no gap; the fuel burns on with the input taken out, and `cook` falls to 0;
  out of fuel with input, `cook` goes back 2 per tick; output stacks up to its max then stops;
  `Toggled` when it lights and when it goes out; `furnace_encoding_is_pinned`. See them fail.
- [x] `Furnace`, `COOK_TICKS`, `FurnaceChange`, `smelting_result`, `fuel_ticks`, `tick`,
  `is_lit`, `stacks`. See them pass. Commit `feat(shared): the furnace smelts by Minecraft's rules`.

### Task 2: The lit furnace block (`shared`, `client`)
- [x] Tests: `LitFurnace` 35 in `block_encoding_is_pinned`, `ALL_BLOCKS`;
  `the_lit_furnace_is_a_furnace_that_glows` (break time, tool, drop, usable, light 13,
  `is_furnace`, the furnace usable too); client: the lit furnace's faces (`LitFurnace` sides,
  `FurnaceTop` top and bottom), its sound (stone). See them fail.
- [x] The variant, its behaviour, `is_furnace`, the shape, the sound, the texture, the preview
  palette, `PROTOCOL_ID` 24. See them pass. Commit `feat: the lit furnace`.

### Task 3: Furnaces in the map, placed and broken (`shared`)
- [x] Refactor: the chests' contents in `Held<Container>`; the chest tests pass unchanged.
  Commit `refactor(shared): the chunk map keeps the chests' contents in a generic store`.
- [x] Tests (chunk_map.rs): a furnace is only stored where a furnace or a lit furnace stands;
  swapping the two keeps it; replacing or removing it drops it; changes are recorded; a removed
  chunk drops its furnaces. (blocks.rs): placing a furnace creates an empty one; a broken
  furnace hands its stacks over; the server drops them (dropped_items.rs). See them fail.
- [x] `Held<Furnace>` and the furnace methods; placement; `BrokenBlock::furnace`;
  `drop_block_items`. See them pass. Commit `feat: a placed furnace holds an empty furnace and
  drops what it holds when broken`.

### Task 4: The furnace faces its player (`shared`, `client`)
- [x] Tests: a furnace placed looking north, east, south, west faces the player; other blocks
  stay `Front`. Client meshing: a turned whole block next to stone hides the face against the
  stone and draws the one toward the air, lit by the cell it looks to. See them fail.
- [x] The direction at placement; the meshing culls and lights the sides of a turned block by
  their turned side. See them pass. Commit `feat: a furnace faces the player who placed it`.

### Task 5: Opening and filling a furnace (`shared`, `client`)
- [x] Tests (inventory.rs): `the_furnace_slots_follow_the_chest_slots`; a furnace (lit or not)
  opens within reach and closes a table or a chest, and opening those closes it; clicks move
  items between the furnace and the inventory; the fuel slot takes only fuels (left, right,
  number key); the output takes nothing but gives (left with the same item tops the cursor
  up); shift-click routes smeltable → input, fuel → fuel, else nothing, and a furnace slot
  back to the inventory; `Close` closes it and keeps its stacks; a furnace walked away from or
  broken closes; its slots are refused when it is not open or its state unknown. Pinned
  `OpenFurnace` (11). See them fail.
- [x] `Inventory::open_furnace`, the action, the lending of the three slots, the rules,
  `simulate_inventory_actions`; `PlayerUpdateEvent::open_furnace` (pinned test builds it);
  the client takes it into its map like the chest, and a right click on a furnace sends
  `OpenFurnace`; a loaded player has no furnace open. See them pass. Commit `feat: a furnace
  opens and takes items in its three slots`.

### Task 6: The server smelts (`server`)
- [x] Tests (`server/src/world/furnaces.rs`): ticking the map lights a furnace with fuel and
  input, swaps its block to a lit furnace keeping its direction, records it changed; a furnace
  that goes out swaps back; an idle one records nothing; a lit block whose furnace burns
  nothing is put out. Broadcast: a changed furnace marks its chunk dirty. The player update
  carries the open furnace. See them fail.
- [x] `tick_furnaces`, `tick_furnaces_system` in `ServerSet::Simulate`; the broadcast drains the
  changed furnaces into `dirty_chunks`; `broadcast_player_updates_system` sends
  `open_furnace`. See them pass. Commit `feat(server): the furnaces smelt on the server`.

### Task 7: The furnaces are saved (`server`)
- [x] Tests: furnaces written and read back by global position (`furnaces/`), a chunk left
  without furnaces loses its file, a furnace in every cell fits the limits; a save writes the
  furnaces of the dirty chunks; a saved chunk is read with its furnaces; a loaded furnace
  block without a saved state is empty, a stray state is dropped; a world without `furnaces/`
  loads. See them fail.
- [x] `Stored`, generic files, `ChunkContents`, the writer, the loader, the clean-up of
  interrupted writes. See them pass. Commit `feat(server): the furnaces are saved with their
  chunks`.

### Task 8: Closing
- [x] Spec decisions appended; `just check`; commit `docs: decisions of the furnace (lot 1)`.
