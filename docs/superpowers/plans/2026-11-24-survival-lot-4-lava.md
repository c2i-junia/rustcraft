# Survival Lot 4 (Lava) Implementation Plan

**Goal:** Lava: a liquid that flows like water but slower and shorter, burns and hurts whoever
swims in it, sets fire to what burns around it, and hardens into obsidian or cobblestone where
water meets it. The bucket takes and pours it, and a lava bucket is the longest furnace fuel.
Caves are flooded with lava up to y = 10. To see: dig down to the bottom of a cave, pour a
water bucket on the lava lake.

**Spec:** `docs/superpowers/specs/2026-11-24-survival-death-design.md`, "Lot 4".

## Global Constraints

- English, Conventional Commits, no trailer. Doc comments on items, tests named as sentences.
- TDD: each test seen failing (not compiling counts) before the code.
- `PROTOCOL_ID` stays 29: `BlockId::Lava`, `BlockId::Obsidian`, `ItemId::Obsidian` and
  `ItemId::LavaBucket` are appended. `FORMAT_VERSION` unchanged: the level of lava is its
  `BlockData.level`, already saved; `Player.lava_damage_wait` is not saved.
- `timeout 600 cargo test -p <crate>` while working, `timeout 1200 just check` at the end.

## Decisions (where the spec is silent)

1. **Liquids made generic** (`shared/src/world/liquid.rs`). `Liquid { block, level_drop,
   ticks_per_step }` with `WATER` (drop 1, 5 ticks) and `LAVA` (drop 2, 30 ticks), and
   `Liquid::of(BlockId)`. Flowing lava takes the levels 2, 4 and 6, as Minecraft's: its reach
   is 3 on flat ground and its surface slopes down as water's does, with no new level encoding.
   `flow_step(world, pos, liquid)` is the old rule for that liquid; a cell of the other liquid
   is a wall to it. `is_source` is true for a source of any liquid (the bucket and the ray aim
   at both).
2. **Water meets lava** (`harden`), checked first in `flow_step` whichever liquid steps: lava
   with water in one of its six neighbours becomes obsidian if it is a source, else
   cobblestone. Lava does not flood water, nor water lava, so they only ever meet side by side
   or one above the other, and the lava cell is the one that turns. The server schedules
   every changed cell in both liquids' schedules, so the hardening comes at the next water
   step (5 ticks), not after the 30 ticks of lava.
3. **The flow on the server** (`server/src/world/flow.rs`). `LiquidFlow` holds one schedule
   per liquid, each with its own period and its `MAX_CELLS_PER_STEP`. Generated chunks mark
   no cell changed, so lava lakes at rest are never scheduled: only what a player or a flow
   changes is.
