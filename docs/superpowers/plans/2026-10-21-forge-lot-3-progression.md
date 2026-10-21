# Forge Lot 3 (Progression) Implementation Plan

**Goal:** Survival gets its loop: cobblestone makes stone tools, a stone pickaxe digs iron ore,
the furnace smelts it into ingots, iron tools dig gold and diamond ore, diamonds make the best
tools. Ingots and diamonds pack into storage blocks and back. Logs smelt into charcoal, which
burns and makes torches like coal. Diamond ore lies deep, 5 to 16 blocks above the bottom. The
item browser shows the smelting of an item as a card beside its crafting cards, the uses of an
item list its smelting and, for a fuel, how many items it smelts. To see: mine iron ore with a
wooden pickaxe (it breaks, drops nothing), then with a stone one (it drops the ore); smelt it;
press R over an iron ingot in the browser.

**Spec:** `docs/superpowers/specs/2026-10-21-forge-design.md`, "Lot 3 — progression", and its
"Decisions" (1 to 23 from lots 1 and 2).

## Global Constraints

- English for code, comments, docs and commits; Conventional Commits, no trailer.
- Doc comments in full sentences, tests named as sentences, sparse comments.
- TDD: each test is written and seen failing (not compiling counts) before the code that
  makes it pass.
- `ItemId`, `BlockId` and the recipe table are append only: the new variants and recipes go at
  their end, in the spec's order; the pinned encoding tests gain their lines. `PROTOCOL_ID`
  stays 24 (bumped once for the branch in lot 1). `FORMAT_VERSION` is unchanged: new variants
  read nothing old differently.
- The smelting tables stay the shared `smelting_result` and `fuel_ticks`, read by the server,
  the dialog and the item browser alike.
- The smelting card of the item browser is a card like the crafting cards: the same frame,
  bevels, cells, fonts and spacing, the flame of the furnace dialog; it fits the browser at
  800x600 and stays crisp at every UI scale.
- `just test-fast` while working; `timeout 1800 just check` at the end.

## Decisions (where the spec is silent)

