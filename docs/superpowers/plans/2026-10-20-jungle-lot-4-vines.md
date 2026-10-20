# Jungle Lot 4 (Vines) Implementation Plan

**Goal:** The vine: a block and its item, drawn as a plate 1/16 off the face of the block that
holds it, placed by the player against the side of a block, and generated hanging straight down
from the giants' trunks and crown rims and from under the jungle trees' crowns. To see: curtains
of vines around the giants, a few under the jungle trees, `/give vine` placed on a wall.

**Spec:** `docs/superpowers/specs/2026-10-20-jungle-biome-design.md`, the vine parts of the rows
"Giant tree", "Jungle tree", "New blocks", "New items", "Block behaviour", "Tint", "Textures"
and "Protocol" of "Decisions", the "Generation" and "Client" paragraphs of "Architecture", and
lot 4 of "Lots".

## Global Constraints

- English for code, comments, docs and commits; Conventional Commits, no trailer.
- Doc comments in full sentences, tests named as sentences, sparse comments.
- TDD: each test is written and seen failing (not compiling counts) before the code that
  makes it pass.
- `BlockId` and `ItemId` are append only; `PROTOCOL_ID` stays 20 (bumped once in lot 1).
- Generation stays a pure function of `(seed, coordinates)`; each new random decision takes a
  salt of its own, never one already used.
- `STRUCTURE_MARGIN` stays 3 and `GIANT_MARGIN` 6; `Decoration::height` and `GIANT_HEIGHT` do
  not change; no structure block lies below its root but the giant's sunk trunk.
- No cocoa (lot 5), no climbing.
- The server `bench_generation` and `bench_jungle_generation` run before and after the
  generation change.
- `cargo test`, `cargo clippy -p shared -p client -p server --all-targets -- -D warnings` and
  `cargo fmt --all -- --check` are clean at each commit.

## Decisions (where the spec is silent)

1. **Variants:** `BlockId::Vine` is 32, `ItemId::Vine` 39.
2. **Direction:** a vine's `BlockDirection` names the side of its cell its support lies on:
   `Front` is −Z (north, as in `CLAUDE.md`), `Back` +Z, `Left` −X (west), `Right` +X (east). It
   is the turn the mesher already gives a shape (`rotate_vertices` turns the front of a shape
   toward +X for `Right`). `BlockDirection::offset` gives the step toward that side and
   `BlockDirection::toward` the direction of a horizontal step, in `shared`, for the placement
   and the generation alike.
3. **Shape:** two quads back to back, each with its own normal, since the chunk material culls
   back faces (the flora gets away with one normal for both): for `Front`, the plane
   z = 1/16 across the whole cell. Its face is `Inset`, never culled; the vine is a
   `Decoration`, which hides no face of its neighbours; a dropped vine shows its icon, as any
   shape with an `Inset` face does. Tinted with `JUNGLE_LEAVES_COLOR`.
4. **Turned shapes:** `rotate_vertices` turned a shape about the corner of its cell, not its
   centre, and `push_face` kept the normals unturned: no block ever had another direction than
   `Front`, so nobody saw it. Both turn about the centre now.
5. **Ray box:** a vine has no collision box; the box rays hit is a slab 1/16 thick around its
   plate, on its support's side (`BlockData::get_ray_hitbox`, which turns the vine's box; every
   other block keeps `BlockId::get_ray_hitbox`). A ray hitting a box reports the cell it walked
   into instead of the floor of the hit point: the same for every box inside its cell, and
   right for the vine's, whose edges lie on the cell's.
6. **Placement:** a vine item aimed at a side face goes into the cell in front of it, turned
   toward the aimed block; aimed at a top or bottom face it is refused (`Refusal::NotASide`),
   the item kept and the cell re-stated by the server. Any block may hold it, and it stays when
   its support goes (the spec asks for no support rule). The code is shared: the client predicts
   what the server decides. Other blocks are still placed `Front`.
7. **Sound:** grass, like tall grass and the fern.
8. **Oriented generated blocks:** `Blocks` holds `(position, block, direction)`; everything but
   vines is `Front`. `place` takes the direction.
9. **Jungle tree vines:** each of the four sides of the lowest layer of the crown (`top - 2`)
   has 5 leaves, a corner on two sides; the outward face of each leaf that grew carries a vine
   one time in four, in the cell outside it (3 columns from the trunk), turned toward the leaf,
   1 to 4 long, from the leaf's height down but never below the root (the vertical culling of
   `generate_chunk` relies on nothing growing under the root): on a short tree a long vine stops
   just above the ground.
10. **Giant trunk vines:** each of the 8 faces of the trunk (2 per side) carries a vine one time
    in five, in the cell beside it, turned toward the trunk, 1 to 7 long, from `top - 2`, just
    under the lowest layer of the crown, down.
11. **Giant rim vines:** the leaves of the widest layer of the crown (radius 5.4, at the height
    of the top of the trunk) that grew and have a face toward a cell outside the disk; each such
    face carries a vine one time in three, in that cell, turned toward the leaf, 1 to 7 long,
    from the leaf's height down. The other layers are skipped: their outward cells lie under or
    in the widest one.
