# Jungle Lot 2 (The biome) Implementation Plan

**Goal:** The jungle itself, without its giants, vines and cocoa: `BiomeType::Jungle` where the
climate is hot and wet, its depth in `ColumnShape`, the `Fern` block and item, and a ground
crowded with ferns, tall grass, bushes and jungle trees whose density grows from the border of
the biome to its heart. Each column of chunks keeps the height of what grows on it, so that the
jungle trees, the tallest structures yet, are never cut. To see: a low jungle along the warm
coasts, no giants.

**Spec:** `docs/superpowers/specs/2026-10-20-jungle-biome-design.md`, the rows "Climate",
"Relief", "Ground", "Depth", "Densities" (without the giant), "Jungle tree" (without vines and
cocoa), "Bush", "New blocks" and "New items" (`Fern`), "Block behaviour" (fern), "Tint" and
"Textures" (fern) of "Decisions", the "Climate" and "Generation" paragraphs of "Architecture"
(without the giant's margin ring), and lot 2 of "Lots".

## Global Constraints

- English for code, comments, docs and commits; Conventional Commits, no trailer.
- Doc comments in full sentences, tests named as sentences, sparse comments.
- TDD: each test is written and seen failing (not compiling counts) before the code that
  makes it pass.
- `BlockId` and `ItemId` are append only. `PROTOCOL_ID` stays 20 (bumped once in lot 1).
- Generation stays a pure function of `(seed, coordinates)`: each new random decision of a
  column takes a new salt of `PositionRandom`.
- `STRUCTURE_MARGIN` stays 3: every block of a bush or a jungle tree lies within 3 columns of
  its root.
- No giants (lot 3), no vines (lot 4), no cocoa (lot 5).
- `cargo test`, `cargo clippy --all-targets -- -D warnings` on `shared`, `client` and `server`,
  and `cargo fmt --all -- --check` are clean at each commit. The server `bench_` benchmarks run
  before and after the generation changes.

## Decisions (where the spec is silent)

1. **Order of the variants:** `BiomeType::Jungle` is appended (10), `BlockId::Fern` is 31 and
   `ItemId::Fern` is 38.
2. **Climate constants** `JUNGLE_TEMPERATURE` (0.65) and `JUNGLE_HUMIDITY` (0.55) live in
   `shared/src/world/biome.rs`, next to `OCEAN_HUMIDITY`, with a pure
   `jungle_depth(temperature, humidity)` that is 0 outside the jungle. The jungle is tested
   after the oceans and before the forest: it only takes the wet half of the hot forests.
3. **Depth in `ColumnShape`:** a field `jungle_depth: f32`. `ColumnShape` drops `Eq` (it keeps
   `PartialEq`; nothing hashes it). The mountains, decided from the altitude, get a depth of 0,
   like every other biome.
4. **`Column`** (server) copies the depth from its shape, and the decorations read it there.
5. **Rolls.** Each column rolls once for its trees (`SALT_TREE_ROLL`, as the forest does): a
   jungle tree below `0.03 × depth`, a bush from there to `0.03 × depth + max(0.01, 0.06 ×
   depth)`, so that a column never carries both (each puts a log on its root). Ferns get their
   own roll (`SALT_FERN_ROLL`): a column with a fern carries no tall grass, so that the two never
   claim the same cell; tall grass keeps its roll and grows on `0.10 + 0.12 × depth` of the
   columns without a fern. Lot 3 will put the giant's chance in front of the jungle tree's.
6. **Jungle tree shape:** the trunk is 5 to 9 logs from the block above the surface; its top
   log is the "top". The two layers below the top hold a 5 × 5 square of leaves whose four
   corners each grow one time in two; the top layer holds a 3 × 3 square, and a plus of five
   leaves lies above the top. Its highest leaf is thus `9 + 1` = 10 above the surface, its
   widest 2 columns from the root.
7. **Bush shape:** a jungle log on the block above the surface; around it, on the same layer, a
   diamond of leaves of radius 2 whose eight outermost leaves each grow one time in two; a plus
   of five leaves above; and one leaf above that one time in two. 3 above the surface, 2 columns
   wide.
8. **Salts:** the jungle structures live in `server/src/world/generation/jungle.rs` with salts of
   their own from `0x400`, apart from those of `generation.rs` (to `0x24b`) and of the animals
   (`0x300`). The trunk height of a jungle tree reuses `SALT_TRUNK_HEIGHT`, which means the same
   for every tree.
9. **Structure height:** `Decoration::height` is the highest block a decoration may reach above
   the surface of its column (1 for a plant, 3 for a cactus or a bush, 7 for a tree, 9 for a big
   tree, 10 for a jungle tree). `STRUCTURE_HEIGHT` becomes the largest of them (10) and is only
   used before the columns are known. `ChunkColumns::highest` becomes the highest block of the
   ground or of anything rooted on any of its columns (margin included), which the "above
   everything" return of `generate_chunk` reads as is.
10. **Ranks:** fern with tall grass (2), jungle leaves with oak leaves (3), jungle log with oak
    log (4).
11. **Animals:** no herds in the jungle (`herds` says so explicitly); `may_stand_on` also refuses
    a column under a jungle tree or a bush, where a herd of a neighbouring biome could spill.
12. **Fern item icon:** `items/Fern.png` is the block texture tinted by `GRASS_COLOR` as the GPU
    tints it, the way lot 1 made `items/JungleLeaves.png`, with the same kind of test.
13. **World preview tools:** `worldgen_preview.py` learns `Fern` and the `Jungle` biome colour;
    `worldgen_dump.rs` counts the fern as growing, not ground.
14. **`CHANGELOG.txt`:** one line in the "Unreleased" changes, with the generation commit.

## Tasks

### Task 1: Climate (`shared`)
- [x] Tests (`terrain.rs`): `the_jungle_lies_only_where_hot_and_wet_on_low_land` (seed 42,
  sampled columns: jungle iff temperature > 0.65, humidity > 0.55, below the sea's humidity and
  below 84), `the_jungle_deepens_from_its_border_to_its_heart` (depth 0 outside, small next to a
  non-jungle land column, 1 somewhere inside), `the_jungle_covers_about_six_percent_of_the_world`
  (seed 42, 5–7 % of sampled columns); unit tests of `determine_biome` and `jungle_depth`
  (`biome.rs`). See them fail.
- [x] `BiomeType::Jungle`, its `get_biome_data`, the constants, the rule, `jungle_depth`, the
  field of `ColumnShape`. See them pass, and the biome width, border and mountain tests too.

### Task 2: The fern (`shared`, `client`)
- [x] Tests: `Fern` in `block_encoding_is_pinned` (31), `ALL_BLOCKS`, `item_encoding_is_pinned`
  (38); `the_fern_behaves_like_tall_grass` (hitboxes, break time, drop, visibility, item type).
  See them fail.
- [x] The variants and their behaviour. See them pass.
- [x] Tests: `the_fern_sounds_like_grass` (sounds.rs), `the_fern_is_flora_tinted_like_the_grass`
  (voxel.rs). See them fail. The sound and shape cases. See them pass.
- [x] Texture `block/fern.png` → `blocks/Fern.png` (checked 16×16).
- [x] Test `the_fern_icon_is_its_block_texture_tinted_as_in_the_world`; see it fail; make
  `items/Fern.png` with a script kept outside the repository; see it pass.
- [x] Credit the texture in `docs/ressources.txt`; commit `feat: the fern`.

### Task 3: Structure height per column of chunks (`server`)
- [ ] Benchmarks before.
- [ ] Tests: `every_decoration_stays_under_its_height`, and `a_chunk_column_knows_the_highest_block_rooted_on_it`.
  See them fail.
- [ ] `Decoration::height`, `ChunkColumns::highest` from it, the early return. See them pass,
  with every existing test.

### Task 4: Jungle generation (`server`)
- [ ] Tests: `jungle_structures_stay_within_the_margin_and_under_their_height` (many seeds and
  columns of a synthetic jungle of depth 1 and of depth 0), `the_jungle_tree_has_its_trunk_and_crown`,
  `the_bush_is_a_log_under_a_mound_of_leaves`, `the_jungle_grows_denser_toward_its_heart`
  (counts per decoration on synthetic columns of depth 0 and 1 against the spec's densities),
  `a_jungle_chunk_generated_alone_equals_it_generated_amid_its_neighbours`, jungle chunks in
  `chunks_without_noise_are_the_ones_the_noise_makes`, a jungle tree across a chunk border is
  complete, the new ranks in `a_higher_rank_wins_whatever_the_order`, no herds in the jungle and
  no animal under a jungle tree or a bush. See them fail.
- [ ] `Decoration::{Fern, JungleTree, Bush}`, `generation/jungle.rs`, the densities, the ranks,
  `may_stand_on`. See them pass.
- [ ] Benchmarks after.

### Task 5: Tools, preview, changelog and commit
- [ ] `worldgen_preview.py` (`Fern`, `Jungle`), `worldgen_dump.rs` (`is_ground`).
- [ ] Render a jungle area of seed 42 with `worldgen_dump` and `worldgen_preview.py`, look at
  it, keep it outside the repository.
- [ ] `CHANGELOG.txt`.
- [ ] `cargo test`, clippy and fmt; commit `feat: the jungle biome`.