1. **Tiers.** `ToolTier { Wood, Stone, Iron, Diamond, Gold }` (in the spec's order) gives its
   `speed`, `durability` and `mining_level`, the spec's table. `Tool` gains `tier`, and keeps
   `speed` and `durability`, filled from it, which the mining and the wear already read.
   `WOODEN_TOOL_SPEED` and `WOODEN_TOOL_DURABILITY` give way to `ToolTier::Wood`. A sword
   still wears twice per block, whatever its tier.
2. **Required tool.** `get_required_tool` returns `Option<RequiredTool { kind, level }>`; the
   new `BlockId::is_harvested_by(Option<Tool>)` says whether a block broken holding a tool drops
   its items: no tool needed, or a tool of that kind whose tier's mining level reaches the
   block's. `break_block` asks it. Levels as the spec: stone, cobblestone, coal ore and both
   furnaces 0; iron ore and the iron block 1; gold ore, diamond ore, the gold block and the
   diamond block 2.
3. **Break times** in mining steps, after Minecraft's hardness as the ores' 15 steps for 3:
   diamond ore 15 (3), gold block 15 (3), iron block and diamond block 25 (5). The three storage
   blocks and the diamond ore are mined faster by a pickaxe, need one, and sound like stone
   (there is no metal sound).
4. **Drops.** Diamond ore drops a diamond; the storage blocks drop themselves.
5. **Generation.** Diamond ore is the fourth `Ore`: heights 5 to 16, 1 attempt per chunk,
   clusters of 2 to 6, `veins: 0.0` (the vein lengths are never drawn). It comes last, so it
   wins a cell another ore claims. Its share is checked by the generation test and measured
   with `worldgen_dump`.
6. **Recipes**, appended in this order: for each of cobblestone, iron ingot, gold ingot and
   diamond, the pickaxe, axe, shovel, hoe and sword with the wooden tools' shapes and sticks;
   then each storage block from 9 of its material in a full 3x3, then 9 of the material from
   its block alone; then the torch from charcoal on a stick (4 torches, as coal). The indices
   of the existing recipes are unchanged.
7. **Smelting** completes lot 1's table: iron ore → iron ingot, gold ore → gold ingot, diamond
   ore → diamond, each of the three logs → charcoal. Charcoal burns 1600 ticks, as coal. The new
   tools are not fuel; the wooden ones still are (200). `smelting_inputs(result)` lists what
   smelts into an item, in the order of `ItemId::ALL`, for the item browser.
8. **Categories.** Ingots, diamond and charcoal: Materials. The twenty tools: Tools. The
   storage blocks: Blocks. The diamond ore: Ores, with the three ores the browser already files
   there (the spec's "ore: Blocks" was written without that category).
9. **Item browser: what a view shows.** A view lists cards: the crafting recipes first, as
   before, then the smelting cards, then, in Uses, a fuel card. The tab chips count all of them,
   and so does the sentence of an item without recipe ("It goes into 2 recipes: see Uses."). The
   Recipes of an item are its crafting recipes and the smelting of each item that makes it
   (charcoal: three cards, one per log); its Uses are the crafting recipes it goes into, its
   smelting, and its burning if it is a fuel.
10. **The smelting card** keeps the crafting card's frame and its two lines. Its body is the
    furnace dialog's: a column of the input cell, the lit flame (the dialog's sprite, at twice
    its texels) and the fuel cell, dashed and empty since any fuel does, then `->` and the
    output slot. The line under it says `Furnace · 10 s` (`COOK_TICKS` in seconds), in the
    muted colour of a station, with no `+ Fill grid`: a furnace is filled by hand, so the
    Craftable filter and "You have the ingredients" keep counting the crafting recipes only.
    No missing tint either: the card says where an item is made, not what the player lacks.
    Accessible name `Smelting of <output>`.
11. **The fuel card** is the same body with the fuel cell holding the item and the input and
    output cells dashed; its line says how many items one of it smelts: `Fuel · burns for 8
    items` (`fuel_ticks / COOK_TICKS`: 1.5 for a log, 1 item for a wooden tool, 0.5 for a stick).
    Accessible name `<item> as fuel`.
12. **Textures** from Pixel Perfection, all 16x16: `item/iron_ingot`, `gold_ingot`, `diamond`,
    `charcoal`, the `stone_`, `iron_`, `golden_` and `diamond_` `pickaxe`, `axe`, `shovel`, `hoe`
    and `sword` in `items/`; `block/diamond_ore`, `iron_block`, `gold_block`, `diamond_block` in
    `blocks/`. Our `IronOre.png`, `GoldOre.png` and `CoalOre.png` are already the pack's,
    byte for byte the same pixels: left as they are.
13. **Preview tools:** the four blocks join the palette of `worldgen_preview.py`.

## Tasks

### Task 1: Tool tiers and mining levels (`shared`)
- [ ] Tests (items.rs): `ToolTier` table (speed, durability, level); every tool's kind and tier
  (the twenty-five); tools stack to 1 and start unworn. (blocks.rs): the required tool and
  level of each block; `is_harvested_by` by tier (wood on iron ore no, stone yes, gold no;
  iron on diamond ore yes; anything on dirt yes; an axe on stone no). (players/blocks.rs): iron
  ore broken with a wooden pickaxe is not harvested, with a stone one it is; a stone pickaxe
  breaks stone faster than a wooden one. See them fail.
- [ ] The new items and blocks (appended, pinned tables, `ALL`), `ToolTier`, `Tool::tier`,
  `RequiredTool`, `is_harvested_by`, break times, drops, faster mining, categories, default
  types; `break_block` asks `is_harvested_by`. Client: the sounds of the new blocks; the
  preview palette; the generation test's ore list. See them pass. Commit `feat: stone, iron,
  gold and diamond tools, and the mining levels`.

### Task 2: Textures (`data`)
- [ ] The 24 textures from the pack (16x16 checked), credited in `docs/ressources.txt`; the
  client test that every item and block has a texture passes. Look at them. Commit `feat: the
  textures of the ingots, the diamond, the new tools and blocks`.

### Task 3: Smelting and recipes (`shared`)
- [ ] Tests (furnace.rs): ores and logs smelt; charcoal burns 1600; the new tools do not burn;
  `smelting_inputs`. (crafting.rs): every tool of every material; 9 ingots ↔ a block; charcoal
  torches; the count of recipes and the uses of a stick. See them fail.
- [ ] The tables and the recipes. See them pass. Commit `feat: ingots, diamonds and charcoal
  from the furnace, and the recipes of the new tools and blocks`.

### Task 4: Diamond ore underground (`server`)
- [ ] Test (ores.rs): diamond ore keeps to its heights and is rarer than gold, within a share.
  See it fail. The ore. See it pass. Measure the share with `worldgen_dump`. Commit `feat(server):
  diamond ore deep underground`.

### Task 5: Smelting in the item browser (`client`)
- [ ] Tests (recipes.rs): the texts (`Furnace · 10 s`, `Fuel · burns for 8 items`, 1.5, 1 item,
  0.5); the cards of a view (charcoal's three smeltings, an oak log's crafting, smelting and
  fuel); the counts and the empty message; craftable now ignores smelting. (goldens.rs): a view
  with a smelting card builds no `+ Fill grid` nor tint for it; goldens of the recipes of an
  iron ingot and of the uses of an oak log. See them fail.
- [ ] `BrowserCard`, `cards_of`, `spawn_smelting_card`, the flame; the review test opens the
  uses of a log beside the furnace at every size. See them pass; look at the goldens and the
  review snapshots at 800x600, 1280x720 and 1920x1080. Commit `feat(client): smelting and
  fuels in the item browser`.

### Task 6: A player progresses (e2e)
- [ ] `client/src/e2e/survival.rs` or `furnace.rs`: a player smelts iron ore into an ingot; the
  item browser shows its smelting card. Commit `test(client): ...`.

### Task 7: Closing
- [ ] `CHANGELOG.txt`; spec decisions appended; goldens reviewed; `timeout 1800 just check`;
  commit `docs: decisions of the progression (lot 3)`.