12. **One salt per face:** its roll gives both whether a vine hangs there and its length
    (`below(salt, 4 × 4) < 4` gives 1 to 4 for 1 in 4; `below(salt, 5 × 7)` and
    `below(salt, 3 × 7)` for the giant's 1 to 7). Salts: jungle tree `0x440` + face (20),
    trunk `0x460` + face (8), rim `0xC00` + `4 × cell + side` over the 12 × 12 cells of the
    widest layer (576, up to `0xE40`).
13. **Ground:** a vine at or below the surface of its column is dropped when placed into a chunk
    (`generate_chunk_from_columns`). The terrain being a height map and a chain hanging
    straight, that is the chain stopping at its first cell of terrain, so that no vine hangs in a
    cave's mouth below the ground; the columns of the cells inside the chunk, the only ones that
    matter, are all in `ChunkColumns`.
14. **Rank:** the vine ranks with tall grass and the fern (2): leaves, logs and the terrain beat
    it, it beats flowers, and the first placed of two equal ranks stays.
15. **Margins:** the jungle tree's vines reach 3 columns from its root, `STRUCTURE_MARGIN` (was
    2); the widest layer of a giant's crown spans −4 to +5 columns from its root, its vines −5 to
    +6, `GIANT_MARGIN`. Vines hang under leaves: no height bound moves; the giant's lowest vine
    is 7 above its root.
16. **Preview tools:** `worldgen_dump`'s `is_ground` skips vines, `worldgen_preview.py` colours
    them like the jungle leaves, a little lighter.
17. **`CHANGELOG.txt`:** the jungle line mentions the vines.

## Tasks

### Task 1: The vine block and item (`shared`, `client`)
- [x] Tests: `Vine` in `block_encoding_is_pinned` (32), `ALL_BLOCKS`, `item_encoding_is_pinned`
  (39); `the_vine_is_a_decoration_without_collision_broken_at_once` (hitbox, ray box, break
  time, drop, visibility, item type); `a_direction_names_the_side_of_its_step` (offset and
  toward, north is −Z). See them fail.
- [x] The variants, their behaviour, `BlockDirection::offset` and `toward`. See them pass.
- [x] Tests: `the_vine_sounds_like_grass`; `the_vine_is_a_plate_one_sixteenth_off_its_support`
  (voxel.rs: one `Inset` face, two quads with opposite normals, each wound toward its normal,
  at z = 1/16, tinted `JUNGLE_LEAVES_COLOR`); in meshing.rs
  `a_turned_shape_turns_about_the_centre_of_its_cell_with_its_normals` and
  `a_vine_lies_against_the_block_that_holds_it_and_hides_none_of_its_faces` (for each
  direction: the plate's vertices 1/16 off the support, its normals across, the stone's face
  toward the vine drawn). See them fail. The shape, the sound and the turn. See them pass.
- [x] Texture `block/vine.png` → `blocks/Vine.png` (16 × 16).
- [x] Test `the_vine_icon_is_its_block_texture_tinted_as_in_the_world`; see it fail; make
  `items/Vine.png` with a script kept outside the repository; see it pass.
- [x] Credit the texture; commit `feat: the vine`.

### Task 2: Aiming at a vine and placing one (`shared`)
- [x] Tests (raycast.rs): `a_vine_is_hit_through_its_plate_in_its_own_cell` (each direction:
  through the plate, not beside it; from the open side and along the support, the cell is the
  vine's). See them fail. `BlockData::get_ray_hitbox`, the cell of an `Aabb` hit. See them
  pass, with the flower tests.
- [x] Tests (players/blocks.rs): `a_vine_placed_against_a_side_hangs_toward_that_block` (the
  four sides, the direction stored, the item used) and
  `a_vine_aimed_at_a_top_or_bottom_face_is_refused_and_kept` (nothing placed, the item kept,
  the cell re-stated by the server). See them fail. The rule in `place_block`. See them pass.
- [x] Commit `feat(shared): a vine hangs on the side of the block it is placed against`.

### Task 3: Vines in the jungle (`server`)
- [x] Benchmarks before (done: see the commit message).
- [x] `Blocks` with a direction, `place` with it (no behaviour change; every test passes).
- [x] Tests (jungle.rs): `vines_hang_under_one_in_four_of_the_jungle_trees_lowest_edge_leaves`
  (top cell beside an edge leaf of `top - 2`, outward, turned toward it, chains straight down,
  1–4 long, never under the root, about one face in four, every length seen),
  `vines_hang_on_one_giant_trunk_face_in_five_and_under_one_rim_leaf_in_three` (trunk chains
  from `top - 2`, rim chains from a leaf of the widest layer with a face outside its disk,
  turned toward their support, 1–7 long, the shares near 1/5 and 1/3, every length seen); the
  margin tests reach 3 for the jungle tree and keep `GIANT_MARGIN` and the heights for the
  giant. See them fail.
- [x] Tests (generation.rs): the vine in `jungle_blocks_rank_with_their_oak_and_grass_siblings`;
  `a_vine_is_never_placed_at_or_below_the_surface_of_its_column` (`place_structure_block`);
  `vines_hang_from_something_and_never_into_the_ground` (sample chunks and those around a
  giant: vines found, each above the surface of its column, under a vine, leaves or a log, or
  beside leaves or a log on its support's side). See them fail.
- [x] The vines of `jungle_tree_blocks` and `giant_tree_blocks`, the rank, the ground rule. See
  them pass, with the determinism, margin, culling and completeness tests.
- [x] Benchmarks after.

### Task 4: Tools, preview, commit
- [x] `worldgen_dump.rs`, `worldgen_preview.py`.
- [x] Render a slice through a giant with its vines and a close top view; look at them; keep them
  outside the repository (`vines_*`).
- [x] `CHANGELOG.txt`; tests, clippy, fmt; commit `feat: vines in the jungle`.

### Task 5 (optional): A picture of a vine in the game
- [ ] If the e2e harness can show a giant of the seed 42 at a fixed camera cheaply, an ignored
  `UI_SNAPSHOTS=1` snapshot of its vines, looked at; otherwise said so in the report.