4. **The blocks.** `Lava`: `Liquid`, no hitbox, unbreakable, replaceable, light 15, no drop,
   no block sound. Drawn with the solid faces (its texture is opaque, and the liquid material
   draws water's waves and reflections), sloped like water, not tinted. Texture
   `lava_still.png` (16x256) as `Lava.png` with `Lava.anim.ron` `(frame_time: 4, interpolate:
   false)`. `Obsidian`: solid, needs a diamond pickaxe (mining level 3), break time 250 hits
   (`breaking_progress` is a `u8` saved in the chunks, so this is as long as it gets: 12.5 s by
   hand, 1.6 s with a diamond pickaxe), drops itself. `ItemId::Obsidian` (Blocks).
5. **In lava** (`shared/src/players/fire.rs`). `Player::bathe_in_lava(in_lava)`: the body
   touching lava sets burning to at least `LAVA_BURNING_TICKS` (300) and hits for
   `LAVA_DAMAGE` (4) at once and every `LAVA_DAMAGE_INTERVAL` (10) ticks (`lava_damage_wait`,
   a hit refused by the invulnerability is tried again the next tick). It runs before `burn`
   in `burn_system`, so the fire's 1 damage lands inside the invulnerability of a lava hit and
   is ignored. A body in lava is not doused (rain, or water touched on the other side).
6. **Movement.** A body touching lava moves at `LAVA_SPEED_MULTIPLIER` (0.3) of the walking
   speed, against 0.6 in water, and cannot swim faster.
7. **Lava's random tick** (`shared/src/world/lava.rs`): one time in 3, a cell drawn among the 18
   of the 3x2x3 box above and around the lava (its own level and the one above); air there
   with a flammable neighbour catches fire (age 0). The server schedules a fire made by any
   random tick already.
8. **Bucket.** An empty bucket takes a water or a lava source, giving `WaterBucket` or
   `LavaBucket`; the lava bucket pours a lava source like water (`pour_liquid`). Lava bucket
   stacks to 1, category Tools.
9. **Furnace fuel.** `fuel_ticks(LavaBucket) = 20000`; `fuel_remains(item)` names what stays in
   the fuel slot once an item is burnt (`LavaBucket` → `Bucket`), for an item that stacks to 1.
10. **World generation.** `LAVA_LEVEL = 10` (`caves.rs`): a carved cell at or below it is a lava
    source (`carved_block(y)`), in both carving paths. The spawn stands on a surface that is
    always above `LOWEST_SURFACE` (> 10), and animals only on grass or snow above the sea:
    tests pin both. Animals that wander refuse any liquid, not only water.
11. **Client.** Fog when the eyes are in lava: `LAVA_FOG_COLOR` (orange), from 0 to 2 blocks.
    `liquid::eye_liquid` names the liquid the eye is in. The minimap reads the block colours.

## Tasks

### Task 1: Blocks and items in shared
- [x] Tests: lava is a glowing, unbreakable, replaceable liquid; obsidian needs a diamond
  pickaxe, takes 250 hits, drops itself; the lava bucket stacks to 1, is a tool, burns 20000
  ticks and leaves its bucket; pinned encodings.
- [x] Variants, tables, fuel remains in `Furnace::burn_fuel`.

### Task 2: Generic flow and hardening
- [x] Tests (`liquid.rs`): lava spreads to levels 2, 4, 6 and no further; water unchanged; a
  lava source by water becomes obsidian, flowing lava cobblestone; lava and water do not
  flood each other. Server (`flow.rs`): a lava source spreads 3 blocks, a step per 30 ticks;
  water poured by a lava lake hardens it within 5 ticks; a lake at rest schedules nothing.
- [x] `Liquid`, `flow_step(.., liquid)`, `harden`, `LiquidFlow`.

### Task 3: Lava's random tick
- [x] Tests (seeded): lava by planks lights fire in the air next to them; never next to stone
  only.
- [x] `lava.rs`, the arm in `random_tick`.

### Task 4: Players in lava
- [x] Tests: 4 damage every 10 ticks and burning 300; death cause Lava; creative unharmed;
  water still puts out burning once out of lava; slower than in water.
- [x] `bathe_in_lava`, `body_in_lava`, `burn_system`, movement.

### Task 5: Bucket
- [x] Tests: a bucket takes a lava source into a lava bucket; the lava bucket pours a source.
- [x] `fill_bucket`, `pour_liquid`.

### Task 6: World generation
- [x] Tests: a carved cell at y <= 10 is a lava source, above it air; the spawn is not in lava.
- [x] `carved_block`, both carving paths.

### Task 7: Client
- [x] Shape, meshing with the solid faces, textures (`Lava.png`, `Lava.anim.ron`,
  `Obsidian.png`, `LavaBucket.png`), `WetSurface`, sounds, the fog.
- [x] e2e (`survival.rs`): a player in a lava lake of the generated world burns, loses health
  and sees the orange fog.
- [x] Goldens of the item browser: only the diffs of the new items regenerated.

## Notes

- `bench_generation` (release, radius 6): 2.25–2.29 s with the lava lakes against 2.27 s
  without (`LAVA_LEVEL = -1`), no measurable cost. Generated chunks mark no cell changed, so the
  flow schedules none of their lava (`a_chunk_loaded_full_of_lava_schedules_nothing`).
- The goldens of the item browser and of the inventory screens that dock it changed by the two
  new items only (obsidian among the blocks, the lava bucket after the water bucket, the
  count); the minimap and world map goldens, rewritten by `UPDATE_GOLDEN` within tolerance,
  were restored.
- Lava makes no block sound (as water); its hardening into obsidian plays the stone placing.
- The air bar counts down in lava too (any liquid, lot 2), though lava kills first.
