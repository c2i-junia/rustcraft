# Jungle Lot 3 (Giants) Implementation Plan

**Goal:** The giant jungle trees: a 2 × 2 trunk of jungle logs 16 to 25 blocks high under a
wide crown of four frayed disks of jungle leaves, with 2 to 4 branches ending in blobs of
leaves, rooted only in the deep half of the jungle and never two within 8 columns of each
other. Their crowns reach 6 columns from their root, further than the margin of every other
structure: chunks look at a lazy ring of columns around that margin, which costs nothing
measurable outside the jungle. To see: the canopy above everything.

**Spec:** `docs/superpowers/specs/2026-10-20-jungle-biome-design.md`, the rows "Densities" (the
giant), "Giant tree" (without the vines) and "Spacing" of "Decisions", the "Generation"
paragraph of "Architecture" (height of structures, margin, caves, spawn), and lot 3 of "Lots".

## Global Constraints

- English for code, comments, docs and commits; Conventional Commits, no trailer.
- Doc comments in full sentences, tests named as sentences, sparse comments.
- TDD: each test is written and seen failing (not compiling counts) before the code that
  makes it pass.
- Generation stays a pure function of `(seed, coordinates)`: whether a giant roots on a column
  depends on the seed and on the shapes of the columns around, never on which chunk asks.
- `STRUCTURE_MARGIN` stays 3 for every structure but the giant; `GIANT_MARGIN` is 6. Every
  block of a giant lies within 6 columns of its root and at most 27 above its surface.
- No vines (lot 4), no cocoa (lot 5). No new block, item or protocol change.
- The server `bench_` benchmarks run before and after; nothing measurable changes outside the
  jungle.
- `cargo test`, `cargo clippy --all-targets -- -D warnings` on `shared`, `client` and `server`,
  and `cargo fmt --all -- --check` are clean at each commit.

## Decisions (where the spec is silent)

1. **One roll for every tree of the jungle.** The giant takes the front of the tree roll
   (`SALT_TREE_ROLL`), as lot 2 announced: below `giant = 0.004 × smoothstep(0.5, 1, depth)` a
   column is a giant *candidate*, then come the jungle tree (`giant + 0.03 × depth`) and the
   bush. The jungle trees and bushes move, their densities stay those of the spec. A candidate
   no giant roots on grows nothing in that slot.
2. **Candidate.** A column is a candidate when its roll is below the giant chance of its depth,
   it lies in the jungle above `SEA_LEVEL + 1` (its surface is then grass), and its `+x`, `+z`
   and `+x+z` neighbours, of any biome, are dry too (above `SEA_LEVEL + 1`) and within 2 blocks
   of its height. All of it is read from `ColumnShape`s.
3. **Spacing.** A candidate roots a giant when no other candidate within 8 columns (Chebyshev)
   has a smaller `(roll, x, z)`. The rule is not recursive (a candidate beaten by one that is
   itself beaten still grows nothing), so that it stays local: it reads the rolls of the 288
   columns around, and the shapes only of those whose roll is smaller (`jungle::giant_roots`).
   The shape of a column is read only when its roll is below `GIANT_CHANCE` (0.004), the
   largest giant chance.
4. **Trunk.** Height `h = 16 + below(SALT_TRUNK_HEIGHT, 10)`, counted from the root (the block
   above the surface): logs on the four columns from `root.y - 3` to `root.y + h - 1`, the top.
5. **Crown.** Four layers from `top - 1` to `top + 2`, of radii 4.6, 5.4, 4.4 and 2.6, measured
   from the trunk's centre (the corner the four trunk columns share) to the centre of each cell.
   The rim, the leaves farther than `radius - 1`, is frayed: each of its leaves grows one time
   in two, with a salt of its own. The highest leaf, always present, is `top + 2`, 27 above the
   surface at most; the crown spans −4 to +5 columns from the root.
6. **Branches.** `2 + below(3)` branches, in distinct directions: a random first one among the
   four sides, the others spread around (`first + k × 4 / count`). Each starts from one of the
   two trunk columns of its side, at random, `h / 2` to `4 × h / 5` above the root; it is
   `2 + below(2)` logs long, each log one column further out and one block higher than the one
   before. Its tip carries a blob: a disk of radius 2.4 on the tip's layer and one of 1.5 above,
   not frayed. A branch reaches −5 or +6 columns from the root, 24 above the surface at most.
7. **The four trunk columns** (`Column::giant`: `Root` or `Beside`) carry the giant only: no
   plant, tree or bush of their own (the trunk would hide them). Their caves stay under the foot
   of the trunk, `cave_ceiling ≤ height - 5` (the foot is 2 under the root's surface and the
   columns within 2 of it), so that no log fills a cave. Animals never appear on them, and the
   spawn needs a column with no trunk around it either.
8. **Lazy ring.** `compute_chunk_columns` decides the giants of the square from
   `-GIANT_MARGIN` to `CHUNK_SIZE + GIANT_MARGIN`; those rooted in the ring outside
   `STRUCTURE_MARGIN` are kept in `ChunkColumns::ring_giants` (their root only). Shapes come from
   the grid already computed for the margin and its steepness ring, or from the noise for the
   rare columns beyond it. `generate_chunk_from_columns` visits the whole square in world
   coordinate order, the ring giants in their place, so that equal ranks resolve the same way in
   every chunk.
