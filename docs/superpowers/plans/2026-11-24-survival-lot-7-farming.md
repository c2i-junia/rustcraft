# Survival Lot 7 (More farming, one random tick) Implementation Plan

**Goal:** One system gives the blocks near the players their random ticks: lot 3's fires and
lava and main's plants, cisterns and campfires, at their rates. Carrots and potatoes grow on
tilled soil like rye, faster by water; potatoes bake. To see: find a carrot in tall grass, hoe
the dirt by a pond, plant it, wait, harvest 2 to 4.

**Spec:** `docs/superpowers/specs/2026-11-24-survival-death-design.md`, "Lot 7".

## Global Constraints

- English, Conventional Commits, no trailer. Doc comments on items, tests named as sentences.
- TDD: each test seen failing (not compiling counts) before the code.
- `PROTOCOL_ID` stays 30. Append only: `BlockId` 99–106 `CarrotSprout`, `YoungCarrots`,
  `TallCarrots`, `RipeCarrots`, `PotatoSprout`, `YoungPotatoes`, `TallPotatoes`,
  `RipePotatoes`; `ItemId` 159–161 `Carrot`, `Potato`, `BakedPotato`. `FORMAT_VERSION`
  unchanged (new variants only).
- `timeout 600 cargo test -p <crate>` while working, `timeout 1200 just check` at the end.

## Decisions (where the spec is silent)

1. **One system.** `server/src/world/growth.rs` (`growth_system`) is folded into
   `server/src/world/ticks.rs` (`block_ticks_system`, `BlockTicks::tick`) and removed. Its
   rules move to shared `shared/src/world/growth.rs`, called by `ticks::random_tick`, but the
   sapling's tree: its shape is the server's generator (`sapling_tree`), so the server grows it
   on the random tick of a sapling, with the shared odds `growth::SAPLING_ODDS`.
2. **One "near the players".** Lot 3's: the loaded chunks within `min(render distance,
   SIMULATION_DISTANCE = 4)` of a player, but the chunks of air
   (`ticks::chunks_near_players`). Main's 7×5×7 chunks around each player widen to lot 3's
   9×9×9: plants a little farther grow too, at the same rate.
3. **Same rates.** A cell's chance of a tick each tick was 4/4096 for main and 3/4096 for lot
   3. `RANDOM_TICKS_PER_SECTION` becomes 4, main's `CELLS_PER_CHUNK`, so every growth odds
   stays as main had it (crop stage 1 in 4, berries 1 in 5, saplings 1 in 6, cisterns and
   campfires at once under rain), and lava's `IGNITE_ODDS` goes from 3 to 4 so that it lights
   fires as often as before (1/4096 a tick). Fires keep their own schedule and bound (64 a tick);
   a fire not yet scheduled is found a third sooner, no matter.
4. **Rain on it.** Main's "no block within 64 above" becomes lot 3's `sees_sky` (decorations
   let the rain through; no height limit): one notion of the open sky.
5. **Crops, one block per stage** as main's rye (no stage in `BlockData.level`): four stages
   each, for Pixel Perfection's `carrots_stage0..3` / `potatoes_stage0..3`. Each stage steps
   up 1 in 4 random ticks, 1 in 2 when watered, as rye's (rye included).
6. **Water.** A crop is watered when the rain falls on it (main's) or when water lies within
   4 blocks of its soil horizontally, at the soil's height or one above (Minecraft's farmland).
   The two do not add up: watered is twice as fast, not four times.
7. **Planting.** `Carrot` and `Potato` are food *and* seed: their item type is the block of
   their sprout, with `soil()` = tilled soil, and a right click on tilled soil with free air
   above plants it (in `use_block`, before eating); anywhere else the right click eats.
8. **Drops.** Unripe stages 1 of their item; ripe 2, 3 or 4 (a table of three equal entries).
   Tall grass: `(24 TallGrass, 16 Fiber, 8 Rye, 1 Carrot, 1 Potato)` of 50, so carrot and
   potato are 2 % each and main's three keep their ratios.
9. **Baked potato.** `smelting_result(Potato) = BakedPotato`; the campfire bakes potatoes as it
   roasts meats. Food: carrot 3/3.6, potato 1/0.6, baked potato 5/6.0 (lot 6).
10. **Items** in `Materials` like main's rye; textures `item/{carrot,potato,baked_potato}.png`.
    Crop blocks drawn as main's rye (flora, swaying top), grass sounds, foliage surfaces, not
    flammable (as rye).

## Tasks

1. Shared growth rules (`shared/src/world/growth.rs`): tests for crop stages, rain and water
   bonus, berries, cisterns, campfires; `random_tick` dispatches to them. Lava odds 4.
2. Server: `BlockTicks::tick` grows saplings, `chunks_near_players`, `growth.rs` removed,
   tests: growth through the unified tick, fire bound kept.
3. Blocks and items: the 8 crop blocks, the 3 items, drops, planting, smelting, campfire, food;
   pinned encodings.
4. Client: rendering lists, sounds, surfaces; textures; item counts; goldens.
5. Docs: ARCHITECTURE, ressources, spec decisions. `just check`.
