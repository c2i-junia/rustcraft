# Jungle biome

Approved proposal: https://claude.ai/artifact/Vf21qDSy2ZTaLMYAZkt4gg (2026-10-20, every default
kept). Branch `feat/jungle-biome`, worktree `feat-jungle-biome`.

## Goal

A hot and wet biome along the warm coasts: giant jungle trees with a 2 × 2 trunk standing
fifteen blocks above the forest, vines hanging from their crowns, and a ground hidden under
bushes, ferns and tall grass. It takes the wet half of today's hot forests.

## Decisions

| Topic | Choice |
|---|---|
| Climate | `Jungle` when temperature > `JUNGLE_TEMPERATURE` (0.65) and humidity > `JUNGLE_HUMIDITY` (0.55), below the sea's humidity (0.73). The mountains still come from the altitude first: no jungle at or above 84. Seed 42: 5.8 % of the world, forest 12.8 % → 7.0 %; it only touches forest, shallow sea, rivers and lakes. |
| Relief | Unchanged. The jungle is low because it follows the coast. |
| Ground | Grass over dirt, like the forest. Beaches and river banks stay sand. |
| Depth | A value in `[0, 1]` read from the climate already computed (no extra noise): `min(smoothstep(0.65, 0.75, temperature), smoothstep(0.55, 0.62, humidity))`, 0 at the border of the biome, 1 at its heart; 0 outside the jungle. It scales the densities, so that the forest turns into jungle gradually. |
| Densities (per column) | Giant tree `0.004 × smoothstep(0.5, 1, depth)` (none below half depth). Jungle tree `0.03 × depth`. Bush `max(0.01, 0.06 × depth)`. Fern `0.04 + 0.08 × depth`. Tall grass `0.10 + 0.12 × depth`. No flowers. No herds of animals. |
| Giant tree | Trunk 2 × 2, 16–25 blocks, rooted on the column and its +x, +z, +x+z neighbours (all three dry and within 2 blocks of the root's height). The trunk starts 3 blocks under the root: the terrain outranks it, so it only fills the gaps under the lower columns. Crown: four disks of jungle leaves around the trunk's centre, radii 4.6, 5.4, 4.4, 2.6 from one block under the top of the trunk, edges frayed. 2–4 branches of logs, 2–3 long, from half to 4/5 of the trunk, each ending in a small blob of leaves (radii 2.4, 1.5). Vines on 1 trunk face in 5 and under 1 crown-rim leaf in 3, 1–7 long. Every block within 6 columns of the root (`GIANT_MARGIN`) and at most 27 above the surface. |
| Spacing | Two giants never root within 8 columns (Chebyshev) of each other: among candidates, only the smallest roll grows (ties broken by coordinates). Decided from positions alone, like the rest. |
| Jungle tree | Trunk of jungle logs, 5–9. Crown 5 × 5 of jungle leaves on the top two layers below the top (corners at random), 3 × 3 at the top, a plus on top. Vines under 1 in 4 of the lowest layer's edge leaves, outwards, 1–4 long. Cocoa (lot 5): on 1 tree in 3, 1–2 pods against the top of the trunk. |
| Bush | One jungle log on the ground under a mound of jungle leaves: a diamond of radius 2 (edges at random), a plus above, sometimes one more leaf on top. |
| New blocks | `JungleLog`, `JunglePlanks`, `JungleLeaves`, `Fern`, `Vine`, `Cocoa` (lot 5), appended to `BlockId`. |
| New items | `JungleLog`, `JunglePlanks`, `JungleLeaves`, `Fern`, `Vine`, `CocoaBeans` (lot 5), appended to `ItemId`. |
| Block behaviour | Like their oak and tall grass siblings. Log and planks: 10 steps, faster with an axe, drop themselves, wood sound. Leaves: 2 steps, faster with a hoe or a sword, transparent, drop nothing, leaves sound. Fern: like tall grass (cross, no hitbox, 1 step, drops itself). Vine: a plate 1/16 off the face that holds it, oriented by `BlockDirection`, no hitbox, decoration, 1 step, drops itself, placed against the side face aimed at (not on a top or bottom face), no climbing. Cocoa: an 8 × 9 × 8 pod hung 1/16 off a jungle log, oriented, 2 steps, drops 2 cocoa beans (no use yet; the beans are not placeable). |
| Recipes | The 9 wood recipes of oak and spruce, again for jungle: log → 4 planks; sticks, crafting table, chest, and the five wooden tools from jungle planks. |
| Tint | Jungle leaves and vines: new `JUNGLE_LEAVES_COLOR = [0.05, 0.62, 0.05]` (linear, like `GRASS_COLOR`). Fern: `GRASS_COLOR`. Grass keeps its single tint (no per-biome tint). |
| Textures | Pixel Perfection: `block/jungle_log`, `jungle_log_top`, `jungle_planks`, `jungle_leaves`, `fern`, `vine`, `cocoa_stage2`, `item/cocoa_beans` → `JungleLog.png`, `JungleLogTop.png`, `JunglePlanks.png`, `JungleLeaves.png`, `Fern.png`, `Vine.png`, `Cocoa.png`, `items/CocoaBeans.png`. Item icons of tinted blocks look like they do in the world, as for the existing leaves and grass. Credited in the commit and `docs/ressources.txt`. |
| Protocol | `PROTOCOL_ID` 19 → 20 once, in lot 1; later lots of this branch append more variants without another bump (nothing is released in between). `FORMAT_VERSION` stays 5: appending does not change the saved encoding. Saved chunks do not change; a tree rooted in a new chunk is cut against an old one. |

## Architecture

**Climate** (`shared/src/world/biome.rs`, `terrain.rs`, `data.rs`): `BiomeType::Jungle` (appended),
its `get_biome_data` entry, the rule in `determine_biome`, and the depth in `ColumnShape`
(which drops `Eq` if it holds an `f32`, or stores the depth quantised; the plan decides).
Tests: jungle only where hot, wet and below 84; `a_biome_spans_about_a_hundred_and_fifty_blocks`
and `the_ground_never_steps_at_the_border_of_two_biomes` still pass.

**Generation** (`server/src/world/generation.rs`, likely a new `generation/jungle.rs` for the
structures): new `Decoration` variants (`Fern`, `JungleTree`, `Bush`, `GiantTree`), their
blocks, and the ranks of `precedence` (fern and vine with tall grass, jungle leaves with oak
leaves, jungle log with oak log, and cocoa between leaves and logs: a log rooted later must
not leave a pod inside its trunk, as happened 3 times in 67 on seed 42 with cocoa ranked as a log).

- *Height of structures:* instead of one `STRUCTURE_HEIGHT` for every chunk, each `ChunkColumns`
  keeps the highest block anything rooted in it or its margin reaches (`Decoration::height`), and
  the "above everything" test of `generate_chunk` uses it. The global bound used before the
  columns are known becomes the tallest structure (the giant). Jungle trees (up to 10 above the
  surface) need this from lot 2.
- *Margin:* `STRUCTURE_MARGIN` stays 3 for everything but giants. `compute_chunk_columns` also
  looks at the ring from 4 to `GIANT_MARGIN` (6): there it only hashes the giant roll, and
  computes a column's shape only when the roll is below the largest giant chance. The spacing
  check reads the rolls of candidates up to 8 columns further, the same way.
- *Caves:* the four columns of a giant's trunk keep their ground, like any decorated column.
- `spawn_position` keeps its rule (a column with nothing growing around it).
- `server/examples/worldgen_dump.rs` and `scripts/worldgen_preview.py` learn the new blocks and
  the jungle colour.

**Client**: `VoxelShape` cases (jungle log with `JungleLogTop`, tinted jungle leaves, fern as
flora, the vine plate, the cocoa box), sounds, minimap colours (leaves shaded like the others),
item icons. The debug HUD shows `Biome: Jungle` with no change.

## Lots

1. **Jungle wood**: `JungleLog`, `JunglePlanks`, `JungleLeaves` (blocks, items, textures,
   break times, tools, drops, sounds, render, minimap), the 9 recipes, `PROTOCOL_ID` 20. To see:
   `/give jungle_log`, place, break, craft.
2. **The biome**: `BiomeType::Jungle`, climate rule, depth, `Fern` (block and item), tall grass,
   bushes, jungle trees (without vines and cocoa), per-chunk-column structure height, world
   preview tools. To see: a low jungle along the warm coasts, no giants.
3. **Giants**: lazy margin ring, giant trees with branches and spacing, server `bench_`
   benchmarks before and after (nothing measurable outside the jungle). To see: the canopy above
   everything.
4. **Vines**: `Vine` (block, item, plate shape, placement against a side face), generated on
   giants' trunks and crowns and under jungle trees' crowns.
5. **Cocoa**: `Cocoa` block, `CocoaBeans` item, the pod shape, generated on jungle trees.

Cross-cutting tests: on several seeds, every block of every structure lies within its margin
and under its height bound; a chunk generated alone equals the same chunk generated amid its
neighbours; `block_encoding_is_pinned` updated per lot.

Each lot: a plan in `docs/superpowers/plans/`, TDD, `cargo test` and `cargo clippy` clean,
review before the next lot. Nothing merged or pushed without asking the user.