9. **Height.** `Decoration::GiantTree.height()` is 27 and `STRUCTURE_HEIGHT` follows it.
   `ChunkColumns::highest` counts the ring giants. A giant reaches 3 below its root: the chunks
   whose top lies there expand its blocks too.
10. **Salts** (`generation/jungle.rs`): the branch count `0x420`, the first direction `0x421`,
    each branch 4 from `0x430`, each rim leaf of the crown from `0x800` (one per cell of the
    12 × 12 × 4 box around the trunk).
11. **Spawn:** the `open` closure of `spawn_position` becomes the method `spawn_height`, so that
    a test can ask it about a giant's columns.
12. **Preview tools:** `worldgen_dump` generates up to 32 blocks above the highest ground of a
    column of chunks (was 16), so that the giants' crowns are in the maps.
13. **Benchmark:** `bench_jungle_generation`, the render distance of `bench_generation` around a
    chunk of the seed 42's jungle, `(-17, 4, 4)`, added before the change so that it measures
    both. Its median "air" chunk is the cost of the chunk columns there, which every chunk of a
    fresh generator recomputes.
14. **`CHANGELOG.txt`:** the jungle line mentions the giant trees.

## Tasks

### Task 1: Benchmarks before
- [x] `bench_jungle_generation` in `server/src/world/bench.rs`; commit
  `test(server): a benchmark of the generation in the jungle`.
- [x] Run every `bench_` in release, three times, with no other cargo process; note the numbers.

### Task 2: The giant's shape (`generation/jungle.rs`)
- [x] Tests: `the_giant_is_a_two_by_two_trunk_under_four_frayed_disks` (trunk, heights 16–25 all
  seen, each crown layer between the disk of `radius - 1` and the disk of `radius`, rims frayed),
  `the_giant_has_two_to_four_branches_rising_outward_to_blobs` (distinct sides, length 2–3, one
  out and one up per log, start between `h / 2` and `4h / 5`, both disks of the blob around the
  tip), `giants_stay_within_their_margin_and_under_their_height` (many seeds and roots: within
  `GIANT_MARGIN`, from 3 below the root to 27 above the surface, both bounds reached). See them
  fail.
- [x] `giant_tree_blocks`, split into trunk, branches and crown (lot 4 adds the vines there).
  See them pass.

### Task 3: Where giants root (`generation/jungle.rs`)
- [x] Tests on synthetic shapes: `a_giant_roots_only_in_the_deep_jungle_on_dry_level_ground`
  (shallow depth, water, a step of 3 or a beach next to the root refuse it),
  `two_giants_never_root_within_eight_columns`, `the_smallest_roll_among_candidates_wins`,
  `a_column_shape_is_read_only_under_the_largest_giant_chance` (a counting closure),
  `giants_are_about_as_dense_as_the_spacing_allows` (a flat deep jungle). See them fail.
- [x] `GIANT_CHANCE`, `giant_chance`, `giant_roots`. See them pass.

### Task 4: Giants in the world (`generation.rs`, `animals.rs`)
- [x] Tests: `the_giant_takes_the_front_of_the_tree_roll` (nothing in the giant's slot of a
  candidate no giant roots on, the giant alone on its columns),
  `giants_grow_in_the_jungle_of_the_seed_42_never_within_eight_columns` (grass under the root,
  the three other columns beside it),
  `a_giants_trunk_keeps_its_ground_and_fills_the_gaps_above_it` (caves under the foot, logs over
  the lower columns, one of them in the chunk under the root's, at (819, 1186)),
  `a_giant_rooted_in_the_ring_of_a_chunk_reaches_into_it` (seed 42, found programmatically: in
  `ring_giants`, every block in the chunk, alone and amid its neighbours), giant chunks in
  `chunks_without_noise_are_the_ones_the_noise_makes`, `sample_chunks` and
  `a_chunk_column_knows_the_highest_block_rooted_on_it` (every giant within `GIANT_MARGIN`
  counted), `no_structure_grows_higher_than_its_bound` reaching the giant's 27,
  `plants_and_trunks_stand_on_something` letting a branch lean on the log before it,
  `no_animal_appears_on_a_giants_trunk`, `no_one_spawns_beside_a_giant`. See them fail.
- [x] `Decoration::GiantTree`, `Column::giant`, the tree roll, `cave_ceiling`, the ring of
  `ChunkColumns`, `highest`, the culling, `STRUCTURE_HEIGHT`, `may_stand_on`, `spawn_height`.
  See them pass, with every existing test.

### Task 5: Tools, preview, benchmarks after, commit
- [x] `worldgen_dump.rs` (32 above the ground).
- [x] Render the jungle of seed 42 with its giants (map, oblique view, a slice through a giant),
  look at it, keep it outside the repository.
- [x] Benchmarks after; giants per km² on the seed 42's jungle.
- [x] `CHANGELOG.txt`; `cargo test`, clippy and fmt; commit `feat: giant trees in the jungle`.
